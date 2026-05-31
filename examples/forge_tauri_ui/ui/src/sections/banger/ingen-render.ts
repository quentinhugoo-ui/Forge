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
  _p1:           f32,
  up:            vec3<f32>,
  _p2:           f32,
  resolution:    vec2<f32>,
  centerOffset:  vec2<f32>,
  sampleIndex:   f32,
  splatSolid:    f32,
  probeSample:   f32,
  _p5:           f32,
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

// Environment radiance for a ray that escapes the scene : a sky gradient
// (Z-up) plus a soft sun glow. Doubles as the indirect "infinite bounce"
// fill term, so surfaces pick up coloured ambient light.
fn sky_env(rd: vec3<f32>) -> vec3<f32> {
  let up = clamp(rd.z * 0.5 + 0.5, 0.0, 1.0);
  var s = mix(vec3<f32>(0.18, 0.20, 0.26), vec3<f32>(0.42, 0.56, 0.86), up);
  let sun = normalize(vec3<f32>(0.5, 0.4, 0.85));
  s = s + vec3<f32>(1.0, 0.92, 0.74) * (pow(max(dot(rd, sun), 0.0), 64.0) * 0.6);
  return s;
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

  // Sky background : an escaped primary ray shows the same environment the
  // GI samples (horizon gradient + sun glow), so the scene reads as a lit
  // world instead of flat void. The grid / surface draw over it below.
  var col = sky_env(dir);

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
    let albedo  = vec3<f32>(0.80, 0.82, 0.86);
    let sun_col = vec3<f32>(1.00, 0.96, 0.88) * 1.6;

    let ndl = max(dot(n, ljit), 0.0);
    var sh = 1.0;
    var ao = 1.0;
    if (si > 0u) {
      if (ndl > 0.0) { sh = soft_shadow(p + n * 0.012, ljit, 0.02, 60.0, 12.0); }
      ao = calc_ao(p, n);
    }
    let direct = albedo * sun_col * (ndl * sh);
    let indirect = albedo * sample_probe(p + n * 0.15) * ao;
    col = min(direct + indirect, vec3<f32>(2.0));
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
  var mean: vec3<f32>;
  if (si == 0u) {
    mean = col;
  } else {
    mean = mix(accum[pix].xyz, col, 1.0 / f32(si + 1u));
  }
  accum[pix] = vec4<f32>(mean, 1.0);
  textureStore(outTex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(mean, 1.0));
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
  private bindGroupLayout: GPUBindGroupLayout | null = null;
  private camBuffer: GPUBuffer | null = null;
  private opsBuffer: GPUBuffer | null = null;
  private svdagBuffer: GPUBuffer | null = null;
  private svdagCapacity = 0;
  private splatsBuffer: GPUBuffer | null = null;
  private splatsCapacity = 0;
  private nsdfBuffer: GPUBuffer | null = null;
  // §19.4 progressive accumulator. One vec4 per pixel ; recreated on resize.
  private accumBuffer: GPUBuffer | null = null;
  private accumPixels = 0;
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
    this.pipelineShadow = this.device.createComputePipeline({
      label: "ingen-shadow-pipeline",
      layout: pipelineLayout,
      compute: { module, entryPoint: "cs_shadow" },
    });
    this.pipelineProbe = this.device.createComputePipeline({
      label: "ingen-probe-pipeline",
      layout: pipelineLayout,
      compute: { module, entryPoint: "cs_probe" },
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
    const pixels = w * h;
    if (pixels !== this.accumPixels) {
      this.accumBuffer?.destroy?.();
      this.accumBuffer = this.device.createBuffer({
        size: pixels * 16, // vec4<f32> per pixel
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        label: "ingen-accum",
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
    this.bindGroup = this.device.createBindGroup({
      label: "ingen-bg",
      layout: this.bindGroupLayout!,
      entries: [
        { binding: 0, resource: { buffer: this.camBuffer! } },
        { binding: 1, resource: { buffer: this.opsBuffer! } },
        { binding: 2, resource: this.outView! },
        { binding: 3, resource: { buffer: this.svdagBuffer! } },
        { binding: 4, resource: { buffer: this.splatsBuffer! } },
        { binding: 5, resource: { buffer: this.nsdfBuffer! } },
        { binding: 6, resource: { buffer: this.accumBuffer! } },
        { binding: 7, resource: { buffer: this.shadowBuffer! } },
        { binding: 8, resource: { buffer: this.probesBuffer! } },
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
    if (realloced && this.bindGroupLayout && this.outView && this.camBuffer && this.opsBuffer && this.svdagBuffer && this.nsdfBuffer && this.accumBuffer && this.shadowBuffer && this.probesBuffer) {
      this.bindGroup = this.device.createBindGroup({
        label: "ingen-bg",
        layout: this.bindGroupLayout,
        entries: [
          { binding: 0, resource: { buffer: this.camBuffer } },
          { binding: 1, resource: { buffer: this.opsBuffer } },
          { binding: 2, resource: this.outView },
          { binding: 3, resource: { buffer: this.svdagBuffer } },
          { binding: 4, resource: { buffer: this.splatsBuffer! } },
          { binding: 5, resource: { buffer: this.nsdfBuffer } },
          { binding: 6, resource: { buffer: this.accumBuffer } },
          { binding: 7, resource: { buffer: this.shadowBuffer } },
          { binding: 8, resource: { buffer: this.probesBuffer } },
        ],
      });
    }
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
      if (this.bindGroupLayout && this.outView && this.camBuffer && this.opsBuffer && this.splatsBuffer && this.nsdfBuffer && this.accumBuffer && this.shadowBuffer && this.probesBuffer) {
        this.bindGroup = this.device.createBindGroup({
          label: "ingen-bg",
          layout: this.bindGroupLayout,
          entries: [
            { binding: 0, resource: { buffer: this.camBuffer } },
            { binding: 1, resource: { buffer: this.opsBuffer } },
            { binding: 2, resource: this.outView },
            { binding: 3, resource: { buffer: this.svdagBuffer! } },
            { binding: 4, resource: { buffer: this.splatsBuffer } },
            { binding: 5, resource: { buffer: this.nsdfBuffer } },
            { binding: 6, resource: { buffer: this.accumBuffer } },
            { binding: 7, resource: { buffer: this.shadowBuffer } },
            { binding: 8, resource: { buffer: this.probesBuffer } },
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
    camU[8] = cam.right[0]; camU[9] = cam.right[1]; camU[10] = cam.right[2]; camU[11] = 0;
    camU[12] = cam.up[0]; camU[13] = cam.up[1]; camU[14] = cam.up[2]; camU[15] = 0;
    camU[16] = w; camU[17] = h; camU[18] = cam.centerOffset?.[0] ?? 0; camU[19] = cam.centerOffset?.[1] ?? 0;
    // sampleIndex (camU[20]) is filled in just before dispatch — it must NOT
    // enter the hash, otherwise every accumulation frame would look "new"
    // and the scene would never be detected as idle. splatSolid (camU[21])
    // IS hashed so flipping the compositing intent forces a fresh render.
    camU[20] = 0; camU[21] = this.splatSolid ? 1 : 0; camU[22] = 0; camU[23] = 0;
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
    const probeBaking = this.probeSample < this.probeMaxSamples;
    // Dispatch whenever the image must change : a moved/converging view, OR a
    // still-baking probe volume (so the main pass re-runs to show the refined
    // GI). Camera orbit with frozen probes still falls under `converging`.
    if (sceneChanged || converging || probeBaking) {
      camU[20] = this.sampleCount;  // primary AA / jitter seed
      camU[22] = this.probeSample;  // probe bake accumulation seed
      this.device.queue.writeBuffer(this.camBuffer!, 0, camU);

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
      && (this.sampleCount < this.maxSamples || this.probeSample < this.probeMaxSamples);
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
    this.accumBuffer?.destroy?.();
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
    this.accumBuffer = null;
    this.accumPixels = 0;
    this.shadowBuffer = null;
    this.shadowCapacity = 0;
    this.splatCount = 0;
    this.probesBuffer = null;
    this.probeSample = 0;
    this.pipeline = null;
    this.pipelineShadow = null;
    this.pipelineProbe = null;
    this.bindGroupLayout = null;
    this.bindGroup = null;
    this.context = null;
    this.device = null;
  }
}
