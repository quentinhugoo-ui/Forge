// @ts-nocheck
// drone-scene.ts — params + ops baked from Forge's hill-climb optimiser.
//
// Source : examples/forge_drone_design.rs   (cargo run --release).
// Re-run that binary to refresh the constants below ; the SDF ops are
// 1:1 with the JSON output `examples/forge_drone_design.out.json`. Do NOT
// hand-tune these numbers — the design is the equilibrium of a multi-
// constraint scalar penalty over geometric clearance, passive stability,
// payload mass and hover thrust, so any local tweak breaks the global
// score. If you want a different drone, change the constraints and rerun.
//
// Doctrine : Forge computed the design ; the LLM did not. The TS side is
// pure data plumbing — it converts the precomputed ops into the SdfOp[]
// shape consumed by INGEN Banger via __forgeBangerSetScene.

import type { SdfOp } from "./scenes.js";

export interface SphericalDroneParams {
  readonly cage_outer_r:  number;
  readonly cage_inner_r:  number;
  readonly prop_radius:   number;
  readonly prop_ring_r:   number;
  readonly prop_ring_z:   number;
  readonly prop_count:    number;
  readonly weight_radius: number;
  readonly rpi_edge:      number;
  readonly cam_radius:    number;
  readonly wifi_length:   number;
}

/** Optimum returned by forge_drone_design after 8 restarts × 4096 steps.
 *  Score = 0 on every constraint, mass = 1.14 kg, hover thrust = 1.87 kg
 *  (1.6× safety margin), all components physically fit. */
export const SPHERICAL_DRONE_PARAMS: SphericalDroneParams = Object.freeze({
  cage_outer_r:  0.104878,
  cage_inner_r:  0.102878,
  prop_radius:   0.028786,
  prop_ring_r:   0.052812,
  prop_ring_z:  -0.008119,
  prop_count:    4,
  weight_radius: 0.028228,
  rpi_edge:      0.036465,
  cam_radius:    0.010065,
  wifi_length:   0.030901,
});

/** SDF ops for INGEN Banger — exactly what `forge_drone_design` emits.
 *  Cage = outer sphere ∖ inner sphere ; props are 4 capsules on a ring,
 *  weight ball sits at the bottom of the cavity, RPi rounded box at the
 *  centre, camera lens on +X equator, WiFi antenna sticks up +Z. Every
 *  union is a SMIN so the parts fuse with organic biological-style
 *  fillets — matches the Banger material doctrine. */
export const SPHERICAL_DRONE_SCENE_OPS: ReadonlyArray<SdfOp> = Object.freeze([
  // 1. Hollow cage.
  { op: "sphere",  center: [0, 0, 0], radius: 0.104878 },
  { op: "sphere",  center: [0, 0, 0], radius: 0.102878 },
  { op: "diff" },
  // 2. 4 propellers.
  { op: "capsule", a: [ 0.052812,  0.028786, -0.008119], b: [ 0.052812, -0.028786, -0.008119], radius: 0.0035 },
  { op: "smin",    k: 0.004 },
  { op: "capsule", a: [-0.028786,  0.052812, -0.008119], b: [ 0.028786,  0.052812, -0.008119], radius: 0.0035 },
  { op: "smin",    k: 0.004 },
  { op: "capsule", a: [-0.052812, -0.028786, -0.008119], b: [-0.052812,  0.028786, -0.008119], radius: 0.0035 },
  { op: "smin",    k: 0.004 },
  { op: "capsule", a: [ 0.028786, -0.052812, -0.008119], b: [-0.028786, -0.052812, -0.008119], radius: 0.0035 },
  { op: "smin",    k: 0.004 },
  // 3. Stabilising weight at the bottom of the cavity.
  { op: "sphere",  center: [0, 0, -0.06965], radius: 0.028228 },
  { op: "smin",    k: 0.005 },
  // 4. Raspberry Pi rounded box, centred.
  { op: "roundedBox", center: [0, 0, 0], halfExtents: [0.018233, 0.018233, 0.007293], cornerRadius: 0.003 },
  { op: "smin",    k: 0.004 },
  // 5. Camera lens on +X equator, flush with the inner wall.
  { op: "sphere",  center: [0.089813, 0, 0], radius: 0.010065 },
  { op: "smin",    k: 0.003 },
  // 6. WiFi antenna sticking out the top.
  { op: "capsule", a: [0, 0, 0.101878], b: [0, 0, 0.132779], radius: 0.0025 },
  { op: "smin",    k: 0.004 },
]) as ReadonlyArray<SdfOp>;

/** Returns a writable shallow copy ready for __forgeBangerSetScene. */
export function buildSphericalDroneSceneOps(): SdfOp[] {
  return SPHERICAL_DRONE_SCENE_OPS.map((op) => ({ ...op })) as SdfOp[];
}
