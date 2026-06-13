import { useEffect, useRef, useState } from "react";
import {
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  type BrainCodeActCommand,
  type SidebarSessionItem
} from "../shared/ipc-contract";
import {
  readBrainAgentMemory,
  readBrainUserMemory,
  writeBrainAgentMemory,
  writeBrainUserMemory
} from "./brain-user-memory-store";
import { headerShadowStore } from "./header-shadow-store";
import { AirbnbIcon, CubeIcon, GmailIcon, GoogleIcon } from "./module-logos";
import { panelsChatBottomStore } from "./panels-chat-bottom-store";
import { sidebarShadowStore, useSidebarShadowStore } from "./sidebar-shadow-store";

type BrainSpace = "codeacts" | "memory" | "godel" | "personality";
type BrainBlobHandle = { destroy(): void };
type WebGpuBuffer = { destroy(): void };
type WebGpuBufferData = ArrayBuffer | ArrayBufferView<ArrayBufferLike>;
type WebGpuQueue = {
  writeBuffer(buffer: WebGpuBuffer, bufferOffset: number, data: WebGpuBufferData, dataOffset?: number, size?: number): void;
  submit(commandBuffers: unknown[]): void;
};
type WebGpuDevice = {
  queue: WebGpuQueue;
  lost: Promise<unknown>;
  createBuffer(descriptor: { label?: string; size: number; usage: number }): WebGpuBuffer;
  createShaderModule(descriptor: { label?: string; code: string }): unknown;
  createRenderPipeline(descriptor: Record<string, unknown>): unknown;
  createBindGroup(descriptor: { label?: string; layout: unknown; entries: Array<{ binding: number; resource: unknown }> }): unknown;
  createCommandEncoder(descriptor?: { label?: string }): {
    beginRenderPass(descriptor: Record<string, unknown>): {
      setPipeline(pipeline: unknown): void;
      setBindGroup(index: number, bindGroup: unknown): void;
      draw(vertexCount: number, instanceCount: number): void;
      end(): void;
    };
    finish(): unknown;
  };
  destroy?: () => void;
};
type WebGpuAdapter = { requestDevice(descriptor?: Record<string, unknown>): Promise<WebGpuDevice> };
type WebGpuRuntime = {
  requestAdapter(options?: { powerPreference?: "high-performance" | "low-power" }): Promise<WebGpuAdapter | null>;
  getPreferredCanvasFormat(): string;
};
type WebGpuCanvasContext = {
  configure(config: { device: WebGpuDevice; format: string; alphaMode: "premultiplied" | "opaque" }): void;
  getCurrentTexture(): { createView(): unknown };
  unconfigure?: () => void;
};
type WebGpuHostNavigator = { gpu?: WebGpuRuntime };

const WEBGPU_BUFFER_USAGE = {
  COPY_DST: 0x8,
  UNIFORM: 0x40
} as const;
const BRAIN_BLOB_SUPERSAMPLE = 2.2;
const BRAIN_BLOB_MAX_FRAMEBUFFER_SIDE = 2400;

/* SDF port of the Uiverse "andrew-manzyk" gooey loader: the seven blurred
   polygons become free-floating metaballs orbiting their CSS transform-origins,
   smooth-min plays the blur+contrast fusion. No containing circle: the goo is
   soft-bodied, with slow orbits, breathing radii, jelly domain warp and mushy
   translucent edges. Orbits, radii, pivots and phases are jittered by a
   per-session random seed. */
const BRAIN_BLOB_SHADER = /* wgsl */ `
const TAU: f32 = 6.28318530718;
/* Loader contract: --time-animation 2s; roundness runs at /2, colorize at *3. */
const TIME_ANIM: f32 = 2.0;

struct Uniforms {
  resolution: vec2<f32>,
  time: f32,
  reducedMotion: f32,
  seed: f32,
  pad0: f32,
  pad1: f32,
  pad2: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOut {
  @builtin(position) position: vec4<f32>,
};

@vertex
fn vertexMain(@builtin(vertex_index) vertexIndex: u32) -> VertexOut {
  var positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0)
  );
  var out: VertexOut;
  out.position = vec4<f32>(positions[vertexIndex], 0.0, 1.0);
  return out;
}

fn saturate(v: f32) -> f32 {
  return clamp(v, 0.0, 1.0);
}

fn hash21(p: vec2<f32>) -> f32 {
  let h = dot(p, vec2<f32>(127.1, 311.7));
  return fract(sin(h) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = hash21(i);
  let b = hash21(i + vec2<f32>(1.0, 0.0));
  let c = hash21(i + vec2<f32>(0.0, 1.0));
  let d = hash21(i + vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p0: vec2<f32>) -> f32 {
  var p = p0;
  var amp = 0.5;
  var sum = 0.0;
  for (var i = 0; i < 5; i = i + 1) {
    sum += noise2(p) * amp;
    p = p * 2.03 + vec2<f32>(13.1, 7.7);
    amp *= 0.52;
  }
  return sum;
}

fn seededRand(n: f32) -> f32 {
  return fract(sin(n * 12.9898 + uniforms.seed * 78.233) * 43758.5453);
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = saturate(0.5 + 0.5 * (b - a) / k);
  return mix(b, a, h) - k * h * (1.0 - h);
}

/* CSS roundness keyframes: contrast 15 -> 3 -> 15, i.e. tight fusion that
   periodically relaxes into a soft gooey mush. Slowed down so the breathing
   reads as squish, not flicker. */
fn roundnessMix(t: f32) -> f32 {
  let phase = fract(t / (TIME_ANIM * 2.0));
  if (phase < 0.2) {
    return smoothstep(0.0, 0.2, phase);
  }
  if (phase < 0.4) {
    return 1.0;
  }
  if (phase < 0.6) {
    return 1.0 - smoothstep(0.4, 0.6, phase);
  }
  return 0.0;
}

/* Free-floating gooey mass: slow orbits, vertical bobbing and breathing
   radii give the squishy soft-body feel. */
fn blobField(q: vec2<f32>, t: f32, k: f32) -> f32 {
  var pivots = array<vec2<f32>, 7>(
    vec2<f32>(0.30, -0.30),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 0.12),
    vec2<f32>(-0.12, -0.12),
    vec2<f32>(-0.12, -0.12),
    vec2<f32>(0.12, -0.12),
    vec2<f32>(0.12, -0.12)
  );
  var dirs = array<f32, 7>(0.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
  var delayPhase = array<f32, 7>(0.0, 0.0, TAU / 3.0, 0.0, TAU / 2.0, 0.0, TAU / 1.5);
  let baseSpeed = (TAU / TIME_ANIM) * 0.45;
  var d = 1e5;
  for (var i = 0; i < 7; i = i + 1) {
    let fi = f32(i);
    let speed = dirs[i] * baseSpeed * mix(0.72, 1.28, seededRand(fi * 7.31 + 1.7));
    let phase = delayPhase[i] + seededRand(fi * 3.97 + 9.2) * TAU;
    let orbit = mix(0.08, 0.20, seededRand(fi * 5.53 + 4.4));
    let breath = 1.0 + sin(t * mix(0.5, 1.1, seededRand(fi * 4.41 + 3.3)) + phase * 2.0) * 0.14;
    let radius = mix(0.26, 0.42, seededRand(fi * 2.17 + 6.6)) * breath;
    let pivot = pivots[i] + (vec2<f32>(seededRand(fi * 9.13 + 2.9), seededRand(fi * 6.71 + 8.1)) - 0.5) * 0.22;
    let bob = vec2<f32>(0.0, sin(t * mix(0.4, 0.9, seededRand(fi * 8.21 + 5.5)) + phase) * 0.05);
    let a = phase + t * speed;
    let center = pivot + bob + vec2<f32>(cos(a), sin(a)) * orbit;
    d = smin(d, length(q - center) - radius, k);
  }
  return d;
}

fn hueRotate(color: vec3<f32>, angle: f32) -> vec3<f32> {
  let c = cos(angle);
  let s = sin(angle);
  let weights = vec3<f32>(0.213, 0.715, 0.072);
  return vec3<f32>(
    dot(color, vec3<f32>(weights.x + c * (1.0 - weights.x) + s * (-weights.x), weights.y + c * (-weights.y) + s * (-weights.y), weights.z + c * (-weights.z) + s * (1.0 - weights.z))),
    dot(color, vec3<f32>(weights.x + c * (-weights.x) + s * 0.143, weights.y + c * (1.0 - weights.y) + s * 0.140, weights.z + c * (-weights.z) + s * -0.283)),
    dot(color, vec3<f32>(weights.x + c * (-weights.x) + s * (-(1.0 - weights.x)), weights.y + c * (-weights.y) + s * weights.y, weights.z + c * (1.0 - weights.z) + s * weights.z))
  );
}

fn colorizeAngle(time: f32) -> f32 {
  let phase = fract(time / (TIME_ANIM * 3.0));
  if (phase < 0.2) {
    return mix(0.0, -0.5235988, smoothstep(0.0, 0.2, phase));
  }
  if (phase < 0.4) {
    return mix(-0.5235988, -1.0471976, smoothstep(0.2, 0.4, phase));
  }
  if (phase < 0.6) {
    return mix(-1.0471976, -1.5707963, smoothstep(0.4, 0.6, phase));
  }
  if (phase < 0.8) {
    return mix(-1.5707963, -0.7853982, smoothstep(0.6, 0.8, phase));
  }
  return mix(-0.7853982, 0.0, smoothstep(0.8, 1.0, phase));
}

fn over(dst: vec4<f32>, src: vec3<f32>, srcAlpha: f32) -> vec4<f32> {
  let a = saturate(srcAlpha);
  return vec4<f32>(src * a + dst.rgb * (1.0 - a), a + dst.a * (1.0 - a));
}

@fragment
fn sceneMain(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
  let res = max(uniforms.resolution, vec2<f32>(1.0));
  let shortSide = min(res.x, res.y);
  let px = 1.0 / shortSide;
  let uv = (position.xy - 0.5 * res) / shortSide;
  let t = uniforms.time * (1.0 - uniforms.reducedMotion) + seededRand(3.3) * 31.7;

  let frameScale = 0.52;
  let q = (uv - vec2<f32>(0.0, 0.015)) / frameScale;

  /* Jelly domain warp: big slow folds plus a finer tremble. */
  let warp = (vec2<f32>(
    fbm(q * 1.6 + vec2<f32>(t * 0.09, 3.1)),
    fbm(q * 1.6 + vec2<f32>(7.7, t * 0.07))
  ) - vec2<f32>(0.5)) * 0.34
  + (vec2<f32>(
    fbm(q * 3.4 + vec2<f32>(-t * 0.13, 11.3)),
    fbm(q * 3.4 + vec2<f32>(5.9, t * 0.11))
  ) - vec2<f32>(0.5)) * 0.10;

  let k = mix(0.24, 0.46, roundnessMix(t));
  var fieldDist = blobField(q + warp, t, k);
  fieldDist += (fbm(q * 2.6 + vec2<f32>(t * 0.10, -t * 0.06)) - 0.5) * 0.10;
  /* Wide organic fade near the canvas border so the goo never meets a hard
     edge; this is a mushy ramp, not a containing circle. */
  fieldDist += smoothstep(0.62, 0.98, length(q)) * 1.0;
  let blobDist = fieldDist * frameScale;

  let aa = max(abs(dpdx(blobDist)) + abs(dpdy(blobDist)), px * 1.75);
  let crisp = smoothstep(aa, -aa, blobDist);
  let mush = 1.0 - smoothstep(-0.035, 0.075, blobDist);
  let body = saturate(crisp * 0.62 + mush * 0.38);
  let depth = saturate(-blobDist / 0.17);

  let colorOne = vec3<f32>(1.0, 0.749, 0.282);
  let colorTwo = vec3<f32>(0.745, 0.290, 0.114);
  /* linear-gradient(180deg, color-one 30%, color-two 70%), bent by the warp. */
  let gradT = smoothstep(-0.7, 0.7, q.y + warp.y * 0.8);

  let grain = fbm(q * 3.2 + vec2<f32>(t * 0.05, -t * 0.04));
  var blobColor = mix(colorOne, colorTwo, gradT) * (0.88 + grain * 0.16 + depth * 0.12);
  /* Subsurface band between rim and core: backlit-gel translucency. */
  blobColor += vec3<f32>(1.0, 0.90, 0.78) * (1.0 - depth) * depth * 0.30;
  blobColor += vec3<f32>(1.0, 0.93, 0.84) * pow(depth, 3.0) * 0.08;

  /* Soft aura hugging the goo contour, replacing the old circular halo. */
  let glowAlpha = exp(-max(blobDist, 0.0) / 0.11) * 0.16;
  let glowColor = mix(colorOne, colorTwo, gradT);

  var acc = vec4<f32>(0.0);
  acc = over(acc, glowColor, glowAlpha * (1.0 - body));
  acc = over(acc, blobColor, body * (0.46 + depth * 0.34));

  let vignette = 1.0 - smoothstep(0.40, 0.495, max(abs(uv.x), abs(uv.y)));
  let rgb = max(hueRotate(acc.rgb / max(acc.a, 0.001), colorizeAngle(t)), vec3<f32>(0.0));
  return vec4<f32>(min(rgb, vec3<f32>(0.98)), saturate(acc.a * vignette));
}
`;

/* Stroke glyphs follow the sidebar icon contract: 24-unit viewBox, 1.65 stroke. */
function Glyph({ kind, size = 16 }: { kind: string; size?: number }) {
  const base = {
    className: "brainGlyph",
    viewBox: "0 0 24 24",
    width: size,
    height: size,
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.65,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true
  };
  if (kind === "brain") {
    return (
      <svg {...base} viewBox="2.25 2.25 15.5 15.5">
        <path d="M9.5 4.5c0-.1-.02-.48-.15-.82a1.22 1.22 0 0 0-.32-.5A.76.76 0 0 0 8.5 3a2.91 2.91 0 0 0-1.76.58C6.28 3.94 6 4.43 6 5a.5.5 0 0 1-.66.47c-.18-.06-.35-.02-.53.12-.2.16-.39.45-.53.83-.28.78-.25 1.73.14 2.3A.5.5 0 0 1 4.5 9h.75a2.25 2.25 0 0 1 2.25 2.25v.34m2-7.09v10m0-7H8.42m2.08 7h.75c.69 0 1.25-.56 1.25-1.25v-1.84M9.5 15.47c-.05.12-.22.45-.55.81-.39.41-.89.72-1.45.72-.81 0-1.43-.4-1.86-.94-.44-.55-.64-1.19-.64-1.56a.5.5 0 0 0-.5-.5c-.13 0-.52-.08-.86-.38C3.31 13.34 3 12.86 3 12c0-.98.12-1.63.32-2.03m7.18-5.47c0-.1.02-.48.15-.82.08-.2.18-.37.32-.5A.76.76 0 0 1 11.5 3c.63 0 1.25.2 1.76.58.46.36.74.85.74 1.42a.5.5 0 0 0 .66.47c.18-.06.35-.02.53.12.2.16.39.45.53.83.28.78.25 1.73-.14 2.3A.5.5 0 0 0 16 9.5c.13 0 .26.03.38.1.12.08.22.2.3.37.2.4.32 1.05.32 2.03 0 .86-.31 1.34-.64 1.62-.34.3-.73.38-.86.38a.5.5 0 0 0-.5.5c0 .37-.2 1.01-.64 1.56-.43.54-1.05.94-1.86.94-.56 0-1.06-.31-1.45-.72a3.63 3.63 0 0 1-.55-.81M6.5 7a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm6 2a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm-6 4a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Z" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }
  if (kind === "identity-card") {
    return (
      <svg {...base} viewBox="0 0 256 256" fill="currentColor" stroke="none">
        <path d="M75.19 198.4a8 8 0 0 0 11.21-1.6a52 52 0 0 1 83.2 0a8 8 0 1 0 12.8-9.6a67.88 67.88 0 0 0-27.4-21.69a40 40 0 1 0-53.94 0A67.88 67.88 0 0 0 73.6 187.2a8 8 0 0 0 1.59 11.2ZM128 112a24 24 0 1 1-24 24a24 24 0 0 1 24-24Zm72-88H56a16 16 0 0 0-16 16v176a16 16 0 0 0 16 16h144a16 16 0 0 0 16-16V40a16 16 0 0 0-16-16Zm0 192H56V40h144ZM88 64a8 8 0 0 1 8-8h64a8 8 0 0 1 0 16H96a8 8 0 0 1-8-8Z" />
      </svg>
    );
  }
  if (kind === "terminal") {
    return <svg {...base}><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>;
  }
  if (kind === "database") {
    return <svg {...base}><ellipse cx="12" cy="5" rx="9" ry="3" /><path d="M3 5v14a9 3 0 0 0 18 0V5" /><path d="M3 12a9 3 0 0 0 18 0" /></svg>;
  }
  if (kind === "shield-check") {
    return (
      <svg {...base}>
        <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />
        <path d="m9 12 2 2 4-4" />
      </svg>
    );
  }
  if (kind === "masks") {
    return (
      <svg {...base} viewBox="0 0 24 25" strokeWidth="1.5">
        <path strokeLinecap="round" d="M5.445 14.775a1.11 1.11 0 0 1 .777-.59c.339-.061.672.053.928.282m4.086 3.31c-.327.61-.878 1.057-1.555 1.18c-.677.122-1.344-.105-1.855-.565m2.733-4.54c.164-.305.439-.529.777-.59c.34-.06.672.053.928.283m.806-5.903c-1.15 1.086-2.899 1.95-4.94 2.318c-2.04.368-3.97.168-5.415-.45a.5.5 0 0 0-.289-.035c-.284.05-.47.348-.417.663l.938 5.443c.7 4.058 4.1 6.007 5.677 6.704c.522.232 1.098.261 1.658.16s1.092-.33 1.506-.73c1.249-1.208 3.792-4.229 3.092-8.287l-.937-5.443c-.055-.315-.33-.529-.614-.477a.5.5 0 0 0-.26.134" />
        <path d="M14.316 17.5c.363 0 .723-.065 1.06-.215c1.577-.697 4.977-2.646 5.677-6.704l.938-5.443c.054-.315-.133-.612-.417-.663a.5.5 0 0 0-.289.035c-1.444.618-3.375.818-5.416.45c-2.04-.368-3.788-1.232-4.939-2.318a.5.5 0 0 0-.259-.134c-.284-.052-.56.162-.614.477L9.12 8.428c-.083.477-.12.94-.12 1.386" />
      </svg>
    );
  }
  if (kind === "codeact") {
    return (
      <svg {...base}>
        <line x1="11.5" y1="4.5" x2="5.5" y2="19.5" />
        <line x1="10" y1="19.5" x2="19" y2="19.5" />
      </svg>
    );
  }
  if (kind === "archive") {
    return <svg {...base}><rect x="2" y="3" width="20" height="5" /><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" /><path d="M10 12h4" /></svg>;
  }
  if (kind === "globe") {
    return <svg {...base}><circle cx="12" cy="12" r="10" /><line x1="2" y1="12" x2="22" y2="12" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" /></svg>;
  }
  if (kind === "image") {
    return <svg {...base}><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="9" cy="9" r="2" /><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" /></svg>;
  }
  if (kind === "questionnaire") {
    return <svg {...base}><path d="M8 6h13" /><path d="M8 12h13" /><path d="M8 18h13" /><path d="M3 6h.01" /><path d="M3 12h.01" /><path d="M3 18h.01" /></svg>;
  }
  if (kind === "pencil") {
    return <svg {...base}><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" /><path d="m15 5 4 4" /></svg>;
  }
  if (kind === "folder") {
    return (
      <svg {...base}>
        <path d="M3.75 7.25A2.25 2.25 0 0 1 6 5h4.15l2 2H18a2.25 2.25 0 0 1 2.25 2.25v7.5A2.25 2.25 0 0 1 18 19H6a2.25 2.25 0 0 1-2.25-2.25v-9.5Z" fill="currentColor" stroke="none" />
      </svg>
    );
  }
  if (kind === "cpu") {
    return (
      <svg {...base}>
        <rect x="4" y="4" width="16" height="16" rx="2" />
        <rect x="9" y="9" width="6" height="6" />
        <path d="M9 2v2M15 2v2M9 20v2M15 20v2M2 9h2M2 15h2M20 9h2M20 15h2" />
      </svg>
    );
  }
  if (kind === "reuse") {
    return <svg {...base}><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /></svg>;
  }
  if (kind === "zap") {
    return <svg {...base}><path d="M13 2 3 14h7l-1 8 10-12h-7l1-8z" /></svg>;
  }
  if (kind === "layout") {
    return <svg {...base}><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 9h18" /><path d="M9 21V9" /></svg>;
  }
  if (kind === "calendar") {
    return <svg {...base}><rect x="3" y="4" width="18" height="18" rx="2" /><path d="M16 2v4M8 2v4M3 10h18" /></svg>;
  }
  if (kind === "modules") {
    return (
      <svg {...base} viewBox="2 2 20 20" strokeWidth="2">
        <rect height="6" rx="0.86" width="6" x="4" y="4" />
        <rect height="6" rx="0.86" width="6" x="4" y="14" />
        <rect height="6" rx="0.86" width="6" x="14" y="14" />
        <rect height="6" rx="0.86" width="6" x="14" y="4" />
      </svg>
    );
  }
  if (kind === "plug") {
    return <svg {...base}><path d="M12 22v-5" /><path d="M9 8V2" /><path d="M15 8V2" /><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z" /></svg>;
  }
  if (kind === "plus") {
    return <svg {...base}><path d="M12 5v14" /><path d="M5 12h14" /></svg>;
  }
  if (kind === "minus") {
    return <svg {...base}><path d="M5 12h14" /></svg>;
  }
  if (kind === "flask") {
    return <svg {...base}><path d="M10 2v6.6L4.7 18a2 2 0 0 0 1.8 3h11a2 2 0 0 0 1.8-3L14 8.6V2" /><path d="M8.5 2h7" /><path d="M7 15h10" /></svg>;
  }
  if (kind === "code") {
    return <svg {...base}><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></svg>;
  }
  return <svg {...base}><circle cx="12" cy="12" r="9" /></svg>;
}

function CodeActIcon({ command }: { command: BrainCodeActCommand }) {
  if (command === "/gmail_" || command === "/gmail_com") return <GmailIcon />;
  if (command === "/airbnb_") return <AirbnbIcon />;
  if (command === "/googleweb_") return <GoogleIcon />;
  if (command === "/newobject_") return <CubeIcon />;
  if (command === "/questionnaire_") return <Glyph kind="questionnaire" />;
  const stroke: Partial<Record<BrainCodeActCommand, string>> = {
    "/searcharchive_": "archive",
    "/sciencebrain_": "flask",
    "/codingbrain_": "code",
    "/newimage_": "image",
    "/editimage_": "pencil",
    "/workspace_": "folder",
    "/newcompute_": "cpu",
    "/selectcompute_": "reuse",
    "/compute_<name>_": "zap",
    "/web_": "globe",
    "/frontdesign_": "layout",
    "/google_agenda_": "calendar",
    "/brain_": "brain",
    "/newmodule_": "modules",
    "/rust_port_adapter_": "plug",
    "/rust_state_store_": "database"
  };
  return <Glyph kind={stroke[command] ?? "terminal"} />;
}

const BRAIN_SPACES: { id: BrainSpace; label: string; glyph: string }[] = [
  { id: "memory", label: "Memory", glyph: "database" },
  { id: "codeacts", label: "CodeActs", glyph: "codeact" },
  { id: "godel", label: "Godel", glyph: "shield-check" },
  { id: "personality", label: "Personality", glyph: "masks" }
];

/* Segmented brain: the general brain is the default; the science and coding
   brains own the CodeActs specialized for their domain. The activator
   commands live in the general brain since they are the switches. */
const BRAIN_ACTIVATOR_COMMANDS: BrainCodeActCommand[] = ["/sciencebrain_", "/codingbrain_"];
const GOOGLE_SUITE_COMMANDS: BrainCodeActCommand[] = ["/googleweb_", "/gmail_", "/google_agenda_"];

const SCIENCE_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/newcompute_",
  "/selectcompute_",
  "/compute_<name>_",
  "/newobject_"
];

const CODING_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/workspace_",
  "/newmodule_",
  "/rust_port_adapter_",
  "/rust_state_store_"
];

const BRAIN_SEGMENTS: { id: string; label: string; glyph: string; commands?: BrainCodeActCommand[] }[] = [
  { id: "general", label: "general brain", glyph: "brain" },
  { id: "science", label: "science brain", glyph: "flask", commands: SCIENCE_BRAIN_COMMANDS },
  { id: "coding", label: "coding brain", glyph: "code", commands: CODING_BRAIN_COMMANDS }
];

type BrainCodeActDisplay = { command: BrainCodeActCommand; description: string };

const HIDDEN_BRAIN_CODEACT_COMMANDS = new Set<BrainCodeActCommand>(["/gmail_com"]);

const BRAIN_CODEACT_UI_DESCRIPTIONS: Partial<Record<BrainCodeActCommand, string>> = {
  "/sciencebrain_": "Switch to science mode for math, engineering, simulation, 3D, or technical analysis.",
  "/codingbrain_": "Switch to coding mode for software projects, files, bugs, builds, and developer tasks.",
  "/searcharchive_": "Search past chats and saved sessions when earlier context can help.",
  "/googleweb_": "Search the web for current public information.",
  "/gmail_": "Use Gmail to find messages, summarize email, or prepare replies.",
  "/airbnb_": "Use Airbnb to search for stays by place, dates, guests, and budget.",
  "/newimage_": "Create a new image from a text description.",
  "/editimage_": "Edit an existing image, such as changing its style, colors, objects, or layout.",
  "/google_agenda_": "Use Google Calendar for events, schedules, reminders, and dates.",
  "/brain_": "Save or update useful memory after the user confirms it is correct.",
  "/questionnaire_": "Ask a short set of questions when the task needs clearer choices.",
  "/newcompute_": "Start a new heavy local calculation, such as a simulation or numeric analysis.",
  "/selectcompute_": "Reuse a saved calculation instead of rebuilding the same work.",
  "/compute_<name>_": "Run a known saved calculation by name when it matches the request.",
  "/newobject_": "Create or modify a 3D object, scene, geometry, material, or design asset.",
  "/workspace_": "Ask the user to choose a local project folder before reading or changing files.",
  "/frontdesign_": "Change the app display colors or color palettes when the user asks.",
  "/newmodule_": "Create a small new app module or feature area.",
  "/rust_port_adapter_": "Add a Rust service bridge when a feature needs native backend access.",
  "/rust_state_store_": "Create durable local storage for settings, indexes, credentials, or cached data.",
  "/web_": "Open or control a web page inside the contained browser."
};

const NEW_COMPUTE_DETAIL_SECTIONS = [
  {
    label: "Measured token savings",
    text: "Local Monster GPU test: about 32,821 tokens saved on one fully slotted Li-ion electrochemical thermal safety compute, compared with carrying the full contract, Forge source, artifact, and proof context in LLM text."
  },
  {
    label: "New Compute templates",
    text: "Formula symbolic, numeric model, simulation dynamics, optimization design, uncertainty statistics, tensor/linalg/autodiff, signal/time-series, and graph/sparse/discrete."
  },
  {
    label: "What that means",
    text: "Use the matching template to run symbolic math, numeric engineering models, dynamic simulations, design optimization, uncertainty estimates, tensor or gradient work, signal analysis, or graph/discrete compute."
  },
  {
    label: "Use the results for",
    text: "Feed compact proof-backed results into 3D objects, simulation scenes, biology/DNA exercises, crypto exercises, trading models, real-estate scoring, logistics plans, or research reports."
  }
] as const;
const BRAIN_SESSION_ARCHIVE_INITIAL_COUNT = 6;
const BRAIN_SESSION_ARCHIVE_STEP = 8;

function codeActDisplay(command: BrainCodeActCommand, fallbackDescription = ""): BrainCodeActDisplay {
  return {
    command,
    description: BRAIN_CODEACT_UI_DESCRIPTIONS[command] ?? fallbackDescription
  };
}

function segmentCodeActs(segment: { commands?: BrainCodeActCommand[] }) {
  const elsewhere = new Set([...SCIENCE_BRAIN_COMMANDS, ...CODING_BRAIN_COMMANDS, ...BRAIN_ACTIVATOR_COMMANDS, ...GOOGLE_SUITE_COMMANDS]);
  return BRAIN_CODEACT_COMMAND_DESCRIPTIONS.filter(({ command }) =>
    !HIDDEN_BRAIN_CODEACT_COMMANDS.has(command) && (segment.commands ? segment.commands.includes(command) : !elsewhere.has(command))
  ).map(({ command, description }) => codeActDisplay(command, description));
}

function activatorCodeActs() {
  return BRAIN_ACTIVATOR_COMMANDS.map((command) => codeActDisplay(command));
}

function googleSuiteCodeActs() {
  return GOOGLE_SUITE_COMMANDS.map((command) => codeActDisplay(command));
}

function isRestorableBrainSession(item: SidebarSessionItem): boolean {
  return item.sessionId.startsWith("chat-") || item.sessionId.startsWith("parallel-chat-");
}

function brainSessionArchiveItems(recentItems: SidebarSessionItem[], archivedItems: SidebarSessionItem[]): SidebarSessionItem[] {
  const seen = new Set<string>();
  return [...recentItems, ...archivedItems].filter((item) => {
    const label = item.label.trim();
    if (!label || !isRestorableBrainSession(item)) return false;
    const key = item.sessionId || `${label}:${item.date}:${item.workspaceLabel}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return item.rowVisible || item.archived || item.pinned || item.working;
  });
}

function BrainSessionArchiveList({
  sessions,
  visibleCount,
  onShowMore,
  onOpenSession
}: {
  sessions: SidebarSessionItem[];
  visibleCount: number;
  onShowMore: () => void;
  onOpenSession: (session: SidebarSessionItem) => void;
}) {
  if (sessions.length === 0) {
    return (
      <div className="brainSessionArchiveList brainSessionArchiveList--empty" aria-label="Saved sessions">
        No saved sessions yet.
      </div>
    );
  }
  const visibleSessions = sessions.slice(0, visibleCount);
  const hiddenCount = Math.max(0, sessions.length - visibleSessions.length);
  return (
    <>
      <div className="brainSessionArchiveList" role="list" aria-label="Saved sessions">
        {visibleSessions.map((session) => (
          <button
            type="button"
            className="brainSessionArchiveItem"
            role="listitem"
            key={session.sessionId || `${session.label}-${session.date}`}
            onClick={() => onOpenSession(session)}
          >
            <span className="brainSessionArchiveItem__line">
              <span className="brainSessionArchiveItem__title">{session.label}</span>
              <span className="brainSessionArchiveItem__date">{session.date}</span>
            </span>
            <span className="brainSessionArchiveItem__meta">
              <span>{session.workspaceLabel || session.section}</span>
              {session.archived ? <span>Archived</span> : null}
            </span>
          </button>
        ))}
      </div>
      {hiddenCount > 0 ? (
        <button type="button" className="brainSessionArchiveMore" onClick={onShowMore}>
          Afficher plus
          <span>{hiddenCount}</span>
        </button>
      ) : null}
    </>
  );
}

function SlotRow({
  glyph,
  icon,
  title,
  text,
  status,
  active = false
}: {
  glyph?: string;
  icon?: React.ReactNode;
  title: string;
  text: string;
  status?: string;
  active?: boolean;
}) {
  return (
    <div className="brainSlotRow" role="listitem">
      <span className="brainRow__icon">{icon ?? <Glyph kind={glyph ?? "terminal"} size={17} />}</span>
      <span className="brainSlotRow__body">
        <strong>{title}</strong>
        <span>{text}</span>
      </span>
      {status ? (
        <span className={active ? "brainStatus brainStatus--active" : "brainStatus"}>
          <i aria-hidden="true" />
          {status}
        </span>
      ) : null}
    </div>
  );
}

function CodeActRow({ command, description }: { command: BrainCodeActCommand; description: string }) {
  const canExpand = command === "/newcompute_";
  const [expanded, setExpanded] = useState(false);
  const detailsId = "brain-new-compute-details";
  return (
    <div className={canExpand ? "brainRow brainRow--expandable" : "brainRow"} role="listitem">
      <span className="brainRow__icon">
        <CodeActIcon command={command} />
      </span>
      <span className="brainRow__commandLine">
        <code>{command}</code>
        {canExpand ? (
          <button
            type="button"
            className="brainRow__expandButton"
            aria-expanded={expanded}
            aria-controls={detailsId}
            aria-label={expanded ? "Hide Codex New Compute details" : "Show Codex New Compute details"}
            onClick={() => setExpanded((isExpanded) => !isExpanded)}
          >
            <Glyph kind={expanded ? "minus" : "plus"} size={14} />
          </button>
        ) : null}
      </span>
      <p>{description}</p>
      {canExpand && expanded ? (
        <div className="brainComputeDetails" id={detailsId} role="region" aria-label="Codex New Compute capabilities">
          <strong>Codex New Compute</strong>
          <div className="brainComputeDetails__grid">
            {NEW_COMPUTE_DETAIL_SECTIONS.map((section) => (
              <span className="brainComputeDetails__item" key={section.label}>
                <b>{section.label}</b>
                <span>{section.text}</span>
              </span>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function BrainMemoryIdentityField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="brainMemoryIdentityField">
      <span className="brainMemoryIdentityField__body">
        <span className="brainMemoryIdentityField__label">{label}</span>
        <span className="brainMemoryIdentityField__control">
          <input
            aria-label={label}
          className="brainMemoryIdentityField__input"
          type="text"
          value={value}
          size={Math.max(10, Math.min(value.length || 10, 24))}
          placeholder="Write here"
          spellCheck={false}
          onChange={(event) => onChange(event.currentTarget.value)}
          />
          <span className="brainMemoryIdentityField__edit" aria-hidden="true">
            <Glyph kind="pencil" size={10} />
          </span>
        </span>
      </span>
    </label>
  );
}

function CodeActsSpace() {
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        CodeActs are autonomous commands the agent runs to move faster and do real work beyond chat.
        Some control a web browser in a contained, controlled environment; others create 3D objects or run heavy science and analysis locally, replacing work that would otherwise burn hundreds of millions of tokens.
      </p>
      <div className="brainCanvas__segments">
        {BRAIN_SEGMENTS.map((segment) => (
          <section className="brainSegment" key={segment.id} aria-label={segment.label}>
            <h2 className="brainSegment__head">
              <Glyph kind={segment.glyph} size={14} />
              {segment.label}
            </h2>
            {segment.id === "general" ? (
              <>
                <div className="brainActivators" role="list" aria-label="brain activators">
                  {activatorCodeActs().map(({ command, description }) => (
                    <CodeActRow command={command} description={description} key={command} />
                  ))}
                </div>
                <div className="brainGoogleSuite" role="list" aria-label="Google Suite">
                  <p className="brainCommandPack__label">Google Suite</p>
                  {googleSuiteCodeActs().map(({ command, description }) => (
                    <CodeActRow command={command} description={description} key={command} />
                  ))}
                </div>
              </>
            ) : null}
            <div className="brainCanvas__rows" role="list">
              {segmentCodeActs(segment).map(({ command, description }) => (
                <CodeActRow command={command} description={description} key={command} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

function MemorySpace() {
  const [userMemory, setUserMemory] = useState(() => readBrainUserMemory());
  const [agentMemory, setAgentMemory] = useState(() => readBrainAgentMemory());
  const [visibleArchiveCount, setVisibleArchiveCount] = useState(BRAIN_SESSION_ARCHIVE_INITIAL_COUNT);
  const { snapshot: sidebarSnapshot } = useSidebarShadowStore();
  const sessions = brainSessionArchiveItems(sidebarSnapshot.recentItems, sidebarSnapshot.archivedItems);

  useEffect(() => {
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: userMemory.preferredFirstName,
      agentFirstName: agentMemory.preferredFirstName
    });
  }, [agentMemory.preferredFirstName, userMemory.preferredFirstName]);

  useEffect(() => {
    setVisibleArchiveCount((current) => Math.min(Math.max(current, BRAIN_SESSION_ARCHIVE_INITIAL_COUNT), Math.max(sessions.length, BRAIN_SESSION_ARCHIVE_INITIAL_COUNT)));
  }, [sessions.length]);

  const commitUserMemory = (value: string) => {
    const next = writeBrainUserMemory(value);
    setUserMemory(next);
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: next.preferredFirstName,
      agentFirstName: agentMemory.preferredFirstName
    });
  };

  const commitAgentMemory = (value: string) => {
    const next = writeBrainAgentMemory(value);
    setAgentMemory(next);
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: userMemory.preferredFirstName,
      agentFirstName: next.preferredFirstName
    });
  };

  const openArchivedSession = async (session: SidebarSessionItem) => {
    if (session.sessionId) {
      await sidebarShadowStore.dispatch(
        sidebarShadowStore.command({ kind: "open_session", sessionId: session.sessionId, section: session.section }),
        session.sessionId
      );
      await panelsChatBottomStore.refresh();
      await headerShadowStore.boot();
      return;
    }
    await sidebarShadowStore.dispatch(
      sidebarShadowStore.command({ kind: "navigate", section: session.section }),
      session.label
    );
    await headerShadowStore.boot();
  };

  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        Memory keeps the names and session history the agent can reuse when it helps the current conversation.
      </p>
      <div className="brainCanvas__rows" role="list">
        <section className="brainMemoryIdentity" aria-label="Visitor and agent names">
          <p className="brainMemoryIdentity__label">
            <Glyph kind="identity-card" size={18} />
            <span>Identity</span>
          </p>
          <div className="brainMemoryIdentity__fields">
            <BrainMemoryIdentityField
              label="Your name"
              value={userMemory.preferredFirstName}
              onChange={commitUserMemory}
            />
            <BrainMemoryIdentityField
              label="Agent name"
              value={agentMemory.preferredFirstName}
              onChange={commitAgentMemory}
            />
          </div>
        </section>
        <div className="brainSessionArchiveHead" role="listitem">
          <span className="brainRow__icon">
            <Glyph kind="archive" size={17} />
          </span>
          <span className="brainSessionArchiveHead__body">
            <strong>Session archive</strong>
            <span>Saved conversations, decisions, and working context the agent can recall when useful.</span>
          </span>
        </div>
        <BrainSessionArchiveList
          sessions={sessions}
          visibleCount={visibleArchiveCount}
          onShowMore={() => setVisibleArchiveCount((current) => Math.min(sessions.length, current + BRAIN_SESSION_ARCHIVE_STEP))}
          onOpenSession={openArchivedSession}
        />
      </div>
    </div>
  );
}

function GodelSpace() {
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        Godel is the verification machine between intent and execution.
      </p>
      <p className="brainCanvas__pipeline">
        BrainCommand <i>-&gt;</i> Godel <i>-&gt;</i> Forge bytecode <i>-&gt;</i> Monster <i>-&gt;</i> proof
      </p>
      <div className="brainCanvas__rows" role="list">
        <SlotRow
          glyph="shield-check"
          title="Semantic verification"
          text="Every CodeAct command is checked against its typed contract before bytecode is emitted."
          status="active"
          active
        />
        <SlotRow
          glyph="terminal"
          title="Proof hashes"
          text="Monster compute returns verifiable artifacts with content-addressed proofs, not generated answers."
          status="active"
          active
        />
      </div>
    </div>
  );
}

function PersonalitySpace() {
  const memory = readBrainUserMemory();
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        How the agent addresses you, and how far it is allowed to act.
      </p>
      <div className="brainCanvas__rows" role="list">
        <SlotRow
          glyph="masks"
          title={memory.preferredFirstName.trim() || "No name set"}
          text="Preferred first name, used across welcome messages and session prose."
          status={memory.trust.replaceAll("_", " ")}
          active
        />
        <SlotRow
          glyph="pencil"
          title="Tone"
          text="Compact, technical, proof-first. Custom tone profiles land here."
          status="soon"
        />
        <SlotRow
          glyph="shield-check"
          title="Autonomy"
          text="Side-effect actions — send, pay, delete — always stay user-confirmed."
          status="soon"
        />
      </div>
    </div>
  );
}

function initBrainBlobWebGpu(canvas: HTMLCanvasElement, onFirstFrame?: () => void): Promise<BrainBlobHandle | null> {
  const gpu = (navigator as WebGpuHostNavigator).gpu;
  if (!gpu) return Promise.resolve(null);

  return gpu.requestAdapter({ powerPreference: "high-performance" }).then(async (adapter) => {
    if (!adapter) return null;
    const device = await adapter.requestDevice();
    const context = canvas.getContext("webgpu") as WebGpuCanvasContext | null;
    if (!context) {
      device.destroy?.();
      return null;
    }

    const format = gpu.getPreferredCanvasFormat();
    const uniformData = new Float32Array(8);
    uniformData[4] = Math.random() * 1000;
    const uniformBuffer = device.createBuffer({
      label: "brain-blob-uniforms",
      size: uniformData.byteLength,
      usage: WEBGPU_BUFFER_USAGE.UNIFORM | WEBGPU_BUFFER_USAGE.COPY_DST
    });
    const shader = device.createShaderModule({ label: "brain-blob-shader", code: BRAIN_BLOB_SHADER });
    const pipeline = device.createRenderPipeline({
      label: "brain-blob-pipeline",
      layout: "auto",
      vertex: { module: shader, entryPoint: "vertexMain" },
      fragment: {
        module: shader,
        entryPoint: "sceneMain",
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
      primitive: { topology: "triangle-list" }
    });
    const bindGroup = device.createBindGroup({
      label: "brain-blob-bind-group",
      layout: (pipeline as { getBindGroupLayout(index: number): unknown }).getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: uniformBuffer } }]
    });

    let configured = false;
    let stopped = false;
    let deviceLost = false;
    let firstFrameSubmitted = false;
    let rafId = 0;
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      const targetScale = dpr * BRAIN_BLOB_SUPERSAMPLE;
      const rawWidth = Math.max(1, rect.width * targetScale);
      const rawHeight = Math.max(1, rect.height * targetScale);
      const limitScale = Math.min(1, BRAIN_BLOB_MAX_FRAMEBUFFER_SIDE / Math.max(rawWidth, rawHeight));
      const width = Math.max(1, Math.round(rawWidth * limitScale));
      const height = Math.max(1, Math.round(rawHeight * limitScale));
      if (canvas.width === width && canvas.height === height && configured) return;
      canvas.width = width;
      canvas.height = height;
      context.configure({ device, format, alphaMode: "premultiplied" });
      configured = true;
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(canvas, { box: "device-pixel-content-box" });
    resize();

    void device.lost.then((info) => {
      deviceLost = true;
      if (!stopped) console.warn("Brain blob WebGPU device lost.", info);
    });

    const startedAt = performance.now();
    const tick = () => {
      if (stopped) return;
      if (!configured || deviceLost || document.hidden) {
        rafId = window.requestAnimationFrame(tick);
        return;
      }
      const now = performance.now();
      uniformData[0] = canvas.width;
      uniformData[1] = canvas.height;
      uniformData[2] = (now - startedAt) / 1000;
      uniformData[3] = reducedMotion ? 1 : 0;
      device.queue.writeBuffer(uniformBuffer, 0, uniformData);

      const encoder = device.createCommandEncoder({ label: "brain-blob-frame" });
      const pass = encoder.beginRenderPass({
        label: "brain-blob-render-pass",
        colorAttachments: [
          {
            view: context.getCurrentTexture().createView(),
            clearValue: { r: 0, g: 0, b: 0, a: 0 },
            loadOp: "clear",
            storeOp: "store"
          }
        ]
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.draw(3, 1);
      pass.end();
      device.queue.submit([encoder.finish()]);
      if (!firstFrameSubmitted) {
        firstFrameSubmitted = true;
        onFirstFrame?.();
      }
      rafId = window.requestAnimationFrame(tick);
    };
    rafId = window.requestAnimationFrame(tick);

    return {
      destroy() {
        stopped = true;
        if (rafId) window.cancelAnimationFrame(rafId);
        resizeObserver.disconnect();
        context.unconfigure?.();
        uniformBuffer.destroy();
        device.destroy?.();
      }
    };
  });
}

function BrainBlob() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [webGpuReady, setWebGpuReady] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;
    let cancelled = false;
    let handle: BrainBlobHandle | null = null;

    void initBrainBlobWebGpu(canvas, () => {
      if (!cancelled) setWebGpuReady(true);
    })
      .then((nextHandle) => {
        if (cancelled) {
          nextHandle?.destroy();
          return;
        }
        handle = nextHandle;
        if (!nextHandle) setWebGpuReady(false);
      })
      .catch((error) => {
        if (!cancelled) {
          console.warn("Brain lava lamp WebGPU renderer unavailable.", error);
          setWebGpuReady(false);
        }
      });

    return () => {
      cancelled = true;
      handle?.destroy();
    };
  }, []);

  return (
    <div className={webGpuReady ? "brainBlob brainBlob--webgpu" : "brainBlob"} aria-hidden="true">
      <canvas ref={canvasRef} className="brainBlob__canvas" />
      <div className="brainBlob__fallback" />
    </div>
  );
}

export function BrainCanvas({ onClose }: { onClose?: () => void }) {
  const [space, setSpace] = useState<BrainSpace>("memory");
  return (
    <section className="profileCanvas brainCanvas" aria-label="Brain canvas">
      <BrainBlob />
      <header className="brainCanvas__head">
        <button type="button" className="brainCanvas__close" aria-label="Close Brain" title="Close Brain" onClick={onClose}>
          <span aria-hidden="true" />
        </button>
        <span className="brainCanvas__mark"><Glyph kind="brain" size={26} /></span>
        <h1>Brain</h1>
      </header>
      <div className="brainCanvas__tabs" role="tablist" aria-label="Brain spaces">
        {BRAIN_SPACES.map(({ id, label, glyph }) => (
          <button
            type="button"
            role="tab"
            aria-selected={space === id}
            key={id}
            onClick={() => setSpace(id)}
          >
            <Glyph kind={glyph} size={20} />
            {label}
          </button>
        ))}
      </div>
      {space === "codeacts" ? <CodeActsSpace /> : null}
      {space === "memory" ? <MemorySpace /> : null}
      {space === "godel" ? <GodelSpace /> : null}
      {space === "personality" ? <PersonalitySpace /> : null}
    </section>
  );
}
