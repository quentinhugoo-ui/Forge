import { execFile, spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
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
  clearColor: [number, number, number, number];
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

let cachedProjection: RustBackendProjection | null = shadowProjection("startup snapshot; native bridge refresh pending");
let cachedAt = 0;
let refreshInFlight: Promise<RustBackendProjection> | null = null;
const cacheTtlMs = 15_000;
let bangerNativeHost:
  | {
      key: string;
      child: ChildProcessWithoutNullStreams;
      ready: RustBangerPresentLoopBootstrap;
    }
  | null = null;

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
  options: { parentWindowHandle?: string; width?: number; height?: number } = {}
): Promise<RustBangerPresentLoopBootstrap> {
  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  if (options.parentWindowHandle) {
    const host = await launchRustBangerNativeHost(shellRoot, options).catch((error) => {
      console.error("Rust Banger native host failed to launch.", error);
      return null;
    });
    if (host) {
      return host;
    }
  }
  const env = {
    ...process.env,
    ...(options.parentWindowHandle ? { FORGE_BANGER_PARENT_HWND: options.parentWindowHandle } : {}),
    ...(options.width ? { FORGE_BANGER_VIEWPORT_WIDTH: String(Math.round(options.width)) } : {}),
    ...(options.height ? { FORGE_BANGER_VIEWPORT_HEIGHT: String(Math.round(options.height)) } : {})
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
  options: { parentWindowHandle?: string; width?: number; height?: number }
): Promise<RustBangerPresentLoopBootstrap | null> {
  const parentWindowHandle = options.parentWindowHandle?.trim();
  if (!parentWindowHandle) {
    return null;
  }
  const width = Math.max(64, Math.round(options.width ?? 1280));
  const height = Math.max(64, Math.round(options.height ?? 720));
  const key = `${parentWindowHandle}:${width}:${height}`;
  if (bangerNativeHost && bangerNativeHost.key === key && !bangerNativeHost.child.killed) {
    return bangerNativeHost.ready;
  }
  if (bangerNativeHost && !bangerNativeHost.child.killed) {
    bangerNativeHost.child.kill();
    bangerNativeHost = null;
  }

  const repoRoot = join(shellRoot, "..", "..");
  const bridgeExe = process.env.FORGE_ELECTRON_BACKEND_EXE;
  const env = {
    ...process.env,
    FORGE_BANGER_PARENT_HWND: parentWindowHandle,
    FORGE_BANGER_VIEWPORT_WIDTH: String(width),
    FORGE_BANGER_VIEWPORT_HEIGHT: String(height)
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
  bangerNativeHost = { key, child, ready: parsed };
  return parsed;
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
    parsed.submittedFrameCount < 1
  ) {
    throw new Error("Rust Banger present loop bootstrap failed validation.");
  }
  return parsed as RustBangerPresentLoopBootstrap;
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
    clearColor: [0, 0, 0, 1],
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
