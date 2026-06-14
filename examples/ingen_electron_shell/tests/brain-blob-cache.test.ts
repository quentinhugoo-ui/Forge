import { describe, expect, it } from "vitest";
import {
  BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA,
  BRAIN_BLOB_MONSTER_FRAME_HZ,
  brainBlobMonsterFrameAddress,
  createBrainBlobMonsterFrameCache
} from "../src/renderer/brain-blob-cache";

const baseFrame = {
  lane: "webgpu" as const,
  shaderHash: "forge.kasm.brain_blob.sdf_metaball_raymarch.v1:wgsl",
  canvasWidth: 1920,
  canvasHeight: 1080,
  seed: 42.25,
  pointerX: 0.1,
  pointerY: -0.2,
  pointerStrength: 0.35,
  pointerOver: true
};

describe("Brain blob Monster frame cache", () => {
  it("content-addresses identical quantized frame inputs", () => {
    const tickTime = 18 / BRAIN_BLOB_MONSTER_FRAME_HZ;
    const first = brainBlobMonsterFrameAddress({
      ...baseFrame,
      timeSeconds: tickTime + 0.0004
    });
    const second = brainBlobMonsterFrameAddress({
      ...baseFrame,
      timeSeconds: tickTime + 0.0012
    });

    expect(first.address).toBe(second.address);
    expect(first.timeTick).toBe(18);
  });

  it("marks repeated addresses as reused instead of unique work", () => {
    const cache = createBrainBlobMonsterFrameCache();
    const timeSeconds = cache.quantizeTime(1.234);
    const first = cache.probe({
      ...baseFrame,
      timeSeconds
    });
    const second = cache.probe({
      ...baseFrame,
      timeSeconds
    });

    expect(first.schema).toBe(BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA);
    expect(first.reused).toBe(false);
    expect(second.reused).toBe(true);
    expect(second.stats.uniqueFrames).toBe(1);
    expect(second.stats.reusedFrames).toBe(1);
  });

  it("separates shader lanes so WebGPU and WebGL never share frame proofs", () => {
    const webGpu = brainBlobMonsterFrameAddress({
      ...baseFrame,
      timeSeconds: 1
    });
    const webGl = brainBlobMonsterFrameAddress({
      ...baseFrame,
      lane: "webgl2",
      shaderHash: "forge.kasm.brain_blob.sdf_metaball_raymarch.v1:glsl",
      timeSeconds: 1
    });

    expect(webGpu.address).not.toBe(webGl.address);
  });
});
