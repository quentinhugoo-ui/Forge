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

const BRAIN_BLOB_MONSTER_HUE_PERIOD_TICKS = BRAIN_BLOB_MONSTER_FRAME_HZ * 6;
const brainBlobMonsterHueRowsByTick = new Map<number, Float32Array>();

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
