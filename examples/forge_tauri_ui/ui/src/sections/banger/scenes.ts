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
export const OP_UNION        = 10;
export const OP_INTERSECT    = 11;
export const OP_DIFF         = 12;
export const OP_SMIN         = 13;

export const SDF_MAX_OPS = 64;
export const SDF_FLOATS_PER_OP = 8;

export type Vec3 = readonly [number, number, number];

export type SdfOp =
  | { op: "sphere";       center: Vec3; radius: number }
  | { op: "box";          center: Vec3; halfExtents: Vec3 }
  | { op: "torus";        center: Vec3; majorRadius: number; minorRadius: number }
  | { op: "capsule";      a: Vec3; b: Vec3; radius: number }
  | { op: "roundedBox";   center: Vec3; halfExtents: Vec3; cornerRadius: number }
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
    } else if (o.op === "smin") {
      buf[base + 4] = o.k;
    }
    // union/intersect/diff carry no params.
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
