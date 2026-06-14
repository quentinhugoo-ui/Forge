export const BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA = "forge.monster.brain_blob.frame_cache.v2";
export const BRAIN_BLOB_MONSTER_FRAME_HZ = 60;
export const BRAIN_BLOB_MONSTER_HUE_ROW_FLOATS = 12;

export type BrainBlobMonsterLane = "webgpu" | "webgl2";

export interface BrainBlobMonsterFrameInput {
  lane: BrainBlobMonsterLane;
  shaderHash: string;
  canvasWidth: number;
  canvasHeight: number;
  timeSeconds: number;
  seed: number;
  pointerX: number;
  pointerY: number;
  pointerStrength: number;
  pointerOver: boolean;
}

export interface BrainBlobMonsterFrameProbe {
  schema: typeof BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA;
  address: string;
  reused: boolean;
  timeTick: number;
  stats: BrainBlobMonsterFrameCacheStats;
}

export interface BrainBlobMonsterFrameCacheStats {
  acceptedFrames: number;
  reusedFrames: number;
  uniqueFrames: number;
  lastAddress: string;
}

export type BrainBlobMonsterScissor = { x: number; y: number; width: number; height: number };

export interface BrainBlobMonsterScissorInput {
  canvasWidth: number;
  canvasHeight: number;
  sphereData: ArrayLike<number>;
  sphereOffset: number;
  sphereCount: number;
  cameraFocal: number;
  cameraY: number;
  cameraZ: number;
  viewCenterX: number;
  viewCenterY: number;
  paddingWorld: number;
  paddingPixels: number;
}

export interface BrainBlobMonsterUniformViews {
  header: Float32Array;
  balls: Float32Array;
  ks: Float32Array;
  mouse: Float32Array;
  hue: Float32Array;
}

type BrainBlobMonsterFrameCache = {
  quantizeTime(timeSeconds: number): number;
  probe(input: BrainBlobMonsterFrameInput): BrainBlobMonsterFrameProbe;
  stats(): BrainBlobMonsterFrameCacheStats;
};

function quantizeFinite(value: number, scale: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.round(value * scale);
}

function fnv1a64Hex(input: string): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  for (let index = 0; index < input.length; index += 1) {
    hash ^= BigInt(input.charCodeAt(index));
    hash = BigInt.asUintN(64, hash * prime);
  }
  return hash.toString(16).padStart(16, "0");
}

function smoothstep(edge0: number, edge1: number, value: number): number {
  const t = Math.max(0, Math.min(1, (value - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

function clampNumber(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

const BRAIN_BLOB_MONSTER_HUE_PERIOD_TICKS = BRAIN_BLOB_MONSTER_FRAME_HZ * 6;
const brainBlobMonsterHueRowsByTick = new Map<number, Float32Array>();
const brainBlobMonsterScissorByAddress = new Map<string, BrainBlobMonsterScissor>();
const brainBlobMonsterUniformViewsByBuffer = new WeakMap<Float32Array, BrainBlobMonsterUniformViews>();

export function brainBlobMonsterColorizeAngle(timeSeconds: number): number {
  const phase = ((timeSeconds / 6) % 1 + 1) % 1;
  if (phase < 0.2) return mix(0, -0.5235988, smoothstep(0, 0.2, phase));
  if (phase < 0.4) return mix(-0.5235988, -1.0471976, smoothstep(0.2, 0.4, phase));
  if (phase < 0.6) return mix(-1.0471976, -1.5707963, smoothstep(0.4, 0.6, phase));
  if (phase < 0.8) return mix(-1.5707963, -0.7853982, smoothstep(0.6, 0.8, phase));
  return mix(-0.7853982, 0, smoothstep(0.8, 1, phase));
}

function computeBrainBlobMonsterHueRows(timeSeconds: number): Float32Array {
  const angle = brainBlobMonsterColorizeAngle(timeSeconds);
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  const wx = 0.213;
  const wy = 0.715;
  const wz = 0.072;
  return new Float32Array([
    wx + c * (1 - wx) + s * -wx,
    wy + c * -wy + s * -wy,
    wz + c * -wz + s * (1 - wz),
    0,
    wx + c * -wx + s * 0.143,
    wy + c * (1 - wy) + s * 0.14,
    wz + c * -wz + s * -0.283,
    0,
    wx + c * -wx + s * -(1 - wx),
    wy + c * -wy + s * wy,
    wz + c * (1 - wz) + s * wz,
    0
  ]);
}

export function brainBlobMonsterHuePeriodAddress(timeSeconds: number): { address: string; tick: number } {
  const tick = ((quantizeFinite(timeSeconds, BRAIN_BLOB_MONSTER_FRAME_HZ) % BRAIN_BLOB_MONSTER_HUE_PERIOD_TICKS)
    + BRAIN_BLOB_MONSTER_HUE_PERIOD_TICKS) % BRAIN_BLOB_MONSTER_HUE_PERIOD_TICKS;
  return {
    address: `monster:brain-blob:hue:${tick.toString(16).padStart(3, "0")}`,
    tick
  };
}

export function writeBrainBlobMonsterHueRows(timeSeconds: number, out: Float32Array, offset: number): void {
  const { tick } = brainBlobMonsterHuePeriodAddress(timeSeconds);
  let rows = brainBlobMonsterHueRowsByTick.get(tick);
  if (!rows) {
    rows = computeBrainBlobMonsterHueRows(tick / BRAIN_BLOB_MONSTER_FRAME_HZ);
    brainBlobMonsterHueRowsByTick.set(tick, rows);
  }
  out.set(rows, offset);
}

export function brainBlobMonsterUniformViews(
  data: Float32Array,
  layout: {
    ballOffset: number;
    ballFloats: number;
    ksOffset: number;
    ksFloats: number;
    mouseOffset: number;
    mouseFloats: number;
    hueOffset: number;
    hueFloats: number;
  }
): BrainBlobMonsterUniformViews {
  const cached = brainBlobMonsterUniformViewsByBuffer.get(data);
  if (cached) return cached;
  const views = {
    header: data.subarray(0, 4),
    balls: data.subarray(layout.ballOffset, layout.ballOffset + layout.ballFloats),
    ks: data.subarray(layout.ksOffset, layout.ksOffset + layout.ksFloats),
    mouse: data.subarray(layout.mouseOffset, layout.mouseOffset + layout.mouseFloats),
    hue: data.subarray(layout.hueOffset, layout.hueOffset + layout.hueFloats)
  };
  brainBlobMonsterUniformViewsByBuffer.set(data, views);
  return views;
}

export function brainBlobMonsterScissorAddress(input: BrainBlobMonsterScissorInput): string {
  const payload = [
    "forge.monster.brain_blob.scissor.v1",
    Math.max(1, Math.round(input.canvasWidth)),
    Math.max(1, Math.round(input.canvasHeight)),
    quantizeFinite(input.cameraFocal, 4096),
    quantizeFinite(input.cameraY, 4096),
    quantizeFinite(input.cameraZ, 4096),
    quantizeFinite(input.viewCenterX, 4096),
    quantizeFinite(input.viewCenterY, 4096),
    quantizeFinite(input.paddingWorld, 4096),
    Math.max(0, Math.round(input.paddingPixels)),
    input.sphereCount
  ];
  for (let index = 0; index < input.sphereCount; index += 1) {
    const offset = input.sphereOffset + index * 4;
    payload.push(
      quantizeFinite(input.sphereData[offset], 4096),
      quantizeFinite(input.sphereData[offset + 1], 4096),
      quantizeFinite(input.sphereData[offset + 2], 4096),
      quantizeFinite(input.sphereData[offset + 3], 4096)
    );
  }
  return `monster:brain-blob:scissor:${fnv1a64Hex(payload.join("\0"))}`;
}

export function brainBlobMonsterFrameScissor(input: BrainBlobMonsterScissorInput): BrainBlobMonsterScissor {
  const address = brainBlobMonsterScissorAddress(input);
  const cached = brainBlobMonsterScissorByAddress.get(address);
  if (cached) return cached;

  const width = Math.max(1, Math.round(input.canvasWidth));
  const height = Math.max(1, Math.round(input.canvasHeight));
  const shortSide = Math.max(1, Math.min(width, height));
  let minX = width;
  let minY = height;
  let maxX = 0;
  let maxY = 0;

  for (let index = 0; index < input.sphereCount; index += 1) {
    const offset = input.sphereOffset + index * 4;
    const x = input.sphereData[offset];
    const y = input.sphereData[offset + 1] - input.cameraY;
    const zDistance = input.sphereData[offset + 2] + input.cameraZ;
    const radius = input.sphereData[offset + 3] + input.paddingWorld;
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(zDistance) || !Number.isFinite(radius)) {
      return { x: 0, y: 0, width, height };
    }
    const nearDistance = Math.max(0.35, zDistance - radius);
    const scale = (input.cameraFocal / nearDistance) * shortSide;
    const screenX = width * input.viewCenterX + (x / zDistance) * input.cameraFocal * shortSide;
    const screenY = height * input.viewCenterY - (y / zDistance) * input.cameraFocal * shortSide;
    const screenRadius = radius * scale;
    minX = Math.min(minX, screenX - screenRadius);
    maxX = Math.max(maxX, screenX + screenRadius);
    minY = Math.min(minY, screenY - screenRadius);
    maxY = Math.max(maxY, screenY + screenRadius);
  }

  const scissor = minX >= maxX || minY >= maxY
    ? { x: 0, y: 0, width, height }
    : {
        x: clampNumber(Math.floor(minX - input.paddingPixels), 0, width),
        y: clampNumber(Math.floor(minY - input.paddingPixels), 0, height),
        width: 1,
        height: 1
      };
  if (minX < maxX && minY < maxY) {
    const right = clampNumber(Math.ceil(maxX + input.paddingPixels), 0, width);
    const bottom = clampNumber(Math.ceil(maxY + input.paddingPixels), 0, height);
    scissor.width = Math.max(1, right - scissor.x);
    scissor.height = Math.max(1, bottom - scissor.y);
  }
  brainBlobMonsterScissorByAddress.set(address, scissor);
  return scissor;
}

export function brainBlobMonsterFrameAddress(input: BrainBlobMonsterFrameInput): { address: string; timeTick: number } {
  const timeTick = quantizeFinite(input.timeSeconds, BRAIN_BLOB_MONSTER_FRAME_HZ);
  const payload = [
    BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA,
    input.lane,
    input.shaderHash,
    Math.max(1, Math.round(input.canvasWidth)),
    Math.max(1, Math.round(input.canvasHeight)),
    timeTick,
    quantizeFinite(input.seed, 1024),
    quantizeFinite(input.pointerX, 2048),
    quantizeFinite(input.pointerY, 2048),
    quantizeFinite(input.pointerStrength, 1024),
    input.pointerOver ? 1 : 0
  ].join("\0");
  return {
    address: `monster:brain-blob:${fnv1a64Hex(payload)}`,
    timeTick
  };
}

export function createBrainBlobMonsterFrameCache(): BrainBlobMonsterFrameCache {
  let acceptedFrames = 0;
  let reusedFrames = 0;
  let uniqueFrames = 0;
  let lastAddress = "";

  const snapshotStats = (): BrainBlobMonsterFrameCacheStats => ({
    acceptedFrames,
    reusedFrames,
    uniqueFrames,
    lastAddress
  });

  return {
    quantizeTime(timeSeconds) {
      return quantizeFinite(timeSeconds, BRAIN_BLOB_MONSTER_FRAME_HZ) / BRAIN_BLOB_MONSTER_FRAME_HZ;
    },
    probe(input) {
      const { address, timeTick } = brainBlobMonsterFrameAddress(input);
      const reused = address === lastAddress;
      acceptedFrames += 1;
      if (reused) {
        reusedFrames += 1;
      } else {
        uniqueFrames += 1;
        lastAddress = address;
      }
      return {
        schema: BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA,
        address,
        reused,
        timeTick,
        stats: snapshotStats()
      };
    },
    stats: snapshotStats
  };
}
