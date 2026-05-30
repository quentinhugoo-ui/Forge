// @ts-nocheck
// INGEN Render — WebGPU compute-driven raymarcher for Banger.
// Phase 0 (INGEN COMPUTE §18-19) : ops buffer → compute pass → present.
// Replaces the WebGL2 fragment-SDF path (catalog.ts FS_SDF / VS_SDF).
// Doctrine : 1 device, 1 context, 1 compute pipeline, 1 ops buffer, 1 present.
// No raster middleman, no fullscreen-quad VS, no per-frame shader recompile.

import {
  OP_SPHERE, OP_BOX, OP_TORUS, OP_CAPSULE, OP_ROUNDED_BOX,
  OP_UNION, OP_INTERSECT, OP_DIFF, OP_SMIN, OP_SVDAG,
} from "./scenes.js";

// Layout-stable opcodes shipped to WGSL. Mirrors scenes.ts 1:1.
// Format per op = 2 * vec4<f32> = 32 bytes :
//   slot0 = (op_code, p0, p1, p2)
//   slot1 = (p3, p4, p5, k)
// Same convention as the legacy FS_SDF stack machine — agents and KASM
// programs that already produce ops for the WebGL2 path stay valid.

const MAX_OPS = 128;
const OPS_BYTES = 16 /* count + pad */ + MAX_OPS * 32;

const WGSL = `
struct Camera {
  pos:           vec3<f32>,
  tanHalfFovY:   f32,
  fwd:           vec3<f32>,
  _p0:           f32,
  right:         vec3<f32>,
  _p1:           f32,
  up:            vec3<f32>,
  _p2:           f32,
  resolution:    vec2<f32>,
  _p3:           vec2<f32>,
};

struct Ops {
  count: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
  data:  array<vec4<f32>, ${MAX_OPS * 2}>,
};

// INGEN COMPUTE §18 Pillar A : Sparse Voxel DAG storage (Phase 5).
// Layout : header [root, dim, depth, _pad] + per-node 9 u32
// (childmask + 8 child indices). Sentinels : 0 = SVDAG_EMPTY, 1 = SVDAG_FULL.
// The buffer is always bound — an "empty SVDAG" is a 4-word header
// with root = 0, so the traverser short-circuits without crashing.
struct Svdag {
  root:  u32,
  dim:   u32,
  depth: u32,
  _pad:  u32,
  nodes: array<u32>,
};

@group(0) @binding(0) var<uniform>          cam: Camera;
@group(0) @binding(1) var<storage, read>    ops: Ops;
@group(0) @binding(2) var                   outTex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<storage, read>    svdag: Svdag;

fn sd_sphere(p: vec3<f32>, r: f32) -> f32 { return length(p) - r; }

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
  let q = abs(p) - b;
  return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sd_torus(p: vec3<f32>, big_r: f32, small_r: f32) -> f32 {
  let q = vec2<f32>(length(p.xy) - big_r, p.z);
  return length(q) - small_r;
}

fn sd_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
  let pa = p - a;
  let ba = b - a;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
  return length(pa - ba * h) - r;
}

fn sd_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
  let q = abs(p) - b + vec3<f32>(r);
  return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn smin_k(a: f32, b: f32, k: f32) -> f32 {
  let kk = max(k, 1e-4);
  let h = clamp(0.5 + 0.5 * (b - a) / kk, 0.0, 1.0);
  return mix(b, a, h) - kk * h * (1.0 - h);
}

// SVDAG traversal — sample a world-space point p, given the voxel grid
// origin (lower corner) and side length 'span' in world units. Returns
// a signed distance suitable for the raymarcher : negative when inside
// an occupied voxel, positive (~voxel_size) when outside or in an empty
// cell. Caps at 'span' to keep raymarch steps bounded outside the AABB.
fn sd_svdag(p: vec3<f32>, origin: vec3<f32>, span: f32, root_override: u32) -> f32 {
  if (span <= 0.0) { return 1.0e6; }
  let dim = svdag.dim;
  if (dim == 0u) { return 1.0e6; }
  // Analytical box SDF on the AABB — when outside it dominates and steers
  // the raymarcher straight to the SVDAG bounding box.
  let half = vec3<f32>(span * 0.5);
  let centre = origin + half;
  let q = abs(p - centre) - half;
  let box_d = length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
  if (box_d > 1e-4) { return box_d; }

  // Map p into voxel index space.
  let voxel_size = span / f32(dim);
  let local = (p - origin) / voxel_size;
  let cx = u32(clamp(floor(local.x), 0.0, f32(dim) - 1.0));
  let cy = u32(clamp(floor(local.y), 0.0, f32(dim) - 1.0));
  let cz = u32(clamp(floor(local.z), 0.0, f32(dim) - 1.0));

  var root_idx: u32 = svdag.root;
  if (root_override != 0u) { root_idx = root_override; }
  var idx: u32 = root_idx;
  var size: u32 = dim;
  var px: u32 = cx;
  var py: u32 = cy;
  var pz: u32 = cz;
  // Bounded loop — log2(dim) iterations max for any supported size.
  for (var i: u32 = 0u; i < 16u; i = i + 1u) {
    if (size <= 1u) { break; }
    if (idx == 0u) { return voxel_size * 0.5; } // EMPTY
    if (idx == 1u) { return -voxel_size * 0.5; } // FULL leaf
    let half_size = size / 2u;
    let ox = select(0u, 1u, px >= half_size);
    let oy = select(0u, 1u, py >= half_size);
    let oz = select(0u, 1u, pz >= half_size);
    let oct = ox | (oy << 1u) | (oz << 2u);
    px = px - ox * half_size;
    py = py - oy * half_size;
    pz = pz - oz * half_size;
    // The 'nodes' array starts at byte 16 of the buffer (after the 4-word
    // header), so its index 0 is the first u32 of the first node body. A
    // node at pool index 'idx' (>=2) occupies words (idx-2)*9 .. (idx-2)*9+8 ;
    // the child for octant 'oct' lives at +1 + oct (slot 0 = childmask).
    let off = (idx - 2u) * 9u + 1u + oct;
    idx = svdag.nodes[off];
    size = half_size;
  }
  if (idx == 1u) { return -voxel_size * 0.5; }
  return voxel_size * 0.5;
}

fn scene(p: vec3<f32>) -> f32 {
  var stack: array<f32, 16>;
  var sp: i32 = 0;
  let n = ops.count;
  for (var i: u32 = 0u; i < n; i = i + 1u) {
    let a = ops.data[i * 2u];
    let b = ops.data[i * 2u + 1u];
    let op = u32(a.x + 0.5);
    if (op == ${OP_SPHERE}u) {
      stack[sp] = sd_sphere(p - a.yzw, b.x);
      sp = sp + 1;
    } else if (op == ${OP_BOX}u) {
      stack[sp] = sd_box(p - a.yzw, b.xyz);
      sp = sp + 1;
    } else if (op == ${OP_TORUS}u) {
      stack[sp] = sd_torus(p - a.yzw, b.x, b.y);
      sp = sp + 1;
    } else if (op == ${OP_CAPSULE}u) {
      stack[sp] = sd_capsule(p, a.yzw, b.xyz, b.w);
      sp = sp + 1;
    } else if (op == ${OP_ROUNDED_BOX}u) {
      stack[sp] = sd_rounded_box(p - a.yzw, b.xyz, b.w);
      sp = sp + 1;
    } else if (op == ${OP_UNION}u) {
      sp = sp - 1;
      stack[sp - 1] = min(stack[sp - 1], stack[sp]);
    } else if (op == ${OP_INTERSECT}u) {
      sp = sp - 1;
      stack[sp - 1] = max(stack[sp - 1], stack[sp]);
    } else if (op == ${OP_DIFF}u) {
      sp = sp - 1;
      stack[sp - 1] = max(stack[sp - 1], -stack[sp]);
    } else if (op == ${OP_SMIN}u) {
      sp = sp - 1;
      stack[sp - 1] = smin_k(stack[sp - 1], stack[sp], b.w);
    } else if (op == ${OP_SVDAG}u) {
      // §18 Pillar A : a.yzw = world-space origin, b.x = side length
      // (world units), b.y = root override (0 = use svdag header root).
      stack[sp] = sd_svdag(p, a.yzw, b.x, u32(b.y + 0.5));
      sp = sp + 1;
    }
  }
  if (sp <= 0) { return 1.0e6; }
  return stack[0];
}

fn normal(p: vec3<f32>) -> vec3<f32> {
  let e = vec2<f32>(0.001, 0.0);
  return normalize(vec3<f32>(
    scene(p + e.xyy) - scene(p - e.xyy),
    scene(p + e.yxy) - scene(p - e.yxy),
    scene(p + e.yyx) - scene(p - e.yyx)
  ));
}

// Analytical sub-pixel grid (Phase 2 will extend with axes + fade).
// Distance to nearest grid line in world XZ plane, sampled with screen-space
// derivative for crisp 1-pixel lines at any zoom — no texture, no MSAA needed.
fn grid_xz(p: vec3<f32>, step: f32) -> f32 {
  let q = abs(fract(p.xz / step - 0.5) - 0.5) * step;
  return min(q.x, q.y);
}

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let dims = textureDimensions(outTex);
  if (gid.x >= dims.x || gid.y >= dims.y) { return; }

  let res = vec2<f32>(f32(dims.x), f32(dims.y));
  let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5)) / res * 2.0 - 1.0;
  let aspect = res.x / res.y;
  let dir = normalize(
    cam.fwd
    + cam.right * (uv.x * aspect * cam.tanHalfFovY)
    + cam.up    * (uv.y * cam.tanHalfFovY)
  );

  // Sphere-trace.
  var t: f32 = 0.0;
  var hit: bool = false;
  for (var i: u32 = 0u; i < 128u; i = i + 1u) {
    let p = cam.pos + dir * t;
    let d = scene(p);
    if (d < 0.0008 * max(t, 1.0)) { hit = true; break; }
    if (t > 200.0) { break; }
    t = t + d;
  }

  var col = vec3<f32>(0.035, 0.035, 0.045);

  // Ground-plane intersection for analytical grid (Phase 0 minimal grid).
  // Solves cam.pos.y + dir.y * tg = 0  → tg.
  if (abs(dir.y) > 1e-4) {
    let tg = -cam.pos.y / dir.y;
    if (tg > 0.0 && (!hit || tg < t)) {
      let p = cam.pos + dir * tg;
      let g = grid_xz(p, 1.0);
      let w = fwidth(g);
      let line = 1.0 - smoothstep(0.0, w * 1.5, g);
      let fade = exp(-tg * 0.012);
      col = mix(col, vec3<f32>(0.45, 0.46, 0.50), line * fade * 0.8);
      // Red X axis, green Z axis (sub-pixel).
      let ax = abs(p.z);
      let az = abs(p.x);
      let wx = fwidth(ax);
      let wz = fwidth(az);
      col = mix(col, vec3<f32>(0.90, 0.25, 0.25), (1.0 - smoothstep(0.0, wx * 1.5, ax)) * fade);
      col = mix(col, vec3<f32>(0.25, 0.85, 0.40), (1.0 - smoothstep(0.0, wz * 1.5, az)) * fade);
    }
  }

  if (hit) {
    let p = cam.pos + dir * t;
    let n = normal(p);
    let l = normalize(vec3<f32>(0.45, 0.85, 0.30));
    let diff = max(dot(n, l), 0.0);
    let rim = pow(1.0 - max(dot(n, -dir), 0.0), 2.0);
    col = vec3<f32>(0.78, 0.80, 0.84) * (0.18 + 0.82 * diff) + vec3<f32>(0.20) * rim;
  }

  textureStore(outTex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(col, 1.0));
}
`;

export interface IngenCamera {
  pos: [number, number, number];
  fwd: [number, number, number];
  right: [number, number, number];
  up: [number, number, number];
  tanHalfFovY: number;
}

// FNV-1a 32-bit hash over a Uint32 stream. Fast inline loop, no deps.
// Used by INGEN COMPUTE §19 Phase 3 (KASM frame cache) — collisions are
// tolerable here : worst case a stale frame is shown for ~16 ms before
// the next mutation forces a fresh dispatch.
function fnv1a32(view: Uint32Array): number {
  let h = 0x811c9dc5 >>> 0;
  for (let i = 0; i < view.length; i += 1) {
    h = ((h ^ (view[i] >>> 0)) >>> 0);
    h = (Math.imul(h, 0x01000193) >>> 0);
  }
  return h >>> 0;
}

export interface IngenStats {
  /** Total render() calls since last reset. */
  frames: number;
  /** Calls that skipped the compute dispatch thanks to the frame cache. */
  hits: number;
  /** Calls that ran a full compute pass. */
  misses: number;
  /** hits / frames, in [0, 1]. */
  hitRatio: number;
}

export class IngenRender {
  readonly canvas: HTMLCanvasElement;
  private device: GPUDevice | null = null;
  private context: GPUCanvasContext | null = null;
  private pipeline: GPUComputePipeline | null = null;
  private camBuffer: GPUBuffer | null = null;
  private opsBuffer: GPUBuffer | null = null;
  private svdagBuffer: GPUBuffer | null = null;
  private svdagCapacity = 0;
  private outTexture: GPUTexture | null = null;
  private outView: GPUTextureView | null = null;
  private bindGroup: GPUBindGroup | null = null;
  private format: GPUTextureFormat = "rgba8unorm";
  private width = 0;
  private height = 0;
  // KASM frame cache (INGEN COMPUTE §19 Phase 3). Frame hash = (opsHash,
  // camHash, dims). If unchanged from the previous render() the compute
  // dispatch is skipped and the persistent storage texture is blitted
  // straight to the swap chain. In an idle viewport this saturates at
  // 100 % hit ; in orbit/edit it drops to 0 % which is correct (every
  // pixel changes anyway, tile-level caching would buy nothing without
  // a spatial index — out of scope for Phase 3).
  private opsHash = 0;
  private dimsKey = 0;
  private cachedCamHash = 0;
  private cachedOpsHash = 0;
  private cachedDimsKey = 0;
  private cacheValid = false;
  private stats: IngenStats = { frames: 0, hits: 0, misses: 0, hitRatio: 0 };

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
  }

  static supported(): boolean {
    return typeof navigator !== "undefined" && !!(navigator as any).gpu;
  }

  async init(): Promise<boolean> {
    const nav = navigator as any;
    if (!nav?.gpu) {
      console.warn("[ingen-render] WebGPU unavailable in this runtime");
      return false;
    }
    const adapter = await nav.gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!adapter) {
      console.warn("[ingen-render] no GPUAdapter");
      return false;
    }
    this.device = await adapter.requestDevice();
    const ctx = this.canvas.getContext("webgpu") as GPUCanvasContext | null;
    if (!ctx) {
      console.warn("[ingen-render] canvas.getContext('webgpu') returned null");
      return false;
    }
    this.context = ctx;
    // Force "rgba8unorm" so the storage compute texture can be blitted to the
    // swap chain via copyTextureToTexture (matching format required). This is
    // the most direct path : 1 compute → 1 copy → present. No fragment blit
    // shader, no second pipeline.
    this.format = "rgba8unorm";
    this.context.configure({
      device: this.device,
      format: this.format,
      alphaMode: "premultiplied",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_DST,
    });

    const module = this.device.createShaderModule({ code: WGSL, label: "ingen-render-wgsl" });
    this.pipeline = this.device.createComputePipeline({
      label: "ingen-render-pipeline",
      layout: "auto",
      compute: { module, entryPoint: "cs_main" },
    });

    this.camBuffer = this.device.createBuffer({
      size: 96, // Camera struct, padded to 16-byte multiples
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      label: "ingen-cam",
    });
    this.opsBuffer = this.device.createBuffer({
      size: OPS_BYTES,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-ops",
    });
    // SVDAG buffer starts as an "empty SVDAG" header — root=0 makes the
    // WGSL traverser short-circuit to "no occupancy" without UB. Grows
    // on the first uploadSvdag() call.
    this.svdagCapacity = 64; // 4 header words + a few spare bodies, all zero.
    this.svdagBuffer = this.device.createBuffer({
      size: this.svdagCapacity * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-svdag",
    });
    this.device.queue.writeBuffer(this.svdagBuffer, 0, new Uint32Array([0, 0, 0, 0]));

    this.resize(this.canvas.width || 1, this.canvas.height || 1);
    return true;
  }

  resize(width: number, height: number): void {
    if (!this.device) return;
    const w = Math.max(1, width | 0);
    const h = Math.max(1, height | 0);
    if (w === this.width && h === this.height && this.outTexture) return;
    this.width = w;
    this.height = h;
    // Resize destroys the storage texture, so any cached frame is gone —
    // invalidate the KASM cache to force a fresh compute on the next render.
    this.cacheValid = false;
    this.dimsKey = ((w & 0xffff) | ((h & 0xffff) << 16)) >>> 0;
    this.outTexture?.destroy?.();
    this.outTexture = this.device.createTexture({
      label: "ingen-out",
      size: { width: w, height: h },
      format: "rgba8unorm",
      usage:
        GPUTextureUsage.STORAGE_BINDING |
        GPUTextureUsage.COPY_SRC |
        GPUTextureUsage.TEXTURE_BINDING,
    });
    this.outView = this.outTexture.createView();
    this.bindGroup = this.device.createBindGroup({
      label: "ingen-bg",
      layout: this.pipeline!.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.camBuffer! } },
        { binding: 1, resource: { buffer: this.opsBuffer! } },
        { binding: 2, resource: this.outView! },
        { binding: 3, resource: { buffer: this.svdagBuffer! } },
      ],
    });
  }

  /**
   * Upload a packed SVDAG buffer (matches `Svdag::packed` from `src/svdag.rs`).
   * The buffer's first 4 u32 are the header [root, dim, depth, _pad] ; the
   * remainder is a flat 9-u32-per-node pool. Pass the result of
   * `Svdag::from_occupancy(...).packed` straight through. Calling with an
   * empty Uint32Array resets the SVDAG to its empty-header state.
   */
  uploadSvdag(packed: Uint32Array): void {
    if (!this.device) return;
    const required = Math.max(4, packed.length);
    if (required > this.svdagCapacity) {
      this.svdagBuffer?.destroy?.();
      // Round up to a power of two so growth doesn't thrash reallocation.
      let cap = this.svdagCapacity;
      while (cap < required) cap *= 2;
      this.svdagCapacity = cap;
      this.svdagBuffer = this.device.createBuffer({
        size: cap * 4,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-svdag",
      });
      // Bind group references the old buffer — recreate it.
      if (this.pipeline && this.outView && this.camBuffer && this.opsBuffer) {
        this.bindGroup = this.device.createBindGroup({
          label: "ingen-bg",
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: this.camBuffer } },
            { binding: 1, resource: { buffer: this.opsBuffer } },
            { binding: 2, resource: this.outView },
            { binding: 3, resource: { buffer: this.svdagBuffer! } },
          ],
        });
      }
    }
    if (packed.length === 0) {
      this.device.queue.writeBuffer(this.svdagBuffer!, 0, new Uint32Array([0, 0, 0, 0]));
    } else {
      this.device.queue.writeBuffer(this.svdagBuffer!, 0, packed);
    }
    // SVDAG changed → invalidate the frame cache so the next render
    // dispatches compute instead of replaying a stale framebuffer.
    this.cacheValid = false;
  }

  /**
   * Write ops to the storage buffer. `ops` is a flat Float32Array laid out
   * as repeating 8-float records (mirrors WebGL2 uOps[i*2..i*2+1]).
   * `count` is the number of OPS (not floats).
   */
  uploadOps(ops: Float32Array, count: number): void {
    if (!this.device || !this.opsBuffer) return;
    const safeCount = Math.max(0, Math.min(MAX_OPS, count | 0));
    const header = new Uint32Array([safeCount, 0, 0, 0]);
    this.device.queue.writeBuffer(this.opsBuffer, 0, header);
    // Hash the active ops region (header + payload) so render() can skip
    // dispatch when the scene hasn't moved.
    const floatsNeeded = safeCount * 8;
    const u32 = new Uint32Array(2 + floatsNeeded);
    u32[0] = safeCount;
    u32[1] = 0;
    if (safeCount > 0) {
      const view = ops.byteLength >= floatsNeeded * 4
        ? new Float32Array(ops.buffer, ops.byteOffset, floatsNeeded)
        : ops;
      const opsU32 = new Uint32Array(view.buffer, view.byteOffset, floatsNeeded);
      u32.set(opsU32, 2);
      this.device.queue.writeBuffer(this.opsBuffer, 16, view);
    }
    this.opsHash = fnv1a32(u32);
  }

  render(cam: IngenCamera): void {
    if (!this.device || !this.context || !this.pipeline || !this.bindGroup) return;
    const w = this.width;
    const h = this.height;

    // Hash camera before writing it to the UBO — saves a writeBuffer on hits.
    const camU = new Float32Array(24);
    camU[0] = cam.pos[0]; camU[1] = cam.pos[1]; camU[2] = cam.pos[2]; camU[3] = cam.tanHalfFovY;
    camU[4] = cam.fwd[0]; camU[5] = cam.fwd[1]; camU[6] = cam.fwd[2]; camU[7] = 0;
    camU[8] = cam.right[0]; camU[9] = cam.right[1]; camU[10] = cam.right[2]; camU[11] = 0;
    camU[12] = cam.up[0]; camU[13] = cam.up[1]; camU[14] = cam.up[2]; camU[15] = 0;
    camU[16] = w; camU[17] = h; camU[18] = 0; camU[19] = 0;
    camU[20] = 0; camU[21] = 0; camU[22] = 0; camU[23] = 0;
    const camHash = fnv1a32(new Uint32Array(camU.buffer));

    // KASM frame cache : a hit needs camera + ops + dims all unchanged
    // since the last successful compute dispatch.
    const hit = this.cacheValid
      && camHash === this.cachedCamHash
      && this.opsHash === this.cachedOpsHash
      && this.dimsKey === this.cachedDimsKey;

    this.stats.frames += 1;
    const encoder = this.device.createCommandEncoder({ label: "ingen-encoder" });

    if (!hit) {
      this.device.queue.writeBuffer(this.camBuffer!, 0, camU);
      const pass = encoder.beginComputePass({ label: "ingen-pass" });
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.bindGroup);
      const gx = Math.ceil(w / 8);
      const gy = Math.ceil(h / 8);
      pass.dispatchWorkgroups(gx, gy, 1);
      pass.end();
      this.cachedCamHash = camHash;
      this.cachedOpsHash = this.opsHash;
      this.cachedDimsKey = this.dimsKey;
      this.cacheValid = true;
      this.stats.misses += 1;
    } else {
      this.stats.hits += 1;
    }

    // Blit storage → swap-chain. Always required because each frame has
    // its own swap-chain texture ; the storage texture itself is persistent.
    encoder.copyTextureToTexture(
      { texture: this.outTexture! },
      { texture: this.context.getCurrentTexture() },
      { width: w, height: h, depthOrArrayLayers: 1 },
    );
    this.device.queue.submit([encoder.finish()]);

    this.stats.hitRatio = this.stats.frames > 0 ? this.stats.hits / this.stats.frames : 0;
  }

  /** Snapshot of frame-cache counters. Cheap, can be polled per frame. */
  getStats(): IngenStats {
    return { ...this.stats };
  }

  /** Reset hit/miss counters (HUD reset, benchmark window). */
  resetStats(): void {
    this.stats = { frames: 0, hits: 0, misses: 0, hitRatio: 0 };
  }

  destroy(): void {
    this.outTexture?.destroy?.();
    this.camBuffer?.destroy?.();
    this.opsBuffer?.destroy?.();
    this.svdagBuffer?.destroy?.();
    this.outTexture = null;
    this.outView = null;
    this.camBuffer = null;
    this.opsBuffer = null;
    this.svdagBuffer = null;
    this.svdagCapacity = 0;
    this.pipeline = null;
    this.bindGroup = null;
    this.context = null;
    this.device = null;
  }
}
