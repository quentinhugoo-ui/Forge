// Banger SDF scenes — INGEN COMPUTE §19.3 (KASM → SDF tree, data form).
//
// The fragment shader interprets a compact opcode buffer instead of a
// hardcoded `scene()` body, so a scene is data (a Float32Array of
// (opcode, params) pairs) and not source code. This is the bridge
// surface that lets `forge_agent` / ForgeSlash emit scenes later
// without touching the WebGL pipeline.
//
// Format mirrors the WGSL stack machine in `catalog.ts::FS_SDF`:
//   each op = 2 vec4 (8 floats)
//   slot[0] = (op_code, p0, p1, p2)
//   slot[1] = (p3, p4, p5, k)
// Postfix order. Binary ops consume the top two stack values; primitives
// push one. The final stack[0] is the signed distance.

export const OP_SPHERE       = 0;
export const OP_BOX          = 1;
export const OP_TORUS        = 2;
export const OP_CAPSULE      = 3;
export const OP_ROUNDED_BOX  = 4;
/** Heightmap terrain : FBM value-noise. dist = p.z - amp*fbm(p.xy*freq) - groundZ. */
export const OP_TERRAIN      = 5;
export const OP_UNION        = 10;
export const OP_INTERSECT    = 11;
export const OP_DIFF         = 12;
export const OP_SMIN         = 13;
/** §11 mesh→SDF bridge : samples the bound 3D texture (uMeshSdf). */
export const OP_SAMPLED_SDF  = 20;
/** Domain repetition : modulates `p` by period.xyz for all subsequent primitives.
 *  period = [0,0,0] resets to no-repeat. 1 tree → forest, 1 brick → wall. */
export const OP_REPEAT       = 30;
/** Sets the current (color, roughness, metallic) — tags every primitive
 *  that follows until the next OP_MATERIAL. Stack-aware : at union /
 *  intersect / diff the winning side's material survives ; at SMIN the
 *  materials lerp by softmin weight (organic blends, not just shapes). */
export const OP_MATERIAL     = 40;
/** §18 Pillar A : Sparse Voxel DAG sample. The op slot encodes a world-
 *  space transform (origin in `center`, side length in `voxelSpan`) ;
 *  the SVDAG buffer itself is bound globally on the GPU (one SVDAG at a
 *  time in Phase 5 ; multi-SVDAG via `rootIndex` arrives in Phase 5b). */
export const OP_SVDAG        = 50;

export const SDF_MAX_OPS = 64;
export const SDF_FLOATS_PER_OP = 8;

export type Vec3 = readonly [number, number, number];

export type SdfOp =
  | { op: "sphere";       center: Vec3; radius: number }
  | { op: "box";          center: Vec3; halfExtents: Vec3 }
  | { op: "torus";        center: Vec3; majorRadius: number; minorRadius: number }
  | { op: "capsule";      a: Vec3; b: Vec3; radius: number }
  | { op: "roundedBox";   center: Vec3; halfExtents: Vec3; cornerRadius: number }
  | { op: "terrain";      amplitude: number; frequency: number; groundZ: number; octaves?: number }
  | { op: "repeat";       period: Vec3 }
  | { op: "material";     color: Vec3; roughness?: number; metallic?: number }
  | { op: "sampledSdf" }
  | { op: "svdag";        center: Vec3; voxelSpan: number; rootIndex?: number }
  | { op: "union" }
  | { op: "intersect" }
  | { op: "diff" }
  | { op: "smin";         k: number };

const OP_CODE: Record<SdfOp["op"], number> = {
  sphere:     OP_SPHERE,
  box:        OP_BOX,
  torus:      OP_TORUS,
  capsule:    OP_CAPSULE,
  roundedBox: OP_ROUNDED_BOX,
  terrain:    OP_TERRAIN,
  repeat:     OP_REPEAT,
  material:   OP_MATERIAL,
  sampledSdf: OP_SAMPLED_SDF,
  svdag:      OP_SVDAG,
  union:      OP_UNION,
  intersect:  OP_INTERSECT,
  diff:       OP_DIFF,
  smin:       OP_SMIN,
};

export interface SerializedScene {
  /** Packed buffer of length SDF_MAX_OPS * SDF_FLOATS_PER_OP. Tail is zero-padded. */
  readonly buffer: Float32Array;
  /** Number of valid op slots (0..SDF_MAX_OPS). */
  readonly count: number;
}

export function serializeScene(ops: readonly SdfOp[]): SerializedScene {
  if (ops.length > SDF_MAX_OPS) {
    throw new Error(`SDF scene too big: ${ops.length} > ${SDF_MAX_OPS}`);
  }
  const buf = new Float32Array(SDF_MAX_OPS * SDF_FLOATS_PER_OP);
  for (let i = 0; i < ops.length; i += 1) {
    const o = ops[i];
    const base = i * SDF_FLOATS_PER_OP;
    buf[base] = OP_CODE[o.op];
    if (o.op === "sphere") {
      buf[base + 1] = o.center[0];
      buf[base + 2] = o.center[1];
      buf[base + 3] = o.center[2];
      buf[base + 4] = o.radius;
    } else if (o.op === "box") {
      buf[base + 1] = o.center[0];
      buf[base + 2] = o.center[1];
      buf[base + 3] = o.center[2];
      buf[base + 4] = o.halfExtents[0];
      buf[base + 5] = o.halfExtents[1];
      buf[base + 6] = o.halfExtents[2];
    } else if (o.op === "torus") {
      // Z-axis aligned : center + major radius (ring) + minor radius (tube).
      buf[base + 1] = o.center[0];
      buf[base + 2] = o.center[1];
      buf[base + 3] = o.center[2];
      buf[base + 4] = o.majorRadius;
      buf[base + 5] = o.minorRadius;
    } else if (o.op === "capsule") {
      // Two endpoints + radius. Endpoint A in slot[0].yzw, B in slot[1].xyz.
      buf[base + 1] = o.a[0];
      buf[base + 2] = o.a[1];
      buf[base + 3] = o.a[2];
      buf[base + 4] = o.b[0];
      buf[base + 5] = o.b[1];
      buf[base + 6] = o.b[2];
      buf[base + 7] = o.radius;
    } else if (o.op === "roundedBox") {
      // Box minus a sphere of radius `cornerRadius` per Inigo Quilez.
      buf[base + 1] = o.center[0];
      buf[base + 2] = o.center[1];
      buf[base + 3] = o.center[2];
      buf[base + 4] = o.halfExtents[0];
      buf[base + 5] = o.halfExtents[1];
      buf[base + 6] = o.halfExtents[2];
      buf[base + 7] = o.cornerRadius;
    } else if (o.op === "terrain") {
      buf[base + 1] = o.amplitude;
      buf[base + 2] = o.frequency;
      buf[base + 3] = o.groundZ;
      buf[base + 4] = Math.max(1, Math.min(6, o.octaves ?? 4));
    } else if (o.op === "repeat") {
      buf[base + 1] = o.period[0];
      buf[base + 2] = o.period[1];
      buf[base + 3] = o.period[2];
    } else if (o.op === "material") {
      buf[base + 1] = o.color[0];
      buf[base + 2] = o.color[1];
      buf[base + 3] = o.color[2];
      buf[base + 4] = Math.max(0.02, Math.min(1.0, o.roughness ?? 0.55));
      buf[base + 5] = Math.max(0.0, Math.min(1.0, o.metallic ?? 0.0));
    } else if (o.op === "smin") {
      buf[base + 4] = o.k;
    } else if (o.op === "svdag") {
      // §18 Pillar A : world-space origin (a.yzw), voxel-grid side length
      // (b.x), root pool index (b.y, defaults to whatever the WGSL header
      // declares — typically 0).
      buf[base + 1] = o.center[0];
      buf[base + 2] = o.center[1];
      buf[base + 3] = o.center[2];
      buf[base + 4] = o.voxelSpan;
      buf[base + 5] = o.rootIndex ?? 0;
    }
    // union/intersect/diff/sampledSdf carry no params.
  }
  return { buffer: buf, count: ops.length };
}

/** Default scene matching the previous hardcoded shader body :
 *  two spheres + smooth union (k=5) centered at XY=0, same Z. */
export const DEFAULT_SCENE: SdfOp[] = [
  { op: "sphere", center: [-0.7, 0.0, 0.0], radius: 0.7 },
  { op: "sphere", center: [ 0.7, 0.0, 0.0], radius: 0.7 },
  { op: "smin", k: 5.0 },
];

// ---------- Auto-recentrage XY (Z préservé) -------------------------------
//
// Doctrine Banger : tout nouvel objet (création SDF / import mesh / animation
// importée) atterrit avec son centroïde XY à (0, 0). L'axe Z conserve la
// hauteur d'origine. Un seul helper par représentation, appelé au chokepoint
// d'entrée — pas de transform stockée sur la scène, pas de matrice supplémen-
// taire à propager dans le shader : on bake directement dans la donnée.

/** Returns ops whose positioned primitives have been shifted so the XY
 *  centroid of their centers lands on (0, 0). Z is untouched. Non-positioned
 *  ops (union/diff/smin/material/terrain/repeat/sampledSdf) pass through.
 *  Capsule endpoints both shift by the same amount. Idempotent : applying
 *  twice yields the same result. */
export function recenterSceneXY(ops: readonly SdfOp[]): SdfOp[] {
  let sumX = 0;
  let sumY = 0;
  let n = 0;
  for (const o of ops) {
    if (o.op === "sphere" || o.op === "box" || o.op === "torus" || o.op === "roundedBox") {
      sumX += o.center[0]; sumY += o.center[1]; n += 1;
    } else if (o.op === "capsule") {
      sumX += 0.5 * (o.a[0] + o.b[0]);
      sumY += 0.5 * (o.a[1] + o.b[1]);
      n += 1;
    }
  }
  if (n === 0) return ops.slice();
  const dx = sumX / n;
  const dy = sumY / n;
  if (Math.abs(dx) < 1e-6 && Math.abs(dy) < 1e-6) return ops.slice();
  return ops.map((o): SdfOp => {
    if (o.op === "sphere") return { ...o, center: [o.center[0] - dx, o.center[1] - dy, o.center[2]] };
    if (o.op === "box") return { ...o, center: [o.center[0] - dx, o.center[1] - dy, o.center[2]] };
    if (o.op === "torus") return { ...o, center: [o.center[0] - dx, o.center[1] - dy, o.center[2]] };
    if (o.op === "roundedBox") return { ...o, center: [o.center[0] - dx, o.center[1] - dy, o.center[2]] };
    if (o.op === "capsule") return {
      ...o,
      a: [o.a[0] - dx, o.a[1] - dy, o.a[2]],
      b: [o.b[0] - dx, o.b[1] - dy, o.b[2]],
    };
    return o;
  });
}

/** Mesh equivalent : shifts every vertex's X/Y by -centroid, Z untouched.
 *  Returns a new meshData with fresh `pos` (cloned), recomputed `bounds`
 *  if present, plus an `appliedRecenterXY` tag so debug tools can see the
 *  shift. `nrm` and `count` and `faceCount` are reused as-is. */
export interface RecenterableMesh {
  pos: Float32Array;
  nrm?: Float32Array;
  count?: number;
  faceCount?: number;
  bounds?: { min: [number, number, number]; max: [number, number, number] };
  [extra: string]: unknown;
}

export function recenterMeshXY<T extends RecenterableMesh>(meshData: T): T & { appliedRecenterXY: [number, number] } {
  const src = meshData.pos;
  const len = src.length;
  if (len === 0) return { ...meshData, appliedRecenterXY: [0, 0] };
  let sumX = 0;
  let sumY = 0;
  const vCount = (len / 3) | 0;
  for (let i = 0; i < len; i += 3) {
    sumX += src[i] ?? 0;
    sumY += src[i + 1] ?? 0;
  }
  const dx = sumX / Math.max(1, vCount);
  const dy = sumY / Math.max(1, vCount);
  if (Math.abs(dx) < 1e-6 && Math.abs(dy) < 1e-6) {
    return { ...meshData, appliedRecenterXY: [0, 0] };
  }
  const pos = new Float32Array(len);
  for (let i = 0; i < len; i += 3) {
    pos[i]     = (src[i] ?? 0) - dx;
    pos[i + 1] = (src[i + 1] ?? 0) - dy;
    pos[i + 2] = src[i + 2] ?? 0;
  }
  const out: any = { ...meshData, pos, appliedRecenterXY: [dx, dy] };
  if (meshData.bounds) {
    out.bounds = {
      min: [meshData.bounds.min[0] - dx, meshData.bounds.min[1] - dy, meshData.bounds.min[2]],
      max: [meshData.bounds.max[0] - dx, meshData.bounds.max[1] - dy, meshData.bounds.max[2]],
    };
  }
  return out as T & { appliedRecenterXY: [number, number] };
}

// ---------- CPU SDF evaluator -------------------------------------------------
//
// Mirrors the GLSL stack machine in catalog.ts::FS_SDF byte-for-byte so that
// scenes.ts stays the single source of truth for SDF semantics. Used by the
// Gaussian sampler below (§19.5 splatting) ; future agents that need raycast
// / picker / collision can reuse it without a GPU round-trip.
//
// OP_SAMPLED_SDF (20) is NOT evaluable on CPU (the 3D texture only lives on
// GPU) ; it returns a large positive value so the sampler skips that region.

const TS_STACK_MAX = 16;

function sdSphere(p: Vec3, r: number): number {
  return Math.hypot(p[0], p[1], p[2]) - r;
}
function sdBox(p: Vec3, b: Vec3): number {
  const qx = Math.abs(p[0]) - b[0];
  const qy = Math.abs(p[1]) - b[1];
  const qz = Math.abs(p[2]) - b[2];
  const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0), Math.max(qz, 0));
  const inside  = Math.min(Math.max(qx, Math.max(qy, qz)), 0);
  return outside + inside;
}
function sdTorus(p: Vec3, R: number, r: number): number {
  return Math.hypot(Math.hypot(p[0], p[1]) - R, p[2]) - r;
}
function sdCapsule(p: Vec3, a: Vec3, b: Vec3, r: number): number {
  const pax = p[0] - a[0], pay = p[1] - a[1], paz = p[2] - a[2];
  const bax = b[0] - a[0], bay = b[1] - a[1], baz = b[2] - a[2];
  const baba = bax * bax + bay * bay + baz * baz;
  const h = Math.max(0, Math.min(1, (pax * bax + pay * bay + paz * baz) / Math.max(baba, 1e-6)));
  return Math.hypot(pax - bax * h, pay - bay * h, paz - baz * h) - r;
}
function sdRoundedBox(p: Vec3, b: Vec3, r: number): number {
  return sdBox(p, [b[0] - r, b[1] - r, b[2] - r]) - r;
}
function smin(a: number, b: number, k: number): number {
  const m = Math.min(a, b);
  return m - Math.log(Math.exp(-k * (a - m)) + Math.exp(-k * (b - m))) / k;
}

// Value noise — mirror of the GLSL hash21/vnoise/fbm in catalog.ts.
// Identical math byte-for-byte so the TS evaluator and the WGSL shader
// agree on terrain heights (within f32 precision).
function hash21(x: number, y: number): number {
  let px = (x * 123.45) % 1;
  let py = (y * 678.91) % 1;
  if (px < 0) px += 1;
  if (py < 0) py += 1;
  const d = px * px + py * py + 45.32;
  px += d; py += d;
  return ((px * py) % 1 + 1) % 1;
}
function vnoise2(x: number, y: number): number {
  const ix = Math.floor(x), iy = Math.floor(y);
  const fx = x - ix, fy = y - iy;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const a = hash21(ix,     iy    );
  const b = hash21(ix + 1, iy    );
  const c = hash21(ix,     iy + 1);
  const d = hash21(ix + 1, iy + 1);
  return (a * (1 - sx) + b * sx) * (1 - sy) + (c * (1 - sx) + d * sx) * sy;
}
function fbm2(x: number, y: number, octaves: number): number {
  let v = 0, a = 0.5;
  for (let i = 0; i < octaves; i += 1) {
    v += a * vnoise2(x, y);
    x *= 2; y *= 2; a *= 0.5;
  }
  return v;
}

function applyRepeat(p: Vec3, period: Vec3): Vec3 {
  // period = 0 on an axis means "no repeat on this axis".
  return [
    period[0] > 0 ? p[0] - period[0] * Math.round(p[0] / period[0]) : p[0],
    period[1] > 0 ? p[1] - period[1] * Math.round(p[1] / period[1]) : p[1],
    period[2] > 0 ? p[2] - period[2] * Math.round(p[2] / period[2]) : p[2],
  ];
}

interface SceneHit {
  /** Signed distance to the closest surface. */
  d: number;
  /** Surface albedo at the winning stack entry. */
  color: Vec3;
  /** [roughness, metallic] of the winning entry. */
  rm: [number, number];
}

const DEFAULT_COLOR: Vec3 = [0.84, 0.85, 0.90];
const DEFAULT_RM: [number, number] = [0.55, 0.0];

/** Full evaluator : returns distance + material at `point`. */
export function evaluateSceneFull(scene: readonly SdfOp[], point: Vec3): SceneHit {
  const dS = new Float64Array(TS_STACK_MAX);
  const cS: Vec3[] = new Array(TS_STACK_MAX) as Vec3[];
  const mS: [number, number][] = new Array(TS_STACK_MAX) as [number, number][];
  let sp = 0;
  let cp: Vec3 = [point[0], point[1], point[2]];
  let curColor: Vec3 = DEFAULT_COLOR;
  let curRM: [number, number] = DEFAULT_RM;
  const push = (d: number) => {
    dS[sp] = d; cS[sp] = curColor; mS[sp] = curRM; sp += 1;
  };
  for (const op of scene) {
    if (sp >= TS_STACK_MAX) break;
    if (op.op === "sphere") {
      push(sdSphere([cp[0] - op.center[0], cp[1] - op.center[1], cp[2] - op.center[2]], op.radius));
    } else if (op.op === "box") {
      push(sdBox([cp[0] - op.center[0], cp[1] - op.center[1], cp[2] - op.center[2]], op.halfExtents));
    } else if (op.op === "torus") {
      push(sdTorus([cp[0] - op.center[0], cp[1] - op.center[1], cp[2] - op.center[2]], op.majorRadius, op.minorRadius));
    } else if (op.op === "capsule") {
      push(sdCapsule(cp, op.a, op.b, op.radius));
    } else if (op.op === "roundedBox") {
      push(sdRoundedBox([cp[0] - op.center[0], cp[1] - op.center[1], cp[2] - op.center[2]], op.halfExtents, op.cornerRadius));
    } else if (op.op === "terrain") {
      const h = fbm2(cp[0] * op.frequency, cp[1] * op.frequency, op.octaves ?? 4);
      push(cp[2] - op.amplitude * h - op.groundZ);
    } else if (op.op === "repeat") {
      cp = applyRepeat([point[0], point[1], point[2]], op.period);
    } else if (op.op === "material") {
      curColor = [op.color[0], op.color[1], op.color[2]];
      curRM = [
        Math.max(0.02, Math.min(1, op.roughness ?? DEFAULT_RM[0])),
        Math.max(0,    Math.min(1, op.metallic  ?? DEFAULT_RM[1])),
      ];
    } else if (op.op === "sampledSdf") {
      push(1e6); // GPU-only ; skip
    } else if (op.op === "union") {
      sp -= 1;
      if (dS[sp] < dS[sp - 1]) { dS[sp - 1] = dS[sp]; cS[sp - 1] = cS[sp]; mS[sp - 1] = mS[sp]; }
    } else if (op.op === "intersect") {
      sp -= 1;
      if (dS[sp] > dS[sp - 1]) { dS[sp - 1] = dS[sp]; cS[sp - 1] = cS[sp]; mS[sp - 1] = mS[sp]; }
    } else if (op.op === "diff") {
      sp -= 1;
      const negB = -dS[sp];
      if (negB > dS[sp - 1]) dS[sp - 1] = negB; // material follows A (carved-out face)
    } else if (op.op === "smin") {
      sp -= 1;
      const ad = dS[sp - 1], bd = dS[sp];
      const k = op.k;
      const m = Math.min(ad, bd);
      const ea = Math.exp(-k * (ad - m));
      const eb = Math.exp(-k * (bd - m));
      const w = ea / (ea + eb); // weight of A
      dS[sp - 1] = m - Math.log(ea + eb) / k;
      const ca = cS[sp - 1], cb = cS[sp];
      cS[sp - 1] = [ca[0] * w + cb[0] * (1 - w), ca[1] * w + cb[1] * (1 - w), ca[2] * w + cb[2] * (1 - w)];
      const ma = mS[sp - 1], mb = mS[sp];
      mS[sp - 1] = [ma[0] * w + mb[0] * (1 - w), ma[1] * w + mb[1] * (1 - w)];
    }
  }
  if (sp <= 0) return { d: 1e9, color: DEFAULT_COLOR, rm: DEFAULT_RM };
  return { d: dS[0], color: cS[0] ?? DEFAULT_COLOR, rm: mS[0] ?? DEFAULT_RM };
}

/** Returns the signed distance of `point` to the SDF scene (back-compat wrapper). */
export function evaluateScene(scene: readonly SdfOp[], point: Vec3): number {
  return evaluateSceneFull(scene, point).d;
}

// ---------- Gaussian splat baking (INGEN §19.5) ------------------------------
//
// Real Gaussian Splatting math (isotropic for V1 ; per-pixel projection +
// exp(-r²/2σ²) accumulation lives inside FS_SDF). The Gaussians are sampled
// FROM the SDF surface — they're a derived view, not an independent asset.

export const SDF_MAX_GAUSSIANS = 64;

export interface BakedGaussians {
  /** Packed Float32Array, 8 floats per gaussian : (pos.xyz, scale, color.rgb, opacity). */
  readonly buffer: Float32Array;
  readonly count: number;
}

export interface BakeOptions {
  /** Cube half-extent around origin where samples are drawn. Default 1.5. */
  readonly searchRadius?: number;
  /** Surface tolerance after one Newton step. Default 0.05. */
  readonly tolerance?: number;
  /** Per-gaussian screen scale (world units). Default 0.07. */
  readonly scale?: number;
}

export function bakeGaussiansOnSurface(
  scene: readonly SdfOp[],
  count: number,
  opts: BakeOptions = {},
): BakedGaussians {
  const n = Math.min(SDF_MAX_GAUSSIANS, Math.max(0, count | 0));
  const buf = new Float32Array(SDF_MAX_GAUSSIANS * 8);
  if (n === 0 || scene.length === 0) return { buffer: buf, count: 0 };

  const R = Math.max(0.01, opts.searchRadius ?? 1.5);
  const tol = Math.max(1e-4, opts.tolerance ?? 0.05);
  const scl = Math.max(1e-3, opts.scale ?? 0.07);
  const eps = 0.0015;

  let written = 0;
  let attempts = 0;
  const maxAttempts = n * 200;
  while (written < n && attempts < maxAttempts) {
    attempts += 1;
    const p: Vec3 = [
      (Math.random() * 2 - 1) * R,
      (Math.random() * 2 - 1) * R,
      (Math.random() * 2 - 1) * R,
    ];
    const d = evaluateScene(scene, p);
    if (!Number.isFinite(d) || Math.abs(d) > R) continue;
    const dx = evaluateScene(scene, [p[0] + eps, p[1], p[2]]) - d;
    const dy = evaluateScene(scene, [p[0], p[1] + eps, p[2]]) - d;
    const dz = evaluateScene(scene, [p[0], p[1], p[2] + eps]) - d;
    const glen = Math.hypot(dx, dy, dz);
    if (glen < 1e-3) continue;
    const nx = dx / glen, ny = dy / glen, nz = dz / glen;
    // One Newton step along the gradient — snaps onto the iso-surface.
    const sx = p[0] - nx * d;
    const sy = p[1] - ny * d;
    const sz = p[2] - nz * d;
    const hit = evaluateSceneFull(scene, [sx, sy, sz]);
    if (Math.abs(hit.d) > tol) continue;
    const base = written * 8;
    buf[base + 0] = sx;
    buf[base + 1] = sy;
    buf[base + 2] = sz;
    buf[base + 3] = scl;
    // Splat colour = surface material, lightly modulated by normal so
    // the cloud still has gradient cues. Falls back to the default grey
    // when no OP_MATERIAL was set in the scene.
    buf[base + 4] = Math.max(0, Math.min(1, hit.color[0] * (0.7 + nx * 0.15)));
    buf[base + 5] = Math.max(0, Math.min(1, hit.color[1] * (0.7 + ny * 0.15)));
    buf[base + 6] = Math.max(0, Math.min(1, hit.color[2] * (0.7 + nz * 0.15)));
    buf[base + 7] = 0.55; // opacity
    written += 1;
  }
  return { buffer: buf, count: written };
}
