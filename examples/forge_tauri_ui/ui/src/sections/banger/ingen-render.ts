// @ts-nocheck
// INGEN Render — WebGPU compute-driven raymarcher for Banger.
// Phase 0 (INGEN COMPUTE §18-19) : ops buffer → compute pass → present.
// Replaces the WebGL2 fragment-SDF path (catalog.ts FS_SDF / VS_SDF).
// Doctrine : 1 device, 1 context, 1 compute pipeline, 1 ops buffer, 1 present.
// No raster middleman, no fullscreen-quad VS, no per-frame shader recompile.

import {
  OP_SPHERE, OP_BOX, OP_TORUS, OP_CAPSULE, OP_ROUNDED_BOX,
  OP_UNION, OP_INTERSECT, OP_DIFF, OP_SMIN, OP_SVDAG, OP_NEURAL_SDF,
} from "./scenes.js";

// Layout-stable opcodes shipped to WGSL. Mirrors scenes.ts 1:1.
// Format per op = 2 * vec4<f32> = 32 bytes :
//   slot0 = (op_code, p0, p1, p2)
//   slot1 = (p3, p4, p5, k)
// Same convention as the legacy FS_SDF stack machine — agents and KASM
// programs that already produce ops for the WebGL2 path stay valid.

const MAX_OPS = 128;
const OPS_BYTES = 16 /* count + pad */ + MAX_OPS * 32;

// §18 Pillar B — Neural SDF compact (Instant-NGP-style, fixed shape so the
// WGSL forward is fully unrolled). L=4 levels of multires hash grid, F=2
// features per entry, T=4096 entries per level, HIDDEN=16 neurons, INPUTS
// = L*F = 8. The buffer layout is :
//   [0..4) header = [active, base_res, _pad, _pad]      (4 floats)
//   [4..4 + L*T*F) hash table interleaved by level       (L*T*F floats)
//   [end .. end+W1) W1 weights (HIDDEN * INPUTS)
//   [.. .. +HIDDEN) b1 biases
//   [.. .. +HIDDEN) W2 weights (1 * HIDDEN)
//   [.. .. +1)      b2 bias
export const NSDF_L = 4;
export const NSDF_F = 2;
export const NSDF_T = 4096;
export const NSDF_HIDDEN = 16;
export const NSDF_INPUTS = NSDF_L * NSDF_F; // 8
export const NSDF_HEADER_FLOATS = 4;
export const NSDF_TABLE_FLOATS = NSDF_L * NSDF_T * NSDF_F;
export const NSDF_W1_FLOATS = NSDF_HIDDEN * NSDF_INPUTS;
export const NSDF_MLP_FLOATS = NSDF_W1_FLOATS + NSDF_HIDDEN + NSDF_HIDDEN + 1;
export const NSDF_TOTAL_FLOATS = NSDF_HEADER_FLOATS + NSDF_TABLE_FLOATS + NSDF_MLP_FLOATS;

const WGSL = `
struct Camera {
  pos:           vec3<f32>,
  tanHalfFovY:   f32,
  fwd:           vec3<f32>,
  showGrid:      f32,
  right:         vec3<f32>,
  _p1:           f32,
  up:            vec3<f32>,
  _p2:           f32,
  resolution:    vec2<f32>,
  centerOffset:  vec2<f32>,
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

// INGEN COMPUTE §18 Pillar C : 3D Gaussian Splatting storage (Phase 6b).
// Each splat is 4 vec4 = 16 floats — anisotropic format compatible with
// the standard 3DGS .ply / .splat files :
//   data[i*4 + 0] = (pos.xyz,   opacity)
//   data[i*4 + 1] = (scale.xyz, _pad)
//   data[i*4 + 2] = (qx, qy, qz, qw)         normalized quaternion
//   data[i*4 + 3] = (color.rgb, _pad)
// Header u32 'count' counts splats (not floats). Additive blend is kept
// as the first-cut output to match the legacy WebGL2 'glow' visually ;
// over-blend with depth sort is Phase 6c.
struct Splats {
  count: u32,
  _p0:   u32,
  _p1:   u32,
  _p2:   u32,
  data:  array<vec4<f32>>,
};

@group(0) @binding(0) var<uniform>          cam: Camera;
@group(0) @binding(1) var<storage, read>    ops: Ops;
@group(0) @binding(2) var                   outTex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<storage, read>    svdag: Svdag;
@group(0) @binding(4) var<storage, read>    splats: Splats;

// §18 Pillar B — Neural SDF storage : flat f32 array with hand-rolled
// offsets. Layout matches ingen-render.ts constants NSDF_* exactly.
// Header[0] > 0.5 → network active ; else the eval returns +1e6 so the
// op cannot affect the scene.
@group(0) @binding(5) var<storage, read>    nsdf: array<f32>;

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

// §18 Pillar B — Neural SDF forward. Multires hash grid (L levels, F
// features per entry, T entries per level) feeds a tiny 2-layer MLP with
// ReLU. All shapes are compile-time constants so the whole forward pass
// is fully unrolled by the WGSL compiler. Inactive net (nsdf[0] <= 0.5)
// short-circuits to +1e6 so OP_NEURAL_SDF cannot affect the scene before
// weights are uploaded.
fn nsdf_hash3(x: i32, y: i32, z: i32) -> u32 {
  // Müller spatial hash (Instant-NGP, eq. 4). Primes chosen to spread
  // collisions uniformly across the hash table for any practical grid.
  let p1: u32 = 1u;
  let p2: u32 = 2654435761u;
  let p3: u32 = 805459861u;
  let ux = bitcast<u32>(x);
  let uy = bitcast<u32>(y);
  let uz = bitcast<u32>(z);
  return ((ux * p1) ^ (uy * p2) ^ (uz * p3));
}

fn nsdf_lookup_level(uvw: vec3<f32>, lvl: u32, base_res: f32) -> vec2<f32> {
  let res = base_res * pow(2.0, f32(lvl));
  let pp = uvw * res;
  let p0 = floor(pp);
  let fr = pp - p0;
  let ix = i32(p0.x);
  let iy = i32(p0.y);
  let iz = i32(p0.z);
  var out = vec2<f32>(0.0);
  // 8 corners trilinear. Bits of 'c' enumerate (dx, dy, dz) in (0, 1).
  for (var c: u32 = 0u; c < 8u; c = c + 1u) {
    let dx = i32(c & 1u);
    let dy = i32((c >> 1u) & 1u);
    let dz = i32((c >> 2u) & 1u);
    let wx = select(1.0 - fr.x, fr.x, dx == 1);
    let wy = select(1.0 - fr.y, fr.y, dy == 1);
    let wz = select(1.0 - fr.z, fr.z, dz == 1);
    let w = wx * wy * wz;
    let key = nsdf_hash3(ix + dx, iy + dy, iz + dz) % ${NSDF_T}u;
    let base = ${NSDF_HEADER_FLOATS}u + lvl * ${NSDF_T * NSDF_F}u + key * ${NSDF_F}u;
    out = out + w * vec2<f32>(nsdf[base], nsdf[base + 1u]);
  }
  return out;
}

fn sd_neural(p_world: vec3<f32>, center: vec3<f32>, half_extent: f32) -> f32 {
  if (nsdf[0] <= 0.5) { return 1.0e6; }
  let he = max(half_extent, 1e-3);
  // Map p_world to [0, 1]^3 cube ; clamp so corners stay in valid hash
  // territory even just outside the bounding cube (raymarcher convergence).
  let uvw = clamp((p_world - center) / (he * 2.0) + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
  let base_res = max(nsdf[1], 1.0);

  // Encode : L * F features concatenated.
  var feat: array<f32, ${NSDF_INPUTS}>;
  for (var lvl: u32 = 0u; lvl < ${NSDF_L}u; lvl = lvl + 1u) {
    let v = nsdf_lookup_level(uvw, lvl, base_res);
    feat[lvl * 2u + 0u] = v.x;
    feat[lvl * 2u + 1u] = v.y;
  }

  // MLP layer 1 : INPUTS -> HIDDEN, ReLU.
  let w1_base: u32 = ${NSDF_HEADER_FLOATS + NSDF_TABLE_FLOATS}u;
  let b1_base: u32 = w1_base + ${NSDF_W1_FLOATS}u;
  let w2_base: u32 = b1_base + ${NSDF_HIDDEN}u;
  let b2_base: u32 = w2_base + ${NSDF_HIDDEN}u;
  var hidden: array<f32, ${NSDF_HIDDEN}>;
  for (var i: u32 = 0u; i < ${NSDF_HIDDEN}u; i = i + 1u) {
    var s = nsdf[b1_base + i];
    for (var j: u32 = 0u; j < ${NSDF_INPUTS}u; j = j + 1u) {
      s = s + nsdf[w1_base + i * ${NSDF_INPUTS}u + j] * feat[j];
    }
    hidden[i] = max(s, 0.0);
  }

  // MLP layer 2 : HIDDEN -> 1 (linear, no activation — distance is signed).
  var d = nsdf[b2_base];
  for (var j: u32 = 0u; j < ${NSDF_HIDDEN}u; j = j + 1u) {
    d = d + nsdf[w2_base + j] * hidden[j];
  }
  // Scale by half_extent so the network output's natural unit matches
  // world distance (the eikonal training target is in normalized coords).
  return d * he;
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
    } else if (op == ${OP_NEURAL_SDF}u) {
      // §18 Pillar B : a.yzw = center of the bounding cube, b.x =
      // half-extent (cube side / 2). The op pushes the neural distance.
      stack[sp] = sd_neural(p, a.yzw, b.x);
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

// Analytical sub-pixel grid.
// Compute shaders cannot use fragment derivatives (fwidth/dpdx/dpdy), so
// line width is estimated in cs_main from depth and viewport height.
fn grid_xy(p: vec3<f32>, step: f32) -> f32 {
  let q = abs(fract(p.xy / step - 0.5) - 0.5) * step;
  return min(q.x, q.y);
}

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let dims = textureDimensions(outTex);
  if (gid.x >= dims.x || gid.y >= dims.y) { return; }

  let res = vec2<f32>(f32(dims.x), f32(dims.y));
  let px = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5)) / res;
  let uv = vec2<f32>(px.x * 2.0 - 1.0, 1.0 - px.y * 2.0);
  let lens_uv = uv - cam.centerOffset;
  let aspect = res.x / res.y;
  let dir = normalize(
    cam.fwd
    + cam.right * (lens_uv.x * aspect * cam.tanHalfFovY)
    + cam.up    * (lens_uv.y * cam.tanHalfFovY)
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

  // Ground-plane intersection for analytical grid (Banger is Z-up).
  // Solves cam.pos.z + dir.z * tg = 0  → tg. Gated by the Scene
  // Collection "Grid" eye toggle (cam.showGrid).
  if (cam.showGrid > 0.5 && abs(dir.z) > 1e-4) {
    let tg = -cam.pos.z / dir.z;
    if (tg > 0.0 && (!hit || tg < t)) {
      let p = cam.pos + dir * tg;
      let g = grid_xy(p, 2.5);
      let pixel_world = max(0.0025, (2.0 * tg * cam.tanHalfFovY) / max(res.y, 1.0));
      let grid_width = pixel_world * 1.35;
      let axis_width = pixel_world * 2.2;
      let line = 1.0 - smoothstep(0.0, grid_width, g);
      let fade = exp(-tg * 0.012);
      col = mix(col, vec3<f32>(0.45, 0.46, 0.50), line * fade * 0.8);
      // Red X axis, green Y axis on the horizontal Z=0 floor.
      let ax = abs(p.y);
      let az = abs(p.x);
      col = mix(col, vec3<f32>(0.90, 0.25, 0.25), (1.0 - smoothstep(0.0, axis_width, ax)) * fade);
      col = mix(col, vec3<f32>(0.25, 0.85, 0.40), (1.0 - smoothstep(0.0, axis_width, az)) * fade);
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

  // §18 Pillar C — Gaussian splat accumulation. Layout 16-float par splat
  // (compatible parseur PLY/SPLAT Phase 6b) ; pour cette première version
  // stable on traite la projection comme isotrope (sigma = moyenne des
  // 3 axes du scale anisotrope, quat ignoré). C'est exact pour les splats
  // bakés sur SDF (scale identique sur 3 axes) et une approximation
  // visuelle correcte pour les vrais 3DGS captures. La projection
  // anisotropique complète (Jacobien Zwicker) reviendra dans un commit
  // dédié avec tests interactifs — la version précédente cassait le
  // pipeline sur AMD Radeon iGPU (écran noir).
  let n_splats = splats.count;
  if (n_splats > 0u) {
    let tanY = cam.tanHalfFovY;
    let uv_x = uv.x * aspect;
    let uv_y = uv.y;
    var accum = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < n_splats; i = i + 1u) {
      let s0 = splats.data[i * 4u + 0u]; // (pos.xyz, opacity)
      let s1 = splats.data[i * 4u + 1u]; // (scale.xyz, _)
      let s3 = splats.data[i * 4u + 3u]; // (color.rgb, _)

      let to_splat = s0.xyz - cam.pos;
      let depth = dot(to_splat, cam.fwd);
      if (depth < 0.05) { continue; }
      let sx = dot(to_splat, cam.right) / (depth * tanY);
      let sy = dot(to_splat, cam.up)    / (depth * tanY);
      let sig_world = max((s1.x + s1.y + s1.z) * (1.0 / 3.0), 1e-4);
      let sig_screen = sig_world / (depth * tanY);
      let dx = uv_x - sx;
      let dy = uv_y - sy;
      let r2 = (dx * dx + dy * dy) / max(sig_screen * sig_screen, 1e-6);
      if (r2 > 9.0) { continue; }
      let w = exp(-0.5 * r2) * s0.w;
      accum = accum + s3.rgb * w;
    }
    col = col + accum;
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
  centerOffset?: [number, number];
  /** Ground grid visibility — defaults to shown when omitted. */
  showGrid?: boolean;
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
  private splatsBuffer: GPUBuffer | null = null;
  private splatsCapacity = 0;
  private nsdfBuffer: GPUBuffer | null = null;
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

    // Diagnostic — surface any WGSL compile error to the console rather
    // than letting the pipeline silently become invalid (= black screen).
    this.device.pushErrorScope("validation");
    const module = this.device.createShaderModule({ code: WGSL, label: "ingen-render-wgsl" });
    this.pipeline = this.device.createComputePipeline({
      label: "ingen-render-pipeline",
      layout: "auto",
      compute: { module, entryPoint: "cs_main" },
    });
    const validationErr = await this.device.popErrorScope();
    if (validationErr) {
      console.error("[ingen-render] WGSL validation error:", (validationErr as any).message);
      this.pipeline = null;
      return false;
    }
    // Async compilation diagnostic — Chrome reports info-level messages
    // (slow shader, deprecated syntax) only through this future.
    if (typeof (module as any).getCompilationInfo === "function") {
      (module as any).getCompilationInfo().then((info: any) => {
        for (const m of info?.messages || []) {
          if (m.type === "error") console.error("[ingen-render] WGSL:", m.message, m);
          else if (m.type === "warning") console.warn("[ingen-render] WGSL:", m.message);
        }
      }).catch(() => {});
    }

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

    // Splats buffer (§18 Pillar C). Header (4 u32) + 8 floats per splat.
    // Empty-by-default : count = 0 → WGSL skips the splat loop entirely.
    // Grows on the first uploadSplats() call ; sized for ~256 default
    // splats so the bake-on-surface helper fits without reallocation.
    const splatsHeaderWords = 4;
    const splatsBodyWords   = 256 * 8;
    this.splatsCapacity = splatsHeaderWords + splatsBodyWords;
    this.splatsBuffer = this.device.createBuffer({
      size: this.splatsCapacity * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-splats",
    });
    this.device.queue.writeBuffer(this.splatsBuffer, 0, new Uint32Array([0, 0, 0, 0]));

    // Neural SDF buffer (§18 Pillar B). Fixed shape — never reallocates,
    // so the bind group stays valid for the entire session. Initialised
    // with header[0]=0 (inactive) ; the WGSL eval short-circuits until
    // uploadNeuralSdf() flips the active flag.
    this.nsdfBuffer = this.device.createBuffer({
      size: NSDF_TOTAL_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-nsdf",
    });
    this.device.queue.writeBuffer(this.nsdfBuffer, 0, new Float32Array([0, 16, 0, 0]));

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
        { binding: 4, resource: { buffer: this.splatsBuffer! } },
        { binding: 5, resource: { buffer: this.nsdfBuffer! } },
      ],
    });
  }

  /**
   * Upload a Neural SDF weight buffer (§18 Pillar B). 'packed' must be a
   * Float32Array of length NSDF_TOTAL_FLOATS — the layout matches the
   * WGSL eval byte-for-byte :
   *   [0]            = active flag (>0.5 → enabled)
   *   [1]            = base_res (level-0 grid resolution, integer)
   *   [2..4)         = padding
   *   [4..4 + L*T*F) = hash table interleaved by level
   *   then           = W1 (HIDDEN*INPUTS) | b1 (HIDDEN) | W2 (HIDDEN) | b2 (1)
   * Reset the network by passing a buffer with active=0 in the first slot.
   */
  uploadNeuralSdf(packed: Float32Array): void {
    if (!this.device || !this.nsdfBuffer) return;
    if (packed.length !== NSDF_TOTAL_FLOATS) {
      console.warn(`[ingen-render] uploadNeuralSdf : expected ${NSDF_TOTAL_FLOATS} floats, got ${packed.length}`);
      return;
    }
    this.device.queue.writeBuffer(this.nsdfBuffer, 0, packed);
    this.cacheValid = false; // weights changed → re-dispatch next frame
  }

  /**
   * Upload anisotropic 3DGS splats (§18 Pillar C, Phase 6b). 'packed' is
   * a Float32Array laid out as 16 floats per splat (4 vec4s) :
   *   [0..3]  = pos.xyz, opacity
   *   [4..7]  = scale.xyz, _
   *   [8..11] = qx, qy, qz, qw   (quaternion, must be normalised)
   *   [12..15]= color.rgb, _
   * 'count' is the number of ACTIVE splats. Passing 0 disables splat
   * rendering without freeing the GPU buffer.
   */
  uploadSplatsAnisotropic(packed: Float32Array, count: number): void {
    if (!this.device || !this.splatsBuffer) return;
    const safeCount = Math.max(0, count | 0);
    const requiredWords = 4 + safeCount * 16;
    if (requiredWords > this.splatsCapacity) {
      this.splatsBuffer.destroy?.();
      let cap = Math.max(this.splatsCapacity, 16);
      while (cap < requiredWords) cap *= 2;
      this.splatsCapacity = cap;
      this.splatsBuffer = this.device.createBuffer({
        size: cap * 4,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-splats",
      });
      if (this.pipeline && this.outView && this.camBuffer && this.opsBuffer && this.svdagBuffer && this.nsdfBuffer) {
        this.bindGroup = this.device.createBindGroup({
          label: "ingen-bg",
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: this.camBuffer } },
            { binding: 1, resource: { buffer: this.opsBuffer } },
            { binding: 2, resource: this.outView },
            { binding: 3, resource: { buffer: this.svdagBuffer } },
            { binding: 4, resource: { buffer: this.splatsBuffer! } },
            { binding: 5, resource: { buffer: this.nsdfBuffer } },
          ],
        });
      }
    }
    this.device.queue.writeBuffer(this.splatsBuffer!, 0, new Uint32Array([safeCount, 0, 0, 0]));
    if (safeCount > 0) {
      const floatsNeeded = safeCount * 16;
      const view = packed.byteLength >= floatsNeeded * 4
        ? new Float32Array(packed.buffer, packed.byteOffset, floatsNeeded)
        : packed;
      this.device.queue.writeBuffer(this.splatsBuffer!, 16, view);
    }
    this.cacheValid = false;
  }

  /**
   * Legacy isotropic adapter — accepts the 8-float layout produced by
   * scenes.ts::bakeGaussiansOnSurface ((pos.xyz, sigma) + (color.rgb,
   * opacity)) and converts to the 16-float anisotropic layout on the fly :
   * identity quaternion, scale = (sigma, sigma, sigma). One-time JS
   * allocation per upload — cheap for the < 256 baked splats produced
   * by the SDF surface walker.
   */
  uploadSplats(packed: Float32Array, count: number): void {
    const safeCount = Math.max(0, count | 0);
    if (safeCount === 0) {
      this.uploadSplatsAnisotropic(new Float32Array(0), 0);
      return;
    }
    const ani = new Float32Array(safeCount * 16);
    for (let i = 0; i < safeCount; i += 1) {
      const src = i * 8;
      const dst = i * 16;
      const sigma = packed[src + 3] ?? 0.0;
      ani[dst + 0]  = packed[src + 0] ?? 0; // pos.x
      ani[dst + 1]  = packed[src + 1] ?? 0; // pos.y
      ani[dst + 2]  = packed[src + 2] ?? 0; // pos.z
      ani[dst + 3]  = packed[src + 7] ?? 0; // opacity (was alpha)
      ani[dst + 4]  = sigma;                // scale.x
      ani[dst + 5]  = sigma;                // scale.y
      ani[dst + 6]  = sigma;                // scale.z
      ani[dst + 7]  = 0;
      ani[dst + 8]  = 0; ani[dst + 9] = 0; ani[dst + 10] = 0; ani[dst + 11] = 1; // identity quat
      ani[dst + 12] = packed[src + 4] ?? 0; // color.r
      ani[dst + 13] = packed[src + 5] ?? 0; // color.g
      ani[dst + 14] = packed[src + 6] ?? 0; // color.b
      ani[dst + 15] = 0;
    }
    this.uploadSplatsAnisotropic(ani, safeCount);
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
      if (this.pipeline && this.outView && this.camBuffer && this.opsBuffer && this.splatsBuffer && this.nsdfBuffer) {
        this.bindGroup = this.device.createBindGroup({
          label: "ingen-bg",
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: this.camBuffer } },
            { binding: 1, resource: { buffer: this.opsBuffer } },
            { binding: 2, resource: this.outView },
            { binding: 3, resource: { buffer: this.svdagBuffer! } },
            { binding: 4, resource: { buffer: this.splatsBuffer } },
            { binding: 5, resource: { buffer: this.nsdfBuffer } },
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
    camU[4] = cam.fwd[0]; camU[5] = cam.fwd[1]; camU[6] = cam.fwd[2]; camU[7] = cam.showGrid === false ? 0 : 1;
    camU[8] = cam.right[0]; camU[9] = cam.right[1]; camU[10] = cam.right[2]; camU[11] = 0;
    camU[12] = cam.up[0]; camU[13] = cam.up[1]; camU[14] = cam.up[2]; camU[15] = 0;
    camU[16] = w; camU[17] = h; camU[18] = cam.centerOffset?.[0] ?? 0; camU[19] = cam.centerOffset?.[1] ?? 0;
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
    this.splatsBuffer?.destroy?.();
    this.nsdfBuffer?.destroy?.();
    this.outTexture = null;
    this.outView = null;
    this.camBuffer = null;
    this.opsBuffer = null;
    this.svdagBuffer = null;
    this.svdagCapacity = 0;
    this.splatsBuffer = null;
    this.splatsCapacity = 0;
    this.nsdfBuffer = null;
    this.pipeline = null;
    this.bindGroup = null;
    this.context = null;
    this.device = null;
  }
}
