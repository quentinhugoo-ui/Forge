export const BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA = "forge.monster.brain_blob.frame_cache.v1";
export const BRAIN_BLOB_MONSTER_FRAME_HZ = 60;

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
