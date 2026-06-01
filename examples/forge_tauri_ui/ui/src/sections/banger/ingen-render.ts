// @ts-nocheck
// INGEN Render — WebGPU compute-driven raymarcher for Banger.
// Phase 0 (INGEN COMPUTE §18-19) : ops buffer → compute pass → present.
// Replaces the WebGL2 fragment-SDF path (catalog.ts FS_SDF / VS_SDF).
// Doctrine : 1 device, 1 context, 1 compute pipeline, 1 ops buffer, 1 present.
// No raster middleman, no fullscreen-quad VS, no per-frame shader recompile.

import {
  OP_SPHERE, OP_BOX, OP_TORUS, OP_CAPSULE, OP_ROUNDED_BOX,
  OP_UNION, OP_INTERSECT, OP_DIFF, OP_SMIN, OP_MATERIAL, OP_SAMPLED_SDF, OP_SVDAG, OP_NEURAL_SDF,
} from "./scenes.js";

// Layout-stable opcodes shipped to WGSL. Mirrors scenes.ts 1:1.
// Format per op = 2 * vec4<f32> = 32 bytes :
//   slot0 = (op_code, p0, p1, p2)
//   slot1 = (p3, p4, p5, k)
// Same convention as the legacy FS_SDF stack machine — agents and KASM
// programs that already produce ops for the WebGL2 path stay valid.

const MAX_OPS = 128;
const OPS_BYTES = 16 /* count + pad */ + MAX_OPS * 32;

// §20 GI cache — probe volume size. Must match WGSL PROBE_GRID.
const PROBE_GRID = 16;
const PROBE_TOTAL = PROBE_GRID * PROBE_GRID * PROBE_GRID;

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
  time:          f32,
  up:            vec3<f32>,
  waterLevel:    f32,
  resolution:    vec2<f32>,
  centerOffset:  vec2<f32>,
  sampleIndex:   f32,
  splatSolid:    f32,
  probeSample:   f32,
  skyEnabled:    f32,
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

// INGEN COMPUTE §19.4 — Progressive accumulation buffer. One vec4 per
// pixel holding the running mean colour (rgb) integrated over many
// sub-pixel / area-light jittered samples while the viewport is idle.
// The KASM frame cache decides when to keep dispatching (converging) vs.
// blit the converged image — turning otherwise-skipped idle frames into
// an alias-free, soft-shadowed ultra-HD render.
@group(0) @binding(6) var<storage, read_write> accum: array<vec4<f32>>;

// §20 Fusion v2 — per-splat sun shadow. cs_shadow marche un rayon
// soft_shadow de chaque splat vers le soleil à travers scene() (le champ
// SDF) et écrit un scalaire d'ombre [0,1] ; cs_main le lit pour assombrir
// les splats sous les objets SDF. Calculé une fois par splat (pas par
// pixel) → vraies ombres SDF sur les captures 3DGS sans faire fondre le GPU.
@group(0) @binding(7) var<storage, read_write> shadow: array<f32>;

// §20 GI cache — world-space radiance probe volume (Lumen-style surface
// cache). PROBE_GRID³ probes over a fixed cube centred on the origin, each
// holding incoming irradiance (rgb). cs_probe bakes them ; cs_main reads
// one trilinear sample per pixel instead of path tracing. The bake is
// view-independent and content-hashed : it only re-runs when geometry or
// lights change, so orbiting the camera reuses the whole GI — the "don't
// recompute billions of identical light calcs" win, on modest hardware.
@group(0) @binding(8) var<storage, read_write> probes: array<vec4<f32>>;

// Fieldlet SDF atlas (§18 / Nanite-like cut). Always bound. Header floats:
// [active, resolution, brick_count, table_stride, values_base, _pad, _pad, header_stride].
// Each table row is 20 floats: bounds min/max, value offset, material id,
// surface bounds, error metadata and classification. The first safe shader
// path only uses full brick bounds + trilinear values; skip distances stay
// uploaded for future gated acceleration, not for primary correctness.
@group(0) @binding(9) var<storage, read> sdfBrick: array<f32>;

// §23 — temporal reprojection history. A stable copy of last frame's result
// so a moving view can reuse the converged shading of surfaces still on
// screen instead of restarting accumulation at 1 spp. Layout :
//   history[0] = (prevCamPos.xyz, reprojectEnabled)
//   history[1] = (prevCamFwd.xyz, _)
//   history[2] = (prevCamRight.xyz, _)
//   history[3] = (prevCamUp.xyz, _)
//   history[4u + pix] = (colour.rgb, depthAlongFwd)
// The 4-vec4 header is written by the host each frame ; the pixel region is
// a copy of accum. One binding carries both, minimal plumbing.
@group(0) @binding(10) var<storage, read> history: array<vec4<f32>>;

const PROBE_GRID: u32 = 16u;
const PROBE_HALF: f32 = 6.0; // world half-extent of the cache cube

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

fn sd_sampled_brick_at(p: vec3<f32>, table: u32, res: u32) -> f32 {
  let bmin = vec3<f32>(sdfBrick[table + 0u], sdfBrick[table + 1u], sdfBrick[table + 2u]);
  let bmax = vec3<f32>(sdfBrick[table + 3u], sdfBrick[table + 4u], sdfBrick[table + 5u]);
  let value_offset = u32(max(sdfBrick[table + 6u], 0.0) + 0.5);
  let denom = max(vec3<f32>(bmax - bmin), vec3<f32>(1.0e-5));
  let uvw = clamp((p - bmin) / denom, vec3<f32>(0.0), vec3<f32>(1.0));
  let rr = max(res, 2u);
  let grid = f32(rr - 1u);
  let gp = uvw * grid;
  let p0 = floor(gp);
  let fr = gp - p0;
  let ix = u32(clamp(p0.x, 0.0, grid));
  let iy = u32(clamp(p0.y, 0.0, grid));
  let iz = u32(clamp(p0.z, 0.0, grid));
  let ix1 = min(ix + 1u, rr - 1u);
  let iy1 = min(iy + 1u, rr - 1u);
  let iz1 = min(iz + 1u, rr - 1u);
  let r2 = rr * rr;
  let i000 = value_offset + iz * r2 + iy * rr + ix;
  let i100 = value_offset + iz * r2 + iy * rr + ix1;
  let i010 = value_offset + iz * r2 + iy1 * rr + ix;
  let i110 = value_offset + iz * r2 + iy1 * rr + ix1;
  let i001 = value_offset + iz1 * r2 + iy * rr + ix;
  let i101 = value_offset + iz1 * r2 + iy * rr + ix1;
  let i011 = value_offset + iz1 * r2 + iy1 * rr + ix;
  let i111 = value_offset + iz1 * r2 + iy1 * rr + ix1;
  let c00 = mix(sdfBrick[i000], sdfBrick[i100], fr.x);
  let c10 = mix(sdfBrick[i010], sdfBrick[i110], fr.x);
  let c01 = mix(sdfBrick[i001], sdfBrick[i101], fr.x);
  let c11 = mix(sdfBrick[i011], sdfBrick[i111], fr.x);
  let c0 = mix(c00, c10, fr.y);
  let c1 = mix(c01, c11, fr.y);
  return mix(c0, c1, fr.z);
}

fn sd_sampled_brick(p: vec3<f32>) -> f32 {
  if (sdfBrick[0] <= 0.5) { return 1.0e6; }
  let res = u32(max(sdfBrick[1], 2.0) + 0.5);
  let brick_count = min(u32(max(sdfBrick[2], 0.0) + 0.5), 64u);
  let stride = max(u32(max(sdfBrick[3], 8.0) + 0.5), 8u);
  let table_base = max(u32(max(sdfBrick[7], 8.0) + 0.5), 8u);
  var nearest_box = 1.0e6;
  var best_table: u32 = table_base;
  var best_volume = 1.0e30;
  var found = false;

  for (var i: u32 = 0u; i < 64u; i = i + 1u) {
    if (i >= brick_count) { break; }
    let table = table_base + i * stride;
    let bmin = vec3<f32>(sdfBrick[table + 0u], sdfBrick[table + 1u], sdfBrick[table + 2u]);
    let bmax = vec3<f32>(sdfBrick[table + 3u], sdfBrick[table + 4u], sdfBrick[table + 5u]);
    let half = max((bmax - bmin) * 0.5, vec3<f32>(1.0e-5));
    let center = (bmin + bmax) * 0.5;
    let q = abs(p - center) - half;
    let box_d = length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
    nearest_box = min(nearest_box, max(box_d, 0.0));
    if (box_d <= 1.0e-4) {
      let volume = half.x * half.y * half.z;
      if (!found || volume < best_volume) {
        found = true;
        best_volume = volume;
        best_table = table;
      }
    }
  }

  if (found) {
    return sd_sampled_brick_at(p, best_table, res);
  }
  return nearest_box;
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
    } else if (op == ${OP_SAMPLED_SDF}u) {
      stack[sp] = sd_sampled_brick(p);
      sp = sp + 1;
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

// §22 PBR — surface material. The distance stack machine above only carries
// floats ; this twin runs the SAME program once at the hit point, tracking
// the (albedo, roughness, metallic) of the CLOSEST surface. OP_MATERIAL sets
// the current material for the primitives that follow ; boolean ops keep the
// material of the winning operand. Called once per shaded pixel — far cheaper
// than the march, so marching/shadows/AO stay distance-only.
struct Mat {
  albedo: vec3<f32>,
  rough:  f32,
  metal:  f32,
}

fn default_mat() -> Mat {
  return Mat(vec3<f32>(0.80, 0.82, 0.86), 0.55, 0.0);
}

fn eval_material(p: vec3<f32>) -> Mat {
  var dstack: array<f32, 16>;
  var mstack: array<Mat, 16>;
  var sp: i32 = 0;
  var cur: Mat = default_mat();
  let n = ops.count;
  for (var i: u32 = 0u; i < n; i = i + 1u) {
    let a = ops.data[i * 2u];
    let b = ops.data[i * 2u + 1u];
    let op = u32(a.x + 0.5);
    if (op == ${OP_MATERIAL}u) {
      cur = Mat(a.yzw, max(b.x, 0.02), clamp(b.y, 0.0, 1.0));
    } else if (op == ${OP_SPHERE}u) {
      dstack[sp] = sd_sphere(p - a.yzw, b.x); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_BOX}u) {
      dstack[sp] = sd_box(p - a.yzw, b.xyz); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_TORUS}u) {
      dstack[sp] = sd_torus(p - a.yzw, b.x, b.y); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_CAPSULE}u) {
      dstack[sp] = sd_capsule(p, a.yzw, b.xyz, b.w); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_ROUNDED_BOX}u) {
      dstack[sp] = sd_rounded_box(p - a.yzw, b.xyz, b.w); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_SAMPLED_SDF}u) {
      dstack[sp] = sd_sampled_brick(p); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_SVDAG}u) {
      dstack[sp] = sd_svdag(p, a.yzw, b.x, u32(b.y + 0.5)); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_NEURAL_SDF}u) {
      dstack[sp] = sd_neural(p, a.yzw, b.x); mstack[sp] = cur; sp = sp + 1;
    } else if (op == ${OP_UNION}u) {
      sp = sp - 1;
      if (dstack[sp] < dstack[sp - 1]) { dstack[sp - 1] = dstack[sp]; mstack[sp - 1] = mstack[sp]; }
    } else if (op == ${OP_INTERSECT}u) {
      sp = sp - 1;
      if (dstack[sp] > dstack[sp - 1]) { dstack[sp - 1] = dstack[sp]; mstack[sp - 1] = mstack[sp]; }
    } else if (op == ${OP_DIFF}u) {
      sp = sp - 1;
      dstack[sp - 1] = max(dstack[sp - 1], -dstack[sp]);
    } else if (op == ${OP_SMIN}u) {
      sp = sp - 1;
      if (dstack[sp] < dstack[sp - 1]) { mstack[sp - 1] = mstack[sp]; }
      dstack[sp - 1] = smin_k(dstack[sp - 1], dstack[sp], b.w);
    }
  }
  if (sp <= 0) { return default_mat(); }
  return mstack[0];
}

fn normal(p: vec3<f32>) -> vec3<f32> {
  let e = vec2<f32>(0.001, 0.0);
  return normalize(vec3<f32>(
    scene(p + e.xyy) - scene(p - e.xyy),
    scene(p + e.yxy) - scene(p - e.yxy),
    scene(p + e.yyx) - scene(p - e.yyx)
  ));
}

// Soft shadow via sphere-tracing toward the (jittered) sun. The running
// min of k*h/t is the classic Inigo Quilez penumbra estimator ; combined
// with the per-sample light jitter the accumulator converges to a true
// area-light soft shadow instead of a hard binary one.
fn soft_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32, k: f32) -> f32 {
  var res = 1.0;
  var t = mint;
  for (var i: u32 = 0u; i < 24u; i = i + 1u) {
    if (t >= maxt) { break; }
    let h = scene(ro + rd * t);
    if (h < 0.0008) { return 0.0; }
    res = min(res, k * h / t);
    t = t + clamp(h, 0.015, 0.4);
  }
  return clamp(res, 0.0, 1.0);
}

// SDF ambient occlusion — 5 taps marched along the surface normal (IQ).
// Cheap proxy for how "buried" a point is ; multiplies the ambient term
// so creases and contacts darken without any extra geometry.
fn calc_ao(p: vec3<f32>, n: vec3<f32>) -> f32 {
  var occ = 0.0;
  var sca = 1.0;
  for (var i: u32 = 0u; i < 5u; i = i + 1u) {
    let hr = 0.012 + 0.14 * f32(i) / 4.0;
    let d = scene(p + n * hr);
    occ = occ + (hr - d) * sca;
    sca = sca * 0.92;
  }
  return clamp(1.0 - 2.6 * occ, 0.0, 1.0);
}

// Halton low-discrepancy radical inverse — drives sub-pixel jitter and
// area-light sampling for the progressive accumulator. Bases 2/3 jitter
// the pixel, 5/7 jitter the sun disk.
fn halton(index: u32, base: u32) -> f32 {
  var f = 1.0;
  var r = 0.0;
  var i = index;
  for (var k: u32 = 0u; k < 16u; k = k + 1u) {
    if (i == 0u) { break; }
    f = f / f32(base);
    r = r + f * f32(i % base);
    i = i / base;
  }
  return r;
}

// §20 GI Tier 1 — path tracing helpers.

// Secondary-ray sphere trace : returns the hit distance or -1 on miss.
// Cheaper step budget than the primary (indirect rays tolerate slack).
fn trace(ro: vec3<f32>, rd: vec3<f32>, max_steps: u32) -> f32 {
  var t = 0.0;
  for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
    let d = scene(ro + rd * t);
    if (d < 0.0008 * max(t, 1.0)) { return t; }
    if (t > 120.0) { break; }
    t = t + d;
  }
  return -1.0;
}

// §21 Atmosphere — analytic physically-flavoured sky. A Rayleigh-ish
// vertical gradient (deep zenith → pale horizon → darker ground), a broad
// warm Mie forward-scatter halo around the sun, and a sharp sun disk. Also
// drives the GI ambient and the ocean's reflection, so the whole frame
// shares one coherent lighting environment.
fn sky_env(rd: vec3<f32>) -> vec3<f32> {
  let sun = normalize(vec3<f32>(0.5, 0.4, 0.85));
  let up = clamp(rd.z, -1.0, 1.0);
  let zenith  = vec3<f32>(0.16, 0.33, 0.66);
  let horizon = vec3<f32>(0.60, 0.69, 0.82);
  let ground  = vec3<f32>(0.09, 0.10, 0.12);
  var col = mix(horizon, zenith, clamp(up, 0.0, 1.0));
  col = mix(col, ground, clamp(-up * 2.5, 0.0, 1.0));
  let mu = max(dot(rd, sun), 0.0);
  // Mie halo (broad warm glow) + crisp sun disk.
  col = col + vec3<f32>(1.0, 0.78, 0.50) * (pow(mu, 8.0) * 0.32);
  col = col + vec3<f32>(1.0, 0.96, 0.88) * (pow(mu, 900.0) * 7.0);
  return col;
}

// --- §21 Ocean : value-noise FBM for animated wave height + normal. ---
fn hash2(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn vnoise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = hash2(i);
  let b = hash2(i + vec2<f32>(1.0, 0.0));
  let c = hash2(i + vec2<f32>(0.0, 1.0));
  let d = hash2(i + vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn fbm2(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var amp = 0.5;
  var q = p;
  for (var i: u32 = 0u; i < 4u; i = i + 1u) {
    v = v + amp * vnoise(q);
    q = q * 2.02;
    amp = amp * 0.5;
  }
  return v;
}
fn water_height(xy: vec2<f32>, t: f32) -> f32 {
  let flow = vec2<f32>(0.6, 0.35) * t;
  return fbm2((xy + flow) * 0.25) * 0.55 + fbm2((xy - flow * 0.7) * 0.62) * 0.18;
}
fn water_normal(xy: vec2<f32>, t: f32) -> vec3<f32> {
  let e = 0.15;
  let h0 = water_height(xy, t);
  let hx = water_height(xy + vec2<f32>(e, 0.0), t);
  let hy = water_height(xy + vec2<f32>(0.0, e), t);
  return normalize(vec3<f32>(-(hx - h0) / e, -(hy - h0) / e, 1.0));
}

// Uniform sphere sample — a probe gathers incoming light from all
// directions, so the bake shoots rays over the full sphere.
fn sphere_dir(u1: f32, u2: f32) -> vec3<f32> {
  let z = 1.0 - 2.0 * u1;
  let r = sqrt(max(0.0, 1.0 - z * z));
  let phi = 6.2831853 * u2;
  return vec3<f32>(r * cos(phi), r * sin(phi), z);
}

// World position of probe (i, j, k) inside the cache cube.
fn probe_pos(i: u32, j: u32, k: u32) -> vec3<f32> {
  let g = f32(PROBE_GRID);
  let span = 2.0 * PROBE_HALF;
  return vec3<f32>(
    -PROBE_HALF + (f32(i) + 0.5) / g * span,
    -PROBE_HALF + (f32(j) + 0.5) / g * span,
    -PROBE_HALF + (f32(k) + 0.5) / g * span,
  );
}

// Trilinear gather of cached irradiance at a world point. Out-of-cube
// points clamp to the boundary probes (graceful far-field fade).
fn sample_probe(p: vec3<f32>) -> vec3<f32> {
  let g = f32(PROBE_GRID);
  let span = 2.0 * PROBE_HALF;
  let local = (p + vec3<f32>(PROBE_HALF)) / span * g - vec3<f32>(0.5);
  let p0 = floor(local);
  let fr = local - p0;
  let gi = i32(PROBE_GRID) - 1;
  var acc = vec3<f32>(0.0);
  for (var c: u32 = 0u; c < 8u; c = c + 1u) {
    let dx = i32(c & 1u);
    let dy = i32((c >> 1u) & 1u);
    let dz = i32((c >> 2u) & 1u);
    let ix = clamp(i32(p0.x) + dx, 0, gi);
    let iy = clamp(i32(p0.y) + dy, 0, gi);
    let iz = clamp(i32(p0.z) + dz, 0, gi);
    let wx = select(1.0 - fr.x, fr.x, dx == 1);
    let wy = select(1.0 - fr.y, fr.y, dy == 1);
    let wz = select(1.0 - fr.z, fr.z, dz == 1);
    let idx = (u32(iz) * PROBE_GRID + u32(iy)) * PROBE_GRID + u32(ix);
    acc = acc + (wx * wy * wz) * probes[idx].xyz;
  }
  return acc;
}

// Per-pixel decorrelated RNG (PCG-style integer hash) keyed on pixel,
// sample index and bounce — gives each accumulation sample a fresh path.
fn hash_u32(x: u32) -> u32 {
  var v = x;
  v = v ^ (v >> 16u);
  v = v * 0x7feb352du;
  v = v ^ (v >> 15u);
  v = v * 0x846ca68bu;
  v = v ^ (v >> 16u);
  return v;
}
fn rand2(px: vec2<u32>, s: u32, b: u32) -> vec2<f32> {
  let seed = px.x * 1973u + px.y * 9277u + s * 26699u + b * 131u + 1u;
  let h1 = hash_u32(seed);
  let h2 = hash_u32(h1 ^ 0x68bc21ebu);
  return vec2<f32>(f32(h1) / 4294967296.0, f32(h2) / 4294967296.0);
}

// §22 — ACES filmic tonemap (Narkowicz approximation). Maps linear HDR to
// display, rolling bright PBR specular / sun / reflections off smoothly
// instead of the old hard clamp that flattened them to white.
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
  let a = 2.51;
  let b = 0.03;
  let c = 2.43;
  let d = 0.59;
  let e = 0.14;
  return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
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
  // Progressive accumulation : Halton sub-pixel jitter keyed on the sample
  // index gives temporal supersampling (alias-free edges) while the
  // viewport is idle. Sample 0 is the un-jittered centre so the first
  // (in-motion) frame stays crisp and responsive.
  let si = u32(cam.sampleIndex + 0.5);

  // §24 Checkerboard — during motion (si==0) march only half the pixels each
  // frame (alternating parity) and reuse the other half from history. ~2×
  // fewer primary marches while moving ; static frames stay full-res so they
  // converge cleanly. Flags ride in the history header (.w of slots 1/2).
  let pix0 = gid.y * dims.x + gid.x;
  if (history[1].w > 0.5 && si == 0u) {
    let parity = u32(history[2].w + 0.5);
    if (((gid.x + gid.y) & 1u) != parity) {
      let h = history[4u + pix0];           // last frame's value at this pixel
      accum[pix0] = h;
      textureStore(outTex, vec2<i32>(i32(gid.x), i32(gid.y)),
        vec4<f32>(aces_tonemap(h.xyz * 1.1), 1.0));
      return;
    }
  }

  var jit = vec2<f32>(0.0);
  if (si > 0u) {
    jit = vec2<f32>(halton(si + 1u, 2u), halton(si + 1u, 3u)) - vec2<f32>(0.5);
  }
  let px = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5) + jit) / res;
  let uv = vec2<f32>(px.x * 2.0 - 1.0, 1.0 - px.y * 2.0);
  let lens_uv = uv - cam.centerOffset;
  let aspect = res.x / res.y;
  let dir = normalize(
    cam.fwd
    + cam.right * (lens_uv.x * aspect * cam.tanHalfFovY)
    + cam.up    * (lens_uv.y * cam.tanHalfFovY)
  );

  // Sun (Z-up) with a per-sample jittered disk offset → soft penumbra and
  // soft specular that converge as the accumulator integrates many rays.
  let sun = normalize(vec3<f32>(0.5, 0.4, 0.85));
  let la1 = halton(si + 1u, 5u);
  let la2 = halton(si + 1u, 7u);
  let lrad = sqrt(la2) * 0.055;
  let ljit = normalize(
    sun
    + cam.right * (cos(6.2831853 * la1) * lrad)
    + cam.up    * (sin(6.2831853 * la1) * lrad)
  );

  // Sphere-trace (primary). 80 steps / 100-unit far plane — tuned for a
  // modest GPU at 1080p ; the accumulator hides the slightly shorter reach.
  var t: f32 = 0.0;
  var hit: bool = false;
  for (var i: u32 = 0u; i < 80u; i = i + 1u) {
    let p = cam.pos + dir * t;
    let d = scene(p);
    if (d < 0.0008 * max(t, 1.0)) { hit = true; break; }
    if (t > 100.0) { break; }
    t = t + d;
  }

  // Default Banger background is the Forge shell's neutral black/gray, not
  // the world-sky. The sky stays available, but only when explicitly enabled.
  var col = vec3<f32>(0.070, 0.072, 0.075);
  if (cam.skyEnabled > 0.5) {
    col = sky_env(dir);
  }

  // Ground-plane intersection for analytical grid (Banger is Z-up).
  // Solves cam.pos.z + dir.z * tg = 0  → tg. Gated by the Scene
  // Collection "Grid" eye toggle (cam.showGrid).
  if (cam.showGrid > 0.5 && abs(dir.z) > 1e-4) {
    let tg = -cam.pos.z / dir.z;
    if (tg > 0.0 && (!hit || tg < t)) {
      let p = cam.pos + dir * tg;
      // Contact shadow : SDF objects cast a soft shadow onto the ground
      // plane. Only traced on accumulation samples (si > 0) so the in-motion
      // preview stays cheap ; the shadow fades in as the view settles.
      if (si > 0u) {
        let gsh = soft_shadow(p + vec3<f32>(0.0, 0.0, 0.02), ljit, 0.05, 60.0, 9.0);
        col = col * (0.35 + 0.65 * gsh);
      }
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
    // §20 GI cache — split lighting (Lumen-style) : sharp DIRECT sun is
    // evaluated per pixel (soft-shadowed + AO when idle), while the soft
    // INDIRECT bounce is a single trilinear read from the cached radiance
    // probe volume baked by cs_probe. This collapses the per-pixel
    // multi-bounce path trace into one lookup, so the cost stays flat as the
    // camera orbits and the GI is reused until the scene actually changes.
    let p = cam.pos + dir * t;
    let n = normal(p);
    let m = eval_material(p);
    let sun_col = vec3<f32>(1.00, 0.96, 0.88) * 1.6;
    let viewd = -dir;

    let ndl = max(dot(n, ljit), 0.0);
    var sh = 1.0;
    var ao = 1.0;
    if (si > 0u) {
      if (ndl > 0.0) { sh = soft_shadow(p + n * 0.012, ljit, 0.02, 60.0, 12.0); }
      ao = calc_ao(p, n);
    }

    // §22 PBR — metallic-roughness. F0 = 0.04 dielectric, tinted by albedo
    // for metals ; kd kills diffuse on metals.
    let f0 = mix(vec3<f32>(0.04), m.albedo, m.metal);
    let kd = 1.0 - m.metal;

    // Direct : Lambert diffuse + GGX specular highlight from the sun.
    let hsun = normalize(ljit + viewd);
    let nh = max(dot(n, hsun), 0.0);
    var a2 = max(m.rough * m.rough, 0.002);
    a2 = a2 * a2;
    let denom = nh * nh * (a2 - 1.0) + 1.0;
    let ndf = a2 / (3.14159265 * denom * denom);
    let fres = f0 + (1.0 - f0) * pow(1.0 - max(dot(hsun, viewd), 0.0), 5.0);
    let direct_d = m.albedo * kd * sun_col * (ndl * sh);
    let direct_s = sun_col * (ndf * ndl * sh) * fres;

    // Indirect : cached diffuse GI (albedo-tinted) + a cheap environment
    // specular (sky reflection) so metals and smooth surfaces actually
    // reflect their surroundings instead of reading as flat clay.
    let indirect_d = m.albedo * kd * sample_probe(p + n * 0.15) * ao;
    let env = sky_env(reflect(dir, n));
    let indirect_s = env * f0 * (1.0 - m.rough * 0.7) * ao;

    // Linear HDR — no hard clamp ; the ACES tonemap at store time handles
    // the rolloff so bright specular/reflections stay detailed.
    col = direct_d + direct_s + indirect_d + indirect_s;
  }

  // §21 Ocean — analytic water plane at cam.waterLevel (disabled when the
  // level sits at the -1e9 sentinel). FBM-wave normal → Fresnel sky
  // reflection over a depth-tinted refraction, plus a sharp sun glint.
  // Composited by depth against the SDF surface (mountains poke through).
  var front_t = select(1.0e9, t, hit);
  if (cam.waterLevel > -1.0e8 && abs(dir.z) > 1.0e-4) {
    let tw = (cam.waterLevel - cam.pos.z) / dir.z;
    if (tw > 0.05 && (!hit || tw < t)) {
      let wp = cam.pos + dir * tw;
      let nrm = water_normal(wp.xy, cam.time);
      let viewd = -dir;
      let f0 = 0.02;
      let fres = f0 + (1.0 - f0) * pow(1.0 - max(dot(nrm, viewd), 0.0), 5.0);
      let refl = sky_env(reflect(dir, nrm));
      let deep = vec3<f32>(0.015, 0.07, 0.10);
      let shallow = vec3<f32>(0.06, 0.20, 0.24);
      let refr = mix(deep, shallow, clamp(nrm.z, 0.0, 1.0));
      let hvec = normalize(ljit + viewd);
      let glint = pow(max(dot(nrm, hvec), 0.0), 200.0) * 4.0;
      col = mix(refr, refl, fres) + vec3<f32>(1.0, 0.95, 0.85) * glint;
      front_t = tw;
    }
  }

  // §21 Aerial perspective — distant surfaces fade into the atmosphere
  // (gated by the sky, so the neutral Forge background stays clean). This is
  // what gives a landscape its sense of scale and depth.
  if (cam.skyEnabled > 0.5 && front_t < 1.0e8) {
    let fog = 1.0 - exp(-front_t * 0.018);
    col = mix(col, sky_env(dir), clamp(fog, 0.0, 0.92));
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
  //
  // §20 Fusion des piliers — profondeur partagée SDF ↔ splats. Le SDF est
  // un occulteur opaque : tout splat dont la profondeur (le long de fwd)
  // passe DERRIÈRE la surface SDF touchée est masqué, au lieu de briller
  // au travers. C'est ce qui fait vivre le réel capturé et le procédural
  // dans le même monde. Le blend reste additif (préserve le glow des
  // gaussiennes bakées) ; les vraies ombres SDF sur splats arrivent en v2
  // via une pré-passe d'ombrage par-splat.
  let n_splats = splats.count;
  if (n_splats > 0u) {
    let tanY = cam.tanHalfFovY;
    let uv_x = uv.x * aspect;
    let uv_y = uv.y;
    // Profondeur de la surface SDF le long de l'axe caméra (1e9 si rien
    // touché → aucun splat occulté).
    let surf_z = select(1.0e9, t * dot(dir, cam.fwd), hit);
    var splat_col = vec3<f32>(0.0);
    var splat_w   = 0.0;
    for (var i: u32 = 0u; i < n_splats; i = i + 1u) {
      let s0 = splats.data[i * 4u + 0u]; // (pos.xyz, opacity)
      let s1 = splats.data[i * 4u + 1u]; // (scale.xyz, _)
      let s3 = splats.data[i * 4u + 3u]; // (color.rgb, _)

      let to_splat = s0.xyz - cam.pos;
      let depth = dot(to_splat, cam.fwd);
      if (depth < 0.05) { continue; }
      let sig_world = max((s1.x + s1.y + s1.z) * (1.0 / 3.0), 1e-4);
      // Occlusion par le SDF : un splat franchement derrière la surface est
      // caché. La marge sig_world laisse passer ceux qui l'affleurent pour
      // éviter une silhouette dure sur les contacts.
      if (depth - sig_world > surf_z) { continue; }
      let sx = dot(to_splat, cam.right) / (depth * tanY);
      let sy = dot(to_splat, cam.up)    / (depth * tanY);
      let sig_screen = sig_world / (depth * tanY);
      let dx = uv_x - sx;
      let dy = uv_y - sy;
      let r2 = (dx * dx + dy * dy) / max(sig_screen * sig_screen, 1e-6);
      if (r2 > 9.0) { continue; }
      // §20 Fusion v2 — the SDF field casts a real shadow on this splat
      // (1 = lit, 0 = fully occluded), precomputed once per splat by cs_shadow.
      let contrib = exp(-0.5 * r2) * s0.w * shadow[i];
      splat_col = splat_col + s3.rgb * contrib;
      splat_w   = splat_w + contrib;
    }
    // §20 Fusion v3 — two intents, gated by cam.splatSolid :
    //  • solid (real 3DGS captures) : weighted-OIT over-blend. The dense
    //    splat cloud saturates toward opaque (alpha = 1 - exp(-Σw)) and
    //    composites OVER the SDF/background, so a scanned scene reads as a
    //    real solid surface. Order-independent → still one compute pass.
    //  • additive (SDF-baked gaussians) : legacy glow, preserved exactly.
    if (cam.splatSolid > 0.5) {
      if (splat_w > 1e-4) {
        let solid = splat_col / splat_w;     // weighted mean colour
        let alpha = 1.0 - exp(-splat_w);     // coverage → opaque where dense
        col = mix(col, solid, alpha);
      }
    } else {
      col = col + splat_col;
    }
  }

  // Progressive accumulation : sample 0 seeds the running mean, every
  // later (idle) sample folds in with weight 1/(n+1) so the buffer holds
  // the unbiased average. The 8-bit storage texture always shows the
  // current mean — the image visibly sharpens and the shadows soften as
  // the accumulator converges, then the host stops dispatching.
  let pix = gid.y * dims.x + gid.x;
  // §23 — depth (along fwd) of the nearest surface, stored in .w so temporal
  // reprojection can depth-validate a reused history pixel.
  let cam_depth = select(1.0e9, front_t * dot(dir, cam.fwd), front_t < 1.0e8);

  var mean: vec3<f32>;
  if (si > 0u) {
    // Static camera : in-place progressive accumulation (own pixel, no race).
    mean = mix(accum[pix].xyz, col, 1.0 / f32(si + 1u));
  } else {
    // First sample after a change. With reprojection enabled, reuse last
    // frame's converged shading for a surface still on screen (depth-checked)
    // instead of restarting at 1 spp ; otherwise start fresh.
    mean = col;
    if (history[0].w > 0.5 && front_t < 1.0e8) {
      let pPos = history[0].xyz;
      let pFwd = history[1].xyz;
      let pRight = history[2].xyz;
      let pUp = history[3].xyz;
      let P = cam.pos + dir * front_t;
      let rel = P - pPos;
      let relf = dot(rel, pFwd);
      if (relf > 0.05) {
        let lx = (dot(rel, pRight) / relf) / (aspect * cam.tanHalfFovY);
        let ly = (dot(rel, pUp)    / relf) / cam.tanHalfFovY;
        let ppx = (lx + 1.0) * 0.5;
        let ppy = (1.0 - ly) * 0.5;
        if (ppx > 0.0 && ppx < 1.0 && ppy > 0.0 && ppy < 1.0) {
          let pix2 = 4u + (u32(ppy * res.y) * dims.x + u32(ppx * res.x));
          let hcol = history[pix2];
          // Reuse only if the history pixel is the same surface (depths agree
          // within a distance-proportional tolerance → rejects disocclusion).
          if (hcol.w < 1.0e8 && abs(hcol.w - relf) < 0.03 * relf + 0.02) {
            mean = mix(hcol.xyz, col, 0.1); // exponential TAA blend
          }
        }
      }
    }
  }
  accum[pix] = vec4<f32>(mean, cam_depth); // linear HDR + depth ; never tonemapped
  // §22 — filmic ACES + exposure at display time only (the buffer stays
  // linear so the running average is unbiased).
  let mapped = aces_tonemap(mean * 1.1);
  textureStore(outTex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(mapped, 1.0));
}

// §20 Fusion v2 — per-splat sun-shadow pre-pass. One thread per splat
// marches a soft shadow ray toward the (un-jittered) sun through the SDF
// field. The scalar is reused by every pixel in cs_main, so an SDF object
// casts a real soft shadow onto a captured 3DGS scene for the cost of
// N_splats rays per frame instead of N_splats × pixels. Recomputed only
// when splats or the SDF field change (never on camera orbit).
@compute @workgroup_size(64)
fn cs_shadow(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= splats.count) { return; }
  let s0 = splats.data[i * 4u + 0u];
  // Must match the base sun direction used in cs_main.
  let sun = normalize(vec3<f32>(0.5, 0.4, 0.85));
  shadow[i] = soft_shadow(s0.xyz + sun * 0.02, sun, 0.02, 60.0, 10.0);
}

// §20 GI cache — radiance probe bake. One thread per probe shoots a few
// uniform-sphere rays, gathering one-bounce incoming radiance (direct sun
// at the hit, or sky for an escaped ray). The result is folded into the
// probe via a running mean keyed on cam.probeSample, so the volume
// converges over a handful of bakes then freezes. View-independent : the
// host only re-bakes (resets probeSample) when geometry or lights change,
// never on camera orbit — that is the KASM content-hash reuse in practice.
@compute @workgroup_size(64)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
  let total = PROBE_GRID * PROBE_GRID * PROBE_GRID;
  let idx = gid.x;
  if (idx >= total) { return; }
  let i = idx % PROBE_GRID;
  let j = (idx / PROBE_GRID) % PROBE_GRID;
  let k = idx / (PROBE_GRID * PROBE_GRID);
  let p = probe_pos(i, j, k);

  let bake = u32(cam.probeSample + 0.5);
  let sun = normalize(vec3<f32>(0.5, 0.4, 0.85));
  let sun_col = vec3<f32>(1.0, 0.96, 0.88) * 1.6;
  let albedo = vec3<f32>(0.80, 0.82, 0.86);

  // Cheap bake : few rays, short traces, and an UNSHADOWED one-bounce (the
  // sharp shadows live on the per-pixel direct term ; shadowing the indirect
  // bounce too would cost a 40-step march per ray for little visible gain).
  let n_rays = 4u;
  var acc = vec3<f32>(0.0);
  for (var r: u32 = 0u; r < n_rays; r = r + 1u) {
    let u = rand2(vec2<u32>(idx, idx ^ 0x9e3779b9u), bake, r);
    let wi = sphere_dir(u.x, u.y);
    let th = trace(p, wi, 32u);
    if (th < 0.0) {
      acc = acc + sky_env(wi);
    } else {
      let hp = p + wi * th;
      let hn = normal(hp);
      acc = acc + albedo * sun_col * max(dot(hn, sun), 0.0);
    }
  }
  acc = acc * (1.0 / f32(n_rays));

  if (bake == 0u) {
    probes[idx] = vec4<f32>(acc, 1.0);
  } else {
    probes[idx] = vec4<f32>(mix(probes[idx].xyz, acc, 1.0 / f32(bake + 1u)), 1.0);
  }
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
  /** Procedural sky visibility — defaults to Forge neutral background. */
  skyEnabled?: boolean;
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
  // §20 Fusion v2 — second compute pipeline (per-splat sun-shadow pre-pass)
  // sharing one explicit bind-group layout with the main raymarch pipeline.
  private pipelineShadow: GPUComputePipeline | null = null;
  // §20 GI cache — third pipeline, the world-space radiance probe bake.
  private pipelineProbe: GPUComputePipeline | null = null;
  private auxiliaryPipelinesStarted = false;
  private bindGroupLayout: GPUBindGroupLayout | null = null;
  private camBuffer: GPUBuffer | null = null;
  private opsBuffer: GPUBuffer | null = null;
  private svdagBuffer: GPUBuffer | null = null;
  private svdagCapacity = 0;
  private splatsBuffer: GPUBuffer | null = null;
  private splatsCapacity = 0;
  private nsdfBuffer: GPUBuffer | null = null;
  private sdfBrickBuffer: GPUBuffer | null = null;
  private sdfBrickCapacity = 0; // capacity in f32 words
  // §19.4 progressive accumulator. One vec4 per pixel ; recreated on resize.
  private accumBuffer: GPUBuffer | null = null;
  private accumPixels = 0;
  // §23 — temporal reprojection. `historyBuffer` = 4-vec4 camera header +
  // a copy of last frame's accum ; `reproject` gates it (default off) ;
  // `prevBasis` holds the previous frame's camera for the reproject header.
  private historyBuffer: GPUBuffer | null = null;
  private reproject = false;
  // §24 Checkerboard — march half the pixels per motion frame. Parity flips
  // each rendered frame ; flags ride in the history header's spare .w slots.
  private checkerboard = false;
  private frameParity = 0;
  private readonly prevBasis = new Float32Array(16); // pos|enable, fwd|cb, right|parity, up
  private prevBasisValid = false;
  // §20 Fusion v2 — per-splat sun-shadow buffer (one f32 per splat) plus the
  // dirty flag that gates the pre-pass. Splat shadows depend only on splat
  // positions + the SDF field + the fixed sun — never the camera — so they
  // are recomputed on upload, not on every orbit frame.
  private shadowBuffer: GPUBuffer | null = null;
  private shadowCapacity = 0; // capacity in splats (f32 entries)
  private splatCount = 0;
  private shadowDirty = true;
  // §20 Fusion v3 — splat compositing intent. false = additive glow (default,
  // for SDF-baked gaussians) ; true = weighted-OIT over-blend (solid look for
  // real 3DGS captures). Enters the camera hash so toggling re-renders.
  private splatSolid = false;
  // §20 GI cache — radiance probe volume + its bake convergence counter.
  // probeSample resets only when geometry/lights change (probeDirty), never
  // on camera orbit, so the baked GI is reused across viewpoints.
  private probesBuffer: GPUBuffer | null = null;
  private probeSample = 0;
  private readonly probeMaxSamples = 24;
  private probeDirty = true;
  // §21 Ocean — world-space water height, or -1e9 = disabled. When enabled,
  // `time` advances so the waves animate (which keeps the loop live).
  private waterLevel = -1e9;
  // Current converged sample count for the static scene. Reset to 0 whenever
  // the camera / ops / dims change ; climbs to `maxSamples` while idle.
  private sampleCount = 0;
  // Convergence budget — kept modest so the GPU isn't pegged for seconds of
  // heavy idle samples after every camera stop on a weak GPU.
  private readonly maxSamples = 64;
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
  // Which GPU adapter WebGPU actually selected (filled in init()). A vendor
  // of "" or isFallback=true means a software/CPU rasteriser — unusable.
  private adapterInfo: { vendor: string; architecture: string; description: string; isFallback: boolean } | null = null;

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
    // Ask explicitly for the discrete high-performance GPU and refuse a
    // software fallback up front (forceFallbackAdapter:false). On a hybrid
    // laptop this is the difference between the RTX 3050 and the weak iGPU
    // — and between the iGPU and an unusable CPU/WARP software rasteriser.
    let adapter = await nav.gpu.requestAdapter({
      powerPreference: "high-performance",
      forceFallbackAdapter: false,
    });
    if (!adapter) {
      // Last resort : let the runtime pick anything (may be the iGPU) so the
      // viewport at least lights up, but we'll flag it loudly below.
      adapter = await nav.gpu.requestAdapter();
    }
    if (!adapter) {
      console.warn("[ingen-render] no GPUAdapter");
      return false;
    }
    // GPU diagnostics — surface exactly which adapter is in use so a wrong
    // (iGPU) or software (CPU) selection is obvious instead of silent lag.
    let info: any = null;
    try {
      info = (adapter as any).info
        || (typeof (adapter as any).requestAdapterInfo === "function"
          ? await (adapter as any).requestAdapterInfo()
          : null);
    } catch (_) { /* info is best-effort */ }
    const isFallback = !!(adapter as any).isFallbackAdapter;
    const desc = `${info?.vendor || "?"} / ${info?.architecture || "?"} / ${info?.description || info?.device || "?"}`;
    this.adapterInfo = {
      vendor: info?.vendor ?? "",
      architecture: info?.architecture ?? "",
      description: info?.description ?? info?.device ?? "",
      isFallback,
    };
    const software = isFallback
      || /software|warp|swiftshader|llvmpipe|basic render|microsoft basic/i.test(desc);
    if (software) {
      console.error(
        `[ingen-render] ⚠️ adaptateur LOGICIEL/CPU détecté (${desc}) — le rendu sera inutilisable. `
        + "Forcer le GPU discret : Windows → Paramètres → Affichage → Graphismes → l'app → Hautes performances.",
      );
    } else {
      console.log(`[ingen-render] GPU adapter: ${desc}`);
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
    // Explicit bind-group layout so the main raymarch pipeline and the
    // per-splat shadow pre-pass can share ONE bind group. (`layout: "auto"`
    // would derive incompatible layouts because each entry point touches a
    // different subset of the bindings.)
    const COMPUTE = GPUShaderStage.COMPUTE;
    this.bindGroupLayout = this.device.createBindGroupLayout({
      label: "ingen-bgl",
      entries: [
        { binding: 0, visibility: COMPUTE, buffer: { type: "uniform" } },
        { binding: 1, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
        { binding: 2, visibility: COMPUTE, storageTexture: { access: "write-only", format: "rgba8unorm", viewDimension: "2d" } },
        { binding: 3, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
        { binding: 4, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
        { binding: 5, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
        { binding: 6, visibility: COMPUTE, buffer: { type: "storage" } },
        { binding: 7, visibility: COMPUTE, buffer: { type: "storage" } },
        { binding: 8, visibility: COMPUTE, buffer: { type: "storage" } },
        { binding: 9, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
        { binding: 10, visibility: COMPUTE, buffer: { type: "read-only-storage" } },
      ],
    });
    const pipelineLayout = this.device.createPipelineLayout({
      label: "ingen-pipeline-layout",
      bindGroupLayouts: [this.bindGroupLayout],
    });
    this.pipeline = this.device.createComputePipeline({
      label: "ingen-render-pipeline",
      layout: pipelineLayout,
      compute: { module, entryPoint: "cs_main" },
    });
    const validationErr = await this.device.popErrorScope();
    if (validationErr) {
      console.error("[ingen-render] WGSL validation error:", (validationErr as any).message);
      this.pipeline = null;
      this.pipelineShadow = null;
      this.pipelineProbe = null;
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
    this.deferAuxiliaryPipelines(module, pipelineLayout);

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

    // Fieldlet SDF atlas. Empty header by default: OP_SAMPLED_SDF returns a
    // far positive distance until uploadSdfBrickAtlas() activates it.
    this.sdfBrickCapacity = 16;
    this.sdfBrickBuffer = this.device.createBuffer({
      size: this.sdfBrickCapacity * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-sdf-brick-atlas",
    });
    this.device.queue.writeBuffer(this.sdfBrickBuffer, 0, new Float32Array([0, 2, 0, 8, 8, 0, 0, 8]));

    // §20 Fusion v2 — per-splat shadow scalars, sized for the default splat
    // capacity ; grows in lock-step with the splats buffer.
    this.shadowCapacity = 256;
    this.shadowBuffer = this.device.createBuffer({
      size: this.shadowCapacity * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-shadow",
    });

    // §20 GI cache — radiance probe volume (PROBE_GRID³ vec4). Fixed size,
    // never reallocates ; zero-initialised so the first frames read black
    // indirect until the bake converges.
    this.probesBuffer = this.device.createBuffer({
      size: PROBE_TOTAL * 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      label: "ingen-probes",
    });

    this.resize(this.canvas.width || 1, this.canvas.height || 1);
    return true;
  }

  private refreshBindGroup(): void {
    if (!this.device || !this.bindGroupLayout || !this.outView
      || !this.camBuffer || !this.opsBuffer || !this.svdagBuffer
      || !this.splatsBuffer || !this.nsdfBuffer || !this.accumBuffer
      || !this.shadowBuffer || !this.probesBuffer || !this.sdfBrickBuffer
      || !this.historyBuffer) {
      return;
    }
    this.bindGroup = this.device.createBindGroup({
      label: "ingen-bg",
      layout: this.bindGroupLayout,
      entries: [
        { binding: 0, resource: { buffer: this.camBuffer } },
        { binding: 1, resource: { buffer: this.opsBuffer } },
        { binding: 2, resource: this.outView },
        { binding: 3, resource: { buffer: this.svdagBuffer } },
        { binding: 4, resource: { buffer: this.splatsBuffer } },
        { binding: 5, resource: { buffer: this.nsdfBuffer } },
        { binding: 6, resource: { buffer: this.accumBuffer } },
        { binding: 7, resource: { buffer: this.shadowBuffer } },
        { binding: 8, resource: { buffer: this.probesBuffer } },
        { binding: 9, resource: { buffer: this.sdfBrickBuffer } },
        { binding: 10, resource: { buffer: this.historyBuffer } },
      ],
    });
  }

  private deferAuxiliaryPipelines(module: GPUShaderModule, pipelineLayout: GPUPipelineLayout): void {
    if (!this.device || this.auxiliaryPipelinesStarted) return;
    this.auxiliaryPipelinesStarted = true;
    const start = () => {
      const device = this.device;
      if (!device) return;
      const makePipeline = async (label: string, entryPoint: string) => {
        const desc = { label, layout: pipelineLayout, compute: { module, entryPoint } };
        if (typeof (device as any).createComputePipelineAsync === "function") {
          return await (device as any).createComputePipelineAsync(desc);
        }
        return device.createComputePipeline(desc);
      };
      device.pushErrorScope("validation");
      Promise.all([
        makePipeline("ingen-shadow-pipeline", "cs_shadow"),
        makePipeline("ingen-probe-pipeline", "cs_probe"),
      ]).then(async ([shadow, probe]) => {
        const validationErr = await device.popErrorScope();
        if (validationErr) {
          console.warn("[ingen-render] auxiliary pipeline validation:", (validationErr as any).message);
          return;
        }
        if (this.device !== device) return;
        this.pipelineShadow = shadow;
        this.pipelineProbe = probe;
        this.shadowDirty = true;
        this.probeDirty = true;
        this.cacheValid = false;
      }).catch(async (err) => {
        try { await device.popErrorScope(); } catch (_) {}
        console.warn("[ingen-render] auxiliary pipeline warmup failed:", err?.message || err);
      });
    };
    if (typeof (globalThis as any).requestIdleCallback === "function") {
      (globalThis as any).requestIdleCallback(start, { timeout: 700 });
    } else {
      globalThis.setTimeout(start, 120);
    }
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
    this.sampleCount = 0;
    this.dimsKey = ((w & 0xffff) | ((h & 0xffff) << 16)) >>> 0;

    // (Re)allocate the accumulation buffer to match the new pixel count.
    // COPY_SRC so each frame's result can be copied into the reproject history.
    const pixels = w * h;
    if (pixels !== this.accumPixels) {
      this.accumBuffer?.destroy?.();
      this.accumBuffer = this.device.createBuffer({
        size: pixels * 16, // vec4<f32> per pixel
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        label: "ingen-accum",
      });
      // §23 — history buffer : 4-vec4 camera header + per-pixel copy of accum.
      this.historyBuffer?.destroy?.();
      this.historyBuffer = this.device.createBuffer({
        size: (4 + pixels) * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-history",
      });
      this.accumPixels = pixels;
    }

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
    this.refreshBindGroup();
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
    this.shadowDirty = true; // neural field changed → recompute splat shadows
    this.probeDirty = true;  // …and rebake the GI probe volume (§20)
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
    let realloced = false;
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
      realloced = true;
    }
    // Grow the per-splat shadow buffer in lock-step (§20 Fusion v2).
    if (safeCount > this.shadowCapacity) {
      this.shadowBuffer?.destroy?.();
      let scap = Math.max(this.shadowCapacity, 16);
      while (scap < safeCount) scap *= 2;
      this.shadowCapacity = scap;
      this.shadowBuffer = this.device.createBuffer({
        size: scap * 4,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-shadow",
      });
      realloced = true;
    }
    if (realloced) this.refreshBindGroup();
    this.device.queue.writeBuffer(this.splatsBuffer!, 0, new Uint32Array([safeCount, 0, 0, 0]));
    if (safeCount > 0) {
      const floatsNeeded = safeCount * 16;
      const view = packed.byteLength >= floatsNeeded * 4
        ? new Float32Array(packed.buffer, packed.byteOffset, floatsNeeded)
        : packed;
      this.device.queue.writeBuffer(this.splatsBuffer!, 16, view);
    }
    this.splatCount = safeCount;
    this.shadowDirty = true; // splat positions changed → recompute shadows
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
    // The isotropic adapter is only ever fed SDF-baked glow gaussians, so it
    // owns the additive intent (§20 Fusion v3). Real captures call
    // uploadSplatsAnisotropic directly and flip setSplatSolid(true).
    this.setSplatSolid(false);
  }

  uploadSdfBrick(values: Float32Array, boundsMin: number[], boundsMax: number[], resolution?: number): boolean {
    return this.uploadSdfBrickAtlas([{ values, boundsMin, boundsMax }], resolution);
  }

  uploadSdfBrickAtlas(bricks: any[], resolution?: number): boolean {
    if (!this.device || !this.sdfBrickBuffer) return false;
    const src = Array.isArray(bricks) ? bricks : [];
    const firstValues = src.find((brick) => brick?.values)?.values;
    const inferredRes = resolution
      || (firstValues?.length ? Math.round(Math.cbrt(Number(firstValues.length))) : 0);
    const res = Math.max(2, Math.min(64, Number(inferredRes) | 0 || 2));
    const voxelsPerBrick = res * res * res;
    const active = src
      .filter((brick) => brick?.values && brick?.boundsMin && brick?.boundsMax)
      .slice(0, 64);

    if (!active.length) {
      this.device.queue.writeBuffer(this.sdfBrickBuffer, 0, new Float32Array([0, res, 0, 20, 8, 0, 0, 8]));
      this.shadowDirty = true;
      this.probeDirty = true;
      this.cacheValid = false;
      return true;
    }

    const headerStride = 8;
    const tableStride = 20;
    const tableBase = headerStride;
    const valuesBase = tableBase + active.length * tableStride;
    const requiredWords = valuesBase + active.length * voxelsPerBrick;
    if (requiredWords > this.sdfBrickCapacity) {
      this.sdfBrickBuffer.destroy?.();
      let cap = Math.max(this.sdfBrickCapacity, 16);
      while (cap < requiredWords) cap *= 2;
      this.sdfBrickCapacity = cap;
      this.sdfBrickBuffer = this.device.createBuffer({
        size: cap * 4,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-sdf-brick-atlas",
      });
      this.refreshBindGroup();
    }

    const packed = new Float32Array(requiredWords);
    packed[0] = 1;
    packed[1] = res;
    packed[2] = active.length;
    packed[3] = tableStride;
    packed[4] = valuesBase;
    packed[5] = 0;
    packed[6] = 0;
    packed[7] = tableBase;

    const vec3 = (value: any, fallback: number[]) => {
      const out = fallback.slice(0, 3);
      for (let i = 0; i < 3; i += 1) {
        const n = Number(value?.[i]);
        if (Number.isFinite(n)) out[i] = n;
      }
      return out;
    };
    const classificationCode = (value: any) => {
      if (typeof value === "string") {
        const lower = value.toLowerCase();
        if (lower.includes("outside")) return 0;
        if (lower.includes("inside")) return 1;
      }
      const n = Number(value);
      return Number.isFinite(n) ? Math.max(0, Math.min(3, n)) : 2;
    };

    for (let i = 0; i < active.length; i += 1) {
      const brick = active[i];
      const row = tableBase + i * tableStride;
      const valueOffset = valuesBase + i * voxelsPerBrick;
      const bmin = vec3(brick.boundsMin, [-1, -1, -1]);
      const bmax = vec3(brick.boundsMax, [1, 1, 1]);
      const smin = vec3(brick.surfaceBoundsMin, bmin);
      const smax = vec3(brick.surfaceBoundsMax, bmax);
      packed[row + 0] = bmin[0];
      packed[row + 1] = bmin[1];
      packed[row + 2] = bmin[2];
      packed[row + 3] = bmax[0];
      packed[row + 4] = bmax[1];
      packed[row + 5] = bmax[2];
      packed[row + 6] = valueOffset;
      packed[row + 7] = Math.max(0, Math.min(1024, Number(brick.materialId) | 0));
      packed[row + 8] = smin[0];
      packed[row + 9] = smin[1];
      packed[row + 10] = smin[2];
      packed[row + 11] = smax[0];
      packed[row + 12] = smax[1];
      packed[row + 13] = smax[2];
      packed[row + 14] = Math.max(0, Number(brick.errorWorld) || 0);
      packed[row + 15] = Math.max(0, Number(brick.skipDistance) || 0);
      packed[row + 16] = classificationCode(brick.classification);
      packed[row + 17] = 0;
      packed[row + 18] = 0;
      packed[row + 19] = 0;

      const values = brick.values instanceof Float32Array
        ? brick.values
        : new Float32Array(brick.values);
      packed.set(values.subarray(0, Math.min(values.length, voxelsPerBrick)), valueOffset);
      if (values.length < voxelsPerBrick) {
        packed.fill(1.0e6, valueOffset + values.length, valueOffset + voxelsPerBrick);
      }
    }

    this.device.queue.writeBuffer(this.sdfBrickBuffer, 0, packed);
    this.shadowDirty = true;
    this.probeDirty = true;
    this.cacheValid = false;
    return true;
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
      this.refreshBindGroup();
    }
    if (packed.length === 0) {
      this.device.queue.writeBuffer(this.svdagBuffer!, 0, new Uint32Array([0, 0, 0, 0]));
    } else {
      this.device.queue.writeBuffer(this.svdagBuffer!, 0, packed);
    }
    // SVDAG changed → invalidate the frame cache so the next render
    // dispatches compute instead of replaying a stale framebuffer.
    // The SDF field changed → splat shadows and GI probes must rebake (§20).
    this.shadowDirty = true;
    this.probeDirty = true;
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
    const newHash = fnv1a32(u32);
    // uploadOps is called every frame ; only flag the SDF-dependent caches
    // dirty when the field actually changed (§20) so orbiting never
    // recomputes splat shadows or the GI probe volume.
    if (newHash !== this.opsHash) {
      this.shadowDirty = true;
      this.probeDirty = true;
    }
    this.opsHash = newHash;
  }

  render(cam: IngenCamera): void {
    if (!this.device || !this.context || !this.pipeline || !this.bindGroup) return;
    const w = this.width;
    const h = this.height;

    // Hash camera before writing it to the UBO — saves a writeBuffer on hits.
    const camU = new Float32Array(24);
    camU[0] = cam.pos[0]; camU[1] = cam.pos[1]; camU[2] = cam.pos[2]; camU[3] = cam.tanHalfFovY;
    camU[4] = cam.fwd[0]; camU[5] = cam.fwd[1]; camU[6] = cam.fwd[2]; camU[7] = cam.showGrid === false ? 0 : 1;
    // §21 — camU[11] = time (advances only while the ocean is on, so a static
    // scene still hashes stable and goes idle) ; camU[15] = water level.
    const waterOn = this.waterLevel > -1e8;
    camU[8] = cam.right[0]; camU[9] = cam.right[1]; camU[10] = cam.right[2];
    camU[11] = waterOn ? (performance.now() * 0.001) : 0;
    camU[12] = cam.up[0]; camU[13] = cam.up[1]; camU[14] = cam.up[2];
    camU[15] = this.waterLevel;
    camU[16] = w; camU[17] = h; camU[18] = cam.centerOffset?.[0] ?? 0; camU[19] = cam.centerOffset?.[1] ?? 0;
    // sampleIndex (camU[20]) is filled in just before dispatch — it must NOT
    // enter the hash, otherwise every accumulation frame would look "new"
    // and the scene would never be detected as idle. splatSolid (camU[21])
    // IS hashed so flipping the compositing intent forces a fresh render.
    camU[20] = 0; camU[21] = this.splatSolid ? 1 : 0; camU[22] = 0; camU[23] = cam.skyEnabled ? 1 : 0;
    const camHash = fnv1a32(new Uint32Array(camU.buffer));

    // KASM frame cache → progressive accumulation. The scene is "unchanged"
    // when camera + ops + dims all match the last dispatch. On a change we
    // restart the accumulator at sample 0 (crisp, responsive 1-spp frame).
    // While unchanged we keep folding in jittered samples up to maxSamples,
    // converging to an alias-free, soft-shadowed image, then stop.
    const sceneChanged = !this.cacheValid
      || camHash !== this.cachedCamHash
      || this.opsHash !== this.cachedOpsHash
      || this.dimsKey !== this.cachedDimsKey;
    if (sceneChanged) this.sampleCount = 0;
    // GI probe cache resets only on geometry/light change (probeDirty), never
    // on camera orbit — so the baked indirect light is reused across all
    // viewpoints. This is the KASM content-hash reuse made concrete.
    if (this.probeDirty) {
      this.probeSample = 0;
      this.probeDirty = false;
    }

    this.stats.frames += 1;
    const encoder = this.device.createCommandEncoder({ label: "ingen-encoder" });

    const converging = this.sampleCount < this.maxSamples;
    // Never let GI probe baking sit in front of the first visible frame.
    // The first dispatch shows direct lighting immediately; cached indirect
    // light starts refining only after at least one image has been blitted and
    // after the deferred probe pipeline is actually ready.
    const probeBaking = !!this.pipelineProbe
      && this.sampleCount > 0
      && this.probeSample < this.probeMaxSamples;
    // Dispatch whenever the image must change : a moved/converging view, OR a
    // still-baking probe volume (so the main pass re-runs to show the refined
    // GI). Camera orbit with frozen probes still falls under `converging`.
    if (sceneChanged || converging || probeBaking) {
      camU[20] = this.sampleCount;  // primary AA / jitter seed
      camU[22] = this.probeSample;  // probe bake accumulation seed
      this.device.queue.writeBuffer(this.camBuffer!, 0, camU);

      // §23/§24 — write the history header = the PREVIOUS frame's camera plus
      // the reproject enable (.w of slot 0) and the checkerboard enable +
      // parity (.w of slots 1/2). Enabled only once a valid prior frame exists.
      if (this.historyBuffer) {
        this.prevBasis[3] = (this.reproject && this.prevBasisValid) ? 1 : 0;
        this.prevBasis[7] = (this.checkerboard && this.prevBasisValid) ? 1 : 0;
        this.prevBasis[11] = this.frameParity;
        this.device.queue.writeBuffer(this.historyBuffer, 0, this.prevBasis);
      }

      // §20 Fusion v2 — per-splat sun-shadow pre-pass. Camera-independent ;
      // read by the main pass below in the same command buffer (pass ordering
      // makes the writes visible). One thread per splat.
      if (this.shadowDirty && this.splatCount > 0 && this.pipelineShadow) {
        const spass = encoder.beginComputePass({ label: "ingen-shadow-pass" });
        spass.setPipeline(this.pipelineShadow);
        spass.setBindGroup(0, this.bindGroup);
        spass.dispatchWorkgroups(Math.ceil(this.splatCount / 64), 1, 1);
        spass.end();
        this.shadowDirty = false;
      }

      // §20 GI cache — bake one more set of probe rays while converging.
      // Camera-independent ; read by the main pass (probe gather) in the same
      // command buffer. Stops once the volume has converged, then frozen.
      if (probeBaking && this.pipelineProbe) {
        const ppass = encoder.beginComputePass({ label: "ingen-probe-pass" });
        ppass.setPipeline(this.pipelineProbe);
        ppass.setBindGroup(0, this.bindGroup);
        ppass.dispatchWorkgroups(Math.ceil(PROBE_TOTAL / 64), 1, 1);
        ppass.end();
        this.probeSample += 1;
      }

      const pass = encoder.beginComputePass({ label: "ingen-pass" });
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.bindGroup);
      const gx = Math.ceil(w / 8);
      const gy = Math.ceil(h / 8);
      pass.dispatchWorkgroups(gx, gy, 1);
      pass.end();

      // §23/§24 — copy this frame's result into the history pixel region (after
      // the 4-vec4 header) so the next frame can reproject / fill from it.
      // Needed by both reprojection and checkerboard ; skipped when both off.
      if ((this.reproject || this.checkerboard) && this.historyBuffer && this.accumBuffer) {
        encoder.copyBufferToBuffer(this.accumBuffer, 0, this.historyBuffer, 64, this.accumPixels * 16);
      }
      // §24 — alternate the checkerboard half each rendered frame.
      this.frameParity ^= 1;

      this.sampleCount += 1;
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

    // §23 — remember this frame's camera for next-frame reprojection.
    this.prevBasis[0] = cam.pos[0]; this.prevBasis[1] = cam.pos[1]; this.prevBasis[2] = cam.pos[2];
    this.prevBasis[4] = cam.fwd[0]; this.prevBasis[5] = cam.fwd[1]; this.prevBasis[6] = cam.fwd[2];
    this.prevBasis[8] = cam.right[0]; this.prevBasis[9] = cam.right[1]; this.prevBasis[10] = cam.right[2];
    this.prevBasis[12] = cam.up[0]; this.prevBasis[13] = cam.up[1]; this.prevBasis[14] = cam.up[2];
    this.prevBasisValid = true;

    this.stats.hitRatio = this.stats.frames > 0 ? this.stats.hits / this.stats.frames : 0;
  }

  /** Snapshot of frame-cache counters. Cheap, can be polled per frame. */
  getStats(): IngenStats {
    return { ...this.stats };
  }

  /**
   * Which GPU adapter WebGPU selected, for diagnostics / HUD. Returns null
   * before init(). `isFallback === true` or an empty vendor means a
   * software/CPU rasteriser is in use (unusable — force the discrete GPU).
   */
  getAdapterInfo(): { vendor: string; architecture: string; description: string; isFallback: boolean } | null {
    return this.adapterInfo ? { ...this.adapterInfo } : null;
  }

  /**
   * True while the progressive accumulator still has samples to integrate
   * for the current static scene. The render host keeps requesting frames
   * until this returns false, then lets the loop go idle — so an untouched
   * viewport sharpens to ultra-HD instead of stopping at 1 spp.
   */
  isConverging(): boolean {
    return !!this.pipeline
      && (this.sampleCount < this.maxSamples
        || (!!this.pipelineProbe && this.probeSample < this.probeMaxSamples));
  }

  /**
   * §20 Fusion v3 — choose how splats composite. `false` (default) keeps the
   * additive glow used by SDF-baked gaussians ; `true` switches to a solid
   * weighted-OIT over-blend so real 3DGS captures read as opaque surfaces.
   * Toggling invalidates the frame cache so the next render reflects it.
   */
  setSplatSolid(on: boolean): void {
    const next = !!on;
    if (next === this.splatSolid) return;
    this.splatSolid = next;
    this.cacheValid = false;
  }

  /**
   * §21 Ocean — enable the analytic water plane at world height `level`, or
   * disable with `null`. While enabled, `time` advances so the waves animate
   * (the loop stays live) ; disabling lets the viewport go idle again.
   * Invalidates the frame cache so the change is shown immediately.
   */
  setWater(level: number | null): void {
    this.waterLevel = (level === null || level === undefined) ? -1e9 : level;
    this.cacheValid = false;
  }

  /**
   * §23 — enable/disable temporal reprojection (default off). When on, a
   * moving view reuses last frame's converged shading for surfaces still on
   * screen (depth-validated) instead of restarting accumulation at 1 spp —
   * so motion stays clean and the post-stop convergence is much shorter.
   * Experimental : watch for ghosting on fast motion ; tune or disable.
   */
  setReproject(on: boolean): void {
    this.reproject = !!on;
    this.cacheValid = false;
  }

  /**
   * §24 — enable/disable checkerboard rendering (default off). When on, a
   * moving view marches only half the pixels each frame (alternating) and
   * reuses the other half from history → ~2× fewer primary marches in motion.
   * Static frames stay full-resolution. Pairs naturally with reprojection ;
   * watch for checkerboard shimmer on fast motion (a sign to add motion-
   * compensated fill for the reused half).
   */
  setCheckerboard(on: boolean): void {
    this.checkerboard = !!on;
    this.cacheValid = false;
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
    this.sdfBrickBuffer?.destroy?.();
    this.accumBuffer?.destroy?.();
    this.historyBuffer?.destroy?.();
    this.shadowBuffer?.destroy?.();
    this.probesBuffer?.destroy?.();
    this.outTexture = null;
    this.outView = null;
    this.camBuffer = null;
    this.opsBuffer = null;
    this.svdagBuffer = null;
    this.svdagCapacity = 0;
    this.splatsBuffer = null;
    this.splatsCapacity = 0;
    this.nsdfBuffer = null;
    this.sdfBrickBuffer = null;
    this.sdfBrickCapacity = 0;
    this.accumBuffer = null;
    this.accumPixels = 0;
    this.historyBuffer = null;
    this.prevBasisValid = false;
    this.shadowBuffer = null;
    this.shadowCapacity = 0;
    this.splatCount = 0;
    this.probesBuffer = null;
    this.probeSample = 0;
    this.pipeline = null;
    this.pipelineShadow = null;
    this.pipelineProbe = null;
    this.auxiliaryPipelinesStarted = false;
    this.bindGroupLayout = null;
    this.bindGroup = null;
    this.context = null;
    this.device = null;
  }
}
