import { useEffect, useRef, useState } from "react";
import { createBrainBlobMonsterFrameCache, getBrainBlobMonsterRuntimeMode } from "./brain-blob-cache";

/* Brain page background: a soft 3D gooey blob raymarched as an SDF metaball
   scene, ported from the Uiverse "andrew-manzyk" loader. The seven blurred
   polygons became seven orbiting metaballs plus three droplets that
   periodically tear off and get caught back by a stretching arm. Ball motion
   is integrated on the CPU each frame and uploaded as uniforms, so one scene
   drives both backends: WebGPU raymarching first, WebGL2 raymarching when
   WebGPU is unavailable, CSS fallback when both fail. Everything is seeded
   per session, so the blob is procedural and never the same twice. */

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
const BRAIN_BLOB_MAX_FRAMEBUFFER_SIDE = 3000;
const BRAIN_BLOB_SHADER_CONTRACT = "forge.kasm.brain_blob.sdf_metaball_raymarch.v1";
/* Global animation pacing: < 1 slows every motion (orbits, droplets, wobble,
   colorize) uniformly, since both backends derive everything from this t. */
const BRAIN_BLOB_TIME_SCALE = 0.6;

/* Uniform layout shared by both backends: header vec4 (resolution.xy, time,
   seed), then 10 balls (xyz, radius), then 10 per-ball smooth-min k, then the
   pointer vec4 (worldX, worldY, strength, _). */
const BLOB_BALL_COUNT = 10;
const BLOB_MASS_COUNT = 7;
const BLOB_UNIFORM_FLOATS = 88;
const BLOB_BALLS_FLOAT_OFFSET = 4;
const BLOB_KS_FLOAT_OFFSET = 44;
const BLOB_MOUSE_FLOAT_OFFSET = 84;

/* Camera the shaders ray-march with; mirrored on the CPU to project the cursor
   onto the blob's z = 0 plane. Keep in sync with the shader `ro`/focal. */
const BLOB_FOCAL = 1.72;
const BLOB_CAM_Z = 2.28;
const BLOB_CAM_Y = 0.03;
const BLOB_POINTER_REACH = 0.78;

/* Live cursor shared between the React component and the render loop. */
type BlobPointer = { x: number; y: number; over: boolean };

const BLOB_TAU = Math.PI * 2;
/* Loader contract: --time-animation 2s; orbits run at 45% for the soft feel. */
const BLOB_TIME_ANIM = 2;
const BLOB_MASS_PIVOTS: ReadonlyArray<readonly [number, number, number]> = [
  [0.2, 0.2, 0.0],
  [0.0, 0.0, 0.0],
  [0.0, -0.09, 0.07],
  [-0.09, 0.09, -0.07],
  [-0.09, 0.09, 0.07],
  [0.09, 0.09, -0.05],
  [0.09, 0.09, 0.05]
];
const BLOB_MASS_DIRS = [0, -1, 1, -1, -1, 1, 1] as const;
const BLOB_MASS_DELAYS = [0, 0, 1 / 3, 0, 1 / 2, 0, 1 / 1.5] as const;

type BlobScene = { timeOffset: number; update(t: number, out: Float32Array, pointer: BlobPointer | null): void };

function createBlobScene(seed: number): BlobScene {
  const rand = (n: number) => {
    const x = Math.sin(n * 12.9898 + seed * 78.233) * 43758.5453;
    return x - Math.floor(x);
  };
  const mix = (a: number, b: number, m: number) => a + (b - a) * m;
  const sstep = (e0: number, e1: number, x: number) => {
    const u = Math.min(Math.max((x - e0) / (e1 - e0), 0), 1);
    return u * u * (3 - 2 * u);
  };
  const norm3 = (x: number, y: number, z: number): [number, number, number] => {
    const l = Math.hypot(x, y, z) || 1;
    return [x / l, y / l, z / l];
  };
  /* Per-ball tilted orbit plane (u, v orthonormal-ish basis). */
  const orbitPlane = (n: number): [[number, number, number], [number, number, number]] => {
    const az = rand(n) * BLOB_TAU;
    const el = (rand(n + 17.7) - 0.5) * 1.2;
    const u = norm3(Math.cos(az) * Math.cos(el), Math.sin(el), Math.sin(az) * Math.cos(el));
    const v = norm3(
      u[1] * 0.15 - u[2] * 1.0,
      u[2] * 0.25 - u[0] * 0.15,
      u[0] * 1.0 - u[1] * 0.25
    );
    return [u, v];
  };

  const mass = BLOB_MASS_PIVOTS.map((p, i) => {
    const [u, v] = orbitPlane(i * 5.13 + 2.21);
    return {
      pivot: [
        p[0] + (rand(i * 9.13 + 2.9) - 0.5) * 0.12,
        p[1] + (rand(i * 6.71 + 8.1) - 0.5) * 0.12,
        p[2] + (rand(i * 3.39 + 5.7) - 0.5) * 0.12
      ] as const,
      u,
      v,
      speed: BLOB_MASS_DIRS[i] * (BLOB_TAU / BLOB_TIME_ANIM) * 0.45 * mix(0.72, 1.28, rand(i * 7.31 + 1.7)),
      phase: BLOB_MASS_DELAYS[i] * BLOB_TAU + rand(i * 3.97 + 9.2) * BLOB_TAU,
      orbit: mix(0.06, 0.15, rand(i * 5.53 + 4.4)),
      radius: mix(0.21, 0.3, rand(i * 2.17 + 6.6)),
      breathW: mix(0.5, 1.1, rand(i * 4.41 + 3.3)),
      bobW: mix(0.4, 0.9, rand(i * 8.21 + 5.5))
    };
  });

  const droplets = [0, 1, 2].map((j) => {
    const fj = j + 11;
    const [u, v] = orbitPlane(fj * 6.13 + 12.7);
    return {
      u,
      v,
      cycle: mix(12, 20, rand(fj * 6.13 + 12.7)),
      cyclePhase: rand(fj * 4.79 + 3.9),
      angle0: rand(fj * 8.7 + 1.3) * BLOB_TAU,
      drift: mix(0.25, 0.55, rand(fj * 2.9 + 7.1)) * (j % 2 === 0 ? -1 : 1),
      radius: mix(0.105, 0.135, rand(fj * 3.31 + 5.2))
    };
  });

  /* CSS roundness keyframes (contrast 15 -> 3 -> 15) slowed to a 4s squish. */
  const roundness = (t: number) => {
    const phase = (t / (BLOB_TIME_ANIM * 2)) % 1;
    if (phase < 0.2) return sstep(0, 0.2, phase);
    if (phase < 0.4) return 1;
    if (phase < 0.6) return 1 - sstep(0.4, 0.6, phase);
    return 0;
  };

  /* Hover-triggered fling: when the cursor first enters the blob a seeded roll
     decides whether one droplet tears off from the cursor and flicks away. */
  let prevOver = false;
  let flingCooldownUntil = -1;
  let fling: null | { drop: number; start: number; dur: number; dirX: number; dirY: number; startR: number } = null;

  return {
    timeOffset: rand(3.3) * 31.7,
    update(t, out, pointer) {
      const over = !!pointer?.over;
      if (over && !prevOver && t > flingCooldownUntil) {
        /* Procedural chance and target picked from the seed + entry moment. */
        const moment = Math.floor(t * 6.0);
        if (rand(moment * 1.7 + 0.3) < 0.5) {
          const startR = Math.hypot(pointer!.x, pointer!.y) || 0.0001;
          fling = {
            drop: Math.floor(rand(moment * 3.1 + 5.9) * 3) % 3,
            start: t,
            dur: mix(1.7, 2.6, rand(moment * 2.3 + 8.4)),
            dirX: pointer!.x / startR,
            dirY: pointer!.y / startR,
            startR
          };
          flingCooldownUntil = t + 3.5;
        }
      }
      prevOver = over;
      const kBase = mix(0.18, 0.3, roundness(t));
      mass.forEach((ball, i) => {
        const a = ball.phase + t * ball.speed;
        const breath = 1 + Math.sin(t * ball.breathW + ball.phase * 2) * 0.14;
        const cosA = Math.cos(a) * ball.orbit;
        const sinA = Math.sin(a) * ball.orbit;
        const o = BLOB_BALLS_FLOAT_OFFSET + i * 4;
        out[o] = ball.pivot[0] + ball.u[0] * cosA + ball.v[0] * sinA;
        out[o + 1] = ball.pivot[1] + ball.u[1] * cosA + ball.v[1] * sinA + Math.sin(t * ball.bobW + ball.phase) * 0.04;
        out[o + 2] = ball.pivot[2] + ball.u[2] * cosA + ball.v[2] * sinA;
        out[o + 3] = ball.radius * breath;
        out[BLOB_KS_FLOAT_OFFSET + i * 4] = kBase;
      });
      droplets.forEach((droplet, j) => {
        const cph = (t / droplet.cycle + droplet.cyclePhase) % 1;
        /* Rest -> tear away -> free flight -> get caught and pulled back. */
        let ext = 0;
        if (cph >= 0.4 && cph < 0.58) ext = sstep(0.4, 0.58, cph);
        else if (cph >= 0.58 && cph < 0.78) ext = 1;
        else if (cph >= 0.78) ext = 1 - sstep(0.78, 1, cph);
        const stretch = sstep(0.4, 0.48, cph) * (1 - sstep(0.55, 0.62, cph));
        const caught = sstep(0.78, 0.9, cph) * (1 - sstep(0.97, 1, cph));
        const a = droplet.angle0 + t * droplet.drift;
        /* Fly far enough to clear the mass surface so the piece can truly
           separate, not just bulge on the body. */
        const flight = mix(0.2, 0.72, ext);
        const cosA = Math.cos(a) * flight;
        const sinA = Math.sin(a) * flight;
        const i = BLOB_MASS_COUNT + j;
        const o = BLOB_BALLS_FLOAT_OFFSET + i * 4;
        out[o] = droplet.u[0] * cosA + droplet.v[0] * sinA;
        out[o + 1] = droplet.u[1] * cosA + droplet.v[1] * sinA;
        out[o + 2] = droplet.u[2] * cosA + droplet.v[2] * sinA;
        out[o + 3] = droplet.radius * (1 - ext * 0.2);
        /* Smooth-min radius profile high -> low -> high: merged into the body
           at rest, a thinning neck while it tears off, a near-zero k in free
           flight so the piece fully pinches away, then a neck reforming as an
           arm catches and re-fuses it. */
        const home = 1 - ext;
        const bridge = Math.max(stretch, caught);
        out[BLOB_KS_FLOAT_OFFSET + i * 4] = Math.max(0.02, kBase * home + 0.17 * bridge);

        /* Hover fling overrides this droplet: it launches from the cursor,
           pinches off (tiny k) and flicks outward while shrinking. */
        if (fling && fling.drop === j) {
          const fp = (t - fling.start) / fling.dur;
          if (fp >= 1) {
            fling = null;
          } else {
            const ease = 1 - (1 - fp) * (1 - fp);
            /* Cap the flight so the flung piece never crosses the canvas. */
            const r = mix(fling.startR, Math.min(0.8, Math.max(0.55, fling.startR + 0.3)), ease);
            out[o] = fling.dirX * r;
            out[o + 1] = fling.dirY * r;
            out[o + 2] = 0;
            out[o + 3] = droplet.radius * (1 - ease * 0.85);
            out[BLOB_KS_FLOAT_OFFSET + i * 4] = mix(0.16, 0.02, sstep(0, 0.3, fp));
          }
        }
      });
    }
  };
}

const BRAIN_BLOB_WGSL = /* wgsl */ `
const TAU: f32 = 6.28318530718;
/* Loader contract: --time-animation 2s; colorize runs at *3. */
const TIME_ANIM: f32 = 2.0;
const BOUND_RADIUS: f32 = 1.58;

struct Uniforms {
  resolution: vec2<f32>,
  time: f32,
  seed: f32,
  balls: array<vec4<f32>, 10>,
  ks: array<vec4<f32>, 10>,
  mouse: vec4<f32>,
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

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = saturate(0.5 + 0.5 * (b - a) / k);
  return mix(b, a, h) - k * h * (1.0 - h);
}

/* Gel surface wobble: cheap harmonic ripples instead of per-step noise. */
fn wobble(p: vec3<f32>, t: f32) -> f32 {
  return (
    sin(p.x * 5.1 + t * 0.9)
    + sin(p.y * 4.3 - t * 0.7)
    + sin(p.z * 4.7 + t * 0.8)
    + sin(dot(p, vec3<f32>(2.7, 3.1, 2.3)) - t * 0.5)
  ) * 0.011;
}

/* Soft membrane texture: low-frequency mottling across the skin only. Kept
   smooth on purpose so the texture itself reads as soft-focus (the blur the
   user wants on the membrane), while the silhouette stays crisp. */
fn membraneTex(p: vec3<f32>, t: f32) -> f32 {
  let a = sin(p.x * 3.1 + p.y * 2.3 + t * 0.18);
  let b = sin(p.y * 2.7 - p.z * 3.3 - t * 0.14);
  let c = sin(p.z * 2.9 + p.x * 2.1 + t * 0.11);
  let d = sin(dot(p, vec3<f32>(1.7, 2.1, 1.9)) + t * 0.12);
  return (a + b + c + d) * 0.25;
}

/* Cursor poke: a local swell of the membrane toward the pointer on the
   camera-facing side, with a faint ripple. mouse = (worldX, worldY, strength). */
fn mouseDeform(p: vec3<f32>, t: f32) -> f32 {
  let s = uniforms.mouse.z;
  if (s <= 0.001) {
    return 0.0;
  }
  let r = length(p.xy - uniforms.mouse.xy);
  let g = exp(-r * r / 0.045);
  let front = smoothstep(0.25, -0.25, p.z);
  return -s * front * (0.13 * g + 0.025 * g * sin(r * 26.0 - t * 5.0));
}

fn field(p: vec3<f32>, t: f32) -> f32 {
  var d = 1e5;
  for (var i = 0; i < 10; i = i + 1) {
    let b = uniforms.balls[i];
    d = smin(d, length(p - b.xyz) - b.w, max(uniforms.ks[i].x, 0.0001));
  }
  return d + wobble(p, t) + mouseDeform(p, t);
}

fn fieldNormal(p: vec3<f32>, t: f32) -> vec3<f32> {
  let e = 0.0045;
  return normalize(vec3<f32>(
    field(p + vec3<f32>(e, 0.0, 0.0), t) - field(p - vec3<f32>(e, 0.0, 0.0), t),
    field(p + vec3<f32>(0.0, e, 0.0), t) - field(p - vec3<f32>(0.0, e, 0.0), t),
    field(p + vec3<f32>(0.0, 0.0, e), t) - field(p - vec3<f32>(0.0, 0.0, e), t)
  ));
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

@fragment
fn sceneMain(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
  let res = max(uniforms.resolution, vec2<f32>(1.0));
  let shortSide = min(res.x, res.y);
  let uv = (position.xy - 0.5 * res) / shortSide;
  let upY = -uv.y;
  let t = uniforms.time;

  let colorOne = vec3<f32>(1.0, 0.749, 0.282);
  let colorTwo = vec3<f32>(0.745, 0.290, 0.114);

  let ro = vec3<f32>(0.0, 0.03, -2.28);
  let rd = normalize(vec3<f32>(uv.x, upY, 1.72));

  /* Bounding-sphere clip: rays that miss the blob volume skip the march. */
  let b = dot(-ro, rd);
  let h2 = dot(ro, ro) - b * b;
  var color = vec3<f32>(0.0);
  var alpha = 0.0;
  if (h2 < BOUND_RADIUS * BOUND_RADIUS) {
    let span = sqrt(BOUND_RADIUS * BOUND_RADIUS - h2);
    let tEnd = b + span;
    var tCur = max(b - span, 0.0);
    var closest = 1e5;
    var hit = false;
    var p = ro;
    for (var i = 0; i < 72; i = i + 1) {
      p = ro + rd * tCur;
      let d = field(p, t);
      closest = min(closest, d);
      if (d < 0.0016) {
        hit = true;
        break;
      }
      tCur += d * 0.85;
      if (tCur > tEnd) {
        break;
      }
    }
    if (hit) {
      let n = fieldNormal(p, t);
      /* linear-gradient(180deg, color-one 30%, color-two 70%) over height. */
      let base = mix(colorOne, colorTwo, smoothstep(0.55, -0.55, p.y));
      let l = normalize(vec3<f32>(-0.42, 0.75, -0.5));
      let wrapDiff = saturate((dot(n, l) + 0.65) / 1.65);
      let diff = wrapDiff * wrapDiff;
      let fres = pow(1.0 - saturate(dot(n, -rd)), 2.6);
      let spec = pow(saturate(dot(n, normalize(l - rd))), 30.0);
      /* Thickness probe: thin lobes and stretched arms glow translucent. */
      let innerD = field(p - n * 0.16, t);
      let thin = saturate(1.0 + innerD / 0.16);
      let sss = thin * thin;
      /* Light Lumen: a cheap ambient-occlusion probe gates a warm
         global-illumination fill, with a back light and a faint emissive
         core so the gel glows gently from within. */
      let ao = saturate(1.0 + field(p + n * 0.13, t) / 0.13);
      let giFill = mix(colorTwo, colorOne, 0.5) * ao * 0.18;
      let l2 = normalize(vec3<f32>(0.5, -0.2, -0.62));
      let backDiff = saturate(dot(n, l2)) * 0.16;
      let emissive = mix(colorTwo, vec3<f32>(1.0, 0.78, 0.42), sss) * (0.10 + sss * 0.34);
      /* Soft membrane texture modulating the skin tone only. */
      let tex = membraneTex(p, t);
      let membrane = base * (1.0 + tex * 0.10);
      color = membrane * (0.30 + 0.70 * diff)
        + giFill
        + base * backDiff
        + mix(colorOne, vec3<f32>(1.0, 0.90, 0.66), 0.6) * sss * 0.38
        + colorOne * fres * 0.34
        + emissive
        + vec3<f32>(1.0, 0.96, 0.88) * spec * 0.55;
      alpha = saturate(0.62 + diff * 0.20 - fres * 0.16 + sss * 0.10);
    } else {
      /* Tight contour aura only: the wide warm Lumen bounce now comes from the
         CSS drop-shadow, so the shader aura stays narrow and does not bridge
         the gap when a droplet pinches off. */
      color = mix(colorOne, colorTwo, saturate(0.5 - upY * 1.4));
      alpha = exp(-max(closest, 0.0) / 0.042) * 0.24;
    }
  }

  /* No viewing frame: the blob's own transparency bounds it, so it can fill
     the whole canvas without a cropped edge. */
  let rgb = max(hueRotate(color, colorizeAngle(t)), vec3<f32>(0.0));
  return vec4<f32>(min(rgb, vec3<f32>(1.0)), saturate(alpha));
}
`;

const BRAIN_BLOB_GLSL_VERTEX = `#version 300 es
void main() {
  vec2 pos = vec2(-1.0, -1.0);
  if (gl_VertexID == 1) { pos = vec2(3.0, -1.0); }
  if (gl_VertexID == 2) { pos = vec2(-1.0, 3.0); }
  gl_Position = vec4(pos, 0.0, 1.0);
}
`;

/* Same raymarcher as the WGSL shader; gl_FragCoord is bottom-up so upY does
   not flip. */
const BRAIN_BLOB_GLSL_FRAGMENT = `#version 300 es
precision highp float;

uniform vec4 uHeader;
uniform vec4 uBalls[10];
uniform vec4 uKs[10];
uniform vec4 uMouse;
out vec4 outColor;

const float TAU = 6.28318530718;
const float TIME_ANIM = 2.0;
const float BOUND_RADIUS = 1.58;

float sat(float v) { return clamp(v, 0.0, 1.0); }

float smin(float a, float b, float k) {
  float h = sat(0.5 + 0.5 * (b - a) / k);
  return mix(b, a, h) - k * h * (1.0 - h);
}

float wobble(vec3 p, float t) {
  return (
    sin(p.x * 5.1 + t * 0.9)
    + sin(p.y * 4.3 - t * 0.7)
    + sin(p.z * 4.7 + t * 0.8)
    + sin(dot(p, vec3(2.7, 3.1, 2.3)) - t * 0.5)
  ) * 0.011;
}

// Soft membrane texture: low-frequency mottling on the skin only, kept smooth
// so the texture reads as soft-focus while the silhouette stays crisp.
float membraneTex(vec3 p, float t) {
  float a = sin(p.x * 3.1 + p.y * 2.3 + t * 0.18);
  float b = sin(p.y * 2.7 - p.z * 3.3 - t * 0.14);
  float c = sin(p.z * 2.9 + p.x * 2.1 + t * 0.11);
  float d = sin(dot(p, vec3(1.7, 2.1, 1.9)) + t * 0.12);
  return (a + b + c + d) * 0.25;
}

// Cursor poke: a local swell of the membrane toward the pointer on the
// camera-facing side, with a faint ripple. uMouse = (worldX, worldY, strength).
float mouseDeform(vec3 p, float t) {
  float s = uMouse.z;
  if (s <= 0.001) { return 0.0; }
  float r = length(p.xy - uMouse.xy);
  float g = exp(-r * r / 0.045);
  float front = smoothstep(0.25, -0.25, p.z);
  return -s * front * (0.13 * g + 0.025 * g * sin(r * 26.0 - t * 5.0));
}

float field(vec3 p, float t) {
  float d = 1e5;
  for (int i = 0; i < 10; i++) {
    vec4 b = uBalls[i];
    d = smin(d, length(p - b.xyz) - b.w, max(uKs[i].x, 0.0001));
  }
  return d + wobble(p, t) + mouseDeform(p, t);
}

vec3 fieldNormal(vec3 p, float t) {
  float e = 0.0045;
  return normalize(vec3(
    field(p + vec3(e, 0.0, 0.0), t) - field(p - vec3(e, 0.0, 0.0), t),
    field(p + vec3(0.0, e, 0.0), t) - field(p - vec3(0.0, e, 0.0), t),
    field(p + vec3(0.0, 0.0, e), t) - field(p - vec3(0.0, 0.0, e), t)
  ));
}

vec3 hueRotate(vec3 color, float angle) {
  float c = cos(angle);
  float s = sin(angle);
  vec3 w = vec3(0.213, 0.715, 0.072);
  return vec3(
    dot(color, vec3(w.x + c * (1.0 - w.x) + s * (-w.x), w.y + c * (-w.y) + s * (-w.y), w.z + c * (-w.z) + s * (1.0 - w.z))),
    dot(color, vec3(w.x + c * (-w.x) + s * 0.143, w.y + c * (1.0 - w.y) + s * 0.140, w.z + c * (-w.z) + s * -0.283)),
    dot(color, vec3(w.x + c * (-w.x) + s * (-(1.0 - w.x)), w.y + c * (-w.y) + s * w.y, w.z + c * (1.0 - w.z) + s * w.z))
  );
}

float colorizeAngle(float time) {
  float phase = fract(time / (TIME_ANIM * 3.0));
  if (phase < 0.2) { return mix(0.0, -0.5235988, smoothstep(0.0, 0.2, phase)); }
  if (phase < 0.4) { return mix(-0.5235988, -1.0471976, smoothstep(0.2, 0.4, phase)); }
  if (phase < 0.6) { return mix(-1.0471976, -1.5707963, smoothstep(0.4, 0.6, phase)); }
  if (phase < 0.8) { return mix(-1.5707963, -0.7853982, smoothstep(0.6, 0.8, phase)); }
  return mix(-0.7853982, 0.0, smoothstep(0.8, 1.0, phase));
}

void main() {
  vec2 res = max(uHeader.xy, vec2(1.0));
  float shortSide = min(res.x, res.y);
  vec2 uv = (gl_FragCoord.xy - 0.5 * res) / shortSide;
  float upY = uv.y;
  float t = uHeader.z;

  vec3 colorOne = vec3(1.0, 0.749, 0.282);
  vec3 colorTwo = vec3(0.745, 0.290, 0.114);

  vec3 ro = vec3(0.0, 0.03, -2.28);
  vec3 rd = normalize(vec3(uv.x, upY, 1.72));

  float b = dot(-ro, rd);
  float h2 = dot(ro, ro) - b * b;
  vec3 color = vec3(0.0);
  float alpha = 0.0;
  if (h2 < BOUND_RADIUS * BOUND_RADIUS) {
    float span = sqrt(BOUND_RADIUS * BOUND_RADIUS - h2);
    float tEnd = b + span;
    float tCur = max(b - span, 0.0);
    float closest = 1e5;
    bool hit = false;
    vec3 p = ro;
    for (int i = 0; i < 72; i++) {
      p = ro + rd * tCur;
      float d = field(p, t);
      closest = min(closest, d);
      if (d < 0.0016) {
        hit = true;
        break;
      }
      tCur += d * 0.85;
      if (tCur > tEnd) {
        break;
      }
    }
    if (hit) {
      vec3 n = fieldNormal(p, t);
      vec3 base = mix(colorOne, colorTwo, smoothstep(0.55, -0.55, p.y));
      vec3 l = normalize(vec3(-0.42, 0.75, -0.5));
      float wrapDiff = sat((dot(n, l) + 0.65) / 1.65);
      float diff = wrapDiff * wrapDiff;
      float fres = pow(1.0 - sat(dot(n, -rd)), 2.6);
      float spec = pow(sat(dot(n, normalize(l - rd))), 30.0);
      float innerD = field(p - n * 0.16, t);
      float thin = sat(1.0 + innerD / 0.16);
      float sss = thin * thin;
      // Light Lumen: cheap AO probe gates a warm GI fill, with a back light and
      // a faint emissive core so the gel glows gently from within.
      float ao = sat(1.0 + field(p + n * 0.13, t) / 0.13);
      vec3 giFill = mix(colorTwo, colorOne, 0.5) * ao * 0.18;
      vec3 l2 = normalize(vec3(0.5, -0.2, -0.62));
      float backDiff = sat(dot(n, l2)) * 0.16;
      vec3 emissive = mix(colorTwo, vec3(1.0, 0.78, 0.42), sss) * (0.10 + sss * 0.34);
      // Soft membrane texture modulating the skin tone only.
      float tex = membraneTex(p, t);
      vec3 membrane = base * (1.0 + tex * 0.10);
      color = membrane * (0.30 + 0.70 * diff)
        + giFill
        + base * backDiff
        + mix(colorOne, vec3(1.0, 0.90, 0.66), 0.6) * sss * 0.38
        + colorOne * fres * 0.34
        + emissive
        + vec3(1.0, 0.96, 0.88) * spec * 0.55;
      alpha = sat(0.62 + diff * 0.20 - fres * 0.16 + sss * 0.10);
    } else {
      // Tight contour aura only; the wide warm bounce comes from CSS so this
      // narrow aura does not bridge a pinched-off droplet's gap.
      color = mix(colorOne, colorTwo, sat(0.5 - upY * 1.4));
      alpha = exp(-max(closest, 0.0) / 0.042) * 0.24;
    }
  }

  // No viewing frame: the blob's own transparency bounds it, so it can fill
  // the whole canvas without a cropped edge.
  vec3 rgb = max(hueRotate(color, colorizeAngle(t)), vec3(0.0));
  outColor = vec4(min(rgb, vec3(1.0)), sat(alpha));
}
`;

function fitBlobFramebuffer(canvas: HTMLCanvasElement): boolean {
  const rect = canvas.getBoundingClientRect();
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  const targetScale = dpr * BRAIN_BLOB_SUPERSAMPLE;
  const rawWidth = Math.max(1, rect.width * targetScale);
  const rawHeight = Math.max(1, rect.height * targetScale);
  const limitScale = Math.min(1, BRAIN_BLOB_MAX_FRAMEBUFFER_SIDE / Math.max(rawWidth, rawHeight));
  const width = Math.max(1, Math.round(rawWidth * limitScale));
  const height = Math.max(1, Math.round(rawHeight * limitScale));
  if (canvas.width === width && canvas.height === height) return false;
  canvas.width = width;
  canvas.height = height;
  return true;
}

type BlobFramePump = { destroy(): void };

function pumpBlobFrames(canvas: HTMLCanvasElement, resize: () => void, renderFrame: (timeSeconds: number) => void): BlobFramePump {
  let stopped = false;
  let rafId = 0;
  const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas, { box: "device-pixel-content-box" });
  resize();
  const startedAt = performance.now();
  const tick = () => {
    if (stopped) return;
    if (!document.hidden) {
      renderFrame(reducedMotion ? 12.5 : (performance.now() - startedAt) / 1000);
    }
    rafId = window.requestAnimationFrame(tick);
  };
  rafId = window.requestAnimationFrame(tick);
  return {
    destroy() {
      stopped = true;
      if (rafId) window.cancelAnimationFrame(rafId);
      resizeObserver.disconnect();
    }
  };
}

function initBrainBlobWebGpu(canvas: HTMLCanvasElement, pointer: BlobPointer, onFirstFrame?: () => void): Promise<BrainBlobHandle | null> {
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
    const seed = Math.random() * 1000;
    const scene = createBlobScene(seed);
    const uniformData = new Float32Array(BLOB_UNIFORM_FLOATS);
    uniformData[3] = seed;
    const uniformBuffer = device.createBuffer({
      label: "brain-blob-uniforms",
      size: uniformData.byteLength,
      usage: WEBGPU_BUFFER_USAGE.UNIFORM | WEBGPU_BUFFER_USAGE.COPY_DST
    });
    const shader = device.createShaderModule({ label: "brain-blob-shader", code: BRAIN_BLOB_WGSL });
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
    let deviceLost = false;
    let firstFrameSubmitted = false;
    let pointerStrength = 0;
    const frameCache = createBrainBlobMonsterFrameCache();
    void device.lost.then((info) => {
      deviceLost = true;
      console.warn("Brain blob WebGPU device lost.", info);
    });

    const resize = () => {
      if (fitBlobFramebuffer(canvas) || !configured) {
        context.configure({ device, format, alphaMode: "premultiplied" });
        configured = true;
      }
    };
    const pump = pumpBlobFrames(canvas, resize, (timeSeconds) => {
      if (!configured || deviceLost) return;
      const t = frameCache.quantizeTime(timeSeconds * BRAIN_BLOB_TIME_SCALE + scene.timeOffset);
      const nextPointerStrength = pointerStrength + ((pointer.over ? 1 : 0) - pointerStrength) * 0.16;
      const frameProbe = frameCache.probe({
        lane: "webgpu",
        shaderHash: `${BRAIN_BLOB_SHADER_CONTRACT}:wgsl`,
        canvasWidth: canvas.width,
        canvasHeight: canvas.height,
        timeSeconds: t,
        seed,
        pointerX: pointer.x,
        pointerY: pointer.y,
        pointerStrength: nextPointerStrength,
        pointerOver: pointer.over
      });
      if (getBrainBlobMonsterRuntimeMode() === "optimized" && frameProbe.reused) return;
      pointerStrength = nextPointerStrength;
      uniformData[0] = canvas.width;
      uniformData[1] = canvas.height;
      uniformData[2] = t;
      uniformData[BLOB_MOUSE_FLOAT_OFFSET] = pointer.x;
      uniformData[BLOB_MOUSE_FLOAT_OFFSET + 1] = pointer.y;
      uniformData[BLOB_MOUSE_FLOAT_OFFSET + 2] = pointerStrength;
      scene.update(t, uniformData, pointer);
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
    });

    return {
      destroy() {
        pump.destroy();
        context.unconfigure?.();
        uniformBuffer.destroy();
        device.destroy?.();
      }
    };
  });
}

function initBrainBlobWebGl(canvas: HTMLCanvasElement, pointer: BlobPointer, onFirstFrame?: () => void): BrainBlobHandle | null {
  const gl = canvas.getContext("webgl2", {
    alpha: true,
    antialias: false,
    premultipliedAlpha: true,
    powerPreference: "high-performance"
  }) as WebGL2RenderingContext | null;
  if (!gl) return null;

  const compile = (type: number, source: string) => {
    const shader = gl.createShader(type);
    if (!shader) return null;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.warn("Brain blob WebGL shader failed.", gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }
    return shader;
  };
  const vs = compile(gl.VERTEX_SHADER, BRAIN_BLOB_GLSL_VERTEX);
  const fs = compile(gl.FRAGMENT_SHADER, BRAIN_BLOB_GLSL_FRAGMENT);
  if (!vs || !fs) return null;
  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn("Brain blob WebGL program failed.", gl.getProgramInfoLog(program));
    gl.deleteProgram(program);
    return null;
  }
  const headerLoc = gl.getUniformLocation(program, "uHeader");
  const ballsLoc = gl.getUniformLocation(program, "uBalls");
  const ksLoc = gl.getUniformLocation(program, "uKs");
  const mouseLoc = gl.getUniformLocation(program, "uMouse");

  const seed = Math.random() * 1000;
  const scene = createBlobScene(seed);
  const uniformData = new Float32Array(BLOB_UNIFORM_FLOATS);
  uniformData[3] = seed;

  gl.enable(gl.BLEND);
  gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  let firstFrameSubmitted = false;
  let pointerStrength = 0;
  const frameCache = createBrainBlobMonsterFrameCache();
  const pump = pumpBlobFrames(canvas, () => fitBlobFramebuffer(canvas), (timeSeconds) => {
    if (gl.isContextLost()) return;
    const t = frameCache.quantizeTime(timeSeconds * BRAIN_BLOB_TIME_SCALE + scene.timeOffset);
    const nextPointerStrength = pointerStrength + ((pointer.over ? 1 : 0) - pointerStrength) * 0.16;
    const frameProbe = frameCache.probe({
      lane: "webgl2",
      shaderHash: `${BRAIN_BLOB_SHADER_CONTRACT}:glsl`,
      canvasWidth: canvas.width,
      canvasHeight: canvas.height,
      timeSeconds: t,
      seed,
      pointerX: pointer.x,
      pointerY: pointer.y,
      pointerStrength: nextPointerStrength,
      pointerOver: pointer.over
    });
    if (getBrainBlobMonsterRuntimeMode() === "optimized" && frameProbe.reused) return;
    pointerStrength = nextPointerStrength;
    uniformData[0] = canvas.width;
    uniformData[1] = canvas.height;
    uniformData[2] = t;
    uniformData[BLOB_MOUSE_FLOAT_OFFSET] = pointer.x;
    uniformData[BLOB_MOUSE_FLOAT_OFFSET + 1] = pointer.y;
    uniformData[BLOB_MOUSE_FLOAT_OFFSET + 2] = pointerStrength;
    scene.update(t, uniformData, pointer);

    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(program);
    gl.uniform4fv(headerLoc, uniformData.subarray(0, 4));
    gl.uniform4fv(ballsLoc, uniformData.subarray(BLOB_BALLS_FLOAT_OFFSET, BLOB_BALLS_FLOAT_OFFSET + BLOB_BALL_COUNT * 4));
    gl.uniform4fv(ksLoc, uniformData.subarray(BLOB_KS_FLOAT_OFFSET, BLOB_KS_FLOAT_OFFSET + BLOB_BALL_COUNT * 4));
    gl.uniform4fv(mouseLoc, uniformData.subarray(BLOB_MOUSE_FLOAT_OFFSET, BLOB_MOUSE_FLOAT_OFFSET + 4));
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    if (!firstFrameSubmitted) {
      firstFrameSubmitted = true;
      onFirstFrame?.();
    }
  });

  return {
    destroy() {
      pump.destroy();
      gl.deleteProgram(program);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    }
  };
}

export function BrainBlob() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gpuReady, setGpuReady] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;
    let cancelled = false;
    let handle: BrainBlobHandle | null = null;

    /* Container is pointer-events:none, so track the cursor on the window and
       project it onto the blob's z = 0 plane in world space. */
    const pointer: BlobPointer = { x: 0, y: 0, over: false };
    const onPointerMove = (event: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {
        pointer.over = false;
        return;
      }
      const cssShort = Math.min(rect.width, rect.height);
      const uvx = (event.clientX - rect.left - rect.width * 0.5) / cssShort;
      const uvYUp = -(event.clientY - rect.top - rect.height * 0.5) / cssShort;
      const wx = (BLOB_CAM_Z * uvx) / BLOB_FOCAL;
      const wy = BLOB_CAM_Y + (BLOB_CAM_Z * uvYUp) / BLOB_FOCAL;
      pointer.x = wx;
      pointer.y = wy;
      const inside =
        event.clientX >= rect.left && event.clientX <= rect.right &&
        event.clientY >= rect.top && event.clientY <= rect.bottom;
      pointer.over = inside && Math.hypot(wx, wy) < BLOB_POINTER_REACH;
    };
    const onPointerLeave = () => {
      pointer.over = false;
    };
    window.addEventListener("mousemove", onPointerMove, { passive: true });
    window.addEventListener("mouseout", onPointerLeave, { passive: true });

    const markReady = () => {
      if (!cancelled) setGpuReady(true);
    };
    const fallBackToWebGl = () => {
      if (cancelled) return;
      handle = initBrainBlobWebGl(canvas, pointer, markReady);
      if (!handle) setGpuReady(false);
    };

    void initBrainBlobWebGpu(canvas, pointer, markReady)
      .then((nextHandle) => {
        if (cancelled) {
          nextHandle?.destroy();
          return;
        }
        if (nextHandle) {
          handle = nextHandle;
          return;
        }
        fallBackToWebGl();
      })
      .catch((error) => {
        console.warn("Brain blob WebGPU renderer unavailable, trying WebGL.", error);
        fallBackToWebGl();
      });

    return () => {
      cancelled = true;
      window.removeEventListener("mousemove", onPointerMove);
      window.removeEventListener("mouseout", onPointerLeave);
      handle?.destroy();
    };
  }, []);

  return (
    <div className={gpuReady ? "brainBlob brainBlob--webgpu" : "brainBlob"} aria-hidden="true">
      <canvas ref={canvasRef} className="brainBlob__canvas" />
      <div className="brainBlob__fallback" />
    </div>
  );
}
