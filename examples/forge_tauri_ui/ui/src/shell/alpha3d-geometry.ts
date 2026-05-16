export type Alpha3dPayload = {
  positions: Float32Array;
  colors: Float32Array;
  sizes?: Float32Array;
  metadata?: unknown[];
  drawMode?: string;
  pointSize?: number;
  [key: string]: unknown;
};

export const ALPHA_3D_Z_METRICS = {
  volume: { label: "Volume", short: "volume" },
  volatility: { label: "Volatility (range)", short: "(high - low) / close" },
  hour: { label: "Hour of day", short: "hour UTC" },
  rsi: { label: "RSI 14", short: "Wilder RSI(14)" },
  cvd: { label: "Cumulative dVol", short: "cum vol*sign(close - open)" },
} as const;

export const ALPHA_3D_LEGEND = {
  candles3d: {
    title: "OANDA pressure city",
    desc: "Each tower is one candle: X = time, Y = approximate pressure, Z = OANDA tick volume.",
    x: { label: "X", text: "time" },
    y: { label: "Y", text: "pressure score" },
    z: { label: "Z", text: "tick volume" },
    colors: [
      { swatch: "#4cc66c", label: "probable buy pressure" },
      { swatch: "#df6158", label: "probable sell pressure" },
      { swatch: "#c7d0e6", label: "abnormal activity veil" },
    ],
    meta: "signal = tower above the veil + confirmed pressure",
  },
  phase: {
    title: "Phase-space",
    desc: "Trajectory of close price across time, lagged by tau.",
    x: { label: "X", text: "close[t]" },
    y: { label: "Y", text: "close[t + tau]" },
    z: { label: "Z", text: "close[t + 2tau]" },
    colors: [
      { swatch: "#3a8acc", label: "early bars" },
      { swatch: "#d8826b", label: "recent bars" },
    ],
    meta: "draw: line strip * tau ~= N/600",
  },
  heightmap: {
    title: "Heightmap",
    desc: "How often each (time, price) cell was visited.",
    x: { label: "X", text: "time (oldest -> newest)" },
    y: { label: "Y", text: "visit density" },
    z: { label: "Z", text: "price level (low -> high)" },
    colors: [
      { swatch: "#4974b8", label: "rare visits" },
      { swatch: "#dba07a", label: "frequent / consolidation" },
    ],
    meta: "grid: 96 x 64 cells",
  },
  manifold: {
    title: "Feature manifold",
    desc: "Each bar plotted by its short / long return + 20-bar volatility.",
    x: { label: "X", text: "5-bar return" },
    y: { label: "Y", text: "20-bar realized volatility" },
    z: { label: "Z", text: "20-bar return" },
    colors: [
      { swatch: "#e85650", label: "negative future return (next 5 bars)" },
      { swatch: "#54c379", label: "positive future return" },
    ],
    meta: "color = sign of price[t+5] - price[t]",
  },
  lattice: {
    title: "Hash lattice",
    desc: "Bars sharing a quantized signature collapse to the same cell.",
    x: { label: "X", text: "hash bits 0-7" },
    y: { label: "Y", text: "hash bits 8-15" },
    z: { label: "Z", text: "hash bits 16-23" },
    colors: [
      { swatch: "#4ea864", label: "unique signature" },
      { swatch: "#e0a44a", label: "many collisions (recurring pattern)" },
    ],
    meta: "size proportional to sqrt(collisions); signature = quantize(price, return, range)",
  },
} as const;

export const ALPHA_3D_AXIS_META = {
  candles3d: { x: "time", y: "pressure", z: "tick volume" },
  phase: { x: "price[t]", y: "price[t+tau]", z: "price[t+2tau]" },
  heightmap: { x: "time", y: "density", z: "price" },
  manifold: { x: "ret 5", y: "vol 20", z: "ret 20" },
  lattice: { x: "hash a", y: "hash b", z: "hash c" },
} as const;

type Alpha3dMode = keyof typeof ALPHA_3D_LEGEND;
type Candle = Record<string, any>;
type Vec3 = [number, number, number];
type Vec4 = [number, number, number, number];

function modeKey(mode: unknown): Alpha3dMode {
  const key = String(mode || "candles3d") as Alpha3dMode;
  return Object.prototype.hasOwnProperty.call(ALPHA_3D_LEGEND, key) ? key : "candles3d";
}

export function alpha3dAxisLabels(mode: unknown) {
  const meta = ALPHA_3D_AXIS_META[modeKey(mode)];
  return { x: meta.x, y: meta.y, z: meta.z };
}

export function alpha3dLegendPayload(mode: unknown) {
  const cfg = ALPHA_3D_LEGEND[modeKey(mode)];
  return {
    title: cfg.title,
    description: cfg.desc,
    axes: {
      x: cfg.x.text,
      y: cfg.y.text,
      z: cfg.z.text,
    },
    colors: cfg.colors,
    meta: cfg.meta,
  };
}

export function attachAlpha3dMetadata(mode: unknown, candles: readonly Candle[], payload: Alpha3dPayload): Alpha3dPayload {
  const pointCount = Math.floor((payload.positions?.length || 0) / 3);
  if (Array.isArray(payload.metadata) && payload.metadata.length === pointCount) return payload;
  const metadata = new Array(pointCount);
  if (!pointCount) {
    payload.metadata = metadata;
    return payload;
  }
  const key = modeKey(mode);
  if (key === "candles3d") {
    for (let i = 0; i < pointCount; i += 1) {
      const barIndex = Math.floor(i / 2);
      const c = candles[barIndex] || {};
      metadata[i] = {
        bar_index: barIndex,
        role: i % 2 === 0 ? "low" : "high",
        time: c.time || null,
        value: i % 2 === 0 ? c.low : c.high,
      };
    }
  } else if (key === "phase") {
    const tau = Math.max(1, Math.floor(candles.length / 600));
    for (let i = 0; i < pointCount; i += 1) {
      metadata[i] = { bar_index: i, role: `lag tau=${tau}`, time: candles[i]?.time || null, value: candles[i]?.close ?? null };
    }
  } else if (key === "heightmap") {
    const W = 96;
    const H = 64;
    for (let i = 0; i < pointCount; i += 1) {
      const x = payload.positions[i * 3 + 0] || 0;
      const y = payload.positions[i * 3 + 1] || 0;
      const z = payload.positions[i * 3 + 2] || 0;
      const tx = Math.max(0, Math.min(W - 1, Math.round((x + 1) * 0.5 * (W - 1))));
      const py = Math.max(0, Math.min(H - 1, Math.round((z + 1) * 0.5 * (H - 1))));
      metadata[i] = { cell: `${tx},${py}`, role: "density", value: y };
    }
  } else if (key === "manifold") {
    for (let i = 0; i < pointCount; i += 1) {
      const barIndex = i + 20;
      metadata[i] = { bar_index: barIndex, role: "feature vector", time: candles[barIndex]?.time || null, value: candles[barIndex]?.close ?? null };
    }
  } else if (key === "lattice") {
    for (let i = 0; i < pointCount; i += 1) {
      const barIndex = i + 5;
      metadata[i] = { bar_index: barIndex, role: "quantized signature", time: candles[barIndex]?.time || null, value: candles[barIndex]?.close ?? null };
    }
  }
  payload.metadata = metadata;
  return payload;
}

export function alpha3dPerspective(fov: number, aspect: number, near: number, far: number): Float32Array {
  const f = 1 / Math.tan(fov / 2);
  const nf = 1 / (near - far);
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (far + near) * nf, -1,
    0, 0, 2 * far * near * nf, 0,
  ]);
}

export function alpha3dLookAt(eye: Vec3, center: Vec3, up: Vec3): Float32Array {
  const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const norm = (v: Vec3): Vec3 => {
    const l = Math.hypot(v[0], v[1], v[2]) || 1;
    return [v[0] / l, v[1] / l, v[2] / l];
  };
  const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const f = norm(sub(center, eye));
  const s = norm(cross(f, up));
  const u = cross(s, f);
  return new Float32Array([
    s[0], u[0], -f[0], 0,
    s[1], u[1], -f[1], 0,
    s[2], u[2], -f[2], 0,
    -dot(s, eye), -dot(u, eye), dot(f, eye), 1,
  ]);
}

export function alpha3dMulMat4Vec4(m: Float32Array, v: Vec4): Vec4 {
  const at = (idx: number) => m[idx] ?? 0;
  const x = v[0];
  const y = v[1];
  const z = v[2];
  const w = v[3];
  return [
    at(0) * x + at(4) * y + at(8) * z + at(12) * w,
    at(1) * x + at(5) * y + at(9) * z + at(13) * w,
    at(2) * x + at(6) * y + at(10) * z + at(14) * w,
    at(3) * x + at(7) * y + at(11) * z + at(15) * w,
  ];
}
