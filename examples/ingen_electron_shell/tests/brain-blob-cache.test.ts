import { describe, expect, it } from "vitest";
import {
  BRAIN_BLOB_MONSTER_FRAME_CACHE_SCHEMA,
  BRAIN_BLOB_MONSTER_FRAME_HZ,
  BRAIN_BLOB_MONSTER_HUE_ROW_FLOATS,
  brainBlobMonsterFrameAddress,
  brainBlobMonsterColorizeAngle,
  brainBlobMonsterHuePeriodAddress,
  brainBlobMonsterFrameScissor,
  brainBlobMonsterScissorAddress,
  brainBlobMonsterUniformViews,
  createBrainBlobMonsterFrameCache,
  writeBrainBlobMonsterHueRows
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

  it("materializes the shader hue rotation as a per-frame Forge artifact", () => {
    const out = new Float32Array(120);
    const offset = 32;
    const timeSeconds = 1.75;
    writeBrainBlobMonsterHueRows(timeSeconds, out, offset);
    const angle = brainBlobMonsterColorizeAngle(timeSeconds);
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    const weights = [0.213, 0.715, 0.072] as const;
    const expected = [
      weights[0] + c * (1 - weights[0]) + s * -weights[0],
      weights[1] + c * -weights[1] + s * -weights[1],
      weights[2] + c * -weights[2] + s * (1 - weights[2]),
      0,
      weights[0] + c * -weights[0] + s * 0.143,
      weights[1] + c * (1 - weights[1]) + s * 0.14,
      weights[2] + c * -weights[2] + s * -0.283,
      0,
      weights[0] + c * -weights[0] + s * -(1 - weights[0]),
      weights[1] + c * -weights[1] + s * weights[1],
      weights[2] + c * (1 - weights[2]) + s * weights[2],
      0
    ];

    expect(out.subarray(offset, offset + BRAIN_BLOB_MONSTER_HUE_ROW_FLOATS)).toEqual(new Float32Array(expected));
  });

  it("reuses hue artifacts across the exact six-second animation period", () => {
    const first = brainBlobMonsterHuePeriodAddress(1.75);
    const second = brainBlobMonsterHuePeriodAddress(7.75);

    expect(first).toEqual(second);
  });

  it("content-addresses repeated scissor projections for identical KASM spheres", () => {
    const spheres = new Float32Array([
      -0.2, 0.1, 0, 0.28,
      0.2, -0.1, 0.05, 0.24
    ]);
    const input = {
      canvasWidth: 1200,
      canvasHeight: 800,
      sphereData: spheres,
      sphereOffset: 0,
      sphereCount: 2,
      cameraFocal: 1.72,
      cameraY: 0.03,
      cameraZ: 2.28,
      viewCenterX: 0.62,
      viewCenterY: 0.68,
      paddingWorld: 0.42,
      paddingPixels: 96
    };
    const firstAddress = brainBlobMonsterScissorAddress(input);
    const first = brainBlobMonsterFrameScissor(input);
    const second = brainBlobMonsterFrameScissor(input);

    expect(firstAddress).toBe(brainBlobMonsterScissorAddress(input));
    expect(second).toBe(first);
    expect(first.x).toBeGreaterThanOrEqual(0);
    expect(first.y).toBeGreaterThanOrEqual(0);
    expect(first.width).toBeGreaterThan(1);
    expect(first.height).toBeGreaterThan(1);
  });

  it("materializes uniform lanes once for the same KASM typed buffer", () => {
    const data = new Float32Array(100);
    const layout = {
      ballOffset: 4,
      ballFloats: 40,
      ksOffset: 44,
      ksFloats: 40,
      mouseOffset: 84,
      mouseFloats: 4,
      hueOffset: 88,
      hueFloats: 12
    };

    const first = brainBlobMonsterUniformViews(data, layout);
    const second = brainBlobMonsterUniformViews(data, layout);

    expect(second).toBe(first);
    expect(first.header.buffer).toBe(data.buffer);
    expect(first.balls.byteOffset).toBe(data.byteOffset + layout.ballOffset * Float32Array.BYTES_PER_ELEMENT);
    expect(first.hue.length).toBe(layout.hueFloats);
  });
});
