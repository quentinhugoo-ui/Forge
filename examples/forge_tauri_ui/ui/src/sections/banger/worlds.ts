// Banger world-building library — composite SDF primitives that the
// agent / ForgeSlash can compose into landscapes, cities, vehicles,
// etc. Pure TS sugar on top of scenes.ts opcodes — no shader changes,
// no new state. Each function returns an SdfOp[] postfix subprogram
// that can be concatenated and ended with the right number of unions.
//
// Doctrine compact : these helpers are stateless and additive ; deleting
// any one of them costs nothing (no caller outside the agent layer).
// Adding a new biome / building type = ~10 lines here.

import type { SdfOp, Vec3 } from "./scenes.js";

const v3 = (x: number, y: number, z: number): Vec3 => [x, y, z];

/** Mulberry32 — tiny deterministic RNG so worlds are reproducible
 *  from a single seed (the agent can store / re-emit the seed). */
export function seededRandom(seed: number): () => number {
  let s = (seed | 0) || 1;
  return () => {
    s = (s + 0x6D2B79F5) | 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Shared material palette — tweak here and every helper picks it up.
export const MATERIALS = {
  grass:    { color: v3(0.26, 0.46, 0.20), roughness: 0.85, metallic: 0.0 },
  bark:     { color: v3(0.36, 0.22, 0.13), roughness: 0.95, metallic: 0.0 },
  foliage:  { color: v3(0.20, 0.55, 0.24), roughness: 0.65, metallic: 0.0 },
  stone:    { color: v3(0.70, 0.68, 0.62), roughness: 0.80, metallic: 0.0 },
  brick:    { color: v3(0.65, 0.30, 0.22), roughness: 0.88, metallic: 0.0 },
  roof:     { color: v3(0.50, 0.18, 0.12), roughness: 0.70, metallic: 0.0 },
  glass:    { color: v3(0.55, 0.70, 0.80), roughness: 0.10, metallic: 0.0 },
  metal:    { color: v3(0.85, 0.85, 0.88), roughness: 0.30, metallic: 0.95 },
  rubber:   { color: v3(0.08, 0.08, 0.09), roughness: 0.95, metallic: 0.0 },
  chrome:   { color: v3(0.92, 0.93, 0.95), roughness: 0.05, metallic: 1.0 },
  paint_red:{ color: v3(0.72, 0.12, 0.10), roughness: 0.35, metallic: 0.20 },
} as const;

function mat(spec: { color: Vec3; roughness: number; metallic: number }): SdfOp {
  return { op: "material", color: spec.color, roughness: spec.roughness, metallic: spec.metallic };
}

/** Ground plane via FBM heightmap. amplitude in world units (peak
 *  height), frequency tunes the bumpiness (higher = noisier). */
export function terrain(amplitude = 1.2, frequency = 0.08, groundZ = -1.0, octaves = 4): SdfOp[] {
  return [
    mat(MATERIALS.grass),
    { op: "terrain", amplitude, frequency, groundZ, octaves },
  ];
}

/** Single tree : capsule trunk (bark) + sphere canopy (foliage),
 *  smin-blended. The smin auto-lerps bark→foliage at the join. */
export function tree(at: Vec3, height = 1.4, canopyRadius = 0.45): SdfOp[] {
  const trunkTop = v3(at[0], at[1], at[2] + height * 0.55);
  return [
    mat(MATERIALS.bark),
    { op: "capsule", a: at, b: trunkTop, radius: 0.07 },
    mat(MATERIALS.foliage),
    { op: "sphere", center: v3(at[0], at[1], at[2] + height), radius: canopyRadius },
    { op: "smin", k: 6.0 },
  ];
}

/** Box house with a sphere roof — brick walls, red roof, doorway carved. */
export function house(at: Vec3, size = 1.0): SdfOp[] {
  const half = size * 0.5;
  const wallZ = at[2] + half;
  return [
    mat(MATERIALS.brick),
    { op: "roundedBox", center: v3(at[0], at[1], wallZ), halfExtents: v3(half, half, half), cornerRadius: 0.04 },
    mat(MATERIALS.roof),
    { op: "sphere", center: v3(at[0], at[1], wallZ + half * 0.85), radius: half * 0.7 },
    { op: "union" },
    // doorway carved out of the front face — diff keeps wall material
    { op: "box", center: v3(at[0], at[1] - half, wallZ - half * 0.35), halfExtents: v3(half * 0.18, half * 0.05, half * 0.3) },
    { op: "diff" },
  ];
}

/** Simple vehicle : painted body + chrome wheels. Type tweaks the
 *  proportions (car = wide low, truck = tall, hover = floating no wheels). */
export function vehicle(at: Vec3, kind: "car" | "truck" | "hover" = "car"): SdfOp[] {
  const cfg = kind === "truck"
    ? { body: v3(0.55, 1.10, 0.35), wheelRadius: 0.18, wheelInset: 0.42, paint: MATERIALS.metal }
    : kind === "hover"
    ? { body: v3(0.50, 1.00, 0.22), wheelRadius: 0,    wheelInset: 0,    paint: MATERIALS.chrome }
    : { body: v3(0.45, 0.95, 0.20), wheelRadius: 0.13, wheelInset: 0.34, paint: MATERIALS.paint_red };
  const bodyZ = at[2] + cfg.body[2] + cfg.wheelRadius;
  const ops: SdfOp[] = [
    mat(cfg.paint),
    { op: "roundedBox", center: v3(at[0], at[1], bodyZ), halfExtents: cfg.body, cornerRadius: 0.06 },
  ];
  if (cfg.wheelRadius > 0) {
    const wx = cfg.body[0] * 0.9, wy = cfg.body[1] * 0.7;
    const wz = at[2] + cfg.wheelRadius;
    const wheelPoses: Vec3[][] = [
      [v3(at[0] - wx, at[1] - wy, wz - cfg.wheelInset), v3(at[0] - wx, at[1] - wy, wz + cfg.wheelInset)],
      [v3(at[0] + wx, at[1] - wy, wz - cfg.wheelInset), v3(at[0] + wx, at[1] - wy, wz + cfg.wheelInset)],
      [v3(at[0] - wx, at[1] + wy, wz - cfg.wheelInset), v3(at[0] - wx, at[1] + wy, wz + cfg.wheelInset)],
      [v3(at[0] + wx, at[1] + wy, wz - cfg.wheelInset), v3(at[0] + wx, at[1] + wy, wz + cfg.wheelInset)],
    ];
    ops.push(mat(MATERIALS.rubber));
    for (const [a, b] of wheelPoses) {
      ops.push({ op: "capsule", a, b, radius: cfg.wheelRadius });
      ops.push({ op: "union" });
    }
  }
  return ops;
}

/** Forest via OP_REPEAT — one tree primitive becomes an infinite grid
 *  in the XY plane. The bounds parameter is purely informational here
 *  (REPEAT itself is infinite ; render distance is bounded by fog +
 *  the raymarch far plane). */
export function forest(spacing = 4.0, treeHeight = 1.8, canopyRadius = 0.55): SdfOp[] {
  return [
    { op: "repeat", period: v3(spacing, spacing, 0) },
    ...tree(v3(0, 0, 0), treeHeight, canopyRadius),
    { op: "repeat", period: v3(0, 0, 0) }, // reset
  ];
}

/** Wall : infinite line of rounded bricks along the X axis. */
export function wall(spacing = 0.6, height = 1.2): SdfOp[] {
  return [
    { op: "repeat", period: v3(spacing, 0, 0) },
    { op: "roundedBox", center: v3(0, 0, height * 0.5), halfExtents: v3(spacing * 0.45, 0.18, height * 0.5), cornerRadius: 0.04 },
    { op: "repeat", period: v3(0, 0, 0) },
  ];
}

/** Distributed scatter of one composite primitive (e.g. houses) over a
 *  bounded XY rectangle, using a seeded RNG so the layout is
 *  reproducible. Useful when REPEAT is too uniform. */
export function scatter(
  bounds: { min: Vec3; max: Vec3 },
  count: number,
  emit: (at: Vec3) => SdfOp[],
  seed = 1337,
): SdfOp[] {
  const rng = seededRandom(seed);
  const ops: SdfOp[] = [];
  let placed = 0;
  for (let i = 0; i < count; i += 1) {
    const at: Vec3 = [
      bounds.min[0] + (bounds.max[0] - bounds.min[0]) * rng(),
      bounds.min[1] + (bounds.max[1] - bounds.min[1]) * rng(),
      bounds.min[2] + (bounds.max[2] - bounds.min[2]) * rng(),
    ];
    const sub = emit(at);
    if (!sub.length) continue;
    ops.push(...sub);
    if (placed > 0) ops.push({ op: "union" });
    placed += 1;
  }
  return ops;
}

/** INGEN-native showcase scene — uses ONLY ops the WebGPU raymarcher
 *  supports (sphere / capsule / torus / smin / union ; no terrain /
 *  material / repeat). A smin-blended mountain, scattered boulders, a ring
 *  of trees, a torus arch and floating spheres — built to exercise the new
 *  lighting (soft shadows, AO, GI radiance cache, sky). ~52 ops, well under
 *  the 128-op budget. Objects rest on the z=0 grid floor. */
export function showcaseScene(): SdfOp[] {
  const ops: SdfOp[] = [];

  // Central mountain : smin-blended spheres rising from the grid.
  ops.push({ op: "sphere", center: v3(0, 0, 1.5), radius: 4.2 });
  ops.push({ op: "sphere", center: v3(3.6, 2.2, 0.4), radius: 2.8 });
  ops.push({ op: "smin", k: 2.4 });
  ops.push({ op: "sphere", center: v3(-3.2, -2.4, 0.7), radius: 3.0 });
  ops.push({ op: "smin", k: 2.4 });
  ops.push({ op: "sphere", center: v3(-1.2, 3.4, 1.0), radius: 2.2 });
  ops.push({ op: "smin", k: 2.0 });
  ops.push({ op: "sphere", center: v3(1.6, -3.2, 0.6), radius: 2.0 });
  ops.push({ op: "smin", k: 2.0 });

  // Boulders scattered around the base.
  const boulders: { c: Vec3; r: number }[] = [
    { c: v3(7, -4.5, 0.66), r: 1.2 },
    { c: v3(-7, 4.5, 0.82), r: 1.5 },
    { c: v3(5.5, 5.5, 0.55), r: 1.0 },
    { c: v3(-5.5, -5.5, 0.72), r: 1.3 },
    { c: v3(8, 1, 0.50), r: 0.9 },
  ];
  for (const b of boulders) {
    ops.push({ op: "sphere", center: b.c, radius: b.r });
    ops.push({ op: "union" });
  }

  // Ring of trees (capsule trunk + sphere canopy, smin-blended).
  const treeCount = 7;
  for (let i = 0; i < treeCount; i += 1) {
    const a = (i / treeCount) * Math.PI * 2;
    const x = Math.cos(a) * 9.5;
    const y = Math.sin(a) * 9.5;
    const h = 1.6 + (i % 3) * 0.3;
    ops.push({ op: "capsule", a: v3(x, y, 0), b: v3(x, y, h), radius: 0.12 });
    ops.push({ op: "sphere", center: v3(x, y, h + 0.5), radius: 0.7 });
    ops.push({ op: "smin", k: 5.0 });
    ops.push({ op: "union" });
  }

  // Torus arch + a couple of floating spheres (shadow / GI showcase).
  ops.push({ op: "torus", center: v3(0, -8.5, 3.0), majorRadius: 2.6, minorRadius: 0.35 });
  ops.push({ op: "union" });
  ops.push({ op: "sphere", center: v3(4, 0, 5.5), radius: 0.9 });
  ops.push({ op: "union" });
  ops.push({ op: "sphere", center: v3(-4, 1, 4.5), radius: 0.7 });
  ops.push({ op: "union" });

  return ops;
}

/** Atmospheric world preset : terrain + scattered trees + a few houses.
 *  Returns a complete SdfOp[] ready for __forgeBangerSetScene. The
 *  caller still has to enable sky / fog separately (those are global
 *  toggles, not part of the scene tree). */
export function defaultLandscape(seed = 42): SdfOp[] {
  return [
    ...terrain(1.5, 0.08, -0.4, 4),
    ...scatter(
      { min: v3(-8, -8, 0.6), max: v3(8, 8, 0.6) },
      18,
      (at) => tree(at, 1.4 + (seed % 7) * 0.05, 0.4),
      seed,
    ),
    { op: "union" },
    ...scatter(
      { min: v3(-6, -6, 0), max: v3(6, 6, 0) },
      4,
      (at) => house(at, 1.0),
      seed + 1,
    ),
    { op: "union" },
  ];
}
