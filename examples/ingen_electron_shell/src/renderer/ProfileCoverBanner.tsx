import { useEffect, useRef } from "react";
import type * as ThreeNamespace from "three";
import type { WelcomeMessage } from "./welcome-message-store";

type ThreeModule = typeof ThreeNamespace;
type BannerHandle = { destroy(): void };
const CHAT_KEY_COLOR_EVENT = "ingen:chat-key-color";
// Composer send animation drains the wave: level 0 = full field, 1 = empty.
// Each point owns a static random threshold, so the field empties point by
// point in random order as the level rises (see ComposerSendBurst).
const WAVE_DRAIN_EVENT = "ingen:wave-drain";
const KEY_COLOR_EVENT_COUNT = 28;
const POINT_COLS = 160;
const POINT_ROWS = 120;
const POINT_SPACING = 0.42;
const INTRO_REVEAL_SECONDS = 3.8;
const MAX_GHOSTS = 6;
const GHOST_LIFETIME = 12;
const WAVE_ACTIVE_FRAME_INTERVAL_MS = 1000 / 60;
const WAVE_REDUCED_FRAME_INTERVAL_MS = 1000 / 12;
const WAVE_POINTER_BLOCKER_SELECTOR = [
  ".leftPanel",
  ".panelsChatBottom",
  ".composer",
  ".composerQuestionnaire",
  ".rightPanel",
  ".canvasFilesPane",
  ".canvasTerminalPane",
  ".permissionModeMenu"
].join(",");

function isWavePointerBlocked(event: PointerEvent): boolean {
  return Boolean(document.elementFromPoint(event.clientX, event.clientY)?.closest(WAVE_POINTER_BLOCKER_SELECTOR));
}

const WEBGPU_BUFFER_USAGE = {
  COPY_DST: 0x8,
  VERTEX: 0x20,
  UNIFORM: 0x40
} as const;
const WEBGPU_TEXTURE_USAGE = {
  RENDER_ATTACHMENT: 0x10
} as const;
const WEBGPU_MSAA_SAMPLES = 4;

type WebGpuBuffer = { destroy(): void };
type WebGpuTexture = { createView(): unknown; destroy(): void };
type WebGpuRenderPipeline = { getBindGroupLayout(index: number): unknown };
type WebGpuBindGroup = unknown;
type WebGpuAdapterInfo = {
  architecture?: string;
  description?: string;
  device?: string;
  isFallbackAdapter?: boolean;
  vendor?: string;
};
type WebGpuBufferData = ArrayBuffer | ArrayBufferView<ArrayBufferLike>;
type WebGpuQueue = {
  writeBuffer(buffer: WebGpuBuffer, bufferOffset: number, data: WebGpuBufferData, dataOffset?: number, size?: number): void;
  submit(commandBuffers: unknown[]): void;
};
type WebGpuRenderPassEncoder = {
  setPipeline(pipeline: WebGpuRenderPipeline): void;
  setBindGroup(index: number, bindGroup: WebGpuBindGroup): void;
  setVertexBuffer(slot: number, buffer: WebGpuBuffer): void;
  draw(vertexCount: number, instanceCount: number): void;
  end(): void;
};
type WebGpuCommandEncoder = {
  beginRenderPass(descriptor: Record<string, unknown>): WebGpuRenderPassEncoder;
  finish(): unknown;
};
type WebGpuDevice = {
  adapterInfo?: WebGpuAdapterInfo;
  queue: WebGpuQueue;
  lost: Promise<unknown>;
  createBuffer(descriptor: { label?: string; size: number; usage: number }): WebGpuBuffer;
  createShaderModule(descriptor: { label?: string; code: string }): unknown;
  createTexture(descriptor: {
    label?: string;
    size: { width: number; height: number };
    sampleCount?: number;
    format: string;
    usage: number;
  }): WebGpuTexture;
  createRenderPipeline(descriptor: Record<string, unknown>): WebGpuRenderPipeline;
  createBindGroup(descriptor: { label?: string; layout: unknown; entries: Array<{ binding: number; resource: unknown }> }): WebGpuBindGroup;
  createCommandEncoder(descriptor?: { label?: string }): WebGpuCommandEncoder;
  destroy?: () => void;
};
type WebGpuAdapter = { info?: WebGpuAdapterInfo; requestDevice(descriptor?: Record<string, unknown>): Promise<WebGpuDevice> };
type WebGpuRuntime = {
  requestAdapter(options?: { powerPreference?: "high-performance" | "low-power" }): Promise<WebGpuAdapter | null>;
  getPreferredCanvasFormat(): string;
};
type WebGpuCanvasContext = {
  configure(config: { device: WebGpuDevice; format: string; alphaMode: "opaque" | "premultiplied" }): void;
  getCurrentTexture(): { createView(): unknown };
  unconfigure?: () => void;
};
type WebGpuHostNavigator = { gpu?: WebGpuRuntime };

interface KeyboardColorEventDetail {
  code?: string;
  key?: string;
  location?: number;
}

interface WavePointField {
  basePos: Float32Array;
  pCoord: Float32Array;
  pBirth: Float32Array;
  pointData: Float32Array;
  count: number;
}

function createWavePointField(): WavePointField {
  const count = POINT_COLS * POINT_ROWS;
  const basePos = new Float32Array(count * 3);
  const pCoord = new Float32Array(count * 2);
  const pBirth = new Float32Array(count);
  const pointData = new Float32Array(count * 6);
  const seedCol = Math.floor(POINT_COLS / 2);
  const seedRow = Math.floor(POINT_ROWS / 2);
  const maxRadius = Math.hypot(0.5, 0.5);

  for (let r = 0; r < POINT_ROWS; r++) {
    for (let c = 0; c < POINT_COLS; c++) {
      const i = r * POINT_COLS + c;
      const nx = c / (POINT_COLS - 1);
      const nz = r / (POINT_ROWS - 1);
      const dx = nx - 0.5;
      const dz = nz - 0.5;
      const radius = Math.hypot(dx, dz) / maxRadius;
      const angle = Math.atan2(dz, dx);
      const branch = Math.sin(angle * 5 + radius * 13.5) * 0.055 + Math.sin(angle * 11 - radius * 8) * 0.035;
      const x = (c - POINT_COLS / 2) * POINT_SPACING;
      const z = (r - POINT_ROWS / 2) * POINT_SPACING;
      const birth = c === seedCol && r === seedRow
        ? 0
        : 0.24 + Math.max(0, Math.pow(radius, 0.68) * 3.05 + branch);

      basePos[i * 3] = x;
      basePos[i * 3 + 1] = 0;
      basePos[i * 3 + 2] = z;
      pCoord[i * 2] = nx;
      pCoord[i * 2 + 1] = nz;
      pBirth[i] = birth;
      pointData[i * 6] = x;
      pointData[i * 6 + 1] = 0;
      pointData[i * 6 + 2] = z;
      pointData[i * 6 + 3] = nx;
      pointData[i * 6 + 4] = nz;
      pointData[i * 6 + 5] = birth;
    }
  }

  return { basePos, pCoord, pBirth, pointData, count };
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function createKeyColorEvents(): Float32Array {
  const keyColorEvents = new Float32Array(KEY_COLOR_EVENT_COUNT * 4);
  for (let i = 0; i < KEY_COLOR_EVENT_COUNT; i++) {
    keyColorEvents[i * 4 + 2] = -100;
    keyColorEvents[i * 4 + 3] = -1;
  }
  return keyColorEvents;
}

function spawnGhostData(ghostData: Float32Array, slot: number, t: number): void {
  ghostData[slot * 4] = Math.random();
  ghostData[slot * 4 + 1] = Math.random();
  ghostData[slot * 4 + 2] = t;
  ghostData[slot * 4 + 3] = 0.5 + Math.random() * 0.9;
}

function compactWebGpuAdapterInfo(info?: WebGpuAdapterInfo): Record<string, unknown> {
  return {
    vendor: info?.vendor || "unreported",
    architecture: info?.architecture || "unreported",
    device: info?.device || "unreported",
    description: info?.description || "unreported",
    isFallbackAdapter: info?.isFallbackAdapter === true
  };
}

const KEYBOARD_GRID: Record<string, [number, number]> = (() => {
  const rows: Array<{ y: number; x0: number; dx: number; codes: string[] }> = [
    { y: 0.16, x0: 0.06, dx: 0.047, codes: ["Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal"] },
    { y: 0.31, x0: 0.095, dx: 0.047, codes: ["KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash"] },
    { y: 0.46, x0: 0.12, dx: 0.047, codes: ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote"] },
    { y: 0.61, x0: 0.15, dx: 0.047, codes: ["IntlBackslash", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash"] },
    { y: 0.78, x0: 0.25, dx: 0.047, codes: ["Space"] }
  ];
  const map: Record<string, [number, number]> = {};
  for (const row of rows) {
    row.codes.forEach((code, index) => {
      map[code] = [row.x0 + row.dx * index, row.y];
    });
  }
  Object.assign(map, {
    NumLock: [0.76, 0.16],
    NumpadDivide: [0.82, 0.16],
    NumpadMultiply: [0.88, 0.16],
    NumpadSubtract: [0.94, 0.16],
    Numpad7: [0.76, 0.31],
    Numpad8: [0.82, 0.31],
    Numpad9: [0.88, 0.31],
    NumpadAdd: [0.94, 0.38],
    Numpad4: [0.76, 0.46],
    Numpad5: [0.82, 0.46],
    Numpad6: [0.88, 0.46],
    Numpad1: [0.76, 0.61],
    Numpad2: [0.82, 0.61],
    Numpad3: [0.88, 0.61],
    NumpadEnter: [0.94, 0.68],
    Numpad0: [0.79, 0.78],
    NumpadDecimal: [0.88, 0.78],
    NumpadComma: [0.88, 0.78],
    NumpadEqual: [0.94, 0.78]
  });
  return map;
})();

const NUMPAD_KEY_GRID: Record<string, string> = {
  ".": "NumpadDecimal",
  ",": "NumpadComma",
  "-": "NumpadSubtract",
  "+": "NumpadAdd",
  "/": "NumpadDivide",
  "*": "NumpadMultiply",
  "=": "NumpadEqual"
};

const RIGHT_TEXT_SYMBOL_GRID: Record<string, [number, number]> = {
  "!": [0.66, 0.16],
  "$": [0.68, 0.16],
  ")": [0.70, 0.16],
  "=": [0.72, 0.16],
  "+": [0.74, 0.16],
  "-": [0.74, 0.16],
  "^": [0.68, 0.31],
  "¨": [0.70, 0.31],
  "]": [0.72, 0.31],
  "}": [0.72, 0.31],
  "\\": [0.74, 0.31],
  "|": [0.74, 0.31],
  "ù": [0.68, 0.46],
  "%": [0.70, 0.46],
  "µ": [0.72, 0.46],
  "*": [0.74, 0.46],
  ";": [0.66, 0.61],
  ":": [0.68, 0.61],
  ".": [0.70, 0.61],
  "/": [0.72, 0.61],
  "?": [0.74, 0.61]
};

function keyboardColorPosition(code?: string, key?: string, location?: number): [number, number] | null {
  if (key && key.length === 1) {
    const normalized = key.toLowerCase();
    const numpadFallback = NUMPAD_KEY_GRID[normalized];
    if (location === 3 && normalized >= "0" && normalized <= "9") {
      return KEYBOARD_GRID[`Numpad${normalized}`];
    }
    if (location === 3 && numpadFallback && KEYBOARD_GRID[numpadFallback]) {
      return KEYBOARD_GRID[numpadFallback];
    }
    const rightTextSymbol = RIGHT_TEXT_SYMBOL_GRID[normalized] ?? RIGHT_TEXT_SYMBOL_GRID[key];
    if (rightTextSymbol) {
      return rightTextSymbol;
    }
  }
  if (code && KEYBOARD_GRID[code]) {
    return KEYBOARD_GRID[code];
  }
  if (key && key.length === 1) {
    const normalized = key.toLowerCase();
    const numpadFallback = NUMPAD_KEY_GRID[normalized];
    const codeFromChar = normalized >= "a" && normalized <= "z"
      ? `Key${normalized.toUpperCase()}`
      : normalized >= "0" && normalized <= "9"
        ? `Digit${normalized}`
        : numpadFallback ?? "";
    if (codeFromChar && KEYBOARD_GRID[codeFromChar]) {
      return KEYBOARD_GRID[codeFromChar];
    }
  }
  return null;
}

async function initPoolClawWebGpuBanner(canvas: HTMLCanvasElement, THREE: ThreeModule): Promise<BannerHandle | null> {
  const gpu = (navigator as unknown as WebGpuHostNavigator).gpu;
  if (!gpu) {
    console.info("[InGen wave renderer] WebGPU unavailable: navigator.gpu missing; using WebGL fallback.");
    return null;
  }

  const adapter = (await gpu.requestAdapter({ powerPreference: "high-performance" }).catch(() => null)) ??
    (await gpu.requestAdapter().catch(() => null));
  if (!adapter) {
    console.info("[InGen wave renderer] WebGPU unavailable: no adapter; using WebGL fallback.");
    return null;
  }

  let device: WebGpuDevice;
  try {
    device = await adapter.requestDevice() as unknown as WebGpuDevice;
  } catch (error) {
    console.warn("InGen wave WebGPU device request failed; falling back to WebGL.", error);
    return null;
  }
  console.info(
    "[InGen wave renderer] WebGPU device acquired.",
    JSON.stringify({
      adapter: compactWebGpuAdapterInfo(adapter.info),
      device: compactWebGpuAdapterInfo(device.adapterInfo)
    })
  );

  const cfg = {
    bgColor: { r: 0.050980392, g: 0.050980392, b: 0.058823529, a: 1 },
    pointSize: 2.5,
    depress: 4.2,
    rippleAmp: 0.5
  };
  const prefersReducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const format = gpu.getPreferredCanvasFormat();
  const { pointData, count } = createWavePointField();
  const ghostData = new Float32Array(MAX_GHOSTS * 4);
  const keyColorEvents = createKeyColorEvents();
  const uniformData = new Float32Array(32);
  let keyColorCursor = 0;
  let keyColorSequence = 0;

  for (let s = 0; s < MAX_GHOSTS; s++) {
    spawnGhostData(ghostData, s, -(s / MAX_GHOSTS) * GHOST_LIFETIME);
  }

  const pointBuffer = device.createBuffer({
    label: "profile-wave-webgpu-points",
    size: pointData.byteLength,
    usage: WEBGPU_BUFFER_USAGE.VERTEX | WEBGPU_BUFFER_USAGE.COPY_DST
  });
  const uniformBuffer = device.createBuffer({
    label: "profile-wave-webgpu-uniforms",
    size: uniformData.byteLength,
    usage: WEBGPU_BUFFER_USAGE.UNIFORM | WEBGPU_BUFFER_USAGE.COPY_DST
  });
  const ghostBuffer = device.createBuffer({
    label: "profile-wave-webgpu-ghosts",
    size: ghostData.byteLength,
    usage: WEBGPU_BUFFER_USAGE.UNIFORM | WEBGPU_BUFFER_USAGE.COPY_DST
  });
  const keyColorBuffer = device.createBuffer({
    label: "profile-wave-webgpu-key-colors",
    size: keyColorEvents.byteLength,
    usage: WEBGPU_BUFFER_USAGE.UNIFORM | WEBGPU_BUFFER_USAGE.COPY_DST
  });
  const destroyGpuResources = () => {
    pointBuffer.destroy();
    uniformBuffer.destroy();
    ghostBuffer.destroy();
    keyColorBuffer.destroy();
    device.destroy?.();
  };
  device.queue.writeBuffer(pointBuffer, 0, pointData);

  const shader = device.createShaderModule({
    label: "profile-wave-webgpu-shader",
    code: `
struct Uniforms {
  viewProj: mat4x4f,
  wave: vec4f,
  point: vec4f,
  pointer: vec4f,
  crop: vec4f,
}

struct Ghosts {
  data: array<vec4f, ${MAX_GHOSTS}>,
}

struct KeyColors {
  data: array<vec4f, ${KEY_COLOR_EVENT_COUNT}>,
}

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) y: f32,
  @location(1) ripple: f32,
  @location(2) depth: f32,
  @location(3) reveal: f32,
  @location(4) seed: f32,
  @location(5) introChroma: f32,
  @location(6) coord: vec2f,
  @location(7) local: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<uniform> ghosts: Ghosts;
@group(0) @binding(2) var<uniform> keyColors: KeyColors;

fn quadCorner(vertexIndex: u32) -> vec2f {
  switch vertexIndex {
    case 0u: { return vec2f(-1.0, -1.0); }
    case 1u: { return vec2f(1.0, -1.0); }
    case 2u: { return vec2f(-1.0, 1.0); }
    case 3u: { return vec2f(-1.0, 1.0); }
    case 4u: { return vec2f(1.0, -1.0); }
    default: { return vec2f(1.0, 1.0); }
  }
}

fn getAmbient(px: f32, pz: f32, t: f32) -> f32 {
  var z = 0.0;
  z += 0.085 * sin(px * 6.28318 * 1.1 + t * 0.045);
  z += 0.072 * sin(pz * 6.28318 * 0.8 + t * 0.035 + 1.1);
  z += 0.058 * sin((px + pz) * 6.28318 * 1.5 + t * 0.055 + 2.3);
  z += 0.044 * sin((px - pz) * 6.28318 * 2.1 + t * 0.065 + 3.7);
  return z;
}

fn ghostRipple(p: vec2f, g: vec4f, t: f32) -> f32 {
  let age = t - g.z;
  if (age < 0.0) {
    return 0.0;
  }
  let lifetime = 12.0;
  if (age > lifetime) {
    return 0.0;
  }
  let fadeIn = smoothstep(0.0, 2.5, age);
  let fadeOut = smoothstep(lifetime, lifetime - 3.0, age);
  let envelope = fadeIn * fadeOut;
  let dist = length(p - g.xy);
  let front = age * 0.032;
  let width = 0.18;
  let ring = exp(-((dist - front) * (dist - front)) / (width * width));
  let osc = sin(dist * 8.0 - age * 1.0);
  return g.w * 1.28 * ring * envelope * osc;
}

@vertex
fn vertexMain(
  @builtin(vertex_index) vertexIndex: u32,
  @location(0) position: vec3f,
  @location(1) pCoord: vec2f,
  @location(2) pBirth: f32
) -> VertexOut {
  let t = uniforms.wave.x;
  let intro = uniforms.wave.y;
  let depressAmp = uniforms.wave.z;
  let rippleAmp = uniforms.wave.w;
  let pointSize = uniforms.point.x;
  let pointScaleX = uniforms.point.y;
  let viewportWidth = max(uniforms.point.z, 1.0);
  let viewportHeight = max(uniforms.point.w, 1.0);
  let sceneHeight = max(uniforms.crop.x, viewportHeight);
  let p = pCoord;
  let drain = uniforms.crop.y;
  let drainHash = fract(sin(dot(pCoord, vec2f(127.1, 311.7))) * 43758.5453);
  let keep = smoothstep(drain * 1.04 - 0.04, drain * 1.04, drainHash);
  let reveal = smoothstep(pBirth, pBirth + 0.42, intro);
  let seed = 1.0 - smoothstep(0.0, 0.001, pBirth);
  let seedPulse = seed * smoothstep(0.02, 0.16, intro) * (1.0 - smoothstep(0.45, 1.1, intro));
  let fresh = 1.0 - smoothstep(pBirth + 0.18, pBirth + 1.12, intro);
  let mdiff = p - uniforms.pointer.xy;
  let mdist = length(mdiff);
  let depress = -depressAmp * exp(-mdist * mdist * 34.0);
  let mripple = rippleAmp * exp(-mdist * 6.1) * sin(mdist * 8.0 - t * 0.6);
  let mspeed = length(uniforms.pointer.zw) * 40.0;
  let turb = mspeed * 0.28 * exp(-mdist * mdist * 44.0) * sin(mdist * 5.0 - t * 0.9);
  var ghostSum = 0.0;
  for (var i = 0; i < ${MAX_GHOSTS}; i++) {
    ghostSum += ghostRipple(p, ghosts.data[i], t);
  }
  let yTotal = (depress + mripple + turb + ghostSum + getAmbient(pCoord.x, pCoord.y, t)) * reveal;
  let corner = quadCorner(vertexIndex);
  let worldPosition = vec3f(position.x * pointScaleX, yTotal, position.z);
  var clip = uniforms.viewProj * vec4f(worldPosition, 1.0);
  clip.z = clip.z * 0.5 + clip.w * 0.5;
  clip.y = clip.y * (sceneHeight / viewportHeight);
  let pointDiameter = max(1.0, pointSize * 0.82 * (reveal + seedPulse * 1.8)) * keep;
  clip.xy += corner * pointDiameter * 0.5 * vec2f(2.0 / viewportWidth, 2.0 / viewportHeight) * clip.w;

  var out: VertexOut;
  out.position = clip;
  out.y = yTotal;
  out.ripple = (mripple + ghostSum) * reveal;
  out.depth = pCoord.y;
  out.reveal = reveal;
  out.seed = seedPulse;
  out.introChroma = reveal * (0.32 + fresh * 0.72) + seedPulse;
  out.coord = pCoord;
  out.local = corner;
  return out;
}

fn hsvKeyboard(h: f32, s: f32, v: f32) -> vec3f {
  let p = abs(fract(vec3f(h) + vec3f(0.0, 0.6666667, 0.3333333)) * 6.0 - vec3f(3.0));
  var rgb = clamp(p - vec3f(1.0), vec3f(0.0), vec3f(1.0));
  rgb = rgb * rgb * (vec3f(3.0) - 2.0 * rgb);
  return v * mix(vec3f(1.0), rgb, vec3f(s));
}

fn keyboardPalette(hue: f32) -> vec3f {
  let h = fract(hue);
  let warm = smoothstep(0.95, 1.0, h) + 1.0 - smoothstep(0.0, 0.08, h);
  let cool = smoothstep(0.38, 0.72, h) * (1.0 - smoothstep(0.72, 0.88, h));
  let value = mix(0.92, 1.0, cool) - warm * 0.05;
  let c = hsvKeyboard(h, 0.97, value);
  return clamp(pow(c, vec3f(0.78)), vec3f(0.0), vec3f(1.0));
}

fn saturateKeyboardColor(c: vec3f) -> vec3f {
  let l = dot(c, vec3f(0.299, 0.587, 0.114));
  return clamp(mix(vec3f(l), c, vec3f(2.55)), vec3f(0.0), vec3f(1.0));
}

@fragment
fn fragmentMain(in: VertexOut) -> @location(0) vec4f {
  let fog = 1.0 - clamp(in.depth * 0.72, 0.0, 0.68);
  let chroma = clamp(in.introChroma, 0.0, 1.15);
  let bright = clamp(0.50 + in.y * 0.10 + in.seed * 0.42 + chroma * 0.08, 0.05, 0.98) * fog;
  let redness = clamp(-in.y * 0.28 + abs(in.ripple) * 2.2 + in.seed * 0.35 + chroma * 0.58, 0.0, 1.0);
  let rs = clamp(in.ripple * 3.0 + chroma * 0.34, -1.0, 1.0);
  let amber = clamp(chroma * 0.34 + max(in.ripple, 0.0) * 0.42, 0.0, 0.55);
  let cyan = clamp((1.0 - smoothstep(0.0, 0.62, redness)) * abs(in.ripple) * 0.22 + smoothstep(0.18, 0.72, chroma) * 0.045, 0.0, 0.16);
  let baseColor = vec3f(
    bright + redness * 0.70 + rs * 0.22 + amber * 0.18,
    bright - redness * 0.18 - rs * 0.05 + chroma * 0.025 + amber * 0.14 + cyan * 0.22,
    bright - redness * 0.20 - rs * 0.14 + cyan * 0.30
  );
  var keyAccum = vec3f(0.0);
  var keyBias = vec3f(0.0);
  var keyWeight = 0.0;
  var keyBiasWeight = 0.0;
  for (var i = 0; i < ${KEY_COLOR_EVENT_COUNT}; i++) {
    let k = keyColors.data[i];
    let age = uniforms.wave.x - k.z;
    if (age >= 0.0 && age < 8.8 && k.w >= 0.0) {
      let birth = smoothstep(0.0, 0.24, age);
      let life = 1.0 - smoothstep(5.6, 8.6, age);
      let spread = smoothstep(0.0, 3.1, age);
      let radius = mix(0.026, 0.24, spread);
      let dist = length((in.coord - k.xy) * vec2f(1.0, 1.22));
      let diffuse = exp(-(dist * dist) / (radius * radius * 1.18));
      let perimeter = 1.0 - smoothstep(radius * 0.78, radius * 1.46, dist);
      let halo = exp(-(dist * dist) / 0.055) * smoothstep(1.1, 3.5, age) * 0.014;
      let w = (diffuse * 0.78 + perimeter * 0.22 + halo) * birth * life * 0.70;
      let bw = pow(max(w, 0.0), 1.26);
      let hueShift = smoothstep(radius * 0.22, radius * 1.25, dist) * 0.11 + smoothstep(radius * 0.70, radius * 1.35, dist) * 0.08;
      let c = mix(keyboardPalette(k.w), keyboardPalette(fract(k.w + hueShift)), vec3f(0.48));
      keyAccum += c * w;
      keyWeight += w;
      keyBias += c * bw;
      keyBiasWeight += bw;
    }
  }
  let keyMix = clamp((1.0 - exp(-keyWeight * 0.92)) * 0.88, 0.0, 0.88) * in.reveal;
  var avgColor = baseColor;
  if (keyWeight > 0.001) {
    avgColor = keyAccum / max(keyWeight, 0.001);
  }
  var biasColor = avgColor;
  if (keyBiasWeight > 0.001) {
    biasColor = keyBias / max(keyBiasWeight, 0.001);
  }
  var mappedColor = baseColor;
  if (keyWeight > 0.001) {
    mappedColor = saturateKeyboardColor(mix(avgColor, biasColor, vec3f(0.38)));
  }
  let litKey = mappedColor * (0.58 + bright * 0.38 + redness * 0.04);
  let mutedBase = baseColor * (1.0 - keyMix * 0.32);
  var finalColor = mix(mutedBase, litKey, vec3f(keyMix));
  let alpha = clamp(in.reveal + in.seed, 0.0, 1.0);
  return vec4f(clamp(finalColor, vec3f(0.0), vec3f(1.0)), alpha);
}
`
  });

  const pipeline = device.createRenderPipeline({
    label: "profile-wave-webgpu-pipeline",
    layout: "auto",
    vertex: {
      module: shader,
      entryPoint: "vertexMain",
      buffers: [
        {
          arrayStride: 24,
          stepMode: "instance",
          attributes: [
            { shaderLocation: 0, offset: 0, format: "float32x3" },
            { shaderLocation: 1, offset: 12, format: "float32x2" },
            { shaderLocation: 2, offset: 20, format: "float32" }
          ]
        }
      ]
    },
    fragment: {
      module: shader,
      entryPoint: "fragmentMain",
      targets: [
        {
          format,
          blend: {
            color: { operation: "add", srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" },
            alpha: { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" }
          }
        }
      ]
    },
    primitive: { topology: "triangle-list" },
    multisample: { count: WEBGPU_MSAA_SAMPLES }
  });

  const bindGroup = device.createBindGroup({
    label: "profile-wave-webgpu-bind-group",
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: uniformBuffer } },
      { binding: 1, resource: { buffer: ghostBuffer } },
      { binding: 2, resource: { buffer: keyColorBuffer } }
    ]
  });

  const context = canvas.getContext("webgpu") as WebGpuCanvasContext | null;
  if (!context) {
    console.info("[InGen wave renderer] WebGPU canvas context unavailable after device acquisition; using WebGL fallback.");
    destroyGpuResources();
    return null;
  }

  const camera = new THREE.PerspectiveCamera(58, 1, 0.1, 500);
  camera.position.set(0, 5.5, 22);
  camera.lookAt(new THREE.Vector3(0, 0, 0));
  camera.updateMatrixWorld();
  const viewProjMatrix = new THREE.Matrix4();
  const raycaster = new THREE.Raycaster();
  const plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  const hitPoint = new THREE.Vector3();
  const pointerNdc = new THREE.Vector2();
  const mouseTarget = new THREE.Vector2(-5, -5);
  const mousePrev = new THREE.Vector2(-5, -5);
  const mouseUniform = new THREE.Vector2(-5, -5);
  const mouseVel = new THREE.Vector2(0, 0);
  const halfCW = (POINT_COLS / 2) * POINT_SPACING;
  const halfCD = (POINT_ROWS / 2) * POINT_SPACING;
  const gridWorldWidth = POINT_COLS * POINT_SPACING;
  const horizontalOverscan = 1.62;
  const cursorBottomEdgeOffset = 0.18;
  let sceneHeight = 600;
  let cropY = 240;
  let pointScaleX = 1;
  let viewportWidth = 1;
  let viewportHeight = 1;
  let msaaTexture: WebGpuTexture | null = null;
  let hasMouse = false;
  let configured = false;
  let deviceLost = false;
  let drainLevel = 0;
  let shutdownRequested = false;
  let stopped = false;
  let listenersAttached = false;
  let resourcesReleased = false;

  const configure = () => {
    context.configure({ device, format, alphaMode: "opaque" });
    msaaTexture?.destroy();
    msaaTexture = device.createTexture({
      label: "profile-wave-webgpu-msaa-color",
      size: { width: viewportWidth, height: viewportHeight },
      sampleCount: WEBGPU_MSAA_SAMPLES,
      format,
      usage: WEBGPU_TEXTURE_USAGE.RENDER_ATTACHMENT
    });
    configured = true;
  };

  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(rect.width));
    const height = Math.max(1, Math.round(rect.height));
    viewportWidth = width;
    viewportHeight = height;
    canvas.width = width;
    canvas.height = height;
    sceneHeight = Math.max(600, height);
    cropY = Math.max(0, Math.round((sceneHeight - height) / 2));
    camera.aspect = width / sceneHeight;
    camera.updateProjectionMatrix();
    camera.updateMatrixWorld();
    const visibleWorldWidth = 2 * camera.position.z * Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2) * camera.aspect;
    pointScaleX = Math.max(horizontalOverscan, (visibleWorldWidth / gridWorldWidth) * horizontalOverscan);
    configure();
  };

  const onMove = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    if (
      isWavePointerBlocked(event) ||
      event.clientX < rect.left ||
      event.clientX > rect.right ||
      event.clientY < rect.top ||
      event.clientY > rect.bottom
    ) {
      onLeave();
      return;
    }

    const cx = (event.clientX - rect.left) / rect.width;
    const cy = (event.clientY - rect.top) / rect.height;
    const vyFull = ((cropY + (1 - cy) * rect.height) / sceneHeight) * 2 - 1;
    pointerNdc.set(cx * 2 - 1, vyFull);
    raycaster.setFromCamera(pointerNdc, camera);
    if (raycaster.ray.intersectPlane(plane, hitPoint)) {
      const ux = (hitPoint.x / pointScaleX + halfCW) / gridWorldWidth;
      const uy = (hitPoint.z + halfCD) / (POINT_ROWS * POINT_SPACING) + 0.15 - cursorBottomEdgeOffset;
      if (!hasMouse) {
        mousePrev.set(ux, uy);
        mouseTarget.set(ux, uy);
        mouseVel.set(0, 0);
        hasMouse = true;
      } else {
        mouseVel.set(ux - mousePrev.x, uy - mousePrev.y);
      }
      mousePrev.copy(mouseTarget);
      mouseTarget.set(ux, uy);
    }
  };

  const onLeave = () => {
    hasMouse = false;
    mouseTarget.set(-5, -5);
    mouseVel.set(0, 0);
  };

  const onChatKeyColor = (event: Event) => {
    const detail = (event as CustomEvent<KeyboardColorEventDetail>).detail;
    const position = keyboardColorPosition(detail?.code, detail?.key, detail?.location);
    if (!position) return;
    const [x, y] = position;
    const i = keyColorCursor;
    const hue = (x * 1.37 + y * 0.91 + keyColorSequence * 0.75487766625) % 1;
    keyColorCursor = (keyColorCursor + 1) % KEY_COLOR_EVENT_COUNT;
    keyColorSequence += 1;
    keyColorEvents[i * 4] = x;
    keyColorEvents[i * 4 + 1] = y;
    keyColorEvents[i * 4 + 2] = elapsed;
    keyColorEvents[i * 4 + 3] = hue;
    device.queue.writeBuffer(keyColorBuffer, 0, keyColorEvents);
  };

  const onWaveDrain = (event: Event) => {
    const level = (event as CustomEvent<{ level?: number }>).detail?.level;
    drainLevel = typeof level === "number" ? Math.min(1, Math.max(0, level)) : 0;
    if (drainLevel >= 1) {
      shutdownRequested = true;
    }
  };

  const detachListeners = () => {
    if (!listenersAttached) return;
    listenersAttached = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerleave", onLeave);
    window.removeEventListener("blur", onLeave);
    window.removeEventListener(CHAT_KEY_COLOR_EVENT, onChatKeyColor);
    window.removeEventListener(WAVE_DRAIN_EVENT, onWaveDrain);
  };

  const releaseResources = () => {
    if (resourcesReleased) return;
    resourcesReleased = true;
    context.unconfigure?.();
    msaaTexture?.destroy();
    msaaTexture = null;
    destroyGpuResources();
  };

  const stopRenderer = () => {
    if (stopped) return;
    stopped = true;
    if (rafId) {
      window.cancelAnimationFrame(rafId);
      rafId = 0;
    }
    resizeObserver.disconnect();
    detachListeners();
    canvas.style.display = "none";
    releaseResources();
  };

  window.addEventListener("pointermove", onMove, { passive: true });
  window.addEventListener("pointerleave", onLeave, { passive: true });
  window.addEventListener("blur", onLeave);
  window.addEventListener(CHAT_KEY_COLOR_EVENT, onChatKeyColor);
  window.addEventListener(WAVE_DRAIN_EVENT, onWaveDrain);
  listenersAttached = true;

  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas);
  resize();
  device.queue.writeBuffer(ghostBuffer, 0, ghostData);
  device.queue.writeBuffer(keyColorBuffer, 0, keyColorEvents);

  void device.lost.then((info) => {
    deviceLost = true;
    if (!stopped) {
      console.warn("InGen wave WebGPU device lost.", info);
    }
  });

  let elapsed = 0;
  let lastNow = performance.now();
  let lastRenderNow = 0;
  const frameIntervalMs = prefersReducedMotion ? WAVE_REDUCED_FRAME_INTERVAL_MS : WAVE_ACTIVE_FRAME_INTERVAL_MS;
  let rafId = 0;

  const tick = () => {
    if (stopped) return;
    const now = performance.now();
    if (document.hidden || deviceLost || !configured || !msaaTexture) {
      lastNow = now;
      lastRenderNow = now;
      if (shutdownRequested) {
        stopRenderer();
        return;
      }
      rafId = window.requestAnimationFrame(tick);
      return;
    }
    if (now - lastRenderNow < frameIntervalMs) {
      rafId = window.requestAnimationFrame(tick);
      return;
    }
    lastRenderNow = now;
    elapsed += Math.min((now - lastNow) / 1000, 0.1);
    lastNow = now;

    for (let s = 0; s < MAX_GHOSTS; s++) {
      const age = elapsed - ghostData[s * 4 + 2];
      if (age > GHOST_LIFETIME) spawnGhostData(ghostData, s, elapsed);
    }

    viewProjMatrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
    uniformData.set(viewProjMatrix.elements, 0);
    uniformData[16] = elapsed;
    uniformData[17] = prefersReducedMotion ? INTRO_REVEAL_SECONDS : Math.min(elapsed, INTRO_REVEAL_SECONDS);
    uniformData[18] = cfg.depress;
    uniformData[19] = cfg.rippleAmp;
    uniformData[20] = cfg.pointSize;
    uniformData[21] = pointScaleX;
    uniformData[22] = viewportWidth;
    uniformData[23] = viewportHeight;
    mouseUniform.copy(mouseTarget);
    uniformData[24] = mouseUniform.x;
    uniformData[25] = mouseUniform.y;
    uniformData[26] = mouseVel.x;
    uniformData[27] = mouseVel.y;
    uniformData[28] = sceneHeight;
    uniformData[29] = drainLevel;
    uniformData[30] = 0;
    uniformData[31] = 0;
    mouseVel.multiplyScalar(0.9);

    device.queue.writeBuffer(uniformBuffer, 0, uniformData);
    device.queue.writeBuffer(ghostBuffer, 0, ghostData);

    const encoder = device.createCommandEncoder({ label: "profile-wave-webgpu-frame" });
    const canvasView = context.getCurrentTexture().createView();
    const pass = encoder.beginRenderPass({
      label: "profile-wave-webgpu-render-pass",
      colorAttachments: [
        {
          view: msaaTexture.createView(),
          resolveTarget: canvasView,
          clearValue: cfg.bgColor,
          loadOp: "clear",
          storeOp: "discard"
        }
      ]
    });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.setVertexBuffer(0, pointBuffer);
    pass.draw(6, count);
    pass.end();
    device.queue.submit([encoder.finish()]);

    if (shutdownRequested) {
      stopRenderer();
      return;
    }
    rafId = window.requestAnimationFrame(tick);
  };

  rafId = window.requestAnimationFrame(tick);

  return {
    destroy() {
      stopRenderer();
    }
  };
}

function initPoolClawBanner(canvas: HTMLCanvasElement, THREE: ThreeModule): BannerHandle {
  console.info("[InGen wave renderer] WebGL renderer active.");
  const cfg = {
    bgColor: 0x0d0d0f,
    pointSize: 2.5,
    depress: 4.2,
    rippleAmp: 0.5
  };
  const prefersReducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

  const renderer = new THREE.WebGLRenderer({
    canvas,
    alpha: false,
    antialias: true,
    powerPreference: "high-performance"
  });
  renderer.setPixelRatio(1);
  renderer.setClearColor(cfg.bgColor, 1);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(58, 1, 0.1, 500);
  camera.position.set(0, 5.5, 22);
  camera.lookAt(new THREE.Vector3(0, 0, 0));

  const { basePos, pCoord, pBirth } = createWavePointField();

  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(basePos, 3));
  geo.setAttribute("pCoord", new THREE.BufferAttribute(pCoord, 2));
  geo.setAttribute("pBirth", new THREE.BufferAttribute(pBirth, 1));

  const ghostData = new Float32Array(MAX_GHOSTS * 4);
  const keyColorEvents = createKeyColorEvents();
  let keyColorCursor = 0;
  let keyColorSequence = 0;
  const spawnGhost = (slot: number, t: number) => {
    spawnGhostData(ghostData, slot, t);
  };
  for (let s = 0; s < MAX_GHOSTS; s++) {
    ghostData[s * 4] = Math.random();
    ghostData[s * 4 + 1] = Math.random();
    ghostData[s * 4 + 2] = -(s / MAX_GHOSTS) * 12;
    ghostData[s * 4 + 3] = 0.5 + Math.random() * 0.9;
  }

  const mat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    uniforms: {
      uTime: { value: 0 },
      uIntro: { value: prefersReducedMotion ? INTRO_REVEAL_SECONDS : 0 },
      uMouse: { value: new THREE.Vector2(-5, -5) },
      uMouseVel: { value: new THREE.Vector2(0, 0) },
      uDepress: { value: cfg.depress },
      uRippleAmp: { value: cfg.rippleAmp },
      uPointSize: { value: cfg.pointSize },
      uDrain: { value: 0 },
      uGhosts: { value: ghostData },
      uKeyColors: { value: keyColorEvents }
    },
    vertexShader: [
      `#define MAX_G ${MAX_GHOSTS}`,
      "attribute vec2 pCoord;",
      "attribute float pBirth;",
      "uniform float uTime;",
      "uniform float uIntro;",
      "uniform vec2 uMouse;",
      "uniform vec2 uMouseVel;",
      "uniform float uDepress;",
      "uniform float uRippleAmp;",
      "uniform float uPointSize;",
      "uniform float uDrain;",
      "uniform vec4 uGhosts[MAX_G];",
      "varying float vY;",
      "varying float vRipple;",
      "varying float vDepth;",
      "varying float vReveal;",
      "varying float vSeed;",
      "varying float vIntroChroma;",
      "varying vec2 vCoord;",
      "float getAmbient(float px,float pz,float t){float z=0.0;z+=0.085*sin(px*6.28318*1.1+t*0.045);z+=0.072*sin(pz*6.28318*0.8+t*0.035+1.1);z+=0.058*sin((px+pz)*6.28318*1.5+t*0.055+2.3);z+=0.044*sin((px-pz)*6.28318*2.1+t*0.065+3.7);return z;}",
      "float ghostRipple(vec2 p,vec4 g,float t){float age=t-g.z;if(age<0.0)return 0.0;float lifetime=12.0;if(age>lifetime)return 0.0;float fadeIn=smoothstep(0.0,2.5,age);float fadeOut=smoothstep(lifetime,lifetime-3.0,age);float envelope=fadeIn*fadeOut;float dist=length(p-g.xy);float front=age*0.032;float width=0.18;float ring=exp(-((dist-front)*(dist-front))/(width*width));float osc=sin(dist*8.0-age*1.0);return g.w*1.28*ring*envelope*osc;}",
      `void main(){float px=pCoord.x;float pz=pCoord.y;float t=uTime;vec2 p=vec2(px,pz);float reveal=smoothstep(pBirth,pBirth+0.42,uIntro);float seed=1.0-smoothstep(0.0,0.001,pBirth);float seedPulse=seed*smoothstep(0.02,0.16,uIntro)*(1.0-smoothstep(0.45,1.1,uIntro));float fresh=1.0-smoothstep(pBirth+0.18,pBirth+1.12,uIntro);vec2 mdiff=p-uMouse;float mdist=length(mdiff);float depress=-uDepress*exp(-mdist*mdist*34.0);float mripple=uRippleAmp*exp(-mdist*6.1)*sin(mdist*8.0-t*0.6);float mspeed=length(uMouseVel)*40.0;float turb=mspeed*0.28*exp(-mdist*mdist*44.0)*sin(mdist*5.0-t*0.9);float ghostSum=0.0;for(int i=0;i<MAX_G;i++){ghostSum+=ghostRipple(p,uGhosts[i],t);}float yTotal=(depress+mripple+turb+ghostSum+getAmbient(px,pz,t))*reveal;vY=yTotal;vRipple=(mripple+ghostSum)*reveal;vDepth=pCoord.y;vReveal=reveal;vSeed=seedPulse;vIntroChroma=reveal*(0.32+fresh*0.72)+seedPulse;vCoord=pCoord;float drainHash=fract(sin(dot(pCoord,vec2(127.1,311.7)))*43758.5453);float keep=smoothstep(uDrain*1.04-0.04,uDrain*1.04,drainHash);vec3 pos=position;pos.y=yTotal-step(keep,0.01)*1000.0;gl_PointSize=max(1.0,uPointSize*(reveal+seedPulse*1.8))*max(keep,0.001);gl_Position=projectionMatrix*modelViewMatrix*vec4(pos,1.0);}`
    ].join("\n"),
    fragmentShader: [
      "varying float vY;",
      "varying float vRipple;",
      "varying float vDepth;",
      "varying float vReveal;",
      "varying float vSeed;",
      "varying float vIntroChroma;",
      "varying vec2 vCoord;",
      `#define MAX_K ${KEY_COLOR_EVENT_COUNT}`,
      "uniform float uTime;",
      "uniform vec4 uKeyColors[MAX_K];",
      "vec3 hsvKeyboard(float h,float s,float v){vec3 p=abs(fract(vec3(h)+vec3(0.0,0.6666667,0.3333333))*6.0-3.0);vec3 rgb=clamp(p-1.0,0.0,1.0);rgb=rgb*rgb*(3.0-2.0*rgb);return v*mix(vec3(1.0),rgb,s);}",
      "vec3 keyboardPalette(float hue){float h=fract(hue);float warm=smoothstep(0.95,1.0,h)+1.0-smoothstep(0.0,0.08,h);float cool=smoothstep(0.38,0.72,h)*(1.0-smoothstep(0.72,0.88,h));float value=mix(0.92,1.0,cool)-warm*0.05;vec3 c=hsvKeyboard(h,0.97,value);return clamp(pow(c,vec3(0.78)),0.0,1.0);}",
      "vec3 saturateKeyboardColor(vec3 c){float l=dot(c,vec3(0.299,0.587,0.114));return clamp(mix(vec3(l),c,2.55),0.0,1.0);}",
      "void main(){float fog=1.0-clamp(vDepth*0.72,0.0,0.68);float chroma=clamp(vIntroChroma,0.0,1.15);float bright=clamp(0.50+vY*0.10+vSeed*0.42+chroma*0.08,0.05,0.98)*fog;float redness=clamp(-vY*0.28+abs(vRipple)*2.2+vSeed*0.35+chroma*0.58,0.0,1.0);float rs=clamp(vRipple*3.0+chroma*0.34,-1.0,1.0);float amber=clamp(chroma*0.34+max(vRipple,0.0)*0.42,0.0,0.55);float cyan=clamp((1.0-smoothstep(0.0,0.62,redness))*abs(vRipple)*0.22+smoothstep(0.18,0.72,chroma)*0.045,0.0,0.16);vec3 baseColor=vec3(bright+redness*0.70+rs*0.22+amber*0.18,bright-redness*0.18-rs*0.05+chroma*0.025+amber*0.14+cyan*0.22,bright-redness*0.20-rs*0.14+cyan*0.30);vec3 keyAccum=vec3(0.0);vec3 keyBias=vec3(0.0);float keyWeight=0.0;float keyBiasWeight=0.0;for(int i=0;i<MAX_K;i++){vec4 k=uKeyColors[i];float age=uTime-k.z;if(age>=0.0&&age<8.8&&k.w>=0.0){float birth=smoothstep(0.0,0.24,age);float life=1.0-smoothstep(5.6,8.6,age);float spread=smoothstep(0.0,3.1,age);float radius=mix(0.026,0.24,spread);float dist=length((vCoord-k.xy)*vec2(1.0,1.22));float diffuse=exp(-(dist*dist)/(radius*radius*1.18));float perimeter=1.0-smoothstep(radius*0.78,radius*1.46,dist);float halo=exp(-(dist*dist)/0.055)*smoothstep(1.1,3.5,age)*0.014;float w=(diffuse*0.78+perimeter*0.22+halo)*birth*life*0.70;float bw=pow(max(w,0.0),1.26);float hueShift=smoothstep(radius*0.22,radius*1.25,dist)*0.11+smoothstep(radius*0.70,radius*1.35,dist)*0.08;vec3 c=mix(keyboardPalette(k.w),keyboardPalette(fract(k.w+hueShift)),0.48);keyAccum+=c*w;keyWeight+=w;keyBias+=c*bw;keyBiasWeight+=bw;}}float keyMix=clamp((1.0-exp(-keyWeight*0.92))*0.88,0.0,0.88)*vReveal;vec3 avgColor=keyWeight>0.001?keyAccum/max(keyWeight,0.001):baseColor;vec3 biasColor=keyBiasWeight>0.001?keyBias/max(keyBiasWeight,0.001):avgColor;vec3 mappedColor=keyWeight>0.001?saturateKeyboardColor(mix(avgColor,biasColor,0.38)):baseColor;vec3 litKey=mappedColor*(0.58+bright*0.38+redness*0.04);vec3 mutedBase=baseColor*(1.0-keyMix*0.32);vec3 finalColor=mix(mutedBase,litKey,keyMix);float alpha=clamp(vReveal+vSeed,0.0,1.0);gl_FragColor=vec4(clamp(finalColor,0.,1.),alpha);}"
    ].join("\n")
  });

  const points = new THREE.Points(geo, mat);
  scene.add(points);

  const raycaster = new THREE.Raycaster();
  const plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  const hitPoint = new THREE.Vector3();
  const pointerNdc = new THREE.Vector2();
  const mouseTarget = new THREE.Vector2(-5, -5);
  const mousePrev = new THREE.Vector2(-5, -5);
  const mouseVel = new THREE.Vector2(0, 0);
  const halfCW = (POINT_COLS / 2) * POINT_SPACING;
  const halfCD = (POINT_ROWS / 2) * POINT_SPACING;
  const gridWorldWidth = POINT_COLS * POINT_SPACING;
  const horizontalOverscan = 1.62;
  const cursorBottomEdgeOffset = 0.18;
  let sceneHeight = 600;
  let cropY = 240;
  let pointScaleX = 1;
  let hasMouse = false;
  let drainLevel = 0;
  let shutdownRequested = false;
  let stopped = false;
  let listenersAttached = false;
  let resourcesReleased = false;

  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(rect.width));
    const height = Math.max(1, Math.round(rect.height));
    sceneHeight = Math.max(600, height);
    cropY = Math.max(0, Math.round((sceneHeight - height) / 2));
    camera.aspect = width / sceneHeight;
    camera.updateProjectionMatrix();
    const visibleWorldWidth = 2 * camera.position.z * Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2) * camera.aspect;
    pointScaleX = Math.max(horizontalOverscan, (visibleWorldWidth / gridWorldWidth) * horizontalOverscan);
    points.scale.x = pointScaleX;
    renderer.setSize(width, height, false);
    renderer.setScissorTest(true);
    renderer.setScissor(0, 0, width, height);
    renderer.setViewport(0, -cropY, width, sceneHeight);
  };

  const onMove = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    if (
      isWavePointerBlocked(event) ||
      event.clientX < rect.left ||
      event.clientX > rect.right ||
      event.clientY < rect.top ||
      event.clientY > rect.bottom
    ) {
      onLeave();
      return;
    }

    const cx = (event.clientX - rect.left) / rect.width;
    const cy = (event.clientY - rect.top) / rect.height;
    const vyFull = ((cropY + (1 - cy) * rect.height) / sceneHeight) * 2 - 1;
    pointerNdc.set(cx * 2 - 1, vyFull);
    raycaster.setFromCamera(pointerNdc, camera);
    if (raycaster.ray.intersectPlane(plane, hitPoint)) {
      const ux = (hitPoint.x / pointScaleX + halfCW) / gridWorldWidth;
      const uy = (hitPoint.z + halfCD) / (POINT_ROWS * POINT_SPACING) + 0.15 - cursorBottomEdgeOffset;
      if (!hasMouse) {
        mousePrev.set(ux, uy);
        mouseTarget.set(ux, uy);
        mouseVel.set(0, 0);
        hasMouse = true;
      } else {
        mouseVel.set(ux - mousePrev.x, uy - mousePrev.y);
      }
      mousePrev.copy(mouseTarget);
      mouseTarget.set(ux, uy);
    }
  };

  const onLeave = () => {
    hasMouse = false;
    mouseTarget.set(-5, -5);
    mouseVel.set(0, 0);
  };

  const onChatKeyColor = (event: Event) => {
    const detail = (event as CustomEvent<KeyboardColorEventDetail>).detail;
    const position = keyboardColorPosition(detail?.code, detail?.key, detail?.location);
    if (!position) return;
    const [x, y] = position;
    const i = keyColorCursor;
    const hue = (x * 1.37 + y * 0.91 + keyColorSequence * 0.75487766625) % 1;
    keyColorCursor = (keyColorCursor + 1) % KEY_COLOR_EVENT_COUNT;
    keyColorSequence += 1;
    keyColorEvents[i * 4] = x;
    keyColorEvents[i * 4 + 1] = y;
    keyColorEvents[i * 4 + 2] = elapsed;
    keyColorEvents[i * 4 + 3] = hue;
    mat.uniforms.uKeyColors.value = keyColorEvents;
  };

  const onWaveDrain = (event: Event) => {
    const level = (event as CustomEvent<{ level?: number }>).detail?.level;
    drainLevel = typeof level === "number" ? Math.min(1, Math.max(0, level)) : 0;
    mat.uniforms.uDrain.value = drainLevel;
    if (drainLevel >= 1) {
      shutdownRequested = true;
    }
  };

  const detachListeners = () => {
    if (!listenersAttached) return;
    listenersAttached = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerleave", onLeave);
    window.removeEventListener("blur", onLeave);
    window.removeEventListener(CHAT_KEY_COLOR_EVENT, onChatKeyColor);
    window.removeEventListener(WAVE_DRAIN_EVENT, onWaveDrain);
  };

  const releaseResources = () => {
    if (resourcesReleased) return;
    resourcesReleased = true;
    renderer.dispose();
    renderer.forceContextLoss();
    geo.dispose();
    mat.dispose();
  };

  const stopRenderer = () => {
    if (stopped) return;
    stopped = true;
    if (rafId) {
      window.cancelAnimationFrame(rafId);
      rafId = 0;
    }
    resizeObserver.disconnect();
    detachListeners();
    canvas.style.display = "none";
    releaseResources();
  };

  window.addEventListener("pointermove", onMove, { passive: true });
  window.addEventListener("pointerleave", onLeave, { passive: true });
  window.addEventListener("blur", onLeave);
  window.addEventListener(CHAT_KEY_COLOR_EVENT, onChatKeyColor);
  window.addEventListener(WAVE_DRAIN_EVENT, onWaveDrain);
  listenersAttached = true;

  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas);
  resize();

  let elapsed = 0;
  let lastNow = performance.now();
  let lastRenderNow = 0;
  const frameIntervalMs = prefersReducedMotion ? WAVE_REDUCED_FRAME_INTERVAL_MS : WAVE_ACTIVE_FRAME_INTERVAL_MS;
  let rafId = 0;

  const tick = () => {
    if (stopped) return;
    const now = performance.now();
    if (document.hidden) {
      lastNow = now;
      lastRenderNow = now;
      if (shutdownRequested) {
        stopRenderer();
        return;
      }
      rafId = window.requestAnimationFrame(tick);
      return;
    }
    if (now - lastRenderNow < frameIntervalMs) {
      rafId = window.requestAnimationFrame(tick);
      return;
    }
    lastRenderNow = now;
    elapsed += Math.min((now - lastNow) / 1000, 0.1);
    lastNow = now;

    for (let s = 0; s < MAX_GHOSTS; s++) {
      const age = elapsed - ghostData[s * 4 + 2];
      if (age > GHOST_LIFETIME) spawnGhost(s, elapsed);
    }

    mat.uniforms.uGhosts.value = ghostData;
    mat.uniforms.uTime.value = elapsed;
    mat.uniforms.uIntro.value = prefersReducedMotion ? INTRO_REVEAL_SECONDS : Math.min(elapsed, INTRO_REVEAL_SECONDS);
    mat.uniforms.uMouse.value.copy(mouseTarget);
    mouseVel.multiplyScalar(0.9);
    mat.uniforms.uMouseVel.value.copy(mouseVel);
    renderer.render(scene, camera);
    if (shutdownRequested) {
      stopRenderer();
      return;
    }
    rafId = window.requestAnimationFrame(tick);
  };

  rafId = window.requestAnimationFrame(tick);

  return {
    destroy() {
      stopRenderer();
    }
  };
}

function ProfileWaveCanvas() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    let disposed = false;
    let banner: BannerHandle | null = null;

    const mount = async () => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const THREE = await import("three");
      if (disposed) return;
      const webGpuBanner = await initPoolClawWebGpuBanner(canvas, THREE).catch((error) => {
        console.warn("InGen wave WebGPU initialization failed; falling back to WebGL.", error);
        return null;
      });
      if (disposed) {
        webGpuBanner?.destroy();
        return;
      }
      banner = webGpuBanner ?? initPoolClawBanner(canvas, THREE);
    };

    void mount();

    return () => {
      disposed = true;
      banner?.destroy();
    };
  }, []);

  return <canvas ref={canvasRef} className="profileCoverBanner" aria-hidden="true" />;
}

export function ProfileCoverBanner({
  leftPanelOpen = false,
  welcomeMessage
}: {
  leftPanelOpen?: boolean;
  welcomeMessage?: WelcomeMessage;
}) {
  const welcomeText = welcomeMessage?.text ?? "Focus on signal over noise.";
  const welcomeAuthor = welcomeMessage?.author ?? "";
  const welcomeRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const welcome = welcomeRef.current;
    if (!welcome) {
      return;
    }
    welcome.style.opacity = "";
    delete welcome.dataset.burstConsumed;
  }, [welcomeText, welcomeAuthor]);

  return (
    <div
      className={leftPanelOpen ? "profileCoverBannerFrame profileCoverBannerFrame--leftOpen" : "profileCoverBannerFrame"}
      aria-hidden="true"
    >
      <ProfileWaveCanvas />
      <div ref={welcomeRef} className="profileCoverWelcomeText">
        <strong>{welcomeText}</strong>
        {welcomeAuthor ? <span>{welcomeAuthor}</span> : null}
      </div>
    </div>
  );
}
