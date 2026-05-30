// @ts-nocheck
// INGEN COMPUTE §18 Pillar C, Phase 6b — Gaussian Splat file parsers.
//
// Two formats are supported :
//   .ply   binary little-endian, the Inria 3DGS reference output. We
//          accept arbitrary property ordering, take only the L=0 SH band
//          (DC color), and apply the standard sigmoid(opacity) /
//          exp(scale) transforms documented in Kerbl et al. 2023.
//   .splat the 32-byte-per-splat community runtime format
//          (Antimatter15 / luma.gs). No SH, no logit space — read
//          straight, normalise quaternion, return.
//
// Both parsers emit the same 16-float anisotropic layout consumed by
// IngenRender.uploadSplatsAnisotropic :
//   [0..3]   pos.xyz, opacity
//   [4..7]   scale.xyz, _
//   [8..11]  qx, qy, qz, qw   (right-handed, normalised)
//   [12..15] color.rgb, _

export interface ParsedSplats {
  /** Float32Array of length count*16. Ready for uploadSplatsAnisotropic. */
  readonly buffer: Float32Array;
  /** Number of splats parsed. */
  readonly count: number;
  /** "ply" or "splat" — for debug / HUD. */
  readonly source: "ply" | "splat";
}

/** Pick the parser by sniffing the buffer head. Throws on unknown format. */
export function parseSplatFile(arrayBuffer: ArrayBuffer): ParsedSplats {
  const bytes = new Uint8Array(arrayBuffer);
  // PLY starts with the ASCII "ply\n" magic.
  if (bytes.length >= 4 && bytes[0] === 0x70 && bytes[1] === 0x6c && bytes[2] === 0x79) {
    return parsePly(arrayBuffer);
  }
  // .splat is 32 bytes per splat with no header — accept any buffer
  // whose length is a multiple of 32.
  if (bytes.length > 0 && bytes.length % 32 === 0) {
    return parseSplatRuntime(arrayBuffer);
  }
  throw new Error(`splat-loader: unrecognised format (${bytes.length} bytes)`);
}

// ---------------------------------------------------------------------------
// .splat (Antimatter15 runtime format)
//
// Each splat = 32 bytes :
//   0..12  : position  (3 × float32)
//   12..24 : scale     (3 × float32, world units, NOT log space)
//   24..28 : RGBA      (4 × uint8, sRGB or linear depending on exporter)
//   28..32 : quaternion (4 × uint8, mapped from [-128, 127] -> [-1, 1])
// ---------------------------------------------------------------------------
function parseSplatRuntime(arrayBuffer: ArrayBuffer): ParsedSplats {
  const count = (arrayBuffer.byteLength / 32) | 0;
  const view = new DataView(arrayBuffer);
  const out = new Float32Array(count * 16);
  for (let i = 0; i < count; i += 1) {
    const off = i * 32;
    const px = view.getFloat32(off + 0, true);
    const py = view.getFloat32(off + 4, true);
    const pz = view.getFloat32(off + 8, true);
    const sx = view.getFloat32(off + 12, true);
    const sy = view.getFloat32(off + 16, true);
    const sz = view.getFloat32(off + 20, true);
    const cr = view.getUint8(off + 24) / 255;
    const cg = view.getUint8(off + 25) / 255;
    const cb = view.getUint8(off + 26) / 255;
    const ca = view.getUint8(off + 27) / 255;
    const qx = (view.getUint8(off + 28) - 128) / 128;
    const qy = (view.getUint8(off + 29) - 128) / 128;
    const qz = (view.getUint8(off + 30) - 128) / 128;
    const qw = (view.getUint8(off + 31) - 128) / 128;
    const qn = Math.hypot(qx, qy, qz, qw) || 1;
    const dst = i * 16;
    out[dst + 0]  = px; out[dst + 1]  = py; out[dst + 2]  = pz; out[dst + 3]  = ca;
    out[dst + 4]  = sx; out[dst + 5]  = sy; out[dst + 6]  = sz; out[dst + 7]  = 0;
    out[dst + 8]  = qx / qn;
    out[dst + 9]  = qy / qn;
    out[dst + 10] = qz / qn;
    out[dst + 11] = qw / qn;
    out[dst + 12] = cr; out[dst + 13] = cg; out[dst + 14] = cb; out[dst + 15] = 0;
  }
  return { buffer: out, count, source: "splat" };
}

// ---------------------------------------------------------------------------
// .ply  binary little-endian, Inria 3DGS export.
//
// We parse the ASCII header, build a dictionary of property name → byte
// offset within one vertex record, then walk the binary block. We only
// take the L=0 SH band (`f_dc_0..2`) and skip every other property
// (normals, f_rest_*) — this covers ~99 % of real splat captures the
// user would drop on the banger today, while keeping the parser to one
// pass with no PLY library dependency.
// ---------------------------------------------------------------------------
function parsePly(arrayBuffer: ArrayBuffer): ParsedSplats {
  const bytes = new Uint8Array(arrayBuffer);
  // Find "end_header\n" — header is plain ASCII.
  const HEADER_END = "end_header\n";
  const headerLimit = Math.min(bytes.length, 1 << 16);
  let textEnd = -1;
  for (let i = 0; i <= headerLimit - HEADER_END.length; i += 1) {
    let match = true;
    for (let j = 0; j < HEADER_END.length; j += 1) {
      if (bytes[i + j] !== HEADER_END.charCodeAt(j)) { match = false; break; }
    }
    if (match) { textEnd = i + HEADER_END.length; break; }
  }
  if (textEnd < 0) throw new Error("splat-loader: PLY header not found");
  const header = new TextDecoder("ascii").decode(bytes.subarray(0, textEnd));
  if (!/format\s+binary_little_endian/.test(header)) {
    throw new Error("splat-loader: only binary_little_endian PLY supported");
  }
  const vertexLine = header.match(/element\s+vertex\s+(\d+)/);
  if (!vertexLine) throw new Error("splat-loader: PLY missing 'element vertex'");
  const count = parseInt(vertexLine[1], 10);

  // Build property table : name -> (offset, type-size).
  const TYPE_BYTES: Record<string, number> = {
    float: 4, float32: 4, double: 8, float64: 8,
    char: 1, uchar: 1, int8: 1, uint8: 1,
    short: 2, ushort: 2, int16: 2, uint16: 2,
    int: 4, uint: 4, int32: 4, uint32: 4,
  };
  const props: Array<{ name: string; type: string; size: number; offset: number }> = [];
  let stride = 0;
  let inVertex = false;
  for (const rawLine of header.split("\n")) {
    const line = rawLine.trim();
    if (line.startsWith("element ")) {
      inVertex = line.startsWith("element vertex");
      continue;
    }
    if (!inVertex || !line.startsWith("property ")) continue;
    const m = line.match(/property\s+(\S+)\s+(\S+)/);
    if (!m) continue;
    const type = m[1];
    const name = m[2];
    const size = TYPE_BYTES[type];
    if (!size) throw new Error(`splat-loader: unknown PLY type '${type}'`);
    props.push({ name, type, size, offset: stride });
    stride += size;
  }
  const propIndex = new Map<string, { offset: number; type: string }>();
  for (const p of props) propIndex.set(p.name, { offset: p.offset, type: p.type });

  // Required fields. We accept the network output names (Inria) and
  // the bare alternatives the community uses.
  const need = (...names: string[]) => {
    for (const n of names) {
      const p = propIndex.get(n);
      if (p) return p;
    }
    throw new Error(`splat-loader: PLY missing one of ${names.join(", ")}`);
  };
  const fx = need("x");
  const fy = need("y");
  const fz = need("z");
  const fop = need("opacity");
  const fs0 = need("scale_0");
  const fs1 = need("scale_1");
  const fs2 = need("scale_2");
  const fr0 = need("rot_0");
  const fr1 = need("rot_1");
  const fr2 = need("rot_2");
  const fr3 = need("rot_3");
  // L=0 SH (DC color). Some exports use red/green/blue uchars instead —
  // we try both. If neither is present, default to grey.
  let dcR: { offset: number; type: string } | null = null;
  let dcG: { offset: number; type: string } | null = null;
  let dcB: { offset: number; type: string } | null = null;
  const tryDC = (a: string, b: string, c: string) => {
    const ra = propIndex.get(a); const rb = propIndex.get(b); const rc = propIndex.get(c);
    if (ra && rb && rc) { dcR = ra; dcG = rb; dcB = rc; }
  };
  tryDC("f_dc_0", "f_dc_1", "f_dc_2");
  if (!dcR) tryDC("red", "green", "blue");

  const view = new DataView(arrayBuffer);
  const out = new Float32Array(count * 16);
  const readField = (recordBase: number, field: { offset: number; type: string }): number => {
    const at = recordBase + field.offset;
    switch (field.type) {
      case "float": case "float32": return view.getFloat32(at, true);
      case "double": case "float64": return view.getFloat64(at, true);
      case "uchar": case "uint8":    return view.getUint8(at);
      case "char": case "int8":      return view.getInt8(at);
      case "ushort": case "uint16":  return view.getUint16(at, true);
      case "short": case "int16":    return view.getInt16(at, true);
      case "uint": case "uint32":    return view.getUint32(at, true);
      case "int": case "int32":      return view.getInt32(at, true);
      default: return 0;
    }
  };
  // 3DGS SH-band-0 DC coefficient → RGB : color = 0.5 + SH_C0 * dc
  // where SH_C0 = 0.28209479177387814 (1 / (2 √π)).
  const SH_C0 = 0.28209479177387814;

  for (let i = 0; i < count; i += 1) {
    const rec = textEnd + i * stride;
    const px = readField(rec, fx);
    const py = readField(rec, fy);
    const pz = readField(rec, fz);
    const op = readField(rec, fop);
    const s0 = readField(rec, fs0);
    const s1 = readField(rec, fs1);
    const s2 = readField(rec, fs2);
    const r0 = readField(rec, fr0);
    const r1 = readField(rec, fr1);
    const r2 = readField(rec, fr2);
    const r3 = readField(rec, fr3);

    // Inria stores opacity in logit space and scale in log space.
    const opacity = 1.0 / (1.0 + Math.exp(-op));
    const sx = Math.exp(s0);
    const sy = Math.exp(s1);
    const sz = Math.exp(s2);
    // Quaternion (Inria order : w, x, y, z). Normalise.
    const qn = Math.hypot(r0, r1, r2, r3) || 1;
    const qw = r0 / qn;
    const qx = r1 / qn;
    const qy = r2 / qn;
    const qz = r3 / qn;

    let cr = 0.5, cg = 0.5, cb = 0.5;
    if (dcR && dcG && dcB) {
      if (dcR.type === "float" || dcR.type === "float32" || dcR.type === "double" || dcR.type === "float64") {
        // Stored as SH-DC float, convert to linear color.
        cr = Math.max(0, Math.min(1, 0.5 + SH_C0 * readField(rec, dcR)));
        cg = Math.max(0, Math.min(1, 0.5 + SH_C0 * readField(rec, dcG)));
        cb = Math.max(0, Math.min(1, 0.5 + SH_C0 * readField(rec, dcB)));
      } else {
        // Stored as plain 0..255 RGB.
        cr = readField(rec, dcR) / 255;
        cg = readField(rec, dcG) / 255;
        cb = readField(rec, dcB) / 255;
      }
    }

    const dst = i * 16;
    out[dst + 0]  = px; out[dst + 1]  = py; out[dst + 2]  = pz; out[dst + 3]  = opacity;
    out[dst + 4]  = sx; out[dst + 5]  = sy; out[dst + 6]  = sz; out[dst + 7]  = 0;
    out[dst + 8]  = qx; out[dst + 9]  = qy; out[dst + 10] = qz; out[dst + 11] = qw;
    out[dst + 12] = cr; out[dst + 13] = cg; out[dst + 14] = cb; out[dst + 15] = 0;
  }
  return { buffer: out, count, source: "ply" };
}
