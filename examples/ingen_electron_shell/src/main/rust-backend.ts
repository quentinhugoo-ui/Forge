import { execFile, spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, statSync } from "node:fs";
import { join } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export interface RustBackendSession {
  sessionId: string;
  label: string;
  date: string;
  section: "forge" | "webexplorer" | "banger" | "trading" | "real-estate" | "alpha" | "shell";
  pinned: boolean;
  working: boolean;
  automated: boolean;
  archived: boolean;
}

export interface RustBackendTranscript {
  id: string;
  role: "system" | "assistant" | "user";
  text: string;
  proofHash: string;
}

export interface RustBackendProjection {
  schema: "ingen.native_services.electron_backend_projection.v1";
  source: string;
  backend: "rust";
  generatedAtUnixMs: number;
  activeSection: RustBackendSession["section"];
  sectionTitle: string;
  sessions: RustBackendSession[];
  transcript: RustBackendTranscript[];
  nativeStatus: {
    stateOwner: string;
    jobs: string;
    banger: string;
    webexplorer: string;
    monster: string;
    brain: string;
    provider: string;
    proof: string;
  };
  proofHash: string;
}

export interface RustBangerPreviewFrame {
  accepted: boolean;
  schema: "forge.banger.visible_preview_frame.v1";
  source: string;
  width: number;
  height: number;
  frameDataUrl: string;
  frameHash: string;
  sceneHash: string;
  proofHash: string;
  metrics: {
    splatCount: number;
    projectedSplatCount: number;
    rasterizedSplatCount: number;
    shadedPixelCount: number;
    tileCount: number;
    benchmarkGateCount: number;
    promotionAllowed: boolean;
    renderPath: string;
  };
  error?: {
    code: string;
    message: string;
    proofHash: string;
  };
}

export interface RustBangerPresentLoopBootstrap {
  ok: boolean;
  schema: "forge.banger.native_present_loop_bootstrap.v1";
  engine: "banger_rust_native_engine";
  lane: "native_tandem_render";
  nativeDomain: "render_3d";
  routeStatus: string;
  parentWindowHandleHash: string;
  childWindowHandleHash?: string;
  viewportWidth: number;
  viewportHeight: number;
  targetFrameMs: number;
  selectedAdapter?: Record<string, unknown> | null;
  adapterCount: number;
  backend: string;
  surfaceKind: string;
  swapchainFormat: string;
  presentMode: string;
  alphaMode: string;
  renderPassCount: number;
  submittedFrameCount: number;
  drawCallCount?: number;
  vertexCount?: number;
  indexCount?: number;
  instanceCount?: number;
  sceneObjectCount?: number;
  sceneGraphHash?: string;
  instanceBufferHash?: string;
  depthFormat?: string;
  frameTargetPolicy?: string;
  frameTargetHash?: string;
  depthTargetHash?: string;
  frameTargetAllocationCount?: number;
  surfaceResizeCount?: number;
  renderLoopPolicy?: string;
  clearColor: [number, number, number, number];
  frameUniformHash?: string;
  cameraUniformHash?: string;
  sceneMeshHash?: string;
  shaderSourceHash?: string;
  renderPipelineHash?: string;
  mapsTilesetContract?: {
    schema: "forge.banger.maps_photorealistic_3d_tiles_contract.v1";
    provider: "google_photorealistic_3d_tiles";
    rendererContract: "Cesium3DTileset_style_native_streamer";
    rootTilesetEndpoint: "ion://google-photorealistic-3d-tiles";
    rootRequestTtlHours: 3;
    nativeStreamer?: {
      schema: "forge.banger.native_3d_tiles_streamer.v1";
      authority: "banger_native_engine";
      status: string;
      rootIngestionStage: string;
      traversalStage: string;
      contentDecodeStage: string;
      georeferenceStage: string;
      gpuSubmissionStage: string;
      visualFallback: string;
      blocker: string;
    };
    attribution: {
      required: true;
      mode: "visible_on_screen";
    };
  } | null;
  mapsVisualGate?: {
    ok: boolean;
    nonblackPixelCount: number;
    nonFallbackBluePixelCount: number;
    frameHash: string;
    drawSource?: string | null;
    drawIndexCount: number;
    drawInstanceCount: number;
  } | null;
  previewWidth?: number;
  previewHeight?: number;
  previewByteCount?: number;
  previewRgbaHash?: string;
  previewProofHash?: string;
  previewRgba8?: number[];
  previewFrameDataUrl?: string;
  frameHash: string;
  presentLoopHash: string;
  proofHash: string;
  hostPid?: number;
  verifier: {
    wall: string;
    frontierHypothesis: string;
    localGate: string;
    rollbackPath: string;
  };
  error?: {
    code: string;
    message: string;
    proofHash: string;
  };
}

type RustBangerNativeHostOptions = {
  parentWindowHandle?: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  sceneKind?: string;
  target?: string;
  latitude?: number;
  longitude?: number;
  heightMeters?: number;
};

let cachedProjection: RustBackendProjection | null = shadowProjection("startup snapshot; native bridge refresh pending");
let cachedAt = 0;
let refreshInFlight: Promise<RustBackendProjection> | null = null;
const cacheTtlMs = 15_000;
let bangerNativeHost:
  | {
      key: string;
      boundsKey: string;
      child: ChildProcessWithoutNullStreams;
      ready: RustBangerPresentLoopBootstrap;
    }
  | null = null;

export function stopRustBangerNativeHost(): void {
  if (bangerNativeHost && !bangerNativeHost.child.killed) {
    if (!bangerNativeHost.child.stdin.destroyed && bangerNativeHost.child.stdin.writable) {
      bangerNativeHost.child.stdin.write("shutdown\n");
    }
    bangerNativeHost.child.kill();
  }
  bangerNativeHost = null;
}

export function cachedRustBackendProjection(): RustBackendProjection | null {
  return cachedProjection;
}

export async function refreshRustBackendProjection(shellRoot: string): Promise<RustBackendProjection> {
  const now = Date.now();
  if (cachedProjection && now - cachedAt < cacheTtlMs) {
    return cachedProjection;
  }
  if (refreshInFlight) {
    return refreshInFlight;
  }
  refreshInFlight = loadRustBackendProjection(shellRoot).finally(() => {
    refreshInFlight = null;
  });
  return refreshInFlight;
}

async function loadRustBackendProjection(shellRoot: string): Promise<RustBackendProjection> {
  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  try {
    const stdout =
      bridgeExe && existsSync(bridgeExe)
        ? await runBackendExe(bridgeExe)
        : process.env.FORGE_ELECTRON_ALLOW_CARGO_BACKEND === "1"
          ? await runBackendViaForgeCargo(repoRoot)
          : null;
    if (!stdout) {
      return rememberProjection(shadowProjection("native bridge binary unavailable; cargo backend disabled"));
    }
    return rememberProjection(parseProjection(stdout));
  } catch (error) {
    console.error("Rust backend projection failed; using non-blocking shadow projection.", error);
    return rememberProjection(shadowProjection("native bridge refresh failed; shadow projection active"));
  }
}

export async function loadRustBangerPreviewFrame(shellRoot: string): Promise<RustBangerPreviewFrame> {
  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  try {
    const stdout =
      bridgeExe && existsSync(bridgeExe)
        ? await runBackendExe(bridgeExe, ["--banger-preview-frame"])
        : process.env.FORGE_ELECTRON_ALLOW_CARGO_BACKEND === "1"
          ? await runBackendViaForgeCargo(repoRoot, ["--banger-preview-frame"])
          : null;
    if (!stdout) {
      return shadowBangerPreviewFrame("native bridge binary unavailable; cargo backend disabled");
    }
    return parseBangerPreviewFrame(stdout);
  } catch (error) {
    console.error("Rust Banger preview frame failed.", error);
    return shadowBangerPreviewFrame("native preview frame failed; no frame promoted");
  }
}

export async function loadRustBangerPresentLoopBootstrap(
  shellRoot: string,
  options: RustBangerNativeHostOptions = {}
): Promise<RustBangerPresentLoopBootstrap> {
  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  if (options.parentWindowHandle) {
    const host = await launchRustBangerNativeHost(shellRoot, options).catch((error) => {
      console.error("Rust Banger native host failed to launch.", error);
      return null;
    });
    if (host) {
      if (host.surfaceKind === "win32_child_window_wgpu_surface" || host.previewFrameDataUrl) {
        return host;
      }
      console.warn("Rust Banger native host has no preview frame; falling back to offscreen present-loop preview.");
    }
  }
  const env = {
    ...process.env,
    ...(options.parentWindowHandle ? { FORGE_BANGER_PARENT_HWND: options.parentWindowHandle } : {}),
    ...(options.x !== undefined ? { FORGE_BANGER_VIEWPORT_X: String(Math.round(options.x)) } : {}),
    ...(options.y !== undefined ? { FORGE_BANGER_VIEWPORT_Y: String(Math.round(options.y)) } : {}),
    ...(options.width ? { FORGE_BANGER_VIEWPORT_WIDTH: String(Math.round(options.width)) } : {}),
    ...(options.height ? { FORGE_BANGER_VIEWPORT_HEIGHT: String(Math.round(options.height)) } : {}),
    ...(options.x !== undefined || options.y !== undefined ? { FORGE_BANGER_VIEWPORT_FIXED: "1" } : {}),
    ...(options.sceneKind ? { FORGE_BANGER_SCENE_KIND: options.sceneKind } : {}),
    ...(Number.isFinite(options.latitude) ? { FORGE_BANGER_MAPS_ORIGIN_LATITUDE: String(options.latitude) } : {}),
    ...(Number.isFinite(options.longitude) ? { FORGE_BANGER_MAPS_ORIGIN_LONGITUDE: String(options.longitude) } : {}),
    ...(Number.isFinite(options.heightMeters) ? { FORGE_BANGER_MAPS_ORIGIN_HEIGHT_METERS: String(options.heightMeters) } : {}),
    ...(options.target ? { FORGE_BANGER_MAPS_TARGET: options.target } : {})
  };
  try {
    const stdout =
      bridgeExe && existsSync(bridgeExe)
        ? await runBackendExe(bridgeExe, ["--banger-present-loop-bootstrap"], env)
        : process.env.FORGE_ELECTRON_ALLOW_CARGO_BACKEND === "1"
          ? await runBackendViaForgeCargo(repoRoot, ["--banger-present-loop-bootstrap"], env)
          : null;
    if (!stdout) {
      return shadowBangerPresentLoopBootstrap("native bridge binary unavailable; cargo backend disabled");
    }
    return parseBangerPresentLoopBootstrap(stdout);
  } catch (error) {
    console.error("Rust Banger present loop bootstrap failed.", error);
    return shadowBangerPresentLoopBootstrap("native present loop bootstrap failed; child surface pending");
  }
}

async function launchRustBangerNativeHost(
  shellRoot: string,
  options: RustBangerNativeHostOptions
): Promise<RustBangerPresentLoopBootstrap | null> {
  const parentWindowHandle = options.parentWindowHandle?.trim();
  if (!parentWindowHandle) {
    return null;
  }
  const width = Math.max(64, Math.round(options.width ?? 1280));
  const height = Math.max(64, Math.round(options.height ?? 720));
  const x = Math.max(0, Math.round(options.x ?? 0));
  const y = Math.max(0, Math.round(options.y ?? 0));
  const sceneKind = options.sceneKind === "maps_sphere" ? "maps_sphere" : "dense_meshlet_field";
  const fixedViewport = options.x !== undefined || options.y !== undefined;
  const latitude = Number.isFinite(options.latitude) ? Number(options.latitude) : undefined;
  const longitude = Number.isFinite(options.longitude) ? Number(options.longitude) : undefined;
  const heightMeters = Number.isFinite(options.heightMeters) ? Number(options.heightMeters) : undefined;
  const target = options.target?.replace(/\s+/g, " ").trim() ?? "";
  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  const bridgeSignature = bridgeExe && existsSync(bridgeExe)
    ? `${bridgeExe}:${Math.round(statSync(bridgeExe).mtimeMs)}:${statSync(bridgeExe).size}`
    : "cargo-backend";
  const key = [
    parentWindowHandle,
    sceneKind,
    latitude?.toFixed(7) ?? "none",
    longitude?.toFixed(7) ?? "none",
    heightMeters?.toFixed(2) ?? "0",
    createHash("sha256").update(target).digest("hex").slice(0, 16),
    createHash("sha256").update(bridgeSignature).digest("hex").slice(0, 16)
  ].join(":");
  const boundsKey = bangerNativeBoundsKey(x, y, width, height);
  if (bangerNativeHost && bangerNativeHost.key === key && !bangerNativeHost.child.killed) {
    if (bangerNativeHost.boundsKey !== boundsKey) {
      sendRustBangerNativeHostBounds(bangerNativeHost.child, x, y, width, height);
      bangerNativeHost.boundsKey = boundsKey;
      bangerNativeHost.ready = {
        ...bangerNativeHost.ready,
        viewportWidth: width,
        viewportHeight: height,
        surfaceResizeCount: (bangerNativeHost.ready.surfaceResizeCount ?? 0) + 1,
        routeStatus: "native_child_surface_host_live_resized"
      };
    }
    return bangerNativeHost.ready;
  }
  if (bangerNativeHost && !bangerNativeHost.child.killed) {
    stopRustBangerNativeHost();
  }

  const env = {
    ...process.env,
    FORGE_BANGER_PARENT_HWND: parentWindowHandle,
    FORGE_BANGER_VIEWPORT_X: String(x),
    FORGE_BANGER_VIEWPORT_Y: String(y),
    FORGE_BANGER_VIEWPORT_WIDTH: String(width),
    FORGE_BANGER_VIEWPORT_HEIGHT: String(height),
    FORGE_BANGER_VIEWPORT_FIXED: fixedViewport ? "1" : "0",
    FORGE_BANGER_SCENE_KIND: sceneKind,
    ...(latitude !== undefined ? { FORGE_BANGER_MAPS_ORIGIN_LATITUDE: String(latitude) } : {}),
    ...(longitude !== undefined ? { FORGE_BANGER_MAPS_ORIGIN_LONGITUDE: String(longitude) } : {}),
    ...(heightMeters !== undefined ? { FORGE_BANGER_MAPS_ORIGIN_HEIGHT_METERS: String(heightMeters) } : {}),
    ...(target ? { FORGE_BANGER_MAPS_TARGET: target } : {})
  };
  const spawnSpec =
    bridgeExe && existsSync(bridgeExe)
      ? { command: bridgeExe, args: ["--banger-native-host"], cwd: shellRoot }
      : process.env.FORGE_ELECTRON_ALLOW_CARGO_BACKEND === "1"
        ? {
            command: join(
              process.env.SystemRoot ?? "C:\\Windows",
              "System32",
              "WindowsPowerShell",
              "v1.0",
              "powershell.exe"
            ),
            args: [
              "-NoProfile",
              "-ExecutionPolicy",
              "Bypass",
              "-File",
              join(repoRoot, "scripts", "forge-cargo.ps1"),
              "run",
              "--manifest-path",
              "examples\\ingen_native_services\\Cargo.toml",
              "--bin",
              "ingen_electron_backend_bridge",
              "--",
              "--banger-native-host"
            ],
            cwd: repoRoot
          }
        : null;
  if (!spawnSpec) {
    return null;
  }

  const child = spawn(spawnSpec.command, spawnSpec.args, {
    cwd: spawnSpec.cwd,
    env,
    windowsHide: true
  });
  child.stderr.on("data", (chunk) => {
    console.warn("Banger native host stderr", chunk.toString());
  });
  child.once("exit", () => {
    if (bangerNativeHost?.child === child) {
      bangerNativeHost = null;
    }
  });
  const ready = await readFirstJsonLine(child, 12_000);
  const parsed = parseBangerPresentLoopBootstrap(ready);
  bangerNativeHost = { key, boundsKey, child, ready: parsed };
  return parsed;
}

function bangerNativeBoundsKey(x: number, y: number, width: number, height: number): string {
  return `${Math.round(x)}:${Math.round(y)}:${Math.round(width)}:${Math.round(height)}`;
}

function sendRustBangerNativeHostBounds(
  child: ChildProcessWithoutNullStreams,
  x: number,
  y: number,
  width: number,
  height: number
): void {
  if (child.killed || child.stdin.destroyed || !child.stdin.writable) {
    return;
  }
  child.stdin.write(`resize ${Math.round(x)} ${Math.round(y)} ${Math.round(width)} ${Math.round(height)}\n`);
}

function readFirstJsonLine(child: ChildProcessWithoutNullStreams, timeoutMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    let stdout = "";
    const timer = setTimeout(() => {
      cleanup();
      child.kill();
      reject(new Error("Timed out waiting for Banger native host readiness JSON."));
    }, timeoutMs);
    const cleanup = () => {
      clearTimeout(timer);
      child.stdout.off("data", onData);
      child.off("error", onError);
      child.off("exit", onExit);
    };
    const onData = (chunk: Buffer) => {
      stdout += chunk.toString();
      const line = stdout.split(/\r?\n/).find((entry) => entry.trim().startsWith("{"));
      if (line) {
        cleanup();
        resolve(line);
      }
    };
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const onExit = (code: number | null) => {
      cleanup();
      reject(new Error(`Banger native host exited before readiness JSON with code ${code ?? "unknown"}.`));
    };
    child.stdout.on("data", onData);
    child.once("error", onError);
    child.once("exit", onExit);
  });
}

async function runBackendExe(
  exePath: string,
  args: string[] = [],
  env: NodeJS.ProcessEnv = process.env
): Promise<string> {
  const { stdout } = await execFileAsync(exePath, args, {
    env,
    timeout: 20_000,
    windowsHide: true,
    maxBuffer: 1024 * 1024
  });
  return stdout;
}

async function runBackendViaForgeCargo(
  repoRoot: string,
  bridgeArgs: string[] = [],
  env: NodeJS.ProcessEnv = process.env
): Promise<string> {
  const powershell = join(
    process.env.SystemRoot ?? "C:\\Windows",
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe"
  );
  const forgeCargo = join(repoRoot, "scripts", "forge-cargo.ps1");
  const { stdout } = await execFileAsync(
    powershell,
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      forgeCargo,
      "run",
      "--manifest-path",
      "examples\\ingen_native_services\\Cargo.toml",
      "--bin",
      "ingen_electron_backend_bridge",
      ...(bridgeArgs.length > 0 ? ["--", ...bridgeArgs] : [])
    ],
    {
      cwd: repoRoot,
      env,
      timeout: 90_000,
      windowsHide: true,
      maxBuffer: 4 * 1024 * 1024
    }
  );
  return stdout;
}

function parseBangerPreviewFrame(stdout: string): RustBangerPreviewFrame {
  const line = stdout
    .trim()
    .split(/\r?\n/)
    .reverse()
    .find((entry) => entry.trim().startsWith("{"));
  if (!line) {
    throw new Error("Rust backend did not return a JSON Banger preview frame.");
  }
  const parsed = JSON.parse(line) as Partial<RustBangerPreviewFrame>;
  if (
    parsed.schema !== "forge.banger.visible_preview_frame.v1" ||
    parsed.accepted !== true ||
    typeof parsed.frameDataUrl !== "string" ||
    !parsed.frameDataUrl.startsWith("data:image/bmp;base64,") ||
    typeof parsed.frameHash !== "string" ||
    typeof parsed.sceneHash !== "string" ||
    typeof parsed.proofHash !== "string" ||
    !parsed.metrics ||
    typeof parsed.metrics.shadedPixelCount !== "number"
  ) {
    throw new Error("Rust Banger preview frame failed validation.");
  }
  return parsed as RustBangerPreviewFrame;
}

function parseBangerPresentLoopBootstrap(stdout: string): RustBangerPresentLoopBootstrap {
  const line = stdout
    .trim()
    .split(/\r?\n/)
    .reverse()
    .find((entry) => entry.trim().startsWith("{"));
  if (!line) {
    throw new Error("Rust backend did not return a JSON Banger present loop bootstrap.");
  }
  const parsed = JSON.parse(line) as Partial<RustBangerPresentLoopBootstrap>;
  if (
    parsed.schema !== "forge.banger.native_present_loop_bootstrap.v1" ||
    parsed.ok !== true ||
    parsed.lane !== "native_tandem_render" ||
    parsed.nativeDomain !== "render_3d" ||
    typeof parsed.backend !== "string" ||
    typeof parsed.surfaceKind !== "string" ||
    typeof parsed.frameHash !== "string" ||
    typeof parsed.presentLoopHash !== "string" ||
    typeof parsed.proofHash !== "string" ||
    typeof parsed.submittedFrameCount !== "number" ||
    parsed.submittedFrameCount < 1 ||
    (typeof parsed.drawCallCount === "number" && parsed.drawCallCount < 1) ||
    (typeof parsed.vertexCount === "number" && parsed.vertexCount < 3) ||
    (typeof parsed.indexCount === "number" && parsed.indexCount < 3) ||
    (typeof parsed.instanceCount === "number" && parsed.instanceCount < 1) ||
    (typeof parsed.sceneObjectCount === "number" && parsed.sceneObjectCount < 1) ||
    (typeof parsed.sceneGraphHash === "string" && parsed.sceneGraphHash.length !== 64) ||
    (typeof parsed.instanceBufferHash === "string" && parsed.instanceBufferHash.length !== 64) ||
    (typeof parsed.depthFormat === "string" && parsed.depthFormat.length === 0) ||
    (typeof parsed.frameTargetHash === "string" && parsed.frameTargetHash.length !== 64) ||
    (typeof parsed.depthTargetHash === "string" && parsed.depthTargetHash.length !== 64) ||
    (typeof parsed.frameTargetAllocationCount === "number" && parsed.frameTargetAllocationCount < 1) ||
    (typeof parsed.surfaceResizeCount === "number" && parsed.surfaceResizeCount < 0) ||
    (typeof parsed.sceneMeshHash === "string" && parsed.sceneMeshHash.length !== 64) ||
    (typeof parsed.cameraUniformHash === "string" && parsed.cameraUniformHash.length !== 64)
  ) {
    throw new Error("Rust Banger present loop bootstrap failed validation.");
  }
  const bootstrap = parsed as RustBangerPresentLoopBootstrap;
  if (
    typeof bootstrap.previewWidth === "number" &&
    typeof bootstrap.previewHeight === "number" &&
    Array.isArray(bootstrap.previewRgba8)
  ) {
    bootstrap.previewFrameDataUrl = bangerPreviewRgba8ToBmpDataUrl(
      bootstrap.previewWidth,
      bootstrap.previewHeight,
      bootstrap.previewRgba8
    );
  }
  return bootstrap;
}

function bangerPreviewRgba8ToBmpDataUrl(width: number, height: number, rgba8: number[]): string {
  const safeWidth = Math.max(1, Math.floor(width));
  const safeHeight = Math.max(1, Math.floor(height));
  const pixelCount = safeWidth * safeHeight;
  if (rgba8.length < pixelCount * 4) {
    throw new Error("Rust Banger present loop preview buffer is shorter than expected.");
  }
  const rowStride = Math.ceil((safeWidth * 3) / 4) * 4;
  const pixelBytes = rowStride * safeHeight;
  const fileBytes = 54 + pixelBytes;
  const bmp = Buffer.alloc(fileBytes);
  bmp.write("BM", 0, "ascii");
  bmp.writeUInt32LE(fileBytes, 2);
  bmp.writeUInt32LE(54, 10);
  bmp.writeUInt32LE(40, 14);
  bmp.writeInt32LE(safeWidth, 18);
  bmp.writeInt32LE(safeHeight, 22);
  bmp.writeUInt16LE(1, 26);
  bmp.writeUInt16LE(24, 28);
  bmp.writeUInt32LE(pixelBytes, 34);
  for (let y = 0; y < safeHeight; y += 1) {
    const sourceY = safeHeight - 1 - y;
    const destRow = 54 + y * rowStride;
    for (let x = 0; x < safeWidth; x += 1) {
      const source = (sourceY * safeWidth + x) * 4;
      const dest = destRow + x * 3;
      bmp[dest] = clampByte(rgba8[source]);
      bmp[dest + 1] = clampByte(rgba8[source + 1]);
      bmp[dest + 2] = clampByte(rgba8[source + 2]);
    }
  }
  return `data:image/bmp;base64,${bmp.toString("base64")}`;
}

function clampByte(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(255, Math.round(value)));
}

function parseProjection(stdout: string): RustBackendProjection {
  const line = stdout
    .trim()
    .split(/\r?\n/)
    .reverse()
    .find((entry) => entry.trim().startsWith("{"));
  if (!line) {
    throw new Error("Rust backend did not return a JSON projection.");
  }
  const parsed = JSON.parse(line) as Partial<RustBackendProjection>;
  if (
    parsed.schema !== "ingen.native_services.electron_backend_projection.v1" ||
    parsed.backend !== "rust" ||
    !Array.isArray(parsed.sessions) ||
    !Array.isArray(parsed.transcript) ||
    !parsed.nativeStatus ||
    typeof parsed.proofHash !== "string"
  ) {
    throw new Error("Rust backend projection failed validation.");
  }
  return parsed as RustBackendProjection;
}

function rememberProjection(projection: RustBackendProjection): RustBackendProjection {
  cachedProjection = projection;
  cachedAt = Date.now();
  return projection;
}

function shadowProjection(reason: string): RustBackendProjection {
  const generatedAtUnixMs = Date.now();
  const projection: RustBackendProjection = {
    schema: "ingen.native_services.electron_backend_projection.v1",
    source: "examples/ingen_native_services/shadow",
    backend: "rust",
    generatedAtUnixMs,
    activeSection: "forge",
    sectionTitle: "Forge",
    sessions: [
      {
        sessionId: "native-front-migration",
        label: "Electron cutover",
        date: "2026-06-09",
        section: "forge",
        pinned: true,
        working: false,
        automated: false,
        archived: false
      },
      {
        sessionId: "test-session-example",
        label: "test session example",
        date: "2026-06-10",
        section: "forge",
        pinned: false,
        working: true,
        automated: false,
        archived: false
      },
      {
        sessionId: "banger-native-surface",
        label: "Banger native surface",
        date: "2026-06-09",
        section: "banger",
        pinned: false,
        working: false,
        automated: true,
        archived: false
      },
      {
        sessionId: "webexplorer-rust-webview",
        label: "WebExplorer Rust WebView",
        date: "2026-06-09",
        section: "webexplorer",
        pinned: false,
        working: false,
        automated: false,
        archived: false
      },
      {
        sessionId: "monster-compute-proof",
        label: "Monster compute proof",
        date: "2026-06-08",
        section: "forge",
        pinned: false,
        working: false,
        automated: false,
        archived: false
      }
    ],
    transcript: [
      {
        id: "native-shadow-online",
        role: "system",
        text: "Electron shell is responsive while the native backend bridge is unavailable.",
        proofHash: "native-shadow-online"
      },
      {
        id: "native-shadow-reason",
        role: "assistant",
        text: reason,
        proofHash: "native-shadow-reason"
      }
    ],
    nativeStatus: {
      stateOwner: "electron-shadow/non-blocking",
      jobs: "queued=0 running=0 done=0 failed=0",
      banger: `native bridge pending (${reason})`,
      webexplorer: "rust-owned-webview-slot=pending",
      monster: "local-compute=pending",
      brain: "evidence-aware-memory=pending",
      provider: "provider=openai ready=false source=shadow",
      proof: "electron-shadow-projection"
    },
    proofHash: ""
  };
  projection.proofHash = hashJson(projection);
  return projection;
}

function shadowBangerPreviewFrame(reason: string): RustBangerPreviewFrame {
  const frame: RustBangerPreviewFrame = {
    accepted: false,
    schema: "forge.banger.visible_preview_frame.v1",
    source: "examples/ingen_native_services/shadow",
    width: 0,
    height: 0,
    frameDataUrl: "",
    frameHash: "",
    sceneHash: "",
    proofHash: "",
    metrics: {
      splatCount: 0,
      projectedSplatCount: 0,
      rasterizedSplatCount: 0,
      shadedPixelCount: 0,
      tileCount: 0,
      benchmarkGateCount: 0,
      promotionAllowed: false,
      renderPath: "native_preview_unavailable"
    },
    error: {
      code: "rust_unavailable",
      message: reason,
      proofHash: hashJson({ reason })
    }
  };
  frame.proofHash = hashJson({ ...frame, proofHash: "" });
  return frame;
}

function shadowBangerPresentLoopBootstrap(reason: string): RustBangerPresentLoopBootstrap {
  const bootstrap: RustBangerPresentLoopBootstrap = {
    ok: false,
    schema: "forge.banger.native_present_loop_bootstrap.v1",
    engine: "banger_rust_native_engine",
    lane: "native_tandem_render",
    nativeDomain: "render_3d",
    routeStatus: "shadow_only",
    parentWindowHandleHash: "unavailable",
    viewportWidth: 0,
    viewportHeight: 0,
    targetFrameMs: 16.67,
    selectedAdapter: null,
    adapterCount: 0,
    backend: "unavailable",
    surfaceKind: "child_surface_pending",
    swapchainFormat: "unavailable",
    presentMode: "unavailable",
    alphaMode: "unavailable",
    renderPassCount: 0,
    submittedFrameCount: 0,
    drawCallCount: 0,
    vertexCount: 0,
    indexCount: 0,
    instanceCount: 0,
    sceneObjectCount: 0,
    sceneGraphHash: "",
    instanceBufferHash: "",
    depthFormat: "unavailable",
    frameTargetPolicy: "shadow_only",
    frameTargetHash: "",
    depthTargetHash: "",
    frameTargetAllocationCount: 0,
    surfaceResizeCount: 0,
    renderLoopPolicy: "shadow_only",
    clearColor: [0, 0, 0, 1],
    frameUniformHash: "",
    cameraUniformHash: "",
    sceneMeshHash: "",
    shaderSourceHash: "",
    renderPipelineHash: "",
    mapsTilesetContract: null,
    frameHash: "",
    presentLoopHash: "",
    proofHash: "",
    verifier: {
      wall: "native_surface",
      frontierHypothesis: "shadow bootstrap keeps Electron non-blocking until the Rust bridge is available.",
      localGate: "npx tsc -p tsconfig.json --noEmit",
      rollbackPath: "remove getBangerPresentLoopBootstrap IPC"
    },
    error: {
      code: "rust_unavailable",
      message: reason,
      proofHash: hashJson({ bangerPresentLoop: false, reason })
    }
  };
  bootstrap.proofHash = hashJson({ ...bootstrap, proofHash: "" });
  bootstrap.frameHash = hashJson({ frame: "shadow", proofHash: bootstrap.proofHash });
  bootstrap.presentLoopHash = hashJson({ presentLoop: "shadow", frameHash: bootstrap.frameHash });
  return bootstrap;
}

function hashJson(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}
