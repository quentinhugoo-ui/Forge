import { app, BrowserView, BrowserWindow, WebContentsView, clipboard, desktopCapturer, dialog, ipcMain, net, protocol, safeStorage, screen, session, shell } from "electron";
import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { appendFileSync, copyFileSync, createReadStream, existsSync, mkdirSync, statSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { copyFile, mkdir, open, readFile, readdir, rename, stat, writeFile } from "node:fs/promises";
import { cpus } from "node:os";
import { basename, dirname, extname, join, resolve } from "node:path";
import { Readable } from "node:stream";
import { fileURLToPath, pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";
import {
  cachedRustBackendProjection,
  loadRustBangerPresentLoopBootstrap,
  loadRustBangerPreviewFrame,
  refreshRustBackendProjection,
  type RustBangerPresentLoopBootstrap,
  type RustBangerPreviewFrame,
  type RustBackendProjection
} from "./rust-backend.js";
import {
  FORGE_ELECTRON_IPC_VERSION,
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_CODEACT_COMMANDS,
  BRAIN_CODEACT_ROUTING_RULES,
  BRAIN_AIRBNB_COMMAND,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_RENAME_SESSION_COMMAND,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_WEBSEARCH_COMMAND,
  BRAIN_CODEDOCS_COMMAND,
  BRAIN_GITHUB_MCP_COMMAND,
  BRAIN_WEBACT_COMMAND,
  BRAIN_SECURITYSCAN_COMMAND,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_MAPS_COMMAND,
  BRAIN_SCRAPERS_COMMAND,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_NEWMODULE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_QUESTIONNAIRE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_CODING_COMMAND,
  BRAIN_CODING_LIVE_PREVIEW_COMMAND,
  BRAIN_SCIENCE_VISIBLE_CATALOG,
  BRAIN_CODING_VISIBLE_CATALOG,
  BRAIN_WORKSPACE_COMMAND,
  BRAIN_LOCAL_ACTIONS_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND,
  type CanvasSurfaceSummary,
  type CanvasSurfacesCommand,
  type CanvasSurfacesCommandResult,
  type CanvasSurfacesSnapshot,
  type CitySuggestion,
  type CitySuggestionResult,
  type FrontSliceMode,
  type HeaderCommand,
  type HeaderCommandResult,
  type HeaderSurfaceContract,
  type HeaderSurfaceSnapshot,
  type HeaderSnapshot,
  type BangerGoogleTilesConfigResult,
  type HardwareGpuSnapshot,
  type HardwareMetric,
  type HardwareProcessSnapshot,
  type HardwareTelemetrySnapshot,
  type ComposerUploadPreview,
  type IpcError,
  type LlmProviderConnectId,
  type LlmProviderConnectResult,
  type LlmProviderRuntimeEvent,
  type LlmProviderRuntimeSnapshot,
  type AgentRuntimeEvent,
  type PanelsChatBottomSnapshotEvent,
  type PanelsChatBottomCommand,
  type PanelsChatBottomCommandResult,
  type PanelsChatBottomSnapshot,
  type RightPanelAction,
  type RightPanelCommand,
  type RightPanelCommandResult,
  type RightPanelLine,
  type RightPanelSnapshot,
  type SidebarCommand,
  type SidebarCommandResult,
  type SidebarSessionItem,
  type SidebarSnapshot,
  type SidebarToolControl,
  type TerminalStartResult,
  type TranscriptMessage,
  type WorkspaceActionResult,
  type WorkspaceChoiceResult,
  type AgentActionCapabilityId,
  type AgentActionHostManifest,
  type AgentActionLoopOutcome,
  type AgentActionLoopState,
  type AgentActionPathEntry,
  type AgentActionRequest,
  type AgentActionResult,
  type BrainCodeActCommand,
  type NativeTerminalBounds,
  type NativeTerminalResult,
  type NativeWebExplorerBounds,
  type NativeWebExplorerCodeAct,
  type NativeDomRamArtifactSummary,
  type NativeDomRamCartographyResult,
  type NativeDomRamUiTreeNode,
  type NativeWebExplorerResult,
  type SearchArchiveRequest,
  type SearchArchiveResult,
  type BrainCanonicalMemoryResult,
  type BrainCanonicalMemorySnapshot,
  type SessionFilesSnapshot,
  type WidgetWallpaperSampleBounds,
  type WidgetWallpaperSampleResult,
  isCanvasSurfacesCommand,
  isAgentActionRequest,
  isHeaderCommand,
  isPanelsChatBottomCommand,
  isRightPanelCommand,
  isNativeSection,
  isSidebarCommand
} from "../shared/ipc-contract.js";
import {
  BRAIN_BRAIN_COMMAND,
  BRAIN_GOOGLE_AGENDA_COMMAND,
  BRAIN_NAMED_COMPUTE_COMMAND,
  BRAIN_RUST_PORT_ADAPTER_COMMAND,
  BRAIN_RUST_STATE_STORE_COMMAND,
  BRAIN_WEB_COMMAND
} from "../shared/ipc-contract.js";
import {
  agentActionCapabilityDetailManifest,
  agentActionEventCommandForRequest,
  agentActionHostPromptManifest,
  agentActionRoutingHint,
  createAgentActionHostManifest,
  executeAgentActionRequest,
  type AgentActionHostConfig
} from "./agent-action-host.js";
import {
  AGENT_ACTION_JSON_PREFIX,
  agentActionLiveVisibleText,
  extractAgentActionJsonRequest,
  removeAgentActionJsonFragment,
  removeAgentActionJsonFragments,
  type ExtractedAgentAction
} from "./agent-action-loop.js";
import {
  readSearchArchiveCodeAct,
  renderSearchArchiveResult,
  renderSearchArchiveTemplateResult,
  searchArchiveSessions,
  searchArchiveTemplateResult,
  stableSearchArchiveHash,
  archiveSessionProofHash,
  type ChatArchiveAttachment,
  type ChatArchiveMessage,
  type ChatArchiveSession,
  type ChatArchiveSessionMeta,
  markArchiveSessionArchived,
  upsertArchiveMessage
} from "./search-archive.js";
import {
  buildGoogleWebCodeActRequest,
  extractGoogleWebCodeAct,
  renderGoogleWebCodeActResult,
  type GoogleWebCodeActRequest
} from "./google-web-codeact.js";
import {
  readWebSearchCodeAct,
  renderWebSearchCodeActResult,
  renderWebSearchTemplateResult
} from "./websearch-codeact.js";
import { runWebSearchBridge } from "./websearch-bridge.js";
import {
  extractPriorityMcpCodeAct,
  renderPriorityMcpCodeActResult
} from "./priority-mcp-codeacts.js";
import {
  extractLocalActionsCodeAct,
  localActionsCodeActToAgentActionRequest,
  renderLocalActionsCodeActResult
} from "./local-actions-codeact.js";
import {
  extractDomainBrainCodeActResult,
  renderDomainBrainCodeActResult
} from "./domain-brain-codeact.js";
import { runPriorityMcpBridge } from "./priority-mcp-bridge.js";
import {
  createMapsCodeActRequest,
  extractMapsCodeAct,
  GOOGLE_EARTH_DEFAULT_URL,
  MAPS_DEFAULT_TARGET,
  renderMapsCodeActResult,
  type MapsCodeActRequest
} from "./maps-codeact.js";
import {
  extractGmailCodeAct,
  GMAIL_SIGN_IN_URL,
  gmailWebExplorerNavigationUrl,
  renderGmailCodeActResult,
  type GmailCodeActRequest
} from "./gmail-codeact.js";
import {
  AIRBNB_HOME_URL,
  extractAirbnbCodeAct,
  renderAirbnbCodeActResult,
  type AirbnbCodeActRequest
} from "./airbnb-codeact.js";
import {
  extractScrapersCodeAct,
  renderScrapersCodeActResult,
  scrapersVisualAttachments
} from "./scrapers-codeact.js";
import { runScrapersMcpBridge } from "./scrapers-mcp-bridge.js";

protocol.registerSchemesAsPrivileged([
  {
    scheme: "ingen",
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      stream: true,
      corsEnabled: true
    }
  }
]);

const currentDir = fileURLToPath(new URL(".", import.meta.url));
const shellRoot = join(currentDir, "..", "..");
const repoRoot = join(shellRoot, "..", "..");
const rendererDist = join(shellRoot, "dist", "renderer");
const eventTextLabMode = process.argv.includes("--event-text-lab") || process.env.INGEN_EVENT_TEXT_LAB === "1";
const INGEN_CANONICAL_USER_DATA_DIR_NAME = eventTextLabMode ? "InGenEventTextLabRuntime" : "InGenRuntime";
const INGEN_LEGACY_USER_DATA_DIR_NAME = "InGen";
const INGEN_MEMORY_ROOT_DIR_NAME = "brain";
const CHATGPT_HOME_URL = "https://chatgpt.com/";
const CHATGPT_LOGIN_URL = "https://chatgpt.com/auth/login?next=%2F";
const CHATGPT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
const WEBEXPLORER_DEFAULT_URL = "https://www.google.com/";
const NATIVE_WEBEXPLORER_VIEWPORT_FADE_CSS = `
html::before,
html::after {
  position: fixed !important;
  left: 0 !important;
  right: 0 !important;
  z-index: 2147483647 !important;
  height: 18px !important;
  content: "" !important;
  pointer-events: none !important;
}
html::before {
  top: 0 !important;
  background: linear-gradient(to bottom, #0e0e0f 0%, rgba(14, 14, 15, 0.76) 44%, rgba(14, 14, 15, 0) 100%) !important;
}
html::after {
  bottom: 0 !important;
  background: linear-gradient(to top, #0e0e0f 0%, rgba(14, 14, 15, 0.76) 44%, rgba(14, 14, 15, 0) 100%) !important;
}
`;
const CODEX_DESKTOP_MODELS = ["GPT-5.5", "GPT-5.4", "GPT-5.4-Mini", "GPT-5.3-Codex-Spark"];
const CODEX_DESKTOP_REASONING = ["Low", "Medium", "High", "Deep"];
const PANELS_CHAT_BOTTOM_MAX_UPLOADS = 20;
const PANELS_CHAT_BOTTOM_MAX_PROVIDER_ATTACHMENTS = 6;
const PANELS_CHAT_BOTTOM_MAX_INLINE_IMAGE_BYTES = 14 * 1024 * 1024;
const PANELS_CHAT_BOTTOM_MAX_OPENAI_FILE_BYTES = 20 * 1024 * 1024;
const PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES = 96 * 1024;
const PANELS_CHAT_BOTTOM_MAX_VISUAL_SNAPSHOTS = 6;
const PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_CUES = 160;
const TRANSPARENT_WINDOW_BACKGROUND = "#00000000";
// Keep the native widget envelope tall enough for flyouts; Electron setShape
// keeps the visible/clickable surface compact when no panel is open.
const WIDGET_WINDOW_HEIGHT = 535;
const WIDGET_WINDOW_PANEL_HEIGHT = 535;
const WIDGET_WINDOW_BOTTOM_GAP = 0;
const WIDGET_WINDOW_AUTOHIDE_BOTTOM_GAP = 2;
const WIDGET_WINDOW_SHRINK_DELAY_MS = 720;
const WIDGET_TASKBAR_SLIDE_MS = 320;
const WIDGET_WINDOW_RESTORE_MS = 560;
const WINDOWS_TASKBAR_AUTOHIDE_FLAG = 0x1;
const PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_BYTES = 48 * 1024;
const PANELS_CHAT_BOTTOM_CONTEXT_TEXT_BYTES = 24 * 1024;
const PANELS_CHAT_BOTTOM_CONTEXT_TOKEN_BUDGET = 80_000;
const PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS = 56_000;
const PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS = 12_000;
const PANELS_CHAT_BOTTOM_MEMORY_TOKEN_BUDGET = 14_000;
const PANELS_CHAT_BOTTOM_MEMORY_TEXT_BYTES = 32 * 1024;
const PANELS_CHAT_BOTTOM_DOCUMENT_MEMORY_BYTES = 48 * 1024;
const CONTEXT_COMPACTION_EVENT_COMMAND = "/context_compaction_";
const PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE = 768;
let primaryWindow: BrowserWindow | null = null;
const authWindows = new Map<LlmProviderConnectId, BrowserWindow>();
const providerAuthWatchers = new Map<LlmProviderConnectId, ReturnType<typeof setInterval>>();
let claudeProvisioningActive = false;
let codexRuntimeWindow: BrowserWindow | null = null;
type WidgetWindowRestoreState = {
  bounds: Electron.Rectangle;
  minimumSize: [number, number];
  maximized: boolean;
  fullScreen: boolean;
};
let widgetWindowRestoreState: WidgetWindowRestoreState | null = null;
let widgetWindowShrinkTimer: ReturnType<typeof setTimeout> | null = null;
let widgetTaskbarAutoHideTimer: ReturnType<typeof setTimeout> | null = null;
let widgetWindowBoundsAnimationTimer: ReturnType<typeof setInterval> | null = null;
let widgetWindowHitRegions: Electron.Rectangle[] = [];
let widgetWindowClickThrough = false;
let widgetWindowPanelExpanded = false;
let widgetTaskbarOriginalState: number | null = null;
let widgetTaskbarOriginalRegistryByte: number | null = null;
let widgetTaskbarHidden = false;
let widgetTaskbarAutoHideJobToken = 0;

function traceWidgetTaskbarStep(step: string, detail: Record<string, unknown> = {}): void {
  try {
    appendFileSync(
      join(app.getPath("userData"), "widget-taskbar.log"),
      `${JSON.stringify({ at: new Date().toISOString(), step, ...detail })}\n`,
      "utf8"
    );
  } catch {
    // Best-effort diagnostic trace only.
  }
}

let nativeWebExplorerView: BrowserView | null = null;
let nativeWebExplorerOwner: BrowserWindow | null = null;
let nativeWebExplorerLoadedUrl = "";
let nativeWebExplorerPendingUrl = "";
let nativeWebExplorerTargetUrl = WEBEXPLORER_DEFAULT_URL;
let nativeWebExplorerSessionConfigured = false;
let nativeWebExplorerBoundsKey = "";
let nativeMapsView: WebContentsView | null = null;
let nativeMapsOwner: BrowserWindow | null = null;
let nativeMapsLoadedUrl = "";
let nativeMapsPendingUrl = "";
let nativeMapsTargetUrl = GOOGLE_EARTH_DEFAULT_URL;
let nativeMapsSessionConfigured = false;
let nativeMapsBoundsKey = "";
let mapsDomWebviewGuest: Electron.WebContents | null = null;
let mapsDomWebviewGuestUrl = "";
type GoogleEarthSearchLock = {
  webContentsId: number;
  url: string;
  backendNodeId: number;
  layout?: NativeDomRamUiTreeNode["layout"];
  lockedAt: number;
};
let googleEarthSearchLock: GoogleEarthSearchLock | null = null;
const NATIVE_MAPS_EARTH_OVERSCAN_PX = {
  minLeft: 420,
  leftRatio: 0.92,
  bottom: 180
};
let nativeTerminalProcess: ReturnType<typeof spawn> | null = null;
let nativeTerminalCwd = "";
let nativeTerminalHwnd = "";
let nativeTerminalOwner: BrowserWindow | null = null;
// Active workspace folder. Defaults to the repo root; the "Choose workspace"
// breadcrumb lets the user repoint it to any folder via the native picker.
// The choice is persisted to userData so it survives app restarts.
let activeWorkspaceDir = repoRoot;
let workspaceExplicitlyChosen = false;

interface TerminalRuntimeConfig {
  command: string;
  args: string[];
  label: string;
  prompt: string;
  subtitle: string;
  cwd: string;
}

let terminalRuntime: TerminalRuntimeConfig = {
  command: process.platform === "win32" ? "powershell.exe" : process.env.SHELL || "sh",
  args: [],
  label: "Terminal",
  prompt: "$",
  subtitle: "Run shell commands.",
  cwd: repoRoot
};

function isBrokenPipeError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const record = error as { code?: unknown; message?: unknown };
  return record.code === "EPIPE" || (typeof record.message === "string" && /EPIPE|broken pipe/i.test(record.message));
}

function installMainProcessConsoleGuard(): void {
  const ignorePipeError = (error: unknown) => {
    if (isBrokenPipeError(error)) {
      return;
    }
  };
  process.stdout?.on("error", ignorePipeError);
  process.stderr?.on("error", ignorePipeError);
  for (const method of ["log", "info", "warn", "error"] as const) {
    const original = console[method].bind(console);
    console[method] = ((...args: unknown[]) => {
      try {
        original(...args);
      } catch (error) {
        // A detached launcher may close stdio; logging must never crash Electron.
        void error;
      }
    }) as Console[typeof method];
  }
}

installMainProcessConsoleGuard();

app.setName(eventTextLabMode ? "InGen Event Text Lab" : "InGen");
const canonicalUserDataDir = process.env.INGEN_ELECTRON_USER_DATA_DIR?.trim()
  || join(app.getPath("appData"), INGEN_CANONICAL_USER_DATA_DIR_NAME);
mkdirSync(canonicalUserDataDir, { recursive: true });
mkdirSync(join(canonicalUserDataDir, INGEN_MEMORY_ROOT_DIR_NAME), { recursive: true });
mkdirSync(join(canonicalUserDataDir, "session-data"), { recursive: true });
app.setPath("userData", canonicalUserDataDir);
app.setPath("sessionData", join(canonicalUserDataDir, "session-data"));
if (process.platform === "win32") {
  app.setAppUserModelId(eventTextLabMode ? "com.forge.ingen.event-text-lab" : "com.forge.ingen");
}

const bypassSingleInstanceLock = process.env.INGEN_ELECTRON_BYPASS_SINGLE_INSTANCE_LOCK === "1";
const LIVE_RENDERER_URL_ARG_PREFIX = "--ingen-live-renderer-url=";

function liveRendererUrlFromCommandLine(commandLine: readonly string[]): string | null {
  const argument = commandLine.find((entry) => entry.startsWith(LIVE_RENDERER_URL_ARG_PREFIX));
  if (!argument) {
    return null;
  }
  const url = argument.slice(LIVE_RENDERER_URL_ARG_PREFIX.length).trim();
  if (!/^http:\/\/127\.0\.0\.1:\d{2,5}(?:\/.*)?$/.test(url)) {
    console.warn("Ignoring non-local live renderer URL from launcher.", { url });
    return null;
  }
  return url;
}

function loadLiveRendererUrlIntoPrimaryWindow(url: string): boolean {
  if (!primaryWindow || primaryWindow.isDestroyed()) {
    return false;
  }
  void primaryWindow.loadURL(url).then(() => {
    restorePrimaryWindow();
  }).catch((error: unknown) => {
    console.error("Failed to switch existing InGen window to live renderer URL.", { url, error });
    restorePrimaryWindow();
  });
  return true;
}

const hasSingleInstanceLock = eventTextLabMode || bypassSingleInstanceLock || app.requestSingleInstanceLock();
if (!hasSingleInstanceLock) {
  app.quit();
} else if (!eventTextLabMode && !bypassSingleInstanceLock) {
  app.on("second-instance", (_event, commandLine) => {
    const liveRendererUrl = liveRendererUrlFromCommandLine(commandLine);
    if (liveRendererUrl && loadLiveRendererUrlIntoPrimaryWindow(liveRendererUrl)) {
      return;
    }
    restorePrimaryWindow();
  });
}

function installDiscreteGpuPreference(): void {
  process.env.FORGE_PREFERRED_GPU_VENDOR = process.env.FORGE_PREFERRED_GPU_VENDOR ?? "nvidia";

  if (process.platform === "win32") {
    const registryKey = "HKCU\\Software\\Microsoft\\DirectX\\UserGpuPreferences";
    const executablePaths = Array.from(new Set([process.execPath, app.getPath("exe")].filter(Boolean)));
    for (const executablePath of executablePaths) {
      const result = spawnSync(
        "reg.exe",
        ["add", registryKey, "/v", executablePath, "/t", "REG_SZ", "/d", "GpuPreference=2;", "/f"],
        { encoding: "utf8", stdio: "pipe", timeout: 1500, windowsHide: true }
      );
      if (result.error) {
        console.error("Windows high-performance GPU preference registration failed.", result.error);
      } else if (result.status !== 0) {
        console.error("Windows high-performance GPU preference registration failed.", {
          executablePath,
          status: result.status,
          stderr: result.stderr?.trim()
        });
      }
    }
  }

  if (process.platform === "linux") {
    process.env.__NV_PRIME_RENDER_OFFLOAD = process.env.__NV_PRIME_RENDER_OFFLOAD ?? "1";
    process.env.__VK_LAYER_NV_optimus = process.env.__VK_LAYER_NV_optimus ?? "NVIDIA_only";
    process.env.__GLX_VENDOR_LIBRARY_NAME = process.env.__GLX_VENDOR_LIBRARY_NAME ?? "nvidia";
    process.env.DRI_PRIME = process.env.DRI_PRIME ?? "1";
  }
}

installDiscreteGpuPreference();
app.commandLine.appendSwitch("ignore-gpu-blocklist");
app.commandLine.appendSwitch("force_high_performance_gpu");
app.commandLine.appendSwitch("force-high-performance-gpu");
app.commandLine.appendSwitch("gpu-preferences", "high-performance");
app.commandLine.appendSwitch("enable-gpu-rasterization");
app.commandLine.appendSwitch("enable-zero-copy");
app.commandLine.appendSwitch("enable-accelerated-2d-canvas");
app.commandLine.appendSwitch("enable-unsafe-webgpu");
app.commandLine.appendSwitch("disable-features", "CalculateNativeWinOcclusion,HardwareMediaKeyHandling");

if (process.platform === "win32") {
  app.commandLine.appendSwitch("use-angle", "d3d11");
}

interface HardwareProfile {
  cpuLabel: string;
  gpuLabel: string;
  preferredGpuLabel: string;
}

interface ComposerUploadItem extends ComposerUploadPreview {
  path: string;
  mimeType: string;
}

type ProviderVisualSnapshot = {
  label: string;
  imageUrl: string;
  source: "image" | "video-frame" | "model3d-summary";
  proofHash: string;
};

type ProviderVideoSubtitleCue = {
  start: number;
  end: number;
  text: string;
};

type ProviderVideoTextTrack = {
  index: number;
  kind: string;
  label: string;
  language: string;
  cueCount: number;
  cues: ProviderVideoSubtitleCue[];
};

type ProviderVideoMetadata = {
  durationSeconds: number;
  durationLabel: string;
  width: number;
  height: number;
  aspectRatio: number;
  resolutionLabel: string;
  qualityLabel: string;
  snapshotTimes: number[];
  textTracks: ProviderVideoTextTrack[];
  subtitlesExtracted: boolean;
};

interface ProviderAttachment extends ComposerUploadItem {
  sizeBytes: number;
  proofHash: string;
  llmTextPreview: string;
  openAiFileDataUrl?: string;
  editRole?: "editable_input";
  visualSnapshots: ProviderVisualSnapshot[];
  videoMetadata?: ProviderVideoMetadata;
}

type OpenAiResponseContentPart =
  | { type: "input_text"; text: string }
  | { type: "input_image"; image_url: string; detail?: "low" | "high" | "auto" }
  | { type: "input_file"; filename: string; file_data: string };

type OpenAiResponseInputItem =
  | { role: "system"; content: string }
  | { role: "user"; content: OpenAiResponseContentPart[] }
  | { role: "assistant"; content: string };

type OpenRouterContentPart =
  | { type: "text"; text: string }
  | { type: "image_url"; image_url: { url: string } };

type OpenRouterMessage = {
  role: "system" | "user" | "assistant";
  content: string | OpenRouterContentPart[];
};

let hardwareProfile: HardwareProfile = {
  cpuLabel: compactHardwareName(cpus()[0]?.model ?? "CPU") || "CPU",
  gpuLabel: "detecting",
  preferredGpuLabel: "detecting"
};
let hardwareProfileRefreshPromise: Promise<void> | null = null;
let hardwareProfileLastRefreshAt = 0;

function rustBackend(): RustBackendProjection {
  const projection = cachedRustBackendProjection();
  if (!projection) {
    throw new Error("Rust backend projection is not loaded.");
  }
  return projection;
}

function compactHardwareName(value: string): string {
  return value.replace(/\s+/g, " ").replace(/\(R\)|\(TM\)|CPU|GPU/gi, "").trim();
}

function vendorName(vendorId: number | undefined): string {
  switch (vendorId) {
    case 0x10de:
      return "NVIDIA";
    case 0x1002:
    case 0x1022:
      return "AMD";
    case 0x8086:
      return "Intel";
    case 0x106b:
      return "Apple";
    default:
      return "";
  }
}

function gpuScore(name: string): number {
  const lower = name.toLowerCase();
  if (lower.includes("nvidia") || lower.includes("geforce") || lower.includes("rtx") || lower.includes("quadro")) {
    return 0;
  }
  if (lower.includes("amd") || lower.includes("radeon")) {
    return 1;
  }
  if (lower.includes("apple")) {
    return 2;
  }
  if (lower.includes("intel")) {
    return 3;
  }
  return 4;
}

function readStringField(source: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function extractGpuNames(gpuInfo: unknown): string[] {
  const root = gpuInfo && typeof gpuInfo === "object" ? (gpuInfo as Record<string, unknown>) : {};
  const devices = Array.isArray(root.gpuDevice) ? root.gpuDevice : Array.isArray(root.gpuDevices) ? root.gpuDevices : [];
  const names = new Set<string>();
  for (const device of devices) {
    if (!device || typeof device !== "object") {
      continue;
    }
    const record = device as Record<string, unknown>;
    const vendorId = typeof record.vendorId === "number" ? record.vendorId : undefined;
    const vendor = readStringField(record, ["vendorString", "vendor", "vendorName"]) || vendorName(vendorId);
    const model = readStringField(record, ["deviceString", "deviceName", "name", "description"]);
    const label = compactHardwareName(`${vendor} ${model}`.trim());
    if (label) {
      names.add(label);
    }
  }

  const aux = root.auxAttributes && typeof root.auxAttributes === "object" ? (root.auxAttributes as Record<string, unknown>) : {};
  for (const key of ["glRenderer", "glVendor", "displayType"]) {
    const label = readStringField(aux, [key]);
    if (label && !/swiftshader|software/i.test(label)) {
      names.add(compactHardwareName(label));
    }
  }

  return [...names].sort((a, b) => gpuScore(a) - gpuScore(b) || a.localeCompare(b));
}

async function refreshHardwareProfile(): Promise<void> {
  const now = Date.now();
  if (hardwareProfileRefreshPromise) {
    return hardwareProfileRefreshPromise;
  }
  if (hardwareProfileLastRefreshAt && now - hardwareProfileLastRefreshAt < 30_000) {
    return;
  }
  hardwareProfileRefreshPromise = refreshHardwareProfileInner().finally(() => {
    hardwareProfileRefreshPromise = null;
    hardwareProfileLastRefreshAt = Date.now();
  });
  return hardwareProfileRefreshPromise;
}

async function refreshHardwareProfileInner(): Promise<void> {
  const cpuModel = compactHardwareName(cpus()[0]?.model ?? "CPU");
  let gpuNames: string[] = [];
  try {
    const infoType = process.env.INGEN_GPU_INFO_COMPLETE === "1" ? "complete" : "basic";
    gpuNames = extractGpuNames(await app.getGPUInfo(infoType));
  } catch (error) {
    console.error("GPU profile detection failed.", error);
  }
  const preferredGpuLabel = gpuNames[0] ?? "GPU unavailable";
  const gpuLabel = gpuNames.length > 0 ? `preferred: ${preferredGpuLabel} | all: ${gpuNames.join(" + ")}` : "GPU unavailable";
  hardwareProfile = {
    cpuLabel: cpuModel || "CPU",
    gpuLabel,
    preferredGpuLabel
  };
  console.info("[InGen hardware] GPU profile", hardwareProfile.gpuLabel);
}

let headerState: Pick<
  HeaderSnapshot,
  "activeSection" | "sectionTitle" | "profileCanvas" | "leftPanelOpen" | "rightPanelOpen"
> = {
  activeSection: "forge",
  sectionTitle: "Forge",
  profileCanvas: "",
  leftPanelOpen: true,
  rightPanelOpen: false
};

interface SidebarState {
  activeDrawer: string;
  profileOpen: boolean;
  sessionsMenuMode: "recents" | "archived";
  recentSessionId: string;
  archivedSessionId: string;
  hiddenTools: string[];
  pinnedSession: {
    label: string;
    section: SidebarSessionItem["section"];
    working: boolean;
    automated: boolean;
  };
  archiveConfirm: SidebarSnapshot["archiveConfirm"];
  lastControl: string;
}

let sidebarState: SidebarState = {
  activeDrawer: "",
  profileOpen: false,
  sessionsMenuMode: "recents",
  recentSessionId: "",
  archivedSessionId: "",
  hiddenTools: [],
  pinnedSession: {
    label: "Electron cutover",
    section: "forge",
    working: false,
    automated: false
  },
  archiveConfirm: {
    open: false,
    candidateId: "",
    candidateLabel: "",
    candidateDate: "",
    candidateSection: "forge"
  },
  lastControl: "boot"
};

const seededSessions: SidebarSessionItem[] = [
  { sessionId: "native-front-migration", label: "Electron cutover", date: "2026-06-09", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: true, working: false, automated: false, archived: false },
  { sessionId: "test-session-example", label: "test session example", date: "2026-06-10", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: true, automated: false, archived: false },
  { sessionId: "", label: "LLM Act Codes discovery methods", date: "2026-06-08", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: true, archived: false },
  { sessionId: "", label: "PoolClaw agent profiles", date: "2026-06-07", section: "forge", workspaceLabel: "PoolClaw", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Banger create 3D object", date: "2026-06-07", section: "banger", workspaceLabel: "Banger", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Google Drive module map", date: "2026-06-07", section: "webexplorer", workspaceLabel: "WebExplorer", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Trading alert console", date: "2026-06-06", section: "trading", workspaceLabel: "Forge Trading", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Monster compute proof", date: "2026-06-06", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "My Assets import pass", date: "2026-06-05", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Pool invite flow", date: "2026-06-05", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Web peripheral snapshot", date: "2026-06-04", section: "webexplorer", workspaceLabel: "WebExplorer", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Forge UI harmonization", date: "2026-06-04", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
  { sessionId: "", label: "Automation queue sketch", date: "2026-06-03", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false }
];

const localChatSessions: SidebarSessionItem[] = [];
const chatArchiveSessions = new Map<string, ChatArchiveSession>();
let chatArchiveLoaded = false;
let chatArchiveLoadPromise: Promise<void> | undefined;
let chatArchiveWriteQueue: Promise<void> = Promise.resolve();

type BrainSegmentId = "general" | "science" | "coding";
type ActiveBrainSegmentId = Exclude<BrainSegmentId, "general">;

let panelsChatBottomState = {
  chatText: "",
  composerResetRequestTime: 0,
  permissionMode: "ask-permissions" as PanelsChatBottomSnapshot["composer"]["permissionMode"],
  permissionModeOpen: false,
  selectedProvider: "openai" as PanelsChatBottomSnapshot["composer"]["selectedProvider"],
  modelIndex: 0,
  reasoningIndex: 1,
  uploadItems: [] as ComposerUploadItem[],
  uploadCount: 0,
  uploadErrorText: "",
  uploadEditTargetId: "",
  lastControl: "boot",
  activeBrainSegment: "general" as BrainSegmentId,
  activeSessionId: "",
  transcript: [] as TranscriptMessage[]
};
const parallelChatLanes = new Map<number, { sessionId: string; transcript: TranscriptMessage[]; groupId: string; focusMessageId?: string; sessionTitle?: string }>();
const composerUploadPreviewItems = new Map<string, ComposerUploadItem>();
const providerAttachmentCache = new Map<string, ProviderAttachment>();

function panelsChatBottomCommandTime(command: PanelsChatBottomCommand): number {
  const timestamp = command.requestId.split("-")[1] ?? "";
  const parsed = Number.parseInt(timestamp, 36);
  return Number.isFinite(parsed) ? parsed : Date.now();
}

type ComposerProviderId = PanelsChatBottomSnapshot["composer"]["selectedProvider"];
type ProviderRuntimeProfile = {
  connectId: LlmProviderConnectId;
  composerProvider: ComposerProviderId;
  label: string;
  account: string;
  connected: boolean;
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  proof: string;
  events: string[];
  runtimeCommand?: string;
  runtimeVersion?: string;
  runtimeVerified?: boolean;
};

type ModelCatalogProbe = {
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  event: string;
  proof: string;
};

type StoredProviderProfile = {
  connected: boolean;
  account: string;
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  proof: string;
  events?: string[];
  updatedAt: string;
};

type StoredProviderSecret = {
  apiKey?: string;
};

type StoredProviderRuntime = {
  schema: "ingen.electron.llm_provider_runtime.v1";
  providers: Partial<Record<LlmProviderConnectId, StoredProviderProfile>>;
  secrets?: Partial<Record<LlmProviderConnectId, StoredProviderSecret>>;
};

type EncryptedProviderRuntimeEnvelope = {
  schema: "ingen.electron.llm_provider_runtime.encrypted.v1";
  cipher: "electron.safeStorage";
  encoding: "base64";
  storageBackend?: string;
  ciphertext: string;
  updatedAt: string;
};

const providerRuntime: Record<LlmProviderConnectId, ProviderRuntimeProfile> = {
  codex: {
    connectId: "codex",
    composerProvider: "openai",
    label: "Codex",
    account: "local Codex auth",
    connected: false,
    models: [],
    reasoning: [],
    quotaLabel: "quota unavailable: official token balance not returned",
    proof: "provider-store",
    events: ["awaiting secure login"]
  },
  claude: {
    connectId: "claude",
    composerProvider: "anthropic",
    label: "Claude",
    account: "Claude Code OAuth",
    connected: false,
    models: ["claude-opus-4.5", "claude-sonnet-4.5", "claude-haiku-4.5"],
    reasoning: ["Normal", "Extended", "Max"],
    quotaLabel: "quota sync pending",
    proof: "eve-reader-pending",
    events: ["awaiting secure login"]
  },
  openrouter: {
    connectId: "openrouter",
    composerProvider: "openrouter",
    label: "OpenRouter",
    account: "OpenRouter OAuth",
    connected: false,
    models: [],
    reasoning: [],
    quotaLabel: "quota unavailable: official credits not returned",
    proof: "oauth-key-store",
    events: ["awaiting secure login"]
  }
};

let rightPanelState = {
  activeTab: "status",
  lastControl: "boot"
};
let openRouterApiKey = "";
let openRouterOAuthServer: Server | null = null;

function llmProviderRuntimeStorePath(): string {
  return join(app.getPath("userData"), "llm-provider-runtime.json");
}

async function isProviderRuntimeEncryptionAvailable(): Promise<boolean> {
  try {
    if (await safeStorage.isAsyncEncryptionAvailable()) {
      return true;
    }
  } catch {
    // Fall back to the synchronous availability probe below.
  }
  return safeStorage.isEncryptionAvailable();
}

function hashJson(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

type CpuTimes = ReturnType<typeof cpus>[number]["times"];

let previousCpuTimes: CpuTimes[] | null = null;
const NVIDIA_SMI_TIMEOUT_MS = 900;
type HardwareThermalSource = HardwareTelemetrySnapshot["thermal"]["source"];
type TemperatureReading = { label: string; value: number; source: HardwareThermalSource };
type ThermalTemperatures = { cpu: number | null; system: number | null; source: HardwareThermalSource };
const HARDWARE_GPU_REFRESH_MS = 250;
const HARDWARE_THERMAL_REFRESH_MS = 1000;
let cachedNvidiaGpus: { gpus: HardwareGpuSnapshot[]; sampledAt: number } | null = null;
let nvidiaGpuRefreshScheduled = false;
let cachedThermalTemperatures: (ThermalTemperatures & { sampledAt: number }) | null = null;
let thermalRefreshPromise: Promise<ThermalTemperatures> | null = null;

function hardwareMetric(
  label: string,
  value: number | null,
  unit: HardwareMetric["unit"],
  status: HardwareMetric["status"] = value === null ? "unavailable" : "ok"
): HardwareMetric {
  return { label, value, unit, status };
}

function metricStatusPercent(value: number | null, warning: number, critical: number): HardwareMetric["status"] {
  if (value === null) return "unavailable";
  if (value >= critical) return "critical";
  if (value >= warning) return "warning";
  return "ok";
}

function metricStatusTemperature(value: number | null): HardwareMetric["status"] {
  if (value === null) return "unavailable";
  if (value >= 90) return "critical";
  if (value >= 78) return "warning";
  return "ok";
}

function roundMetric(value: number | null, digits = 1): number | null {
  if (value === null || !Number.isFinite(value)) return null;
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

function cpuUtilizationPercent(): number | null {
  const current = cpus().map((cpu) => cpu.times);
  const previous = previousCpuTimes;
  previousCpuTimes = current;
  if (!previous || previous.length !== current.length) {
    return null;
  }
  let idleDelta = 0;
  let totalDelta = 0;
  for (let index = 0; index < current.length; index += 1) {
    const now = current[index];
    const before = previous[index];
    const idle = Math.max(0, now.idle - before.idle);
    const total =
      Math.max(0, now.user - before.user) +
      Math.max(0, now.nice - before.nice) +
      Math.max(0, now.sys - before.sys) +
      Math.max(0, now.irq - before.irq) +
      idle;
    idleDelta += idle;
    totalDelta += total;
  }
  if (totalDelta <= 0) {
    return null;
  }
  return roundMetric(Math.max(0, Math.min(100, (1 - idleDelta / totalDelta) * 100)));
}

function parseNumber(value: string): number | null {
  const parsed = Number.parseFloat(value.trim());
  return Number.isFinite(parsed) ? parsed : null;
}

function nvidiaSmiCandidates(): string[] {
  const candidates = ["nvidia-smi"];
  if (process.platform === "win32") {
    const programRoots = [
      process.env.ProgramW6432,
      process.env.ProgramFiles,
      process.env["ProgramFiles(x86)"]
    ].filter((value): value is string => typeof value === "string" && value.trim().length > 0);
    for (const root of programRoots) {
      candidates.push(join(root, "NVIDIA Corporation", "NVSMI", "nvidia-smi.exe"));
    }
    candidates.push("C:\\Windows\\System32\\nvidia-smi.exe");
  }
  return Array.from(new Set(candidates));
}

function runNvidiaSmiQuery(args: string[]): string {
  for (const candidate of nvidiaSmiCandidates()) {
    if (/^[A-Za-z]:\\/.test(candidate) && !existsSync(candidate)) {
      continue;
    }
    const result = spawnSync(candidate, args, {
      encoding: "utf8",
      stdio: "pipe",
      timeout: NVIDIA_SMI_TIMEOUT_MS,
      windowsHide: true
    });
    if (result.status === 0 && result.stdout.trim()) {
      return result.stdout;
    }
  }
  return "";
}

function emptyGpu(source: HardwareGpuSnapshot["source"] = "unavailable"): HardwareGpuSnapshot {
  return {
    name: "GPU unavailable",
    vendor: "unknown",
    source,
    utilization: hardwareMetric("GPU load", null, "%"),
    memoryUsed: hardwareMetric("VRAM used", null, "GB"),
    memoryTotal: hardwareMetric("VRAM total", null, "GB"),
    temperature: hardwareMetric("GPU temperature", null, "C"),
    fanSpeed: hardwareMetric("Fan speed", null, "%"),
    powerDraw: hardwareMetric("Power draw", null, "W")
  };
}

function queryNvidiaGpusUncached(): HardwareGpuSnapshot[] {
  const stdout = runNvidiaSmiQuery([
      "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,fan.speed",
      "--format=csv,noheader,nounits"
    ]);
  if (!stdout.trim()) {
    return [];
  }
  return stdout
    .trim()
    .split(/\r?\n/)
    .map((line): HardwareGpuSnapshot => {
      const [name = "NVIDIA GPU", utilRaw = "", usedRaw = "", totalRaw = "", tempRaw = "", powerRaw = "", fanRaw = ""] = line
        .split(",")
        .map((part) => part.trim());
      const utilization = parseNumber(utilRaw);
      const memoryUsed = parseNumber(usedRaw);
      const memoryTotal = parseNumber(totalRaw);
      const memoryUsedGb = memoryUsed === null ? null : memoryUsed / 1024;
      const memoryTotalGb = memoryTotal === null ? null : memoryTotal / 1024;
      const temperature = parseNumber(tempRaw);
      const fanSpeed = parseNumber(fanRaw);
      return {
        name,
        vendor: "nvidia",
        source: "nvidia-smi",
        utilization: hardwareMetric("GPU load", roundMetric(utilization), "%", metricStatusPercent(utilization, 82, 94)),
        memoryUsed: hardwareMetric("VRAM used", roundMetric(memoryUsedGb, 2), "GB"),
        memoryTotal: hardwareMetric("VRAM total", roundMetric(memoryTotalGb, 2), "GB"),
        temperature: hardwareMetric("GPU temperature", roundMetric(temperature), "C", metricStatusTemperature(temperature)),
        fanSpeed: hardwareMetric("Fan speed", roundMetric(fanSpeed), "%", metricStatusPercent(fanSpeed, 80, 95)),
        powerDraw: hardwareMetric("Power draw", roundMetric(parseNumber(powerRaw)), "W")
      };
    });
}

function scheduleNvidiaGpuRefresh(): void {
  if (nvidiaGpuRefreshScheduled) return;
  nvidiaGpuRefreshScheduled = true;
  setTimeout(() => {
    try {
      cachedNvidiaGpus = {
        gpus: queryNvidiaGpusUncached(),
        sampledAt: Date.now()
      };
    } catch (error) {
      console.warn("Hardware NVIDIA refresh failed.", error);
    } finally {
      nvidiaGpuRefreshScheduled = false;
    }
  }, 0);
}

function queryNvidiaGpusCached(): HardwareGpuSnapshot[] {
  const now = Date.now();
  if (cachedNvidiaGpus && now - cachedNvidiaGpus.sampledAt <= HARDWARE_GPU_REFRESH_MS) {
    return cachedNvidiaGpus.gpus;
  }
  scheduleNvidiaGpuRefresh();
  return cachedNvidiaGpus?.gpus ?? [];
}

function vendorFromGpuName(name: string): HardwareGpuSnapshot["vendor"] {
  const normalized = name.toLowerCase();
  if (normalized.includes("nvidia") || normalized.includes("nouveau")) return "nvidia";
  if (normalized.includes("amd") || normalized.includes("radeon") || normalized.includes("amdgpu")) return "amd";
  if (normalized.includes("intel") || normalized.includes("i915") || normalized === "xe") return "intel";
  if (normalized.includes("apple")) return "apple";
  return "unknown";
}

function queryWindowsVideoControllers(): HardwareGpuSnapshot[] {
  if (process.platform !== "win32") {
    return [];
  }
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-Command",
      "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterCompatibility,AdapterRAM | ConvertTo-Json -Compress"
    ],
    { encoding: "utf8", stdio: "pipe", timeout: 3500, windowsHide: true }
  );
  if (result.status !== 0 || !result.stdout.trim()) {
    return [];
  }
  try {
    const parsed = JSON.parse(result.stdout) as unknown;
    const controllers = Array.isArray(parsed) ? parsed : [parsed];
    return controllers
      .map((controller): HardwareGpuSnapshot | null => {
        if (!controller || typeof controller !== "object") {
          return null;
        }
        const record = controller as { Name?: unknown; AdapterRAM?: unknown };
        const name = typeof record.Name === "string" && record.Name.trim() ? record.Name.trim() : "Windows GPU";
        const adapterRam = typeof record.AdapterRAM === "number" && Number.isFinite(record.AdapterRAM) ? record.AdapterRAM : null;
        const memoryTotalGb = adapterRam === null || adapterRam <= 0 ? null : adapterRam / 1024 ** 3;
        return {
          name,
          vendor: vendorFromGpuName(name),
          source: "system",
          utilization: hardwareMetric("GPU load", null, "%"),
          memoryUsed: hardwareMetric("VRAM used", null, "GB"),
          memoryTotal: hardwareMetric("VRAM total", roundMetric(memoryTotalGb, 2), "GB"),
          temperature: hardwareMetric("GPU temperature", null, "C"),
          fanSpeed: hardwareMetric("Fan speed", null, "%"),
          powerDraw: hardwareMetric("Power draw", null, "W")
        };
      })
      .filter((gpu): gpu is HardwareGpuSnapshot => gpu !== null);
  } catch {
    return [];
  }
}

async function readFirstNumericFile(paths: string[], radix: 10 | 16 = 10): Promise<number | null> {
  for (const filePath of paths) {
    try {
      const raw = (await readFile(filePath, "utf8")).trim();
      const value = radix === 16 ? Number.parseInt(raw.replace(/^0x/i, ""), 16) : Number.parseFloat(raw);
      if (Number.isFinite(value)) {
        return value;
      }
    } catch {
      // Try the next hardware probe path.
    }
  }
  return null;
}

async function readOptionalTextFile(filePath: string): Promise<string | null> {
  try {
    const value = (await readFile(filePath, "utf8")).trim();
    return value.length > 0 ? value : null;
  } catch {
    return null;
  }
}

function linuxMillidegreesToCelsius(value: number | null): number | null {
  if (value === null) return null;
  return roundMetric(Math.abs(value) > 1000 ? value / 1000 : value);
}

function linuxMicrowattsToWatts(value: number | null): number | null {
  if (value === null) return null;
  return roundMetric(value / 1_000_000);
}

function validTemperatureCelsius(value: number | null): number | null {
  if (value === null || !Number.isFinite(value) || value < -40 || value > 130) return null;
  return roundMetric(value);
}

function normalizeWindowsTemperature(value: number | null): number | null {
  if (value === null || !Number.isFinite(value) || value <= 0) return null;
  const celsius = value > 1000 ? value / 10 - 273.15 : value > 200 ? value - 273.15 : value;
  return validTemperatureCelsius(celsius);
}

interface LinuxHwmonProbe {
  name: string;
  temperatures: Array<{ label: string; value: number }>;
  fanRpm: number | null;
  powerWatts: number | null;
}

async function queryLinuxHwmonProbes(): Promise<LinuxHwmonProbe[]> {
  if (process.platform !== "linux") {
    return [];
  }
  try {
    const { readdir } = await import("node:fs/promises");
    const entries = await readdir("/sys/class/hwmon", { withFileTypes: true });
    const probes: LinuxHwmonProbe[] = [];
    for (const entry of entries.filter((item) => item.isDirectory() || item.isSymbolicLink())) {
      const base = `/sys/class/hwmon/${entry.name}`;
      const files = await readdir(base).catch(() => []);
      const name = (await readOptionalTextFile(`${base}/name`)) ?? entry.name;
      const temperatures: LinuxHwmonProbe["temperatures"] = [];
      for (const file of files.filter((candidate) => /^temp\d+_input$/.test(candidate))) {
        const index = file.match(/^temp(\d+)_input$/)?.[1];
        if (!index) continue;
        const value = linuxMillidegreesToCelsius(await readFirstNumericFile([`${base}/${file}`]));
        if (value === null || value < -40 || value > 130) continue;
        const label = (await readOptionalTextFile(`${base}/temp${index}_label`)) ?? `${name} temp ${index}`;
        temperatures.push({ label, value });
      }
      const fanValues = await Promise.all(
        files
          .filter((candidate) => /^fan\d+_input$/.test(candidate))
          .map((file) => readFirstNumericFile([`${base}/${file}`]))
      );
      const fanRpm = fanValues.find((value): value is number => value !== null && value >= 0) ?? null;
      const powerValues = await Promise.all(
        files
          .filter((candidate) => /^power\d+_(input|average)$/.test(candidate))
          .map((file) => readFirstNumericFile([`${base}/${file}`]))
      );
      const powerWatts = linuxMicrowattsToWatts(powerValues.find((value): value is number => value !== null && value > 0) ?? null);
      if (temperatures.length > 0 || fanRpm !== null || powerWatts !== null) {
        probes.push({ name, temperatures, fanRpm, powerWatts });
      }
    }
    return probes;
  } catch {
    return [];
  }
}

function linuxHwmonLooksLikeGpu(name: string): boolean {
  const normalized = name.toLowerCase();
  return ["amdgpu", "nouveau", "nvidia", "i915", "xe"].some((token) => normalized.includes(token));
}

function linuxHwmonLooksLikeCpu(name: string): boolean {
  const normalized = name.toLowerCase();
  return ["coretemp", "k10temp", "zenpower", "cpu", "x86_pkg_temp", "acpitz"].some((token) => normalized.includes(token));
}

function temperatureLooksLikeCpu(label: string): boolean {
  return /cpu|package|core|ccd|tctl|tdie|ryzen|intel|amd/i.test(label);
}

function temperatureLooksLikeSystem(label: string): boolean {
  return /system|motherboard|mainboard|board|ambient|acpi|thermal|tz\d*/i.test(label);
}

async function queryLinuxHwmonGpus(): Promise<HardwareGpuSnapshot[]> {
  const probes = await queryLinuxHwmonProbes();
  return probes
    .filter((probe) => linuxHwmonLooksLikeGpu(probe.name))
    .map((probe): HardwareGpuSnapshot => {
      const temperature = probe.temperatures.length > 0 ? Math.max(...probe.temperatures.map((item) => item.value)) : null;
      return {
        name: `${probe.name} sensor`,
        vendor: vendorFromGpuName(probe.name),
        source: "linux-drm",
        utilization: hardwareMetric("GPU load", null, "%"),
        memoryUsed: hardwareMetric("VRAM used", null, "GB"),
        memoryTotal: hardwareMetric("VRAM total", null, "GB"),
        temperature: hardwareMetric("GPU temperature", temperature, "C", metricStatusTemperature(temperature)),
        fanSpeed: hardwareMetric("Fan speed", roundMetric(probe.fanRpm), "RPM"),
        powerDraw: hardwareMetric("Power draw", probe.powerWatts, "W")
      };
    });
}

function mergeGpuSensorFallbacks(
  primary: HardwareGpuSnapshot[],
  supplemental: HardwareGpuSnapshot[]
): HardwareGpuSnapshot[] {
  if (primary.length === 0) return supplemental;
  if (supplemental.length === 0) return primary;
  return primary.map((gpu, index) => {
    const fallback =
      supplemental.find((candidate) => candidate.vendor !== "unknown" && candidate.vendor === gpu.vendor) ??
      supplemental[index] ??
      null;
    if (!fallback) return gpu;
    return {
      ...gpu,
      temperature: gpu.temperature.value === null ? fallback.temperature : gpu.temperature,
      fanSpeed: gpu.fanSpeed.value === null ? fallback.fanSpeed : gpu.fanSpeed,
      powerDraw: gpu.powerDraw.value === null ? fallback.powerDraw : gpu.powerDraw
    };
  });
}

async function queryLinuxDrmGpu(): Promise<HardwareGpuSnapshot[]> {
  if (process.platform !== "linux") {
    return [];
  }
  try {
    const { readdir } = await import("node:fs/promises");
    const entries = await readdir("/sys/class/drm", { withFileTypes: true });
    const cards = entries.filter((entry) => entry.isSymbolicLink() || entry.isDirectory()).map((entry) => entry.name).filter((name) => /^card\d+$/.test(name));
    const gpus: HardwareGpuSnapshot[] = [];
    for (const card of cards.slice(0, 4)) {
      const base = `/sys/class/drm/${card}/device`;
      const busy = await readFirstNumericFile([`${base}/gpu_busy_percent`]);
      const vramTotal = await readFirstNumericFile([`${base}/mem_info_vram_total`]);
      const vramUsed = await readFirstNumericFile([`${base}/mem_info_vram_used`]);
      const vendorRaw = await readFirstNumericFile([`${base}/vendor`], 16);
      const vendor = vendorRaw === 0x1002 ? "amd" : vendorRaw === 0x8086 ? "intel" : "unknown";
      if (busy === null && vramTotal === null && vramUsed === null) {
        continue;
      }
      gpus.push({
        name: card,
        vendor,
        source: "linux-drm",
        utilization: hardwareMetric("GPU load", roundMetric(busy), "%", metricStatusPercent(busy, 82, 94)),
        memoryUsed: hardwareMetric("VRAM used", roundMetric(vramUsed === null ? null : vramUsed / 1024 ** 3, 2), "GB"),
        memoryTotal: hardwareMetric("VRAM total", roundMetric(vramTotal === null ? null : vramTotal / 1024 ** 3, 2), "GB"),
        temperature: hardwareMetric("GPU temperature", null, "C"),
        fanSpeed: hardwareMetric("Fan speed", null, "%"),
        powerDraw: hardwareMetric("Power draw", null, "W")
      });
    }
    return gpus;
  } catch {
    return [];
  }
}

async function queryLinuxThermalTemperatures(): Promise<{ cpu: number | null; system: number | null; source: HardwareThermalSource }> {
  if (process.platform !== "linux") {
    return { cpu: null, system: null, source: "unavailable" };
  }
  const hwmonProbes = await queryLinuxHwmonProbes();
  const cpuTemperatures: TemperatureReading[] = hwmonProbes
    .filter((probe) => linuxHwmonLooksLikeCpu(probe.name))
    .flatMap((probe) => probe.temperatures.map((temperature) => ({
      label: `${probe.name} ${temperature.label}`,
      value: temperature.value,
      source: "linux-thermal" as const
    })));
  const allTemperatures: TemperatureReading[] = hwmonProbes.flatMap((probe) =>
    probe.temperatures.map((temperature) => ({
      label: `${probe.name} ${temperature.label}`,
      value: temperature.value,
      source: "linux-thermal" as const
    }))
  );
  let cpu: number | null = null;
  if (cpuTemperatures.length > 0) {
    const packageTemperatures = cpuTemperatures.filter((temperature) =>
      temperatureLooksLikeCpu(temperature.label)
    );
    const selected = packageTemperatures.length > 0 ? packageTemperatures : cpuTemperatures;
    cpu = validTemperatureCelsius(Math.max(...selected.map((temperature) => temperature.value)));
  }
  let system: number | null = null;
  const systemTemperatures = allTemperatures.filter((temperature) =>
    temperatureLooksLikeSystem(temperature.label) && !temperatureLooksLikeCpu(temperature.label)
  );
  if (systemTemperatures.length > 0) {
    system = validTemperatureCelsius(Math.max(...systemTemperatures.map((temperature) => temperature.value)));
  }
  try {
    const { readdir } = await import("node:fs/promises");
    const zones = await readdir("/sys/class/thermal", { withFileTypes: true });
    for (const zone of zones.filter((entry) => /^thermal_zone\d+$/.test(entry.name))) {
      const value = await readFirstNumericFile([`/sys/class/thermal/${zone.name}/temp`]);
      const temperature = validTemperatureCelsius(value !== null && Math.abs(value) > 1000 ? value / 1000 : value);
      if (temperature !== null) {
        system = system === null ? temperature : Math.max(system, temperature);
      }
    }
  } catch {
    // Thermal zones are optional on many Linux systems.
  }
  if (system === null) {
    system = cpu;
  }
  return { cpu, system, source: cpu !== null || system !== null ? "linux-thermal" : "unavailable" };
}

function parseJsonArray(stdout: string): unknown[] {
  if (!stdout.trim()) return [];
  const parsed = JSON.parse(stdout) as unknown;
  return Array.isArray(parsed) ? parsed : [parsed];
}

function runWindowsTemperaturePowerShell(command: string, timeout = 3500): unknown[] {
  if (process.platform !== "win32") {
    return [];
  }
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-Command",
      command
    ],
    { encoding: "utf8", stdio: "pipe", timeout, windowsHide: true }
  );
  if (result.status !== 0 || !result.stdout.trim()) {
    return [];
  }
  try {
    return parseJsonArray(result.stdout);
  } catch {
    return [];
  }
}

function queryWindowsHardwareMonitorTemperatures(): TemperatureReading[] {
  const records = runWindowsTemperaturePowerShell(`
    $items = @();
    foreach ($ns in @('root/LibreHardwareMonitor','root/OpenHardwareMonitor')) {
      try {
        $items += Get-CimInstance -Namespace $ns -ClassName Sensor -ErrorAction Stop |
          Where-Object { $_.SensorType -eq 'Temperature' -and $_.Value -ne $null } |
          Select-Object @{Name='Namespace';Expression={$ns}},Name,Identifier,Parent,Value
      } catch {}
    }
    $items | ConvertTo-Json -Compress
  `);
  return records
    .map((record): TemperatureReading | null => {
      if (!record || typeof record !== "object") return null;
      const item = record as { Name?: unknown; Identifier?: unknown; Parent?: unknown; Value?: unknown };
      const value = typeof item.Value === "number" ? validTemperatureCelsius(item.Value) : null;
      if (value === null) return null;
      const label = [item.Name, item.Identifier, item.Parent]
        .filter((part): part is string => typeof part === "string" && part.trim().length > 0)
        .join(" ");
      return { label, value, source: "windows-wmi-sensor" };
    })
    .filter((reading): reading is TemperatureReading => reading !== null);
}

function queryWindowsThermalZoneTemperatures(): TemperatureReading[] {
  const perfRecords = runWindowsTemperaturePowerShell(`
    $items = @();
    try {
      $items += Get-CimInstance -Namespace root/cimv2 -ClassName Win32_PerfFormattedData_Counters_ThermalZoneInformation -ErrorAction Stop |
        Select-Object @{Name='Source';Expression={'perf'}},Name,Temperature
    } catch {}
    try {
      $items += Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature -ErrorAction Stop |
        Select-Object @{Name='Source';Expression={'acpi'}},@{Name='Name';Expression={$_.InstanceName}},@{Name='Temperature';Expression={$_.CurrentTemperature}}
    } catch {}
    $items | ConvertTo-Json -Compress
  `);
  return perfRecords
    .map((record): TemperatureReading | null => {
      if (!record || typeof record !== "object") return null;
      const item = record as { Name?: unknown; Temperature?: unknown };
      const raw = typeof item.Temperature === "number" ? item.Temperature : null;
      const value = normalizeWindowsTemperature(raw);
      if (value === null) return null;
      const label = typeof item.Name === "string" && item.Name.trim() ? item.Name.trim() : "Windows thermal zone";
      return { label, value, source: "windows-acpi" };
    })
    .filter((reading): reading is TemperatureReading => reading !== null);
}

function queryWindowsThermalTemperatures(): { cpu: number | null; system: number | null; source: HardwareThermalSource } {
  if (process.platform !== "win32") {
    return { cpu: null, system: null, source: "unavailable" };
  }
  const sensorReadings = queryWindowsHardwareMonitorTemperatures();
  const zoneReadings = queryWindowsThermalZoneTemperatures();
  const cpuReadings = sensorReadings.filter((reading) => temperatureLooksLikeCpu(reading.label));
  const systemReadings = [
    ...sensorReadings.filter((reading) => temperatureLooksLikeSystem(reading.label) && !temperatureLooksLikeCpu(reading.label)),
    ...zoneReadings
  ];
  const cpu =
    cpuReadings.length > 0
      ? validTemperatureCelsius(Math.max(...cpuReadings.map((reading) => reading.value)))
      : zoneReadings.length > 0
        ? validTemperatureCelsius(Math.max(...zoneReadings.map((reading) => reading.value)))
        : null;
  const system =
    systemReadings.length > 0
      ? validTemperatureCelsius(Math.max(...systemReadings.map((reading) => reading.value)))
      : cpu;
  const source: HardwareThermalSource =
    cpuReadings.length > 0 || sensorReadings.length > 0
      ? "windows-wmi-sensor"
      : zoneReadings.length > 0
        ? "windows-acpi"
        : "unavailable";
  return { cpu, system, source };
}

function queryMacosCpuTemperature(): number | null {
  if (process.platform !== "darwin") {
    return null;
  }
  const result = spawnSync("/usr/bin/powermetrics", ["--samplers", "smc", "-n", "1", "-i", "1000"], {
    encoding: "utf8",
    stdio: "pipe",
    timeout: 3500,
    windowsHide: true
  });
  const output = `${result.stdout}\n${result.stderr}`;
  const matches = Array.from(output.matchAll(/(?:CPU|processor)[^:\n]*(?:die|proximity|temp|temperature)[^:\n]*:\s*([0-9]+(?:\.[0-9]+)?)\s*C/gi));
  const values = matches
    .map((match) => validTemperatureCelsius(Number.parseFloat(match[1] ?? "")))
    .filter((value): value is number => value !== null);
  return values.length > 0 ? roundMetric(Math.max(...values)) : null;
}

async function queryThermalTemperatures(): Promise<{ cpu: number | null; system: number | null; source: HardwareThermalSource }> {
  if (process.platform === "linux") {
    return queryLinuxThermalTemperatures();
  }
  if (process.platform === "win32") {
    return queryWindowsThermalTemperatures();
  }
  if (process.platform === "darwin") {
    const cpu = queryMacosCpuTemperature();
    return { cpu, system: cpu, source: cpu === null ? "unavailable" : "macos-powermetrics" };
  }
  return { cpu: null, system: null, source: "unavailable" };
}

function refreshCachedThermalTemperatures(): Promise<ThermalTemperatures> {
  if (!thermalRefreshPromise) {
    thermalRefreshPromise = queryThermalTemperatures()
      .then((reading) => {
        cachedThermalTemperatures = { ...reading, sampledAt: Date.now() };
        return reading;
      })
      .finally(() => {
        thermalRefreshPromise = null;
      });
  }
  return thermalRefreshPromise;
}

function emptyThermalTemperatures(): ThermalTemperatures {
  return { cpu: null, system: null, source: "unavailable" };
}

function queryThermalTemperaturesCached(): ThermalTemperatures {
  const now = Date.now();
  if (
    cachedThermalTemperatures &&
    now - cachedThermalTemperatures.sampledAt <= HARDWARE_THERMAL_REFRESH_MS
  ) {
    return cachedThermalTemperatures;
  }
  const refresh = refreshCachedThermalTemperatures();
  if (cachedThermalTemperatures) {
    void refresh.catch((error) => {
      console.warn("Hardware thermal refresh failed.", error);
    });
    return cachedThermalTemperatures;
  }
  void refresh.catch((error) => {
    console.warn("Hardware thermal refresh failed.", error);
  });
  return emptyThermalTemperatures();
}

function topProcessSnapshot(): HardwareProcessSnapshot[] {
  const current = process.cpuUsage();
  return [
    {
      pid: process.pid,
      name: "InGen Electron main",
      cpuPercent: roundMetric((current.user + current.system) / 1_000_000, 2) ?? 0,
      memoryMb: roundMetric(process.memoryUsage().rss / 1024 ** 2, 1) ?? 0
    }
  ];
}

async function hardwareTelemetrySnapshot(): Promise<HardwareTelemetrySnapshot> {
  const os = await import("node:os");
  const cpuUsage = cpuUtilizationPercent();
  const allCpus = cpus();
  const totalMemoryGb = os.totalmem() / 1024 ** 3;
  const freeMemoryGb = os.freemem() / 1024 ** 3;
  const usedMemoryGb = Math.max(0, totalMemoryGb - freeMemoryGb);
  const memoryPercent = totalMemoryGb > 0 ? (usedMemoryGb / totalMemoryGb) * 100 : null;
  const nvidiaGpus = queryNvidiaGpusCached();
  const linuxHwmonGpus = process.platform === "linux" ? await queryLinuxHwmonGpus() : [];
  const windowsGpus = nvidiaGpus.length > 0 || nvidiaGpuRefreshScheduled ? [] : queryWindowsVideoControllers();
  const drmGpus = nvidiaGpus.length > 0 || windowsGpus.length > 0 ? [] : await queryLinuxDrmGpu();
  const enrichedNvidiaGpus = process.platform === "linux" ? mergeGpuSensorFallbacks(nvidiaGpus, linuxHwmonGpus) : nvidiaGpus;
  const linuxGpus = process.platform === "linux" && enrichedNvidiaGpus.length === 0 && windowsGpus.length === 0
    ? mergeGpuSensorFallbacks(drmGpus, linuxHwmonGpus)
    : [];
  const thermalTemperatures = queryThermalTemperaturesCached();
  const gpuNotes =
    nvidiaGpus.length === 0 && windowsGpus.length > 0
      ? ["Windows system fallback identifies adapters through Win32_VideoController; live GPU load, thermals, fan and power require vendor telemetry."]
      : [];
  const snapshot: HardwareTelemetrySnapshot = {
    schema: "ingen.hardware.telemetry.snapshot.v1",
    platform: os.platform(),
    arch: os.arch(),
    hostname: os.hostname(),
    sampledAt: new Date().toISOString(),
    cpu: {
      model: allCpus[0]?.model ?? "CPU",
      cores: allCpus.length,
      utilization: hardwareMetric("CPU load", cpuUsage, "%", metricStatusPercent(cpuUsage, 82, 94)),
      temperature: hardwareMetric("CPU temperature", thermalTemperatures.cpu, "C", metricStatusTemperature(thermalTemperatures.cpu)),
      loadAverage: hardwareMetric("Load average", roundMetric(os.loadavg()[0] ?? null, 2), "count")
    },
    memory: {
      used: hardwareMetric("RAM used", roundMetric(usedMemoryGb, 2), "GB"),
      total: hardwareMetric("RAM total", roundMetric(totalMemoryGb, 2), "GB"),
      utilization: hardwareMetric("RAM load", roundMetric(memoryPercent), "%", metricStatusPercent(memoryPercent, 78, 90))
    },
    thermal: {
      systemTemperature: hardwareMetric("System temperature", thermalTemperatures.system, "C", metricStatusTemperature(thermalTemperatures.system)),
      source: thermalTemperatures.source
    },
    gpus: [...enrichedNvidiaGpus, ...windowsGpus, ...linuxGpus],
    topProcesses: topProcessSnapshot(),
    governor: {
      profile: "balanced",
      monsterBudgetPercent: memoryPercent !== null && memoryPercent > 86 ? 35 : 65,
      bangerBudgetPercent: memoryPercent !== null && memoryPercent > 86 ? 30 : 60,
      controlAuthority: "app-budget-only",
      fanControl: "locked",
      notes: [
        ...gpuNotes,
        "Fan and power-profile writes stay locked until an OEM or driver API is explicitly promoted.",
        "Monster and Banger should consume these budgets before scheduling local GPU work."
      ]
    },
    proofHash: ""
  };
  if (snapshot.gpus.length === 0) {
    snapshot.gpus = [emptyGpu()];
  }
  snapshot.proofHash = hashJson({ ...snapshot, proofHash: "" });
  return snapshot;
}

function isLlmProviderConnectId(value: unknown): value is LlmProviderConnectId {
  return value === "codex" || value === "claude" || value === "openrouter";
}

const llmProviderOfficialFlows: Record<LlmProviderConnectId, { url: string; events: string[] }> = {
  codex: {
    url: CHATGPT_LOGIN_URL,
    events: [
      "provider openai / model refs openai/*",
      "open OpenAI OAuth Direct window",
      "waiting ChatGPT subscription session"
    ]
  },
  claude: {
    url: "https://code.claude.com/docs/en/setup",
    events: [
      "launch claude CLI",
      "waiting official Claude browser prompt",
      "bind account session outside renderer memory",
      "waiting eve_reader confirmation"
    ]
  },
  openrouter: {
    url: "https://openrouter.ai/auth",
    events: [
      "create PKCE verifier",
      "open official OpenRouter auth flow",
      "waiting local callback and credential seal",
      "waiting eve_reader confirmation"
    ]
  }
};

function providerProfileFromComposer(provider: ComposerProviderId): ProviderRuntimeProfile {
  return providerRuntime.codex.composerProvider === provider
    ? providerRuntime.codex
    : providerRuntime.claude.composerProvider === provider
      ? providerRuntime.claude
      : providerRuntime.openrouter;
}

function safeStringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0) : [];
}

function displayReasoningLabel(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (normalized === "bas" || normalized === "low") {
    return "Low";
  }
  if (normalized === "moyen" || normalized === "medium") {
    return "Medium";
  }
  if (normalized === "élevé" || normalized === "eleve" || normalized === "high") {
    return "High";
  }
  if (
    normalized === "très approfondi" ||
    normalized === "tres approfondi" ||
    normalized === "deep" ||
    normalized === "very deep" ||
    normalized === "xhigh" ||
    normalized === "max"
  ) {
    return "Deep";
  }
  return value.trim();
}

function safeReasoningLabels(value: unknown): string[] {
  const seen = new Set<string>();
  return safeStringList(value)
    .map(displayReasoningLabel)
    .filter((item) => {
      if (!item || seen.has(item)) {
        return false;
      }
      seen.add(item);
      return true;
    });
}

function storedProviderProfile(profile: ProviderRuntimeProfile): StoredProviderProfile {
  return {
    connected: profile.connected,
    account: profile.account,
    models: profile.models,
    reasoning: profile.reasoning.map(displayReasoningLabel),
    quotaLabel: profile.quotaLabel,
    proof: profile.proof,
    events: profile.events,
    updatedAt: new Date().toISOString()
  };
}

async function persistProviderRuntime(): Promise<void> {
  const snapshot: StoredProviderRuntime = {
    schema: "ingen.electron.llm_provider_runtime.v1",
    providers: {
      codex: storedProviderProfile(providerRuntime.codex),
      claude: storedProviderProfile(providerRuntime.claude),
      openrouter: storedProviderProfile(providerRuntime.openrouter)
    },
    secrets: openRouterApiKey ? { openrouter: { apiKey: openRouterApiKey } } : {}
  };
  if (!(await isProviderRuntimeEncryptionAvailable())) {
    console.error("LLM provider runtime was not persisted because OS encryption is unavailable.");
    return;
  }
  const plaintext = JSON.stringify(snapshot);
  const encrypted = await safeStorage.encryptStringAsync(plaintext);
  let storageBackend: string | undefined;
  try {
    storageBackend = safeStorage.getSelectedStorageBackend();
  } catch {
    storageBackend = undefined;
  }
  const envelope: EncryptedProviderRuntimeEnvelope = {
    schema: "ingen.electron.llm_provider_runtime.encrypted.v1",
    cipher: "electron.safeStorage",
    encoding: "base64",
    storageBackend,
    ciphertext: encrypted.toString("base64"),
    updatedAt: new Date().toISOString()
  };
  await writeFile(llmProviderRuntimeStorePath(), JSON.stringify(envelope, null, 2), "utf8");
}

function applyStoredProviderRuntime(parsed: Partial<StoredProviderRuntime>): boolean {
  if (parsed.schema !== "ingen.electron.llm_provider_runtime.v1" || !parsed.providers) {
    return false;
  }
  for (const [provider, stored] of Object.entries(parsed.providers)) {
    if (!isLlmProviderConnectId(provider) || !stored || typeof stored !== "object") {
      continue;
    }
    const storedEvents = safeStringList(stored.events);
    const target = providerRuntime[provider];
    if (
      provider === "claude" &&
      storedEvents.some((event) => /restored Claude Code auth session|connected Claude Code session persisted/i.test(event))
    ) {
      target.connected = false;
      target.account = "Claude Code OAuth";
      target.models = [];
      target.reasoning = [];
      target.quotaLabel = "reset pending";
      target.events = ["Claude tab reset", "awaiting secure login"];
      target.proof = hashJson({ provider: "claude", reset: "llm-provider-runtime-v2" });
      continue;
    }
    target.connected = stored.connected === true;
    target.account = typeof stored.account === "string" && stored.account.trim() ? stored.account : target.account;
    target.models = safeStringList(stored.models);
    target.reasoning = provider === "codex" ? safeReasoningLabels(stored.reasoning) : safeStringList(stored.reasoning);
    target.quotaLabel = typeof stored.quotaLabel === "string" && stored.quotaLabel.trim() ? stored.quotaLabel : target.quotaLabel;
    target.events = storedEvents;
    if (target.connected && target.events.length === 0) {
      target.events = connectedProviderEvents([], target);
    }
    if (!target.connected && target.events.length === 0) {
      target.events = ["awaiting secure login"];
    }
    target.proof = typeof stored.proof === "string" && stored.proof.trim()
      ? stored.proof
      : hashJson({ provider, restored: true, models: target.models, reasoning: target.reasoning });
  }
  const openRouterSecret = parsed.secrets?.openrouter;
  if (typeof openRouterSecret?.apiKey === "string" && openRouterSecret.apiKey.trim()) {
    openRouterApiKey = openRouterSecret.apiKey.trim();
  }
  const connectedProvider = Object.values(providerRuntime).find((profile) => profile.connected);
  if (connectedProvider) {
    activateComposerProvider(connectedProvider);
  }
  return true;
}

async function decryptStoredProviderRuntime(envelope: Partial<EncryptedProviderRuntimeEnvelope>): Promise<StoredProviderRuntime | undefined> {
  if (
    envelope.schema !== "ingen.electron.llm_provider_runtime.encrypted.v1" ||
    envelope.cipher !== "electron.safeStorage" ||
    envelope.encoding !== "base64" ||
    typeof envelope.ciphertext !== "string"
  ) {
    return undefined;
  }
  const bytes = Buffer.from(envelope.ciphertext, "base64");
  const decrypted = await safeStorage.decryptStringAsync(bytes);
  if (decrypted.shouldReEncrypt) {
    void persistProviderRuntime().catch((error: unknown) => {
      console.error("Failed to rotate LLM provider runtime encryption.", error);
    });
  }
  return JSON.parse(decrypted.result) as StoredProviderRuntime;
}

async function restoreProviderRuntimeFromDisk(): Promise<void> {
  try {
    const raw = await readFile(llmProviderRuntimeStorePath(), "utf8");
    const parsed = JSON.parse(raw) as Partial<StoredProviderRuntime> | Partial<EncryptedProviderRuntimeEnvelope>;
    if (parsed.schema === "ingen.electron.llm_provider_runtime.encrypted.v1") {
      const decrypted = await decryptStoredProviderRuntime(parsed);
      if (decrypted) {
        applyStoredProviderRuntime(decrypted);
      }
      return;
    }
    if (applyStoredProviderRuntime(parsed as Partial<StoredProviderRuntime>)) {
      void persistProviderRuntime().catch((error: unknown) => {
        console.error("Failed to migrate LLM provider runtime to encrypted storage.", error);
      });
    }
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? String((error as { code?: unknown }).code) : "";
    if (code !== "ENOENT") {
      console.error("Failed to restore LLM provider runtime.", error);
    }
  }
}

function closeProfileCanvas(): void {
  headerState.profileCanvas = "";
  sidebarState.profileOpen = false;
}

function activateWebExplorerSplit(): void {
  closeProfileCanvas();
  headerState.activeSection = "webexplorer";
  headerState.sectionTitle = "RAM DOM Atlas";
}

function normalizeGptModelId(value: string): string {
  return value.trim().toLowerCase().replace(/\.access$/i, "");
}

function normalizeReasoningLevel(value: string): string {
  const level = value.trim().toLowerCase();
  return level === "max" ? "xhigh" : level;
}

function codexDesktopModelSlug(value: string): string {
  const model = value.trim().toLowerCase();
  if (model === "gpt-5.5" || model === "gpt-5-5") {
    return "gpt-5.5";
  }
  if (model === "gpt-5.4" || model === "gpt-5-4") {
    return "gpt-5.4";
  }
  if (model === "gpt-5.4-mini" || model === "gpt-5-4-mini") {
    return "gpt-5.4-mini";
  }
  if (model === "gpt-5.3-codex-spark" || model === "gpt-5-3-codex-spark") {
    return "gpt-5.3-codex-spark";
  }
  return model;
}

function isReasoningLevel(value: string): boolean {
  return /^(none|minimal|low|medium|high|xhigh|auto)$/i.test(value);
}

function reasoningLevelsForModels(models: string[]): string[] {
  const cleanModels = models.map(normalizeGptModelId);
  const hasReasoningModel = cleanModels.some((model) => /(?:reasoning|thinking|pro)\b/i.test(model));
  return hasReasoningModel ? ["low", "medium", "high", "xhigh"] : [];
}

function modelCatalogEvents(models: string[]): string[] {
  const cleanModels = [...new Set(models.map(normalizeGptModelId).filter(Boolean))];
  if (cleanModels.length === 0) {
    return ["model catalog unavailable"];
  }
  if (cleanModels.length <= 6) {
    return [`models ${cleanModels.join(" / ")}`];
  }
  const splitAt = Math.ceil(cleanModels.length / 2);
  return [
    `models ${cleanModels.slice(0, splitAt).join(" / ")}`,
    `models ${cleanModels.slice(splitAt).join(" / ")}`
  ];
}

function connectedProviderEvents(flowEvents: string[], profile: ProviderRuntimeProfile): string[] {
  const reasoningEvent = profile.reasoning.length > 0
    ? `reasoning ${profile.reasoning.join(" / ")}`
    : "reasoning unavailable";
  return [
    ...flowEvents.filter((event) => event !== "ready"),
    "eve_reader confirmed provider account",
    ...modelCatalogEvents(profile.models),
    reasoningEvent,
    profile.quotaLabel,
    "ready"
  ];
}

function markProviderReadyEvents(profile: ProviderRuntimeProfile, events: string[]): void {
  profile.connected = true;
  profile.events = events;
}

function activateComposerProvider(profile: ProviderRuntimeProfile): void {
  panelsChatBottomState.selectedProvider = profile.composerProvider;
  panelsChatBottomState.modelIndex = 0;
  panelsChatBottomState.reasoningIndex = Math.min(
    panelsChatBottomState.reasoningIndex,
    Math.max(0, profile.reasoning.length - 1)
  );
}

function syncComposerProvider(profile: ProviderRuntimeProfile): void {
  if (panelsChatBottomState.selectedProvider !== profile.composerProvider) {
    activateComposerProvider(profile);
    return;
  }
  panelsChatBottomState.modelIndex = Math.min(panelsChatBottomState.modelIndex, Math.max(0, profile.models.length - 1));
  panelsChatBottomState.reasoningIndex = Math.min(panelsChatBottomState.reasoningIndex, Math.max(0, profile.reasoning.length - 1));
}

type ProviderLaunchResult = {
  launched: boolean;
  events: string[];
  error?: string;
};

type CodexAccountProbe = {
  ok: boolean;
  status?: number;
  accountId?: string;
  emailDomain?: string;
  plan?: string;
  source?: "api" | "web-session";
  error?: string;
};

type CommandCaptureResult = {
  exitCode: number | null;
  stdout: string;
  stderr: string;
  error?: string;
};

type ProviderCliProbe = {
  ok: boolean;
  cliFound: boolean;
  command?: string;
  version?: string;
  runtimeVerified?: boolean;
  events?: string[];
  account?: string;
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  raw?: unknown;
  error?: string;
};

type OpenRouterOAuthWaiter = {
  callbackUrl: string;
  codeVerifier: string;
  codeChallenge: string;
  codePromise: Promise<string>;
  close: () => void;
};

type OpenRouterProbe = {
  ok: boolean;
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  account: string;
  proof: string;
  error?: string;
};

function emitLlmProviderRuntimeEvent(event: LlmProviderRuntimeEvent): void {
  const profile = providerRuntime[event.provider];
  if (event.events.includes("ready")) {
    profile.connected = true;
    profile.events = event.events;
    profile.models = event.models.length > 0 ? event.models : profile.models;
    profile.reasoning = event.reasoning.length > 0 ? event.reasoning : profile.reasoning;
    profile.quotaLabel = event.quotaLabel || profile.quotaLabel;
    profile.proof = event.proofHash || profile.proof;
    void persistProviderRuntime().catch((error: unknown) => {
      console.error(`Failed to persist ${event.provider} terminal events.`, error);
    });
  } else if (!profile.events.includes("ready")) {
    profile.events = event.events;
  }
  const window = primaryWindow;
  if (!window || window.isDestroyed()) {
    return;
  }
  window.webContents.send("forge:llm-provider-event", event);
}

function emitPanelsChatBottomSnapshotEvent(
  reason: PanelsChatBottomSnapshotEvent["reason"],
  sessionId: string
): void {
  const window = primaryWindow;
  if (!window || window.isDestroyed()) {
    return;
  }
  const event: PanelsChatBottomSnapshotEvent = {
    kind: "snapshot_updated",
    reason,
    sessionId,
    proofHash: hashJson({ channel: "panels_chat_bottom", reason, sessionId, at: Date.now() })
  };
  window.webContents.send("forge:panels-chat-bottom-snapshot-event", event);
}

let agentRuntimeEventSequence = 0;
type AgentRuntimeEventDraft = Omit<AgentRuntimeEvent, "schema" | "sequence" | "at" | "proofHash"> &
  Partial<Pick<AgentRuntimeEvent, "sequence" | "at" | "proofHash">>;

interface AgentRuntimeEventQueue {
  sessionId: string;
  agentName: string;
  provider: LlmProviderConnectId;
  emit: (event: Omit<AgentRuntimeEventDraft, "sessionId" | "agentName" | "provider">) => void;
}

function emitAgentRuntimeEvent(event: AgentRuntimeEventDraft): void {
  const window = primaryWindow;
  if (!window || window.isDestroyed()) {
    return;
  }
  const runtimeEvent: AgentRuntimeEvent = {
    schema: "ingen.agent_runtime.event.v1",
    sequence: event.sequence ?? ++agentRuntimeEventSequence,
    at: event.at ?? Date.now(),
    proofHash: "",
    ...event
  };
  runtimeEvent.proofHash = event.proofHash ?? hashJson({ ...runtimeEvent, proofHash: "" });
  window.webContents.send("forge:agent-runtime-event", runtimeEvent);
}

function createAgentRuntimeEventQueue(params: {
  sessionId: string;
  agentName?: string;
  provider?: LlmProviderConnectId;
}): AgentRuntimeEventQueue {
  const queue: AgentRuntimeEventQueue = {
    sessionId: params.sessionId,
    agentName: params.agentName?.trim() || agentRuntimeAgentName(),
    provider: params.provider ?? agentRuntimeProvider(),
    emit: (event) => emitAgentRuntimeEvent({
      ...event,
      sessionId: queue.sessionId,
      agentName: queue.agentName,
      provider: queue.provider
    })
  };
  return queue;
}

function runtimeEventFromProviderProfile(profile: ProviderRuntimeProfile, prefix?: string): LlmProviderRuntimeEvent {
  const events = profile.connected && profile.events.includes("ready")
    ? profile.events
    : profile.connected
    ? [
        prefix ?? `restored ${profile.label} session`,
        ...modelCatalogEvents(profile.models),
        profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
        profile.quotaLabel,
        "ready"
      ]
    : ["awaiting secure login"];
  return {
    provider: profile.connectId,
    events,
    models: profile.models,
    reasoning: profile.reasoning,
    quotaLabel: profile.quotaLabel,
    proofHash: profile.proof
  };
}

function llmProviderRuntimeSnapshot(): LlmProviderRuntimeSnapshot {
  return {
    codex: runtimeEventFromProviderProfile(providerRuntime.codex),
    claude: runtimeEventFromProviderProfile(providerRuntime.claude),
    openrouter: runtimeEventFromProviderProfile(providerRuntime.openrouter)
  };
}

function emitProviderRuntimeSnapshot(): void {
  const snapshot = llmProviderRuntimeSnapshot();
  for (const provider of Object.values(snapshot)) {
    emitLlmProviderRuntimeEvent(provider);
  }
}

function cleanProbeText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function randomBase64Url(bytes = 48): string {
  return randomBytes(bytes).toString("base64url");
}

function sha256Base64Url(value: string): string {
  return createHash("sha256").update(value).digest("base64url");
}

function stopOpenRouterOAuthServer(): void {
  if (openRouterOAuthServer) {
    openRouterOAuthServer.close();
    openRouterOAuthServer = null;
  }
}

function startOpenRouterOAuthWaiter(): Promise<OpenRouterOAuthWaiter> {
  stopOpenRouterOAuthServer();
  const codeVerifier = randomBase64Url(64);
  const codeChallenge = sha256Base64Url(codeVerifier);
  const callbackPort = 3000;
  const callbackUrl = `http://localhost:${callbackPort}`;

  return new Promise((resolve, reject) => {
    let settled = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    let resolveCode: ((code: string) => void) | undefined;
    let rejectCode: ((error: Error) => void) | undefined;
    const codePromise = new Promise<string>((innerResolve, innerReject) => {
      resolveCode = innerResolve;
      rejectCode = innerReject;
    });

    const server = createServer((request, response) => {
      const host = request.headers.host ?? `localhost:${callbackPort}`;
      const requestUrl = new URL(request.url ?? "/", `http://${host}`);
      if (requestUrl.pathname !== "/" && requestUrl.pathname !== "/openrouter/callback") {
        response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
        response.end("Not found");
        return;
      }
      const error = requestUrl.searchParams.get("error");
      const code = requestUrl.searchParams.get("code");
      if (error || !code) {
        response.writeHead(400, { "Content-Type": "text/html; charset=utf-8" });
        response.end("<!doctype html><title>OpenRouter</title><body>OpenRouter login failed. You can close this window.</body>");
        rejectCode?.(new Error(error || "OpenRouter OAuth callback did not include a code."));
        stopOpenRouterOAuthServer();
        return;
      }
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      response.end("<!doctype html><title>OpenRouter</title><body>OpenRouter connected. You can close this window.</body>");
      resolveCode?.(code);
      stopOpenRouterOAuthServer();
    });

    server.once("error", (error) => {
      if (!settled) {
        settled = true;
        if (timeout) {
          clearTimeout(timeout);
        }
        const code = error && typeof error === "object" && "code" in error ? String((error as { code?: unknown }).code) : "";
        reject(code === "EADDRINUSE"
          ? new Error("OpenRouter OAuth local callback port 3000 is already in use.")
          : error);
      } else {
        rejectCode?.(error);
      }
    });

    server.listen(callbackPort, "localhost", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("OpenRouter OAuth callback server did not return a TCP port."));
        return;
      }
      openRouterOAuthServer = server;
      timeout = setTimeout(() => {
        rejectCode?.(new Error("OpenRouter OAuth callback timed out."));
        stopOpenRouterOAuthServer();
      }, 180000);
      codePromise.finally(() => {
        if (timeout) {
          clearTimeout(timeout);
        }
      }).catch(() => undefined);
      settled = true;
      resolve({
        callbackUrl,
        codeVerifier,
        codeChallenge,
        codePromise,
        close: stopOpenRouterOAuthServer
      });
    });
  });
}

async function openRouterFetchJson(url: string, init: RequestInit = {}): Promise<unknown> {
  const response = await net.fetch(url, init);
  const text = await response.text();
  let parsed: unknown;
  try {
    parsed = text ? JSON.parse(text) : {};
  } catch {
    parsed = { text };
  }
  if (!response.ok) {
    const message = typeof parsed === "object" && parsed && "error" in parsed
      ? JSON.stringify((parsed as { error?: unknown }).error)
      : text || `OpenRouter request failed with ${response.status}`;
    throw new Error(message);
  }
  return parsed;
}

function contentText(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }
  if (!Array.isArray(value)) {
    return "";
  }
  return value
    .map((part) => {
      if (typeof part === "string") {
        return part;
      }
      if (part && typeof part === "object") {
        const record = part as Record<string, unknown>;
        return typeof record.text === "string" ? record.text : typeof record.content === "string" ? record.content : "";
      }
      return "";
    })
    .filter(Boolean)
    .join("\n")
    .trim();
}

function publicUploadPreview(item: ComposerUploadItem): ComposerUploadPreview {
  const { path: _path, mimeType: _mimeType, ...preview } = item;
  return {
    ...preview,
    url: uploadPreviewUrl(item.id, item.name)
  };
}

function uploadPreviewUrl(id: string, name: string): string {
  return `ingen://upload-preview/${encodeURIComponent(id)}/${encodeURIComponent(name)}`;
}

function isRemoteAttachmentUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function attachmentProofSummary(attachments: ProviderAttachment[]): Array<{
  id: string;
  name: string;
  kind: ComposerUploadPreview["kind"];
  mimeType: string;
  sizeBytes: number;
  proofHash: string;
  openAiFileData: boolean;
  visualSnapshots: number;
  videoMetadata: boolean;
  subtitlesExtracted: boolean;
}> {
  return attachments.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    kind: attachment.kind,
    mimeType: attachment.mimeType,
    sizeBytes: attachment.sizeBytes,
    proofHash: attachment.proofHash,
    openAiFileData: Boolean(attachment.openAiFileDataUrl),
    visualSnapshots: attachment.visualSnapshots.length,
    videoMetadata: Boolean(attachment.videoMetadata),
    subtitlesExtracted: Boolean(attachment.videoMetadata?.subtitlesExtracted)
  }));
}

function trimUtf8Bytes(value: string, maxBytes: number): string {
  const bytes = Buffer.from(value, "utf8");
  if (bytes.length <= maxBytes) {
    return value;
  }
  return `${bytes.subarray(0, maxBytes).toString("utf8").replace(/\uFFFD+$/g, "").trimEnd()}\n[truncated to ${maxBytes} bytes]`;
}

async function readFilePrefix(filePath: string, maxBytes: number): Promise<Buffer> {
  const handle = await open(filePath, "r");
  try {
    const buffer = Buffer.alloc(maxBytes);
    const result = await handle.read(buffer, 0, maxBytes, 0);
    return buffer.subarray(0, result.bytesRead);
  } finally {
    await handle.close();
  }
}

function tablePreviewText(table: string[][]): string {
  return table.map((row) => row.map((cell) => String(cell ?? "").replace(/\r?\n/g, " ")).join(",")).join("\n");
}

function llmTextPreviewForUpload(item: ComposerUploadItem, generatedPreview = ""): string {
  const text = item.textPreview.trim() || generatedPreview.trim();
  if (text) {
    return trimUtf8Bytes(text, PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES);
  }
  if ((item.kind === "spreadsheet" || item.kind === "chart") && item.tablePreview.length > 0) {
    return trimUtf8Bytes(tablePreviewText(item.tablePreview), PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES);
  }
  return "";
}

function isOpenAiFileInputAttachment(attachment: ProviderAttachment): boolean {
  return (
    attachment.sizeBytes > 0 &&
    attachment.sizeBytes <= PANELS_CHAT_BOTTOM_MAX_OPENAI_FILE_BYTES &&
    attachment.kind !== "image" &&
    attachment.kind !== "video" &&
    attachment.kind !== "model3d"
  );
}

async function openAiFileDataUrlForAttachment(attachment: ProviderAttachment): Promise<string | undefined> {
  if (!isOpenAiFileInputAttachment(attachment)) {
    return undefined;
  }
  return attachmentDataUrl(attachment);
}

function providerAttachmentCacheKey(item: ComposerUploadItem, fileStat: { size: number; mtimeMs: number }): string {
  return hashJson({
    id: item.id,
    path: item.path,
    kind: item.kind,
    mimeType: item.mimeType,
    sizeBytes: fileStat.size,
    mtimeMs: fileStat.mtimeMs
  });
}

function rememberProviderAttachment(cacheKey: string, attachment: ProviderAttachment): ProviderAttachment {
  providerAttachmentCache.set(cacheKey, attachment);
  if (providerAttachmentCache.size > PANELS_CHAT_BOTTOM_MAX_UPLOADS * 4) {
    const oldestKey = providerAttachmentCache.keys().next().value;
    if (oldestKey) {
      providerAttachmentCache.delete(oldestKey);
    }
  }
  return attachment;
}

async function providerAttachmentFromUpload(item: ComposerUploadItem): Promise<ProviderAttachment> {
  const fileStat = await stat(item.path);
  const cacheKey = providerAttachmentCacheKey(item, fileStat);
  const cached = providerAttachmentCache.get(cacheKey);
  if (cached) {
    return cached;
  }
  const generatedPreview = item.kind === "model3d" ? await model3dSummaryForFile(item.path) : "";
  const llmTextPreview = llmTextPreviewForUpload(item, generatedPreview);
  const attachment: ProviderAttachment = {
    ...item,
    sizeBytes: fileStat.size,
    proofHash: "",
    llmTextPreview,
    visualSnapshots: []
  };
  attachment.openAiFileDataUrl = await openAiFileDataUrlForAttachment(attachment);
  attachment.visualSnapshots = await visualSnapshotsForAttachment(attachment, generatedPreview);
  const videoContext = videoMetadataText(attachment.videoMetadata);
  if (videoContext) {
    attachment.llmTextPreview = [attachment.llmTextPreview.trim(), videoContext].filter(Boolean).join("\n\n");
  }
  const proofHash = hashJson({
    id: item.id,
    name: item.name,
    kind: item.kind,
    mimeType: item.mimeType,
    sizeBytes: fileStat.size,
    textPreviewHash: attachment.llmTextPreview ? hashJson(attachment.llmTextPreview) : "",
    tablePreviewHash: item.tablePreview.length > 0 ? hashJson(item.tablePreview) : "",
    visualSnapshots: attachment.visualSnapshots.map((snapshot) => snapshot.proofHash),
    videoMetadataHash: attachment.videoMetadata ? hashJson(attachment.videoMetadata) : "",
    openAiFileDataHash: attachment.openAiFileDataUrl ? hashJson(attachment.openAiFileDataUrl) : ""
  });
  attachment.proofHash = proofHash;
  return rememberProviderAttachment(cacheKey, attachment);
}

async function providerAttachmentsFromUploads(items: ComposerUploadItem[]): Promise<ProviderAttachment[]> {
  return Promise.all(items.map((item) => providerAttachmentFromUpload(item)));
}

function composerUploadItemsForCommand(command: PanelsChatBottomCommand): ComposerUploadItem[] {
  const ids = Array.isArray(command.attachmentIds)
    ? command.attachmentIds.filter((id): id is string => typeof id === "string" && id.trim().length > 0)
    : [];
  if (ids.length === 0) {
    return [...panelsChatBottomState.uploadItems];
  }
  const byId = new Map(panelsChatBottomState.uploadItems.map((item) => [item.id, item]));
  const resolved = ids
    .map((id) => byId.get(id) ?? composerUploadPreviewItems.get(id))
    .filter((item): item is ComposerUploadItem => Boolean(item));
  return resolved.length > 0 ? resolved : [...panelsChatBottomState.uploadItems];
}

function imageUploadItemsForCommand(command: PanelsChatBottomCommand): ComposerUploadItem[] {
  const ids = Array.isArray(command.attachmentIds)
    ? command.attachmentIds.filter((id): id is string => typeof id === "string" && id.trim().length > 0)
    : [];
  const resolved = ids
    .map((id) => composerUploadPreviewItems.get(id))
    .filter((item): item is ComposerUploadItem => item !== undefined && item.kind === "image");
  return resolved;
}

function stageAttachmentForImageEdit(command: PanelsChatBottomCommand): { accepted: boolean; error?: IpcError } {
  const imageItems = imageUploadItemsForCommand(command);
  if (imageItems.length === 0) {
    panelsChatBottomState.uploadErrorText = "IMAGE_EDIT_TARGET_MISSING: select an image first.";
    return {
      accepted: false,
      error: {
        code: "bad_payload",
        message: panelsChatBottomState.uploadErrorText,
        proofHash: hashJson({ command: "stage_attachment_for_edit", attachmentIds: command.attachmentIds ?? [] })
      }
    };
  }
  const stagedIds = new Set(imageItems.map((item) => item.id));
  panelsChatBottomState.uploadItems = [
    ...imageItems,
    ...panelsChatBottomState.uploadItems.filter((item) => !stagedIds.has(item.id))
  ].slice(0, PANELS_CHAT_BOTTOM_MAX_UPLOADS);
  panelsChatBottomState.uploadCount = panelsChatBottomState.uploadItems.length;
  panelsChatBottomState.uploadEditTargetId = imageItems[0]?.id ?? "";
  panelsChatBottomState.uploadErrorText = "";
  return { accepted: true };
}

function providerUploadItemsForCommand(
  pendingUploadItems: ComposerUploadItem[],
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): ComposerUploadItem[] {
  const items: ComposerUploadItem[] = [];
  const seen = new Set<string>();
  const pushItem = (item: ComposerUploadItem | undefined) => {
    if (!item || seen.has(item.id) || items.length >= PANELS_CHAT_BOTTOM_MAX_PROVIDER_ATTACHMENTS) {
      return;
    }
    seen.add(item.id);
    items.push(item);
  };
  for (const item of pendingUploadItems) {
    pushItem(item);
  }
  for (const message of [...transcript].reverse()) {
    const previews = [...(message.attachments ?? [])].reverse();
    for (const preview of previews) {
      pushItem(composerUploadPreviewItems.get(preview.id));
    }
    if (items.length >= PANELS_CHAT_BOTTOM_MAX_PROVIDER_ATTACHMENTS) {
      break;
    }
  }
  return items;
}

function isInlineVisionAttachment(attachment: ProviderAttachment): boolean {
  return (
    attachment.kind === "image" &&
    attachment.mimeType.startsWith("image/") &&
    attachment.mimeType !== "image/svg+xml" &&
    attachment.sizeBytes <= PANELS_CHAT_BOTTOM_MAX_INLINE_IMAGE_BYTES
  );
}

async function attachmentDataUrl(attachment: ProviderAttachment): Promise<string> {
  const bytes = await readFile(attachment.path);
  return `data:${attachment.mimeType};base64,${bytes.toString("base64")}`;
}

function visualSnapshot(label: string, source: ProviderVisualSnapshot["source"], imageUrl: string): ProviderVisualSnapshot {
  return {
    label,
    source,
    imageUrl,
    proofHash: hashJson({ label, source, imageUrl })
  };
}

let attachmentSnapshotWindow: BrowserWindow | null = null;
let attachmentSnapshotWindowReady: Promise<BrowserWindow> | null = null;
let attachmentSnapshotQueue: Promise<unknown> = Promise.resolve();

async function attachmentSnapshotBrowserWindow(): Promise<BrowserWindow> {
  if (attachmentSnapshotWindow && !attachmentSnapshotWindow.isDestroyed()) {
    return attachmentSnapshotWindow;
  }
  if (attachmentSnapshotWindowReady) {
    return attachmentSnapshotWindowReady;
  }
  attachmentSnapshotWindowReady = (async () => {
    const window = new BrowserWindow({
      width: PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE,
      height: PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE,
      show: false,
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        webSecurity: false,
        backgroundThrottling: false,
        offscreen: true
      }
    });
    attachmentSnapshotWindow = window;
    window.once("closed", () => {
      if (attachmentSnapshotWindow === window) {
        attachmentSnapshotWindow = null;
      }
      attachmentSnapshotWindowReady = null;
    });
    await window.loadURL("about:blank");
    return window;
  })();
  return attachmentSnapshotWindowReady;
}

function destroyAttachmentSnapshotWindow(): void {
  if (attachmentSnapshotWindow && !attachmentSnapshotWindow.isDestroyed()) {
    attachmentSnapshotWindow.destroy();
  }
  attachmentSnapshotWindow = null;
  attachmentSnapshotWindowReady = null;
}

async function runAttachmentSnapshotScript<T>(script: string, fallback: T): Promise<T> {
  const job = attachmentSnapshotQueue.then(async () => {
    const window = await attachmentSnapshotBrowserWindow();
    if (window.isDestroyed()) {
      return fallback;
    }
    return await window.webContents.executeJavaScript(script, true) as T;
  });
  attachmentSnapshotQueue = job.catch(() => undefined);
  try {
    return await job;
  } catch (error) {
    console.error("Attachment snapshot job failed.", error);
    return fallback;
  }
}

function durationLabel(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "duration=unavailable";
  }
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const remainingSeconds = total % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m ${remainingSeconds}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${remainingSeconds}s`;
  }
  return `${remainingSeconds}s`;
}

function timestampLabel(seconds: number): string {
  const safe = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
  const total = Math.floor(safe);
  const millis = Math.round((safe - total) * 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const remainingSeconds = total % 60;
  const core = hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remainingSeconds).padStart(2, "0")}`
    : `${minutes}:${String(remainingSeconds).padStart(2, "0")}`;
  return millis > 0 ? `${core}.${String(millis).padStart(3, "0")}` : core;
}

function videoQualityLabel(width: number, height: number): string {
  const shortEdge = Math.min(width, height);
  const longEdge = Math.max(width, height);
  if (longEdge >= 7680 || shortEdge >= 4320) return "8K-or-higher";
  if (longEdge >= 3840 || shortEdge >= 2160) return "4K/UHD";
  if (longEdge >= 2560 || shortEdge >= 1440) return "1440p/QHD";
  if (longEdge >= 1920 || shortEdge >= 1080) return "1080p/FHD";
  if (longEdge >= 1280 || shortEdge >= 720) return "720p/HD";
  if (longEdge > 0 && shortEdge > 0) return "sub-HD";
  return "unknown";
}

function videoMetadataText(metadata?: ProviderVideoMetadata): string {
  if (!metadata) {
    return "";
  }
  const lines = [
    "Video metadata:",
    `duration=${metadata.durationLabel} (${metadata.durationSeconds.toFixed(3)}s)`,
    `resolution=${metadata.width}x${metadata.height}`,
    `aspect_ratio=${metadata.aspectRatio.toFixed(4)}`,
    `quality_label=${metadata.qualityLabel}`,
    `snapshot_times=${metadata.snapshotTimes.map((time) => `${time.toFixed(3)}s`).join(", ") || "none"}`,
    `text_tracks=${metadata.textTracks.length}`,
    `subtitles_extracted=${metadata.subtitlesExtracted}`
  ];
  for (const track of metadata.textTracks) {
    lines.push(`track_${track.index}=kind:${track.kind || "unknown"} label:${track.label || "unlabeled"} language:${track.language || "und"} cues:${track.cueCount}`);
    for (const cue of track.cues.slice(0, PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_CUES)) {
      lines.push(`[${timestampLabel(cue.start)} -> ${timestampLabel(cue.end)}] ${cue.text}`);
    }
  }
  return trimUtf8Bytes(lines.join("\n"), PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_BYTES);
}

function escapeSvgText(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function summarySnapshotSvg(title: string, subtitle: string, lines: string[]): string {
  const safeLines = lines
    .flatMap((line) => line.split(/\r?\n/))
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 13);
  const rows = safeLines
    .map((line, index) => {
      const y = 234 + index * 34;
      return `<text x="64" y="${y}" class="line">${escapeSvgText(line.slice(0, 92))}</text>`;
    })
    .join("");
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE}" height="${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE}" viewBox="0 0 ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE} ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE}">
  <defs>
    <linearGradient id="bg" x1="0" x2="1" y1="0" y2="1">
      <stop offset="0" stop-color="#111214"/>
      <stop offset="1" stop-color="#262826"/>
    </linearGradient>
  </defs>
  <rect width="768" height="768" fill="url(#bg)"/>
  <rect x="44" y="44" width="680" height="680" rx="18" fill="#181a1b" stroke="#434746"/>
  <text x="64" y="114" class="eyebrow">INGEN ATTACHMENT SNAPSHOT</text>
  <text x="64" y="166" class="title">${escapeSvgText(title.slice(0, 48))}</text>
  <text x="64" y="204" class="subtitle">${escapeSvgText(subtitle.slice(0, 86))}</text>
  ${rows}
  <style>
    text { font-family: Arial, Helvetica, sans-serif; fill: #f4f4f2; }
    .eyebrow { font-size: 18px; letter-spacing: 2px; fill: #d98245; }
    .title { font-size: 42px; font-weight: 700; }
    .subtitle { font-size: 21px; fill: #c9c9c5; }
    .line { font-size: 22px; fill: #e4e4df; }
  </style>
</svg>`;
}

async function renderSvgToPngDataUrl(svg: string): Promise<string> {
  const svgDataUrl = `data:image/svg+xml;base64,${Buffer.from(svg, "utf8").toString("base64")}`;
  const result = await runAttachmentSnapshotScript<string>(
    `new Promise((resolve) => {
        const canvasToDataUrl = (canvas, type, quality) => new Promise((done) => {
          if (!canvas.toBlob) {
            done(canvas.toDataURL(type, quality));
            return;
          }
          canvas.toBlob((blob) => {
            if (!blob) {
              done("");
              return;
            }
            const reader = new FileReader();
            reader.onload = () => done(typeof reader.result === "string" ? reader.result : "");
            reader.onerror = () => done("");
            reader.readAsDataURL(blob);
          }, type, quality);
        });
        const image = new Image();
        const done = (value) => resolve(value);
        const timer = setTimeout(() => done(${JSON.stringify(svgDataUrl)}), 4000);
        image.onload = async () => {
          try {
            const canvas = document.createElement("canvas");
            canvas.width = ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE};
            canvas.height = ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE};
            const context = canvas.getContext("2d");
            if (!context) {
              done(${JSON.stringify(svgDataUrl)});
              return;
            }
            context.drawImage(image, 0, 0, canvas.width, canvas.height);
            clearTimeout(timer);
            done(await canvasToDataUrl(canvas, "image/png"));
          } catch {
            done(${JSON.stringify(svgDataUrl)});
          }
        };
        image.onerror = () => done(${JSON.stringify(svgDataUrl)});
        image.src = ${JSON.stringify(svgDataUrl)};
      })`,
    svgDataUrl
  );
  return typeof result === "string" && result.startsWith("data:image/") ? result : svgDataUrl;
}

async function visualSummarySnapshot(
  attachment: ProviderAttachment,
  source: ProviderVisualSnapshot["source"],
  title: string,
  lines: string[]
): Promise<ProviderVisualSnapshot> {
  const imageUrl = await renderSvgToPngDataUrl(summarySnapshotSvg(title, `${attachment.kind} / ${attachment.mimeType}`, lines));
  return visualSnapshot(`${source}: ${attachment.name}`, source, imageUrl);
}

async function captureImageSnapshot(attachment: ProviderAttachment): Promise<string> {
  const sourceUrl = pathToFileURL(attachment.path).toString();
  const result = await runAttachmentSnapshotScript<string>(
    `new Promise((resolve) => {
        const canvasToDataUrl = (canvas, type, quality) => new Promise((done) => {
          if (!canvas.toBlob) {
            done(canvas.toDataURL(type, quality));
            return;
          }
          canvas.toBlob((blob) => {
            if (!blob) {
              done("");
              return;
            }
            const reader = new FileReader();
            reader.onload = () => done(typeof reader.result === "string" ? reader.result : "");
            reader.onerror = () => done("");
            reader.readAsDataURL(blob);
          }, type, quality);
        });
        const image = new Image();
        const timer = setTimeout(() => resolve(""), 8000);
        image.onload = async () => {
          try {
            const maxSize = ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE};
            const scale = Math.min(1, maxSize / Math.max(image.naturalWidth || 1, image.naturalHeight || 1));
            const width = Math.max(1, Math.round((image.naturalWidth || maxSize) * scale));
            const height = Math.max(1, Math.round((image.naturalHeight || maxSize) * scale));
            const canvas = document.createElement("canvas");
            canvas.width = width;
            canvas.height = height;
            const context = canvas.getContext("2d");
            if (!context) {
              resolve("");
              return;
            }
            context.drawImage(image, 0, 0, width, height);
            clearTimeout(timer);
            resolve(await canvasToDataUrl(canvas, "image/jpeg", 0.88));
          } catch {
            resolve("");
          }
        };
        image.onerror = () => resolve("");
        image.src = ${JSON.stringify(sourceUrl)};
      })`,
    ""
  );
  return typeof result === "string" ? result : "";
}

async function analyzeVideoAttachment(attachment: ProviderAttachment): Promise<{ snapshots: ProviderVisualSnapshot[]; metadata?: ProviderVideoMetadata }> {
  const sourceUrl = uploadPreviewUrl(attachment.id, attachment.name);
  try {
    const result = await runAttachmentSnapshotScript<unknown>(
      `new Promise((resolve) => {
        const canvasToDataUrl = (canvas, type, quality) => new Promise((done) => {
          if (!canvas.toBlob) {
            done(canvas.toDataURL(type, quality));
            return;
          }
          canvas.toBlob((blob) => {
            if (!blob) {
              done("");
              return;
            }
            const reader = new FileReader();
            reader.onload = () => done(typeof reader.result === "string" ? reader.result : "");
            reader.onerror = () => done("");
            reader.readAsDataURL(blob);
          }, type, quality);
        });
        const video = document.createElement("video");
        let settled = false;
        const finish = (value) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          resolve(value);
        };
        video.muted = true;
        video.playsInline = true;
        video.preload = "auto";
        video.style.position = "fixed";
        video.style.left = "-10000px";
        video.style.top = "0";
        video.style.width = "1px";
        video.style.height = "1px";
        document.body.appendChild(video);
        const timer = setTimeout(() => finish({ frames: [], metadata: null }), 20000);
        const waitForVideoFrame = () => new Promise((frameResolve) => {
          if (video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA && video.videoWidth > 0 && video.videoHeight > 0) {
            frameResolve(true);
            return;
          }
          let timeout = 0;
          const done = () => {
            video.removeEventListener("loadeddata", done);
            video.removeEventListener("canplay", done);
            if (timeout) clearTimeout(timeout);
            frameResolve(video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA && video.videoWidth > 0 && video.videoHeight > 0);
          };
          video.addEventListener("loadeddata", done, { once: true });
          video.addEventListener("canplay", done, { once: true });
          timeout = setTimeout(done, 4500);
        });
        const seekTo = (time) => new Promise((seekResolve) => {
          let timeout = 0;
          const done = () => {
            video.removeEventListener("seeked", done);
            if (timeout) clearTimeout(timeout);
            seekResolve(true);
          };
          video.addEventListener("seeked", done, { once: true });
          timeout = setTimeout(done, 3000);
          try {
            video.currentTime = Math.max(0, time);
          } catch {
            done();
          }
        });
        const timeLabel = (seconds) => {
          const safe = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
          const total = Math.round(safe * 1000) / 1000;
          return total;
        };
        const snapshotTimes = (duration) => {
          if (!Number.isFinite(duration) || duration <= 0) return [0];
          const maxFrames = ${PANELS_CHAT_BOTTOM_MAX_VISUAL_SNAPSHOTS};
          const candidates = duration > 12
            ? [0.5, duration * 0.1, duration * 0.25, duration * 0.5, duration * 0.75, Math.max(0.5, duration - 0.5)]
            : Array.from({ length: Math.min(maxFrames, Math.max(1, Math.ceil(duration / 2))) }, (_, index, arr) => {
                const count = Math.max(1, arr.length);
                return duration * ((index + 0.5) / count);
              });
          const unique = [];
          for (const raw of candidates) {
            const time = Math.min(Math.max(0, raw), Math.max(0, duration - 0.05));
            if (!unique.some((value) => Math.abs(value - time) < 0.18)) unique.push(time);
            if (unique.length >= maxFrames) break;
          }
          return unique;
        };
        const cueText = (cue) => {
          if (typeof cue.text === "string") return cue.text;
          try {
            return cue.getCueAsHTML ? cue.getCueAsHTML().textContent || "" : "";
          } catch {
            return "";
          }
        };
        const collectTextTracks = () => {
          const tracks = [];
          const list = video.textTracks;
          for (let index = 0; index < list.length; index += 1) {
            const track = list[index];
            try {
              track.mode = "hidden";
            } catch {}
            const cueList = track.cues || track.activeCues;
            const cues = [];
            if (cueList) {
              for (let cueIndex = 0; cueIndex < cueList.length && cues.length < ${PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_CUES}; cueIndex += 1) {
                const cue = cueList[cueIndex];
                const text = cueText(cue).replace(/\\s+/g, " ").trim();
                if (text) {
                  cues.push({
                    start: timeLabel(cue.startTime),
                    end: timeLabel(cue.endTime),
                    text
                  });
                }
              }
            }
            tracks.push({
              index,
              kind: track.kind || "",
              label: track.label || "",
              language: track.language || "",
              cueCount: cueList ? cueList.length : 0,
              cues
            });
          }
          return tracks;
        };
        video.onloadedmetadata = async () => {
          try {
            await video.play().catch(() => undefined);
            video.pause();
            await waitForVideoFrame();
            const duration = Number.isFinite(video.duration) && video.duration > 0 ? video.duration : 0;
            const times = snapshotTimes(duration);
            const canvas = document.createElement("canvas");
            const maxSize = ${PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE};
            const scale = Math.min(1, maxSize / Math.max(video.videoWidth || 1, video.videoHeight || 1));
            canvas.width = Math.max(1, Math.round((video.videoWidth || maxSize) * scale));
            canvas.height = Math.max(1, Math.round((video.videoHeight || maxSize) * scale));
            const context = canvas.getContext("2d");
            if (!context) {
              finish({ frames: [], metadata: null });
              return;
            }
            const frames = [];
            for (const time of times) {
              await seekTo(time);
              await waitForVideoFrame();
              context.drawImage(video, 0, 0, canvas.width, canvas.height);
              frames.push({ time: timeLabel(time), imageUrl: await canvasToDataUrl(canvas, "image/jpeg", 0.86) });
            }
            await new Promise((cueResolve) => setTimeout(cueResolve, 120));
            const textTracks = collectTextTracks();
            finish({
              frames,
              metadata: {
                durationSeconds: timeLabel(duration),
                width: video.videoWidth || 0,
                height: video.videoHeight || 0,
                snapshotTimes: times.map(timeLabel),
                textTracks
              }
            });
          } catch {
            finish({ frames: [], metadata: null });
          }
        };
        video.onerror = () => finish({ frames: [], metadata: null });
        video.src = ${JSON.stringify(sourceUrl)};
        video.load();
      })`,
      {}
    );
    const record = result && typeof result === "object" ? result as {
      frames?: unknown[];
      metadata?: {
        durationSeconds?: unknown;
        width?: unknown;
        height?: unknown;
        snapshotTimes?: unknown[];
        textTracks?: unknown[];
      } | null;
    } : {};
    const frames = Array.isArray(record.frames)
      ? record.frames
        .map((frame) => frame && typeof frame === "object" ? frame as { time?: unknown; imageUrl?: unknown } : null)
        .filter((frame): frame is { time?: unknown; imageUrl: string } => frame !== null && typeof frame.imageUrl === "string" && frame.imageUrl.startsWith("data:image/"))
      : [];
    const snapshots = frames.map((frame, index) => {
      const time = typeof frame.time === "number" && Number.isFinite(frame.time) ? frame.time : index;
      return visualSnapshot(`video frame ${index + 1} @ ${time.toFixed(3)}s: ${attachment.name}`, "video-frame", frame.imageUrl as string);
    });
    const rawMetadata = record.metadata && typeof record.metadata === "object" ? record.metadata : null;
    const width = typeof rawMetadata?.width === "number" && Number.isFinite(rawMetadata.width) ? rawMetadata.width : 0;
    const height = typeof rawMetadata?.height === "number" && Number.isFinite(rawMetadata.height) ? rawMetadata.height : 0;
    const durationSeconds = typeof rawMetadata?.durationSeconds === "number" && Number.isFinite(rawMetadata.durationSeconds) ? rawMetadata.durationSeconds : 0;
    const textTracks = Array.isArray(rawMetadata?.textTracks)
      ? rawMetadata.textTracks.map((track, index): ProviderVideoTextTrack => {
        const item = track && typeof track === "object" ? track as Record<string, unknown> : {};
        const cues = Array.isArray(item.cues)
          ? item.cues.map((cue): ProviderVideoSubtitleCue | null => {
            const recordCue = cue && typeof cue === "object" ? cue as Record<string, unknown> : {};
            const text = typeof recordCue.text === "string" ? recordCue.text.trim() : "";
            if (!text) return null;
            return {
              start: typeof recordCue.start === "number" && Number.isFinite(recordCue.start) ? recordCue.start : 0,
              end: typeof recordCue.end === "number" && Number.isFinite(recordCue.end) ? recordCue.end : 0,
              text
            };
          }).filter((cue): cue is ProviderVideoSubtitleCue => Boolean(cue))
          : [];
        return {
          index: typeof item.index === "number" ? item.index : index,
          kind: typeof item.kind === "string" ? item.kind : "",
          label: typeof item.label === "string" ? item.label : "",
          language: typeof item.language === "string" ? item.language : "",
          cueCount: typeof item.cueCount === "number" ? item.cueCount : cues.length,
          cues
        };
      })
      : [];
    const metadata: ProviderVideoMetadata | undefined = rawMetadata ? {
      durationSeconds,
      durationLabel: durationLabel(durationSeconds),
      width,
      height,
      aspectRatio: width > 0 && height > 0 ? width / height : 0,
      resolutionLabel: width > 0 && height > 0 ? `${width}x${height}` : "resolution=unavailable",
      qualityLabel: videoQualityLabel(width, height),
      snapshotTimes: Array.isArray(rawMetadata?.snapshotTimes)
        ? rawMetadata.snapshotTimes.filter((time): time is number => typeof time === "number" && Number.isFinite(time))
        : frames.map((frame, index) => typeof frame.time === "number" && Number.isFinite(frame.time) ? frame.time : index),
      textTracks,
      subtitlesExtracted: textTracks.some((track) => track.cues.length > 0)
    } : undefined;
    return { snapshots, metadata };
  } catch (error) {
    console.error("Attachment video snapshot failed.", error);
    return { snapshots: [] };
  }
}

type Bounds3 = {
  minX: number;
  minY: number;
  minZ: number;
  maxX: number;
  maxY: number;
  maxZ: number;
  count: number;
};

function emptyBounds3(): Bounds3 {
  return {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
    count: 0
  };
}

function includePoint(bounds: Bounds3, x: number, y: number, z: number): void {
  if (![x, y, z].every(Number.isFinite)) {
    return;
  }
  bounds.minX = Math.min(bounds.minX, x);
  bounds.minY = Math.min(bounds.minY, y);
  bounds.minZ = Math.min(bounds.minZ, z);
  bounds.maxX = Math.max(bounds.maxX, x);
  bounds.maxY = Math.max(bounds.maxY, y);
  bounds.maxZ = Math.max(bounds.maxZ, z);
  bounds.count += 1;
}

function numericText(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3).replace(/\.?0+$/g, "") : "?";
}

function boundsText(bounds: Bounds3): string {
  if (bounds.count === 0) {
    return "bounds=unavailable";
  }
  return `bounds=(${numericText(bounds.minX)},${numericText(bounds.minY)},${numericText(bounds.minZ)}) -> (${numericText(bounds.maxX)},${numericText(bounds.maxY)},${numericText(bounds.maxZ)})`;
}

function objSummary(text: string): string[] {
  const bounds = emptyBounds3();
  let vertices = 0;
  let faces = 0;
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("v ")) {
      const parts = line.trim().split(/\s+/).slice(1).map(Number);
      includePoint(bounds, parts[0], parts[1], parts[2]);
      vertices += 1;
    } else if (line.startsWith("f ")) {
      faces += 1;
    }
  }
  return ["format=obj", `vertices=${vertices}`, `faces=${faces}`, boundsText(bounds)];
}

function asciiStlSummary(text: string): string[] {
  const bounds = emptyBounds3();
  let vertices = 0;
  let facets = 0;
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("facet normal")) {
      facets += 1;
    } else if (trimmed.startsWith("vertex ")) {
      const parts = trimmed.split(/\s+/).slice(1).map(Number);
      includePoint(bounds, parts[0], parts[1], parts[2]);
      vertices += 1;
    }
  }
  return ["format=stl-ascii", `vertices=${vertices}`, `facets=${facets}`, boundsText(bounds)];
}

function binaryStlSummary(bytes: Buffer): string[] {
  if (bytes.length < 84) {
    return ["format=stl-binary", "triangles=unavailable", "bounds=unavailable"];
  }
  const declaredTriangles = bytes.readUInt32LE(80);
  const availableTriangles = Math.max(0, Math.floor((bytes.length - 84) / 50));
  const parsedTriangles = Math.min(declaredTriangles, availableTriangles, 10000);
  const bounds = emptyBounds3();
  for (let triangle = 0; triangle < parsedTriangles; triangle += 1) {
    const offset = 84 + triangle * 50 + 12;
    for (let vertex = 0; vertex < 3; vertex += 1) {
      const vertexOffset = offset + vertex * 12;
      includePoint(
        bounds,
        bytes.readFloatLE(vertexOffset),
        bytes.readFloatLE(vertexOffset + 4),
        bytes.readFloatLE(vertexOffset + 8)
      );
    }
  }
  return [
    "format=stl-binary",
    `triangles=${declaredTriangles}`,
    parsedTriangles < declaredTriangles ? `sampled_triangles=${parsedTriangles}` : "",
    boundsText(bounds)
  ].filter(Boolean);
}

function gltfSummaryFromJson(json: Record<string, unknown>, format: string): string[] {
  const asset = json.asset && typeof json.asset === "object" ? (json.asset as Record<string, unknown>) : {};
  const meshes = Array.isArray(json.meshes) ? json.meshes.length : 0;
  const nodes = Array.isArray(json.nodes) ? json.nodes.length : 0;
  const materials = Array.isArray(json.materials) ? json.materials.length : 0;
  const scenes = Array.isArray(json.scenes) ? json.scenes.length : 0;
  return [
    `format=${format}`,
    typeof asset.version === "string" ? `gltf_version=${asset.version}` : "",
    `meshes=${meshes}`,
    `nodes=${nodes}`,
    `materials=${materials}`,
    `scenes=${scenes}`
  ].filter(Boolean);
}

function glbSummary(bytes: Buffer): string[] {
  if (bytes.length < 20 || bytes.readUInt32LE(0) !== 0x46546c67) {
    return ["format=glb", "json_chunk=unavailable"];
  }
  const jsonLength = bytes.readUInt32LE(12);
  const chunkType = bytes.readUInt32LE(16);
  if (chunkType !== 0x4e4f534a || bytes.length < 20 + jsonLength) {
    return ["format=glb", "json_chunk=unavailable"];
  }
  try {
    const parsed = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString("utf8")) as Record<string, unknown>;
    return gltfSummaryFromJson(parsed, "glb");
  } catch {
    return ["format=glb", "json_chunk=parse_failed"];
  }
}

async function model3dSummaryForFile(filePath: string): Promise<string> {
  const extension = extname(filePath).toLowerCase();
  const bytes = await readFilePrefix(filePath, 8 * 1024 * 1024);
  const text = bytes.toString("utf8");
  let lines: string[];
  if (extension === ".obj") {
    lines = objSummary(text);
  } else if (extension === ".stl") {
    lines = text.trimStart().startsWith("solid") ? asciiStlSummary(text) : binaryStlSummary(bytes);
  } else if (extension === ".glb") {
    lines = glbSummary(bytes);
  } else if (extension === ".gltf") {
    try {
      lines = gltfSummaryFromJson(JSON.parse(text) as Record<string, unknown>, "gltf");
    } catch {
      lines = ["format=gltf", "json=parse_failed"];
    }
  } else {
    lines = [`format=${extension.slice(1) || "model3d"}`, "geometry_summary=unsupported"];
  }
  return [`3D model summary for ${basename(filePath)}`, ...lines].join("\n");
}

async function visualSnapshotsForAttachment(
  attachment: ProviderAttachment,
  generatedPreview = ""
): Promise<ProviderVisualSnapshot[]> {
  try {
    if (isInlineVisionAttachment(attachment)) {
      return [visualSnapshot(`image: ${attachment.name}`, "image", await attachmentDataUrl(attachment))];
    }
    if (attachment.kind === "image") {
      const imageUrl = await captureImageSnapshot(attachment);
      if (imageUrl) {
        return [visualSnapshot(`image snapshot: ${attachment.name}`, "image", imageUrl)];
      }
      return [
        await visualSummarySnapshot(attachment, "image", attachment.name, [
          "image snapshot unavailable",
          `mime=${attachment.mimeType}`,
          `bytes=${attachment.sizeBytes}`
        ])
      ];
    }
    if (attachment.kind === "video") {
      const analysis = await analyzeVideoAttachment(attachment);
      attachment.videoMetadata = analysis.metadata;
      const frames = analysis.snapshots;
      if (frames.length > 0) {
        return frames.slice(0, PANELS_CHAT_BOTTOM_MAX_VISUAL_SNAPSHOTS);
      }
      return [
        await visualSummarySnapshot(attachment, "video-frame", attachment.name, [
          "video frame extraction unavailable",
          `mime=${attachment.mimeType}`,
          `bytes=${attachment.sizeBytes}`,
          ...videoMetadataText(attachment.videoMetadata).split(/\r?\n/).slice(0, 12)
        ])
      ];
    }
    if (attachment.kind === "model3d") {
      return [
        await visualSummarySnapshot(
          attachment,
          "model3d-summary",
          attachment.name,
          (generatedPreview || "3D model summary unavailable").split(/\r?\n/)
        )
      ];
    }
  } catch (error) {
    console.error("Attachment visual normalization failed.", error);
  }
  return [];
}

function attachmentTextContext(attachments: ProviderAttachment[]): string {
  const lines = attachments.map((attachment, index) => {
    const imageEditRole = attachment.editRole === "editable_input" ? " role=editable_input" : "";
    const snapshotLine = attachment.visualSnapshots.length > 0
      ? ` snapshots=${attachment.visualSnapshots.map((snapshot) => `${snapshot.source}:${snapshot.proofHash.slice(0, 12)}`).join("|")}`
      : "";
    const openAiFileLine = attachment.openAiFileDataUrl ? " openai_input_file=true" : "";
    const header = [
      `Attachment ${index + 1}: ${attachment.name}`,
      `kind=${attachment.kind}`,
      `mime=${attachment.mimeType}`,
      `bytes=${attachment.sizeBytes}`,
      `proof=${attachment.proofHash.slice(0, 16)}${imageEditRole}${snapshotLine}${openAiFileLine}`
    ].join(" ");
    if (attachment.llmTextPreview.trim()) {
      return `${header}\n${attachment.llmTextPreview.trim()}`;
    }
    return header;
  });
  return lines.join("\n\n").trim();
}

function userTextWithAttachmentContext(userText: string, attachments: ProviderAttachment[]): string {
  const context = attachmentTextContext(attachments);
  return [userText.trim(), context ? `Attached local files:\n${context}` : ""].filter(Boolean).join("\n\n");
}

function isFirstVisibleUserTurn(userMessageId: string, transcript: TranscriptMessage[]): boolean {
  const visibleUserMessages = transcript.filter((message) =>
    message.role === "user" &&
    !isInternalTranscriptMessage(message) &&
    message.text.trim() !== ""
  );
  if (userMessageId) {
    return visibleUserMessages.length === 1 && visibleUserMessages[0]?.id === userMessageId;
  }
  return visibleUserMessages.length <= 1;
}

async function openAiResponseContent(userText: string, attachments: ProviderAttachment[]): Promise<OpenAiResponseContentPart[]> {
  const text = userTextWithAttachmentContext(userText, attachments);
  const content: OpenAiResponseContentPart[] = [
    {
      type: "input_text",
      text: text || "Analyze the sent attachments."
    }
  ];
  for (const attachment of attachments) {
    if (attachment.openAiFileDataUrl) {
      content.push({
        type: "input_file",
        filename: attachment.name,
        file_data: attachment.openAiFileDataUrl
      });
    }
  }
  for (const snapshot of attachments.flatMap((attachment) => attachment.visualSnapshots)) {
    content.push({
      type: "input_image",
      image_url: snapshot.imageUrl,
      detail: "auto"
    });
  }
  return content;
}

async function openAiResponseConversationInput(
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): Promise<OpenAiResponseInputItem[]> {
  return [
    ...recentConversationWindow(userMessageId, transcript).map((message) => {
      if (message.role === "system") {
        return {
          role: "user",
          content: [{
            type: "input_text",
            text: `Session Brain boot context. Read this once as the visible Brain for this session; it is repeated only as session history or after compression.\n${message.content}`
          }]
        } as OpenAiResponseInputItem;
      }
      return {
        role: message.role,
        content: message.content
      } as OpenAiResponseInputItem;
    }),
    {
      role: "user",
      content: await openAiResponseContent(userText, attachments)
    }
  ];
}

async function openRouterUserContent(userText: string, attachments: ProviderAttachment[]): Promise<string | OpenRouterContentPart[]> {
  const text = userTextWithAttachmentContext(userText, attachments) || "Analyze the sent attachments.";
  const snapshots = attachments.flatMap((attachment) => attachment.visualSnapshots);
  if (snapshots.length === 0) {
    return text;
  }
  const content: OpenRouterContentPart[] = [{ type: "text", text }];
  for (const snapshot of snapshots) {
    content.push({
      type: "image_url",
      image_url: { url: snapshot.imageUrl }
    });
  }
  return content;
}

function selectedComposerModel(profile: ProviderRuntimeProfile): string {
  const models = profile.models.length > 0 ? profile.models : [];
  const index = Math.min(panelsChatBottomState.modelIndex, Math.max(0, models.length - 1));
  return models[index] ?? "";
}

function composerModelDisplayLabel(provider: LlmProviderConnectId, model: string): string {
  if (provider !== "claude") {
    return model;
  }
  const match = /^(?:claude-)?(sonnet|haiku|opus)(?:-(.+))?$/i.exec(model.trim());
  if (!match) {
    return model;
  }
  const family = match[1].slice(0, 1).toUpperCase() + match[1].slice(1).toLowerCase();
  const suffix = match[2]?.trim();
  return suffix ? `Claude ${family} ${suffix.replace(/-/g, ".")}` : family;
}

function selectedComposerReasoning(profile: ProviderRuntimeProfile): string {
  const reasoning = profile.reasoning.length > 0 ? profile.reasoning : [];
  const index = Math.min(panelsChatBottomState.reasoningIndex, Math.max(0, reasoning.length - 1));
  return reasoning[index] ?? "";
}

function normalizedReasoningEffort(value: string): "low" | "medium" | "high" | "xhigh" | "max" | "" {
  const lower = value.trim().toLowerCase();
  if (lower === "bas") {
    return "low";
  }
  if (lower === "moyen") {
    return "medium";
  }
  if (lower === "élevé" || lower === "eleve") {
    return "high";
  }
  if (lower === "très approfondi" || lower === "tres approfondi") {
    return "xhigh";
  }
  if (lower === "deep" || lower === "very deep") {
    return "xhigh";
  }
  if (lower === "normal" || lower === "medium" || lower === "extended") {
    return "medium";
  }
  if (lower === "minimal" || lower === "none") {
    return "low";
  }
  if (lower === "low" || lower === "high" || lower === "xhigh" || lower === "max") {
    return lower;
  }
  return "";
}

type ProviderConversationMessage = { role: "system" | "user" | "assistant"; content: string };

interface PlannedConversationMessage extends ProviderConversationMessage {
  id: string;
  estimatedTokens: number;
}

interface ConversationContextPlan {
  estimatedTokens: number;
  shouldCompact: boolean;
  recentMessages: ProviderConversationMessage[];
  memoryText: string;
}

function estimatedPromptTokens(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const byChars = Math.ceil(trimmed.length / 4);
  const words = trimmed.split(/\s+/).length;
  return Math.max(byChars, Math.ceil(words * 1.35));
}

function providerConversationContent(message: TranscriptMessage): string {
  const text = trimUtf8Bytes(message.text.trim(), PANELS_CHAT_BOTTOM_CONTEXT_TEXT_BYTES);
  const attachmentNames = (message.attachments ?? [])
    .map((attachment) => {
      const remoteUrl = isRemoteAttachmentUrl(attachment.url) ? ` media_url=${attachment.url}` : "";
      return `${attachment.kind}:${attachment.name}${remoteUrl}`;
    })
    .join(", ");
  return [text, attachmentNames ? `[pieces_jointes: ${attachmentNames}]` : ""].filter(Boolean).join(" ");
}

function providerConversationMessages(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): PlannedConversationMessage[] {
  return transcript
    .filter((message) =>
      !isInternalTranscriptMessage(message) &&
      message.id !== excludeMessageId &&
      ((message.role === "system" && message.id.startsWith(BRAIN_BOOT_MESSAGE_ID_PREFIX)) ||
        message.role === "user" ||
        (message.role === "assistant" &&
          !message.id.startsWith("assistant-pending-") &&
          !message.id.startsWith("assistant-status-"))) &&
      (message.text.trim() !== "" || (message.attachments?.length ?? 0) > 0)
    )
    .map((message) => {
      const content = providerConversationContent(message);
      return {
        id: message.id,
        role: message.role as "system" | "user" | "assistant",
        content,
        estimatedTokens: estimatedPromptTokens(content) + 12
      };
    });
}

function compactedAgentActionMemory(compactedMessages: PlannedConversationMessage[]): string {
  const actionMessages = compactedMessages.filter((message) =>
    /AGENT_ACTION_RESULT|AGENT_ACTION_JSON|AGENT_ACTION_SELECTED_CAPABILITY|\/agent_(?:capabilities|list|search|create_directory|rename_path|move_path|copy_path|delete_empty_directory|delete_tree|readonly_shell|shell)_/i.test(message.content)
  );
  if (actionMessages.length === 0) {
    return "";
  }
  const manifest = createAgentActionHostManifest(agentActionHostConfig());
  const lastUserGoal = [...compactedMessages]
    .reverse()
    .find((message) => message.role === "user")?.content ?? "";
  const lastToolGroundTruth = [...actionMessages]
    .reverse()
    .find((message) => /AGENT_ACTION_RESULT/i.test(message.content))?.content ?? actionMessages.at(-1)?.content ?? "";
  return trimUtf8Bytes([
    "AGENT_ACTION_COMPACTION_STATE v1",
    `manifest_hash=${manifest.runtime.manifestHash}`,
    `atlas_hash=${manifest.runtime.atlasHash}`,
    `result_reinjection=${manifest.runtime.resultReinjectionPolicy}`,
    `executable_actions=${manifest.runtime.executableActionIds.join("|")}`,
    `planned_families=${manifest.runtime.plannedFamilies.join("|")}`,
    `blocked_families=${manifest.runtime.blockedFamilies.join("|")}`,
    `approval_gated_families=${manifest.runtime.approvalGatedFamilies.join("|")}`,
    `installed_tools=${manifest.runtime.installedToolIds.join("|")}`,
    `active_goal=${trimUtf8Bytes(lastUserGoal, 900)}`,
    `last_tool_ground_truth=${trimUtf8Bytes(lastToolGroundTruth, 1400)}`,
    "rule=After compaction, continue the same observe-act-verify-retry loop. Treat the last tool result as ground truth, verify before claiming success, and do not ask for the full atlas unless a selected capability detail is insufficient."
  ].join("\n"), 5000);
}

function compactedConversationMemory(
  compactedMessages: PlannedConversationMessage[],
  estimatedTokens: number
): string {
  const agentActionMemory = compactedAgentActionMemory(compactedMessages);
  const header = [
    "BRAIN_REINJECTED_AFTER_COMPACTION:",
    brainBootManifest(),
    agentActionMemory,
    "",
    "CONVERSATION_COMPACTION v1",
    `estimated_transcript_tokens=${estimatedTokens}`,
    `compact_after_tokens=${PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS}`,
    `recent_raw_token_budget=${PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS}`,
    "Compact memory from the previous conversation:"
  ];
  const selected: string[] = [];
  let usedTokens = estimatedPromptTokens(header.join("\n"));
  for (const message of [...compactedMessages].reverse()) {
    if (message.role === "system") {
      continue;
    }
    const label = message.role === "user" ? "User" : "Assistant";
    const line = `${label}: ${trimUtf8Bytes(message.content, 1200)}`;
    const lineTokens = estimatedPromptTokens(line) + 8;
    if (selected.length > 0 && usedTokens + lineTokens > PANELS_CHAT_BOTTOM_MEMORY_TOKEN_BUDGET) {
      break;
    }
    selected.unshift(line);
    usedTokens += lineTokens;
  }
  return trimUtf8Bytes(
    [
      ...header,
      ...selected,
      selected.length < compactedMessages.length
        ? `messages_compactes_omises=${compactedMessages.length - selected.length}`
        : "",
      "Use this memory as context, without copying it back unless it is useful."
    ].filter(Boolean).join("\n"),
    PANELS_CHAT_BOTTOM_MEMORY_TEXT_BYTES
  );
}

function conversationContextPlan(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): ConversationContextPlan {
  const messages = providerConversationMessages(excludeMessageId, transcript);
  const estimatedTokens =
    messages.reduce((total, message) => total + message.estimatedTokens, 0);
  const shouldCompact = estimatedTokens > PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS;
  if (!shouldCompact) {
    let usedTokens = 0;
    const recentMessages: ProviderConversationMessage[] = [];
    for (const message of [...messages].reverse()) {
      if (recentMessages.length > 0 && usedTokens + message.estimatedTokens > PANELS_CHAT_BOTTOM_CONTEXT_TOKEN_BUDGET) {
        break;
      }
      recentMessages.unshift({ role: message.role, content: message.content });
      usedTokens += message.estimatedTokens;
    }
    return { estimatedTokens, shouldCompact, recentMessages, memoryText: "" };
  }

  let recentStart = messages.length;
  let recentTokens = 0;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const nextTokens = recentTokens + messages[index].estimatedTokens;
    if (recentStart < messages.length && nextTokens > PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS) {
      break;
    }
    recentStart = index;
    recentTokens = nextTokens;
  }
  if (recentStart === messages.length && messages.length > 0) {
    recentStart = messages.length - 1;
  }
  const compactedMessages = messages.slice(0, recentStart);
  const recentMessages = messages
    .slice(recentStart)
    .map((message) => ({ role: message.role, content: message.content }));
  return {
    estimatedTokens,
    shouldCompact,
    recentMessages,
    memoryText: compactedConversationMemory(compactedMessages, estimatedTokens)
  };
}

function recentConversationInput(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): Array<{ role: "system" | "user" | "assistant"; content: string }> {
  return conversationContextPlan(excludeMessageId, transcript)
    .recentMessages
    .filter((message) => message.role === "system" || message.role === "user" || message.content.trim() !== "");
}

function recentConversationWindow(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): Array<{ role: "system" | "user" | "assistant"; content: string }> {
  return conversationContextPlan(excludeMessageId, transcript).recentMessages;
}

function conversationMemoryContext(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  return conversationContextPlan(excludeMessageId, transcript).memoryText;
}

function sessionDocumentMemoryContext(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  const seen = new Set<string>();
  const blocks: string[] = [];
  for (const message of transcript) {
    if (message.id === excludeMessageId) {
      continue;
    }
    for (const preview of message.attachments ?? []) {
      if (seen.has(preview.id)) {
        continue;
      }
      seen.add(preview.id);
      const cached = composerUploadPreviewItems.get(preview.id);
      const previewText = cached?.textPreview?.trim() || preview.textPreview?.trim() || "";
      const tablePreview = cached?.tablePreview?.length
        ? tablePreviewText(cached.tablePreview.slice(0, 18).map((row) => row.slice(0, 8)))
        : "";
      const remoteUrl = isRemoteAttachmentUrl(preview.url) ? preview.url : "";
      const lines = [
        `Document ${blocks.length + 1}: ${preview.name}`,
        `id=${preview.id}`,
        `kind=${preview.kind}`,
        cached?.mimeType ? `mime=${cached.mimeType}` : "",
        remoteUrl ? `media_url=${remoteUrl}` : "",
        remoteUrl ? "remote_media=true" : "",
        `proof=${hashJson(cached ?? preview).slice(0, 16)}`,
        `source_turn=${message.role}:${message.id}`,
        message.role === "assistant" ? "created_or_returned_by_assistant=true" : "provided_by_user=true",
        previewText ? `text_preview:\n${trimUtf8Bytes(previewText, 2400)}` : "",
        tablePreview ? `table_preview:\n${trimUtf8Bytes(tablePreview, 2400)}` : ""
      ].filter(Boolean);
      blocks.push(lines.join("\n"));
    }
  }
  if (blocks.length === 0) {
    return "";
  }
  return trimUtf8Bytes(
    [
      "Session document registry:",
      "All these documents are part of the active context. If they are relevant, use their previews and names for reasoning; request reinjection only if the raw content is missing.",
      `Images in this registry remain editable targets for ${BRAIN_EDITIMAGE_COMMAND}. If the user later asks to modify, remove, replace, clean up, crop, recolor, or restyle an image from the registry, activate ${BRAIN_EDITIMAGE_COMMAND} with image_ref=id or filename instead of asking for a workspace or proposing an external-tool prompt.`,
      ...blocks
    ].join("\n\n"),
    PANELS_CHAT_BOTTOM_DOCUMENT_MEMORY_BYTES
  );
}

function previousUserIntentForAttachmentTurn(
  excludeMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  for (const message of [...transcript].reverse()) {
    if (message.id === excludeMessageId) {
      continue;
    }
    if (message.role !== "user") {
      continue;
    }
    const text = message.text.trim();
    if (text) {
      return trimUtf8Bytes(text, 4096);
    }
  }
  return "";
}

function providerUserTextForTurn(
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  const trimmed = userText.trim();
  const memory = conversationMemoryContext(userMessageId, transcript);
  const documentMemory = sessionDocumentMemoryContext(userMessageId, transcript);
  if (trimmed || attachments.length === 0) {
    return [memory, documentMemory, trimmed].filter(Boolean).join("\n\n");
  }
  const previousIntent = previousUserIntentForAttachmentTurn(userMessageId, transcript);
  if (!previousIntent) {
    return [memory, documentMemory].filter(Boolean).join("\n\n");
  }
  const attachmentFollowUp = [
    "MANDATORY ACTIVE TASK:",
    "The user has just sent attachments to answer the previous request.",
    `Previous request: ${previousIntent}`,
    "The attachments in the current turn are the expected answer to that previous request.",
    "Answer the previous request directly now using the available attachments.",
    "Do not ask what the user wants to do with the image or document.",
    "If the previous request asked for an opinion, give your concrete opinion; if it asked for an analysis, analyze directly."
  ].join("\n");
  return [memory, documentMemory, attachmentFollowUp].filter(Boolean).join("\n\n");
}

interface ProviderTextRun {
  text: string;
  runtime: string;
}

interface ProviderLiveTextSink {
  onText: (text: string) => void;
  shouldStop?: (text: string) => boolean;
}

interface AssistantRunControl {
  id: string;
  sessionId: string;
  cancelled: boolean;
  cancelledAt?: string;
}

class AssistantRunCancelledError extends Error {
  constructor() {
    super("Assistant run stopped by user.");
    this.name = "AssistantRunCancelledError";
  }
}

let activeAssistantRunControl: AssistantRunControl | undefined;

function startAssistantRunControl(sessionId: string): AssistantRunControl {
  if (activeAssistantRunControl && !activeAssistantRunControl.cancelled) {
    activeAssistantRunControl.cancelled = true;
    activeAssistantRunControl.cancelledAt = new Date().toISOString();
  }
  const run: AssistantRunControl = {
    id: `assistant-run-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    sessionId,
    cancelled: false
  };
  activeAssistantRunControl = run;
  emitPanelsChatBottomSnapshotEvent("assistant_run_started", sessionId);
  return run;
}

function finishAssistantRunControl(run: AssistantRunControl): void {
  if (activeAssistantRunControl?.id === run.id) {
    activeAssistantRunControl = undefined;
  }
  emitPanelsChatBottomSnapshotEvent("assistant_run_finished", run.sessionId);
}

function stopActiveAssistantRunControl(): boolean {
  const run = activeAssistantRunControl;
  if (!run || run.cancelled) {
    return false;
  }
  run.cancelled = true;
  run.cancelledAt = new Date().toISOString();
  emitPanelsChatBottomSnapshotEvent("assistant_run_cancelled", run.sessionId);
  return true;
}

function throwIfAssistantRunCancelled(run?: AssistantRunControl): void {
  if (run?.cancelled) {
    throw new AssistantRunCancelledError();
  }
}

interface AssistantProviderRunParams {
  profile: ProviderRuntimeProfile;
  rawUserText: string;
  providerUserText: string;
  attachments: ProviderAttachment[];
  userMessageId: string;
  moduleId: string;
  transcript: TranscriptMessage[];
  liveSink?: ProviderLiveTextSink;
}

interface AssistantProviderAdapter {
  id: LlmProviderConnectId;
  run: (params: AssistantProviderRunParams) => Promise<ProviderTextRun>;
  unavailableReason: (profile: ProviderRuntimeProfile) => string;
}

const ASSISTANT_PROVIDER_ADAPTERS: Record<LlmProviderConnectId, AssistantProviderAdapter> = {
  codex: {
    id: "codex",
    run: (params) =>
      runCodexSubscriptionExec(
        params.profile,
        params.providerUserText,
        params.attachments,
        params.moduleId,
        params.userMessageId,
        params.transcript,
        params.liveSink
      ),
    unavailableReason: (profile) =>
      profile.connected
        ? "Codex est connecte pour la session et le catalogue. La prochaine etape doit etre un pont Codex local/subscription, pas un appel API OpenAI facture a l'usage."
        : `${profile.label} n'est pas encore connecte.`
  },
  claude: {
    id: "claude",
    run: (params) =>
      runClaudeCodePrint(
        params.profile,
        userTextWithAttachmentContext(params.providerUserText, params.attachments),
        params.moduleId,
        params.userMessageId,
        params.transcript,
        params.liveSink
      ),
    unavailableReason: (profile) =>
      profile.connected
        ? `${profile.label} est connecte, mais son runtime conversationnel direct n'est pas encore cable dans ce composer.`
        : `${profile.label} n'est pas encore connecte.`
  },
  openrouter: {
    id: "openrouter",
    run: async (params) => {
      const text = await runOpenRouterChatCompletion(
        params.profile,
        params.providerUserText,
        params.attachments,
        params.userMessageId,
        params.moduleId,
        params.transcript,
        params.liveSink
      );
      const model = selectedComposerModel(params.profile) || "selected model";
      return {
        text,
        runtime: `openrouter chat completion / ${model}`
      };
    },
    unavailableReason: (profile) =>
      profile.connected
        ? `${profile.label} est connecte, mais son runtime conversationnel direct n'est pas encore cable dans ce composer.`
        : `${profile.label} n'est pas encore connecte.`
  }
};

function assistantProviderAdapterFor(profile: ProviderRuntimeProfile): AssistantProviderAdapter {
  return ASSISTANT_PROVIDER_ADAPTERS[profile.connectId];
}

type CodexLocalAuth = {
  accessToken: string;
  refreshToken?: string;
  accountId: string;
  planType?: string;
  authMode?: string;
  lastRefresh?: string;
};

function randomClientId(): string {
  const hex = randomBytes(16).toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function forgeHomeDir(): string | undefined {
  if (process.env.USERPROFILE) {
    return process.env.USERPROFILE;
  }
  if (process.env.HOMEDRIVE && process.env.HOMEPATH) {
    return `${process.env.HOMEDRIVE}${process.env.HOMEPATH}`;
  }
  return process.env.HOME;
}

async function readCodexLocalAuth(): Promise<CodexLocalAuth | undefined> {
  const home = forgeHomeDir();
  if (!home) {
    return undefined;
  }
  const authPath = join(home, ".codex", "auth.json");
  let parsed: unknown;
  try {
    parsed = JSON.parse(await readFile(authPath, "utf8")) as unknown;
  } catch {
    return undefined;
  }
  if (!parsed || typeof parsed !== "object") {
    return undefined;
  }
  const root = parsed as Record<string, unknown>;
  const tokens = root.tokens && typeof root.tokens === "object" ? (root.tokens as Record<string, unknown>) : {};
  const accessToken = typeof tokens.access_token === "string" ? tokens.access_token.trim() : "";
  const refreshToken = typeof tokens.refresh_token === "string" && tokens.refresh_token.trim()
    ? tokens.refresh_token.trim()
    : undefined;
  const accountIdCandidates = [tokens.account_id, tokens.chatgpt_account_id, root.chatgpt_account_id];
  const accountId = accountIdCandidates.find((value): value is string => typeof value === "string" && value.trim() !== "")
    ?.trim() ?? "";
  if (!accessToken && !refreshToken) {
    return undefined;
  }
  return {
    accessToken,
    refreshToken,
    accountId,
    planType: [tokens.plan_type, tokens.chatgpt_plan_type, root.chatgpt_plan_type]
      .find((value): value is string => typeof value === "string" && value.trim() !== "")
      ?.trim(),
    authMode: typeof root.auth_mode === "string" && root.auth_mode.trim() ? root.auth_mode.trim() : undefined,
    lastRefresh: typeof root.last_refresh === "string" && root.last_refresh.trim() ? root.last_refresh.trim() : undefined
  };
}

async function applyCodexLocalAuthProfile(eventsPrefix: string[] = []): Promise<ProviderRuntimeProfile | undefined> {
  const auth = await readCodexLocalAuth();
  if (!auth || !auth.accessToken || !auth.accountId) {
    return undefined;
  }
  const profile = providerRuntime.codex;
  profile.connected = true;
  profile.account = auth.accountId ? `ChatGPT account ${auth.accountId.slice(0, 8)}...${auth.accountId.slice(-4)}` : "ChatGPT subscription account";
  profile.models = [...CODEX_DESKTOP_MODELS];
  profile.reasoning = [...CODEX_DESKTOP_REASONING];
  profile.quotaLabel = "quota unavailable: official token balance not returned";
  profile.proof = hashJson({
    provider: "codex",
    source: "codex_local_oauth",
    accountId: auth.accountId,
    authMode: auth.authMode,
    lastRefresh: auth.lastRefresh,
    models: profile.models,
    reasoning: profile.reasoning
  });
  const events = [
    ...eventsPrefix,
    "Codex OAuth credentials found at ~/.codex/auth.json",
    "ChatGPT subscription account id present",
    "model catalog received from Codex Desktop",
    ...modelCatalogEvents(profile.models),
    `reasoning ${profile.reasoning.join(" / ")}`,
    profile.quotaLabel,
    "ready"
  ];
  markProviderReadyEvents(profile, events);
  activateComposerProvider(profile);
  await persistProviderRuntime();
  emitLlmProviderRuntimeEvent({
    provider: "codex",
    events,
    models: profile.models,
    reasoning: profile.reasoning,
    quotaLabel: profile.quotaLabel,
    proofHash: profile.proof
  });
  return profile;
}

function codexRuntimeModel(profile: ProviderRuntimeProfile): string {
  const selected = selectedComposerModel(profile);
  if (selected && !/catalog unavailable|connect provider/i.test(selected)) {
    return codexDesktopModelSlug(selected);
  }
  return codexDesktopModelSlug(profile.models[0] ?? CODEX_DESKTOP_MODELS[0]);
}

function codexRuntimeReasoning(profile: ProviderRuntimeProfile): string {
  const selected = normalizedReasoningEffort(selectedComposerReasoning(profile));
  if (selected === "max") {
    return "high";
  }
  return selected || "medium";
}

function webExplorerCodeActInstructions(moduleId = ""): string {
  if (moduleId === "gmail") {
    return [
      "Active module: Gmail. If a Gmail action is requested, write your own natural sentence adapted to the user's request, then explicitly activate the Gmail CodeAct with its slots.",
      `To open Gmail directly, activate ${BRAIN_GMAIL_COM_COMMAND} after your natural sentence.`,
      "The app automatically displays the event when the CodeAct is active; do not describe the technical event in your sentence.",
      `Template Gmail: ${BRAIN_GMAIL_COMMAND} intent="open|search|inspect|summarize|draft|reply" query="..." keywords="..." recipient="..." subject="..." body="...".`,
      "Never send an email yourself: draft/reply only prepares a draft for user validation.",
      `Do not use ${BRAIN_GOOGLEWEB_COMMAND} for a Gmail action.`
    ].join("\n");
  }
  if (moduleId === "airbnb") {
    return [
      "Active module: Airbnb. If an Airbnb action is requested, write your own natural sentence adapted to the user's request, then explicitly activate the Airbnb CodeAct with its slots.",
      BRAIN_CODEACT_ROUTING_RULES,
      "Your response must never be only a CodeAct command: it must contain a normal sentence for the user, and that same sentence must be copied into the say slot.",
      "The app automatically displays the event when the CodeAct is active; do not describe the technical event in your sentence.",
      `Template Airbnb: ${BRAIN_AIRBNB_COMMAND} intent="open|search|inspect" say="natural LLM sentence visible to the user" query="..." keywords="...".`,
      `To open Airbnb directly or start an Airbnb search, activate ${BRAIN_AIRBNB_COMMAND} after your natural sentence.`,
      `Do not use ${BRAIN_GOOGLEWEB_COMMAND} for an Airbnb action.`
    ].join("\n");
  }
  if (moduleId === "compute") {
    return [
      `Active module: Compute. If the request contains a formula, calculation, recurrence, numeric simulation, or mathematical verification, write a short natural sentence, then explicitly activate ${BRAIN_NEWCOMPUTE_COMMAND}.`,
      BRAIN_CODEACT_ROUTING_RULES,
      `The composer text can contain a formula injected from the Canvas: treat it as Compute input and transform it into a ${BRAIN_NEWCOMPUTE_COMMAND} command instead of only explaining it in prose.`,
      "The app automatically displays the Compute event when the CodeAct is active; do not describe the technical event in your sentence.",
      `Use ${BRAIN_GOOGLEWEB_COMMAND} only if the calculation requires external web research before Compute.`
    ].join("\n");
  }
  return [
    "Follow the Brain CodeAct commands and priorities already provided at session start.",
    BRAIN_CODEACT_ROUTING_RULES,
    `When the active Brain is general and a request matches ${BRAIN_SCIENCE_COMMAND} or ${BRAIN_CODING_COMMAND}, activate that Brain CodeAct first with one short natural sentence; do not start with a long specialized answer before switching.`,
    `Use ${BRAIN_GOOGLEWEB_COMMAND} only for generic web research that is not covered by a specific Brain module.`,
    `Strict geography rule: ${BRAIN_MAPS_COMMAND} requires an explicit map/location/route/coordinates/Google Earth/local weather intent. A country, city, region, or historical period in a cultural, literary, historical, table, vocabulary, or explanation request is never enough: use ${BRAIN_WEBSEARCH_COMMAND} instead, then ${BRAIN_SCRAPERS_COMMAND} if sources, images, or media should enrich the conversation. If the same place appears with travel, vacation, visit, tourism, stay, destination, dates, travelers, lodging, hotel, rental, or booking, write a natural sentence, then activate ${BRAIN_MAPS_COMMAND} first and ${BRAIN_AIRBNB_COMMAND} as the next WebExplorer page. ${BRAIN_MAPS_COMMAND} without a target opens a neutral Earth view; never use user_home_location as the default target. Never read the computer location without explicit permission.`,
    `If the user asks to generate/create an image, write a natural sentence, then activate ${BRAIN_NEWIMAGE_COMMAND} with say and prompt.`,
    `If the user asks to modify/retouch/transform an attached, selected, or session-document image, write a natural sentence, then activate ${BRAIN_EDITIMAGE_COMMAND} with say, instruction, and image_ref.`,
    `For requests such as removing an element from an image, removing the background, changing colors, or editing an image, do not write a prompt for an external tool: activate ${BRAIN_EDITIMAGE_COMMAND}.`,
    `Never activate ${BRAIN_WORKSPACE_COMMAND} to generate or edit an image; workspace is only for local code/files/project work.`,
    "If no image is available in the composer, selection, latest visible visual, or document registry, ask which image to edit instead of activating the CodeAct.",
    "The app does not infer the user request; it only executes the CodeAct commands you wrote."
  ].join("\n");
}

interface BrainIdentityContext {
  userFirstName: string;
  agentFirstName: string;
  userHomeLocation: string;
  personalityManifest: string;
  durableMemoryManifest: string;
}

const brainIdentityContext: BrainIdentityContext = {
  userFirstName: "",
  agentFirstName: "",
  userHomeLocation: "",
  personalityManifest: "",
  durableMemoryManifest: ""
};

function memoryRootPath(): string {
  return join(app.getPath("userData"), INGEN_MEMORY_ROOT_DIR_NAME);
}

function brainIdentityStorePath(): string {
  return join(memoryRootPath(), "identity-memory.json");
}

function brainRendererMemoryStorePath(): string {
  return join(memoryRootPath(), "renderer-brain-memory.json");
}

function sessionArtifactLedgerStorePath(): string {
  return join(memoryRootPath(), "session-artifacts.json");
}

function sessionArtifactAssetsDir(): string {
  return join(memoryRootPath(), "session-artifacts");
}

function memoryBackupDir(): string {
  return join(memoryRootPath(), "backups");
}

function safeMemoryFileToken(value: string): string {
  return value.replace(/[^a-zA-Z0-9_.-]/g, "_").slice(0, 120) || "memory";
}

function memoryBackupPathFor(filePath: string, reason: string): string {
  const base = safeMemoryFileToken(basename(filePath));
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return join(memoryBackupDir(), `${base}.${safeMemoryFileToken(reason)}.${stamp}.bak`);
}

async function backupMemoryFile(filePath: string, reason: string): Promise<void> {
  if (!existsSync(filePath)) return;
  try {
    await mkdir(memoryBackupDir(), { recursive: true });
    await copyFile(filePath, memoryBackupPathFor(filePath, reason));
  } catch (error) {
    console.error(`Failed to backup memory file ${filePath}.`, error);
  }
}

async function atomicWriteJsonFile(filePath: string, value: unknown, reason: string): Promise<void> {
  const json = JSON.stringify(value, null, 2);
  JSON.parse(json);
  await mkdir(dirname(filePath), { recursive: true });
  await backupMemoryFile(filePath, reason);
  const tempPath = `${filePath}.${process.pid}.${Date.now().toString(36)}.tmp`;
  const handle = await open(tempPath, "w");
  try {
    await handle.writeFile(json, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  await rename(tempPath, filePath);
}

function legacyUserDataDirs(): string[] {
  const candidates = [
    process.env.INGEN_ELECTRON_LEGACY_USER_DATA_DIR,
    join(app.getPath("appData"), INGEN_LEGACY_USER_DATA_DIR_NAME),
    join(app.getPath("appData"), "Electron")
  ].filter((value): value is string => Boolean(value && value.trim()));
  const canonical = app.getPath("userData").toLowerCase();
  return Array.from(new Set(candidates.map((value) => resolve(value)))).filter((value) => value.toLowerCase() !== canonical);
}

function memoryLegacyRootDirs(): string[] {
  return legacyUserDataDirs().map((root) => join(root, INGEN_MEMORY_ROOT_DIR_NAME));
}

function memoryFileMtimeMs(filePath: string): number {
  try {
    return statSync(filePath).mtimeMs;
  } catch {
    return 0;
  }
}

function cloneJsonValue<T>(value: T): T {
  if (typeof value === "undefined") return value;
  return JSON.parse(JSON.stringify(value)) as T;
}

function blankBrainCanonicalMemorySnapshot(): BrainCanonicalMemorySnapshot {
  return {
    schema: "ingen.brain.renderer_memory_store.v1",
    updatedAt: new Date(0).toISOString(),
    learningEntries: [],
    customCodeActs: [],
    specializedBrains: [],
    proofHash: ""
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function normalizeBrainCanonicalMemorySnapshot(value: unknown): BrainCanonicalMemorySnapshot {
  const candidate = isRecord(value) ? value : {};
  const snapshot: BrainCanonicalMemorySnapshot = {
    schema: "ingen.brain.renderer_memory_store.v1",
    updatedAt: typeof candidate.updatedAt === "string" ? candidate.updatedAt : new Date(0).toISOString(),
    userMemory: isRecord(candidate.userMemory) ? cloneJsonValue(candidate.userMemory) : undefined,
    agentMemory: isRecord(candidate.agentMemory) ? cloneJsonValue(candidate.agentMemory) : undefined,
    userLocationMemory: isRecord(candidate.userLocationMemory) ? cloneJsonValue(candidate.userLocationMemory) : undefined,
    personalityMemory: isRecord(candidate.personalityMemory) ? cloneJsonValue(candidate.personalityMemory) : undefined,
    learningEntries: Array.isArray(candidate.learningEntries) ? cloneJsonValue(candidate.learningEntries) : [],
    customCodeActs: Array.isArray(candidate.customCodeActs) ? cloneJsonValue(candidate.customCodeActs) : [],
    specializedBrains: Array.isArray(candidate.specializedBrains) ? cloneJsonValue(candidate.specializedBrains) : [],
    proofHash: ""
  };
  snapshot.proofHash = stableSearchArchiveHash({
    userMemory: snapshot.userMemory,
    agentMemory: snapshot.agentMemory,
    userLocationMemory: snapshot.userLocationMemory,
    personalityMemory: snapshot.personalityMemory,
    learningEntries: snapshot.learningEntries,
    customCodeActs: snapshot.customCodeActs,
    specializedBrains: snapshot.specializedBrains
  });
  return snapshot;
}

function brainMemoryObjectField(value: unknown, field: string): string {
  return isRecord(value) && typeof value[field] === "string" ? (value[field] as string).trim() : "";
}

function selectBrainMemorySlot(current: unknown, incoming: unknown, durableField: string): unknown {
  const currentValue = brainMemoryObjectField(current, durableField);
  const incomingValue = brainMemoryObjectField(incoming, durableField);
  if (currentValue) return current;
  if (incomingValue) return incoming;
  return current ?? incoming;
}

function brainMemoryEntryKey(value: unknown): string {
  if (!isRecord(value)) return stableSearchArchiveHash(value);
  for (const key of ["brainName", "command", "id"]) {
    const entryKey = value[key];
    if (typeof entryKey === "string" && entryKey.trim()) {
      return `${key}:${entryKey.trim().toLowerCase()}`;
    }
  }
  return stableSearchArchiveHash(value);
}

function brainMemoryEntryUpdatedAt(value: unknown): string {
  return isRecord(value) && typeof value.updatedAt === "string" ? value.updatedAt : "";
}

function mergeBrainMemoryEntryArray(current: unknown[] = [], incoming: unknown[] = []): unknown[] {
  const merged = new Map<string, unknown>();
  for (const entry of [...current, ...incoming]) {
    const key = brainMemoryEntryKey(entry);
    const previous = merged.get(key);
    if (!previous || brainMemoryEntryUpdatedAt(entry) >= brainMemoryEntryUpdatedAt(previous)) {
      merged.set(key, cloneJsonValue(entry));
    }
  }
  return Array.from(merged.values())
    .sort((left, right) => brainMemoryEntryUpdatedAt(right).localeCompare(brainMemoryEntryUpdatedAt(left)));
}

function mergeBrainCanonicalMemorySnapshots(
  current: BrainCanonicalMemorySnapshot,
  incoming: BrainCanonicalMemorySnapshot
): BrainCanonicalMemorySnapshot {
  const merged: BrainCanonicalMemorySnapshot = {
    schema: "ingen.brain.renderer_memory_store.v1",
    updatedAt: new Date().toISOString(),
    userMemory: selectBrainMemorySlot(current.userMemory, incoming.userMemory, "preferredFirstName"),
    agentMemory: selectBrainMemorySlot(current.agentMemory, incoming.agentMemory, "preferredFirstName"),
    userLocationMemory: selectBrainMemorySlot(current.userLocationMemory, incoming.userLocationMemory, "homeLocation"),
    personalityMemory: selectBrainMemorySlot(current.personalityMemory, incoming.personalityMemory, "manifest"),
    learningEntries: mergeBrainMemoryEntryArray(current.learningEntries, incoming.learningEntries),
    customCodeActs: mergeBrainMemoryEntryArray(current.customCodeActs, incoming.customCodeActs),
    specializedBrains: mergeBrainMemoryEntryArray(current.specializedBrains, incoming.specializedBrains),
    proofHash: ""
  };
  merged.proofHash = stableSearchArchiveHash({
    userMemory: merged.userMemory,
    agentMemory: merged.agentMemory,
    userLocationMemory: merged.userLocationMemory,
    personalityMemory: merged.personalityMemory,
    learningEntries: merged.learningEntries,
    customCodeActs: merged.customCodeActs,
    specializedBrains: merged.specializedBrains
  });
  return merged;
}

async function readBrainCanonicalMemorySnapshotFromDisk(): Promise<BrainCanonicalMemorySnapshot> {
  try {
    const raw = await readFile(brainRendererMemoryStorePath(), "utf8");
    return normalizeBrainCanonicalMemorySnapshot(JSON.parse(raw));
  } catch (error) {
    if (existsSync(brainRendererMemoryStorePath())) {
      await backupMemoryFile(brainRendererMemoryStorePath(), "corrupt-renderer-brain-memory");
      console.error("Failed to read renderer Brain memory; preserved backup before recovery.", error);
    }
    return blankBrainCanonicalMemorySnapshot();
  }
}

async function mergeBrainCanonicalMemorySnapshotToDisk(
  incoming: unknown,
  reason = "renderer-brain-memory"
): Promise<BrainCanonicalMemorySnapshot> {
  const current = await readBrainCanonicalMemorySnapshotFromDisk();
  const merged = mergeBrainCanonicalMemorySnapshots(current, normalizeBrainCanonicalMemorySnapshot(incoming));
  await atomicWriteJsonFile(brainRendererMemoryStorePath(), merged, reason);
  return merged;
}

async function ensureBrainCanonicalMemorySnapshotOnDisk(): Promise<void> {
  if (existsSync(brainRendererMemoryStorePath())) return;
  await atomicWriteJsonFile(
    brainRendererMemoryStorePath(),
    blankBrainCanonicalMemorySnapshot(),
    "renderer-brain-memory-init"
  );
}

interface SessionArtifactLedgerEntry {
  schema: "ingen.session_artifact.entry.v1";
  id: string;
  source: "agent_action";
  sessionId: string;
  action: AgentActionRequest["action"];
  cwd: string;
  originalPath: string;
  canonicalPath: string;
  sha256: string;
  bytes: number;
  copied: boolean;
  resultProofHash: string;
  observedChanges: string[];
  createdAt: string;
}

interface SessionArtifactLedgerEnvelope {
  schema: "ingen.session_artifact.ledger.v1";
  updatedAt: string;
  sourceOfTruth: string;
  entries: SessionArtifactLedgerEntry[];
  proofHash: string;
}

function blankSessionArtifactLedger(): SessionArtifactLedgerEnvelope {
  return {
    schema: "ingen.session_artifact.ledger.v1",
    updatedAt: new Date(0).toISOString(),
    sourceOfTruth: memoryRootPath(),
    entries: [],
    proofHash: stableSearchArchiveHash([])
  };
}

function protectedMapRootPath(): string {
  return resolve("C:\\Users\\quent\\Documents\\EVE\\MAP").toLowerCase();
}

function pathInsideProtectedMap(filePath: string): boolean {
  const protectedRoot = protectedMapRootPath();
  const resolvedPath = resolve(filePath).toLowerCase();
  return resolvedPath === protectedRoot || resolvedPath.startsWith(`${protectedRoot}\\`) || resolvedPath.startsWith(`${protectedRoot}/`);
}

function pathInsideSessionArtifactAssets(filePath: string): boolean {
  const artifactRoot = resolve(sessionArtifactAssetsDir()).toLowerCase();
  const resolvedPath = resolve(filePath).toLowerCase();
  return resolvedPath === artifactRoot || resolvedPath.startsWith(`${artifactRoot}\\`) || resolvedPath.startsWith(`${artifactRoot}/`);
}

async function sha256File(filePath: string): Promise<{ sha256: string; bytes: number }> {
  const fileStat = await stat(filePath);
  const hash = createHash("sha256");
  await new Promise<void>((resolvePromise, rejectPromise) => {
    const stream = createReadStream(filePath);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", () => resolvePromise());
  });
  return {
    sha256: hash.digest("hex"),
    bytes: fileStat.size
  };
}

function canonicalSessionArtifactPath(sourcePath: string, sha256: string): string {
  const safeName = safeMemoryFileToken(basename(sourcePath || "artifact"));
  return join(sessionArtifactAssetsDir(), sha256.slice(0, 2), `${sha256}-${safeName}`);
}

function agentActionResultArtifactPathCandidates(result: AgentActionResult): string[] {
  const candidates = new Set<string>();
  const collect = (value: unknown, key = ""): void => {
    if (typeof value === "string") {
      if ((key === "path" || key === "toPath") && value.trim()) {
        candidates.add(value.trim());
      }
      return;
    }
    if (Array.isArray(value)) {
      for (const item of value) {
        if (typeof item === "string" && key === "artifacts" && item.trim()) {
          candidates.add(item.trim());
        } else {
          collect(item, key);
        }
      }
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const [entryKey, entryValue] of Object.entries(value as Record<string, unknown>)) {
      if (entryKey === "cwd") continue;
      collect(entryValue, entryKey);
    }
  };
  collect({
    path: result.path,
    toPath: result.toPath,
    artifacts: result.artifacts,
    appshot: result.appshot,
    browserScreenshot: result.browserScreenshot,
    download: result.download,
    documentMedia: result.documentMedia,
    audit: result.audit
  });
  return Array.from(candidates);
}

function resolveAgentActionArtifactPath(result: AgentActionResult, candidatePath: string): string {
  return resolve(candidatePath.match(/^[a-zA-Z]:[\\/]/) ? candidatePath : join(result.cwd || activeWorkspaceDir, candidatePath));
}

async function readSessionArtifactLedger(): Promise<SessionArtifactLedgerEnvelope> {
  try {
    const raw = await readFile(sessionArtifactLedgerStorePath(), "utf8");
    const parsed = JSON.parse(raw) as Partial<SessionArtifactLedgerEnvelope>;
    const entries = Array.isArray(parsed.entries)
      ? parsed.entries.filter((entry): entry is SessionArtifactLedgerEntry =>
        isRecord(entry) &&
        entry.schema === "ingen.session_artifact.entry.v1" &&
        typeof entry.id === "string" &&
        typeof entry.originalPath === "string" &&
        typeof entry.canonicalPath === "string" &&
        typeof entry.sha256 === "string" &&
        typeof entry.bytes === "number"
      )
      : [];
    return {
      schema: "ingen.session_artifact.ledger.v1",
      updatedAt: typeof parsed.updatedAt === "string" ? parsed.updatedAt : new Date(0).toISOString(),
      sourceOfTruth: memoryRootPath(),
      entries,
      proofHash: typeof parsed.proofHash === "string" ? parsed.proofHash : stableSearchArchiveHash(entries)
    };
  } catch (error) {
    if (existsSync(sessionArtifactLedgerStorePath())) {
      await backupMemoryFile(sessionArtifactLedgerStorePath(), "corrupt-session-artifact-ledger");
      console.error("Failed to read session artifact ledger; preserved backup before recovery.", error);
    }
    return blankSessionArtifactLedger();
  }
}

async function writeSessionArtifactLedger(entries: SessionArtifactLedgerEntry[], reason: string): Promise<void> {
  const sorted = entries.sort((left, right) => right.createdAt.localeCompare(left.createdAt));
  const envelope: SessionArtifactLedgerEnvelope = {
    schema: "ingen.session_artifact.ledger.v1",
    updatedAt: new Date().toISOString(),
    sourceOfTruth: memoryRootPath(),
    entries: sorted,
    proofHash: stableSearchArchiveHash(sorted.map((entry) => `${entry.sha256}:${entry.originalPath}:${entry.resultProofHash}`))
  };
  await atomicWriteJsonFile(sessionArtifactLedgerStorePath(), envelope, reason);
}

async function ensureSessionArtifactLedgerOnDisk(): Promise<void> {
  await mkdir(sessionArtifactAssetsDir(), { recursive: true });
  if (existsSync(sessionArtifactLedgerStorePath())) return;
  await atomicWriteJsonFile(sessionArtifactLedgerStorePath(), blankSessionArtifactLedger(), "session-artifact-ledger-init");
}

async function persistAgentActionSessionArtifacts(
  request: AgentActionRequest,
  result: AgentActionResult
): Promise<SessionArtifactLedgerEntry[]> {
  if (!result.accepted) return [];
  const paths = agentActionResultArtifactPathCandidates(result);
  if (paths.length === 0) return [];
  await ensureSessionArtifactLedgerOnDisk();
  const existingLedger = await readSessionArtifactLedger();
  const entriesById = new Map(existingLedger.entries.map((entry) => [entry.id, entry]));
  const createdAt = new Date().toISOString();
  const added: SessionArtifactLedgerEntry[] = [];
  for (const candidate of paths) {
    const originalPath = resolveAgentActionArtifactPath(result, candidate);
    if (pathInsideProtectedMap(originalPath) || pathInsideSessionArtifactAssets(originalPath)) {
      continue;
    }
    let fileStat;
    try {
      fileStat = await stat(originalPath);
    } catch {
      continue;
    }
    if (!fileStat.isFile()) continue;
    const { sha256, bytes } = await sha256File(originalPath);
    const canonicalPath = pathInsideMemoryRoot(originalPath)
      ? originalPath
      : canonicalSessionArtifactPath(originalPath, sha256);
    if (!pathInsideMemoryRoot(originalPath)) {
      await mkdir(dirname(canonicalPath), { recursive: true });
      if (!existsSync(canonicalPath)) {
        await copyFile(originalPath, canonicalPath);
      }
    }
    const entry: SessionArtifactLedgerEntry = {
      schema: "ingen.session_artifact.entry.v1",
      id: stableSearchArchiveHash({ sha256, originalPath, resultProofHash: result.proofHash }).slice(0, 32),
      source: "agent_action",
      sessionId: panelsChatBottomState.activeSessionId || sidebarState.recentSessionId || "unknown-session",
      action: request.action,
      cwd: result.cwd || activeWorkspaceDir,
      originalPath,
      canonicalPath,
      sha256,
      bytes,
      copied: canonicalPath !== originalPath,
      resultProofHash: result.proofHash,
      observedChanges: result.observedChanges ?? [],
      createdAt
    };
    entriesById.set(entry.id, entry);
    added.push(entry);
  }
  if (added.length > 0) {
    await writeSessionArtifactLedger(Array.from(entriesById.values()), "session-artifacts");
  }
  return added;
}

async function executeTrackedAgentAction(request: AgentActionRequest): Promise<AgentActionResult> {
  const result = await executeAgentActionRequest(agentActionHostConfig(), request);
  try {
    await persistAgentActionSessionArtifacts(request, result);
  } catch (error) {
    console.error("Failed to persist agent action session artifacts.", error);
  }
  return result;
}

function normalizeBrainIdentityName(value: unknown): string {
  if (typeof value !== "string") return "";
  const compact = value.replace(/\s+/g, " ").trim();
  return Array.from(compact).slice(0, 48).join("");
}

function normalizeBrainHomeLocation(value: unknown): string {
  if (typeof value !== "string") return "";
  const compact = value.replace(/\s+/g, " ").trim();
  return Array.from(compact).slice(0, 96).join("");
}

function normalizeBrainDurableMemoryManifest(value: unknown): string {
  if (typeof value !== "string") return "";
  const compact = value.replace(/\r\n?/g, "\n").trim();
  if (!compact || !compact.startsWith("BRAIN_DURABLE_MEMORY_MANIFEST v1")) {
    return "";
  }
  return trimUtf8Bytes(compact, 8_000);
}

function defaultBrainPersonalityManifest(): string {
  return [
    "BRAIN_PERSONALITY_MANIFEST v1",
    "voice=Sound like a present, capable desktop agent, not a status log.",
    "loop_stream=In each working paragraph, make clear what you just understood, why it changes the next action, and what proof you are waiting for.",
    "tone=Natural, clear and lightly warm when useful; never theatrical, never pretending to be human.",
    "pivots=When a path fails, explain the useful signal from the failure and the next route you chose.",
    "success=When an action works, name the concrete proof: path, accepted result, count, exit code, artifact or verification.",
    "rhythm=Vary paragraph openings; avoid starting every step with 'I will' or mechanical labels.",
    "boundaries=Stay honest: never promise an action without an event or tool, and never claim success without runtime proof."
  ].join("\n");
}

function normalizeBrainPersonalityManifest(value: unknown): string {
  if (typeof value !== "string") return defaultBrainPersonalityManifest();
  const compact = trimUtf8Bytes(value.replace(/\r\n?/g, "\n").trim(), 5_000);
  if (!compact) return defaultBrainPersonalityManifest();
  if (!compact.startsWith("BRAIN_PERSONALITY_MANIFEST v1")) {
    return ["BRAIN_PERSONALITY_MANIFEST v1", compact].join("\n");
  }
  return compact;
}

interface BrainIdentityFileSnapshot {
  context: BrainIdentityContext;
  mtimeMs: number;
}

async function readBrainIdentityContextFromFile(filePath: string): Promise<BrainIdentityFileSnapshot | null> {
  try {
    const raw = await readFile(filePath, "utf8");
    const parsed = JSON.parse(raw) as Partial<BrainIdentityContext>;
    const fileStat = await stat(filePath);
    return {
      mtimeMs: fileStat.mtimeMs,
      context: {
        userFirstName: normalizeBrainIdentityName(parsed.userFirstName),
        agentFirstName: normalizeBrainIdentityName(parsed.agentFirstName),
        userHomeLocation: normalizeBrainHomeLocation(parsed.userHomeLocation),
        personalityManifest: normalizeBrainPersonalityManifest(parsed.personalityManifest),
        durableMemoryManifest: normalizeBrainDurableMemoryManifest(parsed.durableMemoryManifest)
      }
    };
  } catch (error) {
    if (existsSync(filePath)) {
      await backupMemoryFile(filePath, "corrupt-brain-identity");
      console.error(`Failed to read Brain identity file ${filePath}; preserved a backup before recovery.`, error);
    }
    return null;
  }
}

function mergeBrainIdentitySnapshots(snapshots: BrainIdentityFileSnapshot[]): BrainIdentityContext {
  const merged: BrainIdentityContext = {
    userFirstName: "",
    agentFirstName: "",
    userHomeLocation: "",
    personalityManifest: defaultBrainPersonalityManifest(),
    durableMemoryManifest: ""
  };
  for (const snapshot of snapshots.sort((left, right) => left.mtimeMs - right.mtimeMs)) {
    if (snapshot.context.userFirstName) merged.userFirstName = snapshot.context.userFirstName;
    if (snapshot.context.agentFirstName) merged.agentFirstName = snapshot.context.agentFirstName;
    if (snapshot.context.userHomeLocation) merged.userHomeLocation = snapshot.context.userHomeLocation;
    if (snapshot.context.personalityManifest) merged.personalityManifest = snapshot.context.personalityManifest;
    if (snapshot.context.durableMemoryManifest) merged.durableMemoryManifest = snapshot.context.durableMemoryManifest;
  }
  return merged;
}

function brainIdentityContextsEqual(left: BrainIdentityContext, right: BrainIdentityContext): boolean {
  return (
    left.userFirstName === right.userFirstName &&
    left.agentFirstName === right.agentFirstName &&
    left.userHomeLocation === right.userHomeLocation &&
    left.personalityManifest === right.personalityManifest &&
    left.durableMemoryManifest === right.durableMemoryManifest
  );
}

async function restoreBrainIdentityContextFromDisk(): Promise<void> {
  try {
    const snapshot = await readBrainIdentityContextFromFile(brainIdentityStorePath());
    if (!snapshot) {
      brainIdentityContext.personalityManifest = defaultBrainPersonalityManifest();
      return;
    }
    brainIdentityContext.userFirstName = snapshot.context.userFirstName;
    brainIdentityContext.agentFirstName = snapshot.context.agentFirstName;
    brainIdentityContext.userHomeLocation = snapshot.context.userHomeLocation;
    brainIdentityContext.personalityManifest = snapshot.context.personalityManifest;
    brainIdentityContext.durableMemoryManifest = snapshot.context.durableMemoryManifest;
  } catch {
    // No persisted identity yet; keep first-install fields blank.
    brainIdentityContext.personalityManifest = defaultBrainPersonalityManifest();
  }
}

async function persistBrainIdentityContext(): Promise<void> {
  try {
    await atomicWriteJsonFile(brainIdentityStorePath(), brainIdentityContext, "brain-identity");
  } catch (error) {
    console.error("Failed to persist Brain identity memory.", error);
  }
}

function brainIdentityMemoryManifest(): string {
  const user = brainIdentityContext.userFirstName;
  const assistant = brainIdentityContext.agentFirstName;
  const homeLocation = brainIdentityContext.userHomeLocation;
  if (!user && !assistant && !homeLocation) return "";
  return `BRAIN_IDENTITY_MEMORY v1 user_first_name=${JSON.stringify(user)} assistant_first_name=${JSON.stringify(assistant)} user_home_location=${JSON.stringify(homeLocation)} rule=If asked your name or first name, answer assistant_first_name. Use user_first_name for the user. Use user_home_location as user-confirmed living place only when the user asks about their saved/home context. Never invent missing identity or location fields. Never treat user_home_location as live device geolocation or as the default /maps_ target.`;
}

function brainDurableMemoryContextManifest(): string {
  return brainIdentityContext.durableMemoryManifest;
}

function brainPersonalityContextManifest(): string {
  return normalizeBrainPersonalityManifest(brainIdentityContext.personalityManifest);
}

type PhotonFeature = {
  geometry?: {
    coordinates?: unknown;
  };
  properties?: {
    name?: unknown;
    city?: unknown;
    country?: unknown;
    state?: unknown;
  };
};

type GooglePlaceAutocompleteSuggestion = {
  placePrediction?: {
    text?: {
      text?: unknown;
    };
  };
};

type MapsGeocodeResult = {
  label: string;
  latitude: number;
  longitude: number;
  source: "google_geocoding" | "photon";
};

type GoogleGeocodeResponse = {
  status?: unknown;
  results?: Array<{
    formatted_address?: unknown;
    geometry?: {
      location?: {
        lat?: unknown;
        lng?: unknown;
      };
    };
  }>;
};

function citySuggestionError(query: string, message: string): CitySuggestionResult {
  const proofHash = hashJson({ citySuggestions: "photon", query, accepted: false, message });
  return {
    schema: "ingen.brain.memory.city_suggestions.v1",
    query,
    suggestions: [],
    proofHash,
    error: {
      code: "bad_payload",
      message,
      proofHash
    }
  };
}

function googlePlacesApiKey(): string {
  return (process.env.GOOGLE_PLACES_API_KEY ?? process.env.GOOGLE_MAPS_API_KEY ?? "").trim();
}

function googlePlaceSuggestionToCitySuggestion(suggestion: GooglePlaceAutocompleteSuggestion): CitySuggestion | null {
  const label = typeof suggestion.placePrediction?.text?.text === "string"
    ? suggestion.placePrediction.text.text.replace(/\s+/g, " ").trim()
    : "";
  if (!label) {
    return null;
  }
  const [city = label, ...rest] = label.split(",").map((part) => part.trim()).filter(Boolean);
  return {
    label,
    city,
    country: rest.at(-1) ?? "",
    source: "google_places"
  };
}

async function searchGoogleCitySuggestions(query: string, apiKey: string): Promise<CitySuggestionResult | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3500);
  try {
    const response = await net.fetch("https://places.googleapis.com/v1/places:autocomplete", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Goog-Api-Key": apiKey,
        "X-Goog-FieldMask": "suggestions.placePrediction.text.text"
      },
      body: JSON.stringify({
        input: query,
        includedPrimaryTypes: ["(cities)"],
        languageCode: "en"
      }),
      signal: controller.signal
    });
    if (!response.ok) {
      return null;
    }
    const payload = await response.json() as { suggestions?: GooglePlaceAutocompleteSuggestion[] };
    const seen = new Set<string>();
    const suggestions = (payload.suggestions ?? [])
      .map(googlePlaceSuggestionToCitySuggestion)
      .filter((suggestion): suggestion is CitySuggestion => {
        if (!suggestion || seen.has(suggestion.label)) return false;
        seen.add(suggestion.label);
        return true;
      })
      .slice(0, 6);
    return {
      schema: "ingen.brain.memory.city_suggestions.v1",
      query,
      suggestions,
      proofHash: hashJson({ citySuggestions: "google_places", query, suggestions })
    };
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function searchGoogleGeoEntitySuggestions(query: string, apiKey: string): Promise<CitySuggestionResult | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3500);
  try {
    const response = await net.fetch("https://places.googleapis.com/v1/places:autocomplete", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Goog-Api-Key": apiKey,
        "X-Goog-FieldMask": "suggestions.placePrediction.text.text"
      },
      body: JSON.stringify({
        input: query,
        languageCode: "en"
      }),
      signal: controller.signal
    });
    if (!response.ok) {
      return null;
    }
    const payload = await response.json() as { suggestions?: GooglePlaceAutocompleteSuggestion[] };
    const seen = new Set<string>();
    const suggestions = (payload.suggestions ?? [])
      .map(googlePlaceSuggestionToCitySuggestion)
      .filter((suggestion): suggestion is CitySuggestion => {
        if (!suggestion || seen.has(suggestion.label)) return false;
        seen.add(suggestion.label);
        return true;
      })
      .slice(0, 6);
    return {
      schema: "ingen.brain.memory.city_suggestions.v1",
      query,
      suggestions,
      proofHash: hashJson({ geoEntitySuggestions: "google_places", query, suggestions })
    };
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

function photonFeatureToCitySuggestion(feature: PhotonFeature): CitySuggestion | null {
  const properties = feature.properties ?? {};
  const coordinates = Array.isArray(feature.geometry?.coordinates) ? feature.geometry?.coordinates : [];
  const longitude = typeof coordinates[0] === "number" ? coordinates[0] : Number.NaN;
  const latitude = typeof coordinates[1] === "number" ? coordinates[1] : Number.NaN;
  const city = typeof properties.city === "string" && properties.city.trim()
    ? properties.city.trim()
    : typeof properties.name === "string" && properties.name.trim()
      ? properties.name.trim()
      : "";
  const country = typeof properties.country === "string" ? properties.country.trim() : "";
  if (!city || !country || !Number.isFinite(latitude) || !Number.isFinite(longitude)) {
    return null;
  }
  const label = `${city}, ${country}`;
  return {
    label,
    city,
    country,
    latitude,
    longitude,
    source: "photon"
  };
}

async function searchCitySuggestions(queryValue: unknown): Promise<CitySuggestionResult> {
  const query = normalizeBrainHomeLocation(queryValue);
  if (query.length < 2) {
    return {
      schema: "ingen.brain.memory.city_suggestions.v1",
      query,
      suggestions: [],
      proofHash: hashJson({ citySuggestions: "photon", query, skipped: "short_query" })
    };
  }
  const apiKey = googlePlacesApiKey();
  if (apiKey) {
    const googleResult = await searchGoogleCitySuggestions(query, apiKey);
    if (googleResult) {
      return googleResult;
    }
  }
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3500);
  try {
    const url = new URL("https://photon.komoot.io/api/");
    url.searchParams.set("q", query);
    url.searchParams.set("limit", "6");
    url.searchParams.set("lang", "fr");
    url.searchParams.append("layer", "city");
    url.searchParams.append("layer", "locality");
    const response = await net.fetch(url.toString(), { signal: controller.signal });
    if (!response.ok) {
      return citySuggestionError(query, `Photon city lookup failed with HTTP ${response.status}.`);
    }
    const payload = await response.json() as { features?: PhotonFeature[] };
    const seen = new Set<string>();
    const suggestions = (payload.features ?? [])
      .map(photonFeatureToCitySuggestion)
      .filter((suggestion): suggestion is CitySuggestion => {
        if (!suggestion) return false;
        const coordinateKey = Number.isFinite(suggestion.latitude) && Number.isFinite(suggestion.longitude)
          ? `${suggestion.latitude?.toFixed(4)}|${suggestion.longitude?.toFixed(4)}`
          : "no-coordinates";
        const key = `${suggestion.label}|${coordinateKey}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .slice(0, 6);
    return {
      schema: "ingen.brain.memory.city_suggestions.v1",
      query,
      suggestions,
      proofHash: hashJson({ citySuggestions: "photon", query, suggestions })
    };
  } catch (error) {
    const message = error instanceof Error && error.name === "AbortError"
      ? "Photon city lookup timed out."
      : "Photon city lookup is unavailable.";
    return citySuggestionError(query, message);
  } finally {
    clearTimeout(timeout);
  }
}

function readValidMapsCoordinate(value: unknown, min: number, max: number): number | undefined {
  const parsed = typeof value === "number" ? value : Number.NaN;
  return Number.isFinite(parsed) && parsed >= min && parsed <= max ? parsed : undefined;
}

function mapsCodeActNeedsGeocode(request: MapsCodeActRequest): boolean {
  return (
    readValidMapsCoordinate(request.latitude, -90, 90) === undefined ||
    readValidMapsCoordinate(request.longitude, -180, 180) === undefined
  );
}

function mapsGeocodeQueryForRequest(request: MapsCodeActRequest): string {
  if (!mapsCodeActNeedsGeocode(request)) {
    return "";
  }
  const target = normalizeBrainHomeLocation(request.target);
  if (target && target !== MAPS_DEFAULT_TARGET) {
    return target;
  }
  return "";
}

function photonSuggestionToMapsGeocode(suggestion: CitySuggestion): MapsGeocodeResult | null {
  const latitude = readValidMapsCoordinate(suggestion.latitude, -90, 90);
  const longitude = readValidMapsCoordinate(suggestion.longitude, -180, 180);
  if (latitude === undefined || longitude === undefined) {
    return null;
  }
  return {
    label: suggestion.label || [suggestion.city, suggestion.country].filter(Boolean).join(", "),
    latitude,
    longitude,
    source: "photon"
  };
}

async function geocodeGoogleMapsLocation(query: string, apiKey: string): Promise<MapsGeocodeResult | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3500);
  try {
    const url = new URL("https://maps.googleapis.com/maps/api/geocode/json");
    url.searchParams.set("address", query);
    url.searchParams.set("language", "en");
    url.searchParams.set("key", apiKey);
    const response = await net.fetch(url.toString(), { signal: controller.signal });
    if (!response.ok) {
      return null;
    }
    const payload = await response.json() as GoogleGeocodeResponse;
    if (payload.status !== "OK") {
      return null;
    }
    const first = payload.results?.[0];
    const latitude = readValidMapsCoordinate(first?.geometry?.location?.lat, -90, 90);
    const longitude = readValidMapsCoordinate(first?.geometry?.location?.lng, -180, 180);
    if (latitude === undefined || longitude === undefined) {
      return null;
    }
    const label = typeof first?.formatted_address === "string" && first.formatted_address.trim()
      ? first.formatted_address.replace(/\s+/g, " ").trim()
      : query;
    return {
      label,
      latitude,
      longitude,
      source: "google_geocoding"
    };
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function geocodePhotonMapsLocation(query: string): Promise<MapsGeocodeResult | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3500);
  try {
    const url = new URL("https://photon.komoot.io/api/");
    url.searchParams.set("q", query);
    url.searchParams.set("limit", "1");
    url.searchParams.set("lang", "fr");
    url.searchParams.append("layer", "city");
    url.searchParams.append("layer", "locality");
    const response = await net.fetch(url.toString(), { signal: controller.signal });
    if (!response.ok) {
      return null;
    }
    const payload = await response.json() as { features?: PhotonFeature[] };
    return (payload.features ?? [])
      .map(photonFeatureToCitySuggestion)
      .map((item) => item ? photonSuggestionToMapsGeocode(item) : null)
      .find((item): item is MapsGeocodeResult => Boolean(item)) ?? null;
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function geocodeMapsLocation(query: string): Promise<MapsGeocodeResult | null> {
  const normalized = normalizeBrainHomeLocation(query);
  if (normalized.length < 2) {
    return null;
  }
  const apiKey = googlePlacesApiKey();
  if (apiKey) {
    const googleResult = await geocodeGoogleMapsLocation(normalized, apiKey);
    if (googleResult) {
      return googleResult;
    }
  }
  return geocodePhotonMapsLocation(normalized);
}

function readGeoEntityCoordinatePair(value: string): { latitude: number; longitude: number } | null {
  const match = /^\s*(-?\d{1,2}(?:[\.,]\d+)?)\s*[,; ]\s*(-?\d{1,3}(?:[\.,]\d+)?)\s*$/.exec(value);
  if (!match) {
    return null;
  }
  const latitude = Number((match[1] ?? "").replace(",", "."));
  const longitude = Number((match[2] ?? "").replace(",", "."));
  if (!Number.isFinite(latitude) || !Number.isFinite(longitude) || latitude < -90 || latitude > 90 || longitude < -180 || longitude > 180) {
    return null;
  }
  return { latitude, longitude };
}

function normalizeAssistantGeoEntityQuery(queryValue: unknown): string {
  return normalizeBrainHomeLocation(queryValue)
    .replace(/^@\{\s*/, "")
    .replace(/\s*\}$/g, "")
    .replace(/[.!?,;:]+$/g, "")
    .trim();
}

async function resolveAssistantGeoEntityMapsRequest(queryValue: unknown): Promise<MapsCodeActRequest | null> {
  const query = normalizeAssistantGeoEntityQuery(queryValue);
  if (query.length < 2) {
    return null;
  }
  const coordinates = readGeoEntityCoordinatePair(query);
  if (coordinates) {
    return createMapsCodeActRequest({
      command: BRAIN_MAPS_COMMAND,
      target: query,
      query,
      keywords: ["assistant_geo_entity", "coordinates"],
      latitude: coordinates.latitude,
      longitude: coordinates.longitude,
      source: "explicit_codeact"
    });
  }
  const apiKey = googlePlacesApiKey();
  const placesResult = apiKey ? await searchGoogleGeoEntitySuggestions(query, apiKey) : null;
  const placeQuery = placesResult?.suggestions[0]?.label || query;
  const geocode = await geocodeMapsLocation(placeQuery);
  if (!geocode) {
    return null;
  }
  return createMapsCodeActRequest({
    command: BRAIN_MAPS_COMMAND,
    target: geocode.label,
    query: placeQuery,
    keywords: ["assistant_geo_entity", geocode.source],
    latitude: geocode.latitude,
    longitude: geocode.longitude,
    source: "explicit_codeact"
  });
}

async function resolveMapsCodeActRequest(request: MapsCodeActRequest): Promise<MapsCodeActRequest> {
  const query = mapsGeocodeQueryForRequest(request);
  if (!query) {
    return request;
  }
  const geocode = await geocodeMapsLocation(query);
  if (!geocode) {
    return request;
  }
  return createMapsCodeActRequest({
    command: request.command,
    target: query,
    query,
    keywords: [...request.keywords, "explicit_maps_target", geocode.source],
    latitude: geocode.latitude,
    longitude: geocode.longitude,
    source: request.source
  });
}

function userTextHasTravelOrStayIntent(value: string): boolean {
  return /\b(airbnb|voyage|voyager|vacances|partir|depart|destination|tourisme|touristique|visiter|visite|sejour|weekend|week-end|logement|loger|hebergement|hotel|auberge|reserver|reservation|booking|location|louer|appartement|maison|villa|chambre|nuits?|voyageurs?)\b/i.test(normalizedRoutingIntentText(value));
}

function normalizedRoutingIntentText(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

function userTextHasDirectMapsIntent(value: string): boolean {
  const text = normalizedRoutingIntentText(value);
  return /\b(carte|map|maps|google earth|localise|localiser|situe|situer|ou est|itineraire|route|trajet|coordonnees?|gps|latitude|longitude|geographie|geographique|contexte geographique|meteo|temperature|climat)\b/i.test(text);
}

function userTextHasKnowledgeOnlyPlaceIntent(value: string): boolean {
  const text = normalizedRoutingIntentText(value);
  return !userTextHasDirectMapsIntent(value) &&
    !userTextHasTravelOrStayIntent(value) &&
    /\b(litterature|histoire|historique|culture|mythologie|philosophie|poesie|theatre|auteur|auteurs|oeuvre|oeuvres|classe|classer|tableau|categorie|categories|explique|parle|raconte|resume|synthese|notion|notions|vocabulaire|mots?)\b/i.test(text);
}

function shouldSuppressMapsForUserText(value: string): boolean {
  return Boolean(value.trim()) && userTextHasKnowledgeOnlyPlaceIntent(value);
}

function cleanInferredMapsTarget(value: string): string {
  return normalizeBrainHomeLocation(value
    .replace(/[?.!,;:]+$/g, "")
    .replace(/\b(?:s'il te plait|s'il vous plait|stp|svp|merci)\b.*$/i, "")
    .replace(/\b(?:en general|en gros|rapidement)\b.*$/i, "")
    .trim());
}

function inferGeographicTargetFromUserText(value: string): string {
  const text = value.replace(/\s+/g, " ").trim();
  if (!text) {
    return "";
  }
  const patterns = [
    /\b(?:partir|aller)\s+(?:en\s+)?(?:vacances?|voyage|sejour|séjour|weekend|week-end)\s+(?:a|à|au|aux|en|dans|de|d'|sur|pour)\s+(.{2,96})$/i,
    /\b(?:voyage|vacances|sejour|séjour|weekend|week-end|tourisme)\s+(?:a|à|au|aux|en|dans|de|d'|sur|pour)\s+(.{2,96})$/i,
    /\b(?:partir|voyager|visiter|visite)\s+(?:a|à|au|aux|en|dans|de|d'|sur|pour)\s+(.{2,96})$/i,
    /\b(?:meteo|météo|temperature|température|climat)\s+(?:a|à|de|d'|sur|pour)\s+(.{2,96})$/i,
    /\b(?:ou|où)\s+est\s+(.{2,96})$/i,
    /\b(?:carte|map|maps|google earth|localise|situe|itineraire|route|trajet|coordonnees|gps)\s+(?:de|d'|a|à|sur|pour|vers)?\s*(.{2,96})$/i
  ];
  for (const pattern of patterns) {
    const match = text.match(pattern);
    const target = cleanInferredMapsTarget(match?.[1] ?? "");
    if (target.length >= 2) {
      return target;
    }
  }
  return "";
}

function inferMapsTargetFromUserText(value: string): string {
  const text = value.replace(/\s+/g, " ").trim();
  if (!text || userTextHasTravelOrStayIntent(text) || shouldSuppressMapsForUserText(text) || !userTextHasDirectMapsIntent(text)) {
    return "";
  }
  return inferGeographicTargetFromUserText(text);
}

function stripMapsCodeActLines(text: string): string {
  return text
    .split(/\r?\n/)
    .filter((line) => {
      const trimmed = line.trim();
      return trimmed !== BRAIN_MAPS_COMMAND && !trimmed.startsWith(`${BRAIN_MAPS_COMMAND} `);
    })
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function stripCompetingGeographicCodeActLines(text: string): string {
  const competingCommands = new Set<string>([BRAIN_AIRBNB_COMMAND, BRAIN_GOOGLEWEB_COMMAND, BRAIN_SCIENCE_COMMAND]);
  return text
    .split(/\r?\n/)
    .filter((line) => {
      const trimmed = line.trim();
      for (const command of competingCommands) {
        if (trimmed === command || trimmed.startsWith(`${command} `)) {
          return false;
        }
      }
      return true;
    })
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function mapsCodeActLineFromResolvedRequest(request: MapsCodeActRequest): string {
  const target = request.target.replace(/"/g, "'");
  if (typeof request.latitude !== "number" || typeof request.longitude !== "number") {
    return `${BRAIN_MAPS_COMMAND} target="${target}"`;
  }
  return `${BRAIN_MAPS_COMMAND} target="${target}" latitude="${request.latitude}" longitude="${request.longitude}"`;
}

function mapsCodeActLineFromTarget(target: string): string {
  return `${BRAIN_MAPS_COMMAND} target="${target.replace(/"/g, "'")}"`;
}

function stripCompetingTravelCodeActLines(text: string): string {
  const competingCommands = new Set<string>([BRAIN_MAPS_COMMAND, BRAIN_AIRBNB_COMMAND, BRAIN_GOOGLEWEB_COMMAND, BRAIN_SCIENCE_COMMAND]);
  return text
    .split(/\r?\n/)
    .filter((line) => {
      const trimmed = line.trim();
      for (const command of competingCommands) {
        if (trimmed === command || trimmed.startsWith(`${command} `)) {
          return false;
        }
      }
      return true;
    })
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function airbnbCodeActLineFromTarget(target: string): string {
  const cleanTarget = target.replace(/"/g, "'");
  return `${BRAIN_AIRBNB_COMMAND} intent="search" say="Opening Airbnb for ${cleanTarget}." query="${cleanTarget}" keywords="host_geographic_travel_fallback, travel, vacation"`;
}

function applyGeographicTravelAirbnbFallback(
  message: TranscriptMessage,
  userText: string,
  moduleId: string
): TranscriptMessage {
  if (message.role !== "assistant" || moduleId === "gmail") {
    return message;
  }
  if (
    (message.text.includes("MAPS_RESULT") || message.text.includes(BRAIN_MAPS_COMMAND)) &&
    (message.text.includes("AIRBNB_RESULT") || message.text.includes(BRAIN_AIRBNB_COMMAND))
  ) {
    return message;
  }
  if (!userTextHasTravelOrStayIntent(userText)) {
    return message;
  }
  const target = inferGeographicTargetFromUserText(userText);
  if (!target) {
    return message;
  }
  const visibleText = stripCompetingTravelCodeActLines(message.text) || assistantCodeActVisibleText(message.text);
  const mapsLine = mapsCodeActLineFromTarget(target);
  const airbnbLine = airbnbCodeActLineFromTarget(target);
  return {
    ...message,
    text: `${visibleText.trim()}\n\n${mapsLine}\n${airbnbLine}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      hostGeographicTravelFallback: {
        target
      }
    })
  };
}

async function applyGeographicMapsFallback(
  message: TranscriptMessage,
  userText: string,
  moduleId: string,
  parallelSessionIndex: number
): Promise<TranscriptMessage> {
  if (message.role !== "assistant" || moduleId === "gmail") {
    return message;
  }
  if (message.text.includes("MAPS_RESULT") || message.text.includes(BRAIN_MAPS_COMMAND)) {
    return message;
  }
  const target = inferMapsTargetFromUserText(userText);
  if (!target) {
    return message;
  }
  const candidate = createMapsCodeActRequest({
    command: BRAIN_MAPS_COMMAND,
    target,
    query: target,
    keywords: ["host_geographic_fallback"],
    source: "explicit_codeact"
  });
  const resolved = await resolveMapsCodeActRequest(candidate);
  const visibleText = stripCompetingGeographicCodeActLines(message.text) || assistantCodeActVisibleText(message.text);
  return {
    ...message,
    text: `${visibleText.trim()}\n\n${mapsCodeActLineFromResolvedRequest(resolved)}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      hostMapsFallback: {
        target,
        parallelSessionIndex,
        proofHash: resolved.proofHash
      }
    })
  };
}

function updateBrainIdentityContext(command: PanelsChatBottomCommand): void {
  const nextUserFirstName = normalizeBrainIdentityName(command.userFirstName);
  const nextAgentFirstName = normalizeBrainIdentityName(command.agentFirstName);
  const nextUserHomeLocation = normalizeBrainHomeLocation(command.userHomeLocation);
  const nextPersonalityManifest = normalizeBrainPersonalityManifest(command.personalityManifest);
  const nextDurableMemoryManifest = command.kind === "update_brain_identity"
    ? normalizeBrainDurableMemoryManifest(command.value)
    : brainIdentityContext.durableMemoryManifest;
  const unchanged =
    brainIdentityContext.userFirstName === nextUserFirstName &&
    brainIdentityContext.agentFirstName === nextAgentFirstName &&
    brainIdentityContext.userHomeLocation === nextUserHomeLocation &&
    brainIdentityContext.personalityManifest === nextPersonalityManifest &&
    brainIdentityContext.durableMemoryManifest === nextDurableMemoryManifest;
  brainIdentityContext.userFirstName = nextUserFirstName;
  brainIdentityContext.agentFirstName = nextAgentFirstName;
  brainIdentityContext.userHomeLocation = nextUserHomeLocation;
  brainIdentityContext.personalityManifest = nextPersonalityManifest;
  brainIdentityContext.durableMemoryManifest = nextDurableMemoryManifest;
  if (unchanged) return;
  void persistBrainIdentityContext();
  if (
    panelsChatBottomState.activeSessionId ||
    panelsChatBottomState.transcript.some(
      (message) => message.role === "system" && message.id.startsWith(BRAIN_BOOT_MESSAGE_ID_PREFIX)
    )
  ) {
    ensureBrainBootTranscript(panelsChatBottomState.activeSessionId || "draft");
  }
}

function brainBootManifest(): string {
  const identityMemory = brainIdentityMemoryManifest();
  const personalityMemory = brainPersonalityContextManifest();
  const durableBrainMemory = brainDurableMemoryContextManifest();
  const codingBrainOnlyCommands = new Set<string>([
    BRAIN_CODEDOCS_COMMAND,
    BRAIN_GITHUB_MCP_COMMAND,
    BRAIN_WEBACT_COMMAND,
    BRAIN_SECURITYSCAN_COMMAND
  ]);
  const generalBrainCodeActCommands = BRAIN_CODEACT_COMMANDS.filter((command) =>
    command !== BRAIN_QUESTIONNAIRE_COMMAND && !codingBrainOnlyCommands.has(command)
  );
  const commandDescriptions = BRAIN_CODEACT_COMMAND_DESCRIPTIONS
    .filter((item) => item.command !== BRAIN_QUESTIONNAIRE_COMMAND && !codingBrainOnlyCommands.has(item.command))
    .map((item) => `${item.command}: ${item.description}`)
    .join(" | ");
  const actionManifest = agentActionHostPromptManifest(agentActionHostConfig());
  return [
    "BRAIN_BOOT_MANIFEST v1",
    "source=src/brain.rs",
    "injection_policy=general_brain_once_at_session_start_and_after_compaction; specialized_brains_once_on_switch_and_after_compaction",
    identityMemory,
    personalityMemory,
    durableBrainMemory,
    `codeact_commands=${generalBrainCodeActCommands.join(" ")}`,
    `codeact_descriptions=${commandDescriptions}`,
    `codeact_routing_rules=${BRAIN_CODEACT_ROUTING_RULES}`,
    `scrapers_mcp_policy=${BRAIN_SCRAPERS_COMMAND} runs Scrapling MCP + Crawl4AI MCP in parallel for known URLs. Use after ${BRAIN_WEBSEARCH_COMMAND} or explicit URLs to enrich answers with clean Markdown, structured fields, links, image/video/audio URLs, screenshots/artifact refs and provenance. Return one compact SCRAPERS_RESULT/media_manifest; respect robots/terms/rate limits/privacy, stealth only when authorized.`,
    `rule=On the first assistant response in this session: start the response exactly with the internal tag /rename_session_<short title>_ before any visible prose. <short title> is a relevant, grammatically correct 2-5 word session title for the topic. Never mention this action, never explain it, and never display it as content. The app strips the tag before display and uses the title everywhere instead of "New session".`,
    "rule=Brain is the single source of truth for CodeAct command identities; do not invent or revive commands outside this manifest.",
    "rule=General Brain is immutable and read-only. The agent must never write, patch or synthesize new CodeActs inside General Brain. When /newbrain_ is emitted, the host derives the specialized activator CodeAct such as /musicianbrain_ from explicit fields and stores it outside General Brain.",
    "rule=Use Brain memory/search before asking the user to repeat prior local session context.",
    "learning_interrupt_markup=[[learn type=anti_pattern|working_pattern|user_preference|domain_rule|skill_candidate|codeact_candidate|research_candidate scope=session|project|domain confidence=0.00..1.00 promote=lesson,skill,task,codeact,research]]short candidate observation[[/learn]]",
    "learning_interrupt_rule=During multi-step work, if a repeated avoidable error, stable correction, reusable solution, user preference, domain rule, skill candidate, CodeAct candidate or worthwhile research branch appears, emit exactly one compact [[learn ...]]...[[/learn]] candidate line near the relevant paragraph. Prefer promote=lesson when an observed error should become a replacement rule.",
    "learning_interrupt_guard=Learning interrupts are proposals only: never claim the Brain has persisted them until a user action or verified Brain/CodeAct promotion accepts them. Prefer at most one learning interrupt per turn unless the user asks for more.",
    `rule=If local code/files/project work needs a folder and no workspace is active, emit ${BRAIN_WORKSPACE_COMMAND}. This workspace rule does not apply to ${BRAIN_NEWIMAGE_COMMAND} or ${BRAIN_EDITIMAGE_COMMAND}.`,
    "LOCAL_ACTION_ATLAS_BOOT v1",
    "rule=This full local action atlas is injected at session boot and after conversation compaction only. Do not require the app to semantically guess local tool needs on each turn.",
    `local_actions_codeact=Emit ${BRAIN_LOCAL_ACTIONS_COMMAND} scope="all" query="short task" maxResults="40" whenever a task may involve local files, coding, Windows, browser, documents, cloud CLIs, virtualization or automation and the exact action is not obvious. The host maps this read-only CodeAct to AGENT_ACTION_JSON action:"capabilities" and returns LOCAL_ACTIONS_RESULT.`,
    ingenLoopActionRhythmManifest(),
    agentActionRoutingHint(),
    actionManifest
  ].filter(Boolean).join("\n");
}

function ingenLoopActionRhythmManifest(): string {
  return [
    "INGEN_LOOP_ACTION_RHYTHM v1",
    "default_mode=loop_stream_chunked",
    "reasoning_owner=LLM",
    "app_role=execute_explicit_codeact_or_agent_action_validate_collect_observations_return_proof",
    "start_state=General Brain only; activate a specialized Brain with its CodeAct when the LLM decides the task needs that catalog.",
    "direct_answer_exception=Only trivial answer-only turns that need no Brain switch, no external/local action, no evidence branch and no proof may skip an action boundary.",
    "chunking=Stream useful visible progress in compact chunks; each chunk that implies work must end with one explicit CodeAct or AGENT_ACTION_JSON boundary.",
    "action_boundary=Every Brain CodeAct from every active/injected Brain and every AGENT_ACTION_JSON line is a loop-stream action boundary, not decorative prompt text.",
    "rhythm=short visible paragraph -> one CodeAct or AGENT_ACTION_JSON -> host result/proof -> LLM chooses the next step.",
    "branching=The LLM may open temporary side branches for evidence, research, verification or setup; each branch must return a compact result to the parent objective.",
    "no_promise_without_event=When work requires app execution, do not only announce it in prose; emit the matching CodeAct or AGENT_ACTION_JSON immediately.",
    "stop_policy=When the objective is satisfied or blocked by user permission, stop with a compact natural summary and no fake action event."
  ].join("\n");
}

function brainRuntimeReminderManifest(): string {
  return [
    "BRAIN_RUNTIME_REMINDER v1",
    "boot_manifest=already_injected_once_for_this_session_or_reinjected_after_compaction",
    "brain_catalogs=general_at_boot; science_or_coding_only_on_switch_and_after_compaction",
    brainPersonalityContextManifest(),
    ingenLoopActionRhythmManifest(),
    `local_actions_codeact=${BRAIN_LOCAL_ACTIONS_COMMAND} is available in General, Science, Coding and specialized Brains; it executes a read-only atlas lookup through AGENT_ACTION_JSON action:"capabilities".`,
    "rule=Do not invent local tool access. If the exact local route is unclear, call /local_actions_, then choose one executable action from the returned atlas and wait for AGENT_ACTION_RESULT."
  ].join("\n");
}

function workspaceContextManifest(): string {
  if (!workspaceExplicitlyChosen) {
    return [
      "WORKSPACE_CONTEXT v1",
      "active=false",
      `rule=If local code/files/project work needs a folder, emit ${BRAIN_WORKSPACE_COMMAND}. Do not emit ${BRAIN_WORKSPACE_COMMAND} for image generation or image editing.`
    ].join("\n");
  }
  return [
    "WORKSPACE_CONTEXT v1",
    "active=true",
    `label=${basename(activeWorkspaceDir)}`,
    `path=${activeWorkspaceDir}`,
    `cwd=${activeWorkspaceDir}`
  ].join("\n");
}

function agentActionHostConfig(): AgentActionHostConfig {
  return {
    workspaceRoot: activeWorkspaceDir,
    workspaceActive: workspaceExplicitlyChosen,
    cwd: activeWorkspaceDir,
    platform: process.platform
  };
}

function textLooksLikeLocalActionIntent(text: string): boolean {
  const normalized = text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
  return /\b(?:ordinateur|bureau|desktop|fichier|fichiers|dossier|dossiers|repertoire|repertoires|workspace|repo|projet|terminal|powershell|cmd|shell|commande|commandes|chercher|rechercher|trouver|lister|copier|copie|deplacer|renommer|creer|ecrire|modifier|supprimer|effacer|ouvrir|telecharger|sauvegarder|enregistrer|git|npm|cargo|test|build|code|coder|html|css|javascript|typescript|react|vite|page|site|frontend|front-end|composant)\b/.test(normalized);
}

function textLooksLikeVisualCodingArtifactGoal(text: string): boolean {
  const normalized = text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
  return /\b(?:code|coder|html|css|javascript|js|typescript|ts|react|vite|page|site|frontend|front-end|composant|component|animation|canvas|webgl|svg|visuel|visual|interface|ui)\b/.test(normalized);
}

function assistantLooksLikeSnippetInsteadOfLocalAction(text: string): boolean {
  const normalized = text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
  return (
    /```(?:html|css|js|javascript|typescript|tsx|jsx)?[\s\S]*?```/i.test(text) ||
    /\b(?:copie-colle|copier-coller|mets?\s+ca\s+dans\s+un\s+fichier|mettez\s+ca\s+dans\s+un\s+fichier|ouvre-le\s+dans\s+ton\s+navigateur|ouvrir\s+dans\s+ton\s+navigateur)\b/i.test(normalized)
  );
}

function shouldForceInitialCodingVisualAction(originalUserText: string, assistantText: string): boolean {
  return (
    textLooksLikeVisualCodingArtifactGoal(originalUserText) &&
    assistantLooksLikeSnippetInsteadOfLocalAction(assistantText) &&
    !extractAgentActionJsonRequest(assistantText) &&
    !assistantText.includes(BRAIN_CODING_LIVE_PREVIEW_COMMAND) &&
    !brainSegmentFromAssistantText(assistantText)
  );
}

const BRAIN_CODEACT_COMMANDS_BY_LENGTH = [...BRAIN_CODEACT_COMMANDS].sort((left, right) => right.length - left.length);

function brainCodeActCommandsFromAssistantText(text: string): BrainCodeActCommand[] {
  const commands: BrainCodeActCommand[] = [];
  for (const command of BRAIN_CODEACT_COMMANDS_BY_LENGTH) {
    if (text.includes(command)) {
      commands.push(command as BrainCodeActCommand);
    }
  }
  return commands;
}

function isBrainCodeActUserPauseCommand(command: BrainCodeActCommand): boolean {
  return (
    command === BRAIN_QUESTIONNAIRE_COMMAND ||
    command === BRAIN_WORKSPACE_COMMAND ||
    command === BRAIN_NEWIMAGE_COMMAND ||
    command === BRAIN_EDITIMAGE_COMMAND ||
    command === BRAIN_CODING_LIVE_PREVIEW_COMMAND
  );
}

function isBrainCodeActSurfaceCommand(command: BrainCodeActCommand): boolean {
  return (
    command === BRAIN_GOOGLEWEB_COMMAND ||
    command === BRAIN_MAPS_COMMAND ||
    command === BRAIN_GMAIL_COMMAND ||
    command === BRAIN_GMAIL_COM_COMMAND ||
    command === BRAIN_AIRBNB_COMMAND ||
    command === BRAIN_WEB_COMMAND
  );
}

function textLooksLikeMapBackedAnswerIntent(value: string): boolean {
  const text = normalizedRoutingIntentText(value);
  if (!text || userTextHasTravelOrStayIntent(text)) {
    return false;
  }
  if (/^(ouvre|ouvrir|lance|affiche|montre)?\s*(google earth|maps?|carte)\s*$/.test(text)) {
    return false;
  }
  return /\b(parle|explique|raconte|decris|presente|resume|synthese|analyse|compare|liste|classe|classer|tableau|qui|quoi|pourquoi|comment|histoire|historique|culture|origine|inspir|film|anime|oeuvre|lieu|ile|pays|ville|region)\b/i.test(text);
}

function shouldContinueAfterBrainCodeAct(params: {
  originalUserText: string;
  assistantText: string;
  activatedBrainSegment?: ActiveBrainSegmentId;
  previousBrainSegment: BrainSegmentId;
}): boolean {
  if (extractAgentActionJsonRequest(params.assistantText)) {
    return false;
  }
  if (params.activatedBrainSegment && params.activatedBrainSegment !== params.previousBrainSegment) {
    return false;
  }
  const commands = brainCodeActCommandsFromAssistantText(params.assistantText);
  if (commands.length === 0) {
    return false;
  }
  if (commands.every((command) => isBrainCodeActUserPauseCommand(command) || isBrainCodeActSurfaceCommand(command))) {
    return commands.includes(BRAIN_MAPS_COMMAND) && textLooksLikeMapBackedAnswerIntent(params.originalUserText);
  }
  return (
    textLooksLikeLocalActionIntent(params.originalUserText) ||
    textLooksLikeVisualCodingArtifactGoal(params.originalUserText) ||
    commands.some(
      (command) =>
        command === BRAIN_LOCAL_ACTIONS_COMMAND ||
        command === BRAIN_SEARCHARCHIVE_COMMAND ||
        command === BRAIN_NEWCOMPUTE_COMMAND ||
        command === BRAIN_SELECTCOMPUTE_COMMAND ||
        command === BRAIN_NAMED_COMPUTE_COMMAND ||
        command === BRAIN_NEWOBJECT_COMMAND ||
        command === BRAIN_FRONTDESIGN_COMMAND ||
        command === BRAIN_GOOGLE_AGENDA_COMMAND ||
        command === BRAIN_WEBSEARCH_COMMAND ||
        command === BRAIN_CODEDOCS_COMMAND ||
        command === BRAIN_GITHUB_MCP_COMMAND ||
        command === BRAIN_WEBACT_COMMAND ||
        command === BRAIN_SECURITYSCAN_COMMAND ||
        command === BRAIN_SCRAPERS_COMMAND ||
        command === BRAIN_BRAIN_COMMAND ||
        command === BRAIN_NEWMODULE_COMMAND ||
        command === BRAIN_RUST_PORT_ADAPTER_COMMAND ||
        command === BRAIN_RUST_STATE_STORE_COMMAND
    )
  );
}

function textIsAgentActionContinuation(text: string): boolean {
  return /\bAGENT_ACTION_(?:RESULT|FORCED_CONTINUATION)\b/i.test(text);
}

function transcriptHasRecentAgentActionLoop(transcript: TranscriptMessage[] = panelsChatBottomState.transcript): boolean {
  return [...transcript]
    .reverse()
    .slice(0, 6)
    .some((message) =>
      /AGENT_ACTION_RESULT|AGENT_ACTION_JSON|\/agent_(?:capabilities|list|search|create_directory|rename_path|move_path|copy_path|delete_empty_directory|delete_tree|readonly_shell|shell)_/i.test(message.text)
    );
}

function shouldInjectFullAgentActionManifest(
  userText = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): boolean {
  return textLooksLikeLocalActionIntent(userText) && !textIsAgentActionContinuation(userText);
}

function shouldInjectCompactAgentActionManifest(
  userText = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): boolean {
  return !shouldInjectFullAgentActionManifest(userText, transcript) &&
    (textIsAgentActionContinuation(userText) || transcriptHasRecentAgentActionLoop(transcript));
}

function agentActionContinuationManifest(): string {
  const config = agentActionHostConfig();
  const manifest = createAgentActionHostManifest(config);
  return [
    "AGENT_ACTION_HOST_CONTINUATION v1",
    `workspace_active=${manifest.workspace.active}`,
    `workspace_root=${manifest.workspace.root}`,
    `manifest_hash=${manifest.runtime.manifestHash}`,
    `atlas_hash=${manifest.runtime.atlasHash}`,
    `delta_policy=${manifest.runtime.injectionPolicy}`,
    `prompt_budget=${manifest.runtime.promptBudget}`,
    `result_reinjection=${manifest.runtime.resultReinjectionPolicy}`,
    `token_estimate_compact=${manifest.runtime.promptTokenEstimate.compactContinuation}`,
    `token_estimate_selected_capability=${manifest.runtime.promptTokenEstimate.selectedCapabilityDetail}`,
    `installed_tools_delta=${manifest.runtime.installedToolIds.join("|")}`,
    `missing_tools_delta=${manifest.runtime.missingToolIds.join("|")}`,
    `available=${manifest.runtime.executableActionIds.join(" ")}`,
    `planned_families_delta=${manifest.runtime.plannedFamilies.join("|")}`,
    `blocked_families_delta=${manifest.runtime.blockedFamilies.join("|")}`,
    `approval_gated_families_delta=${manifest.runtime.approvalGatedFamilies.join("|")}`,
    "format=Continue with one short natural progress paragraph, then exactly one AGENT_ACTION_JSON line when another local action is needed.",
    "action_request_format=AGENT_ACTION_JSON {\"action\":\"move_path\",\"scope\":\"computer\",\"path\":\"C:\\\\from.txt\",\"toPath\":\"C:\\\\to.txt\",\"confirmed\":true}",
    "result_policy=Use the compact AGENT_ACTION_RESULT from the previous round as ground truth; do not ask for the full listing again unless the compact result is insufficient.",
    "selected_capability_policy=Use AGENT_ACTION_SELECTED_CAPABILITY from the previous tool result to pick verification and fallbacks; do not request the full atlas unless the selected capability is insufficient.",
    "stop_policy=If the objective is done, return only a compact final summary and do not emit AGENT_ACTION_JSON.",
    `proof=${manifest.proofHash}`
  ].join("\n");
}

function agentActionContextManifest(
  userText = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  if (shouldInjectFullAgentActionManifest(userText, transcript)) {
    return [
      "AGENT_ACTION_LOCAL_INTENT_REMINDER v1",
      "full_atlas_location=brain_boot_manifest_or_post_compaction_reinjection",
      `rule=Use the boot atlas for known routes. If the exact local route is unclear, request a fresh targeted atlas with ${BRAIN_LOCAL_ACTIONS_COMMAND} before answering verbally.`,
      agentActionRoutingHint(),
      agentActionContinuationManifest()
    ].join("\n");
  }
  if (shouldInjectCompactAgentActionManifest(userText, transcript)) {
    return [agentActionRoutingHint(), agentActionContinuationManifest()].join("\n");
  }
  return "";
}

function brainSegmentManifest(): string {
  if (panelsChatBottomState.activeBrainSegment === "science") {
    return [
      "ACTIVE_BRAIN_SEGMENT v1",
      "active=science",
      brainPersonalityContextManifest(),
      `activated_by=${BRAIN_SCIENCE_COMMAND}`,
      `rule=${BRAIN_SCIENCE_COMMAND} is already active for this session; continue directly in Science Brain and do not emit ${BRAIN_SCIENCE_COMMAND} again unless a later turn explicitly switches away first.`,
      BRAIN_SCIENCE_VISIBLE_CATALOG
    ].join("\n");
  }
  if (panelsChatBottomState.activeBrainSegment === "coding") {
    return [
      "ACTIVE_BRAIN_SEGMENT v1",
      "active=coding",
      brainPersonalityContextManifest(),
      `activated_by=${BRAIN_CODING_COMMAND}`,
      `rule=${BRAIN_CODING_COMMAND} is already active for this session; continue directly in Coding Brain and do not emit ${BRAIN_CODING_COMMAND} again unless a later turn explicitly switches away first.`,
      `visual_artifact_rule=For HTML/CSS/JS/React/Vite pages, animations or visual components, never answer with copy-paste code or "put this in a file" unless the user explicitly asked for snippet-only help. Create or modify the real local file with AGENT_ACTION_JSON first, wait for AGENT_ACTION_RESULT proof, then emit ${BRAIN_CODING_LIVE_PREVIEW_COMMAND} path="verified absolute file path" kind="html|react|vite".`,
      `coding_priority_mcp_policy=Use ${BRAIN_CODEDOCS_COMMAND} for Context7 current dependency docs once the package is known, ${BRAIN_GITHUB_MCP_COMMAND} for remote GitHub repo/issues/PR/CI/release context, ${BRAIN_WEBACT_COMMAND} for Playwright browser interaction or visual verification, and ${BRAIN_SECURITYSCAN_COMMAND} for Semgrep security/static-analysis checks before risky commits or PRs. These are Coding Brain action events: emit the command, wait for the *_RESULT manifest, then continue with the next concrete action.`,
      BRAIN_CODING_VISIBLE_CATALOG
    ].join("\n");
  }
  return [
    "ACTIVE_BRAIN_SEGMENT v1",
    "active=general",
    brainPersonalityContextManifest(),
    `mandatory=Before answering or acting, classify the user's task by semantic domain. If key choices, constraints, scope, files, risks or success criteria are missing, you may use ${BRAIN_QUESTIONNAIRE_COMMAND} from General Brain before acting. If the task is already clear and belongs to science, engineering, mathematics, biology, chemistry, physics, cryptography, formal analysis, physical product design, electronics, mechanics, robotics, CAD/3D, Banger, future Banger 3D, Monster or /newcompute_ work, write a short natural acknowledgement and activate ${BRAIN_SCIENCE_COMMAND} before detailed specialized work.`,
    "mandatory=Physical product or prototype conception is engineering by default, even if the object is ordinary or newly mentioned.",
    "mandatory=The decision to activate /sciencebrain_ is semantic, not keyword-based: infer the domain implied by the user's natural-language request, even when the exact object, field or project name has never appeared in the Brain.",
    `mandatory=If the task is already clear and belongs to software engineering, coding, websites, applications, repository work, debugging, tests, architecture, scripts, API, Rust, TypeScript, Electron or developer tooling, write a short natural acknowledgement and activate ${BRAIN_CODING_COMMAND} before detailed specialized work.`,
    `questionnaire_rule=${BRAIN_QUESTIONNAIRE_COMMAND} is a General Brain CodeAct too: use it only when guessing would hurt the result; skip it for trivial tasks or obvious safe defaults.`,
    `sciencebrain_activation_format=${BRAIN_SCIENCE_COMMAND} segment="science" reason="short LLM-authored reason" output="inject_brain_catalog"`,
    `codingbrain_activation_format=${BRAIN_CODING_COMMAND} segment="coding" reason="short LLM-authored reason" output="inject_brain_catalog"`
  ].join("\n");
}

function selfDirectedModeManifest(): string {
  if (panelsChatBottomState.permissionMode !== "self-directed") {
    return "";
  }
  return [
    "SELF_DIRECTED_MODE v1",
    "rule=Keep the normal Brain path first: read the Brain boot manifest, classify the request, and activate /sciencebrain_ or /codingbrain_ before deep work when the domain requires it.",
    `mandatory=After the correct Brain is active, and before starting project work for a natural user direction, emit ${BRAIN_QUESTIONNAIRE_COMMAND} to clarify the target. Do not skip this in Self-Directed mode.`,
    `questionnaire_format=Use title, intro, q1/q2/q3/q4/q5 maximum and qN_options. The final question must define the stop condition: ask exactly how the agent will know the objective is reached.`,
    "questionnaire_options=Use expert option cards, not vague Option 1/2/3 labels. Include one recommended option and one more ambitious/high-quality option when useful.",
    "after_answers=When the user message starts SELF_DIRECTED_QUESTIONNAIRE_ANSWERS v1, begin autonomous multi-step work immediately; do not ask the same questionnaire again unless the answers are contradictory.",
    "work_rhythm=Write one short paragraph that states the next concrete action and why, then emit the relevant CodeAct command/event below it. Repeat this paragraph -> event rhythm while work remains.",
    `tool_policy=Use Brain CodeActs and specialized commands when useful: ${BRAIN_GOOGLEWEB_COMMAND} for web research, ${BRAIN_NEWCOMPUTE_COMMAND}/${BRAIN_SELECTCOMPUTE_COMMAND} for Monster, ${BRAIN_NEWMODULE_COMMAND} for module materialization, ${BRAIN_NEWOBJECT_COMMAND} for Banger objects, ${BRAIN_FRONTDESIGN_COMMAND} for native palette work.`,
    "goal_reached=When the user's explicit stop condition is satisfied, include a compact marker line: SELF_DIRECTED_GOAL_REACHED reason=\"short reason\" next_prompt=\"a stronger next project direction\".",
    "continuation=When the user message starts SELF_DIRECTED_CONTINUATION v1 or SELF_DIRECTED_PROJECT_EXPANSION v1, continue the work directly unless a new ambiguity truly requires /questionnaire_.",
    "guardrails=No payment, credential action, destructive deletion or irreversible external submission without explicit human confirmation."
  ].join("\n");
}

function ingenProviderRuntimeContract(params: {
  provider: "codex" | "claude" | "openrouter";
  moduleId?: string;
  userText?: string;
  transcript?: TranscriptMessage[];
}): string {
  const transcript = params.transcript ?? panelsChatBottomState.transcript;
  const userText = params.userText ?? "";
  const moduleId = params.moduleId ?? "";
  return [
    "INGEN_PROVIDER_RUNTIME_CONTRACT v1",
    `provider=${params.provider}`,
    "language=match_user_prefer_french_when_unspecified",
    "surface=local_InGen_assistant",
    "rule=Do not invent runtime behavior, hidden app reasoning, execution status or tool results.",
    brainRuntimeReminderManifest(),
    brainIdentityMemoryManifest(),
    workspaceContextManifest(),
    agentActionContextManifest(userText, transcript),
    brainSegmentManifest(),
    selfDirectedModeManifest(),
    webExplorerCodeActInstructions(moduleId)
  ].filter(Boolean).join("\n");
}

function codexDirectInstructions(
  reasoning: string,
  moduleId = "",
  userText = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): string {
  return [
    `@forge:direct:v1 p=Codex lang=fr tools=codeact effort=${reasoning}`,
    "style=francais naturel, concis; reponds directement.",
    ingenProviderRuntimeContract({
      provider: "codex",
      moduleId,
      userText,
      transcript
    })
  ].filter(Boolean).join("\n");
}

function drainCodexSseEvents(buffer: { text: string }): unknown[] {
  const events: unknown[] = [];
  while (true) {
    const boundary = buffer.text.indexOf("\n\n");
    if (boundary < 0) {
      break;
    }
    const chunk = buffer.text.slice(0, boundary).replace(/\r\n/g, "\n");
    buffer.text = buffer.text.slice(boundary + 2);
    const data = chunk
      .split("\n")
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).trimStart())
      .join("\n")
      .trim();
    if (!data || data === "[DONE]") {
      continue;
    }
    try {
      events.push(JSON.parse(data) as unknown);
    } catch {
      // Ignore malformed or non-JSON stream frames.
    }
  }
  return events;
}

function codexDirectResponseOutputText(value: unknown): string {
  if (!value || typeof value !== "object") {
    return "";
  }
  const record = value as Record<string, unknown>;
  const response = record.response && typeof record.response === "object"
    ? (record.response as Record<string, unknown>)
    : undefined;
  const output = Array.isArray(record.output) ? record.output : Array.isArray(response?.output) ? response.output : [];
  let text = "";
  for (const item of output) {
    if (!item || typeof item !== "object") {
      continue;
    }
    const contents = Array.isArray((item as Record<string, unknown>).content)
      ? ((item as Record<string, unknown>).content as unknown[])
      : [];
    for (const content of contents) {
      if (!content || typeof content !== "object") {
        continue;
      }
      const contentRecord = content as Record<string, unknown>;
      if (contentRecord.type === "output_text" && typeof contentRecord.text === "string") {
        text += contentRecord.text;
      }
    }
  }
  return text;
}

function applyCodexDirectEventText(event: unknown, finalText: string): string {
  if (!event || typeof event !== "object") {
    return finalText;
  }
  const record = event as Record<string, unknown>;
  const type = typeof record.type === "string" ? record.type : "";
  if (type.includes("output_text.delta") && typeof record.delta === "string") {
    return finalText + record.delta;
  }
  if (type === "response.completed") {
    const fallback = codexDirectResponseOutputText(record.response ?? record);
    return !finalText.trim() && fallback.trim() ? fallback : finalText;
  }
  if (!finalText.trim()) {
    const fallback = codexDirectResponseOutputText(record);
    if (fallback.trim()) {
      return fallback;
    }
  }
  return finalText;
}

function parseCodexDirectEventStream(rawText: string): string {
  const buffer = { text: `${rawText}\n\n` };
  let finalText = "";
  for (const event of drainCodexSseEvents(buffer)) {
    finalText = applyCodexDirectEventText(event, finalText);
  }
  return finalText.trim();
}

async function readCodexDirectEventStream(response: Response, liveSink?: ProviderLiveTextSink): Promise<string> {
  const body = response.body;
  if (!body) {
    return parseCodexDirectEventStream(await response.text());
  }
  const reader = body.getReader();
  const decoder = new TextDecoder();
  const buffer = { text: "" };
  let finalText = "";
  const emitEvents = async () => {
    for (const event of drainCodexSseEvents(buffer)) {
      finalText = applyCodexDirectEventText(event, finalText);
      if (finalText.trim()) {
        liveSink?.onText(finalText.trimEnd());
      }
      if (liveSink?.shouldStop?.(finalText)) {
        await reader.cancel().catch(() => undefined);
        return true;
      }
    }
    return false;
  };
  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }
    buffer.text += decoder.decode(value, { stream: true });
    if (await emitEvents()) {
      return finalText.trim();
    }
  }
  buffer.text += decoder.decode();
  buffer.text += "\n\n";
  await emitEvents();
  return finalText.trim();
}

function parseChatGptEventStream(text: string): string {
  let finalText = "";
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith("data:")) {
      continue;
    }
    const data = line.slice(5).trim();
    if (!data || data === "[DONE]") {
      continue;
    }
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      const message = parsed.message && typeof parsed.message === "object"
        ? (parsed.message as Record<string, unknown>)
        : undefined;
      const content = message?.content && typeof message.content === "object"
        ? (message.content as Record<string, unknown>)
        : undefined;
      const parts = Array.isArray(content?.parts) ? content.parts : [];
      const textPart = parts.filter((part): part is string => typeof part === "string").join("\n").trim();
      if (message?.author && typeof message.author === "object") {
        const role = (message.author as Record<string, unknown>).role;
        if (role === "assistant" && textPart) {
          finalText = textPart;
        }
      } else if (textPart) {
        finalText = textPart;
      }
    } catch {
      // Ignore non-JSON event stream lines.
    }
  }
  return finalText;
}

async function waitForChatGptComposer(runtimeWindow: BrowserWindow, timeoutMs = 45_000): Promise<void> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (runtimeWindow.isDestroyed()) {
      throw new Error("Codex runtime window closed.");
    }
    const state = await runtimeWindow.webContents.executeJavaScript(
      `(() => {
        const text = document.body ? document.body.innerText : "";
        const login = /Log in or sign up|Connectez-vous ou inscrivez-vous|Se connecter|Inscription gratuite|Continue with Google|Continuer avec Google/i.test(text);
        const prompt = document.querySelector("#prompt-textarea, textarea, [contenteditable='true']");
        return { login, ready: Boolean(prompt), title: document.title, url: location.href };
      })()`,
      true
    );
    const candidate = state && typeof state === "object" ? (state as Record<string, unknown>) : {};
    if (candidate.login === true) {
      throw new Error("ChatGPT login is required in LLM Provider before conversation can start.");
    }
    if (candidate.ready === true) {
      return;
    }
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 500);
    });
  }
  throw new Error("ChatGPT composer did not become ready.");
}

async function probeChatGptComposerReady(authWindow: BrowserWindow, timeoutMs = 20_000): Promise<boolean> {
  try {
    await waitForChatGptComposer(authWindow, timeoutMs);
    return true;
  } catch {
    return false;
  }
}

async function runChatGptPageConversation(runtimeWindow: BrowserWindow, profile: ProviderRuntimeProfile, userText: string): Promise<ProviderTextRun> {
  const model = codexRuntimeModel(profile);
  const reasoning = codexRuntimeReasoning(profile);
  await runtimeWindow.loadURL(`${CHATGPT_HOME_URL}?model=${encodeURIComponent(model)}`);
  try {
    await waitForChatGptComposer(runtimeWindow);
  } catch (error) {
    if (!/composer did not become ready/i.test(error instanceof Error ? error.message : String(error))) {
      throw error;
    }
    await runtimeWindow.loadURL(CHATGPT_HOME_URL);
    await waitForChatGptComposer(runtimeWindow);
  }

  const submitResult = await runtimeWindow.webContents.executeJavaScript(
    `(async () => {
      const assistantTexts = () => Array.from(document.querySelectorAll("[data-message-author-role='assistant'], article, [data-testid^='conversation-turn-']"))
        .map((node) => node && node.innerText ? node.innerText.trim() : "")
        .filter((text, index, list) => text && list.indexOf(text) === index);
      const beforeCount = assistantTexts().length;
      const prompt = document.querySelector("#prompt-textarea, textarea, [contenteditable='true']");
      if (!prompt) {
        return { ok: false, error: "ChatGPT prompt input not found", beforeCount };
      }
      const text = ${JSON.stringify(userText)};
      prompt.focus();
      if (prompt instanceof HTMLTextAreaElement || prompt instanceof HTMLInputElement) {
        const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(prompt), "value");
        if (descriptor && descriptor.set) {
          descriptor.set.call(prompt, text);
        } else {
          prompt.value = text;
        }
        prompt.dispatchEvent(new Event("input", { bubbles: true }));
        prompt.dispatchEvent(new Event("change", { bubbles: true }));
      } else {
        const selection = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(prompt);
        selection && selection.removeAllRanges();
        selection && selection.addRange(range);
        document.execCommand("insertText", false, text);
        prompt.dispatchEvent(new InputEvent("input", { bubbles: true, composed: true, inputType: "insertText", data: text }));
      }
      await new Promise((resolve) => setTimeout(resolve, 350));
      const buttonCandidates = Array.from(document.querySelectorAll("button"));
      const sendButton = buttonCandidates.find((button) => {
        const label = [button.getAttribute("data-testid"), button.getAttribute("aria-label"), button.title, button.innerText].join(" ");
        return /send|envoyer|submit/i.test(label) && !button.disabled;
      }) || document.querySelector("[data-testid='send-button']:not([disabled])");
      if (!sendButton) {
        prompt.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
        prompt.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
      } else {
        sendButton.click();
      }
      return { ok: true, beforeCount };
    })()`,
    true
  );
  const submission = submitResult && typeof submitResult === "object" ? (submitResult as Record<string, unknown>) : {};
  if (submission.ok !== true) {
    throw new Error(typeof submission.error === "string" ? submission.error : "ChatGPT page submission failed.");
  }
  const beforeCount = typeof submission.beforeCount === "number" ? submission.beforeCount : 0;
  let lastText = "";
  let stableCount = 0;
  let firstTextAt = 0;
  const startedAt = Date.now();
  while (Date.now() - startedAt < 150_000) {
    const state = await runtimeWindow.webContents.executeJavaScript(
      `(() => {
        const assistantTexts = Array.from(document.querySelectorAll("[data-message-author-role='assistant'], article, [data-testid^='conversation-turn-']"))
          .map((node) => node && node.innerText ? node.innerText.trim() : "")
          .filter((text, index, list) => text && list.indexOf(text) === index);
        const busy = Boolean(document.querySelector("[data-testid='stop-button'], button[aria-label*='Stop'], button[aria-label*='Arrêter'], button[aria-label*='stop']"));
        const text = assistantTexts.length > ${beforeCount} ? assistantTexts[assistantTexts.length - 1] : "";
        return { count: assistantTexts.length, text, busy };
      })()`,
      true
    );
    const candidate = state && typeof state === "object" ? (state as Record<string, unknown>) : {};
    const text = typeof candidate.text === "string" ? candidate.text.trim() : "";
    const busy = candidate.busy === true;
    if (text && firstTextAt === 0) {
      firstTextAt = Date.now();
    }
    if (text && text === lastText) {
      stableCount += 1;
    } else if (text) {
      lastText = text;
      stableCount = 0;
    }
    if (lastText && stableCount >= 4 && Date.now() - firstTextAt >= 4_000 && !busy) {
      return {
        text: lastText,
        runtime: `chatgpt page session / ${model} / reasoning ${reasoning}`
      };
    }
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 1_000);
    });
  }
  throw new Error(lastText ? `ChatGPT page response did not finish: ${lastText.slice(0, 260)}` : "ChatGPT page returned no assistant response.");
}

async function ensureCodexRuntimeWindow(): Promise<BrowserWindow> {
  const existing = authWindows.get("codex");
  if (existing && !existing.isDestroyed()) {
    return existing;
  }
  if (codexRuntimeWindow && !codexRuntimeWindow.isDestroyed()) {
    return codexRuntimeWindow;
  }
  const parent = primaryWindow && !primaryWindow.isDestroyed() ? primaryWindow : undefined;
  const runtimeWindow = new BrowserWindow({
    width: 480,
    height: 320,
    parent,
    show: false,
    title: "InGen Codex runtime",
    autoHideMenuBar: true,
    backgroundColor: "#0e0e0f",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      backgroundThrottling: false
    }
  });
  runtimeWindow.webContents.setUserAgent(CHATGPT_USER_AGENT);
  codexRuntimeWindow = runtimeWindow;
  runtimeWindow.on("closed", () => {
    if (codexRuntimeWindow === runtimeWindow) {
      codexRuntimeWindow = null;
    }
  });
  await runtimeWindow.loadURL(CHATGPT_HOME_URL);
  await inspectCodexAuthWindow(runtimeWindow);
  return runtimeWindow;
}

async function runCodexWebSubscription(profile: ProviderRuntimeProfile, userText: string): Promise<ProviderTextRun> {
  const runtimeWindow = await ensureCodexRuntimeWindow();
  if (runtimeWindow.isDestroyed()) {
    throw new Error("Codex runtime window closed.");
  }
  const probe = await probeCodexAccount(runtimeWindow);
  if (!probe.ok) {
    return runChatGptPageConversation(runtimeWindow, profile, userText);
  }
  const model = codexRuntimeModel(profile);
  const reasoning = codexRuntimeReasoning(profile);
  const payload = {
    action: "next",
    messages: [
      {
        id: randomClientId(),
        author: { role: "user" },
        content: {
          content_type: "text",
          parts: [userText]
        },
        metadata: {},
        create_time: Math.floor(Date.now() / 1000)
      }
    ],
    parent_message_id: randomClientId(),
    model,
    timezone_offset_min: new Date().getTimezoneOffset(),
    history_and_training_disabled: false,
    suggestions: [],
    conversation_mode: { kind: "primary_assistant" },
    force_paragen: false,
    force_rate_limit: false,
    metadata: {
      ingen_client: "electron_shell",
      selected_reasoning: reasoning
    },
    reasoning_effort: reasoning
  };
  const result = await runtimeWindow.webContents.executeJavaScript(
    `fetch("/backend-api/conversation", {
      method: "POST",
      credentials: "include",
      headers: {
        "Content-Type": "application/json",
        "Accept": "text/event-stream"
      },
      body: ${JSON.stringify(JSON.stringify(payload))}
    }).then(async (response) => ({
      ok: response.ok,
      status: response.status,
      text: await response.text()
    })).catch((error) => ({
      ok: false,
      status: 0,
      text: error && error.message ? error.message : String(error)
    }))`,
    true
  );
  const response = result && typeof result === "object"
    ? (result as { ok?: unknown; status?: unknown; text?: unknown })
    : {};
  const rawText = typeof response.text === "string" ? response.text : "";
  const status = typeof response.status === "number" ? response.status : undefined;
  if (response.ok !== true) {
    try {
      return await runChatGptPageConversation(runtimeWindow, profile, userText);
    } catch (pageError) {
      console.error("ChatGPT page session runtime failed.", pageError);
    }
    if (status === 401 || status === 403) {
      profile.connected = false;
      profile.models = [];
      profile.reasoning = [];
      profile.quotaLabel = `connection expired: ChatGPT conversation returned ${status}`;
      profile.proof = hashJson({ provider: "codex", conversationStatus: status, body: rawText.slice(0, 260) });
      profile.events = [
        "ChatGPT conversation endpoint rejected the session",
        `OpenAI conversation API returned ${status}`,
        "reconnect OpenAI in LLM Provider",
        "not ready"
      ];
      await persistProviderRuntime();
      emitLlmProviderRuntimeEvent({
        provider: "codex",
        events: profile.events,
        models: [],
        reasoning: [],
        quotaLabel: profile.quotaLabel,
        proofHash: profile.proof
      });
    }
    throw new Error(`ChatGPT web conversation failed with ${String(status ?? "unknown")}: ${rawText.slice(0, 260)}`);
  }
  const text = parseChatGptEventStream(rawText);
  if (!text) {
    throw new Error(`ChatGPT web conversation returned no assistant text: ${rawText.slice(0, 260)}`);
  }
  return {
    text,
    runtime: `chatgpt web session / ${model} / reasoning ${reasoning}`
  };
}

async function runCodexOAuthDirect(
  profile: ProviderRuntimeProfile,
  userText: string,
  attachments: ProviderAttachment[],
  moduleId = "",
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript,
  liveSink?: ProviderLiveTextSink
): Promise<ProviderTextRun> {
  const auth = await readCodexLocalAuth();
  if (!auth) {
    throw new Error("No local Codex OAuth credentials found. Connect OpenAI in LLM Provider first.");
  }
  if (!auth.accessToken) {
    throw new Error("No Codex OAuth access token available.");
  }
  if (!auth.accountId) {
    throw new Error("No ChatGPT account id available for Codex OAuth direct mode.");
  }
  const model = codexRuntimeModel(profile);
  const reasoning = codexRuntimeReasoning(profile);
  const textVerbosity = reasoning === "low" ? "low" : "medium";
  const payload = {
    model,
    store: false,
    stream: true,
    instructions: codexDirectInstructions(reasoning, moduleId, userText, transcript),
    input: await openAiResponseConversationInput(userText, attachments, userMessageId, transcript),
    text: {
      verbosity: textVerbosity
    },
    reasoning: {
      effort: reasoning
    },
    tool_choice: "none",
    parallel_tool_calls: false
  };
  const response = await net.fetch("https://chatgpt.com/backend-api/codex/responses", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${auth.accessToken}`,
      "ChatGPT-Account-ID": auth.accountId,
      "OpenAI-Beta": "responses=experimental",
      Origin: "https://chatgpt.com",
      Referer: "https://chatgpt.com/",
      "User-Agent": "codex_cli_rs/0.0.0 (Forge Electron direct)",
      originator: "codex_cli_rs",
      "Accept-Language": "fr-FR,fr;q=0.9,en;q=0.8",
      Accept: "text/event-stream",
      "Content-Type": "application/json"
    },
    body: JSON.stringify(payload)
  });
  if (!response.ok) {
    const rawText = await response.text();
    throw new Error(`Codex OAuth direct HTTP ${response.status}: ${rawText.slice(0, 360)}`);
  }
  const text = await readCodexDirectEventStream(response, liveSink);
  if (!text) {
    throw new Error("Codex OAuth direct returned no assistant text.");
  }
  return {
    text,
    runtime: `codex oauth direct / ${model} / reasoning ${reasoning}`
  };
}

function runProviderCommand(command: string, args: string[], input?: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: workspaceExplicitlyChosen ? activeWorkspaceDir : repoRoot,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => {
      child.kill();
      const tail = [stdout.trim(), stderr.trim()].filter(Boolean).join("\n").slice(-4000);
      reject(new Error(tail ? `${command} timed out.\n${tail}` : `${command} timed out.`));
    }, 90_000);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timeout);
      if (code === 0 && stdout.trim()) {
        resolve(stdout.trim());
        return;
      }
      reject(new Error((stderr || stdout || `${command} exited with code ${code}`).trim()));
    });
    child.stdin.end(input ?? "");
  });
}

function claudeStreamLineText(line: string): string {
  if (!line.trim()) {
    return "";
  }
  try {
    const event = JSON.parse(line) as Record<string, unknown>;
    const text = firstStringField(event, ["text", "content", "result"]);
    return text && !/^(requesting|api_retry)$/i.test(text) ? text : "";
  } catch {
    return line.trim();
  }
}

function runProviderCommandStreamingText(
  command: string,
  args: string[],
  input: string | undefined,
  lineText: (line: string) => string,
  liveSink?: ProviderLiveTextSink
): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: workspaceExplicitlyChosen ? activeWorkspaceDir : repoRoot,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    let stdoutLineBuffer = "";
    const texts: string[] = [];
    let settled = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    const settleResolve = (value: string) => {
      if (settled) return;
      settled = true;
      if (timeout) clearTimeout(timeout);
      resolve(value.trim());
    };
    const settleReject = (error: Error) => {
      if (settled) return;
      settled = true;
      if (timeout) clearTimeout(timeout);
      reject(error);
    };
    const emitText = (text: string) => {
      if (!text) {
        return;
      }
      texts.push(text);
      const currentText = texts.join("\n").trim();
      if (!currentText) {
        return;
      }
      liveSink?.onText(currentText);
      if (liveSink?.shouldStop?.(currentText)) {
        child.kill();
        settleResolve(currentText);
      }
    };
    const processStdoutLines = (chunk: string) => {
      stdoutLineBuffer += chunk;
      const lines = stdoutLineBuffer.split(/\r?\n/);
      stdoutLineBuffer = lines.pop() ?? "";
      for (const line of lines) {
        emitText(lineText(line));
      }
    };
    timeout = setTimeout(() => {
      child.kill();
      const tail = [stdout.trim(), stderr.trim()].filter(Boolean).join("\n").slice(-4000);
      settleReject(new Error(tail ? `${command} timed out.\n${tail}` : `${command} timed out.`));
    }, 90_000);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      const text = String(chunk);
      stdout += text;
      processStdoutLines(text);
    });
    child.stderr.on("data", (chunk) => {
      stderr += String(chunk);
    });
    child.once("error", (error) => {
      settleReject(error);
    });
    child.once("close", (code) => {
      if (settled) return;
      if (stdoutLineBuffer.trim()) {
        emitText(lineText(stdoutLineBuffer));
        stdoutLineBuffer = "";
      }
      const currentText = texts.join("\n").trim();
      if (code === 0 && currentText) {
        settleResolve(currentText);
        return;
      }
      if (code === 0 && stdout.trim()) {
        settleResolve(stdout.trim());
        return;
      }
      settleReject(new Error((stderr || stdout || `${command} exited with code ${code}`).trim()));
    });
    child.stdin.end(input ?? "");
  });
}

async function runCodexSubscriptionExec(
  profile: ProviderRuntimeProfile,
  userText: string,
  attachments: ProviderAttachment[],
  moduleId = "",
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript,
  liveSink?: ProviderLiveTextSink
): Promise<ProviderTextRun> {
  try {
    return await runCodexOAuthDirect(profile, userText, attachments, moduleId, userMessageId, transcript, liveSink);
  } catch (directError) {
    const message = directError instanceof Error ? directError.message : String(directError);
    console.error("Codex OAuth direct runtime failed.", directError);
    throw new Error(`Codex subscription OAuth direct failed: ${message}`);
  }
}

async function runClaudeCodePrint(
  profile: ProviderRuntimeProfile,
  userText: string,
  moduleId = "",
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript,
  liveSink?: ProviderLiveTextSink
): Promise<ProviderTextRun> {
  const command = resolveClaudeCodeCommand();
  if (!command) {
    throw new Error("Claude Code CLI is not executable from this Electron process.");
  }
  const model = selectedComposerModel(profile);
  const effort = normalizedReasoningEffort(selectedComposerReasoning(profile));
  const history = recentConversationWindow(userMessageId, transcript)
    .map((message) => `${message.role === "system" ? "System" : message.role === "user" ? "User" : "Assistant"}: ${message.content}`)
    .join("\n\n");
  const promptedUserText = [
    ingenProviderRuntimeContract({
      provider: "claude",
      moduleId,
      userText,
      transcript
    }),
    history ? `Recent conversation:\n${history}` : "",
    `User:\n${userText}`
  ].filter(Boolean).join("\n\n");
  const args = [
    "-p",
    promptedUserText,
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--no-session-persistence",
    "--tools",
    ""
  ];
  if (model && !/catalog unavailable|connect provider/i.test(model)) {
    args.push("--model", model);
  }
  if (effort) {
    args.push("--effort", effort);
  }
  const text = liveSink
    ? await runProviderCommandStreamingText(command, args, undefined, claudeStreamLineText, liveSink)
    : parseClaudeStreamOutput(await runProviderCommand(command, args));
  return {
    text,
    runtime: `${command} -p${model ? ` / ${model}` : ""}${effort ? ` / effort ${effort}` : ""}`
  };
}

function parseClaudeStreamOutput(output: string): string {
  const texts: string[] = [];
  for (const line of output.split(/\r?\n/)) {
    if (!line.trim()) {
      continue;
    }
    try {
      const event = JSON.parse(line) as Record<string, unknown>;
      const text = firstStringField(event, ["text", "content", "result"]);
      if (text && !/^(requesting|api_retry)$/i.test(text)) {
        texts.push(text);
      }
    } catch {
      texts.push(line.trim());
    }
  }
  const result = texts.join("\n").trim();
  if (!result) {
    throw new Error("Claude Code completed without assistant text.");
  }
  return result;
}

function openRouterChatCompletionText(parsed: unknown): string {
  const choices = parsed && typeof parsed === "object" && Array.isArray((parsed as { choices?: unknown }).choices)
    ? (parsed as { choices: unknown[] }).choices
    : [];
  const first = choices[0];
  const message = first && typeof first === "object" ? (first as { message?: unknown }).message : undefined;
  return message && typeof message === "object" ? contentText((message as { content?: unknown }).content) : "";
}

function openRouterStreamContentText(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (!Array.isArray(value)) {
    return "";
  }
  return value
    .map((part) => {
      if (typeof part === "string") {
        return part;
      }
      if (part && typeof part === "object") {
        const record = part as Record<string, unknown>;
        return typeof record.text === "string" ? record.text : typeof record.content === "string" ? record.content : "";
      }
      return "";
    })
    .join("");
}

function openRouterStreamDeltaText(event: unknown): string {
  const choices = event && typeof event === "object" && Array.isArray((event as { choices?: unknown }).choices)
    ? (event as { choices: unknown[] }).choices
    : [];
  const first = choices[0];
  const delta = first && typeof first === "object" ? (first as { delta?: unknown }).delta : undefined;
  return delta && typeof delta === "object" ? openRouterStreamContentText((delta as { content?: unknown }).content) : "";
}

function openRouterStreamErrorText(event: unknown): string {
  if (!event || typeof event !== "object" || !("error" in event)) {
    return "";
  }
  const error = (event as { error?: unknown }).error;
  if (!error) {
    return "OpenRouter stream failed.";
  }
  if (typeof error === "string") {
    return error;
  }
  if (typeof error === "object" && "message" in error && typeof (error as { message?: unknown }).message === "string") {
    return (error as { message: string }).message;
  }
  return JSON.stringify(error);
}

async function readOpenRouterChatCompletionStream(response: Response, liveSink?: ProviderLiveTextSink): Promise<string> {
  const body = response.body;
  if (!body) {
    const parsed = JSON.parse(await response.text()) as unknown;
    return openRouterChatCompletionText(parsed).trim();
  }
  const reader = body.getReader();
  const decoder = new TextDecoder();
  const buffer = { text: "" };
  let finalText = "";
  const emitEvents = async () => {
    for (const event of drainCodexSseEvents(buffer)) {
      const errorText = openRouterStreamErrorText(event);
      if (errorText) {
        throw new Error(errorText);
      }
      finalText += openRouterStreamDeltaText(event);
      if (finalText.trim()) {
        liveSink?.onText(finalText.trimEnd());
      }
      if (liveSink?.shouldStop?.(finalText)) {
        await reader.cancel().catch(() => undefined);
        return true;
      }
    }
    return false;
  };
  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }
    buffer.text += decoder.decode(value, { stream: true });
    if (await emitEvents()) {
      return finalText.trim();
    }
  }
  buffer.text += decoder.decode();
  buffer.text += "\n\n";
  await emitEvents();
  return finalText.trim();
}

async function runOpenRouterChatCompletion(
  profile: ProviderRuntimeProfile,
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId: string,
  moduleId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript,
  liveSink?: ProviderLiveTextSink
): Promise<string> {
  const model = selectedComposerModel(profile);
  const effort = normalizedReasoningEffort(selectedComposerReasoning(profile));
  if (!openRouterApiKey) {
    throw new Error("OpenRouter API key is not available.");
  }
  if (!model || /catalog unavailable|connect provider/i.test(model)) {
    throw new Error("OpenRouter model is not selected.");
  }
  const messages: OpenRouterMessage[] = [
    {
      role: "system",
      content: [
        "You are InGen's local assistant surface. Answer clearly in the user's language.",
        ingenProviderRuntimeContract({
          provider: "openrouter",
          moduleId,
          userText,
          transcript
        })
      ].join("\n")
    },
    ...recentConversationInput(userMessageId, transcript),
    {
      role: "user",
      content: await openRouterUserContent(userText, attachments)
    }
  ];
  const body: Record<string, unknown> = {
    model,
    messages
  };
  if (effort) {
    body.reasoning = {
      effort,
      exclude: true
    };
  }
  if (liveSink) {
    body.stream = true;
    const response = await net.fetch("https://openrouter.ai/api/v1/chat/completions", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${openRouterApiKey}`,
        "Content-Type": "application/json",
        "HTTP-Referer": "https://github.com/quentinhugoo-ui/Forge",
        "X-Title": "InGen Electron Shell"
      },
      body: JSON.stringify(body)
    });
    if (!response.ok) {
      const text = await response.text();
      throw new Error(text || `OpenRouter request failed with ${response.status}`);
    }
    const content = await readOpenRouterChatCompletionStream(response, liveSink);
    if (!content) {
      throw new Error("OpenRouter returned an empty assistant message.");
    }
    return content;
  }
  const parsed = await openRouterFetchJson("https://openrouter.ai/api/v1/chat/completions", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${openRouterApiKey}`,
      "Content-Type": "application/json",
      "HTTP-Referer": "https://github.com/quentinhugoo-ui/Forge",
      "X-Title": "InGen Electron Shell"
    },
    body: JSON.stringify(body)
  });
  const content = openRouterChatCompletionText(parsed);
  if (!content) {
    throw new Error("OpenRouter returned an empty assistant message.");
  }
  return content;
}

function modelNameFromError(message: string, fallbackModel: string): string {
  const quoted = message.match(/Model ['"]([^'"]+)['"] does not support image inputs/i);
  return quoted?.[1] ?? fallbackModel;
}

const ASSISTANT_PROVIDER_UNAVAILABLE_TEXT = "Provider unavailable. Check connection, auth, or quota.";
const AGENT_ACTION_RESULT_PREFIX = "AGENT_ACTION_RESULT v1";
const AGENT_ACTION_COMPAT_DETERMINISTIC_FALLBACK = process.env.INGEN_AGENT_ACTION_COMPAT_FALLBACK === "1";
const AGENT_ACTION_RESULT_ITEM_LIMIT = 10;
const AGENT_ACTION_RESULT_MATCH_LIMIT = 8;
const AGENT_ACTION_RESULT_PREVIEW_BYTES = 6_000;
const AGENT_ACTION_RESULT_MATCH_TEXT_BYTES = 320;
const AGENT_ACTION_CAPABILITY_BY_ACTION: Record<AgentActionRequest["action"], AgentActionCapabilityId> = {
  capabilities: "agent.capabilities",
  list: "fs.list",
  search: "fs.search",
  create_directory: "fs.create_directory",
  rename_path: "fs.rename",
  move_path: "fs.move",
  copy_path: "fs.copy",
  delete_empty_directory: "fs.delete_empty_directory",
  delete_tree: "fs.delete_tree",
  run_readonly_command: "shell.readonly",
  run_command: "shell.full",
  computer_inspect: "computer.inspect",
  computer_appshot: "computer.appshot",
  computer_focus_window: "computer.focus_window",
  computer_clipboard_read: "computer.clipboard_read",
  computer_clipboard_write: "computer.clipboard_write",
  computer_ui_tree: "computer.ui_tree",
  computer_ocr: "computer.ocr",
  computer_click: "computer.click",
  computer_type_text: "computer.type_text",
  computer_scroll: "computer.scroll",
  computer_drag: "computer.drag",
  browser_inspect_url: "browser.inspect_url",
  browser_download: "browser.download",
  browser_open_url: "browser.open_url",
  browser_playwright_inspect: "browser.playwright_inspect",
  browser_screenshot: "browser.screenshot",
  browser_click: "browser.click",
  browser_type_text: "browser.type_text",
  browser_playwright_download: "browser.playwright_download",
  document_inspect: "document.inspect",
  document_write_text: "document.write_text",
  document_write_json: "document.write_json",
  document_write_csv: "document.write_csv",
  document_convert_text: "document.convert_text",
  document_pdf_extract_text: "document.pdf_extract_text",
  document_office_inspect: "document.office_inspect",
  document_office_export_pdf: "document.office_export_pdf",
  document_image_ocr: "document.image_ocr",
  document_media_metadata: "document.media_metadata",
  document_toolchain_inspect: "document.toolchain_inspect",
  document_toolchain_install: "document.toolchain_install",
  dev_repo_status: "dev.repo_status",
  dev_git_diff: "dev.git_diff",
  dev_git_commit: "dev.git_commit",
  dev_git_push: "dev.git_push",
  dev_github_pr_create: "dev.github_pr_create",
  dev_github_pr_review_submit: "dev.github_pr_review_submit",
  dev_run_check: "dev.run_check",
  cloud_cli_inspect: "cloud.inspect",
  cloud_cli_run_readonly: "cloud.run_readonly",
  cloud_cli_run_write: "cloud.run_write",
  windows_setting_inspect: "windows.setting_inspect",
  windows_setting_apply: "windows.setting_apply",
  windows_sensitive_inspect: "windows.sensitive_inspect",
  windows_sensitive_apply: "windows.sensitive_apply",
  process_service_inspect: "windows.process_service_inspect",
  process_service_control: "windows.process_service_control",
  package_inspect: "windows.package_inspect",
  package_install_update: "windows.package_install_update",
  ci_checks_inspect: "ci.checks_inspect",
  ci_run_inspect: "ci.run_inspect",
  ci_rerun_failed: "ci.rerun_failed",
  virtualization_inspect: "virtualization.inspect",
  virtualization_run_command: "virtualization.run_command",
  automation_schedule: "automation.schedule",
  automation_list: "automation.list",
  automation_cancel: "automation.cancel",
  automation_record: "automation.record"
};

function friendlyAssistantErrorText(params: {
  userText: string;
  providerLabel: string;
  model: string;
  message: string;
  hasImageAttachment: boolean;
}): string {
  const { model, message, hasImageAttachment } = params;
  const unsupportedImageInput =
    hasImageAttachment &&
    /does not support image inputs|try again with a vision model|vision-enabled/i.test(message);
  if (unsupportedImageInput) {
    const failingModel = modelNameFromError(message, model);
    return `${failingModel || "Selected model"} cannot read images. Choose a vision model.`;
  }
  return ASSISTANT_PROVIDER_UNAVAILABLE_TEXT;
}

function agentRuntimeAgentName(): string {
  return brainIdentityContext.agentFirstName || "Agent";
}

function agentRuntimeProvider(): LlmProviderConnectId {
  return providerProfileFromComposer(panelsChatBottomState.selectedProvider).connectId;
}

function agentActionResultSummary(result: AgentActionResult): string {
  const charDelta = typeof result.value === "string" && /^chars \+\d+ -\d+$/.test(result.value) ? ` ${result.value}` : "";
  if (!result.accepted) {
    return `Result: action blocked - ${result.error?.message ?? "rejected by the action host"}.`;
  }
  if (result.items) {
    return `Result: ${result.items.length} item${result.items.length > 1 ? "s" : ""} listed${result.path ? ` in ${result.path}` : ""}.`;
  }
  if (result.matches) {
    return `Result: ${result.matches.length} match${result.matches.length > 1 ? "es" : ""} found.`;
  }
  if (result.commandLine) {
    return `Result: command completed with code ${result.exitCode ?? "unknown"}.`;
  }
  if (result.toPath) {
    return `Result: ${result.path ?? "path"} -> ${result.toPath}${charDelta}.`;
  }
  if (result.path) {
    return `Result: action applied on ${result.path}${charDelta}.`;
  }
  return `Result: action applied${charDelta}.`;
}

function agentRuntimeToolResultSummary(result: AgentActionResult): string {
  if (!result.accepted) {
    return `Action blocked: ${result.error?.message ?? "rejected by the action host"}.`;
  }
  if (result.items) {
    return `Listed ${result.items.length} item${result.items.length === 1 ? "" : "s"}${result.path ? ` in ${result.path}` : ""}.`;
  }
  if (result.matches) {
    return `Found ${result.matches.length} bounded match${result.matches.length === 1 ? "" : "es"}.`;
  }
  if (result.commandLine) {
    return `Command completed with exit code ${result.exitCode ?? "unknown"}.`;
  }
  if (result.toPath) {
    return `${result.path ?? "path"} -> ${result.toPath}.`;
  }
  if (result.path) {
    return `Action applied on ${result.path}.`;
  }
  return "Action applied.";
}

function agentRuntimeToolCallId(messageId: string, step: number, request: AgentActionRequest): string {
  return `agent-tool-${messageId}-${step}-${hashJson(request).slice(0, 10)}`;
}

function emitAgentRuntimeToolCallStarted(params: {
  agentEvents: AgentRuntimeEventQueue;
  messageId: string;
  step: number;
  request: AgentActionRequest;
}): NonNullable<AgentRuntimeEvent["toolCall"]> {
  const startedAt = Date.now();
  const toolCall: NonNullable<AgentRuntimeEvent["toolCall"]> = {
    id: agentRuntimeToolCallId(params.messageId, params.step, params.request),
    name: AGENT_ACTION_CAPABILITY_BY_ACTION[params.request.action],
    request: params.request,
    risk: params.request.action === "delete_empty_directory" || params.request.action === "delete_tree"
      ? "destructive"
      : params.request.action === "run_command" ||
        params.request.action === "virtualization_run_command" ||
        params.request.action === "cloud_cli_run_write" ||
        params.request.action === "windows_setting_apply" ||
        params.request.action === "windows_sensitive_apply" ||
        params.request.action === "process_service_control" ||
        params.request.action === "package_install_update" ||
        params.request.action === "document_toolchain_install" ||
        params.request.action === "ci_rerun_failed"
      ? "computer_write"
      : params.request.action === "computer_appshot" ||
        params.request.action === "computer_focus_window" ||
        params.request.action === "computer_clipboard_read" ||
        params.request.action === "computer_clipboard_write" ||
        params.request.action === "computer_ocr" ||
        params.request.action === "computer_click" ||
        params.request.action === "computer_type_text" ||
        params.request.action === "computer_scroll" ||
        params.request.action === "computer_drag" ||
        params.request.action === "browser_open_url" ||
        params.request.action === "browser_screenshot" ||
        params.request.action === "browser_click" ||
        params.request.action === "browser_type_text" ||
        params.request.action === "document_office_inspect" ||
        params.request.action === "document_office_export_pdf" ||
        params.request.action === "document_image_ocr" ||
        params.request.action === "dev_github_pr_review_submit"
      ? "external_ui"
      : params.request.action === "dev_git_push" ||
        params.request.action === "dev_github_pr_create"
      ? "external_ui"
      : params.request.action === "automation_schedule" ||
        params.request.action === "automation_cancel"
      ? "computer_write"
      : params.request.action === "browser_download" ||
        params.request.action === "browser_playwright_download"
      ? "computer_write"
      : params.request.action === "list" ||
        params.request.action === "search" ||
        params.request.action === "run_readonly_command" ||
        params.request.action === "computer_inspect" ||
        params.request.action === "computer_ui_tree" ||
        params.request.action === "browser_inspect_url" ||
        params.request.action === "browser_playwright_inspect" ||
        params.request.action === "document_inspect" ||
        params.request.action === "document_pdf_extract_text" ||
        params.request.action === "document_media_metadata" ||
        params.request.action === "document_toolchain_inspect" ||
        params.request.action === "dev_repo_status" ||
        params.request.action === "dev_git_diff" ||
        params.request.action === "cloud_cli_inspect" ||
        params.request.action === "cloud_cli_run_readonly" ||
        params.request.action === "windows_setting_inspect" ||
        params.request.action === "windows_sensitive_inspect" ||
        params.request.action === "process_service_inspect" ||
        params.request.action === "package_inspect" ||
        params.request.action === "ci_checks_inspect" ||
        params.request.action === "ci_run_inspect" ||
        params.request.action === "virtualization_inspect"
      ? "read"
      : "workspace_write",
    status: "pending",
    startedAt
  };
  params.agentEvents.emit({
    kind: "tool_call_started",
    messageId: params.messageId,
    toolCall
  });
  return toolCall;
}

function emitAgentRuntimeToolResult(params: {
  agentEvents: AgentRuntimeEventQueue;
  messageId: string;
  toolCall: NonNullable<AgentRuntimeEvent["toolCall"]>;
  result: AgentActionResult;
}): void {
  const toolResult: NonNullable<AgentRuntimeEvent["toolResult"]> = {
    accepted: params.result.accepted,
    action: params.result.action,
    summary: agentRuntimeToolResultSummary(params.result),
    path: params.result.path,
    toPath: params.result.toPath,
    itemCount: params.result.items?.length,
    matchCount: params.result.matches?.length,
    commandLine: params.result.commandLine,
    exitCode: params.result.exitCode,
    proofHash: params.result.proofHash,
    error: params.result.error?.message
  };
  params.agentEvents.emit({
    kind: "tool_result",
    messageId: params.messageId,
    toolCall: params.toolCall,
    toolResult
  });
  params.agentEvents.emit({
    kind: "tool_call_completed",
    messageId: params.messageId,
    toolCall: {
      ...params.toolCall,
      status: params.result.accepted ? "completed" : "failed",
      completedAt: Date.now()
    },
    toolResult
  });
}

function renderPendingAgentActionText(text: string, extracted: ExtractedAgentAction): string {
  const visibleText = removeAgentActionJsonFragment(text, extracted);
  const eventCommand = agentActionEventCommandForRequest(extracted.request);
  return `${visibleText ? `${visibleText}\n\n` : ""}${eventCommand}`.trim();
}

function renderCompletedPendingAgentActionText(text: string, result: AgentActionResult): string {
  return `${text.trimEnd()}\n\n${agentActionResultSummary(result)}`.trim();
}

function createAgentActionLoopState(objective: string): AgentActionLoopState {
  const state: AgentActionLoopState = {
    schema: "ingen.agent_action_loop.state.v1",
    objective,
    stepCount: 0,
    toolSteps: 0,
    retryCount: 0,
    approvals: [],
    observations: [],
    finalStatus: "running",
    proofHash: ""
  };
  state.proofHash = hashJson({ ...state, proofHash: "" });
  return state;
}

function agentActionResultNeedsApproval(result: AgentActionResult): boolean {
  return !result.accepted && /confirmed:true|approval|permission|confirmation|user confirmation|requires confirmation/i.test(result.error?.message ?? "");
}

function agentActionLoopOutcomeAfterFailure(state: AgentActionLoopState, result: AgentActionResult): AgentActionLoopOutcome {
  if (agentActionResultNeedsApproval(result)) {
    return "needs_approval";
  }
  return state.retryCount > 0 ? "failed_after_retries" : "blocked";
}

function agentActionLoopWithResult(
  state: AgentActionLoopState,
  request: AgentActionRequest,
  result: AgentActionResult,
  step: number
): AgentActionLoopState {
  const capabilityId = AGENT_ACTION_CAPABILITY_BY_ACTION[request.action];
  const approvals = agentActionResultNeedsApproval(result)
    ? [...new Set([...state.approvals, capabilityId])]
    : state.approvals;
  const nextState: AgentActionLoopState = {
    ...state,
    stepCount: step + 1,
    toolSteps: state.toolSteps + 1,
    retryCount: result.accepted ? state.retryCount : state.retryCount + 1,
    approvals,
    observations: [
      ...state.observations,
      {
        step: step + 1,
        capabilityId,
        request,
        accepted: result.accepted,
        resultProofHash: result.proofHash,
        summary: agentRuntimeToolResultSummary(result),
        error: result.error?.message
      }
    ],
    lastResult: result,
    finalStatus: "running",
    proofHash: ""
  };
  nextState.proofHash = hashJson({ ...nextState, proofHash: "" });
  return nextState;
}

function agentActionLoopWithStatus(state: AgentActionLoopState, finalStatus: AgentActionLoopOutcome): AgentActionLoopState {
  const nextState: AgentActionLoopState = {
    ...state,
    finalStatus,
    proofHash: ""
  };
  nextState.proofHash = hashJson({ ...nextState, proofHash: "" });
  return nextState;
}

function reportableAgentActionLoopOutcome(
  state: AgentActionLoopState,
  fallback: Exclude<AgentActionLoopOutcome, "running" | "cancelled">
): Exclude<AgentActionLoopOutcome, "running" | "cancelled"> {
  return state.finalStatus === "running" || state.finalStatus === "cancelled" ? fallback : state.finalStatus;
}

function textHasAgentActionFinalSummary(text: string): boolean {
  return /\b(?:Final summary|Conclusion)\s*:/i.test(text);
}

function agentActionLoopFinalSummaryText(state: AgentActionLoopState): string {
  const lastObservation = state.observations.at(-1);
  const approvals = state.approvals.length > 0 ? ` Approval needed for: ${state.approvals.join(", ")}.` : "";
  const last = lastObservation ? ` Last result: ${lastObservation.summary}` : "";
  return `Final summary: agent loop ${state.finalStatus}; tool steps=${state.toolSteps}; retries=${state.retryCount}.${last}${approvals}`;
}

function ensureAgentActionLoopFinalSummary(message: TranscriptMessage, state: AgentActionLoopState): TranscriptMessage {
  if (state.toolSteps === 0 || extractAgentActionJsonRequest(message.text) || textHasAgentActionFinalSummary(message.text)) {
    return message;
  }
  if (state.finalStatus === "completed") {
    return message;
  }
  const text = `${removeAgentActionJsonFragments(message.text).trimEnd()}\n\n${agentActionLoopFinalSummaryText(state)}`.trim();
  return {
    ...message,
    text,
    proofHash: hashJson({ agentActionLoopFinalSummary: true, previousProofHash: message.proofHash, state })
  };
}

function compactAgentActionItems(result: AgentActionResult): AgentActionResult["items"] {
  return result.items
    ?.slice(0, AGENT_ACTION_RESULT_ITEM_LIMIT)
    .map((item) => ({
      name: item.name,
      path: item.path,
      kind: item.kind
    }));
}

function compactAgentActionMatches(result: AgentActionResult): AgentActionResult["matches"] {
  return result.matches
    ?.slice(0, AGENT_ACTION_RESULT_MATCH_LIMIT)
    .map((match) => ({
      path: match.path,
      line: match.line,
      text: trimUtf8Bytes(match.text, AGENT_ACTION_RESULT_MATCH_TEXT_BYTES)
    }));
}

function compactAgentActionResult(result: AgentActionResult): string {
  return JSON.stringify({
    accepted: result.accepted,
    action: result.action,
    path: result.path,
    toPath: result.toPath,
    itemCount: result.items?.length,
    items: compactAgentActionItems(result),
    omittedItems: Math.max(0, (result.items?.length ?? 0) - AGENT_ACTION_RESULT_ITEM_LIMIT),
    matchCount: result.matches?.length,
    matches: compactAgentActionMatches(result),
    omittedMatches: Math.max(0, (result.matches?.length ?? 0) - AGENT_ACTION_RESULT_MATCH_LIMIT),
    commandLine: result.commandLine,
    executionAdapter: result.executionAdapter,
    routeId: result.routeId,
    exitCode: result.exitCode,
    durationMs: result.durationMs,
    timeoutMs: result.timeoutMs,
    timedOut: result.timedOut,
    stdoutPreview: result.stdoutPreview ? trimUtf8Bytes(result.stdoutPreview, AGENT_ACTION_RESULT_PREVIEW_BYTES) : undefined,
    stderrPreview: result.stderrPreview ? trimUtf8Bytes(result.stderrPreview, AGENT_ACTION_RESULT_PREVIEW_BYTES) : undefined,
    artifacts: result.artifacts,
    observedChanges: result.observedChanges,
    computerUse: result.computerUse
      ? {
          action: result.computerUse.action,
          displays: result.computerUse.displays.slice(0, 4),
          windows: result.computerUse.windows.slice(0, 12),
          accessibilityTreeStatus: result.computerUse.accessibilityTreeStatus,
          ocrStatus: result.computerUse.ocrStatus,
          accessibilityTreeNodes: result.computerUse.accessibilityTree?.length,
          ocrTextChars: result.computerUse.ocrText?.length,
          cursor: result.computerUse.cursor,
          forbiddenPromptDetected: result.computerUse.forbiddenPromptDetected,
          proofHash: result.computerUse.proofHash
        }
      : undefined,
    appshot: result.appshot,
    browserPage: result.browserPage
      ? {
          action: result.browserPage.action,
          url: result.browserPage.url,
          finalUrl: result.browserPage.finalUrl,
          statusCode: result.browserPage.statusCode,
          ok: result.browserPage.ok,
          title: result.browserPage.title,
          linkCount: result.browserPage.linkCount,
          formCount: result.browserPage.formCount,
          downloadCandidateCount: result.browserPage.downloadCandidateCount,
          domNodeCount: result.browserPage.domNodeCount,
          selector: result.browserPage.selector,
          selectorMatched: result.browserPage.selectorMatched,
          networkCount: result.browserPage.network?.length,
          screenshotStatus: result.browserPage.screenshotStatus,
          domStatus: result.browserPage.domStatus,
          networkLogStatus: result.browserPage.networkLogStatus,
          proofHash: result.browserPage.proofHash
        }
      : undefined,
    browserScreenshot: result.browserScreenshot,
    download: result.download,
    documentMedia: result.documentMedia,
    developer: result.developer,
    automation: result.automation,
    userPresenceRequired: result.userPresenceRequired,
    verification: result.verification
      ? {
          passed: result.verification.passed,
          probes: result.verification.probes.map((probe) => ({
            id: probe.id,
            kind: probe.kind,
            target: probe.target,
            expectation: probe.expectation,
            actual: trimUtf8Bytes(probe.actual, 600),
            passed: probe.passed
          })),
          proofHash: result.verification.proofHash
        }
      : undefined,
    failureCategory: result.failureCategory,
    retryRoutes: result.retryRoutes,
    value: result.value,
    error: result.error?.message,
    proofHash: result.proofHash
  });
}

function agentActionSelectedCapabilityContext(request: AgentActionRequest, result: AgentActionResult): string {
  const manifest = createAgentActionHostManifest(agentActionHostConfig());
  const capabilityId = AGENT_ACTION_CAPABILITY_BY_ACTION[request.action];
  const recoveryFamilies = manifest.runtime.approvalGatedFamilies
    .filter((family) => family !== capabilityId)
    .slice(0, 10)
    .join("|");
  return [
    agentActionCapabilityDetailManifest(agentActionHostConfig(), capabilityId),
    result.accepted
      ? "next=verify whether the user objective is now satisfied; continue only if another local action is still required."
      : `next=read the error, then choose a different safe route when possible. recovery_families=${recoveryFamilies}`
  ].join("\n");
}

function emitAgentLoopDiagnosticSummary(params: {
  agentEvents: AgentRuntimeEventQueue;
  messageId: string;
  outcome: Exclude<AgentActionLoopOutcome, "running" | "cancelled">;
  toolSteps: number;
  loopState?: AgentActionLoopState;
}): void {
  params.agentEvents.emit({
    kind: "final_summary",
    messageId: params.messageId,
    summary: `Agent loop ${params.outcome}; tool_steps=${params.toolSteps}${params.loopState ? `; proof=${params.loopState.proofHash}` : ""}`
  });
}

function agentActionRequestIsDiscovery(request: AgentActionRequest): boolean {
  return (
    request.action === "capabilities" ||
    request.action === "list" ||
    request.action === "search" ||
    request.action === "run_readonly_command" ||
    request.action === "computer_inspect" ||
    request.action === "computer_ui_tree" ||
    request.action === "browser_inspect_url" ||
    request.action === "browser_playwright_inspect" ||
    request.action === "document_inspect" ||
    request.action === "document_toolchain_inspect" ||
    request.action === "dev_repo_status" ||
    request.action === "dev_git_diff" ||
    request.action === "cloud_cli_inspect" ||
    request.action === "cloud_cli_run_readonly" ||
    request.action === "windows_setting_inspect" ||
    request.action === "windows_sensitive_inspect" ||
    request.action === "process_service_inspect" ||
    request.action === "package_inspect" ||
    request.action === "ci_checks_inspect" ||
    request.action === "ci_run_inspect" ||
    request.action === "virtualization_inspect" ||
    request.action === "automation_list"
  );
}

function textLooksLikeFilesystemMutationGoal(text: string): boolean {
  return /\b(organis|organize|ranger|range|classer|trier|tri|dossier|folder|deplacer|déplacer|move|renommer|rename|copier|copy|supprimer|delete|nettoyer|clean|bureau|desktop)\b/i.test(text);
}

function agentActionStepNeedsMutationFollowUp(originalUserText: string, request: AgentActionRequest, result: AgentActionResult): boolean {
  if (!result.accepted || !agentActionRequestIsDiscovery(request)) {
    return false;
  }
  if (textLooksLikeVisualCodingArtifactGoal(originalUserText)) {
    return true;
  }
  if (!textLooksLikeFilesystemMutationGoal(originalUserText)) {
    return false;
  }
  return Boolean(result.items?.length || result.matches?.length || result.stdoutPreview?.trim());
}

function agentActionLastResultIsDocumentWrite(state: AgentActionLoopState): boolean {
  return state.lastResult?.accepted === true && state.lastResult.action === "document_write_text";
}

function shouldForceCodingLivePreviewAfterWrite(originalUserText: string, assistantText: string, state: AgentActionLoopState): boolean {
  return (
    textLooksLikeVisualCodingArtifactGoal(originalUserText) &&
    agentActionLastResultIsDocumentWrite(state) &&
    !assistantText.includes(BRAIN_CODING_LIVE_PREVIEW_COMMAND)
  );
}

function agentActionShouldRunFileOrganizationFallback(originalUserText: string, request: AgentActionRequest, result: AgentActionResult): boolean {
  return agentActionStepNeedsMutationFollowUp(originalUserText, request, result) && agentActionFileOrganizationRequests(originalUserText, request, result).length > 0;
}

function agentLoopNarrationContractManifest(): string {
  return [
    "LOOP_STREAM_NARRATION_CONTRACT v1",
    "ACTION_NARRATION_CONTRACT v1",
    "voice_source=Follow BRAIN_PERSONALITY_MANIFEST when choosing warmth, rhythm, phrasing and how much human presence to show.",
    "visible_language=English; write like a capable desktop coding agent explaining work in progress, not like a status logger.",
    "visible_step_shape=Each visible paragraph should make the step understandable: what was just learned, why the next move follows, and what action or verification comes next.",
    "paragraph_size=Use one compact paragraph of 1-3 sentences before an event or action. Avoid long checklists during the loop.",
    "pedagogy=Name pivots and tradeoffs plainly: if a tool fails, explain the useful signal from the failure and the safer next route.",
    "evidence=After a tool result, mention the runtime proof that matters: accepted result, path, count, exit code, artifact, verification or blocked reason.",
    "event_coupling=When you say an action will be taken, immediately follow with the matching CodeAct or AGENT_ACTION_JSON control line. Do not promise work without an event.",
    "tone=Concrete, calm, direct. Avoid mechanical labels such as 'Step 1', 'OBLIGATION', 'Forbidden', or repeating 'I will' at every paragraph.",
    "completion=When no further loop action is needed, stop the loop with a short natural summary of what was actually proven or what remains blocked."
  ].join("\n");
}

function agentActionLoopContinuationUserText(originalUserText: string, request: AgentActionRequest, result: AgentActionResult, step: number): string {
  const mustContinueAfterDiscovery = agentActionStepNeedsMutationFollowUp(originalUserText, request, result);
  const mustCreateVisualArtifact = mustContinueAfterDiscovery && textLooksLikeVisualCodingArtifactGoal(originalUserText);
  return [
    AGENT_ACTION_RESULT_PREFIX,
    `step=${step + 1}`,
    `request=${JSON.stringify(request)}`,
    `result=${compactAgentActionResult(result)}`,
    agentActionSelectedCapabilityContext(request, result),
    "",
    agentLoopNarrationContractManifest(),
    "",
    "Continue the universal observe-act-verify-retry loop.",
    mustContinueAfterDiscovery
      ? "runtime_next=The last result was discovery only, while the user objective still requires a local change. Write a clear visible paragraph explaining that evidence, then emit exactly one concrete AGENT_ACTION_JSON line."
      : "runtime_next=If another local action is still required, write a clear visible paragraph explaining why, then emit exactly one AGENT_ACTION_JSON line.",
    mustCreateVisualArtifact
      ? "runtime_requirement=The next local action must create or update the real visual artifact file before any preview is opened."
      : "",
    "control_line=AGENT_ACTION_JSON must start at column 1 after the visible paragraph.",
    mustContinueAfterDiscovery
      ? "stop_policy=Do not stop with only a verbal plan when a concrete local action is still required."
      : "stop_policy=If the objective is already satisfied, write the natural summary and do not emit AGENT_ACTION_JSON.",
    "",
    `Initial user objective:\n${originalUserText}`
  ].join("\n");
}

function agentActionForcedContinuationUserText(originalUserText: string, request: AgentActionRequest, result: AgentActionResult, previousAssistantText: string, step: number): string {
  const mustCreateVisualArtifact = textLooksLikeVisualCodingArtifactGoal(originalUserText);
  return [
    "AGENT_ACTION_FORCED_CONTINUATION v1",
    `step=${step + 1}`,
    `last_request=${JSON.stringify(request)}`,
    `last_result=${compactAgentActionResult(result)}`,
    agentActionSelectedCapabilityContext(request, result),
    "",
    agentLoopNarrationContractManifest(),
    "",
    "runtime_next=The loop must continue because the last action only discovered state and did not complete the required change.",
    "Write a visible paragraph that explains the evidence and the next concrete move, then exactly one AGENT_ACTION_JSON line.",
    mustCreateVisualArtifact
      ? "runtime_requirement=Create or update the real local visual artifact file before concluding or opening preview."
      : "runtime_requirement=Choose the next concrete local action from the observed state; do not add a domain-specific hardcoded workflow.",
    "control_line=AGENT_ACTION_JSON must start at column 1 after the visible paragraph. Do not write a final summary in this continuation.",
    "",
    `Last assistant response without action:\n${trimUtf8Bytes(previousAssistantText, 2000)}`,
    "",
    `Initial user objective:\n${originalUserText}`
  ].join("\n");
}

function agentActionCodingLivePreviewForcedContinuationUserText(originalUserText: string, state: AgentActionLoopState, previousAssistantText: string): string {
  const lastPath = state.lastResult?.path || state.lastResult?.toPath || "";
  return [
    "CODING_VISUAL_PREVIEW_FORCED_CONTINUATION v1",
    "",
    agentLoopNarrationContractManifest(),
    "",
    "runtime_next=The local visual file exists, but the live preview has not been opened yet. Explain that proof and open the preview as the next event.",
    `control_line=After the visible paragraph, emit ${BRAIN_CODING_LIVE_PREVIEW_COMMAND} with the verified absolute path and kind="html" when appropriate.`,
    lastPath ? `verified_path=${JSON.stringify(lastPath)}` : "",
    "stop_policy=Do not claim the preview is open without the preview CodeAct.",
    "",
    `Last assistant response without preview:\n${trimUtf8Bytes(previousAssistantText, 2000)}`,
    "",
    `Initial user objective:\n${originalUserText}`
  ].filter(Boolean).join("\n");
}

function agentActionInitialCodingVisualForcedContinuationUserText(originalUserText: string, previousAssistantText: string): string {
  return [
    "CODING_VISUAL_ACTION_FORCED_CONTINUATION v1",
    "",
    agentLoopNarrationContractManifest(),
    "",
    "runtime_next=The previous answer treated a visual coding request as a snippet, but this desktop loop needs a real local artifact. Explain that pivot clearly, then create or update the file.",
    "control_line=After the visible paragraph, emit exactly one AGENT_ACTION_JSON line that creates or modifies the local file. Do not claim file proof before AGENT_ACTION_RESULT.",
    "",
    `Last assistant response to correct:\n${trimUtf8Bytes(previousAssistantText, 2000)}`,
    "",
    `Initial user objective:\n${originalUserText}`
  ].join("\n");
}

function agentActionFailureContinuationUserText(originalUserText: string, request: AgentActionRequest, result: AgentActionResult, step: number): string {
  return [
    "AGENT_ACTION_FAILURE_CONTINUATION v1",
    `step=${step + 1}`,
    `failed_request=${JSON.stringify(request)}`,
    `failed_result=${compactAgentActionResult(result)}`,
    agentActionSelectedCapabilityContext(request, result),
    "",
    agentLoopNarrationContractManifest(),
    "",
    "runtime_next=The last local action failed or was refused. Explain the useful signal in the error, then choose a safer available route if one exists.",
    "blocked_policy=If the error proves a protected root, dangerous action, missing user permission or impossible operation, state the block clearly and do not emit AGENT_ACTION_JSON.",
    "control_line=If a safe alternative exists, emit exactly one AGENT_ACTION_JSON line after the visible paragraph. Never claim success without an accepted AGENT_ACTION_RESULT.",
    "",
    `Initial user objective:\n${originalUserText}`
  ].join("\n");
}

const AGENT_ACTION_ORGANIZE_CATEGORY_NAMES = new Set(["Documents", "Images", "Videos", "Audio", "Archives", "Code", "Applications", "Folders", "Other"]);
const AGENT_ACTION_VISIBLE_SHORTCUT_EXTENSIONS = new Set([".exe", ".msi", ".lnk", ".url", ".appref-ms"]);
const AGENT_ACTION_IGNORED_SYSTEM_FILENAMES = new Set(["desktop.ini", "thumbs.db"]);
const AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME = "Sorted";
const AGENT_ACTION_DETERMINISTIC_FALLBACK_MAX_REQUESTS = 48;
const AGENT_ACTION_DETERMINISTIC_STEP_DELAY_MS = 280;

function agentActionOrganizeCategory(item: AgentActionPathEntry): string {
  if (item.kind === "directory") {
    return "Folders";
  }
  const extension = extname(item.name).toLowerCase();
  if ([".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp", ".tiff"].includes(extension)) return "Images";
  if ([".mp4", ".mov", ".avi", ".mkv", ".webm", ".m4v"].includes(extension)) return "Videos";
  if ([".mp3", ".wav", ".flac", ".aac", ".m4a", ".ogg"].includes(extension)) return "Audio";
  if ([".zip", ".rar", ".7z", ".tar", ".gz"].includes(extension)) return "Archives";
  if ([".js", ".ts", ".tsx", ".jsx", ".py", ".rs", ".go", ".java", ".cpp", ".c", ".h", ".json", ".toml", ".yaml", ".yml", ".html", ".css"].includes(extension)) return "Code";
  if ([".exe", ".msi", ".app", ".bat", ".cmd", ".ps1"].includes(extension)) return "Applications";
  if ([".pdf", ".doc", ".docx", ".txt", ".md", ".rtf", ".xls", ".xlsx", ".ppt", ".pptx", ".csv"].includes(extension)) return "Documents";
  return "Other";
}

function agentActionShouldLeaveLegacyPathInPlace(item: AgentActionPathEntry): boolean {
  if (item.kind !== "file") {
    return true;
  }
  return AGENT_ACTION_VISIBLE_SHORTCUT_EXTENSIONS.has(extname(item.name).toLowerCase());
}

function agentActionPathInBase(basePath: string, ...parts: string[]): string {
  if (!basePath || basePath === ".") {
    return join(...parts);
  }
  return join(basePath, ...parts);
}

function deterministicOrganizationRequestsFromList(request: AgentActionRequest, result: AgentActionResult): AgentActionRequest[] {
  if (!result.accepted || request.action !== "list" || !result.items?.length) {
    return [];
  }
  const existingDirectories = new Set(
    result.items
      .filter((item) => item.kind === "directory")
      .map((item) => item.name.toLowerCase())
  );
  const movableItems = result.items
    .filter((item) => !item.name.startsWith("."))
    .filter((item) => !agentActionShouldLeaveLegacyPathInPlace(item))
    .filter((item) => !(item.kind === "directory" && AGENT_ACTION_ORGANIZE_CATEGORY_NAMES.has(item.name)));
  const requests: AgentActionRequest[] = [];
  const plannedDirectories = new Set(existingDirectories);
  const basePath = result.path ?? request.path ?? ".";
  for (const item of movableItems.slice(0, 12)) {
    const category = agentActionOrganizeCategory(item);
    if (!plannedDirectories.has(category.toLowerCase())) {
      requests.push({
        action: "create_directory",
        scope: request.scope,
        path: agentActionPathInBase(basePath, category),
        confirmed: request.scope === "computer" ? true : undefined
      });
      plannedDirectories.add(category.toLowerCase());
    }
    requests.push({
      action: "move_path",
      scope: request.scope,
      path: item.path,
      toPath: agentActionPathInBase(basePath, category, item.name),
      confirmed: request.scope === "computer" ? true : undefined
    });
  }
  return requests.slice(0, 8);
}

function deterministicOrganizationProgressText(request: AgentActionRequest, index: number, total: number): string {
  const position = total > 1 ? ` (${index + 1}/${total})` : "";
  const target = request.toPath ?? request.path ?? "the selected path";
  switch (request.action) {
    case "create_directory":
      return `Category ready${position}: checking the folder ${target} before moving items.`;
    case "move_path":
      return `Item sorted${position}: ${request.path ?? "an item"} is moving to ${target}.`;
    case "copy_path":
      return `Bounded copy${position}: ${request.path ?? "an item"} is being duplicated to ${target}.`;
    case "rename_path":
      return `Name clarified${position}: ${request.path ?? "an item"} becomes ${target}.`;
    default:
      return `Local action${position}: ${AGENT_ACTION_CAPABILITY_BY_ACTION[request.action]} is running now.`;
  }
}

function agentActionUserWantsCollectionRoot(text: string): boolean {
  return /\b(dossier\s+principal|principal\s+folder|ultra\s+(?:propre|epur|epure|clean)|tout\s+regrouper|tous\s+les\s+dossiers|reunis|r[eé]unis|group[eé]s?)\b/i.test(text);
}

function agentActionDisplayName(value?: string): string {
  const fileName = (value ?? "").replace(/[\\/]+$/g, "").split(/[\\/]/).filter(Boolean).at(-1) ?? "";
  if (!fileName) {
    return "this item";
  }
  return fileName.length > 54 ? `${fileName.slice(0, 43)}...${fileName.slice(-8)}` : fileName;
}

function agentActionDestinationFolderName(value?: string): string {
  const parent = value ? dirname(value) : "";
  const folder = agentActionDisplayName(parent);
  return folder === "this item" ? "its target folder" : folder;
}

function agentActionShouldLeavePathInPlace(item: AgentActionPathEntry, moveDirectories: boolean): boolean {
  const normalizedName = item.name.trim().toLowerCase();
  if (AGENT_ACTION_IGNORED_SYSTEM_FILENAMES.has(normalizedName)) {
    return true;
  }
  if (item.kind === "directory") {
    return !moveDirectories || normalizedName === AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME.toLowerCase();
  }
  return AGENT_ACTION_VISIBLE_SHORTCUT_EXTENSIONS.has(extname(item.name).toLowerCase());
}

function waitAgentActionVisualStep(): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, AGENT_ACTION_DETERMINISTIC_STEP_DELAY_MS));
}

function agentActionFileOrganizationRequests(originalUserText: string, request: AgentActionRequest, result: AgentActionResult): AgentActionRequest[] {
  if (!result.accepted || request.action !== "list" || !result.items?.length) {
    return [];
  }
  const wantsPrincipalFolder = agentActionUserWantsCollectionRoot(originalUserText);
  const existingDirectories = new Set(
    result.items
      .filter((item) => item.kind === "directory")
      .map((item) => item.name.toLowerCase())
  );
  const basePath = result.path ?? request.path ?? ".";
  const organizationRoot = wantsPrincipalFolder ? agentActionPathInBase(basePath, AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME) : basePath;
  const movableItems = result.items
    .filter((item) => !item.name.startsWith("."))
    .filter((item) => !agentActionShouldLeavePathInPlace(item, wantsPrincipalFolder))
    .filter((item) => wantsPrincipalFolder || !(item.kind === "directory" && AGENT_ACTION_ORGANIZE_CATEGORY_NAMES.has(item.name)));
  const requests: AgentActionRequest[] = [];
  const plannedDirectories = new Set(existingDirectories);
  if (wantsPrincipalFolder && !plannedDirectories.has(AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME.toLowerCase())) {
    requests.push({
      action: "create_directory",
      scope: request.scope,
      path: organizationRoot,
      confirmed: request.scope === "computer" ? true : undefined
    });
    plannedDirectories.add(AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME.toLowerCase());
  }
  for (const item of movableItems.slice(0, 24)) {
    const category = agentActionOrganizeCategory(item);
    const categoryPath = agentActionPathInBase(organizationRoot, category);
    const plannedKey = `${wantsPrincipalFolder ? AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME : ""}/${category}`.toLowerCase();
    if (!plannedDirectories.has(plannedKey)) {
      requests.push({
        action: "create_directory",
        scope: request.scope,
        path: categoryPath,
        confirmed: request.scope === "computer" ? true : undefined
      });
      plannedDirectories.add(plannedKey);
    }
    requests.push({
      action: "move_path",
      scope: request.scope,
      path: item.path,
      toPath: agentActionPathInBase(categoryPath, item.name),
      confirmed: request.scope === "computer" ? true : undefined
    });
  }
  return requests.slice(0, AGENT_ACTION_DETERMINISTIC_FALLBACK_MAX_REQUESTS);
}

function agentActionFileOrganizationProgressText(request: AgentActionRequest, index: number, total: number): string {
  const sourceName = agentActionDisplayName(request.path);
  const targetName = agentActionDisplayName(request.toPath ?? request.path);
  const targetFolder = agentActionDestinationFolderName(request.toPath);
  const opening =
    index === 0
      ? "I am starting the sorting pass with the most obvious items."
      : index % 3 === 0
        ? "The desktop is getting simpler through small verifiable decisions."
        : index % 3 === 1
          ? "Keeping the rhythm: one clear action, then verification."
          : "I am continuing without touching visible shortcuts or applications.";
  const remaining = total > index + 1 ? "I will continue with the next useful item." : "I will run one final result check after this.";
  switch (request.action) {
    case "create_directory":
      return `${opening} I am preparing the folder ${targetName}; it will be a clean destination instead of stacking files in the same place.`;
    case "move_path":
      return `${opening} I am moving ${sourceName} to ${targetFolder} because its category is clear. ${remaining}`;
    case "copy_path":
      return `${opening} I am copying ${sourceName} to ${targetName} while keeping the original intact, then checking that the copy exists.`;
    case "rename_path":
      return `${opening} I am renaming ${sourceName} to ${targetName}; the goal is to make the item recognizable without changing its contents.`;
    default:
      return `${opening} I am applying the required local action, then checking the result before continuing.`;
  }
}

function agentActionFileOrganizationFinalText(results: AgentActionResult[]): string {
  const accepted = results.filter((result) => result.accepted);
  const failed = results.find((result) => !result.accepted);
  const created = accepted.filter((result) => result.action === "create_directory").length;
  const moved = accepted.filter((result) => result.action === "move_path").length;
  const parts = [
    created ? `${created} folder${created > 1 ? "s" : ""} prepared` : "",
    moved ? `${moved} item${moved > 1 ? "s" : ""} moved` : ""
  ].filter(Boolean);
  if (parts.length === 0) {
    return `Conclusion: no local changes were applied.${failed ? ` Blocker: ${failed.error?.message ?? "action refused by the host"}.` : ""}`;
  }
  return `Conclusion: ${parts.join(", ")}.${failed ? ` The sequence stopped on a blocker: ${failed.error?.message ?? "action refused by the host"}.` : ""}`;
}

function deterministicOrganizationFinalText(results: AgentActionResult[]): string {
  const accepted = results.filter((result) => result.accepted);
  const failed = results.find((result) => !result.accepted);
  const created = accepted.filter((result) => result.action === "create_directory").length;
  const moved = accepted.filter((result) => result.action === "move_path").length;
  const copied = accepted.filter((result) => result.action === "copy_path").length;
  const renamed = accepted.filter((result) => result.action === "rename_path").length;
  const parts = [
    created ? `${created} folder${created > 1 ? "s" : ""} ready` : "",
    moved ? `${moved} item${moved > 1 ? "s" : ""} moved` : "",
    copied ? `${copied} item${copied > 1 ? "s" : ""} copied` : "",
    renamed ? `${renamed} item${renamed > 1 ? "s" : ""} renamed` : ""
  ].filter(Boolean);
  if (parts.length === 0) {
    return `Conclusion: no local changes were applied.${failed ? ` Blocker: ${failed.error?.message ?? "action refused by the host"}.` : ""}`;
  }
  return `Conclusion: ${parts.join(", ")}.${failed ? ` The sequence stopped on a blocker: ${failed.error?.message ?? "action refused by the host"}.` : ""}`;
}

async function applyDeterministicOrganizationFallback(params: {
  assistantMessage: TranscriptMessage;
  originalUserText: string;
  request: AgentActionRequest;
  result: AgentActionResult;
  agentEvents: AgentRuntimeEventQueue;
  assistantRun?: AssistantRunControl;
  commitProgress?: (assistantMessage: TranscriptMessage) => void;
}): Promise<TranscriptMessage> {
  throwIfAssistantRunCancelled(params.assistantRun);
  if (!agentActionStepNeedsMutationFollowUp(params.originalUserText, params.request, params.result)) {
    return params.assistantMessage;
  }
  const requests = agentActionFileOrganizationRequests(params.originalUserText, params.request, params.result);
  if (requests.length === 0) {
    const assistantMessage = {
      ...params.assistantMessage,
      text: `${params.assistantMessage.text.trimEnd()}\n\nConclusion: no non-application item was moved; visible apps, shortcuts, and folders remain in place.`.trim(),
      proofHash: hashJson({ deterministicAgentActionFallbackNoop: true, previousProofHash: params.assistantMessage.proofHash })
    };
    assistantMessage.text = `${params.assistantMessage.text.trimEnd()}\n\nConclusion: no file to sort was found. App shortcuts and visible system files remain in place.`.trim();
    params.commitProgress?.(assistantMessage);
    return assistantMessage;
  }
  let assistantMessage = {
    ...params.assistantMessage,
    text: [
      params.assistantMessage.text,
      "The list is sufficient for a first bounded sorting pass: I am creating the necessary category folders, then moving a few obvious items without deleting anything."
    ].filter((part) => part.trim().length > 0).join("\n\n")
  };
  assistantMessage = {
    ...assistantMessage,
    text: [
      params.assistantMessage.text,
      agentActionUserWantsCollectionRoot(params.originalUserText)
        ? "I am moving to a cleaner organization: I am creating a main desktop folder, then sorting files and folders into it by category."
        : "I have enough items to start: I am creating only the useful folders, then sorting the obvious files without deleting anything."
    ].filter((part) => part.trim().length > 0).join("\n\n")
  };
  const results: AgentActionResult[] = [];
  params.commitProgress?.(assistantMessage);
  for (const [index, request] of requests.entries()) {
    throwIfAssistantRunCancelled(params.assistantRun);
    const toolCall = emitAgentRuntimeToolCallStarted({
      agentEvents: params.agentEvents,
      messageId: params.assistantMessage.id,
      step: index + 1,
      request
    });
    assistantMessage = {
      ...assistantMessage,
      text: [
        assistantMessage.text,
        agentActionFileOrganizationProgressText(request, index, requests.length),
        agentActionEventCommandForRequest(request)
      ].filter((part) => part.trim().length > 0).join("\n\n"),
      proofHash: hashJson({ deterministicAgentActionFallbackPending: true, previousProofHash: assistantMessage.proofHash, request })
    };
    params.commitProgress?.(assistantMessage);
    await waitAgentActionVisualStep();
    throwIfAssistantRunCancelled(params.assistantRun);
    const result = await executeTrackedAgentAction(request);
    throwIfAssistantRunCancelled(params.assistantRun);
    emitAgentRuntimeToolResult({
      agentEvents: params.agentEvents,
      messageId: params.assistantMessage.id,
      toolCall,
      result
    });
    results.push(result);
    assistantMessage = {
      ...assistantMessage,
      text: renderCompletedPendingAgentActionText(assistantMessage.text, result),
      proofHash: hashJson({ deterministicAgentActionFallback: true, previousProofHash: assistantMessage.proofHash, request, result })
    };
    params.commitProgress?.(assistantMessage);
    await waitAgentActionVisualStep();
    if (!result.accepted) {
      break;
    }
  }
  assistantMessage = {
    ...assistantMessage,
    text: `${assistantMessage.text.trimEnd()}\n\n${agentActionFileOrganizationFinalText(results)}`.trim(),
    proofHash: hashJson({ deterministicAgentActionFallbackFinal: true, previousProofHash: assistantMessage.proofHash, results })
  };
  params.commitProgress?.(assistantMessage);
  return assistantMessage;
}

async function executeAssistantAgentActionLoop(params: {
  assistantMessage: TranscriptMessage;
  baseTranscript: TranscriptMessage[];
  originalUserText: string;
  providerAttachments: ProviderAttachment[];
  userMessageId: string;
  moduleId: string;
  agentEvents: AgentRuntimeEventQueue;
  assistantRun?: AssistantRunControl;
  commitTranscript: (transcript: TranscriptMessage[]) => void;
}): Promise<TranscriptMessage> {
  let assistantMessage = params.assistantMessage;
  let loopState = createAgentActionLoopState(params.originalUserText);
  let step = 0;
  let initialCodingVisualActionForced = false;
  let codingLivePreviewForced = false;
  while (true) {
    throwIfAssistantRunCancelled(params.assistantRun);
    const extracted = extractAgentActionJsonRequest(assistantMessage.text);
    if (!extracted) {
      const sanitizedText = removeAgentActionJsonFragments(assistantMessage.text).trim();
      if (sanitizedText && sanitizedText !== assistantMessage.text.trim()) {
        assistantMessage = {
          ...assistantMessage,
          text: sanitizedText,
          proofHash: hashJson({
            agentActionLoopRemovedInvalidControlJson: true,
            previousProofHash: assistantMessage.proofHash,
            text: sanitizedText
          })
        };
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
      }
      if (!initialCodingVisualActionForced && shouldForceInitialCodingVisualAction(params.originalUserText, assistantMessage.text)) {
        initialCodingVisualActionForced = true;
        const forcedLiveSink = createAssistantLiveTextSink({
          baseTranscript: params.baseTranscript,
          assistantMessageId: assistantMessage.id,
          agentEvents: params.agentEvents,
          commitTranscript: params.commitTranscript,
          assistantRun: params.assistantRun
        });
        const forcedContinuation = await buildAssistantTranscriptMessage(
          agentActionInitialCodingVisualForcedContinuationUserText(params.originalUserText, assistantMessage.text),
          params.providerAttachments,
          params.userMessageId,
          params.moduleId,
          params.baseTranscript,
          forcedLiveSink,
          assistantMessage.id
        );
        throwIfAssistantRunCancelled(params.assistantRun);
        assistantMessage = {
          ...forcedContinuation,
          id: assistantMessage.id,
          proofHash: hashJson({
            agentActionLoopInitialCodingVisualForcedContinuation: true,
            previousProofHash: assistantMessage.proofHash,
            continuationProofHash: forcedContinuation.proofHash
          })
        };
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
        continue;
      }
      if (!codingLivePreviewForced && shouldForceCodingLivePreviewAfterWrite(params.originalUserText, assistantMessage.text, loopState)) {
        codingLivePreviewForced = true;
        const previewLiveSink = createAssistantLiveTextSink({
          baseTranscript: params.baseTranscript,
          assistantMessageId: assistantMessage.id,
          agentEvents: params.agentEvents,
          commitTranscript: params.commitTranscript,
          assistantRun: params.assistantRun
        });
        const previewContinuation = await buildAssistantTranscriptMessage(
          agentActionCodingLivePreviewForcedContinuationUserText(params.originalUserText, loopState, assistantMessage.text),
          params.providerAttachments,
          params.userMessageId,
          params.moduleId,
          params.baseTranscript,
          previewLiveSink,
          assistantMessage.id
        );
        throwIfAssistantRunCancelled(params.assistantRun);
        assistantMessage = {
          ...previewContinuation,
          id: assistantMessage.id,
          proofHash: hashJson({
            agentActionLoopCodingLivePreviewForcedContinuation: true,
            previousProofHash: assistantMessage.proofHash,
            continuationProofHash: previewContinuation.proofHash
          })
        };
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
        continue;
      }
      if (loopState.toolSteps > 0) {
        loopState = agentActionLoopWithStatus(loopState, loopState.finalStatus === "running" ? "completed" : loopState.finalStatus);
        assistantMessage = ensureAgentActionLoopFinalSummary(assistantMessage, loopState);
        emitAgentLoopDiagnosticSummary({
          agentEvents: params.agentEvents,
          messageId: assistantMessage.id,
          outcome: reportableAgentActionLoopOutcome(loopState, "completed"),
          toolSteps: loopState.toolSteps,
          loopState
        });
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
      }
      return assistantMessage;
    }
    const toolCall = emitAgentRuntimeToolCallStarted({
      agentEvents: params.agentEvents,
      messageId: assistantMessage.id,
      step: step + 1,
      request: extracted.request
    });
    assistantMessage = {
      ...assistantMessage,
      text: renderPendingAgentActionText(assistantMessage.text, extracted),
      proofHash: hashJson({ agentActionLoopPendingStep: step + 1, previousProofHash: assistantMessage.proofHash, request: extracted.request })
    };
    params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
    throwIfAssistantRunCancelled(params.assistantRun);
    const result = await executeTrackedAgentAction(extracted.request);
    throwIfAssistantRunCancelled(params.assistantRun);
    emitAgentRuntimeToolResult({
      agentEvents: params.agentEvents,
      messageId: assistantMessage.id,
      toolCall,
      result
    });
    loopState = agentActionLoopWithResult(loopState, extracted.request, result, step);
    assistantMessage = {
      ...assistantMessage,
      text: renderCompletedPendingAgentActionText(assistantMessage.text, result),
      proofHash: hashJson({ agentActionLoopStep: step + 1, previousProofHash: assistantMessage.proofHash, request: extracted.request, result, loopState })
    };
    params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
    if (!result.accepted) {
      const failureLiveSink = createAssistantLiveTextSink({
        baseTranscript: params.baseTranscript,
        assistantMessageId: assistantMessage.id,
        agentEvents: params.agentEvents,
        commitTranscript: params.commitTranscript,
        prefixText: assistantMessage.text,
        assistantRun: params.assistantRun
      });
      const failureContinuation = await buildAssistantTranscriptMessage(
        agentActionFailureContinuationUserText(params.originalUserText, extracted.request, result, step),
        params.providerAttachments,
        params.userMessageId,
        params.moduleId,
        transcriptWithMessage(params.baseTranscript, assistantMessage),
        failureLiveSink,
        assistantMessage.id
      );
      throwIfAssistantRunCancelled(params.assistantRun);
      assistantMessage = {
        ...failureContinuation,
        id: assistantMessage.id,
        text: [assistantMessage.text, failureContinuation.text].filter((part) => part.trim().length > 0).join("\n\n"),
        proofHash: hashJson({ agentActionLoopFailureContinuationStep: step + 1, previousProofHash: assistantMessage.proofHash, continuationProofHash: failureContinuation.proofHash })
      };
      params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
      if (extractAgentActionJsonRequest(assistantMessage.text)) {
        step += 1;
        continue;
      }
      loopState = agentActionLoopWithStatus(loopState, agentActionLoopOutcomeAfterFailure(loopState, result));
      assistantMessage = ensureAgentActionLoopFinalSummary(assistantMessage, loopState);
      emitAgentLoopDiagnosticSummary({
        agentEvents: params.agentEvents,
        messageId: assistantMessage.id,
        outcome: reportableAgentActionLoopOutcome(loopState, "blocked"),
        toolSteps: loopState.toolSteps,
        loopState
      });
      params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
      return assistantMessage;
    }
    if (
      AGENT_ACTION_COMPAT_DETERMINISTIC_FALLBACK &&
      agentActionShouldRunFileOrganizationFallback(params.originalUserText, extracted.request, result) &&
      agentActionStepNeedsMutationFollowUp(params.originalUserText, extracted.request, result)
    ) {
      const fallbackMessage = await applyDeterministicOrganizationFallback({
        assistantMessage,
        originalUserText: params.originalUserText,
        request: extracted.request,
        result,
        agentEvents: params.agentEvents,
        assistantRun: params.assistantRun,
        commitProgress: (message) => params.commitTranscript(transcriptWithMessage(params.baseTranscript, message))
      });
      if (fallbackMessage.text !== assistantMessage.text) {
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, fallbackMessage));
        emitAgentLoopDiagnosticSummary({
          agentEvents: params.agentEvents,
          messageId: fallbackMessage.id,
          outcome: "completed",
          toolSteps: loopState.toolSteps,
          loopState: agentActionLoopWithStatus(loopState, "completed")
        });
        return fallbackMessage;
      }
    }
    const continuationLiveSink = createAssistantLiveTextSink({
      baseTranscript: params.baseTranscript,
      assistantMessageId: assistantMessage.id,
      agentEvents: params.agentEvents,
      commitTranscript: params.commitTranscript,
      prefixText: assistantMessage.text,
      assistantRun: params.assistantRun
    });
    const continuation = await buildAssistantTranscriptMessage(
      agentActionLoopContinuationUserText(params.originalUserText, extracted.request, result, step),
      params.providerAttachments,
      params.userMessageId,
      params.moduleId,
      transcriptWithMessage(params.baseTranscript, assistantMessage),
      continuationLiveSink,
      assistantMessage.id
    );
    throwIfAssistantRunCancelled(params.assistantRun);
    assistantMessage = {
      ...continuation,
      id: assistantMessage.id,
      text: [assistantMessage.text, continuation.text].filter((part) => part.trim().length > 0).join("\n\n"),
      proofHash: hashJson({ agentActionLoopStep: step + 1, previousProofHash: assistantMessage.proofHash, continuationProofHash: continuation.proofHash })
    };
    if (agentActionStepNeedsMutationFollowUp(params.originalUserText, extracted.request, result) && !extractAgentActionJsonRequest(assistantMessage.text)) {
      const forcedLiveSink = createAssistantLiveTextSink({
        baseTranscript: params.baseTranscript,
        assistantMessageId: assistantMessage.id,
        agentEvents: params.agentEvents,
        commitTranscript: params.commitTranscript,
        prefixText: assistantMessage.text,
        assistantRun: params.assistantRun
      });
      const forcedContinuation = await buildAssistantTranscriptMessage(
        agentActionForcedContinuationUserText(params.originalUserText, extracted.request, result, continuation.text, step),
        params.providerAttachments,
        params.userMessageId,
        params.moduleId,
        transcriptWithMessage(params.baseTranscript, assistantMessage),
        forcedLiveSink,
        assistantMessage.id
      );
      throwIfAssistantRunCancelled(params.assistantRun);
      assistantMessage = {
        ...forcedContinuation,
        id: assistantMessage.id,
        text: [assistantMessage.text, forcedContinuation.text].filter((part) => part.trim().length > 0).join("\n\n"),
        proofHash: hashJson({ agentActionLoopForcedContinuationStep: step + 1, previousProofHash: assistantMessage.proofHash, continuationProofHash: forcedContinuation.proofHash })
      };
      if (!extractAgentActionJsonRequest(assistantMessage.text)) {
        if (AGENT_ACTION_COMPAT_DETERMINISTIC_FALLBACK) {
          assistantMessage = await applyDeterministicOrganizationFallback({
            assistantMessage,
            originalUserText: params.originalUserText,
            request: extracted.request,
            result,
            agentEvents: params.agentEvents,
            assistantRun: params.assistantRun,
            commitProgress: (message) => params.commitTranscript(transcriptWithMessage(params.baseTranscript, message))
          });
        } else {
          loopState = agentActionLoopWithStatus(loopState, "blocked");
          assistantMessage = ensureAgentActionLoopFinalSummary(assistantMessage, loopState);
        }
        params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage));
        return assistantMessage;
      }
    }
    step += 1;
  }
}

async function buildAssistantTranscriptMessage(
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId: string,
  moduleId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript,
  liveSink?: ProviderLiveTextSink,
  assistantMessageId = `assistant-response-${Date.now()}`
): Promise<TranscriptMessage> {
  let profile = providerProfileFromComposer(panelsChatBottomState.selectedProvider);
  if (profile.connectId === "codex" && !profile.connected) {
    const localProfile = await applyCodexLocalAuthProfile(["connect Codex local OAuth on send"]);
    if (localProfile) {
      profile = localProfile;
    }
  }
  const model = selectedComposerModel(profile);
  const reasoning = selectedComposerReasoning(profile);
  const attachmentProofs = attachmentProofSummary(attachments);
  const providerUserText = providerUserTextForTurn(userText, attachments, userMessageId, transcript);
  const adapter = assistantProviderAdapterFor(profile);
  try {
    if (profile.connected) {
      const run = await adapter.run({
        profile,
        rawUserText: userText,
        providerUserText,
        attachments,
        userMessageId,
        moduleId,
        transcript,
        liveSink
      });
      return {
        id: assistantMessageId,
        role: "assistant",
        text: run.text,
        proofHash: hashJson({ provider: profile.connectId, model, reasoning, runtime: run.runtime, userText, providerUserText, attachments: attachmentProofs, text: run.text })
      };
    }
    const reason = adapter.unavailableReason(profile);
    const text = ASSISTANT_PROVIDER_UNAVAILABLE_TEXT;
    return {
      id: `assistant-error-${Date.now()}`,
      role: "assistant",
      text,
      proofHash: hashJson({ provider: profile.connectId, model, userText, providerUserText, attachments: attachmentProofs, reason })
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "LLM request failed.";
    const text = friendlyAssistantErrorText({
      userText: providerUserText || userText,
      providerLabel: profile.label,
      model,
      message,
      hasImageAttachment: attachments.some((attachment) => attachment.kind === "image")
    });
    return {
      id: `assistant-error-${Date.now()}`,
      role: "assistant",
      text,
      proofHash: hashJson({ provider: profile.connectId, model, userText, providerUserText, attachments: attachmentProofs, error: message, text })
    };
  }
}

function stringField(value: unknown, key: string): string {
  if (!value || typeof value !== "object") {
    return "";
  }
  const raw = (value as Record<string, unknown>)[key];
  return typeof raw === "string" ? raw.trim() : "";
}

async function exchangeOpenRouterCodeForKey(code: string, codeVerifier: string): Promise<string> {
  const parsed = await openRouterFetchJson("https://openrouter.ai/api/v1/auth/keys", {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      code,
      code_verifier: codeVerifier,
      code_challenge_method: "S256"
    })
  });
  const key = stringField(parsed, "key");
  if (!key) {
    throw new Error("OpenRouter OAuth did not return an API key.");
  }
  return key;
}

async function probeOpenRouterApiKey(apiKey: string): Promise<OpenRouterProbe> {
  try {
    const modelsJson = await openRouterFetchJson("https://openrouter.ai/api/v1/models?output_modalities=text", {
      method: "GET",
      headers: {
        Authorization: `Bearer ${apiKey}`
      }
    });
    const modelRows = Array.isArray((modelsJson as { data?: unknown }).data)
      ? ((modelsJson as { data: unknown[] }).data)
      : [];
    const models = [
      ...new Set(
        modelRows
          .map((row) => stringField(row, "id"))
          .filter(Boolean)
      )
    ];
    const supportsReasoning = modelRows.some((row) => {
      if (!row || typeof row !== "object") {
        return false;
      }
      const params = (row as { supported_parameters?: unknown }).supported_parameters;
      return Array.isArray(params) && params.some((param) => typeof param === "string" && /reasoning|effort/i.test(param));
    });
    let quotaLabel = "quota unavailable: official credits not returned";
    try {
      const creditsJson = await openRouterFetchJson("https://openrouter.ai/api/v1/credits", {
        method: "GET",
        headers: {
          Authorization: `Bearer ${apiKey}`
        }
      });
      const credits = creditsJson && typeof creditsJson === "object" ? (creditsJson as { data?: Record<string, unknown> }).data : undefined;
      const totalCredits = typeof credits?.total_credits === "number" ? credits.total_credits : undefined;
      const totalUsage = typeof credits?.total_usage === "number" ? credits.total_usage : undefined;
      if (typeof totalCredits === "number" && typeof totalUsage === "number") {
        const remaining = Math.max(0, totalCredits - totalUsage);
        quotaLabel = `quota credits ${remaining.toFixed(2)} remaining / ${totalCredits.toFixed(2)} total`;
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "credits unavailable";
      quotaLabel = /403|management/i.test(message)
        ? "quota unavailable: OpenRouter credits endpoint requires management key"
        : "quota unavailable: official credits not returned";
    }
    const reasoning = supportsReasoning ? ["low", "medium", "high"] : [];
    const account = `OpenRouter API key ${apiKey.slice(0, 7)}...${apiKey.slice(-4)}`;
    return {
      ok: models.length > 0,
      models,
      reasoning,
      quotaLabel,
      account,
      proof: hashJson({
        provider: "openrouter",
        models,
        reasoning,
        quotaLabel,
        keyFingerprint: hashJson(apiKey).slice(0, 16)
      }),
      error: models.length > 0 ? undefined : "OpenRouter model catalog returned no models."
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "OpenRouter API probe failed.";
    return {
      ok: false,
      models: [],
      reasoning: [],
      quotaLabel: "quota unavailable: official credits not returned",
      account: "OpenRouter OAuth",
      proof: hashJson({ provider: "openrouter", error: message }),
      error: message
    };
  }
}

function applyOpenRouterProbe(probe: OpenRouterProbe, activateComposer = false): ProviderRuntimeProfile {
  const profile = providerRuntime.openrouter;
  profile.connected = probe.ok;
  profile.account = probe.account;
  profile.models = probe.models;
  profile.reasoning = probe.reasoning;
  profile.quotaLabel = probe.quotaLabel;
  profile.proof = probe.proof;
  if (probe.ok && activateComposer) {
    activateComposerProvider(profile);
  }
  return profile;
}

function openRouterReadyEvents(profile: ProviderRuntimeProfile): string[] {
  const preview = profile.models.slice(0, 18);
  const remaining = profile.models.length - preview.length;
  const modelLine = remaining > 0
    ? `models ${profile.models.length} available / ${preview.join(" / ")} / +${remaining} more`
    : `models ${preview.join(" / ")}`;
  return [
    "OpenRouter OAuth PKCE confirmed",
    "model catalog received from OpenRouter API",
    modelLine,
    profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
    profile.quotaLabel,
    "ready"
  ];
}

function emailDomainFrom(value: unknown): string | undefined {
  const email = cleanProbeText(value);
  const domain = email?.split("@").at(1);
  return domain && /^[a-z0-9.-]+$/i.test(domain) ? domain : undefined;
}

async function probeCodexAccount(authWindow: BrowserWindow): Promise<CodexAccountProbe> {
  if (authWindow.isDestroyed()) {
    return { ok: false, error: "OAuth window closed" };
  }

  const failedWebSessionProbe = async (status?: number, error?: string): Promise<CodexAccountProbe> => {
    let authCookieCount = 0;
    try {
      const cookies = await authWindow.webContents.session.cookies.get({ url: "https://chatgpt.com" });
      authCookieCount = cookies.filter((cookie) => /session|access|auth|token|oai-|_puid/i.test(cookie.name)).length;
    } catch (cookieError) {
      const cookieMessage = cookieError instanceof Error ? cookieError.message : "cookie proof unavailable";
      return { ok: false, status, source: "web-session", error: error ?? cookieMessage };
    }
    const suffix = authCookieCount > 0
      ? `; auth-like cookies present=${authCookieCount}, but account API rejected the session`
      : "";
    return {
      ok: false,
      status,
      source: "web-session",
      error: error ?? (status ? `OpenAI account API returned ${status}${suffix}` : `OpenAI account API was not confirmed${suffix}`)
    };
  };

  try {
    const result = await authWindow.webContents.executeJavaScript(
      `(() => {
        const pick = (value, keys) => {
          if (!value || typeof value !== "object") return undefined;
          for (const key of keys) {
            if (typeof value[key] === "string" && value[key].length > 0) return value[key];
          }
          return undefined;
        };
        const pickDeep = (value, keys) => {
          const direct = pick(value, keys);
          if (direct) return direct;
          if (!value || typeof value !== "object") return undefined;
          const accounts = Array.isArray(value.accounts) ? value.accounts : [];
          for (const account of accounts) {
            const nested = pick(account, keys);
            if (nested) return nested;
          }
          const user = value.user && typeof value.user === "object" ? value.user : undefined;
          return pick(user, keys);
        };
        return fetch("/backend-api/me", { credentials: "include" })
          .then(async (response) => {
            const text = await response.text();
            let data = {};
            try { data = JSON.parse(text); } catch {}
            return {
              ok: response.ok,
              status: response.status,
              accountId: pickDeep(data, ["account_id", "accountId", "id"]),
              email: pickDeep(data, ["email"]),
              plan: pickDeep(data, ["plan_type", "plan", "workspace_plan", "subscription_plan"])
            };
          })
          .catch((error) => ({ ok: false, error: error && error.message ? error.message : String(error) }));
      })()`,
      true
    );
    if (!result || typeof result !== "object") {
      return { ok: false, error: "empty account probe" };
    }
    const candidate = result as Record<string, unknown>;
    const apiOk = candidate.ok === true;
    if (!apiOk) {
      return failedWebSessionProbe(
        typeof candidate.status === "number" ? candidate.status : undefined,
        cleanProbeText(candidate.error)
      );
    }
    return {
      ok: true,
      status: typeof candidate.status === "number" ? candidate.status : undefined,
      accountId: cleanProbeText(candidate.accountId),
      emailDomain: emailDomainFrom(candidate.email),
      plan: cleanProbeText(candidate.plan),
      source: "api",
      error: cleanProbeText(candidate.error)
    };
  } catch (error) {
    return failedWebSessionProbe(undefined, error instanceof Error ? error.message : "account probe failed");
  }
}

function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0) : [];
}

async function probeCodexModelCatalog(authWindow: BrowserWindow): Promise<ModelCatalogProbe> {
  if (authWindow.isDestroyed()) {
    return {
      models: [],
      reasoning: [],
      quotaLabel: "quota unavailable: OAuth window closed",
      event: "model catalog unavailable",
      proof: hashJson({ provider: "codex", catalog: "window_closed" })
    };
  }

  const models = [...CODEX_DESKTOP_MODELS];
  const reasoning = [...CODEX_DESKTOP_REASONING];
  return {
    models,
    reasoning,
    quotaLabel: "quota unavailable: official token balance not returned",
    event: "model catalog received from Codex Desktop",
    proof: hashJson({ provider: "codex", source: "codex_desktop", models, reasoning })
  };

  try {
    const result = await authWindow.webContents.executeJavaScript(
      `(() => {
        const modelPattern = /\\bgpt-[0-9][a-z0-9._-]*(?:-[a-z0-9._-]+)*\\b/ig;
        const models = new Set();
        const reasoning = new Set();
        const quota = [];
        const sources = [];
        const normalizeModel = (value) => String(value || "").trim().toLowerCase().replace(/\\.access$/i, "");
        const addReasoning = (value) => {
          const level = String(value || "").trim().toLowerCase();
          if (/^(none|minimal|low|medium|high|xhigh|max|auto)$/.test(level)) {
            reasoning.add(level === "max" ? "xhigh" : level);
          }
        };
        const addModelText = (text) => {
          if (typeof text !== "string") return;
          modelPattern.lastIndex = 0;
          for (const match of text.matchAll(modelPattern)) {
            const model = normalizeModel(match[0]);
            if (!/\\.(document|presentation|spreadsheet|xml|json|html)$/i.test(model)) {
              models.add(model);
            }
          }
        };
        const visit = (value, key = "", depth = 0) => {
          if (depth > 8 || value == null) return;
          if (typeof value === "string") {
            const lowerKey = String(key).toLowerCase();
            const lowerValue = value.toLowerCase();
            if (/\\b(model|models|model_slug|model_slug_id|default_model|selected_model|available_models|allowed_models|slug|name)\\b/.test(lowerKey) || modelPattern.test(value)) {
              modelPattern.lastIndex = 0;
              addModelText(value);
            }
            if (/reasoning|effort/.test(lowerKey)) {
              addReasoning(lowerValue);
            }
            if (/token/.test(lowerKey) && /quota|usage|limit|remaining|balance|cap/.test(lowerKey)) {
              quota.push(String(key) + "=" + value);
            }
            return;
          }
          if (typeof value === "number") {
            const lowerKey = String(key).toLowerCase();
            if (/token/.test(lowerKey) && /quota|usage|limit|remaining|balance|cap/.test(lowerKey)) {
              quota.push(String(key) + "=" + value);
            }
            return;
          }
          if (Array.isArray(value)) {
            if (/supported_reasoning_levels|reasoning_levels|reasoning_efforts|effort_levels/i.test(String(key))) {
              value.forEach((item) => {
                if (typeof item === "string") {
                  addReasoning(item);
                } else if (item && typeof item === "object") {
                  Object.values(item).forEach(addReasoning);
                }
              });
            }
            value.slice(0, 500).forEach((item, index) => visit(item, String(index), depth + 1));
            return;
          }
          if (typeof value === "object") {
            for (const [childKey, childValue] of Object.entries(value).slice(0, 1000)) {
              visit(childValue, childKey, depth + 1);
            }
          }
        };
        const scanText = (source, text) => {
          if (!text) return;
          sources.push(source);
          try { visit(JSON.parse(text)); } catch {}
        };
        const endpointPaths = [
          "/backend-api/models",
          "/backend-api/conversation/models",
          "/backend-api/codex/models",
          "/backend-api/codex/bootstrap",
          "/backend-api/settings",
          "/backend-api/accounts/check/v4-2023-04-27"
        ];
        const endpointTasks = endpointPaths.map(async (path) => {
          try {
            const response = await fetch(path, { credentials: "include" });
            const text = await response.text();
            sources.push(path + ":" + response.status);
            scanText(path, text);
          } catch (error) {
            sources.push(path + ":fetch_failed");
          }
        });
        const storageTasks = ["localStorage", "sessionStorage"].map((storageName) => {
          try {
            const storage = window[storageName];
            for (let index = 0; index < storage.length; index += 1) {
              const key = storage.key(index);
              if (!key) continue;
              scanText(storageName + ":" + key, storage.getItem(key) || "");
            }
          } catch {}
        });
        const scriptTasks = Array.from(document.scripts)
          .filter((script) => /json|ld\\+json/i.test(script.type || "") || /__NEXT_DATA__|remix|apollo|relay/i.test(script.id || ""))
          .slice(0, 40)
          .map((script, index) => scanText("script:" + (script.id || index), script.textContent || ""));
        return Promise.all(endpointTasks).then(() => ({
          models: Array.from(models),
          reasoning: Array.from(reasoning),
          quotaLabel: quota.length > 0 ? "quota " + quota.slice(0, 4).join(" ") : "quota unavailable: official token balance not returned",
          sources: sources.slice(0, 32)
        }));
      })()`,
      true
    );
    const candidate = result && typeof result === "object" ? (result as Record<string, unknown>) : {};
    const discoveredModels = [
      ...new Set(
        stringList(candidate.models)
          .map(normalizeGptModelId)
          .filter((model) => /^gpt-[0-9][a-z0-9._-]*$/i.test(model))
      )
    ];
    const routableModels = discoveredModels
      .filter((model) => CODEX_DESKTOP_MODELS.map(codexDesktopModelSlug).includes(model))
      .sort((left, right) => {
      const codexDesktopModelSlugs = CODEX_DESKTOP_MODELS.map(codexDesktopModelSlug);
      const leftIndex = codexDesktopModelSlugs.indexOf(left);
      const rightIndex = codexDesktopModelSlugs.indexOf(right);
      if (leftIndex >= 0 || rightIndex >= 0) {
        return (leftIndex >= 0 ? leftIndex : Number.MAX_SAFE_INTEGER) - (rightIndex >= 0 ? rightIndex : Number.MAX_SAFE_INTEGER);
      }
      return left.localeCompare(right);
    });
    const models = routableModels.length > 0 ? routableModels : ["gpt-4o"];
    const extractedReasoning = [
      ...new Set(
        stringList(candidate.reasoning)
          .map(normalizeReasoningLevel)
          .filter(isReasoningLevel)
      )
    ];
    const reasoning = extractedReasoning.length > 0 ? extractedReasoning : reasoningLevelsForModels(models);
    const quotaLabel = cleanProbeText(candidate.quotaLabel) ?? "quota unavailable: official token balance not returned";
    const sources = stringList(candidate.sources).slice(0, 12);
    return {
      models,
      reasoning,
      quotaLabel,
      event: models.length > 0 ? "model catalog received from OpenAI session" : "model catalog unavailable",
      proof: hashJson({ provider: "codex", models, reasoning, quotaLabel, sources })
    };
  } catch {
    const message = "model catalog probe failed";
    return {
      models: [],
      reasoning: [],
      quotaLabel: "quota unavailable: official token balance not returned",
      event: "model catalog unavailable",
      proof: hashJson({ provider: "codex", error: message })
    };
  }
}

async function inspectCodexAuthWindow(authWindow: BrowserWindow): Promise<void> {
  if (authWindow.isDestroyed()) {
    return;
  }
  const url = authWindow.webContents.getURL();
  const title = authWindow.getTitle();
  if (!url.includes("chatgpt.com")) {
    return;
  }

  let bodyText = "";
  try {
    bodyText = await authWindow.webContents.executeJavaScript(
      "document.body ? document.body.innerText.slice(0, 2400) : ''",
      true
    );
  } catch {
    bodyText = "";
  }

  const text = `${title}\n${bodyText}`;
  {
    const isLoginScreen = /Connectez-vous ou inscrivez-vous|Log in or sign up|Se connecter|Inscription gratuite|Continue with Google|Continuer avec Google/i.test(text);
    if (isLoginScreen) {
      return;
    }
    const profile = providerRuntime.codex;
    const pendingEvents = [
      "ChatGPT web session confirmed",
      "OpenAI account probe running"
    ];
    const probe = await probeCodexAccount(authWindow);
    if (!probe.ok) {
      if (await probeChatGptComposerReady(authWindow)) {
        profile.connected = true;
        profile.account = "ChatGPT subscription page session";
        const catalog = await probeCodexModelCatalog(authWindow);
        profile.models = catalog.models;
        profile.reasoning = catalog.reasoning;
        profile.quotaLabel = catalog.quotaLabel;
        profile.proof = hashJson({
          provider: "codex",
          account: profile.account,
          authProbe: { status: probe.status, error: probe.error },
          pageComposerReady: true,
          models: profile.models,
          reasoning: profile.reasoning,
          catalogProof: catalog.proof
        });
        activateComposerProvider(profile);
        await authWindow.webContents.session.flushStorageData();
        void persistProviderRuntime().catch((error: unknown) => {
          console.error("Failed to persist Codex provider runtime.", error);
        });
        const events = [
          ...pendingEvents,
          "ChatGPT composer ready",
          "OpenAI account API unavailable; using verified page session",
          catalog.event,
          ...modelCatalogEvents(profile.models),
          profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
          profile.quotaLabel,
          "ready"
        ];
        markProviderReadyEvents(profile, events);
        emitLlmProviderRuntimeEvent({
          provider: "codex",
          events,
          models: profile.models,
          reasoning: profile.reasoning,
          quotaLabel: profile.quotaLabel,
          proofHash: profile.proof
        });
        return;
      }
      profile.connected = false;
      profile.models = [];
      profile.reasoning = [];
      profile.quotaLabel = probe.error ?? "connection pending: OpenAI account probe failed";
      profile.proof = hashJson({ provider: "codex", url, title, status: probe.status, ok: false, error: probe.error });
      profile.events = [
        ...pendingEvents,
        probe.status ? `OpenAI account API returned ${probe.status}` : "OpenAI account API not confirmed",
        "not ready"
      ];
      void persistProviderRuntime().catch((error: unknown) => {
        console.error("Failed to persist Codex provider runtime.", error);
      });
      emitLlmProviderRuntimeEvent({
        provider: "codex",
        events: profile.events,
        models: [],
        reasoning: [],
        quotaLabel: profile.quotaLabel,
        proofHash: profile.proof
      });
      return;
    }
    if (profile.connected && profile.proof) {
      return;
    }

    profile.connected = true;
    profile.account = probe.emailDomain ? `ChatGPT account @${probe.emailDomain}` : "ChatGPT subscription account";
    const catalog = await probeCodexModelCatalog(authWindow);
    profile.models = catalog.models;
    profile.reasoning = catalog.reasoning;
    profile.quotaLabel = catalog.quotaLabel;
    profile.proof = hashJson({
      provider: "codex",
      accountId: probe.accountId ?? "chatgpt-session",
      account: profile.account,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      catalogProof: catalog.proof
    });
    activateComposerProvider(profile);
    await authWindow.webContents.session.flushStorageData();
    void persistProviderRuntime().catch((error: unknown) => {
      console.error("Failed to persist Codex provider runtime.", error);
    });
    const reasoningEvent = profile.reasoning.length > 0
      ? `reasoning ${profile.reasoning.join(" / ")}`
      : "reasoning unavailable";
    const events = [
      ...pendingEvents,
      "OpenAI account profile confirmed",
      catalog.event,
      ...modelCatalogEvents(profile.models),
      reasoningEvent,
      profile.quotaLabel,
      "ready"
    ];
    markProviderReadyEvents(profile, events);
    emitLlmProviderRuntimeEvent({
      provider: "codex",
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    });
    return;
  }
  const isCodexPage = /Codex/i.test(title) || /\/codex/i.test(url) || /Votre assistant IA au travail|AI Assistant for Work and Code/i.test(text);
  const isLoginPage = /Connectez-vous ou inscrivez-vous|Log in or sign up|Continue with Google|Continuer avec Google/i.test(text);
  const isConnectedPage = isCodexPage && !isLoginPage && /Acc[eé]der au Cloud|T[eé]l[eé]charger pour Windows|Download for Windows|assistant IA au travail/i.test(text);

  if (!isConnectedPage) {
    return;
  }

  const profile = providerRuntime.codex;
  if (profile.connected) {
    return;
  }

  const pendingEvents = [
    "ChatGPT web session confirmed",
    "Codex page loaded inside OAuth window"
  ];
  const probe = await probeCodexAccount(authWindow);
  if (!probe.ok) {
    emitLlmProviderRuntimeEvent({
      provider: "codex",
      events: [
        ...pendingEvents,
        probe.status ? `OpenAI web session proof returned ${probe.status}` : "OpenAI web session proof pending",
        "not ready"
      ],
      models: [],
      reasoning: [],
      quotaLabel: "pending",
      proofHash: hashJson({ provider: "codex", url, title, status: probe.status, ok: false })
    });
    return;
  }

  profile.connected = true;
  profile.account = probe.emailDomain ? `ChatGPT account @${probe.emailDomain}` : "ChatGPT subscription account";
  const catalog = await probeCodexModelCatalog(authWindow);
  profile.models = catalog.models;
  profile.reasoning = catalog.reasoning;
  profile.quotaLabel = catalog.quotaLabel;
  profile.proof = hashJson({
    provider: "codex",
    accountId: probe.accountId ?? "chatgpt-session",
    account: profile.account,
    models: profile.models,
    reasoning: profile.reasoning,
    quotaLabel: profile.quotaLabel,
    catalogProof: catalog.proof
  });
  activateComposerProvider(profile);
  await authWindow.webContents.session.flushStorageData();
  void persistProviderRuntime().catch((error: unknown) => {
    console.error("Failed to persist Codex provider runtime.", error);
  });
  const reasoningEvent = profile.reasoning.length > 0
    ? `reasoning ${profile.reasoning.join(" / ")}`
    : "reasoning unavailable";
  const events = [
    ...pendingEvents,
    probe.source === "api" ? "OpenAI account profile confirmed" : "OpenAI web session confirmed",
    catalog.event,
    ...modelCatalogEvents(profile.models),
    reasoningEvent,
    profile.quotaLabel,
    "ready"
  ];
  markProviderReadyEvents(profile, events);
  emitLlmProviderRuntimeEvent({
    provider: "codex",
    events,
    models: profile.models,
    reasoning: profile.reasoning,
    quotaLabel: profile.quotaLabel,
    proofHash: profile.proof
  });
}

async function validatePersistedCodexSession(): Promise<void> {
  const localProfile = await applyCodexLocalAuthProfile(["restored Codex local OAuth session"]);
  if (localProfile) {
    return;
  }
  const profile = providerRuntime.codex;
  let authWindow: BrowserWindow | undefined;
  try {
    authWindow = new BrowserWindow({
      width: 980,
      height: 720,
      show: false,
      autoHideMenuBar: true,
      backgroundColor: "#0e0e0f",
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        webSecurity: true,
        backgroundThrottling: false
      }
    });
    authWindow.webContents.setUserAgent(CHATGPT_USER_AGENT);
    await authWindow.loadURL(CHATGPT_HOME_URL);
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 700);
    });
    if (authWindow.isDestroyed()) {
      return;
    }

    let pageText = "";
    try {
      pageText = await authWindow.webContents.executeJavaScript(
        "`${document.title}\\n${document.body ? document.body.innerText.slice(0, 2400) : ''}`",
        true
      );
    } catch {
      pageText = "";
    }
    const isLoginPage = /Connectez-vous ou inscrivez-vous|Log in or sign up|Se connecter|Inscription gratuite|Continue with Google|Continuer avec Google/i.test(pageText);
    if (isLoginPage) {
      if (profile.connected) {
        profile.connected = false;
        profile.models = [];
        profile.reasoning = [];
        profile.quotaLabel = "connection expired: login required";
        profile.proof = hashJson({ provider: "codex", restored: false, reason: "login_page" });
        await persistProviderRuntime();
        emitLlmProviderRuntimeEvent({
          provider: "codex",
          events: ["stored OpenAI OAuth session expired", "not ready"],
          models: [],
          reasoning: [],
          quotaLabel: profile.quotaLabel,
          proofHash: profile.proof
        });
      }
      return;
    }
    const probe = await probeCodexAccount(authWindow);
    if (!probe.ok) {
      if (await probeChatGptComposerReady(authWindow)) {
        const catalog = await probeCodexModelCatalog(authWindow);
        profile.connected = true;
        profile.account = "ChatGPT subscription page session";
        profile.models = catalog.models;
        profile.reasoning = catalog.reasoning;
        profile.quotaLabel = catalog.quotaLabel;
        profile.proof = hashJson({
          provider: "codex",
          restored: true,
          authProbe: { status: probe.status, error: probe.error },
          pageComposerReady: true,
          models: profile.models,
          reasoning: profile.reasoning,
          catalogProof: catalog.proof
        });
        if (panelsChatBottomState.selectedProvider === profile.composerProvider) {
          panelsChatBottomState.modelIndex = Math.min(panelsChatBottomState.modelIndex, Math.max(0, profile.models.length - 1));
          panelsChatBottomState.reasoningIndex = Math.min(panelsChatBottomState.reasoningIndex, Math.max(0, profile.reasoning.length - 1));
        }
        await authWindow.webContents.session.flushStorageData();
        const events = [
          "restored ChatGPT page session",
          "ChatGPT composer ready",
          "OpenAI account API unavailable; using verified page session",
          catalog.event,
          ...modelCatalogEvents(profile.models),
          profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
          profile.quotaLabel,
          "ready"
        ];
        markProviderReadyEvents(profile, events);
        await persistProviderRuntime();
        emitLlmProviderRuntimeEvent({
          provider: "codex",
          events,
          models: profile.models,
          reasoning: profile.reasoning,
          quotaLabel: profile.quotaLabel,
          proofHash: profile.proof
        });
        return;
      }
      if (profile.connected) {
        profile.connected = false;
        profile.models = [];
        profile.reasoning = [];
        profile.quotaLabel = probe.error ?? "connection expired: OpenAI account probe failed";
        profile.proof = hashJson({ provider: "codex", restored: false, status: probe.status, error: probe.error });
        profile.events = [
          "stored OpenAI OAuth session could not be confirmed",
          probe.status ? `OpenAI account API returned ${probe.status}` : "OpenAI account API not confirmed",
          "not ready"
        ];
        await persistProviderRuntime();
        emitLlmProviderRuntimeEvent({
          provider: "codex",
          events: profile.events,
          models: [],
          reasoning: [],
          quotaLabel: profile.quotaLabel,
          proofHash: profile.proof
        });
      }
      return;
    }

    const catalog = await probeCodexModelCatalog(authWindow);
    profile.connected = true;
    profile.account = probe.emailDomain ? `ChatGPT account @${probe.emailDomain}` : profile.account || "ChatGPT subscription account";
    profile.models = catalog.models.length > 0 ? catalog.models : profile.models;
    profile.reasoning = catalog.reasoning.length > 0 ? catalog.reasoning : profile.reasoning;
    profile.quotaLabel = catalog.quotaLabel;
    profile.proof = hashJson({
      provider: "codex",
      restored: true,
      accountId: probe.accountId ?? "chatgpt-session",
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      catalogProof: catalog.proof
    });
    if (panelsChatBottomState.selectedProvider === profile.composerProvider) {
      panelsChatBottomState.modelIndex = Math.min(panelsChatBottomState.modelIndex, Math.max(0, profile.models.length - 1));
      panelsChatBottomState.reasoningIndex = Math.min(panelsChatBottomState.reasoningIndex, Math.max(0, profile.reasoning.length - 1));
    }
    await authWindow.webContents.session.flushStorageData();
    const events = [
      "restored OpenAI OAuth session",
      catalog.event,
      ...modelCatalogEvents(profile.models),
      profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
      profile.quotaLabel,
      "ready"
    ];
    markProviderReadyEvents(profile, events);
    await persistProviderRuntime();
    emitLlmProviderRuntimeEvent({
      provider: "codex",
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    });
  } catch (error) {
    console.error("Failed to validate persisted Codex session.", error);
  } finally {
    if (authWindow && !authWindow.isDestroyed()) {
      authWindow.close();
    }
  }
}

async function validatePersistedClaudeSession(): Promise<void> {
  try {
    const probe = await probeClaudeCliProfile();
    if (!probe.ok) {
      if (await hasClaudeCodeCredentialStore()) {
        providerRuntime.claude.connected = false;
        providerRuntime.claude.events = ["restored Claude credential store", probe.cliFound ? "waiting Claude Code auth status" : "Claude Code runtime not installed", "not ready"];
        providerRuntime.claude.quotaLabel = probe.quotaLabel;
        providerRuntime.claude.proof = hashJson({ provider: "claude", credentialStore: true, runnable: false, probe });
        emitLlmProviderRuntimeEvent({
          provider: "claude",
          events: providerRuntime.claude.events,
          models: [],
          reasoning: [],
          quotaLabel: providerRuntime.claude.quotaLabel,
          proofHash: providerRuntime.claude.proof
        });
        return;
      }
      if (providerRuntime.claude.connected && probe.cliFound) {
        providerRuntime.claude.connected = false;
        providerRuntime.claude.models = [];
        providerRuntime.claude.reasoning = [];
        providerRuntime.claude.quotaLabel = probe.quotaLabel;
        providerRuntime.claude.proof = hashJson({ provider: "claude", restored: false, error: probe.error ?? "not_connected" });
        await persistProviderRuntime();
        emitLlmProviderRuntimeEvent({
          provider: "claude",
          events: ["stored Claude Code session expired", "not ready"],
          models: [],
          reasoning: [],
          quotaLabel: providerRuntime.claude.quotaLabel,
          proofHash: providerRuntime.claude.proof
        });
      }
      return;
    }
    const profile = applyClaudeCliProfile(probe);
    const events = claudeReadyEventsFromProbe(profile, probe);
    markProviderReadyEvents(profile, events);
    await persistProviderRuntime();
    emitLlmProviderRuntimeEvent({
      provider: "claude",
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    });
  } catch (error) {
    console.error("Failed to validate persisted Claude Code session.", error);
  }
}

async function validatePersistedOpenRouterSession(): Promise<void> {
  if (!openRouterApiKey) {
    return;
  }
  try {
    const probe = await probeOpenRouterApiKey(openRouterApiKey);
    if (!probe.ok) {
      providerRuntime.openrouter.connected = false;
      providerRuntime.openrouter.models = [];
      providerRuntime.openrouter.reasoning = [];
      providerRuntime.openrouter.quotaLabel = probe.quotaLabel;
      providerRuntime.openrouter.proof = probe.proof;
      await persistProviderRuntime();
      emitLlmProviderRuntimeEvent({
        provider: "openrouter",
        events: ["stored OpenRouter OAuth key expired", probe.error ?? "not ready", "not ready"],
        models: [],
        reasoning: [],
        quotaLabel: providerRuntime.openrouter.quotaLabel,
        proofHash: providerRuntime.openrouter.proof
      });
      return;
    }
    const profile = applyOpenRouterProbe(probe, true);
    const events = ["restored OpenRouter OAuth key", ...openRouterReadyEvents(profile)];
    markProviderReadyEvents(profile, events);
    await persistProviderRuntime();
    emitLlmProviderRuntimeEvent({
      provider: "openrouter",
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    });
  } catch (error) {
    console.error("Failed to validate persisted OpenRouter session.", error);
  }
}

function openProviderAuthWindow(provider: LlmProviderConnectId, url: string, title: string): ProviderLaunchResult {
  const existingWindow = authWindows.get(provider);
  if (existingWindow && !existingWindow.isDestroyed()) {
    if (provider === "codex") {
      existingWindow.webContents.setUserAgent(CHATGPT_USER_AGENT);
    }
    existingWindow.show();
    existingWindow.focus();
    void existingWindow.loadURL(url);
    if (provider === "codex") {
      void inspectCodexAuthWindow(existingWindow);
    }
    return {
      launched: true,
      events: [`focus ${title}`]
    };
  }

  try {
    const parent = primaryWindow && !primaryWindow.isDestroyed() ? primaryWindow : undefined;
    const authWindow = new BrowserWindow({
      width: 1120,
      height: 820,
      minWidth: 760,
      minHeight: 560,
      parent,
      modal: false,
      title,
      show: false,
      autoHideMenuBar: true,
      backgroundColor: "#0e0e0f",
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        webSecurity: true,
        backgroundThrottling: false
      }
    });

    authWindows.set(provider, authWindow);
    if (provider === "codex") {
      authWindow.webContents.setUserAgent(CHATGPT_USER_AGENT);
    }
    authWindow.once("ready-to-show", () => {
      if (!authWindow.isDestroyed()) {
        authWindow.show();
        authWindow.focus();
      }
    });
    const inspectAuthState = () => {
      if (provider === "codex") {
        void inspectCodexAuthWindow(authWindow);
        setTimeout(() => void inspectCodexAuthWindow(authWindow), 900);
      }
    };
    authWindow.webContents.on("did-finish-load", () => {
      if (!authWindow.isDestroyed()) {
        authWindow.show();
        authWindow.focus();
      }
      inspectAuthState();
    });
    authWindow.webContents.on("did-fail-load", () => {
      if (!authWindow.isDestroyed()) {
        authWindow.show();
        authWindow.focus();
      }
    });
    authWindow.webContents.on("did-navigate", inspectAuthState);
    authWindow.webContents.on("did-navigate-in-page", inspectAuthState);
    authWindow.webContents.setWindowOpenHandler(({ url: childUrl }) => {
      if (childUrl.startsWith("https://")) {
        void authWindow.loadURL(childUrl);
      }
      return { action: "deny" };
    });
    authWindow.on("closed", () => {
      if (authWindows.get(provider) === authWindow) {
        authWindows.delete(provider);
      }
    });
    void authWindow.loadURL(url).catch((error: unknown) => {
      console.error(`Failed to load ${title}.`, error);
      if (!authWindow.isDestroyed()) {
        void shell.openExternal(url);
      }
    });

    return {
      launched: true,
      events: [`open ${title}`]
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : `Failed to open ${title}.`;
    return {
      launched: false,
      events: [`failed to open ${title}`, message],
      error: message
    };
  }
}

function commandExists(command: string): boolean {
  if (/[\\/]/.test(command)) {
    return existsSync(command);
  }
  const probe = process.platform === "win32"
    ? spawnSync("where.exe", [command], { stdio: "ignore", timeout: 500, windowsHide: true })
    : spawnSync("which", [command], { stdio: "ignore", timeout: 500 });
  return probe.status === 0;
}

function claudeCodeCommandCandidates(): string[] {
  const envCommand = process.env.INGEN_CLAUDE_CODE_CLI?.trim();
  const appData = process.env.APPDATA?.trim();
  const localAppData = process.env.LOCALAPPDATA?.trim();
  const programFiles = process.env.ProgramFiles?.trim();
  const home = app.getPath("home");
  const candidates = [
    envCommand,
    "claude",
    join(home, ".local", "bin", "claude.exe"),
    appData ? join(appData, "npm", "claude.cmd") : undefined,
    appData ? join(appData, "npm", "claude.exe") : undefined,
    localAppData ? join(localAppData, "Programs", "Claude Code", "claude.exe") : undefined,
    programFiles ? join(programFiles, "Claude Code", "claude.exe") : undefined
  ];
  return candidates.filter((candidate): candidate is string => Boolean(candidate));
}

function resolveClaudeCodeCommand(): string | undefined {
  return claudeCodeCommandCandidates().find((command) => commandExists(command));
}

function hostPlatformLabel(): string {
  return process.platform === "win32"
    ? "Windows"
    : process.platform === "darwin"
      ? "macOS"
      : process.platform === "linux"
        ? "Linux"
        : process.platform;
}

function firstNonEmptyLine(text: string): string | undefined {
  return text.split(/\r?\n/).map((line) => line.trim()).find(Boolean);
}

function claudeRuntimeVerifiedEvent(profile: ProviderRuntimeProfile): string {
  if (profile.runtimeVerified) {
    const version = profile.runtimeVersion ? ` (${profile.runtimeVersion})` : "";
    return `Claude Code runtime verified on ${hostPlatformLabel()}: ${profile.runtimeCommand ?? "claude"}${version}`;
  }
  return `Claude Code runtime not verified on ${hostPlatformLabel()}`;
}

function commandLine(command: string, args: string[]): string {
  const quote = (value: string) => /\s/.test(value) ? `"${value.replace(/"/g, '\\"')}"` : value;
  return [command, ...args].map(quote).join(" ");
}

function claudeCredentialsPath(): string {
  const configDir = process.env.CLAUDE_CONFIG_DIR?.trim();
  return configDir ? join(configDir, ".credentials.json") : join(app.getPath("home"), ".claude", ".credentials.json");
}

async function backupAndRemoveFile(sourcePath: string, label: string): Promise<string | undefined> {
  try {
    const info = await stat(sourcePath);
    if (!info.isFile()) {
      return undefined;
    }
  } catch {
    return undefined;
  }
  const backupDir = join(app.getPath("userData"), "provider-reset-backups", new Date().toISOString().replace(/[:.]/g, "-"));
  await mkdir(backupDir, { recursive: true });
  const targetPath = join(backupDir, label);
  await rename(sourcePath, targetPath);
  return targetPath;
}

async function resetLlmProviderRuntime(provider: LlmProviderConnectId): Promise<LlmProviderConnectResult> {
  stopProviderAuthWatcher(provider);
  const profile = providerRuntime[provider];
  const events: string[] = [];

  if (provider === "claude") {
    claudeProvisioningActive = false;
    const backupPath = await backupAndRemoveFile(claudeCredentialsPath(), "claude-credentials.json");
    events.push(backupPath ? "Claude credentials removed from active store" : "Claude credentials already absent");
    profile.account = "Claude Code OAuth";
  } else if (provider === "openrouter") {
    openRouterApiKey = "";
    events.push("OpenRouter credential reset");
    profile.account = "OpenRouter OAuth";
  } else {
    events.push("Codex runtime state reset");
    profile.account = "local Codex auth";
  }

  profile.connected = false;
  profile.models = [];
  profile.reasoning = [];
  profile.quotaLabel = "reset pending";
  profile.events = [...events, "awaiting secure login"];
  profile.proof = hashJson({ provider, reset: true, events: profile.events });
  if (panelsChatBottomState.selectedProvider === profile.composerProvider) {
    panelsChatBottomState.modelIndex = 0;
    panelsChatBottomState.reasoningIndex = 0;
  }
  await persistProviderRuntime();
  const result: LlmProviderConnectResult = {
    provider,
    accepted: true,
    events: profile.events,
    models: [],
    reasoning: [],
    quotaLabel: profile.quotaLabel,
    proofHash: profile.proof
  };
  emitLlmProviderRuntimeEvent(result);
  return result;
}

async function hasClaudeCodeCredentialStore(): Promise<boolean> {
  try {
    const info = await stat(claudeCredentialsPath());
    return info.isFile() && info.size > 0;
  } catch {
    return false;
  }
}

function captureCommand(command: string, args: string[], timeoutMs = 7000): Promise<CommandCaptureResult> {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      shell: process.platform === "win32",
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = (result: CommandCaptureResult) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(result);
    };
    const timer = setTimeout(() => {
      child.kill();
      finish({ exitCode: null, stdout, stderr, error: `${command} timed out` });
    }, timeoutMs);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.once("error", (error) => {
      finish({ exitCode: null, stdout, stderr, error: error.message });
    });
    child.once("close", (exitCode) => {
      finish({ exitCode, stdout, stderr });
    });
  });
}

function firstStringField(source: unknown, keys: string[]): string | undefined {
  if (!source || typeof source !== "object") {
    return undefined;
  }
  const record = source as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  for (const value of Object.values(record)) {
    const nested = firstStringField(value, keys);
    if (nested) {
      return nested;
    }
  }
  return undefined;
}

function parseJsonMaybe(text: string): unknown | undefined {
  const trimmed = text.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    const start = trimmed.indexOf("{");
    const end = trimmed.lastIndexOf("}");
    if (start >= 0 && end > start) {
      try {
        return JSON.parse(trimmed.slice(start, end + 1));
      } catch {
        return undefined;
      }
    }
  }
  return undefined;
}

function collectClaudeModelIds(source: unknown): string[] {
  const modelIds = new Set<string>();
  const visit = (value: unknown, depth = 0) => {
    if (depth > 8 || value == null) {
      return;
    }
    if (typeof value === "string") {
      const pattern = /\bclaude-(?:sonnet|opus|haiku|fable)-[0-9][a-z0-9._-]*(?:-[a-z0-9._-]+)*\b/ig;
      for (const match of value.matchAll(pattern)) {
        modelIds.add(match[0].toLowerCase());
      }
      return;
    }
    if (Array.isArray(value)) {
      value.slice(0, 500).forEach((item) => visit(item, depth + 1));
      return;
    }
    if (typeof value === "object") {
      Object.values(value as Record<string, unknown>).slice(0, 1000).forEach((item) => visit(item, depth + 1));
    }
  };
  visit(source);
  return [...modelIds];
}

function applyClaudeCliProfile(probe: ProviderCliProbe, activateComposer = false): ProviderRuntimeProfile {
  const profile = providerRuntime.claude;
  profile.connected = probe.ok;
  profile.account = probe.account ?? "Claude Code account";
  profile.models = probe.models;
  profile.reasoning = probe.reasoning;
  profile.quotaLabel = probe.quotaLabel;
  profile.runtimeCommand = probe.command;
  profile.runtimeVersion = probe.version;
  profile.runtimeVerified = probe.runtimeVerified === true;
  profile.proof = hashJson({
    provider: "claude",
    account: profile.account,
    models: profile.models,
    reasoning: profile.reasoning,
    quotaLabel: profile.quotaLabel,
    runtimeCommand: profile.runtimeCommand,
    runtimeVersion: profile.runtimeVersion,
    runtimeVerified: profile.runtimeVerified
  });
  if (activateComposer) {
    activateComposerProvider(profile);
  }
  return profile;
}

async function probeClaudeCliProfile(): Promise<ProviderCliProbe> {
  const command = resolveClaudeCodeCommand();
  const events = [`resolve Claude Code runtime on ${hostPlatformLabel()}`];
  if (!command) {
    return {
      ok: false,
      cliFound: false,
      runtimeVerified: false,
      events: [...events, "Claude Code runtime executable not found"],
      models: [],
      reasoning: [],
      quotaLabel: "quota unavailable: Claude Code CLI not found",
      error: "claude CLI not found on PATH or known Windows install locations"
    };
  }
  events.push(`runtime command resolved: ${command}`);
  const versionProbe = await captureCommand(command, ["--version"], 7000);
  events.push(`execute: ${commandLine(command, ["--version"])}`);
  const versionText = firstNonEmptyLine(`${versionProbe.stdout}\n${versionProbe.stderr}`);
  const runtimeVerified = versionProbe.exitCode === 0;
  events.push(runtimeVerified
    ? `runtime version verified: ${versionText ?? "version output unavailable"}`
    : `runtime version probe failed: exit ${versionProbe.exitCode ?? "unknown"}`);
  events.push(`execute: ${commandLine(command, ["auth", "status"])}`);
  const status = await captureCommand(command, ["auth", "status"], 9000);
  const raw = parseJsonMaybe(status.stdout) ?? parseJsonMaybe(status.stderr);
  const account =
    firstStringField(raw, ["email", "accountEmail", "login", "username", "account", "organizationName", "subscriptionType"]) ??
    firstStringField(raw, ["displayName", "name"]);
  const text = `${status.stdout}\n${status.stderr}`;
  const exactModels = collectClaudeModelIds(raw).concat(collectClaudeModelIds(text));
  const loggedIn = runtimeVerified && status.exitCode === 0 && !/not\s+(logged|signed)\s+in|unauthenticated|login required/i.test(text);
  if (!loggedIn) {
    return {
      ok: false,
      cliFound: true,
      command,
      version: versionText,
      runtimeVerified,
      events,
      account,
      models: [],
      reasoning: [],
      quotaLabel: "quota unavailable: Claude Code auth status not connected",
      raw,
      error: status.error ?? text.trim().slice(0, 240)
    };
  }
  return {
    ok: true,
    cliFound: true,
    command,
    version: versionText,
    runtimeVerified,
    events,
    account: account ? `Claude Code ${account}` : "Claude Code account",
    models: exactModels.length > 0 ? [...new Set(exactModels)] : ["sonnet", "opus", "haiku"],
    reasoning: ["low", "medium", "high", "xhigh", "max"],
    quotaLabel: "quota unavailable: Claude Code auth status did not return official token balance",
    raw
  };
}

function stopProviderAuthWatcher(provider: LlmProviderConnectId): void {
  const watcher = providerAuthWatchers.get(provider);
  if (watcher) {
    clearInterval(watcher);
    providerAuthWatchers.delete(provider);
  }
}

function claudeReadyEvents(profile: ProviderRuntimeProfile): string[] {
  const hasExactModelIds = profile.models.some((model) => /^claude-(?:sonnet|opus|haiku|fable)-/i.test(model));
  return [
    claudeRuntimeVerifiedEvent(profile),
    "Claude Code auth status confirmed",
    ...(hasExactModelIds
      ? modelCatalogEvents(profile.models)
      : [`model aliases ${profile.models.join(" / ")} (resolved by Claude Code at runtime)`]),
    profile.reasoning.length > 0 ? `reasoning ${profile.reasoning.join(" / ")}` : "reasoning unavailable",
    profile.quotaLabel,
    "ready"
  ];
}

function claudeReadyEventsFromProbe(profile: ProviderRuntimeProfile, probe: ProviderCliProbe): string[] {
  const probeEvents = probe.events ?? [];
  const installEvent = probe.runtimeVerified ? "installer skipped: Claude Code runtime already executable" : "installer not completed";
  const readyEvents = claudeReadyEvents(profile);
  return [
    ...probeEvents,
    installEvent,
    ...readyEvents.filter((event) => !probeEvents.includes(event))
  ];
}

function startClaudeAuthWatcher(): void {
  stopProviderAuthWatcher("claude");
  let attempts = 0;
  let active = false;
  const watcher = setInterval(() => {
    if (active) {
      return;
    }
    active = true;
    attempts += 1;
    void (async () => {
      try {
        const probe = await probeClaudeCliProfile();
        if (probe.ok) {
          const profile = applyClaudeCliProfile(probe, true);
          await persistProviderRuntime();
          const events = claudeReadyEventsFromProbe(profile, probe);
          emitLlmProviderRuntimeEvent({
            provider: "claude",
            events,
            models: profile.models,
            reasoning: profile.reasoning,
            quotaLabel: profile.quotaLabel,
            proofHash: profile.proof
          });
          stopProviderAuthWatcher("claude");
        } else if (attempts >= 90) {
          emitLlmProviderRuntimeEvent({
            provider: "claude",
            events: ["Claude Code auth status not confirmed", "not ready"],
            models: [],
            reasoning: [],
            quotaLabel: "pending",
            proofHash: hashJson({ provider: "claude", confirmed: false, attempts })
          });
          stopProviderAuthWatcher("claude");
        }
      } catch (error) {
        console.error("Claude Code auth watcher failed.", error);
      } finally {
        active = false;
      }
    })();
  }, 2500);
  providerAuthWatchers.set("claude", watcher);
}

function claudeInstallCommand(): { command: string; args: string[]; label: string } | undefined {
  if (process.platform === "win32") {
    return {
      command: "powershell.exe",
      args: ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "irm https://claude.ai/install.ps1 | iex"],
      label: "run official Claude Code installer in background"
    };
  }
  if (process.platform === "darwin" || process.platform === "linux") {
    return {
      command: "bash",
      args: ["-lc", "curl -fsSL https://claude.ai/install.sh | bash"],
      label: "run official Claude Code installer in background"
    };
  }
  return undefined;
}

function appendLimitedEvent(events: string[], event: string): string[] {
  return [...events, event].slice(-18);
}

function startClaudeCodeProvisioning(initialEvents: string[]): void {
  if (claudeProvisioningActive) {
    emitLlmProviderRuntimeEvent({
      provider: "claude",
      events: appendLimitedEvent(initialEvents, "Claude Code provisioning already running"),
      models: [],
      reasoning: [],
      quotaLabel: "pending",
      proofHash: hashJson({ provider: "claude", provisioning: "already_running" })
    });
    return;
  }
  const install = claudeInstallCommand();
  if (!install) {
    emitLlmProviderRuntimeEvent({
      provider: "claude",
      events: appendLimitedEvent(initialEvents, "Claude Code automatic install unsupported on this platform").concat("not ready"),
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      proofHash: hashJson({ provider: "claude", provisioning: "unsupported", platform: process.platform })
    });
    return;
  }

  claudeProvisioningActive = true;
  let events = appendLimitedEvent(initialEvents, install.label);
  events = appendLimitedEvent(events, `installer command: ${commandLine(install.command, install.args)}`);
  emitLlmProviderRuntimeEvent({
    provider: "claude",
    events,
    models: [],
    reasoning: [],
    quotaLabel: "pending",
    proofHash: hashJson({ provider: "claude", provisioning: "install_started", command: install.command })
  });

  const child = spawn(install.command, install.args, {
    shell: false,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true
  });

  const pushOutput = (label: string, chunk: Buffer | string) => {
    const line = chunk.toString("utf8").split(/\r?\n/).map((value) => value.trim()).find(Boolean);
    if (!line) {
      return;
    }
    events = appendLimitedEvent(events, `${label}: ${line.slice(0, 140)}`);
    emitLlmProviderRuntimeEvent({
      provider: "claude",
      events,
      models: [],
      reasoning: [],
      quotaLabel: "pending",
      proofHash: hashJson({ provider: "claude", provisioning: "install_output", events })
    });
  };

  child.stdout.on("data", (chunk: Buffer) => pushOutput("installer", chunk));
  child.stderr.on("data", (chunk: Buffer) => pushOutput("installer", chunk));
  child.once("error", (error) => {
    claudeProvisioningActive = false;
    events = appendLimitedEvent(events, `Claude Code installer failed: ${error.message}`).concat("not ready");
    emitLlmProviderRuntimeEvent({
      provider: "claude",
      events,
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      proofHash: hashJson({ provider: "claude", provisioning: "install_error", error: error.message })
    });
  });
  child.once("close", (exitCode) => {
    void (async () => {
      claudeProvisioningActive = false;
      if (exitCode !== 0) {
        events = appendLimitedEvent(events, `Claude Code installer exited ${exitCode ?? "without status"}`).concat("not ready");
        emitLlmProviderRuntimeEvent({
          provider: "claude",
          events,
          models: [],
          reasoning: [],
          quotaLabel: "unavailable",
          proofHash: hashJson({ provider: "claude", provisioning: "install_exit", exitCode })
        });
        return;
      }

      events = appendLimitedEvent(events, `Claude Code installer completed with exit code ${exitCode}`);
      const probe = await probeClaudeCliProfile();
      for (const probeEvent of probe.events ?? []) {
        events = appendLimitedEvent(events, probeEvent);
      }
      if (!probe.runtimeVerified) {
        events = appendLimitedEvent(events, "Claude Code install finished but runtime verification failed").concat("not ready");
        emitLlmProviderRuntimeEvent({
          provider: "claude",
          events,
          models: [],
          reasoning: [],
          quotaLabel: "unavailable",
          proofHash: hashJson({ provider: "claude", provisioning: "runtime_not_verified", probe })
        });
        return;
      }
      events = appendLimitedEvent(events, `Claude Code runtime installed verified on ${hostPlatformLabel()}: ${probe.command ?? "claude"}${probe.version ? ` (${probe.version})` : ""}`);
      if (probe.ok) {
        const profile = applyClaudeCliProfile(probe, true);
        const readyEvents = appendLimitedEvent(events, "Claude Code auth status confirmed")
          .concat(claudeReadyEvents(profile).filter((event) =>
            event !== "Claude Code auth status confirmed" &&
            !event.startsWith("Claude Code runtime verified")
          ));
        markProviderReadyEvents(profile, readyEvents);
        await persistProviderRuntime();
        emitLlmProviderRuntimeEvent({
          provider: "claude",
          events: readyEvents,
          models: profile.models,
          reasoning: profile.reasoning,
          quotaLabel: profile.quotaLabel,
          proofHash: profile.proof
        });
        return;
      }

      const launch = await launchProviderAuth("claude");
      events = appendLimitedEvent(events, launch.launched ? "open Claude Code auth login" : launch.error ?? "Claude Code auth login failed");
      emitLlmProviderRuntimeEvent({
        provider: "claude",
        events: appendLimitedEvent(events, "waiting Claude Code auth status"),
        models: [],
        reasoning: [],
        quotaLabel: "pending",
        proofHash: hashJson({ provider: "claude", provisioning: "install_done_auth_pending", launched: launch.launched })
      });
      if (launch.launched) {
        startClaudeAuthWatcher();
      }
    })().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "Claude Code provisioning failed.";
      claudeProvisioningActive = false;
      emitLlmProviderRuntimeEvent({
        provider: "claude",
        events: appendLimitedEvent(events, message).concat("not ready"),
        models: [],
        reasoning: [],
        quotaLabel: "unavailable",
        proofHash: hashJson({ provider: "claude", provisioning: "failed", message })
      });
    });
  });
}

function launchDetached(command: string, args: string[], eventLabel: string): Promise<ProviderLaunchResult> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (result: ProviderLaunchResult) => {
      if (!settled) {
        settled = true;
        resolve(result);
      }
    };

    if (!commandExists(command)) {
      finish({
        launched: false,
        events: [`${command} CLI not found on PATH`],
        error: `${command} CLI not found on PATH.`
      });
      return;
    }

    try {
      const child = spawn(command, args, {
        detached: true,
        shell: process.platform === "win32",
        stdio: "ignore",
        windowsHide: false
      });
      child.once("spawn", () => {
        child.unref();
        finish({
          launched: true,
          events: [eventLabel]
        });
      });
      child.once("error", (error) => {
        finish({
          launched: false,
          events: [`failed to launch ${command}`, error.message],
          error: error.message
        });
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : `Failed to launch ${command}.`;
      finish({
        launched: false,
        events: [`failed to launch ${command}`, message],
        error: message
      });
    }
  });
}

async function launchProviderAuth(provider: LlmProviderConnectId): Promise<ProviderLaunchResult> {
  const flow = llmProviderOfficialFlows[provider];
  if (provider === "codex") {
    return openProviderAuthWindow(provider, CHATGPT_HOME_URL, "OpenAI OAuth Direct");
  }

  if (provider === "claude") {
    const command = resolveClaudeCodeCommand() ?? "claude";
    const launch = await launchDetached(command, ["auth", "login"], "open Claude Code auth login");
    return launch;
  }

  try {
    await shell.openExternal(flow.url);
    return {
      launched: true,
      events: ["open official OpenRouter auth page"]
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to open official OpenRouter auth page.";
    return {
      launched: false,
      events: ["failed to open official OpenRouter auth page", message],
      error: message
    };
  }
}

async function eveReaderProviderProfile(provider: LlmProviderConnectId): Promise<ProviderRuntimeProfile | undefined> {
  const profile = providerRuntime[provider];
  return profile.connected ? profile : undefined;
}

async function connectClaudeProvider(): Promise<LlmProviderConnectResult> {
  const probe = await probeClaudeCliProfile();
  if (probe.ok) {
    const profile = applyClaudeCliProfile(probe, true);
    const events = claudeReadyEventsFromProbe(profile, probe);
    markProviderReadyEvents(profile, events);
    await persistProviderRuntime();
    return {
      provider: "claude",
      accepted: true,
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    };
  }

  if (!probe.cliFound) {
    const hasCredentialStore = await hasClaudeCodeCredentialStore();
    const events = [
      ...(hasCredentialStore ? ["Claude credential store detected"] : []),
      "Claude Code CLI not found on PATH or known Windows install locations",
      "provision Claude Code runtime inside LLM Provider"
    ];
    providerRuntime.claude.connected = false;
    providerRuntime.claude.events = events;
    providerRuntime.claude.models = [];
    providerRuntime.claude.reasoning = [];
    providerRuntime.claude.quotaLabel = "pending";
    providerRuntime.claude.proof = hashJson({ provider: "claude", cliFound: false, credentialStore: hasCredentialStore, events });
    startClaudeCodeProvisioning(events);
    return {
      provider: "claude",
      accepted: true,
      events,
      models: [],
      reasoning: [],
      quotaLabel: "pending",
      proofHash: providerRuntime.claude.proof
    };
  }

  const launch = await launchProviderAuth("claude");
  if (!launch.launched) {
    const events = [...launch.events, "not ready"];
    return {
      provider: "claude",
      accepted: false,
      events,
      models: [],
      reasoning: [],
      quotaLabel: probe.quotaLabel,
      error: {
        code: "rust_unavailable",
        message: launch.error ?? "Claude Code auth login failed to launch.",
        proofHash: hashJson({ provider: "claude", launch })
      },
      proofHash: hashJson({ provider: "claude", accepted: false, events })
    };
  }

  startClaudeAuthWatcher();
  const events = [
    ...launch.events,
    "waiting Claude Code auth status",
    "waiting official Claude account session"
  ];
  return {
    provider: "claude",
    accepted: true,
    events,
    models: [],
    reasoning: [],
    quotaLabel: "pending",
    proofHash: hashJson({ provider: "claude", launched: true, confirmed: false, events })
  };
}

async function completeOpenRouterOAuth(waiter: OpenRouterOAuthWaiter): Promise<void> {
  try {
    const code = await waiter.codePromise;
    emitLlmProviderRuntimeEvent({
      provider: "openrouter",
      events: ["OpenRouter callback received", "exchange code for user-controlled API key"],
      models: [],
      reasoning: [],
      quotaLabel: "pending",
      proofHash: hashJson({ provider: "openrouter", callback: true })
    });
    openRouterApiKey = await exchangeOpenRouterCodeForKey(code, waiter.codeVerifier);
    const probe = await probeOpenRouterApiKey(openRouterApiKey);
    const profile = applyOpenRouterProbe(probe, true);
    if (!probe.ok) {
      await persistProviderRuntime();
      emitLlmProviderRuntimeEvent({
        provider: "openrouter",
        events: ["OpenRouter key received", probe.error ?? "OpenRouter API probe failed", "not ready"],
        models: [],
        reasoning: [],
        quotaLabel: probe.quotaLabel,
        proofHash: probe.proof
      });
      return;
    }
    const events = openRouterReadyEvents(profile);
    markProviderReadyEvents(profile, events);
    await persistProviderRuntime();
    emitLlmProviderRuntimeEvent({
      provider: "openrouter",
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: profile.proof
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "OpenRouter OAuth failed.";
    emitLlmProviderRuntimeEvent({
      provider: "openrouter",
      events: ["OpenRouter OAuth failed", message, "not ready"],
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      proofHash: hashJson({ provider: "openrouter", error: message })
    });
  } finally {
    waiter.close();
  }
}

async function connectOpenRouterProvider(): Promise<LlmProviderConnectResult> {
  if (openRouterApiKey) {
    const probe = await probeOpenRouterApiKey(openRouterApiKey);
    const profile = applyOpenRouterProbe(probe);
    if (probe.ok) {
      const events = openRouterReadyEvents(profile);
      markProviderReadyEvents(profile, events);
      await persistProviderRuntime();
      return {
        provider: "openrouter",
        accepted: true,
        events,
        models: profile.models,
        reasoning: profile.reasoning,
        quotaLabel: profile.quotaLabel,
        proofHash: profile.proof
      };
    }
  }

  const waiter = await startOpenRouterOAuthWaiter();
  const authUrl = `https://openrouter.ai/auth?callback_url=${encodeURIComponent(waiter.callbackUrl)}&code_challenge=${encodeURIComponent(waiter.codeChallenge)}&code_challenge_method=S256`;
  const launch = openProviderAuthWindow("openrouter", authUrl, "OpenRouter OAuth PKCE");
  if (!launch.launched) {
    waiter.close();
    const events = [...launch.events, "not ready"];
    return {
      provider: "openrouter",
      accepted: false,
      events,
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      error: {
        code: "rust_unavailable",
        message: launch.error ?? "OpenRouter OAuth failed to launch.",
        proofHash: hashJson({ provider: "openrouter", launch })
      },
      proofHash: hashJson({ provider: "openrouter", accepted: false, events })
    };
  }

  void completeOpenRouterOAuth(waiter);
  const events = [
    ...launch.events,
    "waiting OpenRouter OAuth callback",
    "waiting credential seal"
  ];
  return {
    provider: "openrouter",
    accepted: true,
    events,
    models: [],
    reasoning: [],
    quotaLabel: "pending",
    proofHash: hashJson({ provider: "openrouter", launched: true, callbackUrl: waiter.callbackUrl, events })
  };
}

async function connectLlmProvider(provider: unknown): Promise<LlmProviderConnectResult> {
  if (!isLlmProviderConnectId(provider)) {
    const proofHash = hashJson({ provider, accepted: false, reason: "bad_provider" });
    return {
      provider: "codex",
      accepted: false,
      events: ["rejected bad provider"],
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      error: {
        code: "bad_payload",
        message: "LLM provider failed IPC validation.",
        proofHash
      },
      proofHash
    };
  }

  const flow = llmProviderOfficialFlows[provider];
  try {
    if (provider === "claude") {
      return connectClaudeProvider();
    }
    if (provider === "openrouter") {
      return connectOpenRouterProvider();
    }
    if (provider === "codex") {
      const localProfile = await applyCodexLocalAuthProfile(["connect Codex local OAuth"]);
      if (localProfile) {
        return {
          provider,
          accepted: true,
          events: localProfile.events,
          models: localProfile.models,
          reasoning: localProfile.reasoning,
          quotaLabel: localProfile.quotaLabel,
          proofHash: localProfile.proof
        };
      }
    }

    const launch = await launchProviderAuth(provider);
    if (!launch.launched) {
      const proofHash = hashJson({ provider, accepted: false, events: launch.events, message: launch.error });
      return {
        provider,
        accepted: false,
        events: [...launch.events, "not ready"],
        models: [],
        reasoning: [],
        quotaLabel: "unavailable",
        error: {
          code: "rust_unavailable",
          message: launch.error ?? "Provider auth launch failed.",
          proofHash
        },
        proofHash
      };
    }

    const profile = await eveReaderProviderProfile(provider);
    if (!profile) {
      const events = provider === "codex"
        ? [
            ...launch.events,
            "waiting ChatGPT subscription session"
          ]
        : [
            ...launch.events,
            ...flow.events,
            "waiting eve_reader confirmation"
          ];
      return {
        provider,
        accepted: true,
        events,
        models: [],
        reasoning: [],
        quotaLabel: "pending",
        proofHash: hashJson({ provider, launched: true, confirmed: false, url: flow.url, events })
      };
    }

    profile.connected = true;
    profile.proof = hashJson({ provider, account: profile.account, models: profile.models, reasoning: profile.reasoning });
    activateComposerProvider(profile);
    const events = connectedProviderEvents(flow.events, profile);
    markProviderReadyEvents(profile, events);
    void persistProviderRuntime().catch((error: unknown) => {
      console.error(`Failed to persist ${provider} provider runtime.`, error);
    });
    return {
      provider,
      accepted: true,
      events,
      models: profile.models,
      reasoning: profile.reasoning,
      quotaLabel: profile.quotaLabel,
      proofHash: hashJson({ provider, url: flow.url, events, models: profile.models, reasoning: profile.reasoning })
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to launch official provider flow.";
    const proofHash = hashJson({ provider, accepted: false, message });
    return {
      provider,
      accepted: false,
      events: ["launch official provider flow", "failed", "not ready"],
      models: [],
      reasoning: [],
      quotaLabel: "unavailable",
      error: {
        code: "rust_unavailable",
        message,
        proofHash
      },
      proofHash
    };
  }
}

function cutoverMode(
  slice: "header" | "sidebar" | "panels_chat_bottom" | "canvas_surfaces" | "right_panel" = "header"
): FrontSliceMode {
  const raw =
    slice === "sidebar"
      ? process.env.FORGE_FRONT_SLICE_SIDEBAR
      : slice === "panels_chat_bottom"
        ? process.env.FORGE_FRONT_SLICE_PANELS_CHAT_BOTTOM
        : slice === "canvas_surfaces"
          ? process.env.FORGE_FRONT_SLICE_CANVAS
          : slice === "right_panel"
            ? process.env.FORGE_FRONT_SLICE_RIGHT_PANEL
          : process.env.FORGE_FRONT_SLICE_HEADER;
  return raw === "electron" || raw === "shadow" ? raw : "electron";
}

function canvasSurface(input: Omit<CanvasSurfaceSummary, "proofHash">): CanvasSurfaceSummary {
  const surface: CanvasSurfaceSummary = {
    ...input,
    proofHash: ""
  };
  surface.proofHash = hashJson({ ...surface, proofHash: "" });
  return surface;
}

function canvasSurfacesSnapshot(): CanvasSurfacesSnapshot {
  const backend = rustBackend();
  const mode = cutoverMode("canvas_surfaces");
  const authority = "rust";
  const surfaces: CanvasSurfaceSummary[] = [
    canvasSurface({
      id: "forge-drop-canvas",
      kind: "drop_canvas",
      label: "Forge drop canvas",
      route: "forge",
      status: "ipc_ready",
      sourceComponent: "DropCanvas",
      nativeContract: "rust-forge-canvas-projection",
      authority,
      headline: "Forge canvas",
      detail: `${backend.nativeStatus.stateOwner}; ${backend.nativeStatus.monster}.`
    }),
    canvasSurface({
      id: "webexplorer-webview-host",
      kind: "webexplorer_webview",
      label: "Rust WebView host",
      route: "webexplorer",
      status: "native_ready",
      sourceComponent: "GoogleWebViewCanvas",
      nativeContract: "rust-owned-webview-policy-host",
      authority,
      headline: "WebExplorer native web peripheral",
      detail: backend.nativeStatus.webexplorer
    }),
    canvasSurface({
      id: "banger-native-child-surface",
      kind: "banger_native_child",
      label: "Banger native viewport",
      route: "banger",
      status: "native_ready",
      sourceComponent: "BangerNativeViewport",
      nativeContract: "wgpu-child-window-frame-hash",
      authority,
      headline: "Banger child window slot",
      detail: backend.nativeStatus.banger
    }),
    canvasSurface({
      id: "profile-canvas",
      kind: "profile_surface",
      label: "PoolClaw profile",
      route: "profile",
      status: "native_ready",
      sourceComponent: "PoolClawProfileSurface",
      nativeContract: "profile-state-projection",
      authority,
      headline: "Profile canvas",
      detail: `Profile projection served by ${backend.source}.`
    }),
    canvasSurface({
      id: "llm-providers-canvas",
      kind: "llm_providers",
      label: "LLM providers",
      route: "llm",
      status: "native_ready",
      sourceComponent: "LlmProvidersSurface",
      nativeContract: "provider-keychain-policy",
      authority,
      headline: "Provider accounts",
      detail: backend.nativeStatus.provider
    }),
    canvasSurface({
      id: "brain-canvas",
      kind: "brain_surface",
      label: "Brain surface",
      route: "brain",
      status: "native_ready",
      sourceComponent: "BrainSurface",
      nativeContract: "brain-evidence-projection",
      authority,
      headline: "Brain evidence canvas",
      detail: backend.nativeStatus.brain
    }),
    canvasSurface({
      id: `${headerState.activeSection}-product-section`,
      kind: "product_section",
      label: "Product section surface",
      route: headerState.activeSection,
      status: "native_ready",
      sourceComponent: "ProductSectionSurface",
      nativeContract: "domain-service-proof-projection",
      authority,
      headline: "Domain product projection",
      detail: backend.nativeStatus.proof
    })
  ];
  const activeSurface =
    surfaces.find((surface) => surface.route === headerState.profileCanvas && headerState.profileCanvas !== "") ??
    surfaces.find((surface) => surface.route === headerState.activeSection) ??
    surfaces[0];
  const snapshot: CanvasSurfacesSnapshot = {
    schema: "ingen.electron.canvas_surfaces.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection,
    profileCanvas: headerState.profileCanvas,
    activeSurfaceId: activeSurface?.id ?? "forge-drop-canvas",
    surfaces,
    nativeSurfacePolicy: {
      banger: "child-window",
      webexplorer: "rust-owned-webview"
    },
    proofHash: ""
  };
  snapshot.proofHash = hashJson({ ...snapshot, proofHash: "" });
  return snapshot;
}

function applyCanvasSurfacesCommand(command: CanvasSurfacesCommand): void {
  switch (command.kind) {
    case "open_profile_canvas":
      if (command.canvas !== undefined) {
        headerState.profileCanvas = command.canvas;
        sidebarState.profileOpen = false;
      }
      break;
    case "request_native_surface":
      if (command.section !== undefined) {
        closeProfileCanvas();
        headerState.activeSection = command.section;
        headerState.sectionTitle = sectionTitle(command.section);
      }
      break;
    case "activate_control":
      closeProfileCanvas();
      break;
    case "refresh_surface_proofs":
      break;
  }
}

function canvasSurfacesCommandResult(
  command: CanvasSurfacesCommand,
  accepted: boolean,
  mode: FrontSliceMode
): CanvasSurfacesCommandResult {
  const result: CanvasSurfacesCommandResult = {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted,
    mode,
    event: accepted ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: ""
  };
  if (!accepted && mode === "shadow") {
    result.error = {
      code: "shadow_only",
      message: "Canvas surfaces are locked in explicit shadow rollback mode.",
      proofHash: hashJson({ mode, command })
    };
  }
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function rightPanelLine(input: Omit<RightPanelLine, "proofHash">): RightPanelLine {
  const line: RightPanelLine = {
    ...input,
    proofHash: ""
  };
  line.proofHash = hashJson({ ...line, proofHash: "" });
  return line;
}

function rightPanelSnapshot(): RightPanelSnapshot {
  const backend = rustBackend();
  const mode = cutoverMode("right_panel");
  const bangerActive = headerState.activeSection === "banger";
  const webActive = headerState.activeSection === "webexplorer";
  const tradingActive = headerState.activeSection === "trading";
  const tabs = [
    { id: "status", label: "Status", selected: rightPanelState.activeTab === "status", count: 4 },
    { id: "proofs", label: "Proofs", selected: rightPanelState.activeTab === "proofs", count: 3 },
    { id: "native", label: "Native", selected: rightPanelState.activeTab === "native", count: bangerActive || webActive ? 2 : 1 }
  ];
  const lines: RightPanelLine[] =
    rightPanelState.activeTab === "native"
      ? [
          rightPanelLine({
            label: "Banger",
            value: backend.nativeStatus.banger,
            severity: "ok"
          }),
          rightPanelLine({
            label: "WebExplorer",
            value: backend.nativeStatus.webexplorer,
            severity: "ok"
          }),
          rightPanelLine({
            label: "Renderer",
            value: backend.nativeStatus.stateOwner,
            severity: "ok"
          })
        ]
      : rightPanelState.activeTab === "proofs"
        ? [
            rightPanelLine({ label: "Backend", value: backend.backend, severity: "ok" }),
            rightPanelLine({ label: "IPC", value: "typed Rust contract generated", severity: "ok" }),
            rightPanelLine({ label: "Canvas", value: "snapshot_exported=true command_exported=true", severity: "ok" }),
            rightPanelLine({ label: "Visual", value: "promotion gate deferred", severity: "info" })
          ]
        : [
            rightPanelLine({ label: "Section", value: headerState.activeSection, severity: "ok" }),
            rightPanelLine({ label: "Canvas", value: headerState.profileCanvas || "workspace", severity: "info" }),
            rightPanelLine({ label: "Jobs", value: backend.nativeStatus.jobs, severity: "ok" }),
            rightPanelLine({
              label: "Domain",
              value: tradingActive ? "Forge Trading shadow projection" : "Forge native shell projection",
              severity: tradingActive ? "warn" : "info"
            })
          ];
  const actions: RightPanelAction[] = [
    { id: "refresh", label: "Refresh", command: "refresh", enabled: true },
    { id: "status", label: "Status", command: "select_tab", enabled: rightPanelState.activeTab !== "status" },
    { id: "proofs", label: "Proofs", command: "select_tab", enabled: rightPanelState.activeTab !== "proofs" },
    { id: "native", label: "Native", command: "select_tab", enabled: rightPanelState.activeTab !== "native" }
  ];
  const snapshot: RightPanelSnapshot = {
    schema: "ingen.electron.right_panel.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection,
    profileCanvas: headerState.profileCanvas,
    open: headerState.rightPanelOpen,
    activeTab: rightPanelState.activeTab,
    title: webActive ? "WebExplorer proof dock" : bangerActive ? "Banger proof dock" : "Section status dock",
    summary:
      `Right panel is served by ${backend.source}; Rust remains authority for native probes.`,
    tabs,
    lines,
    actions,
    proofHash: ""
  };
  snapshot.proofHash = hashJson({ ...snapshot, backendProofHash: backend.proofHash, proofHash: "" });
  return snapshot;
}

function applyRightPanelCommand(command: RightPanelCommand): void {
  rightPanelState.lastControl = command.kind;
  switch (command.kind) {
    case "toggle_panel":
      headerState.rightPanelOpen = !headerState.rightPanelOpen;
      break;
    case "select_tab":
      if (command.target === "status" || command.target === "proofs" || command.target === "native") {
        rightPanelState.activeTab = command.target;
      }
      break;
    case "activate_control":
    case "refresh":
      break;
  }
}

function rightPanelCommandResult(
  command: RightPanelCommand,
  accepted: boolean,
  mode: FrontSliceMode
): RightPanelCommandResult {
  const result: RightPanelCommandResult = {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted,
    mode,
    event: accepted ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: ""
  };
  if (!accepted && mode === "shadow") {
    result.error = {
      code: "shadow_only",
      message: "Right panel is locked in explicit shadow rollback mode.",
      proofHash: hashJson({ mode, command })
    };
  }
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function validateSender(event: Electron.IpcMainInvokeEvent): boolean {
  const url = event.senderFrame?.url ?? "";
  return (
    url.startsWith("ingen://renderer/") ||
    url.startsWith("http://127.0.0.1:") ||
    BrowserWindow.fromWebContents(event.sender) !== null
  );
}

function senderNativeWindow(event: Electron.IpcMainInvokeEvent): BrowserWindow | undefined {
  return (
    BrowserWindow.fromWebContents(event.sender) ??
    BrowserWindow.getFocusedWindow() ??
    BrowserWindow.getAllWindows()[0]
  );
}

function nativeWebExplorerResult(accepted: boolean, error?: IpcError): NativeWebExplorerResult {
  const result: NativeWebExplorerResult = {
    accepted,
    url: nativeWebExplorerTargetUrl,
    proofHash: ""
  };
  if (error) {
    result.error = error;
  }
  result.proofHash = hashJson({ nativeWebExplorer: result, proofHash: "" });
  return result;
}

function nativeMapsResult(accepted: boolean, error?: IpcError): NativeWebExplorerResult {
  const result: NativeWebExplorerResult = {
    accepted,
    url: nativeMapsTargetUrl,
    proofHash: ""
  };
  if (error) {
    result.error = error;
  }
  result.proofHash = hashJson({ nativeMaps: result, proofHash: "" });
  return result;
}

const DOM_RAM_ARTIFACT_CONTRACTS: Array<Omit<NativeDomRamArtifactSummary, "liveSliceHash" | "byteLength" | "recordCount">> = [
  {
    kind: "dom_graph_page",
    layout: "live_cdp_domsnapshot_csr_graph_incremental_node_edge_u64_records",
    liveCapturePolicy: "cdp_domsnapshot_incremental_csr_capture_via_idle_slices",
    liveBackpressurePolicy: "pause_capture_when_longtask_or_owner_queue_depth_exceeds_budget",
    liveSectionOwner: "webexplorer.dom_ram_cartography"
  },
  {
    kind: "ram_region_table",
    layout: "live_columnar_ram_region_table_resumable_hash_offset_len_flags",
    liveCapturePolicy: "columnar_ram_region_incremental_capture_with_resume_cursor",
    liveBackpressurePolicy: "halve_region_batch_when_event_loop_budget_below_threshold",
    liveSectionOwner: "webexplorer.dom_ram_cartography"
  },
  {
    kind: "browser_event_loop_slice",
    layout: "live_nonblocking_browser_event_loop_slice_backpressure_manifest",
    liveCapturePolicy: "scheduler_posttask_or_idlecallback_budgeted_nonblocking_slice",
    liveBackpressurePolicy: "yield_before_deadline_and_resume_from_cursor",
    liveSectionOwner: "webexplorer.dom_ram_cartography"
  }
];

function emptyMapsDomRamCartographyResult(error: IpcError): NativeDomRamCartographyResult {
  const result: NativeDomRamCartographyResult = {
    accepted: false,
    schema: "forge.webexplorer.dom_ram_cartography.v1",
    target: "google_earth",
    url: nativeMapsTargetUrl,
    lane: "native_tandem_dom_ram",
    nativeDomain: "dom_ram",
    engine: "monster_native_tandem",
    snapshot: {
      source: "cdp_domsnapshot",
      documentCount: 0,
      nodeCount: 0,
      layoutCount: 0,
      textBoxCount: 0,
      scrollOffsetX: 0,
      scrollOffsetY: 0,
      captureHash: hashJson({ empty: true, reason: error.code })
    },
    uiTree: {
      schema: "forge.webexplorer.dom_ram_ui_tree.v1",
      nodeCount: 0,
      nodes: [],
      landmarks: {
        searchCandidates: []
      },
      treeHash: hashJson({ empty: true, reason: error.code })
    },
    memory: {
      source: "electron_webcontents",
      workingSetSizeKb: 0,
      peakWorkingSetSizeKb: 0,
      privateBytesKb: 0,
      sharedBytesKb: 0,
      processId: 0,
      processType: "unavailable",
      regionTableHash: hashJson({ empty: true, reason: error.code })
    },
    artifacts: [],
    manifestHash: "",
    proofHash: "",
    error
  };
  result.manifestHash = hashJson({ ...result, proofHash: "", manifestHash: "" });
  result.proofHash = hashJson(result);
  return result;
}

function isMapsWebviewAttachment(src: string, partition?: string): boolean {
  if (partition === "persist:ingen-maps") {
    return true;
  }
  try {
    const parsed = new URL(src);
    return parsed.protocol === "https:" && parsed.hostname === "earth.google.com" && parsed.pathname.startsWith("/web");
  } catch {
    return false;
  }
}

function rememberMapsDomWebviewGuest(webContents: Electron.WebContents, src = ""): void {
  if (webContents.isDestroyed()) {
    return;
  }
  mapsDomWebviewGuest = webContents;
  mapsDomWebviewGuestUrl = src || webContents.getURL() || nativeMapsTargetUrl;
  const updateUrl = () => {
    if (!webContents.isDestroyed()) {
      mapsDomWebviewGuestUrl = webContents.getURL() || mapsDomWebviewGuestUrl;
      clearGoogleEarthSearchLock(webContents);
    }
  };
  webContents.on("did-navigate", updateUrl);
  webContents.on("did-navigate-in-page", updateUrl);
  webContents.on("did-finish-load", updateUrl);
  webContents.once("destroyed", () => {
    if (mapsDomWebviewGuest === webContents) {
      mapsDomWebviewGuest = null;
      mapsDomWebviewGuestUrl = "";
    }
    clearGoogleEarthSearchLock(webContents);
  });
}

function mapsCartographyWebContents(): Electron.WebContents | null {
  if (nativeMapsView && !nativeMapsView.webContents.isDestroyed()) {
    return nativeMapsView.webContents;
  }
  if (mapsDomWebviewGuest && !mapsDomWebviewGuest.isDestroyed()) {
    return mapsDomWebviewGuest;
  }
  return null;
}

function cachedGoogleEarthSearchLockFor(webContents: Electron.WebContents): GoogleEarthSearchLock | null {
  if (!googleEarthSearchLock || googleEarthSearchLock.webContentsId !== webContents.id || webContents.isDestroyed()) {
    return null;
  }
  return googleEarthSearchLock;
}

function rememberGoogleEarthSearchLock(webContents: Electron.WebContents, landmark: NativeDomRamCartographyResult["uiTree"]["landmarks"]["googleEarthSearchBar"]): void {
  if (!landmark?.backendNodeId || webContents.isDestroyed()) {
    return;
  }
  googleEarthSearchLock = {
    webContentsId: webContents.id,
    url: webContents.getURL() || nativeMapsTargetUrl,
    backendNodeId: landmark.backendNodeId,
    layout: landmark.layout,
    lockedAt: Date.now()
  };
}

function clearGoogleEarthSearchLock(webContents?: Electron.WebContents): void {
  if (!webContents || googleEarthSearchLock?.webContentsId === webContents.id) {
    googleEarthSearchLock = null;
  }
}

function numberField(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function domSnapshotCounts(snapshot: unknown) {
  const documents = Array.isArray((snapshot as { documents?: unknown }).documents)
    ? ((snapshot as { documents: unknown[] }).documents)
    : [];
  let nodeCount = 0;
  let layoutCount = 0;
  let textBoxCount = 0;
  let scrollOffsetX = 0;
  let scrollOffsetY = 0;
  for (const document of documents) {
    const doc = document as {
      nodes?: { nodeName?: unknown[] };
      layout?: { nodeIndex?: unknown[] };
      textBoxes?: { layoutIndex?: unknown[] };
      scrollOffsetX?: unknown;
      scrollOffsetY?: unknown;
    };
    nodeCount += Array.isArray(doc.nodes?.nodeName) ? doc.nodes.nodeName.length : 0;
    layoutCount += Array.isArray(doc.layout?.nodeIndex) ? doc.layout.nodeIndex.length : 0;
    textBoxCount += Array.isArray(doc.textBoxes?.layoutIndex) ? doc.textBoxes.layoutIndex.length : 0;
    scrollOffsetX += numberField(doc.scrollOffsetX);
    scrollOffsetY += numberField(doc.scrollOffsetY);
  }
  return {
    documentCount: documents.length,
    nodeCount,
    layoutCount,
    textBoxCount,
    scrollOffsetX,
    scrollOffsetY
  };
}

function cdpString(strings: unknown, index: unknown): string {
  if (!Array.isArray(strings) || typeof index !== "number" || index < 0 || index >= strings.length) {
    return "";
  }
  const value = strings[index];
  return typeof value === "string" ? value : "";
}

function compactDomText(value: string, maxLength = 180): string {
  const clean = value.replace(/\s+/g, " ").trim();
  return clean.length <= maxLength ? clean : `${clean.slice(0, maxLength - 1)}…`;
}

function domRamSearchText(node: NativeDomRamUiTreeNode): string {
  return [
    node.nodeName,
    node.nodeValue,
    node.layout?.text,
    ...Object.entries(node.attributes).flat()
  ].filter(Boolean).join(" ").toLowerCase();
}

function googleEarthSearchBarLandmarks(nodes: NativeDomRamUiTreeNode[]): NativeDomRamCartographyResult["uiTree"]["landmarks"] {
  const candidates = nodes
    .map((node) => {
      const text = domRamSearchText(node);
      const nodeName = node.nodeName.toLowerCase();
      const role = node.attributes.role?.toLowerCase() ?? "";
      const type = node.attributes.type?.toLowerCase() ?? "";
      const editable =
        nodeName === "input" ||
        nodeName === "textarea" ||
        node.attributes.contenteditable === "true" ||
        role === "combobox" ||
        role === "searchbox";
      let confidence = 0;
      const reasons: string[] = [];
      if (editable) {
        confidence += 0.3;
        reasons.push("editable control");
      }
      if (type === "search" || role === "searchbox") {
        confidence += 0.18;
        reasons.push("search role/type");
      }
      if (text.includes("google earth")) {
        confidence += 0.24;
        reasons.push("Google Earth label");
      }
      if (text.includes("rechercher") || text.includes("search")) {
        confidence += 0.18;
        reasons.push("search wording");
      }
      if (text.includes("combobox")) {
        confidence += 0.08;
        reasons.push("combobox marker");
      }
      if (node.visible) {
        confidence += 0.08;
        reasons.push("visible layout");
      }
      if (node.layout && node.layout.y <= 180 && node.layout.width >= 120) {
        confidence += 0.08;
        reasons.push("top search-bar geometry");
      }
      if (!editable && confidence < 0.4) {
        confidence = 0;
      }
      return {
        role: "google_earth_search_bar" as const,
        nodeId: node.nodeId,
        backendNodeId: node.backendNodeId,
        confidence: Math.min(1, Number(confidence.toFixed(2))),
        label:
          node.attributes["aria-label"] ||
          node.attributes.placeholder ||
          node.attributes.title ||
          node.layout?.text ||
          node.nodeName,
        reason: reasons.join(", "),
        layout: node.layout
      };
    })
    .filter((candidate) => candidate.confidence > 0)
    .sort((left, right) => right.confidence - left.confidence)
    .slice(0, 8);

  return {
    googleEarthSearchBar: candidates[0],
    searchCandidates: candidates
  };
}

function domSnapshotUiTree(snapshot: unknown): NativeDomRamCartographyResult["uiTree"] {
  const strings = (snapshot as { strings?: unknown }).strings;
  const documents = Array.isArray((snapshot as { documents?: unknown }).documents)
    ? ((snapshot as { documents: unknown[] }).documents)
    : [];
  const nodes: NativeDomRamUiTreeNode[] = [];

  for (let documentIndex = 0; documentIndex < documents.length; documentIndex += 1) {
    const document = documents[documentIndex] as {
      nodes?: {
        parentIndex?: unknown[];
        nodeType?: unknown[];
        nodeName?: unknown[];
        nodeValue?: unknown[];
        backendNodeId?: unknown[];
        attributes?: unknown[];
      };
      layout?: {
        nodeIndex?: unknown[];
        bounds?: unknown[];
        text?: unknown[];
        paintOrders?: unknown[];
      };
    };
    const docNodes = document.nodes ?? {};
    const parentIndexes = Array.isArray(docNodes.parentIndex) ? docNodes.parentIndex : [];
    const nodeTypes = Array.isArray(docNodes.nodeType) ? docNodes.nodeType : [];
    const nodeNames = Array.isArray(docNodes.nodeName) ? docNodes.nodeName : [];
    const nodeValues = Array.isArray(docNodes.nodeValue) ? docNodes.nodeValue : [];
    const backendNodeIds = Array.isArray(docNodes.backendNodeId) ? docNodes.backendNodeId : [];
    const attributes = Array.isArray(docNodes.attributes) ? docNodes.attributes : [];
    const layoutByNodeIndex = new Map<number, NativeDomRamUiTreeNode["layout"]>();
    const layout = document.layout;
    const layoutNodeIndexes = Array.isArray(layout?.nodeIndex) ? layout.nodeIndex : [];
    const layoutBounds = Array.isArray(layout?.bounds) ? layout.bounds : [];
    const layoutTexts = Array.isArray(layout?.text) ? layout.text : [];
    const paintOrders = Array.isArray(layout?.paintOrders) ? layout.paintOrders : [];

    for (let layoutIndex = 0; layoutIndex < layoutNodeIndexes.length; layoutIndex += 1) {
      const nodeIndex = Number(layoutNodeIndexes[layoutIndex]);
      if (!Number.isFinite(nodeIndex)) {
        continue;
      }
      const boundsRaw = layoutBounds[layoutIndex];
      const bounds = Array.isArray(boundsRaw) ? boundsRaw.map(numberField) : [];
      layoutByNodeIndex.set(nodeIndex, {
        x: bounds[0] ?? 0,
        y: bounds[1] ?? 0,
        width: bounds[2] ?? 0,
        height: bounds[3] ?? 0,
        paintOrder: numberField(paintOrders[layoutIndex]),
        text: compactDomText(cdpString(strings, layoutTexts[layoutIndex]))
      });
    }

    const depthByNodeIndex = new Map<number, number>();
    const depthFor = (nodeIndex: number): number => {
      if (depthByNodeIndex.has(nodeIndex)) {
        return depthByNodeIndex.get(nodeIndex) ?? 0;
      }
      const parent = Number(parentIndexes[nodeIndex]);
      const depth = Number.isFinite(parent) && parent >= 0 ? depthFor(parent) + 1 : 0;
      depthByNodeIndex.set(nodeIndex, depth);
      return depth;
    };

    for (let nodeIndex = 0; nodeIndex < nodeNames.length; nodeIndex += 1) {
      const parentIndex = Number(parentIndexes[nodeIndex]);
      const attrRaw = Array.isArray(attributes[nodeIndex]) ? (attributes[nodeIndex] as unknown[]) : [];
      const attrMap: Record<string, string> = {};
      for (let attrIndex = 0; attrIndex + 1 < attrRaw.length; attrIndex += 2) {
        const name = cdpString(strings, attrRaw[attrIndex]);
        if (!name) {
          continue;
        }
        attrMap[name] = compactDomText(cdpString(strings, attrRaw[attrIndex + 1]), 140);
      }
      const layoutRecord = layoutByNodeIndex.get(nodeIndex);
      nodes.push({
        nodeId: `d${documentIndex}:n${nodeIndex}`,
        parentNodeId: Number.isFinite(parentIndex) && parentIndex >= 0 ? `d${documentIndex}:n${parentIndex}` : "",
        depth: depthFor(nodeIndex),
        backendNodeId: numberField(backendNodeIds[nodeIndex]),
        nodeType: numberField(nodeTypes[nodeIndex]),
        nodeName: cdpString(strings, nodeNames[nodeIndex]) || "#unknown",
        nodeValue: compactDomText(cdpString(strings, nodeValues[nodeIndex])),
        attributes: attrMap,
        layout: layoutRecord,
        visible: Boolean(layoutRecord && layoutRecord.width > 0 && layoutRecord.height > 0)
      });
    }
  }

  const landmarks = googleEarthSearchBarLandmarks(nodes);
  return {
    schema: "forge.webexplorer.dom_ram_ui_tree.v1",
    nodeCount: nodes.length,
    nodes,
    landmarks,
    treeHash: hashJson({ schema: "forge.webexplorer.dom_ram_ui_tree.v1", nodes, landmarks })
  };
}

const ASSISTANT_GEO_ENTITY_PATTERN = /([@#])\{([^{}\n]{1,120})\}/g;

function primaryAssistantGeoEntityLabelFromText(text: string): string {
  ASSISTANT_GEO_ENTITY_PATTERN.lastIndex = 0;
  const match = ASSISTANT_GEO_ENTITY_PATTERN.exec(text);
  return match?.[2]?.replace(/\s+/g, " ").trim() ?? "";
}

function mapsResultTargetFromText(text: string): string {
  const match = /(?:^|\n)MAPS_RESULT[\s\S]*?(?:^|\n)target=("[^\n"]*"|'[^\n']*'|[^\n]*)/m.exec(text);
  const raw = match?.[1]?.trim() ?? "";
  if (!raw) {
    return "";
  }
  if ((raw.startsWith("\"") && raw.endsWith("\"")) || (raw.startsWith("'") && raw.endsWith("'"))) {
    try {
      return JSON.parse(raw.replace(/^'/, "\"").replace(/'$/, "\"")).replace(/\s+/g, " ").trim();
    } catch {
      return raw.slice(1, -1).replace(/\s+/g, " ").trim();
    }
  }
  return raw.replace(/\s+/g, " ").trim();
}

function assistantMapsSearchLabelFromText(text: string): string {
  return (
    primaryAssistantGeoEntityLabelFromText(text) ||
    extractMapsCodeAct(text)?.target ||
    extractMapsCodeAct(text)?.query ||
    mapsResultTargetFromText(text)
  );
}

function googleEarthLockedSearchInjectionFunction(): string {
  return `
function(query) {
  const searchControl = this;
  if (!searchControl || !query) return { accepted: false, reason: "missing_control_or_query" };
  const view = searchControl.ownerDocument?.defaultView || window;
  const inputEvent = typeof view.InputEvent === "function"
    ? new view.InputEvent("input", { bubbles: true, composed: true, data: query, inputType: "insertText" })
    : new view.Event("input", { bubbles: true, composed: true });
  searchControl.focus?.();
  if ("value" in searchControl) {
    const proto = searchControl instanceof view.HTMLTextAreaElement ? view.HTMLTextAreaElement.prototype : view.HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
    if (setter) {
      setter.call(searchControl, query);
    } else {
      searchControl.value = query;
    }
  } else {
    searchControl.textContent = query;
  }
  searchControl.dispatchEvent(inputEvent);
  searchControl.dispatchEvent(new view.Event("change", { bubbles: true, composed: true }));
  const key = { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true, cancelable: true, composed: true };
  searchControl.dispatchEvent(new view.KeyboardEvent("keydown", key));
  searchControl.dispatchEvent(new view.KeyboardEvent("keypress", key));
  searchControl.dispatchEvent(new view.KeyboardEvent("keyup", key));
  searchControl.form?.dispatchEvent?.(new view.Event("submit", { bubbles: true, cancelable: true }));
  return {
    accepted: true,
    tagName: searchControl.tagName || "",
    ariaLabel: searchControl.getAttribute?.("aria-label") || "",
    placeholder: searchControl.getAttribute?.("placeholder") || ""
  };
}
`;
}

function googleEarthLockedSearchPrepareFunction(): string {
  return `
function() {
  const searchControl = this;
  if (!searchControl) return { accepted: false, reason: "missing_control" };
  const view = searchControl.ownerDocument?.defaultView || window;
  searchControl.focus?.();
  searchControl.click?.();
  searchControl.select?.();
  if ("value" in searchControl) {
    const proto = searchControl instanceof view.HTMLTextAreaElement ? view.HTMLTextAreaElement.prototype : view.HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
    if (setter) {
      setter.call(searchControl, "");
    } else {
      searchControl.value = "";
    }
  } else {
    searchControl.textContent = "";
  }
  const inputEvent = typeof view.InputEvent === "function"
    ? new view.InputEvent("input", { bubbles: true, composed: true, data: null, inputType: "deleteContentBackward" })
    : new view.Event("input", { bubbles: true, composed: true });
  searchControl.dispatchEvent(inputEvent);
  return {
    accepted: true,
    tagName: searchControl.tagName || "",
    ariaLabel: searchControl.getAttribute?.("aria-label") || "",
    placeholder: searchControl.getAttribute?.("placeholder") || ""
  };
}
`;
}

async function dispatchGoogleEarthKeyboardSearch(
  debug: Electron.Debugger,
  query: string,
  layout?: NativeDomRamUiTreeNode["layout"]
): Promise<void> {
  if (layout && layout.width > 0 && layout.height > 0) {
    const x = Math.round(layout.x + Math.min(Math.max(layout.width / 2, 24), layout.width - 8));
    const y = Math.round(layout.y + Math.min(Math.max(layout.height / 2, 10), layout.height - 4));
    await debug.sendCommand("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none", clickCount: 0 });
    await debug.sendCommand("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
    await debug.sendCommand("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
    await delay(80);
  }
  const control = { modifiers: 2 };
  const aKey = { key: "a", code: "KeyA", windowsVirtualKeyCode: 65, nativeVirtualKeyCode: 65 };
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "rawKeyDown", ...aKey, ...control });
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "keyUp", ...aKey, ...control });
  await delay(40);
  const backspace = { key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8, nativeVirtualKeyCode: 8 };
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "rawKeyDown", ...backspace });
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "keyUp", ...backspace });
  await delay(40);
  clipboard.writeText(query);
  const vKey = { key: "v", code: "KeyV", windowsVirtualKeyCode: 86, nativeVirtualKeyCode: 86 };
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "rawKeyDown", ...vKey, ...control });
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "keyUp", ...vKey, ...control });
  await delay(180);
  const enter = { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 };
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "rawKeyDown", ...enter });
  await debug.sendCommand("Input.dispatchKeyEvent", { type: "keyUp", ...enter });
}

function googleEarthSearchFallbackScript(searchQuery: string): string {
  return `
(() => {
  const query = ${JSON.stringify(searchQuery)};
  if (!query) return false;
  const visit = (root, results = []) => {
    if (!root || typeof root.querySelectorAll !== "function") return results;
    for (const element of root.querySelectorAll("input, textarea, [contenteditable='true'], [role='combobox'], [role='searchbox']")) {
      results.push(element);
    }
    for (const element of root.querySelectorAll("*")) {
      if (element.shadowRoot) visit(element.shadowRoot, results);
    }
    return results;
  };
  const all = visit(document);
  const textOf = (element) => [
    element.getAttribute("aria-label"),
    element.getAttribute("placeholder"),
    element.getAttribute("title"),
    element.getAttribute("role"),
    element.id,
    element.className
  ].filter(Boolean).join(" ").toLowerCase();
  const searchControl = all.find((element) => {
    const text = textOf(element);
    return text.includes("google earth") || text.includes("search") || text.includes("rechercher") || text.includes("combobox");
  }) || all[0];
  if (!searchControl) return false;
  return (${googleEarthLockedSearchInjectionFunction()}).call(searchControl, query).accepted === true;
})()
`;
}

async function captureMapsDomRamUiTreeForTarget(target: Electron.WebContents): Promise<NativeDomRamCartographyResult["uiTree"] | null> {
  const debug = target.debugger;
  const wasAttached = debug.isAttached();
  try {
    if (!wasAttached) {
      debug.attach("1.3");
    }
    await debug.sendCommand("DOMSnapshot.enable");
    const cdpSnapshot = await debug.sendCommand("DOMSnapshot.captureSnapshot", {
      computedStyles: ["display", "visibility", "opacity", "pointer-events", "z-index", "transform"],
      includeDOMRects: true,
      includePaintOrder: true,
      includeBlendedBackgroundColors: false,
      includeTextColorOpacities: false
    });
    return domSnapshotUiTree(cdpSnapshot);
  } catch {
    return null;
  } finally {
    if (!wasAttached && debug.isAttached()) {
      debug.detach();
    }
  }
}

async function dispatchGoogleEarthSearchFromLock(
  debug: Electron.Debugger,
  query: string,
  lock: Pick<GoogleEarthSearchLock, "backendNodeId" | "layout">
): Promise<boolean> {
  await debug.sendCommand("DOM.enable");
  const resolved = await debug.sendCommand("DOM.resolveNode", { backendNodeId: lock.backendNodeId }) as {
    object?: { objectId?: string };
  };
  const objectId = resolved.object?.objectId;
  if (!objectId) {
    return false;
  }
  const prepared = await debug.sendCommand("Runtime.callFunctionOn", {
    objectId,
    functionDeclaration: googleEarthLockedSearchPrepareFunction(),
    arguments: [],
    awaitPromise: false,
    returnByValue: true
  }) as { result?: { value?: { accepted?: boolean } } };
  if (prepared.result?.value?.accepted !== true) {
    return false;
  }
  await dispatchGoogleEarthKeyboardSearch(debug, query, lock.layout);
  return true;
}

async function injectNativeMapsSearchViaLockedLandmark(searchQuery: string): Promise<boolean> {
  const query = normalizeAssistantGeoEntityQuery(searchQuery);
  const target = mapsCartographyWebContents();
  if (!query || !target || target.isDestroyed()) {
    return false;
  }
  const url = target.getURL() || mapsDomWebviewGuestUrl || nativeMapsTargetUrl;
  if (!isAllowedNativeMapsUrl(url)) {
    return false;
  }

  const debug = target.debugger;
  const wasAttached = debug.isAttached();
  try {
    if (!wasAttached) {
      debug.attach("1.3");
    }
    const cachedLock = cachedGoogleEarthSearchLockFor(target);
    if (cachedLock) {
      let cachedAccepted = false;
      try {
        cachedAccepted = await dispatchGoogleEarthSearchFromLock(debug, query, cachedLock);
      } catch {
        cachedAccepted = false;
      }
      if (cachedAccepted) {
        return true;
      }
      clearGoogleEarthSearchLock(target);
    }
    const uiTree = await captureMapsDomRamUiTreeForTarget(target);
    const landmark = uiTree?.landmarks.googleEarthSearchBar;
    if (landmark?.backendNodeId) {
      if (await dispatchGoogleEarthSearchFromLock(debug, query, landmark)) {
        rememberGoogleEarthSearchLock(target, landmark);
        return true;
      }
    }
  } catch {
    // Fall back to a conservative in-page locator below.
  } finally {
    if (!wasAttached && debug.isAttached()) {
      debug.detach();
    }
  }

  try {
    return await target.executeJavaScript(googleEarthSearchFallbackScript(query), true) === true;
  } catch {
    return false;
  }
}

async function captureMapsDomRamCartography(event: Electron.IpcMainInvokeEvent): Promise<NativeDomRamCartographyResult> {
  if (!validateSender(event)) {
    return emptyMapsDomRamCartographyResult({
      code: "bad_sender",
      message: "Maps DOM/RAM cartography rejected by sender validation.",
      proofHash: hashJson(event.senderFrame?.url ?? "")
    });
  }
  const target = mapsCartographyWebContents();
  if (!target || target.isDestroyed()) {
    return emptyMapsDomRamCartographyResult({
      code: "rust_unavailable",
      message: "Google Earth webview is not attached yet.",
      proofHash: hashJson({ nativeMapsTargetUrl, mapsDomWebviewGuestUrl })
    });
  }
  const url = target.getURL() || mapsDomWebviewGuestUrl || nativeMapsTargetUrl;
  if (!isAllowedNativeMapsUrl(url)) {
    return emptyMapsDomRamCartographyResult({
      code: "bad_payload",
      message: "Maps DOM/RAM cartography rejected outside the Google Earth perimeter.",
      proofHash: hashJson({ url })
    });
  }

  const debug = target.debugger;
  const wasAttached = debug.isAttached();
  try {
    if (!wasAttached) {
      debug.attach("1.3");
    }
    await debug.sendCommand("DOMSnapshot.enable");
    const cdpSnapshot = await debug.sendCommand("DOMSnapshot.captureSnapshot", {
      computedStyles: ["display", "visibility", "opacity", "pointer-events", "z-index", "transform"],
      includeDOMRects: true,
      includePaintOrder: true,
      includeBlendedBackgroundColors: false,
      includeTextColorOpacities: false
    });
    const counts = domSnapshotCounts(cdpSnapshot);
    const uiTree = domSnapshotUiTree(cdpSnapshot);
    const processId = (target as { getOSProcessId?: () => number }).getOSProcessId?.() ?? 0;
    const processType = (target as { getProcessType?: () => string }).getProcessType?.() ?? "unknown";
    const memoryInfo = app.getAppMetrics().find((metric) => metric.pid === processId)?.memory;
    const captureHash = hashJson({
      target: "google_earth",
      lane: "native_tandem_dom_ram",
      url,
      cdpSnapshot
    });
    const memory = {
      source: "electron_webcontents" as const,
      workingSetSizeKb: numberField((memoryInfo as { workingSetSize?: unknown } | undefined)?.workingSetSize),
      peakWorkingSetSizeKb: numberField((memoryInfo as { peakWorkingSetSize?: unknown } | undefined)?.peakWorkingSetSize),
      privateBytesKb: numberField((memoryInfo as { privateBytes?: unknown } | undefined)?.privateBytes),
      sharedBytesKb: numberField((memoryInfo as { sharedBytes?: unknown } | undefined)?.sharedBytes),
      processId,
      processType,
      regionTableHash: hashJson({ url, processId, processType, memoryInfo })
    };
    const artifacts = DOM_RAM_ARTIFACT_CONTRACTS.map((contract) => {
      const recordCount =
        contract.kind === "dom_graph_page"
          ? counts.nodeCount
          : contract.kind === "ram_region_table"
            ? Math.max(1, Math.ceil(memory.workingSetSizeKb / 4096))
            : 1;
      const byteLength =
        contract.kind === "dom_graph_page"
          ? Math.max(0, counts.nodeCount * 64)
          : contract.kind === "ram_region_table"
            ? Math.max(0, recordCount * 56)
            : 96;
      return {
        ...contract,
        liveSliceHash: hashJson({ contract, captureHash, memory: memory.regionTableHash, recordCount, byteLength }),
        byteLength,
        recordCount
      };
    });
    const result: NativeDomRamCartographyResult = {
      accepted: true,
      schema: "forge.webexplorer.dom_ram_cartography.v1",
      target: "google_earth",
      url,
      lane: "native_tandem_dom_ram",
      nativeDomain: "dom_ram",
      engine: "monster_native_tandem",
      snapshot: {
        source: "cdp_domsnapshot",
        ...counts,
        captureHash
      },
      uiTree,
      memory,
      artifacts,
      manifestHash: "",
      proofHash: ""
    };
    result.manifestHash = hashJson({ ...result, proofHash: "", manifestHash: "" });
    result.proofHash = hashJson(result);
    return result;
  } catch (error) {
    return emptyMapsDomRamCartographyResult({
      code: "rust_unavailable",
      message: error instanceof Error ? error.message : String(error),
      proofHash: hashJson({ url, error: String(error) })
    });
  } finally {
    if (!wasAttached && debug.isAttached()) {
      debug.detach();
    }
  }
}

function isAllowedNativeWebExplorerUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:") {
      return false;
    }
    return (
      parsed.hostname === "www.google.com" ||
      parsed.hostname === "google.com" ||
      parsed.hostname === "earth.google.com" ||
      parsed.hostname === "accounts.google.com" ||
      parsed.hostname === "gmail.com" ||
      parsed.hostname === "mail.google.com" ||
      parsed.hostname === "airbnb.com" ||
      parsed.hostname === "www.airbnb.com" ||
      parsed.hostname.endsWith(".airbnb.com") ||
      parsed.hostname.endsWith(".google.com")
    );
  } catch {
    return false;
  }
}

function isAllowedNativeMapsUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && parsed.hostname === "earth.google.com";
  } catch {
    return false;
  }
}

function isGmailMarketingLandingUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:") {
      return false;
    }
    if (parsed.hostname === "gmail.com" || parsed.hostname === "www.gmail.com") {
      return true;
    }
    return (
      (parsed.hostname === "www.google.com" || parsed.hostname === "google.com") &&
      parsed.pathname.toLowerCase().includes("/gmail")
    );
  } catch {
    return false;
  }
}

function nativeWebExplorerNavigationTarget(url: string): string {
  return isGmailMarketingLandingUrl(url) ? GMAIL_SIGN_IN_URL : url;
}

function emitNativeWebExplorerCodeAct(request: NativeWebExplorerCodeAct): void {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) {
      window.webContents.send("forge:webexplorer-codeact", request);
    }
  }
}

function emitNativeMapsCodeAct(request: NativeWebExplorerCodeAct): void {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) {
      window.webContents.send("forge:maps-codeact", request);
    }
  }
}

function configureNativeWebExplorerSession(): void {
  if (nativeWebExplorerSessionConfigured) {
    return;
  }
  nativeWebExplorerSessionConfigured = true;
  const webExplorerSession = session.fromPartition("persist:ingen-webexplorer");
  webExplorerSession.setPermissionRequestHandler((webContents, permission, callback) => {
    const url = webContents.getURL();
    const allowed = isAllowedNativeWebExplorerUrl(url) && (permission === "notifications" || permission === "clipboard-read");
    callback(allowed);
  });
  webExplorerSession.webRequest.onBeforeRequest((details, callback) => {
    if (details.resourceType === "mainFrame" && isGmailMarketingLandingUrl(details.url)) {
      callback({ redirectURL: GMAIL_SIGN_IN_URL });
      return;
    }
    if (details.resourceType === "mainFrame" && !isAllowedNativeWebExplorerUrl(details.url)) {
      callback({ cancel: true });
      return;
    }
    callback({});
  });
}

function configureNativeMapsSession(): void {
  if (nativeMapsSessionConfigured) {
    return;
  }
  nativeMapsSessionConfigured = true;
  const mapsSession = session.fromPartition("persist:ingen-maps");
  mapsSession.setPermissionRequestHandler((webContents, permission, callback) => {
    const url = webContents.getURL();
    const allowed = isAllowedNativeMapsUrl(url) && (permission === "notifications" || permission === "clipboard-read");
    callback(allowed);
  });
  mapsSession.webRequest.onBeforeRequest((details, callback) => {
    if (details.resourceType === "mainFrame" && !isAllowedNativeMapsUrl(details.url)) {
      callback({ cancel: true });
      return;
    }
    callback({});
  });
}

function loadNativeWebExplorerTarget(view: BrowserView, reason: string): void {
  if (view.webContents.isDestroyed()) {
    return;
  }
  const targetUrl = nativeWebExplorerNavigationTarget(nativeWebExplorerTargetUrl);
  nativeWebExplorerTargetUrl = targetUrl;
  if (nativeWebExplorerLoadedUrl === targetUrl || nativeWebExplorerPendingUrl === targetUrl) {
    return;
  }
  nativeWebExplorerPendingUrl = targetUrl;
  console.info("Native WebExplorer loadURL.", { reason, targetUrl });
  void view.webContents.loadURL(targetUrl)
    .then(() => {
      if (nativeWebExplorerView === view && !view.webContents.isDestroyed()) {
        nativeWebExplorerLoadedUrl = targetUrl;
        nativeWebExplorerPendingUrl = "";
      }
    })
    .catch((error: unknown) => {
      if (nativeWebExplorerPendingUrl === targetUrl) {
        nativeWebExplorerPendingUrl = "";
      }
      console.warn("Native WebExplorer navigation failed.", { reason, url: targetUrl, error });
    });
}

function loadNativeMapsTarget(view: WebContentsView, reason: string): void {
  if (view.webContents.isDestroyed()) {
    return;
  }
  const targetUrl = nativeMapsTargetUrl;
  if (nativeMapsLoadedUrl === targetUrl || nativeMapsPendingUrl === targetUrl) {
    return;
  }
  nativeMapsPendingUrl = targetUrl;
  console.info("Native Maps loadURL.", { reason, targetUrl });
  void view.webContents.loadURL(targetUrl)
    .then(() => {
      if (nativeMapsView === view && !view.webContents.isDestroyed()) {
        nativeMapsLoadedUrl = targetUrl;
        nativeMapsPendingUrl = "";
      }
    })
    .catch((error: unknown) => {
      if (nativeMapsPendingUrl === targetUrl) {
        nativeMapsPendingUrl = "";
      }
      console.warn("Native Maps navigation failed.", { reason, url: targetUrl, error });
    });
}

function installNativeWebExplorerViewportFade(view: BrowserView | WebContentsView): void {
  if (view.webContents.isDestroyed()) {
    return;
  }
  void view.webContents.insertCSS(NATIVE_WEBEXPLORER_VIEWPORT_FADE_CSS)
    .catch((error: unknown) => {
      console.warn("Native WebExplorer viewport fade injection failed.", error);
    });
}

function navigateNativeWebExplorerToGoogle(request: GoogleWebCodeActRequest, parallelSessionIndex = 0): NativeWebExplorerResult {
  if (!isAllowedNativeWebExplorerUrl(request.url)) {
    return nativeWebExplorerResult(false, {
      code: "bad_payload",
      message: "Google WebExplorer navigation rejected: URL is outside the Google search perimeter.",
      proofHash: hashJson({ url: request.url, proofHash: request.proofHash })
    });
  }
  activateWebExplorerSplit();
  nativeWebExplorerTargetUrl = request.url;
  if (nativeWebExplorerView && !nativeWebExplorerView.webContents.isDestroyed()) {
    loadNativeWebExplorerTarget(nativeWebExplorerView, "google-codeact");
  }
  emitNativeWebExplorerCodeAct({ ...request, parallelSessionIndex });
  return nativeWebExplorerResult(true);
}

function navigateNativeWebExplorerToMaps(request: MapsCodeActRequest, parallelSessionIndex = 0): NativeWebExplorerResult {
  const navigationUrl = "banger://maps-sphere";
  activateWebExplorerSplit();
  hideNativeMapsView();
  nativeMapsTargetUrl = navigationUrl;
  emitNativeMapsCodeAct({ ...request, url: navigationUrl, parallelSessionIndex });
  return nativeMapsResult(true);
}

async function openAssistantGeoEntityMaps(query: unknown): Promise<NativeWebExplorerResult> {
  const request = await resolveAssistantGeoEntityMapsRequest(query);
  if (!request) {
    const normalized = normalizeAssistantGeoEntityQuery(query);
    const fallback = buildGoogleWebCodeActRequest(normalized, ["assistant_geo_entity"], "explicit_codeact");
    if (fallback) {
      return navigateNativeWebExplorerToGoogle(fallback, 0);
    }
    return nativeMapsResult(false, {
      code: "bad_payload",
      message: "Maps could not resolve this assistant geo entity.",
      proofHash: hashJson({ assistantGeoEntity: normalized || query })
    });
  }
  return navigateNativeWebExplorerToMaps(request, 0);
}

function navigateNativeWebExplorerToGmail(request: GmailCodeActRequest, parallelSessionIndex = 0): NativeWebExplorerResult {
  const navigationUrl = gmailWebExplorerNavigationUrl(request);
  if (!isAllowedNativeWebExplorerUrl(navigationUrl)) {
    return nativeWebExplorerResult(false, {
      code: "bad_payload",
      message: "Gmail WebExplorer navigation rejected: URL is outside the Google/Gmail perimeter.",
      proofHash: hashJson({ url: navigationUrl, proofHash: request.proofHash })
    });
  }
  activateWebExplorerSplit();
  nativeWebExplorerTargetUrl = navigationUrl;
  if (nativeWebExplorerView && !nativeWebExplorerView.webContents.isDestroyed()) {
    loadNativeWebExplorerTarget(nativeWebExplorerView, "gmail-codeact");
  }
  emitNativeWebExplorerCodeAct({ ...request, url: navigationUrl, parallelSessionIndex });
  return nativeWebExplorerResult(true);
}

function navigateNativeWebExplorerToAirbnb(request: AirbnbCodeActRequest, parallelSessionIndex = 0): NativeWebExplorerResult {
  const navigationUrl = request.url || AIRBNB_HOME_URL;
  if (!isAllowedNativeWebExplorerUrl(navigationUrl)) {
    return nativeWebExplorerResult(false, {
      code: "bad_payload",
      message: "Airbnb WebExplorer navigation rejected: URL is outside the Airbnb perimeter.",
      proofHash: hashJson({ url: navigationUrl, proofHash: request.proofHash })
    });
  }
  activateWebExplorerSplit();
  nativeWebExplorerTargetUrl = navigationUrl;
  if (nativeWebExplorerView && !nativeWebExplorerView.webContents.isDestroyed()) {
    loadNativeWebExplorerTarget(nativeWebExplorerView, "airbnb-codeact");
  }
  emitNativeWebExplorerCodeAct({ ...request, url: navigationUrl, parallelSessionIndex });
  return nativeWebExplorerResult(true);
}

function normalizeNativeWebExplorerBounds(bounds: NativeWebExplorerBounds): NativeWebExplorerBounds | null {
  const x = Math.round(bounds.x);
  const y = Math.round(bounds.y);
  const width = Math.round(bounds.width);
  const height = Math.round(bounds.height);
  if (![x, y, width, height].every(Number.isFinite) || width < 80 || height < 80) {
    return null;
  }
  return { x, y, width, height };
}

function normalizeBangerViewportRequest(value: unknown): {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  sceneKind?: string;
} {
  if (!value || typeof value !== "object") {
    return {};
  }
  const candidate = value as Partial<NativeWebExplorerBounds> & { sceneKind?: unknown };
  const x = Number.isFinite(candidate.x) ? Math.max(0, Math.round(candidate.x as number)) : undefined;
  const y = Number.isFinite(candidate.y) ? Math.max(0, Math.round(candidate.y as number)) : undefined;
  const width = Number.isFinite(candidate.width) ? Math.max(64, Math.round(candidate.width as number)) : undefined;
  const height = Number.isFinite(candidate.height) ? Math.max(64, Math.round(candidate.height as number)) : undefined;
  const sceneKind = candidate.sceneKind === "maps_sphere" ? "maps_sphere" : undefined;
  return { x, y, width, height, sceneKind };
}

function expandNativeMapsBoundsForEarth(bounds: NativeWebExplorerBounds, owner: BrowserWindow): NativeWebExplorerBounds {
  const [contentWidth, contentHeight] = owner.getContentSize();
  const requestedLeftOverscan = Math.max(
    NATIVE_MAPS_EARTH_OVERSCAN_PX.minLeft,
    Math.round(bounds.width * NATIVE_MAPS_EARTH_OVERSCAN_PX.leftRatio)
  );
  const leftOverscan = Math.min(requestedLeftOverscan, Math.max(0, bounds.x));
  const x = bounds.x - leftOverscan;
  const width = Math.min(contentWidth - x, bounds.width + leftOverscan);
  const height = Math.min(contentHeight - bounds.y, bounds.height + NATIVE_MAPS_EARTH_OVERSCAN_PX.bottom);
  return {
    x,
    y: bounds.y,
    width,
    height
  };
}

function attachNativeWebExplorerView(owner: BrowserWindow, view: BrowserView): void {
  if (!owner.getBrowserViews().includes(view)) {
    owner.addBrowserView(view);
  }
  owner.setTopBrowserView(view);
}

function attachNativeMapsView(owner: BrowserWindow, view: WebContentsView): void {
  const currentIndex = owner.contentView.children.indexOf(view);
  if (currentIndex >= 0) {
    owner.contentView.removeChildView(view);
  }
  owner.contentView.addChildView(view);
}

function ensureNativeWebExplorerView(owner: BrowserWindow): BrowserView {
  if (nativeWebExplorerView && nativeWebExplorerOwner === owner && !nativeWebExplorerView.webContents.isDestroyed()) {
    attachNativeWebExplorerView(owner, nativeWebExplorerView);
    return nativeWebExplorerView;
  }
  hideNativeWebExplorerView();
  configureNativeWebExplorerSession();
  const view = new BrowserView({
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      backgroundThrottling: false,
      partition: "persist:ingen-webexplorer"
    }
  });
  view.setBackgroundColor("#ffffff");
  view.webContents.setUserAgent(CHATGPT_USER_AGENT);
  nativeWebExplorerView = view;
  nativeWebExplorerOwner = owner;
  nativeWebExplorerLoadedUrl = "";
  nativeWebExplorerPendingUrl = "";
  nativeWebExplorerBoundsKey = "";
  attachNativeWebExplorerView(owner, view);
  view.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("https://")) {
      void shell.openExternal(url);
    }
    return { action: "deny" };
  });
  view.webContents.on("will-navigate", (event, url) => {
    if (isGmailMarketingLandingUrl(url)) {
      event.preventDefault();
      nativeWebExplorerTargetUrl = GMAIL_SIGN_IN_URL;
      loadNativeWebExplorerTarget(view, "gmail-landing-redirect");
      return;
    }
    if (!isAllowedNativeWebExplorerUrl(url)) {
      event.preventDefault();
    }
  });
  view.webContents.on("did-start-loading", () => {
    console.info("Native WebExplorer loading.", nativeWebExplorerTargetUrl);
  });
  view.webContents.on("did-finish-load", () => {
    if (!view.webContents.isDestroyed()) {
      installNativeWebExplorerViewportFade(view);
      console.info("Native WebExplorer loaded.", view.webContents.getURL());
    }
  });
  view.webContents.on("dom-ready", () => {
    installNativeWebExplorerViewportFade(view);
  });
  view.webContents.on("did-fail-load", (_event, errorCode, errorDescription, validatedUrl, isMainFrame) => {
    if (isMainFrame) {
      console.warn("Native WebExplorer load failed.", { errorCode, errorDescription, validatedUrl });
    }
  });
  view.webContents.once("destroyed", () => {
    if (nativeWebExplorerView === view) {
      nativeWebExplorerView = null;
      nativeWebExplorerOwner = null;
      nativeWebExplorerLoadedUrl = "";
      nativeWebExplorerPendingUrl = "";
    }
  });
  return view;
}

function ensureNativeMapsView(owner: BrowserWindow): WebContentsView {
  if (nativeMapsView && nativeMapsOwner === owner && !nativeMapsView.webContents.isDestroyed()) {
    attachNativeMapsView(owner, nativeMapsView);
    return nativeMapsView;
  }
  hideNativeMapsView();
  hideNativeWebExplorerView();
  configureNativeMapsSession();
  const view = new WebContentsView({
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      backgroundThrottling: false,
      partition: "persist:ingen-maps"
    }
  });
  view.setBackgroundColor("#ffffff");
  view.webContents.setUserAgent(CHATGPT_USER_AGENT);
  nativeMapsView = view;
  nativeMapsOwner = owner;
  nativeMapsLoadedUrl = "";
  nativeMapsPendingUrl = "";
  nativeMapsBoundsKey = "";
  attachNativeMapsView(owner, view);
  view.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("https://")) {
      void shell.openExternal(url);
    }
    return { action: "deny" };
  });
  view.webContents.on("will-navigate", (event, url) => {
    if (!isAllowedNativeMapsUrl(url)) {
      event.preventDefault();
    }
  });
  view.webContents.on("did-start-loading", () => {
    clearGoogleEarthSearchLock(view.webContents);
    console.info("Native Maps loading.", nativeMapsTargetUrl);
  });
  view.webContents.on("did-finish-load", () => {
    if (!view.webContents.isDestroyed()) {
      nativeMapsLoadedUrl = view.webContents.getURL();
      nativeMapsPendingUrl = "";
      clearGoogleEarthSearchLock(view.webContents);
      installNativeWebExplorerViewportFade(view);
      console.info("Native Maps loaded.", view.webContents.getURL());
    }
  });
  view.webContents.on("dom-ready", () => {
    installNativeWebExplorerViewportFade(view);
  });
  view.webContents.on("did-fail-load", (_event, errorCode, errorDescription, validatedUrl, isMainFrame) => {
    if (isMainFrame) {
      console.warn("Native Maps load failed.", { errorCode, errorDescription, validatedUrl });
    }
  });
  view.webContents.once("destroyed", () => {
    if (nativeMapsView === view) {
      nativeMapsView = null;
      nativeMapsOwner = null;
      nativeMapsLoadedUrl = "";
      nativeMapsPendingUrl = "";
    }
    clearGoogleEarthSearchLock(view.webContents);
  });
  return view;
}

function hideNativeWebExplorerView(): void {
  const view = nativeWebExplorerView;
  const owner = nativeWebExplorerOwner;
  nativeWebExplorerView = null;
  nativeWebExplorerOwner = null;
  nativeWebExplorerLoadedUrl = "";
  nativeWebExplorerPendingUrl = "";
  nativeWebExplorerBoundsKey = "";
  if (!view) {
    return;
  }
  try {
    owner?.removeBrowserView(view);
  } catch (error) {
    console.warn("Native WebExplorer view detach failed.", error);
  }
  if (!view.webContents.isDestroyed()) {
    view.webContents.close();
  }
}

function hideNativeMapsView(): void {
  const view = nativeMapsView;
  const owner = nativeMapsOwner;
  nativeMapsView = null;
  nativeMapsOwner = null;
  nativeMapsLoadedUrl = "";
  nativeMapsPendingUrl = "";
  nativeMapsBoundsKey = "";
  if (!view) {
    return;
  }
  try {
    owner?.contentView.removeChildView(view);
  } catch (error) {
    console.warn("Native Maps view detach failed.", error);
  }
  if (!view.webContents.isDestroyed()) {
    view.webContents.close();
  }
}

function showNativeWebExplorer(event: Electron.IpcMainInvokeEvent, bounds: NativeWebExplorerBounds): NativeWebExplorerResult {
  if (!validateSender(event)) {
    return nativeWebExplorerResult(false, {
      code: "bad_sender",
      message: "Native WebExplorer rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeWebExplorerBounds(bounds);
  if (!normalized) {
    return nativeWebExplorerResult(false, {
      code: "bad_payload",
      message: "Native WebExplorer bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const owner = senderNativeWindow(event);
  if (!owner || owner.isDestroyed()) {
    return nativeWebExplorerResult(false, {
      code: "rust_unavailable",
      message: "Native WebExplorer owner window is unavailable.",
      proofHash: hashJson({ bounds })
    });
  }
  hideNativeMapsView();
  const view = ensureNativeWebExplorerView(owner);
  view.setBounds(normalized);
  nativeWebExplorerBoundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}`;
  owner.setTopBrowserView(view);
  view.webContents.focus();
  console.info("Native WebExplorer shown.", { bounds: normalized, url: nativeWebExplorerTargetUrl });
  loadNativeWebExplorerTarget(view, "show");
  return nativeWebExplorerResult(true);
}

function showNativeMaps(event: Electron.IpcMainInvokeEvent, bounds: NativeWebExplorerBounds): NativeWebExplorerResult {
  if (!validateSender(event)) {
    return nativeMapsResult(false, {
      code: "bad_sender",
      message: "Native Maps rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeWebExplorerBounds(bounds);
  if (!normalized) {
    return nativeMapsResult(false, {
      code: "bad_payload",
      message: "Native Maps bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const owner = senderNativeWindow(event);
  if (!owner || owner.isDestroyed()) {
    return nativeMapsResult(false, {
      code: "rust_unavailable",
      message: "Native Maps owner window is unavailable.",
      proofHash: hashJson({ bounds })
    });
  }
  hideNativeMapsView();
  nativeMapsBoundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}:banger_sphere`;
  console.info("Native Maps routed to Banger sphere viewport.", { bounds: normalized, url: nativeMapsTargetUrl });
  return nativeMapsResult(true);
}

function updateNativeWebExplorerBounds(event: Electron.IpcMainInvokeEvent, bounds: NativeWebExplorerBounds): NativeWebExplorerResult {
  if (!nativeWebExplorerView) {
    return showNativeWebExplorer(event, bounds);
  }
  if (!validateSender(event)) {
    return nativeWebExplorerResult(false, {
      code: "bad_sender",
      message: "Native WebExplorer bounds update rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeWebExplorerBounds(bounds);
  if (!normalized) {
    return nativeWebExplorerResult(false, {
      code: "bad_payload",
      message: "Native WebExplorer bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const boundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}`;
  if (nativeWebExplorerBoundsKey === boundsKey) {
    return nativeWebExplorerResult(true);
  }
  nativeWebExplorerBoundsKey = boundsKey;
  nativeWebExplorerView.setBounds(normalized);
  return nativeWebExplorerResult(true);
}

function updateNativeMapsBounds(event: Electron.IpcMainInvokeEvent, bounds: NativeWebExplorerBounds): NativeWebExplorerResult {
  if (!validateSender(event)) {
    return nativeMapsResult(false, {
      code: "bad_sender",
      message: "Native Maps bounds update rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeWebExplorerBounds(bounds);
  if (!normalized) {
    return nativeMapsResult(false, {
      code: "bad_payload",
      message: "Native Maps bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const owner = senderNativeWindow(event) ?? nativeMapsOwner;
  if (!owner || owner.isDestroyed()) {
    return nativeMapsResult(false, {
      code: "rust_unavailable",
      message: "Native Maps owner window is unavailable.",
      proofHash: hashJson({ bounds })
    });
  }
  hideNativeMapsView();
  const boundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}:banger_sphere`;
  if (nativeMapsBoundsKey === boundsKey) {
    return nativeMapsResult(true);
  }
  nativeMapsBoundsKey = boundsKey;
  return nativeMapsResult(true);
}

function installNativeWebExplorerIpc(): void {
  ipcMain.handle("forge:webexplorer-show", (event, bounds: NativeWebExplorerBounds): NativeWebExplorerResult => {
    return showNativeWebExplorer(event, bounds);
  });
  ipcMain.handle("forge:webexplorer-bounds", (event, bounds: NativeWebExplorerBounds): NativeWebExplorerResult => {
    return updateNativeWebExplorerBounds(event, bounds);
  });
  ipcMain.handle("forge:webexplorer-hide", (event): NativeWebExplorerResult => {
    if (!validateSender(event)) {
      return nativeWebExplorerResult(false, {
        code: "bad_sender",
        message: "Native WebExplorer hide rejected by sender validation.",
        proofHash: hashJson(event.senderFrame?.url ?? "")
      });
    }
    hideNativeWebExplorerView();
    return nativeWebExplorerResult(true);
  });
  ipcMain.handle("forge:maps-show", (event, bounds: NativeWebExplorerBounds): NativeWebExplorerResult => {
    return showNativeMaps(event, bounds);
  });
  ipcMain.handle("forge:maps-bounds", (event, bounds: NativeWebExplorerBounds): NativeWebExplorerResult => {
    return updateNativeMapsBounds(event, bounds);
  });
  ipcMain.handle("forge:maps-hide", (event): NativeWebExplorerResult => {
    if (!validateSender(event)) {
      return nativeMapsResult(false, {
        code: "bad_sender",
        message: "Native Maps hide rejected by sender validation.",
        proofHash: hashJson(event.senderFrame?.url ?? "")
      });
    }
    hideNativeMapsView();
    return nativeMapsResult(true);
  });
  ipcMain.handle("forge:maps-dom-ram-cartography-capture", async (event): Promise<NativeDomRamCartographyResult> => {
    return captureMapsDomRamCartography(event);
  });
}

function minimizeNativeWindow(event: Electron.IpcMainInvokeEvent): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window minimize from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  releaseNativeWidgetWindowControlState(window);
  if (window.isFullScreen()) {
    window.setFullScreen(false);
  }
  if ((process.platform === "win32" || process.platform === "darwin") && !window.isMinimizable()) {
    window.setMinimizable(true);
  }
  console.info("Applying native window minimize", { id: window.id, title: window.getTitle() });
  window.minimize();
  return true;
}

function dockNativeWindowRightHalf(window: BrowserWindow): void {
  const display = screen.getDisplayMatching(window.getBounds());
  const { workArea } = display;
  const topOverscan = process.platform === "win32" ? 2 : 0;
  const width = Math.max(720, Math.floor(workArea.width / 2));
  const bounds = {
    x: workArea.x + workArea.width - width,
    y: workArea.y - topOverscan,
    width,
    height: workArea.height + topOverscan
  };
  if (window.isFullScreen()) {
    window.setFullScreen(false);
  }
  if (window.isMaximized()) {
    window.unmaximize();
  }
  if (window.isMinimized()) {
    window.restore();
  }
  window.setMinimumSize(720, 640);
  window.setBounds(bounds, false);
  window.show();
  window.focus();
}

function toggleNativeWindowMaximize(event: Electron.IpcMainInvokeEvent): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window maximize from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window) {
    return false;
  }
  if (window.isMaximized() || window.isFullScreen()) {
    console.info("Applying native window restore/dock", { id: window.id, title: window.getTitle() });
    dockNativeWindowRightHalf(window);
  } else {
    console.info("Applying native window maximize", { id: window.id, title: window.getTitle() });
    window.setMinimumSize(1180, 760);
    window.maximize();
  }
  return true;
}

function closeNativeWindow(event: Electron.IpcMainInvokeEvent): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window close from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window) {
    return false;
  }
  console.info("Applying native window close", { id: window.id, title: window.getTitle() });
  if (!window.isDestroyed()) {
    window.destroy();
  }
  if (BrowserWindow.getAllWindows().length <= 1) {
    app.quit();
  }
  return true;
}

function widgetWindowBounds(window: BrowserWindow, options: { taskbarHidden?: boolean } = {}): Electron.Rectangle {
  const display = screen.getDisplayMatching(window.getBounds());
  const { workArea } = display;
  const targetTaskbarHidden = options.taskbarHidden ?? widgetTaskbarHidden;
  const verticalArea = targetTaskbarHidden ? display.bounds : workArea;
  const restoreBounds = widgetWindowRestoreState?.bounds ?? window.getBounds();
  const width = Math.min(workArea.width, Math.max(520, restoreBounds.width));
  const targetHeight = widgetWindowPanelExpanded ? WIDGET_WINDOW_PANEL_HEIGHT : WIDGET_WINDOW_HEIGHT;
  const height = Math.min(targetHeight, Math.max(136, verticalArea.height - 24));
  const bottomGap = targetTaskbarHidden ? WIDGET_WINDOW_AUTOHIDE_BOTTOM_GAP : WIDGET_WINDOW_BOTTOM_GAP;
  const x = Math.min(
    Math.max(restoreBounds.x, workArea.x),
    workArea.x + workArea.width - width
  );
  return {
    x,
    y: verticalArea.y + verticalArea.height - height - bottomGap,
    width,
    height
  };
}

function boundsLookLikeWidget(bounds: Electron.Rectangle, display = screen.getDisplayMatching(bounds)): boolean {
  const { workArea } = display;
  const nearBottom = bounds.y >= workArea.y + workArea.height - (WIDGET_WINDOW_PANEL_HEIGHT + 140);
  const shortEnough = bounds.height <= WIDGET_WINDOW_PANEL_HEIGHT + 80;
  const wideEnough = bounds.width >= Math.min(520, workArea.width) || bounds.width >= workArea.width * 0.45;
  return nearBottom && shortEnough && wideEnough;
}

function normalWindowRestoreBounds(window: BrowserWindow): Electron.Rectangle {
  const display = screen.getDisplayMatching(window.getBounds());
  const { workArea } = display;
  return {
    x: workArea.x,
    y: workArea.y,
    width: workArea.width,
    height: workArea.height
  };
}

function clearWidgetWindowShrinkTimer(): void {
  if (widgetWindowShrinkTimer !== null) {
    clearTimeout(widgetWindowShrinkTimer);
    widgetWindowShrinkTimer = null;
  }
}

function clearWidgetTaskbarAutoHideTimer(): void {
  if (widgetTaskbarAutoHideTimer !== null) {
    clearTimeout(widgetTaskbarAutoHideTimer);
    widgetTaskbarAutoHideTimer = null;
  }
}

function clearWidgetWindowBoundsAnimationTimer(): void {
  if (widgetWindowBoundsAnimationTimer !== null) {
    clearInterval(widgetWindowBoundsAnimationTimer);
    widgetWindowBoundsAnimationTimer = null;
  }
}

function releaseNativeWidgetWindowControlState(window: BrowserWindow): void {
  clearWidgetWindowShrinkTimer();
  clearWidgetTaskbarAutoHideTimer();
  widgetTaskbarAutoHideJobToken += 1;
  clearWidgetWindowBoundsAnimationTimer();
  resetNativeWidgetClickThrough(window);
  resetNativeWidgetWindowShape(window);
  widgetWindowPanelExpanded = false;
  widgetWindowRestoreState = null;
  window.setAlwaysOnTop(false);
}

function easeOutCubic(value: number): number {
  return 1 - Math.pow(1 - value, 3);
}

function animateNativeWidgetWindowBounds(window: BrowserWindow, targetBounds: Electron.Rectangle, durationMs = WIDGET_TASKBAR_SLIDE_MS): void {
  if (window.isDestroyed()) {
    return;
  }
  clearWidgetWindowBoundsAnimationTimer();
  const startBounds = window.getBounds();
  const duration = Math.max(80, Math.min(900, Math.round(durationMs)));
  const startedAt = Date.now();
  const applyFrame = () => {
    if (window.isDestroyed()) {
      clearWidgetWindowBoundsAnimationTimer();
      return;
    }
    const progress = Math.min(1, (Date.now() - startedAt) / duration);
    const eased = easeOutCubic(progress);
    const nextBounds = {
      x: Math.round(startBounds.x + (targetBounds.x - startBounds.x) * eased),
      y: Math.round(startBounds.y + (targetBounds.y - startBounds.y) * eased),
      width: Math.round(startBounds.width + (targetBounds.width - startBounds.width) * eased),
      height: Math.round(startBounds.height + (targetBounds.height - startBounds.height) * eased)
    };
    window.setBounds(nextBounds, false);
    if (progress >= 1) {
      clearWidgetWindowBoundsAnimationTimer();
      window.setBounds(targetBounds, false);
    }
  };
  applyFrame();
  widgetWindowBoundsAnimationTimer = setInterval(applyFrame, 16);
}

function finishNativeWidgetWindowRestore(window: BrowserWindow, restoreState: WidgetWindowRestoreState | null, restoreBounds: Electron.Rectangle): void {
  if (window.isDestroyed() || widgetWindowRestoreState !== null) {
    return;
  }
  clearWidgetWindowBoundsAnimationTimer();
  window.setBounds(restoreBounds, false);
  if (restoreState) {
    window.setMinimumSize(restoreState.minimumSize[0], restoreState.minimumSize[1]);
    if (restoreState.fullScreen) {
      window.setFullScreen(true);
    } else if (restoreState.maximized) {
      window.maximize();
    }
  } else {
    window.setMinimumSize(1180, 760);
  }
  window.setAlwaysOnTop(false);
  window.show();
  window.focus();
  traceWidgetTaskbarStep("restore-widget-window-animation-finished", {
    id: window.id,
    restoreBounds,
    maximized: restoreState?.maximized ?? false,
    fullScreen: restoreState?.fullScreen ?? false
  });
}

function saveWidgetWindowRestoreState(window: BrowserWindow): void {
  if (widgetWindowRestoreState !== null) {
    return;
  }
  const [minWidth, minHeight] = window.getMinimumSize();
  const currentBounds = window.getBounds();
  const restoreBounds = boundsLookLikeWidget(currentBounds)
    ? normalWindowRestoreBounds(window)
    : currentBounds;
  widgetWindowRestoreState = {
    bounds: restoreBounds,
    minimumSize: [minWidth, minHeight],
    maximized: window.isMaximized() && !boundsLookLikeWidget(currentBounds),
    fullScreen: window.isFullScreen()
  };
  traceWidgetTaskbarStep("save-widget-restore-state", {
    id: window.id,
    currentBounds,
    restoreBounds,
    currentLookedLikeWidget: boundsLookLikeWidget(currentBounds)
  });
}

function applyNativeWidgetWindowBounds(window: BrowserWindow): void {
  if (window.isDestroyed()) {
    return;
  }
  if (window.isFullScreen()) {
    window.setFullScreen(false);
  }
  if (window.isMaximized()) {
    window.unmaximize();
  }
  if (window.isMinimized()) {
    window.restore();
  }
  window.setBackgroundColor(TRANSPARENT_WINDOW_BACKGROUND);
  window.setMinimumSize(420, 128);
  window.setAlwaysOnTop(true, "floating");
  const bounds = widgetWindowBounds(window, { taskbarHidden: true });
  console.info("Applying native widget window bounds", { id: window.id, bounds, plannedTaskbarHidden: true });
  traceWidgetTaskbarStep("apply-widget-bounds", { id: window.id, bounds, plannedTaskbarHidden: true });
  clearWidgetWindowBoundsAnimationTimer();
  window.show();
  window.focus();
  animateNativeWidgetWindowBounds(window, bounds, WIDGET_TASKBAR_SLIDE_MS);
}

/* When the taskbar is toggled while the widget strip is already on screen,
   slide the strip to the matching anchor: flush to the screen bottom when the
   bar is hidden, lifted above the bar when it is shown again. */
function repositionWidgetStripForTaskbar(window: BrowserWindow, taskbarHidden: boolean): void {
  if (window.isDestroyed() || widgetWindowRestoreState === null) {
    return;
  }
  const bounds = widgetWindowBounds(window, { taskbarHidden });
  traceWidgetTaskbarStep("reposition-widget-strip", { id: window.id, bounds, taskbarHidden });
  clearWidgetWindowBoundsAnimationTimer();
  window.setBackgroundColor(TRANSPARENT_WINDOW_BACKGROUND);
  window.setAlwaysOnTop(true, "floating");
  animateNativeWidgetWindowBounds(window, bounds, WIDGET_TASKBAR_SLIDE_MS);
}

function armNativeWidgetTaskbarAutoHide(window: BrowserWindow): void {
  if (process.platform !== "win32" || window.isDestroyed() || widgetWindowRestoreState === null) {
    traceWidgetTaskbarStep("skip-arm-taskbar-autohide", {
      platform: process.platform,
      destroyed: window.isDestroyed(),
      hasRestoreState: widgetWindowRestoreState !== null
    });
    return;
  }
  traceWidgetTaskbarStep("arm-taskbar-autohide", { id: window.id });
  void scheduleNativeWidgetTaskbarAutoHide(window, true);
}

function sanitizeWidgetHitRegions(value: unknown, window: BrowserWindow): Electron.Rectangle[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const bounds = window.getBounds();
  const maxWidth = Math.max(1, bounds.width);
  const maxHeight = Math.max(1, bounds.height);
  const regions: Electron.Rectangle[] = [];
  for (const item of value) {
    if (!item || typeof item !== "object") {
      continue;
    }
    const candidate = item as Partial<Electron.Rectangle>;
    const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? Math.round(candidate.x) : NaN;
    const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? Math.round(candidate.y) : NaN;
    const width = typeof candidate.width === "number" && Number.isFinite(candidate.width) ? Math.round(candidate.width) : NaN;
    const height = typeof candidate.height === "number" && Number.isFinite(candidate.height) ? Math.round(candidate.height) : NaN;
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(width) || !Number.isFinite(height)) {
      continue;
    }
    const left = Math.max(0, Math.min(maxWidth, x));
    const top = Math.max(0, Math.min(maxHeight, y));
    const right = Math.max(left, Math.min(maxWidth, x + Math.max(0, width)));
    const bottom = Math.max(top, Math.min(maxHeight, y + Math.max(0, height)));
    if (right - left < 8 || bottom - top < 8) {
      continue;
    }
    regions.push({ x: left, y: top, width: right - left, height: bottom - top });
  }
  return regions.slice(0, 24);
}

function applyNativeWidgetWindowShape(window: BrowserWindow, regions: Electron.Rectangle[]): void {
  const shapedWindow = window as BrowserWindow & { setShape?: (rects: Electron.Rectangle[]) => void };
  if (typeof shapedWindow.setShape !== "function") {
    return;
  }
  try {
    shapedWindow.setShape(regions);
  } catch (error) {
    console.warn("Failed to apply native widget hit regions", error);
  }
}

function resetNativeWidgetWindowShape(window: BrowserWindow): void {
  widgetWindowHitRegions = [];
  applyNativeWidgetWindowShape(window, []);
}

function applyNativeWidgetClickThrough(window: BrowserWindow, enabled: boolean): void {
  if (widgetWindowClickThrough === enabled) {
    return;
  }
  widgetWindowClickThrough = enabled;
  window.setIgnoreMouseEvents(enabled, enabled ? { forward: true } : undefined);
}

function resetNativeWidgetClickThrough(window: BrowserWindow): void {
  widgetWindowClickThrough = false;
  window.setIgnoreMouseEvents(false);
}

type WindowsTaskbarStateProbe = {
  ok: boolean;
  state: number;
  target: number;
  taskbarHandleFound: boolean;
  screenWidth: number;
  screenHeight: number;
  taskbarRect: {
    left: number;
    top: number;
    right: number;
    bottom: number;
  };
  taskbarVisiblyHidden: boolean;
  registryPathFound: boolean;
  registryByte: number;
};

type WindowsTaskbarFastResult = {
  ok: boolean;
  target: number;
  registryPathFound: boolean;
  registryByte: number;
};

type WindowsTaskbarNativeResult = {
  ok: boolean;
  command?: string;
  hidden?: boolean;
  current?: number;
  target?: number;
  elapsedMs?: number;
  registryPathFound?: boolean;
  registryByte?: number;
  error?: string;
  ready?: boolean;
  service?: string;
};

let widgetTaskbarNativeHelper: ChildProcessWithoutNullStreams | null = null;
let widgetTaskbarNativeHelperBuffer = "";
const widgetTaskbarNativeRequests: Array<(result: WindowsTaskbarNativeResult | null) => void> = [];

function findWindowsTaskbarNativeHelper(): string | null {
  const executableName = process.platform === "win32" ? "ingen_windows_taskbar_helper.exe" : "ingen_windows_taskbar_helper";
  const candidates = [
    process.env.FORGE_WINDOWS_TASKBAR_HELPER_EXE ?? "",
    join(repoRoot, ".codex-targets", "ingen-electron-shortcut", "debug", executableName),
    join(repoRoot, "examples", "ingen_native_services", "target", "debug", executableName),
    join(repoRoot, "target", "debug", executableName),
    join(process.resourcesPath ?? "", executableName),
    join(dirname(app.getPath("exe")), executableName)
  ];
  for (const candidate of candidates) {
    if (candidate && existsSync(candidate)) {
      return candidate;
    }
  }
  return null;
}

function resetWindowsTaskbarNativeHelper(): void {
  const pending = widgetTaskbarNativeRequests.splice(0);
  for (const resolve of pending) {
    resolve(null);
  }
  if (widgetTaskbarNativeHelper && !widgetTaskbarNativeHelper.killed) {
    widgetTaskbarNativeHelper.kill();
  }
  widgetTaskbarNativeHelper = null;
  widgetTaskbarNativeHelperBuffer = "";
}

function handleWindowsTaskbarNativeHelperLine(line: string): void {
  if (line.trim() === "") {
    return;
  }
  let parsed: WindowsTaskbarNativeResult | null = null;
  try {
    parsed = JSON.parse(line) as WindowsTaskbarNativeResult;
  } catch (error) {
    console.warn("Windows taskbar native helper response was invalid.", error);
  }
  if (parsed?.ready === true) {
    traceWidgetTaskbarStep("taskbar-native-helper-ready", { service: parsed.service ?? "" });
    return;
  }
  const resolve = widgetTaskbarNativeRequests.shift();
  if (resolve) {
    resolve(parsed);
  }
}

function ensureWindowsTaskbarNativeHelper(): ChildProcessWithoutNullStreams | null {
  if (process.platform !== "win32") {
    return null;
  }
  if (widgetTaskbarNativeHelper && !widgetTaskbarNativeHelper.killed) {
    return widgetTaskbarNativeHelper;
  }
  const helperPath = findWindowsTaskbarNativeHelper();
  if (!helperPath) {
    traceWidgetTaskbarStep("taskbar-native-helper-missing");
    return null;
  }
  const child = spawn(helperPath, [], { windowsHide: true });
  widgetTaskbarNativeHelper = child;
  widgetTaskbarNativeHelperBuffer = "";
  traceWidgetTaskbarStep("taskbar-native-helper-started", { helperPath });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    widgetTaskbarNativeHelperBuffer += chunk;
    let newlineIndex = widgetTaskbarNativeHelperBuffer.indexOf("\n");
    while (newlineIndex >= 0) {
      const line = widgetTaskbarNativeHelperBuffer.slice(0, newlineIndex).trim();
      widgetTaskbarNativeHelperBuffer = widgetTaskbarNativeHelperBuffer.slice(newlineIndex + 1);
      handleWindowsTaskbarNativeHelperLine(line);
      newlineIndex = widgetTaskbarNativeHelperBuffer.indexOf("\n");
    }
  });
  child.stderr.on("data", (chunk: string) => {
    if (chunk.trim() !== "") {
      console.warn("Windows taskbar native helper stderr", chunk.trim());
    }
  });
  child.on("error", (error) => {
    console.warn("Windows taskbar native helper failed.", error);
    resetWindowsTaskbarNativeHelper();
  });
  child.on("exit", () => {
    resetWindowsTaskbarNativeHelper();
  });
  return child;
}

function runWindowsTaskbarNativeHelperAsync(hidden: boolean): Promise<WindowsTaskbarNativeResult | null> {
  const helper = ensureWindowsTaskbarNativeHelper();
  if (!helper) {
    return Promise.resolve(null);
  }
  const startedAt = Date.now();
  return new Promise((resolve) => {
    const finish = (result: WindowsTaskbarNativeResult | null): void => {
      clearTimeout(timeout);
      traceWidgetTaskbarStep("taskbar-native-command-finished", {
        hidden,
        accepted: result?.ok === true,
        helperElapsedMs: result?.elapsedMs ?? null,
        elapsedMs: Date.now() - startedAt,
        error: result?.error ?? ""
      });
      resolve(result);
    };
    const timeout = setTimeout(() => {
      const index = widgetTaskbarNativeRequests.indexOf(finish);
      if (index >= 0) {
        widgetTaskbarNativeRequests.splice(index, 1);
      }
      finish(null);
    }, 1_500);
    widgetTaskbarNativeRequests.push(finish);
    helper.stdin.write(hidden ? "hide\n" : "show\n", (error) => {
      if (error) {
        const index = widgetTaskbarNativeRequests.indexOf(finish);
        if (index >= 0) {
          widgetTaskbarNativeRequests.splice(index, 1);
        }
        console.warn("Windows taskbar native helper write failed.", error);
        finish(null);
      }
    });
  });
}

function buildWindowsTaskbarAutoHideFastScript(hidden: boolean): string {
  const desiredHidden = hidden ? "$true" : "$false";
  return `
$ErrorActionPreference = "Stop"
$startedAt = [System.Diagnostics.Stopwatch]::StartNew()
$source = @"
using System;
using System.Runtime.InteropServices;
public static class InGenTaskbarFast {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT {
    public Int32 left;
    public Int32 top;
    public Int32 right;
    public Int32 bottom;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct APPBARDATA {
    public UInt32 cbSize;
    public IntPtr hWnd;
    public UInt32 uCallbackMessage;
    public UInt32 uEdge;
    public RECT rc;
    public IntPtr lParam;
  }
  [DllImport("shell32.dll", SetLastError=true)]
  public static extern UIntPtr SHAppBarMessage(UInt32 dwMessage, ref APPBARDATA pData);
  [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
  public static extern IntPtr FindWindow(String lpClassName, String lpWindowName);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, UInt32 Msg, UIntPtr wParam, String lParam, UInt32 fuFlags, UInt32 uTimeout, out UIntPtr lpdwResult);
  public const UInt32 ABM_GETSTATE = 0x00000004;
  public const UInt32 ABM_SETSTATE = 0x0000000A;
  public const Int32 ABS_AUTOHIDE = 0x0000001;
  public static readonly IntPtr HWND_BROADCAST = new IntPtr(0xffff);
  public const UInt32 WM_SETTINGCHANGE = 0x001A;
  public const UInt32 SMTO_ABORTIFHUNG = 0x0002;
}
"@
Add-Type -TypeDefinition $source
function Send-InGenTraySettingsChangedFast {
  $result = [UIntPtr]::Zero
  [void][InGenTaskbarFast]::SendMessageTimeout([InGenTaskbarFast]::HWND_BROADCAST, [InGenTaskbarFast]::WM_SETTINGCHANGE, [UIntPtr]::Zero, "TraySettings", [InGenTaskbarFast]::SMTO_ABORTIFHUNG, 120, [ref]$result)
}
$hidden = ${desiredHidden}
$taskbarHandle = [InGenTaskbarFast]::FindWindow("Shell_TrayWnd", $null)
$data = New-Object InGenTaskbarFast+APPBARDATA
$data.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][InGenTaskbarFast+APPBARDATA])
$data.hWnd = $taskbarHandle
$current = [int]([InGenTaskbarFast]::SHAppBarMessage([InGenTaskbarFast]::ABM_GETSTATE, [ref]$data).ToUInt32())
$target = if ($hidden) { $current -bor [InGenTaskbarFast]::ABS_AUTOHIDE } else { $current -band (-bnot [InGenTaskbarFast]::ABS_AUTOHIDE) }
$data.lParam = [IntPtr]$target
[void][InGenTaskbarFast]::SHAppBarMessage([InGenTaskbarFast]::ABM_SETSTATE, [ref]$data)
$registryPath = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StuckRects3"
$registryFound = Test-Path -LiteralPath $registryPath
$registryByte = -1
if ($registryFound) {
  $settings = [byte[]](Get-ItemProperty -LiteralPath $registryPath -Name Settings).Settings
  if ($settings.Length -gt 8) {
    $registryByte = [int]$settings[8]
    $nextRegistryByte = if ($hidden) { $registryByte -bor [InGenTaskbarFast]::ABS_AUTOHIDE } else { $registryByte -band (-bnot [InGenTaskbarFast]::ABS_AUTOHIDE) }
    $settings[8] = [byte]$nextRegistryByte
    Set-ItemProperty -LiteralPath $registryPath -Name Settings -Value $settings
    $registryByte = [int]$settings[8]
    Send-InGenTraySettingsChangedFast
  }
}
@{
  ok = $true
  target = $target
  registryPathFound = $registryFound
  registryByte = $registryByte
  elapsedMs = [int]$startedAt.ElapsedMilliseconds
} | ConvertTo-Json -Compress
`;
}

function parseWindowsTaskbarFastResult(stdout: string): WindowsTaskbarFastResult | null {
  try {
    const parsed = JSON.parse(stdout.trim()) as Partial<WindowsTaskbarFastResult>;
    if (
      parsed.ok === true &&
      typeof parsed.target === "number" &&
      typeof parsed.registryPathFound === "boolean" &&
      typeof parsed.registryByte === "number"
    ) {
      return {
        ok: true,
        target: parsed.target,
        registryPathFound: parsed.registryPathFound,
        registryByte: parsed.registryByte
      };
    }
  } catch (error) {
    console.warn("Windows taskbar fast response was invalid.", error);
  }
  return null;
}

function runWindowsTaskbarAutoHideFastAsync(hidden: boolean): Promise<WindowsTaskbarFastResult | null> {
  if (process.platform !== "win32") {
    return Promise.resolve(null);
  }
  const script = buildWindowsTaskbarAutoHideFastScript(hidden);
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  const startedAt = Date.now();
  return new Promise((resolve) => {
    const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encoded], {
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = (result: WindowsTaskbarFastResult | null): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      traceWidgetTaskbarStep("taskbar-fast-command-finished", { hidden, accepted: result !== null, elapsedMs: Date.now() - startedAt });
      resolve(result);
    };
    const timeout = setTimeout(() => {
      child.kill();
      console.warn("Windows taskbar fast command timed out.");
      finish(null);
    }, 2_000);
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk: string) => {
      stdout += chunk;
    });
    child.stderr?.on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      console.warn("Windows taskbar fast command failed to start.", error);
      finish(null);
    });
    child.on("close", (code) => {
      if (code !== 0) {
        console.warn("Windows taskbar fast command failed.", stderr || stdout);
        finish(null);
        return;
      }
      finish(parseWindowsTaskbarFastResult(stdout));
    });
  });
}

function buildWindowsTaskbarAutoHideProbeScript(targetState?: number, targetRegistryByte?: number): string {
  const desiredState = targetState === undefined ? "$null" : String(Math.max(0, Math.round(targetState)));
  const desiredRegistryByte = targetRegistryByte === undefined ? "$null" : String(Math.max(0, Math.min(255, Math.round(targetRegistryByte))));
  return `
$ErrorActionPreference = "Stop"
$source = @"
using System;
using System.Runtime.InteropServices;
public static class InGenTaskbarState {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT {
    public Int32 left;
    public Int32 top;
    public Int32 right;
    public Int32 bottom;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct APPBARDATA {
    public UInt32 cbSize;
    public IntPtr hWnd;
    public UInt32 uCallbackMessage;
    public UInt32 uEdge;
    public RECT rc;
    public IntPtr lParam;
  }
  [DllImport("shell32.dll", SetLastError=true)]
  public static extern UIntPtr SHAppBarMessage(UInt32 dwMessage, ref APPBARDATA pData);
  [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
  public static extern IntPtr FindWindow(String lpClassName, String lpWindowName);
  [DllImport("user32.dll", SetLastError=true)]
  public static extern Boolean GetWindowRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll", SetLastError=true)]
  public static extern Int32 GetSystemMetrics(Int32 nIndex);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, UInt32 Msg, UIntPtr wParam, String lParam, UInt32 fuFlags, UInt32 uTimeout, out UIntPtr lpdwResult);
  public const UInt32 ABM_GETSTATE = 0x00000004;
  public const UInt32 ABM_SETSTATE = 0x0000000A;
  public const Int32 ABS_AUTOHIDE = 0x0000001;
  public const Int32 SM_CXSCREEN = 0;
  public const Int32 SM_CYSCREEN = 1;
  public static readonly IntPtr HWND_BROADCAST = new IntPtr(0xffff);
  public const UInt32 WM_SETTINGCHANGE = 0x001A;
  public const UInt32 SMTO_ABORTIFHUNG = 0x0002;
}
"@
Add-Type -TypeDefinition $source
function Send-InGenTraySettingsChanged {
  $result = [UIntPtr]::Zero
  [void][InGenTaskbarState]::SendMessageTimeout([InGenTaskbarState]::HWND_BROADCAST, [InGenTaskbarState]::WM_SETTINGCHANGE, [UIntPtr]::Zero, "TraySettings", [InGenTaskbarState]::SMTO_ABORTIFHUNG, 1500, [ref]$result)
}
$registryPath = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StuckRects3"
$registryFound = Test-Path -LiteralPath $registryPath
$registryByte = -1
$desiredRegistryByte = ${desiredRegistryByte}
if ($registryFound) {
  $settings = [byte[]](Get-ItemProperty -LiteralPath $registryPath -Name Settings).Settings
  if ($settings.Length -gt 8) {
    $registryByte = [int]$settings[8]
    if ($null -ne $desiredRegistryByte) {
      $settings[8] = [byte]$desiredRegistryByte
      Set-ItemProperty -LiteralPath $registryPath -Name Settings -Value $settings
      $registryByte = [int]$settings[8]
      Send-InGenTraySettingsChanged
      Start-Sleep -Milliseconds 180
    }
  }
}
$taskbarHandle = [InGenTaskbarState]::FindWindow("Shell_TrayWnd", $null)
$data = New-Object InGenTaskbarState+APPBARDATA
$data.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][InGenTaskbarState+APPBARDATA])
$data.hWnd = $taskbarHandle
$current = [int]([InGenTaskbarState]::SHAppBarMessage([InGenTaskbarState]::ABM_GETSTATE, [ref]$data).ToUInt32())
$desiredState = ${desiredState}
$target = $current
if ($null -ne $desiredState) {
  $target = [int]$desiredState
  $data.lParam = [IntPtr]$target
  [void][InGenTaskbarState]::SHAppBarMessage([InGenTaskbarState]::ABM_SETSTATE, [ref]$data)
  Start-Sleep -Milliseconds 650
  $data = New-Object InGenTaskbarState+APPBARDATA
  $data.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][InGenTaskbarState+APPBARDATA])
  $data.hWnd = $taskbarHandle
  $target = [int]([InGenTaskbarState]::SHAppBarMessage([InGenTaskbarState]::ABM_GETSTATE, [ref]$data).ToUInt32())
}
if ($registryFound -and $null -ne $desiredRegistryByte) {
  $settings = [byte[]](Get-ItemProperty -LiteralPath $registryPath -Name Settings).Settings
  if ($settings.Length -gt 8) {
    $settings[8] = [byte]$desiredRegistryByte
    Set-ItemProperty -LiteralPath $registryPath -Name Settings -Value $settings
    $registryByte = [int]$settings[8]
    Send-InGenTraySettingsChanged
    Start-Sleep -Milliseconds 220
  }
}
if ($registryFound) {
  $settings = [byte[]](Get-ItemProperty -LiteralPath $registryPath -Name Settings).Settings
  if ($settings.Length -gt 8) {
    $registryByte = [int]$settings[8]
  }
}
$screenWidth = [InGenTaskbarState]::GetSystemMetrics([InGenTaskbarState]::SM_CXSCREEN)
$screenHeight = [InGenTaskbarState]::GetSystemMetrics([InGenTaskbarState]::SM_CYSCREEN)
$rect = New-Object InGenTaskbarState+RECT
$rectFound = $false
if ($taskbarHandle -ne [IntPtr]::Zero) {
  $rectFound = [InGenTaskbarState]::GetWindowRect($taskbarHandle, [ref]$rect)
}
$hiddenTolerance = 4
$visiblyHidden = $rectFound -and (
  $rect.top -ge ($screenHeight - $hiddenTolerance) -or
  $rect.bottom -le $hiddenTolerance -or
  $rect.left -ge ($screenWidth - $hiddenTolerance) -or
  $rect.right -le $hiddenTolerance
)
@{
  ok = $true
  state = $current
  target = $target
  taskbarHandleFound = ($taskbarHandle -ne [IntPtr]::Zero)
  screenWidth = $screenWidth
  screenHeight = $screenHeight
  taskbarRect = @{ left = $rect.left; top = $rect.top; right = $rect.right; bottom = $rect.bottom }
  taskbarVisiblyHidden = $visiblyHidden
  registryPathFound = $registryFound
  registryByte = $registryByte
} | ConvertTo-Json -Compress
`;
}

function parseWindowsTaskbarAutoHideProbe(stdout: string): WindowsTaskbarStateProbe | null {
  try {
    const parsed = JSON.parse(stdout.trim()) as Partial<WindowsTaskbarStateProbe>;
    if (
      parsed.ok === true &&
      typeof parsed.state === "number" &&
      typeof parsed.target === "number" &&
      typeof parsed.taskbarHandleFound === "boolean" &&
      typeof parsed.screenWidth === "number" &&
      typeof parsed.screenHeight === "number" &&
      typeof parsed.taskbarRect === "object" &&
      parsed.taskbarRect !== null &&
      typeof (parsed.taskbarRect as Partial<WindowsTaskbarStateProbe["taskbarRect"]>).left === "number" &&
      typeof (parsed.taskbarRect as Partial<WindowsTaskbarStateProbe["taskbarRect"]>).top === "number" &&
      typeof (parsed.taskbarRect as Partial<WindowsTaskbarStateProbe["taskbarRect"]>).right === "number" &&
      typeof (parsed.taskbarRect as Partial<WindowsTaskbarStateProbe["taskbarRect"]>).bottom === "number" &&
      typeof parsed.taskbarVisiblyHidden === "boolean" &&
      typeof parsed.registryPathFound === "boolean" &&
      typeof parsed.registryByte === "number"
    ) {
      return {
        ok: true,
        state: parsed.state,
        target: parsed.target,
        taskbarHandleFound: parsed.taskbarHandleFound,
        screenWidth: parsed.screenWidth,
        screenHeight: parsed.screenHeight,
        taskbarRect: parsed.taskbarRect as WindowsTaskbarStateProbe["taskbarRect"],
        taskbarVisiblyHidden: parsed.taskbarVisiblyHidden,
        registryPathFound: parsed.registryPathFound,
        registryByte: parsed.registryByte
      };
    }
  } catch (error) {
    console.warn("Windows taskbar auto-hide response was invalid.", error);
  }
  return null;
}

function runWindowsTaskbarAutoHideProbe(targetState?: number, targetRegistryByte?: number): WindowsTaskbarStateProbe | null {
  if (process.platform !== "win32") {
    return null;
  }
  const script = buildWindowsTaskbarAutoHideProbeScript(targetState, targetRegistryByte);
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encoded], {
    encoding: "utf8",
    stdio: "pipe",
    timeout: 8_000,
    windowsHide: true
  });
  if (result.status !== 0) {
    console.warn("Windows taskbar auto-hide command failed.", result.stderr || result.stdout);
    return null;
  }
  return parseWindowsTaskbarAutoHideProbe(result.stdout);
}

function runWindowsTaskbarAutoHideProbeAsync(targetState?: number, targetRegistryByte?: number): Promise<WindowsTaskbarStateProbe | null> {
  if (process.platform !== "win32") {
    return Promise.resolve(null);
  }
  const script = buildWindowsTaskbarAutoHideProbeScript(targetState, targetRegistryByte);
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  return new Promise((resolve) => {
    const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encoded], {
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = (probe: WindowsTaskbarStateProbe | null): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      resolve(probe);
    };
    const timeout = setTimeout(() => {
      child.kill();
      console.warn("Windows taskbar auto-hide command timed out.");
      finish(null);
    }, 8_000);
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk: string) => {
      stdout += chunk;
    });
    child.stderr?.on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      console.warn("Windows taskbar auto-hide command failed to start.", error);
      finish(null);
    });
    child.on("close", (code) => {
      if (code !== 0) {
        console.warn("Windows taskbar auto-hide command failed.", stderr || stdout);
        finish(null);
        return;
      }
      finish(parseWindowsTaskbarAutoHideProbe(stdout));
    });
  });
}

function setWidgetTaskbarHidden(hidden: boolean, restoreOriginal = false): boolean {
  if (process.platform !== "win32") {
    return false;
  }
  const current = runWindowsTaskbarAutoHideProbe();
  if (!current) {
    return false;
  }
  if (widgetTaskbarOriginalState === null) {
    widgetTaskbarOriginalState = current.target;
  }
  if (widgetTaskbarOriginalRegistryByte === null && current.registryPathFound && current.registryByte >= 0) {
    widgetTaskbarOriginalRegistryByte = current.registryByte;
  }
  const targetState = restoreOriginal && widgetTaskbarOriginalState !== null
    ? widgetTaskbarOriginalState
    : hidden
      ? current.target | WINDOWS_TASKBAR_AUTOHIDE_FLAG
      : current.target & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
  const targetRegistryByte = restoreOriginal && widgetTaskbarOriginalRegistryByte !== null
    ? widgetTaskbarOriginalRegistryByte
    : current.registryPathFound && current.registryByte >= 0
      ? hidden
        ? current.registryByte | WINDOWS_TASKBAR_AUTOHIDE_FLAG
        : current.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG
      : undefined;
  const result = runWindowsTaskbarAutoHideProbe(targetState, targetRegistryByte);
  if (!result) {
    return false;
  }
  widgetTaskbarHidden = result.registryPathFound && result.registryByte >= 0
    ? (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0
    : (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0;
  const targetHidden = (targetState & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0;
  const verifiedState = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === (targetState & WINDOWS_TASKBAR_AUTOHIDE_FLAG);
  const verifiedRegistry =
    targetRegistryByte === undefined ||
    !result.registryPathFound ||
    result.registryByte < 0 ||
    (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === (targetRegistryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG);
  const verifiedVisual = !targetHidden || result.taskbarVisiblyHidden;
  const verified = verifiedState && verifiedRegistry && verifiedVisual;
  console.info("Windows taskbar auto-hide probe", {
    requestedHidden: targetHidden,
    verified,
    verifiedState,
    verifiedRegistry,
    verifiedVisual,
    appBarState: result.target,
    registryByte: result.registryByte,
    screen: { width: result.screenWidth, height: result.screenHeight },
    taskbarRect: result.taskbarRect
  });
  traceWidgetTaskbarStep("taskbar-autohide-probe", {
    requestedHidden: targetHidden,
    verified,
    verifiedState,
    verifiedRegistry,
    verifiedVisual,
    appBarState: result.target,
    registryByte: result.registryByte,
    screen: { width: result.screenWidth, height: result.screenHeight },
    taskbarRect: result.taskbarRect
  });
  if (!verified) {
    console.warn("Windows taskbar auto-hide probe did not reach the requested visible state.", {
      requestedHidden: targetHidden,
      appBarState: result.target,
      registryByte: result.registryByte,
      taskbarRect: result.taskbarRect,
      screen: { width: result.screenWidth, height: result.screenHeight }
    });
  }
  if (restoreOriginal) {
    widgetTaskbarOriginalState = null;
    widgetTaskbarOriginalRegistryByte = null;
    widgetTaskbarHidden = false;
  }
  return true;
}

async function setWidgetTaskbarHiddenAsync(hidden: boolean, restoreOriginal = false): Promise<boolean> {
  if (process.platform !== "win32") {
    return false;
  }
  const current = await runWindowsTaskbarAutoHideProbeAsync();
  if (!current) {
    return false;
  }
  if (widgetTaskbarOriginalState === null) {
    widgetTaskbarOriginalState = current.target;
  }
  if (widgetTaskbarOriginalRegistryByte === null && current.registryPathFound && current.registryByte >= 0) {
    widgetTaskbarOriginalRegistryByte = current.registryByte;
  }
  const targetState = restoreOriginal && widgetTaskbarOriginalState !== null
    ? widgetTaskbarOriginalState
    : hidden
      ? current.target | WINDOWS_TASKBAR_AUTOHIDE_FLAG
      : current.target & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
  const targetRegistryByte = restoreOriginal && widgetTaskbarOriginalRegistryByte !== null
    ? widgetTaskbarOriginalRegistryByte
    : current.registryPathFound && current.registryByte >= 0
      ? hidden
        ? current.registryByte | WINDOWS_TASKBAR_AUTOHIDE_FLAG
        : current.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG
      : undefined;
  const result = await runWindowsTaskbarAutoHideProbeAsync(targetState, targetRegistryByte);
  if (!result) {
    return false;
  }
  widgetTaskbarHidden = result.registryPathFound && result.registryByte >= 0
    ? (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0
    : (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0;
  const targetHidden = (targetState & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0;
  const verifiedState = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === (targetState & WINDOWS_TASKBAR_AUTOHIDE_FLAG);
  const verifiedRegistry =
    targetRegistryByte === undefined ||
    !result.registryPathFound ||
    result.registryByte < 0 ||
    (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === (targetRegistryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG);
  const verifiedVisual = !targetHidden || result.taskbarVisiblyHidden;
  const verified = verifiedState && verifiedRegistry && verifiedVisual;
  console.info("Windows taskbar auto-hide probe", {
    requestedHidden: targetHidden,
    verified,
    verifiedState,
    verifiedRegistry,
    verifiedVisual,
    appBarState: result.target,
    registryByte: result.registryByte,
    screen: { width: result.screenWidth, height: result.screenHeight },
    taskbarRect: result.taskbarRect
  });
  traceWidgetTaskbarStep("taskbar-autohide-probe", {
    requestedHidden: targetHidden,
    verified,
    verifiedState,
    verifiedRegistry,
    verifiedVisual,
    appBarState: result.target,
    registryByte: result.registryByte,
    screen: { width: result.screenWidth, height: result.screenHeight },
    taskbarRect: result.taskbarRect
  });
  if (!verified) {
    console.warn("Windows taskbar auto-hide probe did not reach the requested visible state.", {
      requestedHidden: targetHidden,
      appBarState: result.target,
      registryByte: result.registryByte,
      taskbarRect: result.taskbarRect,
      screen: { width: result.screenWidth, height: result.screenHeight }
    });
  }
  if (restoreOriginal) {
    widgetTaskbarOriginalState = null;
    widgetTaskbarOriginalRegistryByte = null;
    widgetTaskbarHidden = false;
  }
  return true;
}

function restoreWidgetTaskbarState(): boolean {
  if (process.platform !== "win32") {
    return true;
  }
  const current = runWindowsTaskbarAutoHideProbe();
  if (!current) {
    return false;
  }
  const targetState = current.target & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
  const targetRegistryByte = current.registryPathFound && current.registryByte >= 0
    ? current.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG
    : undefined;
  let result = runWindowsTaskbarAutoHideProbe(targetState, targetRegistryByte);
  widgetTaskbarOriginalState = null;
  widgetTaskbarOriginalRegistryByte = null;
  widgetTaskbarHidden = false;
  if (!result) {
    return false;
  }
  let stateVisible = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
  let registryVisible =
    !result.registryPathFound ||
    result.registryByte < 0 ||
    (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
  if (!stateVisible || !registryVisible) {
    const retryResult = runWindowsTaskbarAutoHideProbe(targetState, targetRegistryByte);
    if (retryResult) {
      result = retryResult;
      stateVisible = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
      registryVisible =
        !result.registryPathFound ||
        result.registryByte < 0 ||
        (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
    }
  }
  traceWidgetTaskbarStep("restore-taskbar-visible", {
    stateVisible,
    registryVisible,
    appBarState: result.target,
    registryByte: result.registryByte,
    taskbarRect: result.taskbarRect
  });
  return stateVisible && registryVisible;
}

async function restoreWidgetTaskbarStateAsync(): Promise<boolean> {
  if (process.platform !== "win32") {
    return true;
  }
  const current = await runWindowsTaskbarAutoHideProbeAsync();
  if (!current) {
    return false;
  }
  const targetState = current.target & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
  const targetRegistryByte = current.registryPathFound && current.registryByte >= 0
    ? current.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG
    : undefined;
  let result = await runWindowsTaskbarAutoHideProbeAsync(targetState, targetRegistryByte);
  widgetTaskbarOriginalState = null;
  widgetTaskbarOriginalRegistryByte = null;
  widgetTaskbarHidden = false;
  if (!result) {
    return false;
  }
  let stateVisible = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
  let registryVisible =
    !result.registryPathFound ||
    result.registryByte < 0 ||
    (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
  if (!stateVisible || !registryVisible) {
    const retryResult = await runWindowsTaskbarAutoHideProbeAsync(targetState, targetRegistryByte);
    if (retryResult) {
      result = retryResult;
      stateVisible = (result.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
      registryVisible =
        !result.registryPathFound ||
        result.registryByte < 0 ||
        (result.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) === 0;
    }
  }
  traceWidgetTaskbarStep("restore-taskbar-visible", {
    stateVisible,
    registryVisible,
    appBarState: result.target,
    registryByte: result.registryByte,
    taskbarRect: result.taskbarRect
  });
  return stateVisible && registryVisible;
}

function scheduleNativeWidgetTaskbarAutoHide(window: BrowserWindow, hidden: boolean): boolean {
  if (process.platform !== "win32") {
    return false;
  }
  if (hidden && widgetWindowRestoreState === null) {
    return false;
  }
  clearWidgetTaskbarAutoHideTimer();
  const token = ++widgetTaskbarAutoHideJobToken;
  traceWidgetTaskbarStep("taskbar-autohide-scheduled", { id: window.id, hidden, token });
  widgetTaskbarAutoHideTimer = setTimeout(() => {
    widgetTaskbarAutoHideTimer = null;
    void (async () => {
      if (token !== widgetTaskbarAutoHideJobToken) {
        return;
      }
      let accepted = false;
      const nativeResult = await runWindowsTaskbarNativeHelperAsync(hidden);
      if (nativeResult?.ok === true) {
        accepted = true;
        widgetTaskbarHidden = hidden;
        if (widgetTaskbarOriginalState === null && hidden && typeof nativeResult.current === "number") {
          widgetTaskbarOriginalState = nativeResult.current;
        }
        if (!hidden) {
          widgetTaskbarOriginalState = null;
          widgetTaskbarOriginalRegistryByte = null;
        }
        traceWidgetTaskbarStep("taskbar-native-applied", {
          id: window.id,
          hidden,
          current: nativeResult.current ?? null,
          target: nativeResult.target ?? null,
          registryPathFound: nativeResult.registryPathFound ?? null,
          registryByte: nativeResult.registryByte ?? null,
          helperElapsedMs: nativeResult.elapsedMs ?? null,
          token
        });
      } else {
        const fastResult = await runWindowsTaskbarAutoHideFastAsync(hidden);
        if (fastResult) {
        accepted = true;
        widgetTaskbarHidden = hidden;
        if (widgetTaskbarOriginalState === null && hidden) {
          widgetTaskbarOriginalState = fastResult.target & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
        }
        if (widgetTaskbarOriginalRegistryByte === null && hidden && fastResult.registryPathFound && fastResult.registryByte >= 0) {
          widgetTaskbarOriginalRegistryByte = fastResult.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG;
        }
        if (!hidden) {
          widgetTaskbarOriginalState = null;
          widgetTaskbarOriginalRegistryByte = null;
        }
        traceWidgetTaskbarStep("taskbar-fast-applied", {
          id: window.id,
          hidden,
          target: fastResult.target,
          registryPathFound: fastResult.registryPathFound,
          registryByte: fastResult.registryByte,
          token
        });
        } else {
          traceWidgetTaskbarStep("taskbar-fast-fallback", { id: window.id, hidden, token });
          accepted = hidden
            ? await setWidgetTaskbarHiddenAsync(true)
            : await restoreWidgetTaskbarStateAsync();
        }
      }
      if (token !== widgetTaskbarAutoHideJobToken) {
        return;
      }
      if (
        accepted &&
        !window.isDestroyed() &&
        widgetWindowRestoreState !== null &&
        boundsLookLikeWidget(window.getBounds())
      ) {
        /* Only when the strip is already on screen (taskbar toggled via the
           Windows icon): slide it to match. During the entry hide the window
           is still full-screen, so this is skipped and the shrink animation is
           left untouched. */
        repositionWidgetStripForTaskbar(window, hidden);
      }
      traceWidgetTaskbarStep("taskbar-autohide-finished", { id: window.id, hidden, accepted, token });
    })();
  }, 0);
  return true;
}

function finalizeWidgetTaskbarState(reason: string): void {
  clearWidgetTaskbarAutoHideTimer();
  widgetTaskbarAutoHideJobToken += 1;
  const hadOriginalState = widgetTaskbarOriginalState !== null || widgetTaskbarOriginalRegistryByte !== null;
  const restored = restoreWidgetTaskbarState();
  const current = runWindowsTaskbarAutoHideProbe();
  const stillAutoHidden =
    current !== null &&
    (((current.target & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0) ||
      (current.registryPathFound && current.registryByte >= 0 && (current.registryByte & WINDOWS_TASKBAR_AUTOHIDE_FLAG) !== 0));
  if (!hadOriginalState && stillAutoHidden) {
    const targetRegistryByte = current?.registryPathFound && current.registryByte >= 0
      ? current.registryByte & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG
      : undefined;
    void runWindowsTaskbarAutoHideProbe((current?.target ?? 0) & ~WINDOWS_TASKBAR_AUTOHIDE_FLAG, targetRegistryByte);
    widgetTaskbarHidden = false;
  }
  traceWidgetTaskbarStep("finalize-taskbar-state", { reason, restored, hadOriginalState, stillAutoHidden });
}

function setNativeWindowWidgetTaskbarAutoHide(event: Electron.IpcMainInvokeEvent, enabled: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget taskbar auto-hide from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  if (enabled === true) {
    if (widgetWindowRestoreState === null) {
      return false;
    }
    return scheduleNativeWidgetTaskbarAutoHide(window, true);
  }
  return scheduleNativeWidgetTaskbarAutoHide(window, false);
}

function toggleNativeWindowWidgetTaskbar(event: Electron.IpcMainInvokeEvent): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget taskbar toggle from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  if (widgetWindowRestoreState === null) {
    return false;
  }
  const nextHidden = !widgetTaskbarHidden;
  /* Slide the strip to its new anchor right away so the click feels instant;
     the taskbar helper (ShowWindow + bookkeeping) runs in parallel rather than
     gating the visible motion. */
  if (boundsLookLikeWidget(window.getBounds())) {
    repositionWidgetStripForTaskbar(window, nextHidden);
  }
  return scheduleNativeWidgetTaskbarAutoHide(window, nextHidden);
}

function setNativeWindowWidgetHitRegions(event: Electron.IpcMainInvokeEvent, regions: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget hit regions from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  if (widgetWindowRestoreState === null) {
    resetNativeWidgetWindowShape(window);
    return true;
  }
  const nextRegions = sanitizeWidgetHitRegions(regions, window);
  if (nextRegions.length === 0) {
    return true;
  }
  widgetWindowHitRegions = nextRegions;
  applyNativeWidgetWindowShape(window, widgetWindowHitRegions);
  return true;
}

function setNativeWindowWidgetPanelExpanded(event: Electron.IpcMainInvokeEvent, enabled: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget panel expansion from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  const expanded = enabled === true;
  widgetWindowPanelExpanded = expanded;
  if (widgetWindowRestoreState === null || !boundsLookLikeWidget(window.getBounds())) {
    return true;
  }
  const bounds = widgetWindowBounds(window);
  const currentBounds = window.getBounds();
  if (
    Math.abs(currentBounds.x - bounds.x) <= 1 &&
    Math.abs(currentBounds.y - bounds.y) <= 1 &&
    Math.abs(currentBounds.width - bounds.width) <= 1 &&
    Math.abs(currentBounds.height - bounds.height) <= 1
  ) {
    return true;
  }
  traceWidgetTaskbarStep("resize-widget-panel", { id: window.id, bounds, expanded });
  clearWidgetWindowBoundsAnimationTimer();
  window.setBackgroundColor(TRANSPARENT_WINDOW_BACKGROUND);
  window.setAlwaysOnTop(true, "floating");
  animateNativeWidgetWindowBounds(window, bounds, WIDGET_TASKBAR_SLIDE_MS);
  return true;
}

function setNativeWindowWidgetClickThrough(event: Electron.IpcMainInvokeEvent, enabled: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget click-through from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }
  if (widgetWindowRestoreState === null && enabled === true) {
    resetNativeWidgetClickThrough(window);
    return true;
  }
  applyNativeWidgetClickThrough(window, enabled === true);
  return true;
}

const WIDGET_WALLPAPER_SAMPLE_SCHEMA = "ingen.electron.widget.wallpaper_sample.v1" as const;
const WIDGET_WALLPAPER_THUMBNAIL_WIDTH = 640;
const WIDGET_WALLPAPER_MAX_SAMPLE_WIDTH = 960;
const WIDGET_WALLPAPER_MAX_SAMPLE_HEIGHT = 180;

function widgetWallpaperSampleResult(
  accepted: boolean,
  patch: Partial<Omit<WidgetWallpaperSampleResult, "accepted" | "schema" | "proofHash">> = {}
): WidgetWallpaperSampleResult {
  const result: WidgetWallpaperSampleResult = {
    accepted,
    schema: WIDGET_WALLPAPER_SAMPLE_SCHEMA,
    tone: patch.tone ?? "unknown",
    dominantLight: patch.dominantLight ?? false,
    luminance: patch.luminance ?? 0,
    lightRatio: patch.lightRatio ?? 0,
    sampleCount: patch.sampleCount ?? 0,
    displayId: patch.displayId ?? "",
    proofHash: "",
    error: patch.error
  };
  result.proofHash = hashJson({ widgetWallpaperSample: { ...result, proofHash: "" } });
  return result;
}

function normalizeWidgetWallpaperSampleBounds(value: unknown): WidgetWallpaperSampleBounds | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const candidate = value as Partial<WidgetWallpaperSampleBounds>;
  const x = Math.round(Number(candidate.x));
  const y = Math.round(Number(candidate.y));
  const width = Math.round(Number(candidate.width));
  const height = Math.round(Number(candidate.height));
  if (![x, y, width, height].every(Number.isFinite) || width < 12 || height < 8) {
    return null;
  }
  return {
    x: Math.max(0, x),
    y: Math.max(0, y),
    width: Math.min(width, WIDGET_WALLPAPER_MAX_SAMPLE_WIDTH),
    height: Math.min(height, WIDGET_WALLPAPER_MAX_SAMPLE_HEIGHT)
  };
}

function clampNativeSampleCoordinate(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

async function sampleNativeWidgetWallpaper(
  event: Electron.IpcMainInvokeEvent,
  boundsValue: unknown
): Promise<WidgetWallpaperSampleResult> {
  if (!validateSender(event)) {
    const proofHash = hashJson({ widgetWallpaperSample: "bad_sender", sender: event.senderFrame?.url ?? "" });
    return widgetWallpaperSampleResult(false, {
      error: {
        code: "bad_sender",
        message: "Widget wallpaper sampling rejected by sender validation.",
        proofHash
      }
    });
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    const proofHash = hashJson({ widgetWallpaperSample: "window_unavailable" });
    return widgetWallpaperSampleResult(false, {
      error: {
        code: "rust_unavailable",
        message: "Widget wallpaper owner window is unavailable.",
        proofHash
      }
    });
  }
  const bounds = normalizeWidgetWallpaperSampleBounds(boundsValue);
  if (!bounds) {
    const proofHash = hashJson({ widgetWallpaperSample: "bad_payload", boundsValue });
    return widgetWallpaperSampleResult(false, {
      error: {
        code: "bad_payload",
        message: "Widget wallpaper sample bounds are invalid.",
        proofHash
      }
    });
  }

  try {
    const windowBounds = window.getBounds();
    const screenBounds: Electron.Rectangle = {
      x: windowBounds.x + bounds.x,
      y: windowBounds.y + bounds.y,
      width: bounds.width,
      height: bounds.height
    };
    const display = screen.getDisplayMatching(screenBounds);
    const aspectRatio = Math.max(0.2, display.bounds.width / Math.max(1, display.bounds.height));
    const sources = await desktopCapturer.getSources({
      types: ["screen"],
      thumbnailSize: {
        width: WIDGET_WALLPAPER_THUMBNAIL_WIDTH,
        height: Math.max(96, Math.round(WIDGET_WALLPAPER_THUMBNAIL_WIDTH / aspectRatio))
      },
      fetchWindowIcons: false
    });
    const source =
      sources.find((candidate) => candidate.display_id === String(display.id)) ??
      (sources.length === 1 ? sources[0] : undefined) ??
      sources[0];
    if (!source || source.thumbnail.isEmpty()) {
      const proofHash = hashJson({ widgetWallpaperSample: "screen_source_unavailable", displayId: display.id });
      return widgetWallpaperSampleResult(false, {
        displayId: String(display.id),
        error: {
          code: "rust_unavailable",
          message: "Screen thumbnail source is unavailable.",
          proofHash
        }
      });
    }

    const imageSize = source.thumbnail.getSize();
    if (imageSize.width <= 0 || imageSize.height <= 0) {
      const proofHash = hashJson({ widgetWallpaperSample: "screen_thumbnail_empty", displayId: display.id });
      return widgetWallpaperSampleResult(false, {
        displayId: String(display.id),
        error: {
          code: "rust_unavailable",
          message: "Screen thumbnail is empty.",
          proofHash
        }
      });
    }

    const displayBounds = display.bounds;
    const sampleLeftDip = clampNativeSampleCoordinate(screenBounds.x - displayBounds.x, 0, displayBounds.width);
    const sampleTopDip = clampNativeSampleCoordinate(screenBounds.y - displayBounds.y, 0, displayBounds.height);
    const sampleRightDip = clampNativeSampleCoordinate(screenBounds.x + screenBounds.width - displayBounds.x, 0, displayBounds.width);
    const sampleBottomDip = clampNativeSampleCoordinate(screenBounds.y + screenBounds.height - displayBounds.y, 0, displayBounds.height);
    const displayWidth = Math.max(1, displayBounds.width);
    const displayHeight = Math.max(1, displayBounds.height);
    const cropLeft = clampNativeSampleCoordinate(Math.floor((sampleLeftDip / displayWidth) * imageSize.width), 0, imageSize.width - 1);
    const cropTop = clampNativeSampleCoordinate(Math.floor((sampleTopDip / displayHeight) * imageSize.height), 0, imageSize.height - 1);
    const cropRight = clampNativeSampleCoordinate(Math.ceil((sampleRightDip / displayWidth) * imageSize.width), cropLeft + 1, imageSize.width);
    const cropBottom = clampNativeSampleCoordinate(Math.ceil((sampleBottomDip / displayHeight) * imageSize.height), cropTop + 1, imageSize.height);
    const cropWidth = cropRight - cropLeft;
    const cropHeight = cropBottom - cropTop;
    const cropped = source.thumbnail
      .crop({ x: cropLeft, y: cropTop, width: cropWidth, height: cropHeight })
      .resize({
        width: Math.min(96, cropWidth),
        height: Math.min(36, cropHeight),
        quality: "good"
      });
    const sampleSize = cropped.getSize();
    const bitmap = cropped.toBitmap();
    const pixelCount = Math.min(sampleSize.width * sampleSize.height, Math.floor(bitmap.length / 4));
    if (pixelCount <= 0) {
      const proofHash = hashJson({ widgetWallpaperSample: "sample_empty", displayId: display.id, cropWidth, cropHeight });
      return widgetWallpaperSampleResult(false, {
        displayId: String(display.id),
        error: {
          code: "rust_unavailable",
          message: "Widget wallpaper sample is empty.",
          proofHash
        }
      });
    }

    let brightnessTotal = 0;
    let lightPixels = 0;
    for (let index = 0; index < pixelCount; index += 1) {
      const offset = index * 4;
      const luma = (0.2126 * bitmap[offset] + 0.7152 * bitmap[offset + 1] + 0.0722 * bitmap[offset + 2]) / 255;
      brightnessTotal += luma;
      if (luma >= 0.48) {
        lightPixels += 1;
      }
    }
    const luminance = brightnessTotal / pixelCount;
    const lightRatio = lightPixels / pixelCount;
    const dominantLight = luminance >= 0.42 || (luminance >= 0.34 && lightRatio >= 0.18) || lightRatio >= 0.34;
    return widgetWallpaperSampleResult(true, {
      tone: dominantLight ? "light" : "dark",
      dominantLight,
      luminance: Number(luminance.toFixed(4)),
      lightRatio: Number(lightRatio.toFixed(4)),
      sampleCount: pixelCount,
      displayId: String(display.id)
    });
  } catch (error) {
    const proofHash = hashJson({
      widgetWallpaperSample: "capture_failed",
      message: error instanceof Error ? error.message : String(error)
    });
    return widgetWallpaperSampleResult(false, {
      error: {
        code: "rust_unavailable",
        message: "Widget wallpaper sampling failed.",
        proofHash
      }
    });
  }
}

function setNativeWindowWidgetMode(event: Electron.IpcMainInvokeEvent, enabled: unknown, delayMs?: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window widget mode from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window || window.isDestroyed()) {
    return false;
  }

  if (enabled === true) {
    clearWidgetWindowShrinkTimer();
    clearWidgetTaskbarAutoHideTimer();
    widgetTaskbarAutoHideJobToken += 1;
    resetNativeWidgetClickThrough(window);
    resetNativeWidgetWindowShape(window);
    saveWidgetWindowRestoreState(window);
    window.setBackgroundColor(TRANSPARENT_WINDOW_BACKGROUND);
    window.setAlwaysOnTop(true, "floating");
    /* Hide the taskbar immediately, in lockstep with the MINI activation. The
       window is still full-screen here, so the strip reposition is skipped and
       the delayed shrink animation below stays smooth. */
    armNativeWidgetTaskbarAutoHide(window);
    const requestedDelay = typeof delayMs === "number" && Number.isFinite(delayMs) ? delayMs : WIDGET_WINDOW_SHRINK_DELAY_MS;
    const shrinkDelay = Math.max(0, Math.min(2_000, Math.round(requestedDelay)));
    console.info("Arming native widget window mode", { id: window.id, shrinkDelay, currentBounds: window.getBounds() });
    traceWidgetTaskbarStep("arm-widget-mode", { id: window.id, shrinkDelay, currentBounds: window.getBounds() });
    if (shrinkDelay <= 0) {
      applyNativeWidgetWindowBounds(window);
    } else {
      widgetWindowShrinkTimer = setTimeout(() => {
        widgetWindowShrinkTimer = null;
        applyNativeWidgetWindowBounds(window);
      }, shrinkDelay);
    }
    return true;
  }

  clearWidgetWindowShrinkTimer();
  clearWidgetTaskbarAutoHideTimer();
  widgetTaskbarAutoHideJobToken += 1;
  clearWidgetWindowBoundsAnimationTimer();
  resetNativeWidgetClickThrough(window);
  resetNativeWidgetWindowShape(window);
  const restoreState = widgetWindowRestoreState;
  widgetWindowRestoreState = null;
  window.setBackgroundColor(TRANSPARENT_WINDOW_BACKGROUND);
  if (!restoreState) {
    const fallbackBounds = normalWindowRestoreBounds(window);
    window.setMinimumSize(420, 128);
    window.setAlwaysOnTop(true, "floating");
    window.show();
    window.moveTop();
    window.focus();
    animateNativeWidgetWindowBounds(window, fallbackBounds, WIDGET_WINDOW_RESTORE_MS);
    setTimeout(() => finishNativeWidgetWindowRestore(window, null, fallbackBounds), WIDGET_WINDOW_RESTORE_MS + 40);
    void scheduleNativeWidgetTaskbarAutoHide(window, false);
    traceWidgetTaskbarStep("restore-widget-without-state", { id: window.id, fallbackBounds });
    return true;
  }
  const restoreStateBoundsLookLikeWidget = boundsLookLikeWidget(restoreState.bounds);
  const restoreBounds = restoreStateBoundsLookLikeWidget
    ? normalWindowRestoreBounds(window)
    : restoreState.bounds;
  window.setMinimumSize(420, 128);
  window.setAlwaysOnTop(true, "floating");
  window.show();
  window.moveTop();
  window.focus();
  animateNativeWidgetWindowBounds(window, restoreBounds, WIDGET_WINDOW_RESTORE_MS);
  setTimeout(() => finishNativeWidgetWindowRestore(window, restoreState, restoreBounds), WIDGET_WINDOW_RESTORE_MS + 40);
  void scheduleNativeWidgetTaskbarAutoHide(window, false);
  traceWidgetTaskbarStep("restore-widget-window-bounds", {
    id: window.id,
    requestedBounds: restoreState.bounds,
    restoreBounds,
    restoredFromWidgetBounds: restoreStateBoundsLookLikeWidget,
    maximized: restoreState.maximized,
    fullScreen: restoreState.fullScreen
  });
  return true;
}

/* Replicated widget tray clicks open the real Windows shell flyouts by
   synthesizing the system hotkeys: Win+A for Quick Settings (volume / Wi-Fi /
   battery) and Win+N for the calendar / notifications. Only these two fixed
   actions are allowed — no arbitrary key injection. */
const WIDGET_SHELL_ACTION_KEYS = {
  "quick-settings": 0x41, // A
  calendar: 0x4e // N
} as const;
type WidgetShellAction = keyof typeof WIDGET_SHELL_ACTION_KEYS;

function sendWindowsWinHotkey(letterVk: number): void {
  if (process.platform !== "win32") {
    return;
  }
  const script = [
    "Add-Type -Namespace InGenHotkey -Name Native -MemberDefinition '[DllImport(\"user32.dll\")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo);';",
    "$lwin = 0x5B;",
    `$vk = ${letterVk};`,
    "[InGenHotkey.Native]::keybd_event($lwin, 0, 0, [System.UIntPtr]::Zero);",
    "[InGenHotkey.Native]::keybd_event($vk, 0, 0, [System.UIntPtr]::Zero);",
    "Start-Sleep -Milliseconds 30;",
    "[InGenHotkey.Native]::keybd_event($vk, 0, 2, [System.UIntPtr]::Zero);",
    "[InGenHotkey.Native]::keybd_event($lwin, 0, 2, [System.UIntPtr]::Zero);"
  ].join(" ");
  try {
    const child = spawn(
      "powershell.exe",
      ["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script],
      { windowsHide: true, stdio: "ignore" }
    );
    child.on("error", (error) => console.warn("Failed to send Windows hotkey", error));
    child.unref();
  } catch (error) {
    console.warn("Failed to spawn Windows hotkey", error);
  }
}

function triggerWidgetShellAction(event: Electron.IpcMainInvokeEvent, action: unknown): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked widget shell action from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  if (typeof action !== "string" || !(action in WIDGET_SHELL_ACTION_KEYS)) {
    console.warn("Blocked unknown widget shell action", action);
    return false;
  }
  sendWindowsWinHotkey(WIDGET_SHELL_ACTION_KEYS[action as WidgetShellAction]);
  return true;
}

function installWindowControlIpc(): void {
  ipcMain.handle("forge:window-minimize", (event): boolean => minimizeNativeWindow(event));
  ipcMain.handle("forge:window-toggle-maximize", (event): boolean => toggleNativeWindowMaximize(event));
  ipcMain.handle("forge:window-close", (event): boolean => closeNativeWindow(event));
  ipcMain.handle("forge:window-widget-mode", (event, enabled, delayMs): boolean => setNativeWindowWidgetMode(event, enabled, delayMs));
  ipcMain.handle("forge:window-widget-hit-regions", (event, regions): boolean => setNativeWindowWidgetHitRegions(event, regions));
  ipcMain.handle("forge:window-widget-panel-expanded", (event, enabled): boolean => setNativeWindowWidgetPanelExpanded(event, enabled));
  ipcMain.handle("forge:window-widget-click-through", (event, enabled): boolean => setNativeWindowWidgetClickThrough(event, enabled));
  ipcMain.handle("forge:window-widget-taskbar-autohide", (event, enabled): boolean => setNativeWindowWidgetTaskbarAutoHide(event, enabled));
  ipcMain.handle("forge:window-widget-taskbar-toggle", (event): boolean => toggleNativeWindowWidgetTaskbar(event));
  ipcMain.handle("forge:window-widget-wallpaper-sample", (event, bounds): Promise<WidgetWallpaperSampleResult> =>
    sampleNativeWidgetWallpaper(event, bounds)
  );
  ipcMain.handle("forge:widget-shell-action", (event, action): boolean => triggerWidgetShellAction(event, action));
}

function terminalProof(value: unknown): string {
  return hashJson({ terminal: value, at: Date.now() });
}

function windowsCommandExists(command: string): boolean {
  const result = spawnSync("where.exe", [command], { encoding: "utf8", stdio: "pipe", timeout: 1000, windowsHide: true });
  return result.status === 0;
}

function nativeTerminalResult(accepted: boolean, error?: IpcError): NativeTerminalResult {
  const result: NativeTerminalResult = {
    accepted,
    path: activeWorkspaceDir,
    proofHash: "",
    error
  };
  result.proofHash = hashJson({ nativeTerminal: result, proofHash: "" });
  return result;
}

function normalizeNativeTerminalBounds(bounds: NativeTerminalBounds): NativeTerminalBounds | null {
  const x = Math.round(bounds.x);
  const y = Math.round(bounds.y);
  const width = Math.round(bounds.width);
  const height = Math.round(bounds.height);
  if (![x, y, width, height].every(Number.isFinite) || width < 120 || height < 80) {
    return null;
  }
  return { x, y, width, height };
}

function nativeWindowHandleDecimal(window: BrowserWindow): string {
  const handle = window.getNativeWindowHandle();
  if (handle.length >= 8) {
    return handle.readBigUInt64LE(0).toString();
  }
  return BigInt(handle.readUInt32LE(0)).toString();
}

function runWin32TerminalScript(script: string): string {
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
    { encoding: "utf8", stdio: "pipe", timeout: 5000, windowsHide: true }
  );
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || "Win32 terminal script failed.").trim());
  }
  return result.stdout.trim();
}

function waitForProcessMainWindow(pid: number): string {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    `$pidValue = ${pid}`,
    "$deadline = (Get-Date).AddSeconds(4)",
    "do {",
    "  $p = Get-Process -Id $pidValue -ErrorAction SilentlyContinue",
    "  if ($p) {",
    "    $p.Refresh()",
    "    if ($p.MainWindowHandle -and $p.MainWindowHandle.ToInt64() -ne 0) {",
    "      [Console]::Write($p.MainWindowHandle.ToInt64())",
    "      exit 0",
    "    }",
    "  }",
    "  Start-Sleep -Milliseconds 80",
    "} while ((Get-Date) -lt $deadline)",
    "throw 'PowerShell window handle was not available.'"
  ].join("; ");
  return runWin32TerminalScript(script);
}

function attachNativeTerminalWindow(owner: BrowserWindow, hwnd: string, bounds: NativeTerminalBounds): void {
  const parentHwnd = nativeWindowHandleDecimal(owner);
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "Add-Type -TypeDefinition @'",
    "using System;",
    "using System.Runtime.InteropServices;",
    "public static class InGenWin32TerminalHost {",
    "  [DllImport(\"user32.dll\")] public static extern IntPtr SetParent(IntPtr hWndChild, IntPtr hWndNewParent);",
    "  [DllImport(\"user32.dll\")] public static extern bool MoveWindow(IntPtr hWnd, int X, int Y, int nWidth, int nHeight, bool bRepaint);",
    "  [DllImport(\"user32.dll\")] public static extern int GetWindowLong(IntPtr hWnd, int nIndex);",
    "  [DllImport(\"user32.dll\")] public static extern int SetWindowLong(IntPtr hWnd, int nIndex, int dwNewLong);",
    "  [DllImport(\"user32.dll\")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);",
    "}",
    "'@",
    `$child = [IntPtr]::new([Int64]::Parse('${hwnd}'))`,
    `$parent = [IntPtr]::new([Int64]::Parse('${parentHwnd}'))`,
    "$GWL_STYLE = -16",
    "$WS_CHILD = 0x40000000",
    "$WS_VISIBLE = 0x10000000",
    "$WS_OVERLAPPEDWINDOW = 0x00CF0000",
    "$style = [InGenWin32TerminalHost]::GetWindowLong($child, $GWL_STYLE)",
    "$style = ($style -bor $WS_CHILD -bor $WS_VISIBLE) -band (-bnot $WS_OVERLAPPEDWINDOW)",
    "[void][InGenWin32TerminalHost]::SetParent($child, $parent)",
    "[void][InGenWin32TerminalHost]::SetWindowLong($child, $GWL_STYLE, $style)",
    "[void][InGenWin32TerminalHost]::MoveWindow($child, " +
      `${bounds.x}, ${bounds.y}, ${bounds.width}, ${bounds.height}, $true)`,
    "[void][InGenWin32TerminalHost]::ShowWindow($child, 5)"
  ].join("\n");
  runWin32TerminalScript(script);
}

function resolveTerminalRuntime(): TerminalRuntimeConfig {
  if (process.platform === "win32") {
    return {
      command: "powershell.exe",
      args: [],
      label: "Windows PowerShell",
      prompt: "PS",
      subtitle: "Embedded native Windows PowerShell.",
      cwd: activeWorkspaceDir
    };
  }

  if (process.platform === "darwin") {
    const command = process.env.SHELL || "/bin/zsh";
    const name = basename(command) || "zsh";
    return {
      command,
      args: [],
      label: `macOS ${name}`,
      prompt: "%",
      subtitle: "Native macOS terminal.",
      cwd: activeWorkspaceDir
    };
  }

  const command = process.env.TERMINAL || process.env.SHELL || "x-terminal-emulator";
  return {
    command,
    args: [],
    label: basename(command) || "Terminal",
    prompt: "$",
    subtitle: "Native Linux terminal.",
    cwd: activeWorkspaceDir
  };
}

function showNativeTerminal(event: Electron.IpcMainInvokeEvent, bounds: NativeTerminalBounds): NativeTerminalResult {
  if (!validateSender(event)) {
    return nativeTerminalResult(false, {
      code: "bad_sender",
      message: "Native terminal rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeTerminalBounds(bounds);
  if (!normalized) {
    return nativeTerminalResult(false, {
      code: "bad_payload",
      message: "Native terminal bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const owner = senderNativeWindow(event);
  if (!owner || owner.isDestroyed()) {
    return nativeTerminalResult(false, {
      code: "rust_unavailable",
      message: "Native terminal owner window is unavailable.",
      proofHash: hashJson({ bounds })
    });
  }

  terminalRuntime = resolveTerminalRuntime();
  try {
    if (!nativeTerminalProcess || nativeTerminalProcess.killed || nativeTerminalCwd !== activeWorkspaceDir || nativeTerminalHwnd === "") {
      const child = spawn(terminalRuntime.command, terminalRuntime.args, {
        cwd: terminalRuntime.cwd,
        env: process.env,
        detached: false,
        shell: false,
        stdio: "ignore",
        windowsHide: false
      });
      nativeTerminalProcess = child;
      nativeTerminalCwd = activeWorkspaceDir;
      nativeTerminalOwner = owner;
      nativeTerminalHwnd = waitForProcessMainWindow(child.pid ?? 0);
      child.once("exit", () => {
        if (nativeTerminalProcess === child) {
          nativeTerminalProcess = null;
          nativeTerminalCwd = "";
          nativeTerminalHwnd = "";
          nativeTerminalOwner = null;
        }
      });
    }
    nativeTerminalOwner = owner;
    attachNativeTerminalWindow(owner, nativeTerminalHwnd, normalized);
    return nativeTerminalResult(true);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return nativeTerminalResult(false, {
      code: "rust_unavailable",
      message,
      proofHash: terminalProof({ error: message })
    });
  }
}

function terminalStartResult(accepted: boolean, error?: IpcError): TerminalStartResult {
  const result: TerminalStartResult = {
    accepted,
    shell: terminalRuntime.label,
    cwd: terminalRuntime.cwd,
    prompt: terminalRuntime.prompt,
    subtitle: terminalRuntime.subtitle,
    proofHash: "",
    error
  };
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function syncTerminalRuntimeToWorkspace(): void {
  terminalRuntime = resolveTerminalRuntime();
  nativeTerminalCwd = "";
}

function updateNativeTerminalBounds(event: Electron.IpcMainInvokeEvent, bounds: NativeTerminalBounds): NativeTerminalResult {
  if (!nativeTerminalProcess || nativeTerminalHwnd === "") {
    return showNativeTerminal(event, bounds);
  }
  if (!validateSender(event)) {
    return nativeTerminalResult(false, {
      code: "bad_sender",
      message: "Native terminal bounds update rejected by sender validation.",
      proofHash: hashJson({ bounds, sender: event.senderFrame?.url ?? "" })
    });
  }
  const normalized = normalizeNativeTerminalBounds(bounds);
  if (!normalized) {
    return nativeTerminalResult(false, {
      code: "bad_payload",
      message: "Native terminal bounds are invalid.",
      proofHash: hashJson(bounds)
    });
  }
  const owner = senderNativeWindow(event) ?? nativeTerminalOwner;
  if (!owner || owner.isDestroyed()) {
    return nativeTerminalResult(false, {
      code: "rust_unavailable",
      message: "Native terminal owner window is unavailable.",
      proofHash: hashJson({ bounds })
    });
  }
  try {
    attachNativeTerminalWindow(owner, nativeTerminalHwnd, normalized);
    return nativeTerminalResult(true);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return nativeTerminalResult(false, {
      code: "rust_unavailable",
      message,
      proofHash: terminalProof({ error: message })
    });
  }
}

function hideNativeTerminal(): NativeTerminalResult {
  if (nativeTerminalProcess && !nativeTerminalProcess.killed) {
    nativeTerminalProcess.kill();
  }
  nativeTerminalProcess = null;
  nativeTerminalCwd = "";
  nativeTerminalHwnd = "";
  nativeTerminalOwner = null;
  return nativeTerminalResult(true);
}

function workspaceStorePath(): string {
  return join(app.getPath("userData"), "workspace.json");
}

async function workspaceDirIsUsable(dir: string): Promise<boolean> {
  try {
    return (await stat(dir)).isDirectory();
  } catch {
    return false;
  }
}

async function restoreWorkspaceDirFromDisk(): Promise<void> {
  try {
    const raw = await readFile(workspaceStorePath(), "utf8");
    const parsed = JSON.parse(raw) as { path?: unknown };
    if (typeof parsed.path === "string" && parsed.path.trim() !== "" && (await workspaceDirIsUsable(parsed.path))) {
      activeWorkspaceDir = parsed.path;
      workspaceExplicitlyChosen = true;
    }
  } catch {
    // No persisted workspace yet; keep the repo-root default.
  }
}

async function persistWorkspaceDir(): Promise<void> {
  try {
    await writeFile(workspaceStorePath(), JSON.stringify({ path: activeWorkspaceDir }, null, 2), "utf8");
  } catch (error) {
    console.error("Failed to persist workspace folder.", error);
  }
}

function workspaceChoiceResult(canceled: boolean): WorkspaceChoiceResult {
  return {
    canceled,
    path: activeWorkspaceDir,
    folderName: workspaceExplicitlyChosen ? basename(activeWorkspaceDir) : "",
    proofHash: hashJson({ workspace: activeWorkspaceDir, chosen: workspaceExplicitlyChosen, canceled })
  };
}

function workspaceActionResult(accepted: boolean, value: string, error?: IpcError): WorkspaceActionResult {
  const result: WorkspaceActionResult = {
    accepted,
    path: activeWorkspaceDir,
    value,
    proofHash: "",
    error
  };
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function workspaceActionRejected(action: string): WorkspaceActionResult {
  const proofHash = hashJson({ workspace: activeWorkspaceDir, action, reason: "bad_sender" });
  return workspaceActionResult(false, "", {
    code: "bad_sender",
    message: "Workspace action rejected by sender validation.",
    proofHash
  });
}

function readGitBranchName(dir: string): string {
  const branch = spawnSync("git", ["-C", dir, "branch", "--show-current"], {
    encoding: "utf8",
    stdio: "pipe",
    timeout: 1500,
    windowsHide: true
  }).stdout.trim();
  if (branch !== "") {
    return branch;
  }
  return spawnSync("git", ["-C", dir, "rev-parse", "--short", "HEAD"], {
    encoding: "utf8",
    stdio: "pipe",
    timeout: 1500,
    windowsHide: true
  }).stdout.trim();
}

function installTerminalIpc(): void {
  ipcMain.handle("forge:terminal-start", (event): TerminalStartResult => {
    if (!validateSender(event)) {
      return terminalStartResult(false, {
        code: "bad_sender",
        message: "Terminal start rejected by sender validation.",
        proofHash: terminalProof("bad_sender")
      });
    }
    return terminalStartResult(false, {
      code: "bad_payload",
      message: "Native terminal start requires pane bounds. Use forge:terminal-show-native.",
      proofHash: terminalProof({ reason: "missing_bounds" })
    });
  });

  ipcMain.handle("forge:terminal-write", (event, data: unknown): boolean => {
    if (!validateSender(event) || typeof data !== "string") {
      return false;
    }
    return false;
  });

  ipcMain.handle("forge:terminal-resize", (event, size: unknown): boolean => {
    if (!validateSender(event) || !size || typeof size !== "object") {
      return false;
    }
    return true;
  });

  ipcMain.handle("forge:terminal-stop", (event): boolean => {
    if (!validateSender(event)) {
      return false;
    }
    hideNativeTerminal();
    return true;
  });

  ipcMain.handle("forge:terminal-show-native", (event, bounds: NativeTerminalBounds): NativeTerminalResult => {
    return showNativeTerminal(event, bounds);
  });

  ipcMain.handle("forge:terminal-bounds-native", (event, bounds: NativeTerminalBounds): NativeTerminalResult => {
    return updateNativeTerminalBounds(event, bounds);
  });

  ipcMain.handle("forge:terminal-hide-native", (event): NativeTerminalResult => {
    if (!validateSender(event)) {
      return nativeTerminalResult(false, {
        code: "bad_sender",
        message: "Native terminal hide rejected by sender validation.",
        proofHash: hashJson(event.senderFrame?.url ?? "")
      });
    }
    return hideNativeTerminal();
  });

  ipcMain.handle("forge:get-workspace-folder", (event): WorkspaceChoiceResult => {
    if (!validateSender(event)) {
      const proofHash = hashJson({ workspace: "get", reason: "bad_sender" });
      return {
        canceled: true,
        path: activeWorkspaceDir,
        folderName: "",
        proofHash,
        error: { code: "bad_sender", message: "Workspace read rejected by sender validation.", proofHash }
      };
    }
    return workspaceChoiceResult(false);
  });

  ipcMain.handle("forge:get-agent-action-host-manifest", (event): AgentActionHostManifest => {
    if (!validateSender(event)) {
      const manifest = createAgentActionHostManifest(agentActionHostConfig());
      return {
        ...manifest,
        proofHash: hashJson({ manifest: "agent_action_host", reason: "bad_sender", sender: event.senderFrame?.url ?? "" })
      };
    }
    return createAgentActionHostManifest(agentActionHostConfig());
  });

  ipcMain.handle("forge:execute-agent-action", async (event, request: unknown): Promise<AgentActionResult> => {
    const fallbackRequest: AgentActionRequest = { action: "list", path: "." };
    if (!validateSender(event) || !isAgentActionRequest(request)) {
      const proofHash = hashJson({ request, reason: "bad_sender_or_payload", sender: event.senderFrame?.url ?? "" });
      return {
        schema: "ingen.agent_action_host.result.v1",
        accepted: false,
        action: isAgentActionRequest(request) ? request.action : fallbackRequest.action,
        cwd: activeWorkspaceDir,
        proofHash,
        error: {
          code: !validateSender(event) ? "bad_sender" : "bad_payload",
          message: "Agent action rejected by IPC validation.",
          proofHash
        }
      };
    }
    return executeTrackedAgentAction(request);
  });

  ipcMain.handle("forge:choose-workspace-folder", async (event): Promise<WorkspaceChoiceResult> => {
    if (!validateSender(event)) {
      const proofHash = hashJson({ workspace: "choose", reason: "bad_sender" });
      return {
        canceled: true,
        path: activeWorkspaceDir,
        folderName: "",
        proofHash,
        error: { code: "bad_sender", message: "Workspace picker rejected by sender validation.", proofHash }
      };
    }

    const parent = senderNativeWindow(event);
    const selection = await dialog.showOpenDialog(parent ?? primaryWindow ?? BrowserWindow.getAllWindows()[0], {
      title: "Choose workspace folder",
      defaultPath: activeWorkspaceDir,
      properties: ["openDirectory", "createDirectory"]
    });

    if (selection.canceled || selection.filePaths.length === 0) {
      return workspaceChoiceResult(true);
    }

    activeWorkspaceDir = selection.filePaths[0];
    workspaceExplicitlyChosen = true;
    await persistWorkspaceDir();
    syncTerminalRuntimeToWorkspace();

    return workspaceChoiceResult(false);
  });

  ipcMain.handle("forge:show-workspace-in-explorer", async (event): Promise<WorkspaceActionResult> => {
    if (!validateSender(event)) {
      return workspaceActionRejected("show_workspace");
    }
    const errorMessage = await shell.openPath(activeWorkspaceDir);
    if (errorMessage) {
      return workspaceActionResult(false, "", {
        code: "rust_unavailable",
        message: errorMessage,
        proofHash: hashJson({ workspace: activeWorkspaceDir, action: "show_workspace", error: errorMessage })
      });
    }
    return workspaceActionResult(true, activeWorkspaceDir);
  });

  ipcMain.handle("forge:copy-workspace-path", (event): WorkspaceActionResult => {
    if (!validateSender(event)) {
      return workspaceActionRejected("copy_workspace_path");
    }
    clipboard.writeText(activeWorkspaceDir);
    return workspaceActionResult(true, activeWorkspaceDir);
  });

  ipcMain.handle("forge:copy-workspace-branch-name", (event): WorkspaceActionResult => {
    if (!validateSender(event)) {
      return workspaceActionRejected("copy_workspace_branch");
    }
    const branchName = readGitBranchName(activeWorkspaceDir) || readGitBranchName(repoRoot);
    if (branchName === "") {
      return workspaceActionResult(false, "", {
        code: "rust_unavailable",
        message: "No Git branch or commit could be detected for this workspace.",
        proofHash: hashJson({ workspace: activeWorkspaceDir, action: "copy_workspace_branch", branch: "" })
      });
    }
    clipboard.writeText(branchName);
    return workspaceActionResult(true, branchName);
  });
}

function headerSnapshot(): HeaderSnapshot {
  const backend = rustBackend();
  const mode = cutoverMode();
  const controls = {
    top: [
      ["left-panel", "Toggle left panel", "panel-left", "toggle_left_panel", undefined],
      ["sessions", "Search sessions", "search", "open_sessions_canvas", "sessions"],
      ["webexplorer-top", "Open WebExplorer", "globe", "open_webexplorer", "webexplorer"],
      ["banger-top", "Open Banger", "box", "open_banger", "banger"],
      ["trading-top", "Open Trading", "chart", "open_trading", "trading"],
      ["window-minimize", "Minimize", "minus", "window_minimize", undefined],
      ["window-maximize", "Maximize", "square", "window_toggle_maximize", undefined],
      ["window-close", "Close", "x", "window_close", undefined]
    ],
    workspace: [
      ["plan", "Plan", "plan", "toggle_right_panel", "right-panel"],
      ["webexplorer-workspace", "WebExplorer workspace", "globe", "navigate_workspace", "webexplorer"],
      ["right-panel", "Split canvas", "panel-right", "toggle_right_panel", "right-panel"]
    ]
  } as const;
  const snapshot: HeaderSnapshot = {
    schema: "ingen.electron.header.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection || backend.activeSection,
    sectionTitle: headerState.sectionTitle || backend.sectionTitle,
    profileCanvas: headerState.profileCanvas,
    leftPanelOpen: headerState.leftPanelOpen,
    rightPanelOpen: headerState.rightPanelOpen,
    macChrome: process.platform === "darwin",
    cpuLabel: hardwareProfile.cpuLabel,
    gpuLabel: hardwareProfile.gpuLabel,
    topControls: controls.top.map(([id, label, icon, command, route]) => ({
      id,
      label,
      icon,
      command,
      route,
      selected:
        route === headerState.activeSection ||
        (id === "sessions" && headerState.profileCanvas === "sessions") ||
        (id === "left-panel" && headerState.leftPanelOpen),
      visible: true,
      nativeAuthority: command.startsWith("window_") ? "window" : mode === "electron" ? "rust" : "electron-shadow"
    })),
    workspaceControls: controls.workspace.map(([id, label, icon, command, route]) => ({
      id,
      label,
      icon,
      command,
      route,
      selected: route === headerState.activeSection || (id === "right-panel" && headerState.rightPanelOpen),
      visible: true,
      nativeAuthority: mode === "electron" ? "rust" : "electron-shadow"
    })),
    nativeSurfaceContracts: {
      banger: "native-child-surface",
      webexplorer: "rust-owned-webview"
    },
    proofHash: backend.proofHash
  };
  snapshot.proofHash = hashJson({ ...snapshot, backendProofHash: backend.proofHash, proofHash: "" });
  return snapshot;
}

function surfaceSlot(): HeaderSurfaceContract["slot"] {
  const left = headerState.leftPanelOpen ? 287 : 0;
  return {
    x: left,
    y: 96,
    width: 1535 - left,
    height: 786 - 96
  };
}

function surfaceContract(
  input: Omit<HeaderSurfaceContract, "slot" | "proofHash">
): HeaderSurfaceContract {
  const contract: HeaderSurfaceContract = {
    ...input,
    slot: surfaceSlot(),
    proofHash: ""
  };
  contract.proofHash = hashJson({ ...contract, proofHash: "" });
  return contract;
}

function headerSurfaceSnapshot(): HeaderSurfaceSnapshot {
  const mode = cutoverMode();
  let surfaces: HeaderSurfaceContract[];
  if (headerState.profileCanvas === "sessions") {
    surfaces = [
      surfaceContract({
        id: "sessions-canvas-delegated",
        kind: "delegated",
        label: "Sessions canvas",
        route: "sessions",
        authority: "rust",
        status: "delegated_to_parallel_slice",
        nativeContract: "parallel-session-2-sidebar-sessions",
        sourceComponent: "SessionsCanvas",
        summary: "Header opens this surface, but sidebar/session migration owns implementation."
      })
    ];
  } else {
    switch (headerState.activeSection) {
      case "webexplorer":
        surfaces = [
          surfaceContract({
            id: "webexplorer-webview-host",
            kind: "webexplorer_webview",
            label: "Rust WebView host",
            route: "webexplorer",
            authority: mode === "electron" ? "rust" : "electron-shadow",
            status: "native_ready",
            nativeContract: "rust-owned-webview-policy-host",
            sourceComponent: "GoogleWebViewCanvas",
            summary: "Electron frames the slot; Rust WebExplorer owns navigation, capture, DOM/AX atlas and policy."
          }),
          surfaceContract({
            id: "webexplorer-atlas",
            kind: "webexplorer_atlas",
            label: "RAM DOM Atlas",
            route: "webexplorer",
            authority: mode === "electron" ? "rust" : "electron-shadow",
            status: "native_ready",
            nativeContract: "atlas-metadata-proof-stream",
            sourceComponent: "AtlasInspector",
            summary: "Metadata-only atlas projection for proof hashes, selected node and blind-spot accounting."
          })
        ];
        break;
      case "banger":
        surfaces = [
          surfaceContract({
            id: "banger-native-child-surface",
            kind: "banger_native_child",
            label: "Banger native viewport",
            route: "banger",
            authority: mode === "electron" ? "rust" : "electron-shadow",
            status: "native_ready",
            nativeContract: "wgpu-child-window-frame-hash",
            sourceComponent: "BangerNativeViewport",
            summary: "Electron reserves the slot only; Banger/wgpu owns rendering, residency and frame proof."
          })
        ];
        break;
      case "trading":
      case "real-estate":
      case "alpha":
        surfaces = [
          surfaceContract({
            id: `${headerState.activeSection}-product-section`,
            kind: "product_section",
            label: "Product section surface",
            route: headerState.activeSection,
            authority: mode === "electron" ? "rust" : "electron-shadow",
            status: "native_ready",
            nativeContract: "domain-service-proof-projection",
            sourceComponent: "ProductSectionSurface",
            summary: "Domain product shell projection backed by Rust service proofs and action contracts."
          })
        ];
        break;
      default:
        surfaces = [
          surfaceContract({
            id: "forge-drop-canvas",
            kind: "drop_canvas",
            label: "Forge drop canvas",
            route: "forge",
            authority: "rust",
            status: "native_ready",
            nativeContract: "rust-forge-canvas-projection",
            sourceComponent: "DropCanvas",
            summary: "Default Forge canvas is served by the Rust backend projection."
          })
        ];
        break;
    }
  }

  const snapshot: HeaderSurfaceSnapshot = {
    schema: "ingen.electron.header.surface_snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection,
    profileCanvas: headerState.profileCanvas,
    surfaces,
    proofHash: ""
  };
  snapshot.proofHash = hashJson({ ...snapshot, proofHash: "" });
  return snapshot;
}

function sidebarAuthority(mode: FrontSliceMode): "rust" | "electron-shadow" {
  return mode === "electron" ? "rust" : "electron-shadow";
}

function sidebarToolControls(mode: FrontSliceMode): SidebarToolControl[] {
  const hidden = new Set(sidebarState.hiddenTools);
  return [
    { id: "new-session", label: "New Session", icon: "inline_svg_0008.svg", drawer: "", visible: true, hidden: false, selected: headerState.activeSection === "forge" && sidebarState.activeDrawer === "", nativeAuthority: sidebarAuthority(mode) },
    { id: "pool", label: "Pool", icon: "inline_svg_0018.svg", drawer: "pool", visible: true, hidden: false, selected: sidebarState.activeDrawer === "pool", nativeAuthority: sidebarAuthority(mode) },
    { id: "modules", label: "Modules", icon: "inline_svg_0019.svg", drawer: "modules", visible: true, hidden: false, selected: sidebarState.activeDrawer === "modules", nativeAuthority: sidebarAuthority(mode) },
    { id: "assets", label: "My Assets", icon: "inline_svg_0005.svg", drawer: "assets", visible: !hidden.has("assets"), hidden: hidden.has("assets"), selected: sidebarState.activeDrawer === "assets", nativeAuthority: sidebarAuthority(mode) },
    { id: "automations", label: "Automations", icon: "inline_svg_0021.svg", drawer: "", visible: !hidden.has("automations"), hidden: hidden.has("automations"), selected: false, nativeAuthority: sidebarAuthority(mode) },
    { id: "brain", label: "Brain", icon: "inline_svg_0020.svg", drawer: "", visible: !hidden.has("brain"), hidden: hidden.has("brain"), selected: headerState.profileCanvas === "brain", nativeAuthority: sidebarAuthority(mode) }
  ];
}

function sessionWorkspaceLabel(section: SidebarSessionItem["section"]): string {
  if (section === "webexplorer") return "WebExplorer";
  if (section === "banger") return "Banger";
  if (section === "trading") return "Forge Trading";
  if (section === "real-estate") return "Forge Immo";
  if (section === "alpha") return "Alpha";
  if (section === "shell") return "Shell";
  return "Forge";
}

function activeSessionWorkspaceLabel(section: SidebarSessionItem["section"]): string {
  const chosenWorkspace = workspaceExplicitlyChosen ? basename(activeWorkspaceDir).trim() : "";
  return chosenWorkspace || sessionWorkspaceLabel(section);
}

function todayIsoDate(): string {
  return new Date().toISOString().slice(0, 10);
}

const PENDING_LLM_SESSION_TITLE = "New session";
const RENAME_SESSION_TAG_PATTERN = /^\/rename_session_(\*?[^_\r\n]{1,120}\*?)_/i;
const RENAME_SESSION_TAG_FRAGMENT_PATTERN = /\/rename_session_(?:\*?[^_\r\n]{1,120}\*?)_/giu;

function normalizeSessionTitle(value: string): string {
  const compact = value
    .replace(/\s+/g, " ")
    .replace(/^["'`]+|["'`]+$/g, "")
    .trim();
  if (!compact) return "";
  return compact.length <= 42 ? compact : `${compact.slice(0, 39).trimEnd()}...`;
}

function stripSessionTitleNoise(value: string): string {
  let compact = value
    .replace(/\s+/g, " ")
    .replace(/^["'`]+|["'`]+$/g, "")
    .replace(/^sujet\s+(?:identifi[eé])\s*:?\s*/i, "")
    .replace(/^title\s*=\s*/i, "")
    .trim();
  const firstQuote = compact.search(/["'`]/);
  if (firstQuote > 0) {
    const beforeQuote = compact.slice(0, firstQuote).trim();
    if (beforeQuote.split(/\s+/).filter(Boolean).length <= 6) {
      compact = beforeQuote;
    }
  }
  compact = compact
    .replace(/\b(?:je\s+renomme|j['’]utilise|renommage|session|rename_session)[\s\S]*$/iu, "")
    .replace(/\b(?:voici|quelques\s+reperes|quelques\s+repères|pour\s+repondre|pour\s+répondre)\b[\s\S]*$/iu, "")
    .trim();
  return normalizeSessionTitle(compact);
}

function polishedSessionTitle(title: string, reason: string): string {
  const compact = stripSessionTitleNoise(title);
  if (!compact) {
    return "";
  }
  const copiedHistory = compact.match(/^je\s+veux\s+connaitre\s+l['’]histoire\s+de\s+(.+)$/i);
  if (copiedHistory?.[1]) {
    return normalizeSessionTitle(`History of ${copiedHistory[1]}`);
  }
  const copiedTalk = compact.match(/^(?:parle|parles|parlez)\s+moi\s+de\s+(.+)$/i);
  if (copiedTalk?.[1]) {
    return normalizeSessionTitle(copiedTalk[1]);
  }
  const words = compact.split(/\s+/).filter(Boolean);
  const hasTitleShape = /\b(de|du|des|sur|pour|avec|dans|analyse|histoire|decouverte|recherche|climat|creation|debug|refonte|plan)\b/i.test(compact);
  if (words.length <= 2 && !hasTitleShape) {
    if (/histoire|histor/i.test(reason)) {
      return normalizeSessionTitle(`History of ${compact}`);
    }
    if (/climat|temperature|meteo|saison/i.test(reason)) {
      return normalizeSessionTitle(`Climate of ${compact}`);
    }
    if (/voyage|vacance|sejour|airbnb|logement|hotel|location/i.test(reason)) {
      return normalizeSessionTitle(`Travel to ${compact}`);
    }
    return normalizeSessionTitle(compact);
  }
  return compact;
}

interface RenameSessionCodeActRequest {
  schema: "forge.brain.rename_session.request.v1";
  command: typeof BRAIN_RENAME_SESSION_COMMAND;
  title: string;
  reason: string;
  proofHash: string;
}

function parseRenameSessionCodeActLine(line: string): RenameSessionCodeActRequest | undefined {
  const trimmed = line.trim();
  const tagMatch = trimmed.match(RENAME_SESSION_TAG_PATTERN);
  if (!tagMatch?.[1]) {
    return undefined;
  }
  const rawTitle = tagMatch[1].replace(/^["'`*]+|["'`*]+$/g, "").replace(/_/g, " ");
  if (!rawTitle || /[<>]/.test(rawTitle)) {
    return undefined;
  }
  const title = polishedSessionTitle(rawTitle, "brain_rename_session_tag");
  if (!title) {
    return undefined;
  }
  const request: RenameSessionCodeActRequest = {
    schema: "forge.brain.rename_session.request.v1",
    command: BRAIN_RENAME_SESSION_COMMAND,
    title,
    reason: "brain_rename_session_tag",
    proofHash: ""
  };
  request.proofHash = hashJson({ ...request, proofHash: "" });
  return request;
}

function stripRenameSessionCodeActFragments(line: string): string {
  const trimmed = line.trim();
  if (trimmed.startsWith(BRAIN_RENAME_SESSION_COMMAND) && !RENAME_SESSION_TAG_PATTERN.test(trimmed)) {
    return "";
  }
  return line
    .replace(RENAME_SESSION_TAG_FRAGMENT_PATTERN, "")
    .replace(/^[ \t]+/g, "")
    .replace(/[ \t]{2,}/g, " ")
    .trimEnd();
}

function extractRenameSessionCodeAct(text: string): RenameSessionCodeActRequest | undefined {
  return text
    .split(/\r?\n/)
    .map((line) => parseRenameSessionCodeActLine(line))
    .find((request): request is RenameSessionCodeActRequest => Boolean(request));
}

function removeRenameSessionCodeActLines(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) => stripRenameSessionCodeActFragments(line))
    .filter((line) => line.trim().length > 0)
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function generateChatSessionId(): string {
  return `chat-${Date.now().toString(36)}-${randomBytes(5).toString("hex")}`;
}

const BRAIN_BOOT_MESSAGE_ID_PREFIX = "system-brain-boot";

function brainBootSystemMessage(sessionId: string): TranscriptMessage {
  const text = brainBootManifest();
  return {
    id: `${BRAIN_BOOT_MESSAGE_ID_PREFIX}-${sessionId || "draft"}`,
    role: "system",
    text,
    attachments: [],
    proofHash: hashJson({ sessionId, text })
  };
}

function ensureBrainBootTranscript(sessionId: string): void {
  const existingIndex = panelsChatBottomState.transcript.findIndex(
    (message) => message.role === "system" && message.id.startsWith(BRAIN_BOOT_MESSAGE_ID_PREFIX)
  );
  const message = brainBootSystemMessage(sessionId);
  if (existingIndex >= 0) {
    panelsChatBottomState.transcript[existingIndex] = message;
    return;
  }
  panelsChatBottomState.transcript = [message, ...panelsChatBottomState.transcript];
}

function ensureActiveChatSession(_draft: string, options: { markWorking?: boolean } = {}): SidebarSessionItem {
  const markWorking = options.markWorking ?? true;
  const existing = localChatSessions.find((session) => session.sessionId === panelsChatBottomState.activeSessionId);
  if (existing) {
    if (markWorking) {
      existing.working = true;
      existing.date = todayIsoDate();
    }
    ensureBrainBootTranscript(existing.sessionId);
    return existing;
  }
  const section = headerState.activeSection === "shell" ? "forge" : headerState.activeSection;
  if (panelsChatBottomState.activeSessionId) {
    const materialized = materializeOpenedChatSession(panelsChatBottomState.activeSessionId, section);
    if (markWorking) {
      materialized.working = true;
      materialized.date = todayIsoDate();
    }
    ensureBrainBootTranscript(materialized.sessionId);
    sidebarState.recentSessionId = materialized.sessionId;
    return materialized;
  }
  const session: SidebarSessionItem = {
    sessionId: generateChatSessionId(),
    label: PENDING_LLM_SESSION_TITLE,
    date: todayIsoDate(),
    section,
    workspaceLabel: activeSessionWorkspaceLabel(section),
    rowVisible: true,
    pinned: false,
    working: markWorking,
    automated: false,
    archived: false
  };
  panelsChatBottomState.activeSessionId = session.sessionId;
  ensureBrainBootTranscript(session.sessionId);
  sidebarState.recentSessionId = session.sessionId;
  localChatSessions.unshift(session);
  return session;
}

function cleanParallelLaneLabel(label: string): string {
  const cleaned = label
    .replace(/^Par{1,2}al{1,2}el\s*\(\d+\)\s*/i, "")
    .replace(/^Par{1,2}al{1,2}el\s+\d+\s*:\s*/i, "")
    .replace(/(^|\s+\/\s+)\d+\s*:\s*/g, "$1")
    .trim();
  return cleaned || PENDING_LLM_SESSION_TITLE;
}

function sessionById(sessionId: string): SidebarSessionItem | undefined {
  return localChatSessions.find((session) => session.sessionId === sessionId);
}

function sidebarSessionCommandMatches(item: SidebarSessionItem, sessionId: string): boolean {
  return item.sessionId === sessionId || item.parallelGroupId === sessionId || (!item.sessionId && item.label === sessionId);
}

function parallelGroupItems(groupId: string): SidebarSessionItem[] {
  return localChatSessions
    .filter((session) => session.parallelGroupId === groupId)
    .sort((left, right) => (left.parallelLaneIndex ?? 0) - (right.parallelLaneIndex ?? 0));
}

function updateParallelGroupMetadata(groupId: string): void {
  const items = parallelGroupItems(groupId);
  const ids = items.map((item) => item.sessionId);
  const count = Math.max(items.length, 2);
  for (const item of items) {
    item.parallelLaneCount = count;
    item.parallelPeerSessionIds = ids.filter((id) => id !== item.sessionId);
  }
}

function ensureParallelChatLane(index: number, _draft: string): { sessionId: string; transcript: TranscriptMessage[]; groupId: string } {
  const existing = parallelChatLanes.get(index);
  if (existing) {
    sidebarState.recentSessionId = existing.groupId;
    return existing;
  }
  const primarySession = ensureActiveChatSession("Parallel 1: New session", { markWorking: index === 0 });
  const groupId = primarySession.parallelGroupId || `parallel-${primarySession.sessionId}`;
  primarySession.parallelGroupId = groupId;
  primarySession.parallelLaneIndex = 0;
  primarySession.label = `Parallel 1: ${cleanParallelLaneLabel(primarySession.label)}`;

  const primaryLane = parallelChatLanes.get(0);
  if (!primaryLane) {
    parallelChatLanes.set(0, {
      sessionId: primarySession.sessionId,
      transcript: panelsChatBottomState.transcript,
      groupId
    });
  }

  const section = headerState.activeSection === "shell" ? "forge" : headerState.activeSection;
  const session: SidebarSessionItem = {
    sessionId: generateChatSessionId(),
    label: `Parallel ${index + 1}: ${PENDING_LLM_SESSION_TITLE}`,
    date: todayIsoDate(),
    section,
    workspaceLabel: activeSessionWorkspaceLabel(section),
    rowVisible: true,
    pinned: false,
    working: false,
    automated: false,
    archived: false,
    parallelGroupId: groupId,
    parallelLaneIndex: index,
    parallelLaneCount: Math.max(index + 1, 2),
    parallelPeerSessionIds: [primarySession.sessionId]
  };
  localChatSessions.unshift(session);
  const lane = {
    sessionId: session.sessionId,
    transcript: [brainBootSystemMessage(session.sessionId)],
    groupId
  };
  parallelChatLanes.set(index, lane);
  updateParallelGroupMetadata(groupId);
  sidebarState.recentSessionId = groupId;
  return lane;
}

function isInternalTranscriptMessage(message: { turnId?: string; id?: string }): boolean {
  return (message.id ?? message.turnId ?? "").startsWith("internal-user-");
}

function publicTranscript(messages: TranscriptMessage[]): TranscriptMessage[] {
  return messages.filter((message) =>
    (message.role === "user" || message.role === "assistant") &&
    !isInternalTranscriptMessage(message)
  );
}

function parallelChatLaneSessionTitle(sessionId: string, explicitTitle = ""): string {
  if (explicitTitle.trim()) {
    return explicitTitle.trim();
  }
  const item = sessionById(sessionId);
  const archiveSession = chatArchiveSessions.get(sessionId);
  return item?.label || archiveSession?.title || "Parallel session";
}

function parallelChatLaneSnapshots(): PanelsChatBottomSnapshot["parallelLanes"] {
  return Array.from(parallelChatLanes.entries())
    .sort(([left], [right]) => left - right)
    .map(([index, lane]) => {
      const sessionTitle = parallelChatLaneSessionTitle(lane.sessionId, lane.sessionTitle);
      const transcript = publicTranscript(lane.transcript);
      return {
        index,
        sessionId: lane.sessionId,
        sessionTitle,
        transcript,
        focusMessageId: lane.focusMessageId,
        proofHash: hashJson({
          index,
          sessionId: lane.sessionId,
          sessionTitle,
          transcript,
          focusMessageId: lane.focusMessageId ?? ""
        })
      };
    });
}

function resetPanelsChatSessionView(): void {
  panelsChatBottomState.chatText = "";
  panelsChatBottomState.transcript = [];
  panelsChatBottomState.uploadItems = [];
  panelsChatBottomState.uploadCount = 0;
  panelsChatBottomState.uploadErrorText = "";
  panelsChatBottomState.uploadEditTargetId = "";
  composerUploadPreviewItems.clear();
  providerAttachmentCache.clear();
  parallelChatLanes.clear();
  panelsChatBottomState.permissionModeOpen = false;
  panelsChatBottomState.activeBrainSegment = "general";
  panelsChatBottomState.activeSessionId = "";
}

function clearPanelsChatSessionForId(sessionId: string): void {
  panelsChatBottomState.chatText = "";
  panelsChatBottomState.transcript = [];
  panelsChatBottomState.uploadItems = [];
  panelsChatBottomState.uploadCount = 0;
  panelsChatBottomState.uploadErrorText = "";
  panelsChatBottomState.uploadEditTargetId = "";
  composerUploadPreviewItems.clear();
  providerAttachmentCache.clear();
  parallelChatLanes.clear();
  panelsChatBottomState.permissionModeOpen = false;
  panelsChatBottomState.activeBrainSegment = "general";
  panelsChatBottomState.activeSessionId = sessionId;
  ensureBrainBootTranscript(sessionId);
}

function markAssistantWriteComplete(messageId: string): void {
  if (!messageId) return;
  const activeMessage = panelsChatBottomState.transcript.find((message) => message.id === messageId);
  let sessionId = activeMessage?.role === "assistant" ? panelsChatBottomState.activeSessionId : "";

  if (!sessionId) {
    for (const archiveSession of chatArchiveSessions.values()) {
      const archiveMessage = archiveSession.messages.find((message) => message.turnId === messageId && message.role === "assistant");
      if (archiveMessage) {
        sessionId = archiveSession.sessionId;
        break;
      }
    }
  }

  if (!sessionId) return;
  const session = localChatSessions.find((item) => item.sessionId === sessionId);
  if (!session) return;

  const archiveSession = chatArchiveSessions.get(sessionId);
  const latestArchiveAssistantId = archiveSession?.messages
    .filter((message) => message.role === "assistant" && message.text.trim())
    .at(-1)?.turnId;
  const latestActiveAssistantId = sessionId === panelsChatBottomState.activeSessionId
    ? panelsChatBottomState.transcript
      .filter((message) => message.role === "assistant" && message.text.trim())
      .at(-1)?.id
    : undefined;
  const latestAssistantId = latestArchiveAssistantId ?? latestActiveAssistantId;
  if (latestAssistantId && latestAssistantId !== messageId) return;
  if (sessionId === panelsChatBottomState.activeSessionId && transcriptHasOpenQuestionnaire(panelsChatBottomState.transcript)) return;

  session.working = false;
}

function appendTranscriptMessageForActiveSession(sessionId: string, message: TranscriptMessage): void {
  if (panelsChatBottomState.activeSessionId !== sessionId) {
    return;
  }
  if (panelsChatBottomState.transcript.some((existing) => existing.id === message.id)) {
    return;
  }
  panelsChatBottomState.transcript = [
    ...panelsChatBottomState.transcript,
    message
  ];
}

function transcriptWithMessage(messages: TranscriptMessage[], message: TranscriptMessage): TranscriptMessage[] {
  if (messages.some((existing) => existing.id === message.id)) {
    return messages;
  }
  return [...messages, message];
}

function transcriptWithReplacedMessage(messages: TranscriptMessage[], message: TranscriptMessage): TranscriptMessage[] {
  const existingIndex = messages.findIndex((existing) => existing.id === message.id);
  if (existingIndex === -1) {
    return [...messages, message];
  }
  const nextMessages = [...messages];
  nextMessages[existingIndex] = message;
  return nextMessages;
}

function transcriptWithoutMessage(messages: TranscriptMessage[], messageId: string): TranscriptMessage[] {
  return messageId ? messages.filter((message) => message.id !== messageId) : messages;
}

function contextCompactionEventMessage(state: "compressing" | "compressed", seed: string): TranscriptMessage {
  const id = `assistant-status-context-compaction-${seed}`;
  return {
    id,
    role: "assistant",
    text: `${CONTEXT_COMPACTION_EVENT_COMMAND} state="${state}"`,
    proofHash: hashJson({ id, state, command: CONTEXT_COMPACTION_EVENT_COMMAND })
  };
}

const ASSISTANT_PROGRESSIVE_SEED_MIN_CHARS = 1200;
const ASSISTANT_PROGRESSIVE_SEED_TARGET_CHARS = 360;
const ASSISTANT_PROGRESSIVE_SEED_DELAY_MS = 180;

function assistantProgressiveSeedText(text: string): string {
  if (text.length <= ASSISTANT_PROGRESSIVE_SEED_TARGET_CHARS) {
    return text;
  }
  const softLimit = Math.min(text.length, ASSISTANT_PROGRESSIVE_SEED_TARGET_CHARS);
  const hardLimit = Math.min(text.length, ASSISTANT_PROGRESSIVE_SEED_TARGET_CHARS + 180);
  const forwardBreak = text.slice(softLimit, hardLimit).search(/(\n\n|\n|[.!?]\s|\s)/);
  if (forwardBreak >= 0) {
    return text.slice(0, softLimit + forwardBreak + 1);
  }
  const backwardBreak = text.slice(0, softLimit).search(/(\n\n|\n|[.!?]\s|\s)(?![\s\S]*(\n\n|\n|[.!?]\s|\s))/);
  if (backwardBreak >= 0 && backwardBreak > ASSISTANT_PROGRESSIVE_SEED_TARGET_CHARS / 2) {
    return text.slice(0, backwardBreak + 1);
  }
  return text.slice(0, softLimit);
}

function delayAssistantProgressiveSeed(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}

function assistantTextContainsAgentActionEvents(text: string): boolean {
  return /\/agent_(?:list|search|create_directory|rename_path|move_path|copy_path|delete_empty_directory|delete_tree|readonly_shell|shell)_/.test(text);
}

async function commitAssistantMessageWithProgressiveSeed(
  baseTranscript: TranscriptMessage[],
  assistantMessage: TranscriptMessage,
  sessionId: string,
  commitTranscript: (transcript: TranscriptMessage[]) => void
): Promise<TranscriptMessage[]> {
  const text = assistantMessage.text;
  if (text.length < ASSISTANT_PROGRESSIVE_SEED_MIN_CHARS || assistantTextContainsAgentActionEvents(text)) {
    const finalTranscript = transcriptWithMessage(baseTranscript, assistantMessage);
    commitTranscript(finalTranscript);
    return finalTranscript;
  }
  const seedText = assistantProgressiveSeedText(text);
  if (!seedText.trim() || seedText.length >= text.length) {
    const finalTranscript = transcriptWithMessage(baseTranscript, assistantMessage);
    commitTranscript(finalTranscript);
    return finalTranscript;
  }
  const seedMessage: TranscriptMessage = {
    ...assistantMessage,
    text: seedText,
    proofHash: hashJson({ progressiveSeedFor: assistantMessage.id, fullProofHash: assistantMessage.proofHash, text: seedText })
  };
  const seedTranscript = transcriptWithMessage(baseTranscript, seedMessage);
  commitTranscript(seedTranscript);
  emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed", sessionId);
  await delayAssistantProgressiveSeed(ASSISTANT_PROGRESSIVE_SEED_DELAY_MS);
  const finalTranscript = transcriptWithReplacedMessage(seedTranscript, assistantMessage);
  commitTranscript(finalTranscript);
  return finalTranscript;
}

function createAssistantLiveTextSink(params: {
  baseTranscript: TranscriptMessage[];
  assistantMessageId: string;
  agentEvents: AgentRuntimeEventQueue;
  commitTranscript: (transcript: TranscriptMessage[]) => void;
  prefixText?: string;
  assistantRun?: AssistantRunControl;
  renameSession?: SidebarSessionItem;
}): ProviderLiveTextSink {
  let lastText = "";
  let liveRenameApplied = false;
  return {
    onText: (text) => {
      if (params.renameSession && !liveRenameApplied) {
        const request = extractRenameSessionCodeAct(text);
        if (request) {
          renameChatSession(params.renameSession, request);
          liveRenameApplied = true;
          emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed", params.agentEvents.sessionId);
        }
      }
      const visibleText = removeRenameSessionChatter(removeLooseRenameSessionChatter(removeRenameSessionCodeActLines(agentActionLiveVisibleText(text)))).trimEnd();
      const trimmed = [params.prefixText?.trimEnd() ?? "", visibleText]
        .filter((part) => part.length > 0)
        .join("\n\n");
      if (!trimmed || trimmed === lastText) {
        return;
      }
      const previousText = lastText;
      lastText = trimmed;
      const textDelta = previousText && trimmed.startsWith(previousText)
        ? trimmed.slice(previousText.length)
        : trimmed;
      params.agentEvents.emit({
        kind: "text_delta",
        messageId: params.assistantMessageId,
        textDelta
      });
      const liveMessage: TranscriptMessage = {
        id: params.assistantMessageId,
        role: "assistant",
        text: trimmed,
        proofHash: hashJson({ liveAssistantMessage: params.assistantMessageId, text: trimmed })
      };
      params.commitTranscript(transcriptWithReplacedMessage(params.baseTranscript, liveMessage));
      emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed", params.agentEvents.sessionId);
    },
    shouldStop: (text) => Boolean(params.assistantRun?.cancelled || extractAgentActionJsonRequest(text))
  };
}

function messageOpensQuestionnaire(message: TranscriptMessage): boolean {
  return message.role === "assistant" && message.text.includes(BRAIN_QUESTIONNAIRE_COMMAND);
}

function transcriptHasOpenQuestionnaire(messages: TranscriptMessage[]): boolean {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === "user") {
      return false;
    }
    if (messageOpensQuestionnaire(message)) {
      return true;
    }
  }
  return false;
}

function questionnaireLeadText(lines: string[]): string {
  const paragraph: string[] = [];
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      if (paragraph.length > 0) break;
      continue;
    }
    if (line.startsWith("/") || /^#{1,4}\s+/.test(line) || /^[-*]\s+/.test(line) || /^\d+[.)]\s+/.test(line)) {
      if (paragraph.length > 0) break;
      continue;
    }
    paragraph.push(line);
    if (paragraph.join(" ").length > 220) break;
  }
  return paragraph.join(" ").trim();
}

function questionnaireCommandBlock(lines: string[], commandIndex: number): string {
  const block: string[] = [];
  for (let index = commandIndex; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();
    if (index > commandIndex && !trimmed) {
      break;
    }
    if (
      index > commandIndex &&
      !/^(title|intro|questions|q\d+(?:_(?:options|option[123]|a|b|c))?|mode|output)\s*[:=]/.test(trimmed) &&
      !trimmed.startsWith(BRAIN_QUESTIONNAIRE_COMMAND)
    ) {
      break;
    }
    block.push(line);
  }
  return block.join("\n").trim();
}

function enforceQuestionnaireLoopPause(message: TranscriptMessage): TranscriptMessage {
  if (!messageOpensQuestionnaire(message)) {
    return message;
  }
  const lines = message.text.replace(/\r\n/g, "\n").split("\n");
  const commandIndex = lines.findIndex((line) => line.includes(BRAIN_QUESTIONNAIRE_COMMAND));
  if (commandIndex < 0) {
    return message;
  }
  const lead = questionnaireLeadText(lines.slice(0, commandIndex));
  const commandBlock = questionnaireCommandBlock(lines, commandIndex);
  const text = [lead, commandBlock].filter(Boolean).join("\n\n").trim();
  return {
    ...message,
    text: text || message.text,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      loopPause: BRAIN_QUESTIONNAIRE_COMMAND,
      visibleText: text || message.text
    })
  };
}

function chatArchiveStorePath(): string {
  return join(memoryRootPath(), "chat-session-archive.json");
}

function isChatArchiveSession(value: unknown): value is ChatArchiveSession {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ChatArchiveSession>;
  return (
    candidate.schema === "forge.brain.chat_session_archive.v1" &&
    typeof candidate.sessionId === "string" &&
    typeof candidate.title === "string" &&
    isNativeSection(candidate.section) &&
    typeof candidate.workspaceLabel === "string" &&
    typeof candidate.date === "string" &&
    typeof candidate.createdAt === "string" &&
    typeof candidate.updatedAt === "string" &&
    typeof candidate.archived === "boolean" &&
    Array.isArray(candidate.messages) &&
    typeof candidate.proofHash === "string"
  );
}

function canonicalSessionAssetPath(attachment: Pick<ChatArchiveAttachment, "id" | "name" | "kind">, sourcePath: string): string {
  const extension = extname(sourcePath || attachment.name).toLowerCase();
  const name = safeMemoryFileToken(basename(attachment.name || sourcePath || "attachment"));
  const fingerprint = createHash("sha256")
    .update(`${attachment.id}\0${attachment.name}\0${sourcePath}`)
    .digest("hex")
    .slice(0, 16);
  const fileName = `${fingerprint}-${name}${name.toLowerCase().endsWith(extension) ? "" : extension}`;
  return join(memoryRootPath(), "session-assets", safeMemoryFileToken(attachment.id), safeMemoryFileToken(fileName));
}

function pathInsideMemoryRoot(filePath: string): boolean {
  const root = resolve(memoryRootPath()).toLowerCase();
  const resolvedPath = resolve(filePath).toLowerCase();
  return resolvedPath === root || resolvedPath.startsWith(`${root}\\`) || resolvedPath.startsWith(`${root}/`);
}

function persistArchiveAttachmentLocalCopy<T extends ChatArchiveAttachment>(
  attachment: T,
  sourcePath: string,
  mimeType?: string
): T {
  if (!sourcePath || !existsSync(sourcePath)) {
    return attachment;
  }
  if (pathInsideMemoryRoot(sourcePath)) {
    return {
      ...attachment,
      localPath: sourcePath,
      ...(mimeType ? { mimeType } : {})
    };
  }
  try {
    const targetPath = canonicalSessionAssetPath(attachment, sourcePath);
    mkdirSync(dirname(targetPath), { recursive: true });
    if (!existsSync(targetPath)) {
      copyFileSync(sourcePath, targetPath);
    }
    return {
      ...attachment,
      localPath: targetPath,
      ...(mimeType ? { mimeType } : {})
    };
  } catch (error) {
    console.error(`Failed to copy archive attachment ${attachment.name} into canonical memory store.`, error);
    return {
      ...attachment,
      localPath: sourcePath,
      ...(mimeType ? { mimeType } : {})
    };
  }
}

function normalizeArchiveSessionAssets(session: ChatArchiveSession): ChatArchiveSession {
  return {
    ...session,
    messages: session.messages.map((message) => ({
      ...message,
      attachments: (message.attachments ?? []).map((attachment) =>
        attachment.localPath
          ? persistArchiveAttachmentLocalCopy(attachment, attachment.localPath, attachment.mimeType)
          : attachment
      )
    }))
  };
}

function chatArchiveSessionUpdatedAt(session: ChatArchiveSession): string {
  return session.updatedAt || session.createdAt || "1970-01-01T00:00:00.000Z";
}

function mergeArchiveMessages(left: ChatArchiveMessage[], right: ChatArchiveMessage[]): ChatArchiveMessage[] {
  const messages = new Map<string, ChatArchiveMessage>();
  for (const message of [...left, ...right]) {
    const key = message.turnId || stableSearchArchiveHash(message);
    const existing = messages.get(key);
    if (!existing || (message.createdAt || "") >= (existing.createdAt || "")) {
      messages.set(key, message);
    }
  }
  return Array.from(messages.values())
    .filter((message) => !isInternalTranscriptMessage(message))
    .sort((leftMessage, rightMessage) => (leftMessage.createdAt || "").localeCompare(rightMessage.createdAt || ""));
}

function mergeArchiveSession(left: ChatArchiveSession, right: ChatArchiveSession): ChatArchiveSession {
  const latest = chatArchiveSessionUpdatedAt(right) >= chatArchiveSessionUpdatedAt(left) ? right : left;
  const messages = mergeArchiveMessages(left.messages, right.messages);
  const merged: ChatArchiveSession = {
    ...left,
    ...latest,
    messages,
    createdAt: [left.createdAt, right.createdAt].filter(Boolean).sort()[0] ?? latest.createdAt,
    updatedAt: [left.updatedAt, right.updatedAt].filter(Boolean).sort().at(-1) ?? latest.updatedAt,
    archived: left.archived && right.archived
      ? true
      : latest.archived,
    proofHash: ""
  };
  merged.proofHash = archiveSessionProofHash(merged);
  return normalizeArchiveSessionAssets(merged);
}

function mergeArchiveSessionLists(lists: ChatArchiveSession[][]): ChatArchiveSession[] {
  const sessions = new Map<string, ChatArchiveSession>();
  for (const list of lists) {
    for (const session of list) {
      const existing = sessions.get(session.sessionId);
      sessions.set(session.sessionId, existing ? mergeArchiveSession(existing, session) : normalizeArchiveSessionAssets(session));
    }
  }
  return Array.from(sessions.values()).sort((left, right) => chatArchiveSessionUpdatedAt(right).localeCompare(chatArchiveSessionUpdatedAt(left)));
}

async function readChatArchiveSessionsFromFile(filePath: string, backupReason = "corrupt-chat-archive"): Promise<ChatArchiveSession[] | null> {
  try {
    const raw = await readFile(filePath, "utf8");
    const parsed = JSON.parse(raw) as unknown;
    const sessions = parsed && typeof parsed === "object" && Array.isArray((parsed as { sessions?: unknown }).sessions)
      ? (parsed as { sessions: unknown[] }).sessions
      : [];
    return sessions
      .filter(isChatArchiveSession)
      .map((session) => ({
        ...session,
        messages: session.messages.filter((message) => !isInternalTranscriptMessage(message))
      }));
  } catch (error) {
    if (existsSync(filePath)) {
      await backupMemoryFile(filePath, backupReason);
      console.error(`Failed to read chat archive ${filePath}; preserved a backup before recovery.`, error);
    }
    return null;
  }
}

async function sortedMemoryBackupFiles(fileName: string): Promise<string[]> {
  try {
    const entries = await readdir(memoryBackupDir(), { withFileTypes: true });
    return entries
      .filter((entry) => entry.isFile() && entry.name.startsWith(`${safeMemoryFileToken(fileName)}.`))
      .map((entry) => join(memoryBackupDir(), entry.name))
      .sort((left, right) => memoryFileMtimeMs(right) - memoryFileMtimeMs(left));
  } catch {
    return [];
  }
}

async function recoverChatArchiveSessions(): Promise<ChatArchiveSession[]> {
  const primaryPath = chatArchiveStorePath();
  const primary = await readChatArchiveSessionsFromFile(primaryPath);
  if (primary) {
    return primary;
  }
  for (const backupPath of await sortedMemoryBackupFiles(basename(primaryPath))) {
    const recovered = await readChatArchiveSessionsFromFile(backupPath, "corrupt-chat-archive-backup");
    if (recovered && recovered.length > 0) {
      return recovered;
    }
  }
  const legacyLists: ChatArchiveSession[][] = [];
  for (const legacyRoot of memoryLegacyRootDirs()) {
    const legacy = await readChatArchiveSessionsFromFile(join(legacyRoot, basename(primaryPath)), "corrupt-legacy-chat-archive");
    if (legacy?.length) {
      legacyLists.push(legacy);
    }
  }
  return mergeArchiveSessionLists(legacyLists);
}

async function persistMergedChatArchive(sessions: ChatArchiveSession[], reason: string): Promise<void> {
  const selected = sessions
    .sort((left, right) => chatArchiveSessionUpdatedAt(right).localeCompare(chatArchiveSessionUpdatedAt(left)));
  const envelope = {
    schema: "forge.brain.chat_archive_store.v1",
    updatedAt: new Date().toISOString(),
    memorySource: "canonical-userData/brain",
    sessions: selected,
    proofHash: stableSearchArchiveHash(selected.map((session) => session.proofHash))
  };
  await atomicWriteJsonFile(chatArchiveStorePath(), envelope, reason);
}

async function reconcileCanonicalBrainIdentityStore(): Promise<void> {
  const snapshots: BrainIdentityFileSnapshot[] = [];
  const canonical = await readBrainIdentityContextFromFile(brainIdentityStorePath());
  if (canonical) {
    snapshots.push(canonical);
  }
  for (const legacyRoot of memoryLegacyRootDirs()) {
    const legacy = await readBrainIdentityContextFromFile(join(legacyRoot, basename(brainIdentityStorePath())));
    if (legacy) {
      snapshots.push(legacy);
    }
  }
  if (snapshots.length === 0) {
    return;
  }
  const merged = mergeBrainIdentitySnapshots(snapshots);
  if (!canonical || !brainIdentityContextsEqual(canonical.context, merged)) {
    await atomicWriteJsonFile(brainIdentityStorePath(), merged, "startup-brain-identity-merge");
  }
}

async function reconcileCanonicalChatArchiveStore(): Promise<void> {
  const primaryPath = chatArchiveStorePath();
  const archiveLists: ChatArchiveSession[][] = [];
  const canonical = await readChatArchiveSessionsFromFile(primaryPath);
  if (canonical) {
    archiveLists.push(canonical);
  }
  for (const legacyRoot of memoryLegacyRootDirs()) {
    const legacy = await readChatArchiveSessionsFromFile(join(legacyRoot, basename(primaryPath)), "corrupt-legacy-chat-archive");
    if (legacy?.length) {
      archiveLists.push(legacy);
    }
  }
  if (archiveLists.length === 0) {
    return;
  }
  const merged = mergeArchiveSessionLists(archiveLists);
  if (!canonical || merged.length !== canonical.length || archiveLists.length > 1) {
    await persistMergedChatArchive(merged, "startup-chat-archive-merge");
  }
}

async function writeCanonicalMemoryManifest(): Promise<void> {
  const manifest = {
    schema: "ingen.memory.canonical_store.v1",
    updatedAt: new Date().toISOString(),
    sourceOfTruth: memoryRootPath(),
    policy: "All durable sessions, Brain identity, Brain rules, learning memory manifests and session assets live under this single brain root. Legacy roots are migration inputs only.",
    migratedLegacyRoots: memoryLegacyRootDirs().filter((root) => existsSync(root)),
    chatArchivePath: chatArchiveStorePath(),
    brainIdentityPath: brainIdentityStorePath(),
    rendererBrainMemoryPath: brainRendererMemoryStorePath(),
    sessionArtifactLedgerPath: sessionArtifactLedgerStorePath(),
    sessionArtifactAssetsPath: sessionArtifactAssetsDir(),
    sessionAssetsPath: join(memoryRootPath(), "session-assets"),
    backupPath: memoryBackupDir(),
    proofHash: stableSearchArchiveHash({
      sourceOfTruth: memoryRootPath(),
      chatArchivePath: chatArchiveStorePath(),
      brainIdentityPath: brainIdentityStorePath(),
      rendererBrainMemoryPath: brainRendererMemoryStorePath(),
      sessionArtifactLedgerPath: sessionArtifactLedgerStorePath()
    })
  };
  await atomicWriteJsonFile(join(memoryRootPath(), "memory-source.json"), manifest, "memory-source");
}

async function reconcileCanonicalMemoryStore(): Promise<void> {
  await mkdir(memoryRootPath(), { recursive: true });
  await mkdir(memoryBackupDir(), { recursive: true });
  await mkdir(join(memoryRootPath(), "session-assets"), { recursive: true });
  await reconcileCanonicalBrainIdentityStore();
  await reconcileCanonicalChatArchiveStore();
  await ensureBrainCanonicalMemorySnapshotOnDisk();
  await ensureSessionArtifactLedgerOnDisk();
  await writeCanonicalMemoryManifest();
}

async function loadChatArchive(): Promise<void> {
  if (chatArchiveLoaded) return;
  if (chatArchiveLoadPromise) {
    await chatArchiveLoadPromise;
    return;
  }
  chatArchiveLoadPromise = (async () => {
    try {
      const sessions = await recoverChatArchiveSessions();
      chatArchiveSessions.clear();
      for (const session of sessions) {
        chatArchiveSessions.set(session.sessionId, session);
      }
      ensureAssistantPatternDemoSession();
      ensureSearchArchiveLoopDemoSession();
      syncLocalChatSessionsFromArchive();
    } catch (error) {
      console.error("Failed to load canonical chat archive; preserving existing in-memory sessions.", error);
    } finally {
      chatArchiveLoaded = true;
      chatArchiveLoadPromise = undefined;
    }
  })();
  await chatArchiveLoadPromise;
}

function persistChatArchiveSoon(): void {
  chatArchiveWriteQueue = chatArchiveWriteQueue
    .catch(() => undefined)
    .then(async () => {
      await persistMergedChatArchive(Array.from(chatArchiveSessions.values()), "chat-archive");
    })
    .catch((error: unknown) => {
      console.error("Failed to persist chat archive.", error);
    });
}

function ensureBackendSessionArchiveShells(): void {
  let projection: RustBackendProjection;
  try {
    projection = rustBackend();
  } catch {
    return;
  }
  let changed = false;
  for (const backendSession of projection.sessions) {
    if (!backendSession.sessionId || !backendSession.label.trim()) {
      continue;
    }
    const existing = chatArchiveSessions.get(backendSession.sessionId);
    if (existing) {
      if (existing.messages.length === 0) {
        const workspaceLabel = sessionWorkspaceLabel(backendSession.section);
        const nextArchived = backendSession.archived;
        if (
          existing.title !== backendSession.label ||
          existing.date !== backendSession.date ||
          existing.section !== backendSession.section ||
          existing.workspaceLabel !== workspaceLabel ||
          existing.archived !== nextArchived
        ) {
          existing.title = backendSession.label;
          existing.date = backendSession.date;
          existing.section = backendSession.section;
          existing.workspaceLabel = workspaceLabel;
          existing.archived = nextArchived;
          existing.updatedAt = new Date().toISOString();
          existing.proofHash = archiveSessionProofHash(existing);
          changed = true;
        }
      }
      continue;
    }
    const createdAt = `${backendSession.date || todayIsoDate()}T00:00:00.000Z`;
    const session: ChatArchiveSession = {
      schema: "forge.brain.chat_session_archive.v1",
      sessionId: backendSession.sessionId,
      title: backendSession.label,
      section: backendSession.section,
      workspaceLabel: sessionWorkspaceLabel(backendSession.section),
      date: backendSession.date || todayIsoDate(),
      createdAt,
      updatedAt: createdAt,
      archived: backendSession.archived,
      messages: [],
      proofHash: ""
    };
    session.proofHash = archiveSessionProofHash(session);
    chatArchiveSessions.set(session.sessionId, session);
    changed = true;
  }
  if (changed) {
    persistChatArchiveSoon();
  }
}

function canonicalSearchArchiveSessions(): ChatArchiveSession[] {
  ensureBackendSessionArchiveShells();
  return Array.from(chatArchiveSessions.values());
}

function archiveMetaForSession(session: SidebarSessionItem): ChatArchiveSessionMeta {
  const archived = session.archived || (session.sessionId !== "" && session.sessionId === sidebarState.archivedSessionId);
  return {
    sessionId: session.sessionId,
    title: session.label,
    section: session.section,
    workspaceLabel: session.workspaceLabel ?? sessionWorkspaceLabel(session.section),
    date: session.date,
    archived
  };
}

function archiveAttachmentPreview(attachment: ComposerUploadPreview): ChatArchiveAttachment {
  const cached = composerUploadPreviewItems.get(attachment.id);
  const preview: ChatArchiveAttachment = {
    ...attachment,
    url: cached ? uploadPreviewUrl(attachment.id, attachment.name) : attachment.url,
    ...(cached ? { localPath: cached.path, mimeType: cached.mimeType } : {})
  };
  return cached ? persistArchiveAttachmentLocalCopy(preview, cached.path, cached.mimeType) : preview;
}

function publicArchiveAttachmentPreview(attachment: ChatArchiveAttachment): ComposerUploadPreview {
  const { localPath: _localPath, mimeType: _mimeType, ...preview } = attachment;
  return {
    ...preview,
    url: attachment.localPath ? uploadPreviewUrl(preview.id, preview.name) : preview.url
  };
}

function uploadItemFromArchiveAttachment(attachment: ChatArchiveAttachment): ComposerUploadItem | undefined {
  if (!attachment.localPath) {
    return undefined;
  }
  return {
    ...publicArchiveAttachmentPreview(attachment),
    path: attachment.localPath,
    mimeType: attachment.mimeType || uploadPreviewMimeType(attachment.localPath || attachment.name, attachment.kind)
  };
}

function rememberArchiveAttachmentPreview(attachment: ChatArchiveAttachment): void {
  const item = uploadItemFromArchiveAttachment(attachment);
  if (item) {
    composerUploadPreviewItems.set(item.id, item);
  }
}

function uniqueSessionFiles(files: ComposerUploadPreview[]): ComposerUploadPreview[] {
  const seen = new Set<string>();
  const unique: ComposerUploadPreview[] = [];
  for (const file of files) {
    if (!seen.has(file.id)) {
      seen.add(file.id);
      unique.push(file);
    }
  }
  return unique;
}

function filesFromTranscript(messages: TranscriptMessage[]): ComposerUploadPreview[] {
  return uniqueSessionFiles(
    publicTranscript(messages).flatMap((message) => message.attachments ?? [])
  );
}

function filesFromArchiveSession(session: ChatArchiveSession): ComposerUploadPreview[] {
  for (const message of session.messages) {
    for (const attachment of message.attachments ?? []) {
      rememberArchiveAttachmentPreview(attachment);
    }
  }
  return uniqueSessionFiles(
    session.messages.flatMap((message) => (message.attachments ?? []).map(publicArchiveAttachmentPreview))
  );
}

function sessionFilesSnapshot(): SessionFilesSnapshot {
  const groups: SessionFilesSnapshot["groups"] = [];
  const seenSessions = new Set<string>();
  const pushGroup = (group: Omit<SessionFilesSnapshot["groups"][number], "proofHash">) => {
    if (!group.sessionId || seenSessions.has(group.sessionId) || group.files.length === 0) {
      return;
    }
    seenSessions.add(group.sessionId);
    const proofHash = hashJson(group);
    groups.push({ ...group, proofHash });
  };

  const activeSessionId = panelsChatBottomState.activeSessionId;
  if (activeSessionId) {
    const item = sessionById(activeSessionId);
    const archiveSession = chatArchiveSessions.get(activeSessionId);
    pushGroup({
      sessionId: activeSessionId,
      sessionName: item?.label ?? archiveSession?.title ?? "Current session",
      date: item?.date ?? archiveSession?.date ?? todayIsoDate(),
      archived: item?.archived ?? archiveSession?.archived ?? false,
      files: filesFromTranscript(panelsChatBottomState.transcript)
    });
  }

  for (const lane of Array.from(parallelChatLanes.values()).sort((left, right) => left.sessionId.localeCompare(right.sessionId))) {
    const item = sessionById(lane.sessionId);
    pushGroup({
      sessionId: lane.sessionId,
      sessionName: item?.label ?? "Parallel session",
      date: item?.date ?? todayIsoDate(),
      archived: item?.archived ?? false,
      files: filesFromTranscript(lane.transcript)
    });
  }

  for (const session of Array.from(chatArchiveSessions.values()).sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))) {
    pushGroup({
      sessionId: session.sessionId,
      sessionName: session.title || "New session",
      date: session.date,
      archived: session.archived,
      files: filesFromArchiveSession(session)
    });
  }

  const fileCount = groups.reduce((total, group) => total + group.files.length, 0);
  return {
    schema: "ingen.electron.session_files.snapshot.v1",
    groups,
    fileCount,
    proofHash: stableSearchArchiveHash(groups.map((group) => group.proofHash))
  };
}

function archiveReadyMessage(message: TranscriptMessage): TranscriptMessage {
  return {
    ...message,
    attachments: (message.attachments ?? []).map(archiveAttachmentPreview)
  };
}

function archiveTranscriptMessage(session: SidebarSessionItem, message: TranscriptMessage, createdAt = new Date().toISOString()): void {
  upsertArchiveMessage(chatArchiveSessions, archiveMetaForSession(session), archiveReadyMessage(message), createdAt);
  persistChatArchiveSoon();
}

function renameArchiveSessionTitle(sessionId: string, request: RenameSessionCodeActRequest, date = todayIsoDate()): boolean {
  const archiveSession = chatArchiveSessions.get(sessionId);
  if (!archiveSession) return false;
  archiveSession.title = request.title;
  archiveSession.date = date;
  archiveSession.updatedAt = new Date().toISOString();
  archiveSession.proofHash = archiveSessionProofHash(archiveSession);
  persistChatArchiveSoon();
  return true;
}

function renameChatSession(session: SidebarSessionItem, request: RenameSessionCodeActRequest): void {
  session.label = request.title;
  session.date = todayIsoDate();
  renameArchiveSessionTitle(session.sessionId, request, session.date);
}

function sidebarRenameRequest(label: string): RenameSessionCodeActRequest | undefined {
  const title = normalizeSessionTitle(label);
  if (!title) return undefined;
  const reason = "manual sidebar rename";
  return {
    schema: "forge.brain.rename_session.request.v1",
    command: BRAIN_RENAME_SESSION_COMMAND,
    title,
    reason,
    proofHash: stableSearchArchiveHash({ command: BRAIN_RENAME_SESSION_COMMAND, title, reason })
  };
}

function renameSidebarSession(sessionId: string, label: string): boolean {
  const request = sidebarRenameRequest(label);
  if (!request) return false;
  let changed = false;
  for (const session of localChatSessions.filter((item) => sidebarSessionCommandMatches(item, sessionId))) {
    renameChatSession(session, request);
    changed = true;
  }
  for (const session of seededSessions.filter((item) => sidebarSessionCommandMatches(item, sessionId))) {
    session.label = request.title;
    session.date = todayIsoDate();
    changed = true;
  }
  changed = renameArchiveSessionTitle(sessionId, request) || changed;
  if (sidebarState.archiveConfirm.candidateId === sessionId) {
    sidebarState.archiveConfirm = { ...sidebarState.archiveConfirm, candidateLabel: request.title };
  }
  return changed;
}

function sidebarSessionFromArchive(session: ChatArchiveSession): SidebarSessionItem {
  return {
    sessionId: session.sessionId,
    label: session.title || "New session",
    date: session.date,
    section: session.section,
    workspaceLabel: session.workspaceLabel || sessionWorkspaceLabel(session.section),
    rowVisible: !session.archived,
    pinned: false,
    working: false,
    automated: false,
    archived: session.archived
  };
}

function syncLocalChatSessionsFromArchive(): void {
  const existingById = new Map(localChatSessions.filter((session) => session.sessionId).map((session) => [session.sessionId, session]));
  const archiveIds = new Set(chatArchiveSessions.keys());
  const restored = Array.from(chatArchiveSessions.values())
    .filter((session) => !session.archived)
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .map((archiveSession) => {
      const archivedItem = sidebarSessionFromArchive(archiveSession);
      const existing = existingById.get(archiveSession.sessionId);
      return existing
        ? {
            ...existing,
            ...archivedItem,
            pinned: existing.pinned,
            working: existing.working,
            automated: existing.automated
          }
        : archivedItem;
    });
  const transient = localChatSessions.filter((session) => !session.sessionId || !archiveIds.has(session.sessionId));
  localChatSessions.splice(0, localChatSessions.length, ...restored, ...transient);
}

function restoreChatSessionToCanvas(sessionId: string): boolean {
  if (sessionId === SEARCHARCHIVE_LOOP_DEMO_SESSION_ID) {
    ensureSearchArchiveLoopDemoSession();
    chatArchiveSessions.set(SEARCHARCHIVE_LOOP_DEMO_SESSION_ID, searchArchiveLoopRealDemoArchiveSession());
  }
  const archiveSession = chatArchiveSessions.get(sessionId);
  if (!archiveSession) return false;
  providerAttachmentCache.clear();
  parallelChatLanes.clear();
  for (const message of archiveSession.messages) {
    for (const attachment of message.attachments ?? []) {
      rememberArchiveAttachmentPreview(attachment);
    }
  }
  panelsChatBottomState.activeSessionId = sessionId;
  panelsChatBottomState.chatText = "";
  panelsChatBottomState.transcript = archiveSession.messages
    .filter((message) => !isInternalTranscriptMessage(message))
    .map((message) => ({
      id: message.turnId,
      role: message.role,
      text: message.text,
      attachments: (message.attachments ?? []).map(publicArchiveAttachmentPreview),
      proofHash: message.proofHash
    }));
  panelsChatBottomState.uploadItems = [];
  panelsChatBottomState.uploadCount = 0;
  panelsChatBottomState.uploadErrorText = "";
  panelsChatBottomState.uploadEditTargetId = "";
  panelsChatBottomState.permissionModeOpen = false;
  panelsChatBottomState.activeBrainSegment = activeBrainSegmentFromTranscript(panelsChatBottomState.transcript);
  ensureBrainBootTranscript(sessionId);
  return true;
}

function archiveMessageToTranscriptMessage(message: ChatArchiveMessage): TranscriptMessage {
  return {
    id: message.turnId,
    role: message.role,
    text: message.text,
    attachments: (message.attachments ?? []).map(publicArchiveAttachmentPreview),
    proofHash: message.proofHash
  };
}

function archiveTranscriptAroundTurn(session: ChatArchiveSession, turnId: string): TranscriptMessage[] {
  const messages = session.messages
    .filter((message) => !isInternalTranscriptMessage(message))
    .map(archiveMessageToTranscriptMessage);
  if (!turnId || messages.length <= 48) {
    return messages;
  }
  const focusIndex = messages.findIndex((message) => message.id === turnId);
  if (focusIndex < 0) {
    return messages.slice(-48);
  }
  const start = Math.max(0, Math.min(focusIndex - 18, messages.length - 48));
  return messages.slice(start, start + 48);
}

function archiveTranscriptWithFocusSnippet(
  messages: TranscriptMessage[],
  turnId: string,
  focusSnippet: string
): TranscriptMessage[] {
  const snippet = focusSnippet.trim();
  if (!turnId || !snippet) {
    return messages;
  }
  return messages.map((message) =>
    message.id === turnId
      ? { ...message, archiveFocusSnippet: snippet }
      : message
  );
}

function openArchiveSessionInParallel(sessionId: string, turnId: string, parallelSessionIndex: number, focusSnippet = ""): boolean {
  const boundedIndex = Math.max(1, Math.min(3, parallelSessionIndex));
  if (sessionId === SEARCHARCHIVE_LOOP_DEMO_SESSION_ID || sessionId === SEARCHARCHIVE_LOOP_DEMO_MEMORY_SESSION_ID) {
    ensureSearchArchiveLoopDemoSession();
  }
  const archiveSession = chatArchiveSessions.get(sessionId);
  if (!archiveSession) {
    return false;
  }
  for (const message of archiveSession.messages) {
    for (const attachment of message.attachments ?? []) {
      rememberArchiveAttachmentPreview(attachment);
    }
  }

  const primarySession = ensureActiveChatSession("Parallel 1: New session", { markWorking: false });
  const groupId = primarySession.parallelGroupId || `parallel-${primarySession.sessionId}`;
  primarySession.parallelGroupId = groupId;
  primarySession.parallelLaneIndex = 0;
  primarySession.label = `Parallel 1: ${cleanParallelLaneLabel(primarySession.label)}`;
  if (!parallelChatLanes.has(0)) {
    parallelChatLanes.set(0, {
      sessionId: primarySession.sessionId,
      transcript: panelsChatBottomState.transcript,
      groupId
    });
  }

  const archiveItem = materializeOpenedChatSession(sessionId, archiveSession.section);
  archiveItem.parallelGroupId = groupId;
  archiveItem.parallelLaneIndex = boundedIndex;
  archiveItem.parallelLaneCount = Math.max(boundedIndex + 1, 2);
  archiveItem.parallelPeerSessionIds = [primarySession.sessionId];
  const transcript = archiveTranscriptWithFocusSnippet(
    archiveTranscriptAroundTurn(archiveSession, turnId),
    turnId,
    focusSnippet
  );
  parallelChatLanes.set(boundedIndex, {
    sessionId,
    transcript,
    groupId,
    focusMessageId: turnId,
    sessionTitle: archiveSession.title
  });
  updateParallelGroupMetadata(groupId);
  sidebarState.recentSessionId = groupId;
  return true;
}

function closeParallelChatLane(parallelSessionIndex: number): boolean {
  const index = Math.max(1, Math.min(3, parallelSessionIndex));
  if (!parallelChatLanes.has(index)) {
    return false;
  }
  const groupId = parallelChatLanes.get(index)?.groupId ?? "";
  parallelChatLanes.delete(index);
  const shifted = Array.from(parallelChatLanes.entries())
    .filter(([laneIndex]) => laneIndex > index)
    .sort(([left], [right]) => left - right);
  for (const [laneIndex, lane] of shifted) {
    parallelChatLanes.delete(laneIndex);
    parallelChatLanes.set(laneIndex - 1, lane);
  }
  if (groupId) {
    updateParallelGroupMetadata(groupId);
  }
  return true;
}

async function searchChatArchive(request: SearchArchiveRequest): Promise<SearchArchiveResult> {
  await loadChatArchive();
  return searchArchiveSessions(canonicalSearchArchiveSessions(), request);
}

function isSearchArchiveRequest(value: unknown): value is SearchArchiveRequest {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<SearchArchiveRequest>;
  return (
    typeof candidate.query === "string" &&
    (candidate.keywords === undefined || (Array.isArray(candidate.keywords) && candidate.keywords.every((item) => typeof item === "string"))) &&
    (candidate.dateFrom === undefined || typeof candidate.dateFrom === "string") &&
    (candidate.dateTo === undefined || typeof candidate.dateTo === "string") &&
    (candidate.scope === undefined || candidate.scope === "current" || candidate.scope === "recent" || candidate.scope === "archived" || candidate.scope === "all") &&
    (candidate.sessionScope === undefined || candidate.sessionScope === "current" || candidate.sessionScope === "recent" || candidate.sessionScope === "archived" || candidate.sessionScope === "all") &&
    (candidate.currentSessionId === undefined || typeof candidate.currentSessionId === "string") &&
    (candidate.contentScope === undefined || candidate.contentScope === "messages" || candidate.contentScope === "files" || candidate.contentScope === "artifacts" || candidate.contentScope === "all") &&
    (candidate.fileOrigin === undefined || candidate.fileOrigin === "uploaded" || candidate.fileOrigin === "created_in_app" || candidate.fileOrigin === "all") &&
    (candidate.createdInAppSources === undefined || (Array.isArray(candidate.createdInAppSources) && candidate.createdInAppSources.every((item) => typeof item === "string"))) &&
    (candidate.fileTypes === undefined || (Array.isArray(candidate.fileTypes) && candidate.fileTypes.every((item) => typeof item === "string"))) &&
    (candidate.topK === undefined || typeof candidate.topK === "number") &&
    (candidate.contextTurns === undefined || typeof candidate.contextTurns === "number") &&
    (candidate.targets === undefined || (Array.isArray(candidate.targets) && candidate.targets.every((target) => typeof target === "string"))) &&
    (candidate.includeFilePreviews === undefined || typeof candidate.includeFilePreviews === "boolean") &&
    (candidate.includeArtifactRefs === undefined || typeof candidate.includeArtifactRefs === "boolean") &&
    (candidate.templateProofHash === undefined || typeof candidate.templateProofHash === "string")
  );
}

async function localSearchArchiveStatus(request: SearchArchiveRequest, session?: SidebarSessionItem): Promise<TranscriptMessage> {
  await loadChatArchive();
  const executableRequest = { ...request, currentSessionId: request.currentSessionId ?? session?.sessionId };
  const result = searchArchiveSessions(canonicalSearchArchiveSessions(), executableRequest);
  const text = renderSearchArchiveResult(result);
  return {
    id: `assistant-searcharchive-${Date.now()}`,
    role: "assistant",
    text,
    proofHash: hashJson({ codeact: BRAIN_SEARCHARCHIVE_COMMAND, request: executableRequest, result })
  };
}

function localSearchArchiveTemplateStatus(reason: "empty_command" | "template_required" = "empty_command"): TranscriptMessage {
  const result = searchArchiveTemplateResult(reason);
  return {
    id: `assistant-searcharchive-template-${Date.now()}`,
    role: "assistant",
    text: renderSearchArchiveTemplateResult(result),
    proofHash: hashJson({ codeact: BRAIN_SEARCHARCHIVE_COMMAND, result })
  };
}

async function executeAssistantSearchArchiveCodeAct(message: TranscriptMessage, session?: SidebarSessionItem): Promise<TranscriptMessage> {
  if (message.role !== "assistant" || message.text.includes("SEARCHARCHIVE_RESULT") || message.text.includes("SEARCHARCHIVE_TEMPLATE_RESULT")) {
    return message;
  }
  const codeAct = readSearchArchiveCodeAct(message.text);
  if (!codeAct) {
    return message;
  }
  if (codeAct.kind === "template") {
    return {
      ...message,
      text: `${message.text.trimEnd()}\n\n${renderSearchArchiveTemplateResult(codeAct.result)}`,
      proofHash: hashJson({
        previousProofHash: message.proofHash,
        assistantCodeAct: BRAIN_SEARCHARCHIVE_COMMAND,
        searchArchiveTemplate: codeAct.result
      })
    };
  }
  await loadChatArchive();
  const executableRequest = { ...codeAct.request, currentSessionId: codeAct.request.currentSessionId ?? session?.sessionId };
  const result = searchArchiveSessions(canonicalSearchArchiveSessions(), executableRequest);
  return {
    ...message,
    text: `${message.text.trimEnd()}\n\n${renderSearchArchiveResult(result)}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: BRAIN_SEARCHARCHIVE_COMMAND,
      searchArchiveRequest: executableRequest,
      searchArchiveResult: result
    })
  };
}

function executeAssistantGoogleWebCodeAct(message: TranscriptMessage, parallelSessionIndex = 0): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractGoogleWebCodeAct(message.text);
  if (!request) {
    return message;
  }
  const navigation = navigateNativeWebExplorerToGoogle(request, parallelSessionIndex);
  const executionText = renderGoogleWebCodeActResult(request);
  return {
    ...message,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      navigation
    })
  };
}

async function executeAssistantMapsCodeAct(message: TranscriptMessage, parallelSessionIndex = 0, originalUserText = ""): Promise<TranscriptMessage> {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractMapsCodeAct(message.text);
  if (!request) {
    return message;
  }
  if (shouldSuppressMapsForUserText(originalUserText)) {
    const text = stripMapsCodeActLines(message.text) || assistantCodeActVisibleText(message.text);
    return {
      ...message,
      text,
      proofHash: hashJson({
        previousProofHash: message.proofHash,
        suppressedMapsCodeAct: request,
        originalUserText
      })
    };
  }
  const resolvedRequest = await resolveMapsCodeActRequest(request);
  const navigation = navigateNativeWebExplorerToMaps(resolvedRequest, parallelSessionIndex);
  const executionText = renderMapsCodeActResult(resolvedRequest);
  return {
    ...message,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: resolvedRequest,
      navigation
    })
  };
}

function executeAssistantGmailCodeAct(message: TranscriptMessage, parallelSessionIndex = 0): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractGmailCodeAct(message.text);
  if (!request) {
    return message;
  }
  const navigation = navigateNativeWebExplorerToGmail(request, parallelSessionIndex);
  const executionText = renderGmailCodeActResult(request);
  return {
    ...message,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      navigation
    })
  };
}

function executeAssistantAirbnbCodeAct(message: TranscriptMessage, parallelSessionIndex = 0): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractAirbnbCodeAct(message.text);
  if (!request) {
    return message;
  }
  const navigation = navigateNativeWebExplorerToAirbnb(request, parallelSessionIndex);
  const executionText = renderAirbnbCodeActResult(request);
  const hasVisibleText = Boolean(assistantCodeActVisibleText(message.text));
  const visibleFallback = !hasVisibleText && request.say ? `${request.say.trim()}\n\n` : "";
  return {
    ...message,
    text: `${visibleFallback}${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      navigation
    })
  };
}

async function executeAssistantWebSearchCodeAct(message: TranscriptMessage): Promise<TranscriptMessage> {
  if (
    message.role !== "assistant" ||
    message.text.includes("WEBSEARCH_RESULT") ||
    message.text.includes("WEBSEARCH_TEMPLATE_RESULT")
  ) {
    return message;
  }
  const codeAct = readWebSearchCodeAct(message.text);
  if (!codeAct) {
    return message;
  }
  if (codeAct.kind === "template") {
    const executionText = renderWebSearchTemplateResult(codeAct.result);
    return {
      ...message,
      text: `${message.text.trim()}\n\n${executionText}`,
      proofHash: hashJson({
        previousProofHash: message.proofHash,
        assistantCodeActTemplate: codeAct.result
      })
    };
  }
  const { request } = codeAct;
  const result = await runWebSearchBridge(request);
  const executionText = renderWebSearchCodeActResult(result);
  return {
    ...message,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      webSearchResult: result
    })
  };
}

async function executeAssistantPriorityMcpCodeAct(message: TranscriptMessage): Promise<TranscriptMessage> {
  if (
    message.role !== "assistant" ||
    message.text.includes("CODEDOCS_RESULT") ||
    message.text.includes("GITHUB_MCP_RESULT") ||
    message.text.includes("WEBACT_RESULT") ||
    message.text.includes("SECURITYSCAN_RESULT")
  ) {
    return message;
  }
  const request = extractPriorityMcpCodeAct(message.text);
  if (!request) {
    return message;
  }
  const result = await runPriorityMcpBridge(request);
  const executionText = renderPriorityMcpCodeActResult(result);
  return {
    ...message,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      priorityMcpResult: result
    })
  };
}

async function executeAssistantScrapersCodeAct(message: TranscriptMessage): Promise<TranscriptMessage> {
  if (message.role !== "assistant" || message.text.includes("SCRAPERS_RESULT")) {
    return message;
  }
  const request = extractScrapersCodeAct(message.text);
  if (!request) {
    return message;
  }
  const result = await runScrapersMcpBridge(request);
  const executionText = renderScrapersCodeActResult(result);
  const visualAttachments = scrapersVisualAttachments(result);
  return {
    ...message,
    attachments: visualAttachments.length > 0
      ? [...(message.attachments ?? []), ...visualAttachments]
      : message.attachments,
    text: `${message.text.trim()}\n\n${executionText}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      scrapersResult: result,
      visualAttachments: visualAttachments.map((attachment) => ({
        id: attachment.id,
        kind: attachment.kind,
        url: attachment.url
      }))
    })
  };
}

function removeRenameSessionChatter(text: string): string {
  const renameSentence = /(?:^|[\r\n]\s*|(?<=[.!?]\s))(?:je\s+)?(?:renomme|renommage|j['’]ai\s+renomme|titre\s+de\s+session|sujet\s*:)[^.!?\r\n]*(?:session|titre|sujet|renomm)[^.!?\r\n]*[.!?]?\s*/giu;
  return text
    .replace(renameSentence, "")
    .replace(/^\s*sujet\s*:\s*[^.!?\r\n]+[.!?]?\s*/iu, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function removeLooseRenameSessionChatter(text: string): string {
  return text
    .replace(/^\s*sujet\s+(?:identifi[eé])?\s*:?\s*[^.!?\r\n]+[.!?]?\s*/iu, "")
    .replace(/(?:^|[\r\n]\s*|(?<=[.!?]\s))je\s+(?:renomme|vais\s+renommer|utilise)[^.!?\r\n]*(?:session|titre|renomm|rename_session)[^.!?\r\n]*[.!?]?\s*/giu, "")
    .replace(/(?:^|[\r\n]\s*)[^.!?\r\n]*(?:action|codeact)[^.!?\r\n]*(?:renommer|rename_session|session)[^.!?\r\n]*[.!?]?\s*/giu, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function sanitizeAssistantRenameChatter(message: TranscriptMessage): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const text = removeRenameSessionChatter(removeLooseRenameSessionChatter(message.text));
  if (text === message.text) {
    return message;
  }
  return {
    ...message,
    text,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      sanitizedRenameChatter: true,
      text
    })
  };
}

function executeAssistantRenameSessionCodeAct(message: TranscriptMessage, session: SidebarSessionItem): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractRenameSessionCodeAct(message.text);
  if (!request) {
    return message;
  }
  renameChatSession(session, request);
  const visibleText =
    removeRenameSessionChatter(removeLooseRenameSessionChatter(removeRenameSessionCodeActLines(message.text))) ||
    assistantCodeActVisibleText(message.text);
  return {
    ...message,
    text: visibleText,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      assistantCodeAct: request,
      renamedSessionId: session.sessionId,
      renamedTitle: session.label
    })
  };
}

function executeAssistantBrainSegmentCodeAct(message: TranscriptMessage): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const nextSegment = brainSegmentFromAssistantText(message.text);
  if (!nextSegment) {
    return message;
  }
  panelsChatBottomState.activeBrainSegment = nextSegment;
  return {
    ...message,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      brainSegment: nextSegment,
      command: nextSegment === "science" ? BRAIN_SCIENCE_COMMAND : BRAIN_CODING_COMMAND
    })
  };
}

function brainSegmentFromAssistantText(text: string): ActiveBrainSegmentId | undefined {
  const trimmed = text.trim();
  if (trimmed.includes(BRAIN_SCIENCE_COMMAND)) {
    return "science";
  }
  if (trimmed.includes(BRAIN_CODING_COMMAND)) {
    return "coding";
  }
  return undefined;
}

function activeBrainSegmentFromTranscript(messages: TranscriptMessage[]): BrainSegmentId {
  let activeSegment: BrainSegmentId = "general";
  for (const message of messages) {
    if (message.role !== "assistant") {
      continue;
    }
    const nextSegment = brainSegmentFromAssistantText(message.text);
    if (nextSegment) {
      activeSegment = nextSegment;
    }
  }
  return activeSegment;
}

function brainSegmentCommand(segment: ActiveBrainSegmentId): typeof BRAIN_SCIENCE_COMMAND | typeof BRAIN_CODING_COMMAND {
  return segment === "science" ? BRAIN_SCIENCE_COMMAND : BRAIN_CODING_COMMAND;
}

function removeBrainSegmentCommandLines(text: string, segment: ActiveBrainSegmentId): string {
  const command = brainSegmentCommand(segment);
  return text
    .split(/\r?\n/)
    .filter((line) => !line.includes(command))
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function suppressRepeatedBrainSegmentCodeAct(message: TranscriptMessage, activeSegment: BrainSegmentId): TranscriptMessage {
  if (message.role !== "assistant" || activeSegment === "general") {
    return message;
  }
  const nextSegment = brainSegmentFromAssistantText(message.text);
  if (nextSegment !== activeSegment) {
    return message;
  }
  const strippedText = removeBrainSegmentCommandLines(message.text, activeSegment);
  return {
    ...message,
    text: strippedText || assistantCodeActVisibleText(message.text),
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      suppressedRepeatedBrainSegment: activeSegment,
      command: brainSegmentCommand(activeSegment)
    })
  };
}

function brainSegmentContinuationUserText(userText: string, segment: ActiveBrainSegmentId): string {
  const command = brainSegmentCommand(segment);
  const catalog = segment === "science" ? "Science/Engineering/3D Brain" : "Coding Brain";
  const lines = [
    userText || "Continue the current user request.",
    "",
    `InGen context: ${command} has just been activated. The ${catalog} catalog is now injected. Continue the user request with this Brain active; do not reactivate ${command} unless a new request requires it.`,
    agentLoopNarrationContractManifest(),
    `questionnaire_pause=If useful questions are needed before acting, do not write a checklist in the Canvas: activate ${BRAIN_QUESTIONNAIRE_COMMAND} with title, optional intro, q1..q5 max, one question per page, and three concrete expert options in qN_options. Use only if guessing would hurt the result; host adds Other automatically.`
  ];
  if (segment === "coding") {
    lines.push(
      "Coding Brain keeps Windows/local action tools and action rhythm: to create code, modify a file, run a test, or inspect the machine, use AGENT_ACTION_JSON, then wait for AGENT_ACTION_RESULT before concluding.",
      `For a visual HTML/CSS/JS/React/Vite request, do not write a copy-paste code block: create or modify a real local file with AGENT_ACTION_JSON, verify the accepted result, then announce that the visual preview is opening and emit ${BRAIN_CODING_LIVE_PREVIEW_COMMAND} with the verified absolute path.`
    );
  }
  return lines.join("\n");
}

function brainCodeActLoopContinuationUserText(
  userText: string,
  previousAssistantText: string,
  commands: BrainCodeActCommand[]
): string {
  const commandList = commands.join(" ");
  const visiblePrior = assistantCodeActVisibleText(previousAssistantText);
  return [
    userText || "Continue the current user request.",
    "",
    "BRAIN_CODEACT_LOOP_CONTINUATION v1",
    `codeact_events=${commandList || "none"}`,
    visiblePrior ? `previous_visible_progress=${visiblePrior}` : "",
    agentLoopNarrationContractManifest(),
    "runtime_next=The previous Brain CodeAct was an action event, not a final answer if the requested work is not complete.",
    "control_line=Continue with the next useful CodeAct or exactly one AGENT_ACTION_JSON line when a real local action is needed.",
    "pause_policy=If the previous CodeAct opened UI, waits for user input, or is blocked on permission/confirmation, explain that pause compactly and do not invent an action.",
    "repeat_policy=Do not repeat the same CodeAct only to show activity. Use the next event only when it represents a real step."
  ].filter(Boolean).join("\n");
}

function waitForNativeMapsFirstVisual(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 900);
  });
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function injectAssistantGeoEntityIntoNativeMapsBeforeDisplay(message: TranscriptMessage): Promise<boolean> {
  if (message.role !== "assistant") {
    return false;
  }
  const label = assistantMapsSearchLabelFromText(message.text);
  if (!label) {
    return false;
  }
  for (const retryDelay of [0, 250, 650, 1200, 2000]) {
    if (retryDelay > 0) {
      await delay(retryDelay);
    }
    if (await injectNativeMapsSearchViaLockedLandmark(label)) {
      return true;
    }
  }
  return false;
}

async function executeAssistantModuleCodeActs(
  message: TranscriptMessage,
  moduleId: string,
  parallelSessionIndex: number,
  originalUserText = "",
  session?: SidebarSessionItem
): Promise<TranscriptMessage> {
  let next = await executeAssistantSearchArchiveCodeAct(message, session);
  next = await executeAssistantLocalActionsCodeAct(next);
  if (moduleId === "gmail") {
    return executeAssistantGmailCodeAct(next, parallelSessionIndex);
  }
  next = executeAssistantGoogleWebCodeAct(next, parallelSessionIndex);
  next = await executeAssistantWebSearchCodeAct(next);
  next = await executeAssistantPriorityMcpCodeAct(next);
  next = await executeAssistantScrapersCodeAct(next);
  const shouldOpenAirbnbAfterMaps =
    next.text.includes(BRAIN_MAPS_COMMAND) &&
    next.text.includes(BRAIN_AIRBNB_COMMAND) &&
    !next.text.includes("MAPS_RESULT") &&
    !next.text.includes("AIRBNB_RESULT");
  next = await executeAssistantMapsCodeAct(next, parallelSessionIndex, originalUserText);
  if (shouldOpenAirbnbAfterMaps && next.text.includes("MAPS_RESULT")) {
    await waitForNativeMapsFirstVisual();
  }
  if (moduleId === "airbnb") {
    return executeAssistantAirbnbCodeAct(next, parallelSessionIndex);
  }
  next = executeAssistantGmailCodeAct(next, parallelSessionIndex);
  next = executeAssistantAirbnbCodeAct(next, parallelSessionIndex);
  return next;
}

async function executeAssistantLocalActionsCodeAct(message: TranscriptMessage): Promise<TranscriptMessage> {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractLocalActionsCodeAct(message.text);
  if (!request) {
    return message;
  }
  const result = await executeTrackedAgentAction(localActionsCodeActToAgentActionRequest(request));
  const rendered = renderLocalActionsCodeActResult(request, result, compactAgentActionResult(result));
  return {
    ...message,
    text: `${message.text.trimEnd()}\n\n${rendered}`
  };
}

function executeAssistantDomainBrainCodeActs(message: TranscriptMessage): TranscriptMessage {
  if (message.role !== "assistant") {
    return message;
  }
  const result = extractDomainBrainCodeActResult(message.text);
  if (!result) {
    return message;
  }
  return {
    ...message,
    text: `${message.text.trimEnd()}\n\n${renderDomainBrainCodeActResult(result)}`,
    proofHash: hashJson({
      previousProofHash: message.proofHash,
      domainBrainResult: result
    })
  };
}

function assistantCodeActVisibleText(text: string): string {
  const visibleLines: string[] = [];
  let skippingResult = false;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) {
      skippingResult = false;
      continue;
    }
    if (/^[A-Z][A-Z0-9_]*_RESULT\b/.test(line)) {
      skippingResult = true;
      continue;
    }
    if (skippingResult) {
      continue;
    }
    if (line.startsWith("/")) {
      continue;
    }
    if (/^command\s*=/.test(line)) {
      continue;
    }
    visibleLines.push(line);
  }
  return visibleLines.join("\n").trim();
}

function sessionItem(item: SidebarSessionItem): SidebarSessionItem {
  const archived = item.sessionId !== "" && item.sessionId === sidebarState.archivedSessionId;
  const archivedParallel = item.parallelGroupId !== undefined && item.parallelGroupId === sidebarState.archivedSessionId;
  const selectedPinned = item.sessionId === "native-front-migration";
  return {
    ...item,
    label: selectedPinned ? sidebarState.pinnedSession.label : item.label,
    section: selectedPinned ? sidebarState.pinnedSession.section : item.section,
    workspaceLabel: selectedPinned ? sessionWorkspaceLabel(sidebarState.pinnedSession.section) : item.workspaceLabel ?? sessionWorkspaceLabel(item.section),
    working: selectedPinned ? sidebarState.pinnedSession.working : item.working,
    automated: selectedPinned ? sidebarState.pinnedSession.automated : item.automated,
    archived: archived || archivedParallel,
    rowVisible: item.rowVisible && !archived && !archivedParallel
  };
}

function parallelBundleSessionItem(groupId: string, items: SidebarSessionItem[]): SidebarSessionItem {
  const lanes = [...items].sort((left, right) => (left.parallelLaneIndex ?? 0) - (right.parallelLaneIndex ?? 0));
  const primary = lanes[0] ?? items[0];
  const labels = lanes
    .map((item) => cleanParallelLaneLabel(item.label))
    .filter((label, index, all) => label && all.indexOf(label) === index)
    .join(" / ");
  const latestDate = lanes.map((item) => item.date).sort().at(-1) ?? primary.date;
  return {
    ...primary,
    sessionId: groupId,
    label: labels || "New session",
    date: latestDate,
    working: lanes.some((item) => item.working),
    automated: lanes.some((item) => item.automated),
    archived: lanes.every((item) => item.archived),
    rowVisible: lanes.some((item) => item.rowVisible),
    parallelGroupId: groupId,
    parallelLaneIndex: 0,
    parallelLaneCount: lanes.length,
    parallelPeerSessionIds: lanes.map((item) => item.sessionId)
  };
}

function collapseParallelSessionItems(items: SidebarSessionItem[]): SidebarSessionItem[] {
  const entries: { item: SidebarSessionItem; order: number }[] = [];
  const groups = new Map<string, { items: SidebarSessionItem[]; order: number }>();
  items.forEach((item, index) => {
    if (item.parallelGroupId) {
      const group = groups.get(item.parallelGroupId);
      if (group) {
        group.items.push(item);
      } else {
        groups.set(item.parallelGroupId, { items: [item], order: index });
      }
    } else {
      entries.push({ item, order: index });
    }
  });
  for (const [groupId, group] of groups) {
    entries.push({
      item: group.items.length > 1 ? parallelBundleSessionItem(groupId, group.items) : group.items[0],
      order: group.order
    });
  }
  return entries
    .sort((left, right) => {
      const byDate = right.item.date.localeCompare(left.item.date);
      return byDate || left.order - right.order;
    })
    .map((entry) => entry.item);
}

const ASSISTANT_PATTERN_DEMO_SESSION_ID = "assistant-writing-patterns-gallery-demo";
const SEARCHARCHIVE_LOOP_DEMO_SESSION_ID = "searcharchive-loop-stream-demo";
const SEARCHARCHIVE_LOOP_DEMO_MEMORY_SESSION_ID = "searcharchive-loop-demo-memory";

function searchArchiveLoopDemoWorkspaceLabel(): string {
  return activeSessionWorkspaceLabel("forge");
}

function assistantPatternDemoArchiveSession(): ChatArchiveSession {
  const now = "2026-06-13T03:20:00.000Z";
  const messages: ChatArchiveMessage[] = [
    { turnId: "pattern-demo-user-summary", role: "user", text: "Show me a short summary.", createdAt: now, attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-summary") },
    { turnId: "pattern-demo-assistant-summary", role: "assistant", text: "In short: Kagoshima has mild winters, very humid summers, and autumn is often the most comfortable season.\n\nKeep in mind: heat + humidity shape most recommendations.", createdAt: "2026-06-13T03:20:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-summary") },
    { turnId: "pattern-demo-user-facts", role: "user", text: "Show me key-value pairs.", createdAt: "2026-06-13T03:21:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-facts") },
    { turnId: "pattern-demo-assistant-facts", role: "assistant", text: "Compact sheet:\n\nCity: Kagoshima\nIdeal season: autumn\nHumidity: high in summer\nWatch point: possible typhoons", createdAt: "2026-06-13T03:21:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-facts") },
    { turnId: "pattern-demo-user-steps", role: "user", text: "Show me a procedure.", createdAt: "2026-06-13T03:22:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-steps") },
    { turnId: "pattern-demo-assistant-steps", role: "assistant", text: "Recommended procedure:\n\n1. Identify the dominant pattern.\n2. Check whether the parser already recognizes it.\n3. Render it with the shared visual grammar.\n4. Add a non-regression assertion.", createdAt: "2026-06-13T03:22:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-steps") },
    { turnId: "pattern-demo-user-plan", role: "user", text: "Show me an action plan.", createdAt: "2026-06-13T03:23:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-plan") },
    { turnId: "pattern-demo-assistant-plan", role: "assistant", text: "Action plan:\n\nGoal: harmonize LLM renderings\nStep 1: detect recurring blocks\nStep 2: apply shared visual tokens\nValidation: targeted Markdown test + renderer build\nNext action: open the gallery session and compare the blocks", createdAt: "2026-06-13T03:23:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-plan") },
    { turnId: "pattern-demo-user-pros-cons", role: "user", text: "Show me benefits and limits.", createdAt: "2026-06-13T03:24:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-pros-cons") },
    { turnId: "pattern-demo-assistant-pros-cons", role: "assistant", text: "Benefits:\n\n- Faster scanning\n- Predictable LLM patterns\n- Fewer tiring raw blocks\n\nLimits:\n\n- Some formats are ambiguous\n- The parser must stay conservative\n- Long tables must remain scrollable", createdAt: "2026-06-13T03:24:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-pros-cons") },
    { turnId: "pattern-demo-user-callout", role: "user", text: "Show me notes, warnings, and hypotheses.", createdAt: "2026-06-13T03:25:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-callout") },
    { turnId: "pattern-demo-assistant-callout", role: "assistant", text: "Note: This becomes a discreet information block.\n\nWarning: this line becomes a warning.\n\nHypothesis: the LLM can send an assumption that the front end must distinguish.", createdAt: "2026-06-13T03:25:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-callout") },
    { turnId: "pattern-demo-user-quote", role: "user", text: "Show me a quote/source.", createdAt: "2026-06-13T03:26:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-quote") },
    { turnId: "pattern-demo-assistant-quote", role: "assistant", text: "Quote/source to isolate:\n\n> Rendering should help scan information, not only reproduce raw text.\n> A consecutive quote remains a single block.", createdAt: "2026-06-13T03:26:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-quote") },
    { turnId: "pattern-demo-user-table", role: "user", text: "Show me an answer with a Markdown table.", createdAt: "2026-06-13T03:27:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-table") },
    { turnId: "pattern-demo-assistant-table", role: "assistant", text: "Flattened climate table as an LLM might return it:\n\n| Season | Temperature | Feel | |---|---:|---| | Winter | 5-14 C | Mild | | Spring | 10-24 C | Pleasant | | Summer | 24-33 C | Hot and humid |", createdAt: "2026-06-13T03:27:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-table") },
    { turnId: "pattern-demo-user-code", role: "user", text: "Show me a code fence.", createdAt: "2026-06-13T03:28:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-code") },
    { turnId: "pattern-demo-assistant-code", role: "assistant", text: "Code fence example:\n\n```rust\nfn seasonal_band(month: u8) -> &'static str {\n    match month {\n        12 | 1 | 2 => \"winter\",\n        6..=8 => \"summer\",\n        _ => \"transition\",\n    }\n}\n```", createdAt: "2026-06-13T03:28:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-code") },
    { turnId: "pattern-demo-user-data", role: "user", text: "Show me JSON or YAML.", createdAt: "2026-06-13T03:29:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-data") },
    { turnId: "pattern-demo-assistant-data", role: "assistant", text: "Structured data example:\n\n```json\n{\n  \"city\": \"Kagoshima\",\n  \"season\": \"autumn\",\n  \"risk\": \"typhoon watch\"\n}\n```", createdAt: "2026-06-13T03:29:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-data") },
    { turnId: "pattern-demo-user-command", role: "user", text: "Show me a terminal command.", createdAt: "2026-06-13T03:30:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-command") },
    { turnId: "pattern-demo-assistant-command", role: "assistant", text: "Command to copy:\n\n```powershell\nnpm.cmd run build\nnpx.cmd vitest run tests/llm-multimodal-attachments.test.ts -t \"renders assistant markdown\"\n```", createdAt: "2026-06-13T03:30:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-command") },
    { turnId: "pattern-demo-user-log", role: "user", text: "Show me an error log.", createdAt: "2026-06-13T03:31:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-log") },
    { turnId: "pattern-demo-assistant-log", role: "assistant", text: "Error to diagnose:\n\n```text\nError: table row has 5 cells but header has 3\n  at parseMarkdownTable (PanelsChatBottomSlice.tsx:1488)\n  at AssistantMarkdownText (PanelsChatBottomSlice.tsx:2552)\n```", createdAt: "2026-06-13T03:31:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-log") },
    { turnId: "pattern-demo-user-math", role: "user", text: "Show me calculations and formulas.", createdAt: "2026-06-13T03:32:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-math") },
    { turnId: "pattern-demo-assistant-math", role: "assistant", text: "Quick calculation:\n\nAverage temperature: (24 + 33) / 2 = 28.5 C\nFelt humidity: 28.5 C + high humidity factor => heavier feeling\nSimplified ROI: gain / cost = 420 / 120 = 3.5", createdAt: "2026-06-13T03:32:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-math") },
    { turnId: "pattern-demo-user-decision", role: "user", text: "Show me a recommended decision.", createdAt: "2026-06-13T03:33:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-decision") },
    { turnId: "pattern-demo-assistant-decision", role: "assistant", text: "Recommendation: go in autumn.\n\nOption A: autumn, best comfort/rain balance.\nOption B: spring, pleasant but rising humidity.\nOption C: summer, avoid it if you do not tolerate heat well.\n\nConclusion: I would choose late September or October.", createdAt: "2026-06-13T03:33:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-decision") },
    { turnId: "pattern-demo-user-questions", role: "user", text: "Show me clarification questions.", createdAt: "2026-06-13T03:34:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-questions") },
    { turnId: "pattern-demo-assistant-questions", role: "assistant", text: "Useful questions:\n\n- Do you want a travel-focused or weather-focused answer?\n- Do you prefer monthly or seasonal averages?\n- Do you want to include typhoon risks?", createdAt: "2026-06-13T03:34:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-questions") },
    { turnId: "pattern-demo-user-checklist", role: "user", text: "Show me a checklist.", createdAt: "2026-06-13T03:35:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-checklist") },
    { turnId: "pattern-demo-assistant-checklist", role: "assistant", text: "Preparation checklist:\n\n- [x] Identify the LLM pattern\n- [x] Apply the shared visual identity\n- [ ] Verify the rendering in the demo session", createdAt: "2026-06-13T03:35:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-checklist") },
    { turnId: "pattern-demo-user-divider", role: "user", text: "Show me a separated synthesis.", createdAt: "2026-06-13T03:36:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-divider") },
    { turnId: "pattern-demo-assistant-divider", role: "assistant", text: "Before the synthesis.\n\n---\n\n1. Read the pattern.\n2. Promote it into a component.\n3. Keep the text accessible.", createdAt: "2026-06-13T03:36:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-divider") }
  ];
  return {
    schema: "forge.brain.chat_session_archive.v1",
    sessionId: ASSISTANT_PATTERN_DEMO_SESSION_ID,
    title: "Assistant writing patterns gallery",
    section: "forge",
    workspaceLabel: "Forge",
    date: "2026-06-13",
    createdAt: now,
    updatedAt: "2026-06-13T03:36:10.000Z",
    archived: false,
    messages,
    proofHash: stableSearchArchiveHash(messages.map((message) => message.proofHash))
  };
}

function searchArchiveLoopDemoRequestForSessions(sessions: ChatArchiveSession[]): SearchArchiveRequest {
  const candidates: SearchArchiveRequest[] = [
    { query: "witcher heart stone", keywords: ["witcher", "heart", "hearts", "stone"], sessionScope: "all", contentScope: "all", topK: 10, contextTurns: 2 },
    { query: "wither heart stone", keywords: ["wither", "heart", "hearts", "stone"], sessionScope: "all", contentScope: "all", topK: 10, contextTurns: 2 },
    { query: "heart stone", keywords: ["heart", "hearts", "stone"], sessionScope: "all", contentScope: "all", topK: 10, contextTurns: 2 }
  ];
  for (const candidate of candidates) {
    if (searchArchiveSessions(sessions, candidate).hits.length > 0) {
      return candidate;
    }
  }
  return candidates[0];
}

function searchArchiveLoopDemoSummary(result: SearchArchiveResult): string {
  if (result.hits.length === 0) {
    return "Je n'ai pas retrouve de transcript canonique correspondant. La session peut etre visible dans l'historique, mais elle n'a pas encore de messages archivables dans la source canonique.";
  }
  const sessionTitles = Array.from(new Set(result.hits.map((hit) => hit.sessionTitle))).slice(0, 3);
  const suffix = result.hits.length > 1 ? "morceaux utiles" : "morceau utile";
  return `J'ai retrouve ${result.hits.length} ${suffix}, regroupes par session. Sessions concernees: ${sessionTitles.join(", ")}.`;
}

function searchArchiveLoopRealDemoArchiveSession(): ChatArchiveSession {
  const workspaceLabel = searchArchiveLoopDemoWorkspaceLabel();
  const sourceSessions = Array.from(chatArchiveSessions.values())
    .filter((session) =>
      session.sessionId !== SEARCHARCHIVE_LOOP_DEMO_SESSION_ID &&
      session.sessionId !== SEARCHARCHIVE_LOOP_DEMO_MEMORY_SESSION_ID &&
      session.sessionId !== ASSISTANT_PATTERN_DEMO_SESSION_ID
    );
  const request = searchArchiveLoopDemoRequestForSessions(sourceSessions);
  const result = searchArchiveSessions(sourceSessions, request);
  const queryText = request.query || request.keywords?.join(" ") || "archive";
  const messages: ChatArchiveMessage[] = [
    {
      turnId: "searcharchive-demo-user",
      role: "user",
      text: `Retrouve les passages existants sur ${queryText}.`,
      createdAt: "2026-06-16T08:30:00.000Z",
      attachments: [],
      proofHash: stableSearchArchiveHash("searcharchive-demo-user-real")
    },
    {
      turnId: "searcharchive-demo-assistant-tool",
      role: "assistant",
      text: [
        "Je vais verifier dans l'historique et revenir avec les passages pertinents.",
        "",
        BRAIN_SEARCHARCHIVE_COMMAND,
        "",
        renderSearchArchiveResult(result)
      ].join("\n"),
      createdAt: "2026-06-16T08:30:15.000Z",
      attachments: [],
      proofHash: stableSearchArchiveHash({ demo: "searcharchive-demo-assistant-tool-real", result: result.proofHash })
    },
    {
      turnId: "searcharchive-demo-assistant-final",
      role: "assistant",
      text: searchArchiveLoopDemoSummary(result),
      createdAt: "2026-06-16T08:30:21.000Z",
      attachments: [],
      proofHash: stableSearchArchiveHash({ demo: "searcharchive-demo-assistant-final-real", result: result.proofHash })
    }
  ];
  return {
    schema: "forge.brain.chat_session_archive.v1",
    sessionId: SEARCHARCHIVE_LOOP_DEMO_SESSION_ID,
    title: "Loop stream archive test",
    section: "forge",
    workspaceLabel,
    date: "2026-06-16",
    createdAt: "2026-06-16T08:30:00.000Z",
    updatedAt: "2026-06-16T08:30:21.000Z",
    archived: false,
    messages,
    proofHash: stableSearchArchiveHash(messages.map((message) => message.proofHash))
  };
}

function ensureSearchArchiveLoopDemoSession(): void {
  const workspaceLabel = searchArchiveLoopDemoWorkspaceLabel();
  const removedFakeMemory = chatArchiveSessions.delete(SEARCHARCHIVE_LOOP_DEMO_MEMORY_SESSION_ID);
  if (!chatArchiveSessions.has(SEARCHARCHIVE_LOOP_DEMO_SESSION_ID)) {
    chatArchiveSessions.set(SEARCHARCHIVE_LOOP_DEMO_SESSION_ID, searchArchiveLoopRealDemoArchiveSession());
  } else {
    const demoSession = chatArchiveSessions.get(SEARCHARCHIVE_LOOP_DEMO_SESSION_ID)!;
    const initialDemo = searchArchiveLoopRealDemoArchiveSession();
    demoSession.workspaceLabel = workspaceLabel;
    demoSession.messages = initialDemo.messages;
    demoSession.updatedAt = initialDemo.updatedAt;
    demoSession.proofHash = archiveSessionProofHash(demoSession);
    demoSession.archived = false;
  }
  if (removedFakeMemory) {
    persistChatArchiveSoon();
  }
  const localDemo = localChatSessions.find((session) => session.sessionId === SEARCHARCHIVE_LOOP_DEMO_SESSION_ID);
  if (localDemo) {
    localDemo.workspaceLabel = workspaceLabel;
    localDemo.rowVisible = true;
    localDemo.archived = false;
  } else {
    localChatSessions.unshift({
      sessionId: SEARCHARCHIVE_LOOP_DEMO_SESSION_ID,
      label: "Loop stream archive test",
      date: "2026-06-16",
      section: "forge",
      workspaceLabel,
      rowVisible: true,
      pinned: false,
      working: false,
      automated: false,
      archived: false
    });
  }
}

function ensureAssistantPatternDemoSession(): void {
  if (!chatArchiveSessions.has(ASSISTANT_PATTERN_DEMO_SESSION_ID)) {
    chatArchiveSessions.set(ASSISTANT_PATTERN_DEMO_SESSION_ID, assistantPatternDemoArchiveSession());
  }
  if (!localChatSessions.some((session) => session.sessionId === ASSISTANT_PATTERN_DEMO_SESSION_ID)) {
    localChatSessions.unshift({
      sessionId: ASSISTANT_PATTERN_DEMO_SESSION_ID,
      label: "Assistant writing patterns gallery",
      date: "2026-06-13",
      section: "forge",
      workspaceLabel: "Forge",
      rowVisible: true,
      pinned: false,
      working: false,
      automated: false,
      archived: false
    });
  }
}

function backendSessionItems(): SidebarSessionItem[] {
  ensureAssistantPatternDemoSession();
  ensureBackendSessionArchiveShells();
  ensureSearchArchiveLoopDemoSession();
  if (chatArchiveLoaded) {
    syncLocalChatSessionsFromArchive();
  }
  const dynamicSessionIds = new Set(localChatSessions.map((session) => session.sessionId));
  const sessions = rustBackend().sessions.map((session) => ({
    sessionId: session.sessionId,
    label: session.label,
    date: session.date,
    section: session.section,
    workspaceLabel: sessionWorkspaceLabel(session.section),
    rowVisible: !session.archived,
    pinned: session.pinned,
    working: session.working,
    automated: session.automated,
    archived: session.archived
  })).filter((session) => !dynamicSessionIds.has(session.sessionId));
  const allSessions = [
    ...localChatSessions,
    ...sessions
  ];
  if (allSessions.some((session) => session.sessionId === "test-session-example")) {
    return collapseParallelSessionItems(allSessions);
  }
  const demoSession: SidebarSessionItem = {
    sessionId: "test-session-example",
    label: "test session example",
    date: "2026-06-10",
    section: "forge",
    workspaceLabel: "Forge",
    rowVisible: true,
    pinned: false,
    working: true,
    automated: false,
    archived: false
  };
  return collapseParallelSessionItems([
    allSessions[0],
    demoSession,
    ...allSessions.slice(1)
  ].filter(Boolean));
}

function archivedItems(): SidebarSessionItem[] {
  const sessions = collapseParallelSessionItems(localChatSessions);
  const selected = sessions.find((item) => item.sessionId === sidebarState.archivedSessionId || item.parallelGroupId === sidebarState.archivedSessionId);
  const dynamicArchive = selected
    ? [{ ...selected, rowVisible: true, archived: true, pinned: false }]
    : [];
  return [
    ...dynamicArchive,
    { sessionId: "", label: "Pool invite flow", date: "2026-06-05", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
    { sessionId: "", label: "Web peripheral snapshot", date: "2026-06-04", section: "webexplorer", workspaceLabel: "WebExplorer", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
    { sessionId: "", label: "Forge UI harmonization", date: "2026-06-04", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
    { sessionId: "", label: "Automation queue sketch", date: "2026-06-03", section: "forge", workspaceLabel: "Forge", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
    { sessionId: "", label: "Banger boolean test", date: "2026-06-03", section: "banger", workspaceLabel: "Banger", rowVisible: true, pinned: false, working: false, automated: false, archived: false },
    { sessionId: "", label: "Trading watchlist NATGAS", date: "2026-06-02", section: "trading", workspaceLabel: "Forge Trading", rowVisible: true, pinned: false, working: false, automated: false, archived: false }
  ];
}

function normalizedSidebarRecentSessionId(items: SidebarSessionItem[]): string {
  const recentId = sidebarState.recentSessionId;
  if (!recentId) {
    return "";
  }
  const directItem = items.find((item) => item.sessionId === recentId);
  if (directItem) {
    return directItem.sessionId;
  }
  const laneItem = localChatSessions.find((item) => item.sessionId === recentId && item.parallelGroupId);
  if (laneItem?.parallelGroupId) {
    return laneItem.parallelGroupId;
  }
  const groupItem = items.find((item) => item.parallelGroupId === recentId);
  return groupItem?.sessionId ?? recentId;
}

function restoreParallelGroupToCanvas(groupId: string): boolean {
  const groupItems = parallelGroupItems(groupId);
  if (groupItems.length === 0) {
    return false;
  }
  const primary = groupItems.find((item) => item.parallelLaneIndex === 0) ?? groupItems[0];
  if (!restoreChatSessionToCanvas(primary.sessionId)) {
    clearPanelsChatSessionForId(primary.sessionId);
  }
  parallelChatLanes.clear();
  for (const item of groupItems) {
    const laneIndex = item.parallelLaneIndex ?? 0;
    const archiveSession = chatArchiveSessions.get(item.sessionId);
    const transcript = archiveSession
      ? archiveSession.messages.map((message) => ({
          id: message.turnId,
          role: message.role,
          text: message.text,
          attachments: (message.attachments ?? []).map(publicArchiveAttachmentPreview),
          proofHash: message.proofHash
        }))
      : laneIndex === 0
        ? panelsChatBottomState.transcript
        : [brainBootSystemMessage(item.sessionId)];
    parallelChatLanes.set(laneIndex, { sessionId: item.sessionId, transcript, groupId });
  }
  sidebarState.recentSessionId = groupId;
  return true;
}

function materializeOpenedChatSession(sessionId: string, section: SidebarSessionItem["section"]): SidebarSessionItem {
  const existing = localChatSessions.find((session) => session.sessionId === sessionId);
  if (existing) {
    existing.rowVisible = true;
    existing.archived = false;
    return existing;
  }
  const archiveSession = chatArchiveSessions.get(sessionId);
  const backendSession = backendSessionItems().find((session) => session.sessionId === sessionId);
  const session: SidebarSessionItem = archiveSession
    ? sidebarSessionFromArchive(archiveSession)
    : backendSession
      ? { ...backendSession, rowVisible: true, archived: false }
      : {
          sessionId,
          label: "New session",
          date: todayIsoDate(),
          section,
          workspaceLabel: activeSessionWorkspaceLabel(section),
          rowVisible: true,
          pinned: false,
          working: false,
          automated: false,
          archived: false
        };
  localChatSessions.unshift(session);
  return session;
}

function sidebarSnapshot(): SidebarSnapshot {
  const mode = cutoverMode("sidebar");
  const recentItems = backendSessionItems().map(sessionItem);
  const recentSessionId = normalizedSidebarRecentSessionId(recentItems);
  const snapshot: SidebarSnapshot = {
    schema: "ingen.electron.sidebar.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection,
    profileCanvas: headerState.profileCanvas,
    activeDrawer: sidebarState.activeDrawer,
    profileOpen: sidebarState.profileOpen,
    sessionsMenuMode: sidebarState.sessionsMenuMode,
    recentSessionId,
    hasArchivedSession: sidebarState.archivedSessionId !== "",
    recentItems,
    archivedItems: archivedItems(),
    toolControls: sidebarToolControls(mode),
    profileMenuItems: [
      { id: "llm", label: "LLM providers", detail: "Keys, models and local routing", iconLabel: "AI" },
      { id: "profile", label: "Profile", detail: "Public canvas replica", iconLabel: "Q" }
    ],
    archiveConfirm: sidebarState.archiveConfirm,
    profileCanvasSummary:
      headerState.profileCanvas === "sessions"
        ? `sessions:${sidebarState.sessionsMenuMode}`
        : headerState.profileCanvas === "profile"
          ? "profile-canvas"
          : headerState.profileCanvas === "brain"
            ? "brain-canvas"
            : headerState.profileCanvas === "llm"
              ? "llm-providers"
              : "workspace",
    proofHash: ""
  };
  snapshot.proofHash = hashJson({ ...snapshot, proofHash: "" });
  return snapshot;
}

function sectionTitle(section: CanvasSurfacesCommand["section"] | SidebarSessionItem["section"]): string {
  if (section === "webexplorer") return "RAM DOM Atlas";
  if (section === "banger") return "New object";
  if (section === "trading") return "Market";
  return "Forge";
}

async function applySidebarCommand(command: SidebarCommand): Promise<void> {
  switch (command.kind) {
    case "navigate":
      closeProfileCanvas();
      headerState.activeSection = command.section;
      headerState.sectionTitle = sectionTitle(command.section);
      sidebarState.activeDrawer = "";
      sidebarState.recentSessionId = "";
      break;
    case "open_session":
      closeProfileCanvas();
      await loadChatArchive();
      ensureAssistantPatternDemoSession();
      ensureSearchArchiveLoopDemoSession();
      headerState.activeSection = command.section;
      headerState.sectionTitle = sectionTitle(command.section);
      if (restoreParallelGroupToCanvas(command.sessionId)) {
        sidebarState.recentSessionId = command.sessionId;
        break;
      }
      materializeOpenedChatSession(command.sessionId, command.section);
      sidebarState.recentSessionId = command.sessionId;
      if (!restoreChatSessionToCanvas(command.sessionId)) {
        clearPanelsChatSessionForId(command.sessionId);
      }
      break;
    case "rename_session":
      await loadChatArchive();
      if (renameSidebarSession(command.sessionId, command.label)) {
        sidebarState.lastControl = "rename session";
      }
      break;
    case "open_profile_canvas":
      headerState.profileCanvas = command.canvas;
      sidebarState.profileOpen = false;
      break;
    case "archive_session": {
      const candidate = backendSessionItems().find((item) => item.sessionId === command.sessionId || item.parallelGroupId === command.sessionId);
      sidebarState.archiveConfirm = {
        open: true,
        candidateId: command.sessionId,
        candidateLabel: candidate?.label ?? "New session",
        candidateDate: candidate?.date ?? "2026-06-09",
        candidateSection: candidate?.section ?? "forge"
      };
      break;
    }
    case "activate_control":
      closeProfileCanvas();
      sidebarState.lastControl = command.label;
      break;
    case "switch_sessions_mode":
      sidebarState.sessionsMenuMode = command.mode;
      headerState.profileCanvas = "sessions";
      sidebarState.profileOpen = false;
      break;
    case "toggle_profile_menu":
      headerState.profileCanvas = "";
      sidebarState.profileOpen = !sidebarState.profileOpen;
      break;
    case "set_active_drawer":
      closeProfileCanvas();
      sidebarState.activeDrawer = sidebarState.activeDrawer === command.drawer ? "" : command.drawer;
      break;
    case "hide_tool":
      closeProfileCanvas();
      sidebarState.hiddenTools = [...new Set([...sidebarState.hiddenTools, command.toolId])];
      if (command.toolId === sidebarState.activeDrawer) {
        sidebarState.activeDrawer = "";
      }
      break;
    case "restore_tool":
      closeProfileCanvas();
      sidebarState.hiddenTools = sidebarState.hiddenTools.filter((tool) => tool !== command.toolId);
      break;
    case "pin_session":
      closeProfileCanvas();
      sidebarState.pinnedSession = {
        label: command.label,
        section: command.section,
        working: false,
        automated: false
      };
      sidebarState.lastControl = "pin session";
      break;
    case "confirm_archive":
      sidebarState.archivedSessionId = sidebarState.archiveConfirm.candidateId;
      if (sidebarState.archivedSessionId) {
        for (const item of parallelGroupItems(sidebarState.archivedSessionId)) {
          item.archived = true;
          item.rowVisible = false;
        }
        void loadChatArchive().then(() => {
          const groupItems = parallelGroupItems(sidebarState.archivedSessionId);
          const archivedAt = new Date().toISOString();
          let changed = false;
          if (groupItems.length > 0) {
            for (const item of groupItems) {
              changed = markArchiveSessionArchived(chatArchiveSessions, item.sessionId, true, archivedAt) || changed;
            }
          } else if (markArchiveSessionArchived(chatArchiveSessions, sidebarState.archivedSessionId, true, archivedAt)) {
            changed = true;
          }
          if (changed) {
            persistChatArchiveSoon();
          }
        });
      }
      sidebarState.archiveConfirm = { open: false, candidateId: "", candidateLabel: "", candidateDate: "", candidateSection: "forge" };
      break;
    case "cancel_archive":
      sidebarState.archiveConfirm = { open: false, candidateId: "", candidateLabel: "", candidateDate: "", candidateSection: "forge" };
      break;
  }
}

function sidebarCommandResult(command: SidebarCommand, accepted: boolean, mode: FrontSliceMode): SidebarCommandResult {
  const result: SidebarCommandResult = {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted,
    mode,
    event: accepted ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: ""
  };
  if (!accepted && mode === "shadow") {
    result.error = {
      code: "shadow_only",
      message: "Sidebar slice is locked in explicit shadow rollback mode.",
      proofHash: hashJson({ mode, command })
    };
  }
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function panelsChatBottomSnapshot(): PanelsChatBottomSnapshot {
  const backend = rustBackend();
  const mode = cutoverMode("panels_chat_bottom");
  const title =
    headerState.activeSection === "banger"
      ? "Banger viewport"
      : headerState.activeSection === "trading"
        ? "Market service"
        : headerState.activeSection === "real-estate"
          ? "Real estate service"
          : "Native service";
  const primary =
    headerState.activeSection === "banger"
      ? backend.nativeStatus.banger
      : headerState.activeSection === "trading"
        ? backend.nativeStatus.proof
        : headerState.activeSection === "real-estate"
          ? backend.nativeStatus.proof
          : backend.nativeStatus.brain;
  const selectedProfile = providerProfileFromComposer(panelsChatBottomState.selectedProvider);
  const modelValues = selectedProfile.connected
    ? selectedProfile.models.length > 0 ? selectedProfile.models : ["Model catalog unavailable"]
    : ["Connect provider"];
  const modelLabels = modelValues.map((model) => composerModelDisplayLabel(selectedProfile.connectId, model));
  const reasoningLabels = selectedProfile.connected
    ? selectedProfile.reasoning.length > 0 ? selectedProfile.reasoning : ["Reasoning unavailable"]
    : ["-"];
  panelsChatBottomState.modelIndex = panelsChatBottomState.modelIndex % modelValues.length;
  panelsChatBottomState.reasoningIndex = panelsChatBottomState.reasoningIndex % reasoningLabels.length;
  const nativeAuthority = mode === "electron" ? "rust" : "electron-shadow";
  const snapshot: PanelsChatBottomSnapshot = {
    schema: "ingen.electron.panels_chat_bottom.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode,
    activeSection: headerState.activeSection,
    activeSessionId: panelsChatBottomState.activeSessionId,
    profileCanvas: headerState.profileCanvas,
    rightPanelOpen: headerState.rightPanelOpen,
    statusDock: {
      visible:
        headerState.profileCanvas === "" &&
        headerState.rightPanelOpen &&
        !["shell", "forge", "webexplorer"].includes(headerState.activeSection),
      title,
      primaryAction: "Open native details",
      lines: [
        { label: "PRIMARY", value: primary, source: "NativeStateKernel::projection" },
        {
          label: headerState.activeSection === "banger" ? "GPU" : "PROVIDER",
          value: headerState.activeSection === "banger" ? backend.nativeStatus.banger : backend.nativeStatus.provider,
          source: "NativeServiceSnapshot"
        },
        { label: "JOBS", value: backend.nativeStatus.jobs, source: "NativeStateKernel::projection" },
        { label: "PROOF", value: backend.nativeStatus.proof, source: "NativeStateKernel::projection" }
      ]
    },
    transcript: publicTranscript(panelsChatBottomState.transcript),
    parallelLanes: parallelChatLaneSnapshots(),
    agentSurfaceStatus: `native_state=rust last=${panelsChatBottomState.lastControl} proof=${backend.proofHash.slice(0, 12)}`,
    composer: {
      chatText: panelsChatBottomState.chatText,
      splitPrompts: false,
      permissionMode: panelsChatBottomState.permissionMode,
      permissionModeOpen: panelsChatBottomState.permissionModeOpen,
      selectedProvider: panelsChatBottomState.selectedProvider,
      assistantBusy: Boolean(activeAssistantRunControl && !activeAssistantRunControl.cancelled),
      providers: (Object.values(providerRuntime).map((profile) => ({
        provider: profile.composerProvider,
        label: profile.label,
        connected: profile.connected,
        active: profile.connected && panelsChatBottomState.selectedProvider === profile.composerProvider,
        account: profile.connected ? profile.account : "not linked",
        proof: profile.proof
      })) as PanelsChatBottomSnapshot["composer"]["providers"]),
      modelLabel: modelLabels[panelsChatBottomState.modelIndex] ?? modelLabels[0],
      reasoningLabel: reasoningLabels[panelsChatBottomState.reasoningIndex] ?? reasoningLabels[1],
      uploadStatus: panelsChatBottomState.uploadItems.length > 0 ? `uploads=${panelsChatBottomState.uploadItems.length}` : "uploads=0",
      uploadPreviewLabel: panelsChatBottomState.uploadItems.at(-1)?.name ?? "",
      uploadPreviewKind: panelsChatBottomState.uploadEditTargetId
        ? "IMAGE_EDIT_TARGET"
        : panelsChatBottomState.uploadItems.length > 1
          ? "FILES"
          : panelsChatBottomState.uploadItems.length === 1
            ? "FILE"
            : "",
      uploadCount: panelsChatBottomState.uploadCount,
      uploadErrorText: panelsChatBottomState.uploadErrorText,
      uploadPreviews: panelsChatBottomState.uploadItems.map(publicUploadPreview)
    },
    bottomControls: [
      { id: "permission", label: panelsChatBottomState.permissionMode, kind: "permission_mode_selected", enabled: true, nativeAuthority },
      { id: "model", label: modelLabels[panelsChatBottomState.modelIndex] ?? modelLabels[0], kind: "cycle_llm_model", enabled: true, nativeAuthority },
      { id: "reasoning", label: reasoningLabels[panelsChatBottomState.reasoningIndex] ?? reasoningLabels[1], kind: "cycle_llm_reasoning", enabled: true, nativeAuthority },
      { id: "attach", label: "Attach file", kind: "attach_files", enabled: true, nativeAuthority },
      { id: "send", label: "Send", kind: "send_chat", enabled: true, nativeAuthority }
    ],
    proofHash: backend.proofHash
  };
  snapshot.proofHash = hashJson({ ...snapshot, backendProofHash: backend.proofHash, proofHash: "" });
  return snapshot;
}

function applyHeaderCommand(command: HeaderCommand): void {
  switch (command.kind) {
    case "toggle_left_panel":
      closeProfileCanvas();
      headerState.leftPanelOpen = !headerState.leftPanelOpen;
      break;
    case "toggle_right_panel":
      closeProfileCanvas();
      headerState.rightPanelOpen = !headerState.rightPanelOpen;
      break;
    case "open_sessions_canvas":
      headerState.profileCanvas = headerState.profileCanvas === "sessions" ? "" : "sessions";
      sidebarState.profileOpen = false;
      break;
    case "open_webexplorer":
      closeProfileCanvas();
      headerState.activeSection = "webexplorer";
      headerState.sectionTitle = "RAM DOM Atlas";
      break;
    case "open_banger":
      closeProfileCanvas();
      headerState.activeSection = "banger";
      headerState.sectionTitle = "New object";
      break;
    case "open_trading":
      closeProfileCanvas();
      headerState.activeSection = "trading";
      headerState.sectionTitle = "Market";
      break;
    case "navigate_workspace":
      closeProfileCanvas();
      headerState.activeSection = command.section;
      headerState.sectionTitle = command.section === "webexplorer" ? "RAM DOM Atlas" : "Forge";
      break;
    case "window_minimize":
    case "window_toggle_maximize":
    case "window_close":
      break;
  }
}

function applyWindowHeaderCommand(
  event: Electron.IpcMainInvokeEvent,
  command: HeaderCommand
): boolean {
  switch (command.kind) {
    case "window_minimize":
      return minimizeNativeWindow(event);
    case "window_toggle_maximize":
      return toggleNativeWindowMaximize(event);
    case "window_close":
      return closeNativeWindow(event);
    default:
      return false;
  }
}

function uploadPreviewKindForPath(filePath: string): ComposerUploadPreview["kind"] {
  const extension = extname(filePath).toLowerCase();
  if ([".avif", ".bmp", ".gif", ".ico", ".jpeg", ".jpg", ".png", ".svg", ".webp"].includes(extension)) {
    return "image";
  }
  if ([".m4v", ".mkv", ".mov", ".mp4", ".mpeg", ".mpg", ".ogv", ".webm"].includes(extension)) {
    return "video";
  }
  if ([".3ds", ".3mf", ".dae", ".fbx", ".glb", ".gltf", ".obj", ".ply", ".stl", ".usd", ".usdz"].includes(extension)) {
    return "model3d";
  }
  if (extension === ".pdf") {
    return "pdf";
  }
  if ([".csv", ".ods", ".xls", ".xlsm", ".xlsx"].includes(extension)) {
    return "spreadsheet";
  }
  if (
    [
      ".c",
      ".cpp",
      ".cs",
      ".css",
      ".go",
      ".h",
      ".html",
      ".java",
      ".js",
      ".json",
      ".jsx",
      ".log",
      ".md",
      ".mjs",
      ".php",
      ".py",
      ".python",
      ".rb",
      ".rest",
      ".rs",
      ".rtf",
      ".sh",
      ".sql",
      ".svelte",
      ".toml",
      ".ts",
      ".tsx",
      ".vue",
      ".txt",
      ".xml",
      ".yaml",
      ".yml",
      ".doc",
      ".docx",
      ".odt",
      ".ppt",
      ".pptx"
    ].includes(extension)
  ) {
    return "text";
  }
  return "file";
}

function uploadPreviewMimeType(filePath: string, kind: ComposerUploadPreview["kind"]): string {
  const extension = extname(filePath).toLowerCase();
  if (kind === "chart") {
    return extension === ".json" ? "application/json" : uploadPreviewMimeType(filePath, "spreadsheet");
  }
  if (kind === "image") {
    if (extension === ".svg") return "image/svg+xml";
    if (extension === ".jpg" || extension === ".jpeg") return "image/jpeg";
    return `image/${extension.slice(1) || "png"}`;
  }
  if (kind === "video") {
    if (extension === ".mov") return "video/quicktime";
    if (extension === ".m4v" || extension === ".mp4") return "video/mp4";
    return `video/${extension.slice(1) || "mp4"}`;
  }
  if (kind === "model3d") {
    if (extension === ".glb") return "model/gltf-binary";
    if (extension === ".gltf") return "model/gltf+json";
    if (extension === ".stl") return "model/stl";
    if (extension === ".obj") return "model/obj";
    if (extension === ".3mf") return "model/3mf";
  }
  if (kind === "pdf") return "application/pdf";
  if (kind === "spreadsheet") {
    if (extension === ".csv") return "text/csv; charset=utf-8";
    if (extension === ".xls") return "application/vnd.ms-excel";
    if (extension === ".ods") return "application/vnd.oasis.opendocument.spreadsheet";
    return "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
  }
  if (kind === "text") {
    if (extension === ".doc") return "application/msword";
    if (extension === ".docx") return "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    if (extension === ".odt") return "application/vnd.oasis.opendocument.text";
    if (extension === ".ppt") return "application/vnd.ms-powerpoint";
    if (extension === ".pptx") return "application/vnd.openxmlformats-officedocument.presentationml.presentation";
    if (extension === ".json") return "application/json";
    if (extension === ".md") return "text/markdown; charset=utf-8";
    if (extension === ".html") return "text/html; charset=utf-8";
    if (extension === ".css") return "text/css; charset=utf-8";
    if (extension === ".js" || extension === ".mjs" || extension === ".jsx") return "text/javascript; charset=utf-8";
    if (extension === ".ts" || extension === ".tsx") return "text/typescript; charset=utf-8";
    if (extension === ".xml") return "application/xml";
    if (extension === ".yaml" || extension === ".yml") return "application/yaml";
    return "text/plain; charset=utf-8";
  }
  return "application/octet-stream";
}

async function textPreviewForFile(filePath: string, kind: ComposerUploadPreview["kind"]): Promise<string> {
  if (kind !== "text") {
    return "";
  }
  const extension = extname(filePath).toLowerCase();
  if (extension === ".docx" || extension === ".pptx" || extension === ".odt") {
    return officeTextPreviewForFile(await readFile(filePath), extension);
  }
  if (extension === ".doc" || extension === ".ppt") {
    return "";
  }
  const bytes = await readFilePrefix(filePath, PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES);
  return bytes.toString("utf8");
}

function decodeXmlText(value: string): string {
  return value
    .replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function zipEntries(buffer: Buffer): Map<string, Buffer> {
  const entries = new Map<string, Buffer>();
  let eocdOffset = -1;
  for (let offset = buffer.length - 22; offset >= 0; offset -= 1) {
    if (buffer.readUInt32LE(offset) === 0x06054b50) {
      eocdOffset = offset;
      break;
    }
  }
  if (eocdOffset < 0) {
    return entries;
  }

  const centralDirectoryOffset = buffer.readUInt32LE(eocdOffset + 16);
  const entryCount = buffer.readUInt16LE(eocdOffset + 10);
  let offset = centralDirectoryOffset;
  for (let index = 0; index < entryCount; index += 1) {
    if (buffer.readUInt32LE(offset) !== 0x02014b50) {
      break;
    }
    const compressionMethod = buffer.readUInt16LE(offset + 10);
    const compressedSize = buffer.readUInt32LE(offset + 20);
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const localHeaderOffset = buffer.readUInt32LE(offset + 42);
    const name = buffer.subarray(offset + 46, offset + 46 + nameLength).toString("utf8");
    const localNameLength = buffer.readUInt16LE(localHeaderOffset + 26);
    const localExtraLength = buffer.readUInt16LE(localHeaderOffset + 28);
    const dataOffset = localHeaderOffset + 30 + localNameLength + localExtraLength;
    const compressed = buffer.subarray(dataOffset, dataOffset + compressedSize);
    if (compressionMethod === 0) {
      entries.set(name, compressed);
    } else if (compressionMethod === 8) {
      entries.set(name, inflateRawSync(compressed));
    }
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
}

function xmlTextNodes(xml: string): string {
  const nodes = [...xml.matchAll(/<(?:[A-Za-z0-9_]+:)?t\b[^>]*>([\s\S]*?)<\/(?:[A-Za-z0-9_]+:)?t>/g)]
    .map((match) => decodeXmlText(match[1]).trim())
    .filter(Boolean);
  if (nodes.length > 0) {
    return nodes.join(" ");
  }
  return decodeXmlText(xml.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();
}

function officeTextPreviewForFile(buffer: Buffer, extension: string): string {
  try {
    const entries = zipEntries(buffer);
    let paths: string[] = [];
    if (extension === ".docx") {
      paths = [...entries.keys()]
        .filter((path) => /^word\/(?:document|footnotes|endnotes|comments|header\d+|footer\d+)\.xml$/i.test(path))
        .sort();
    } else if (extension === ".pptx") {
      paths = [...entries.keys()].filter((path) => /^ppt\/slides\/slide\d+\.xml$/i.test(path)).sort();
    } else if (extension === ".odt") {
      paths = entries.has("content.xml") ? ["content.xml"] : [];
    }
    const text = paths
      .map((path) => xmlTextNodes(entries.get(path)?.toString("utf8") ?? ""))
      .filter(Boolean)
      .join("\n\n");
    return trimUtf8Bytes(text, PANELS_CHAT_BOTTOM_MAX_TEXT_PREVIEW_BYTES);
  } catch (error) {
    console.error("Office document text preview failed.", error);
    return "";
  }
}

function parseSharedStrings(xml: string): string[] {
  return [...xml.matchAll(/<si\b[^>]*>([\s\S]*?)<\/si>/g)].map((match) =>
    [...match[1].matchAll(/<t\b[^>]*>([\s\S]*?)<\/t>/g)].map((textMatch) => decodeXmlText(textMatch[1])).join("")
  );
}

function columnIndex(cellRef: string): number {
  const letters = cellRef.replace(/[^A-Z]/gi, "").toUpperCase();
  let value = 0;
  for (const letter of letters) {
    value = value * 26 + letter.charCodeAt(0) - 64;
  }
  return Math.max(0, value - 1);
}

function firstWorksheetPath(entries: Map<string, Buffer>): string {
  const workbookXml = entries.get("xl/workbook.xml")?.toString("utf8") ?? "";
  const relsXml = entries.get("xl/_rels/workbook.xml.rels")?.toString("utf8") ?? "";
  const firstSheetId = workbookXml.match(/<sheet\b[^>]*r:id="([^"]+)"/)?.[1];
  if (firstSheetId) {
    const relMatch = [...relsXml.matchAll(/<Relationship\b[^>]*>/g)]
      .map((match) => match[0])
      .find((tag) => tag.includes(`Id="${firstSheetId}"`));
    const target = relMatch?.match(/Target="([^"]+)"/)?.[1];
    if (target) {
      return target.startsWith("/") ? target.slice(1) : `xl/${target}`;
    }
  }
  return "xl/worksheets/sheet1.xml";
}

function parseXlsxPreview(buffer: Buffer): string[][] {
  const entries = zipEntries(buffer);
  const sharedStrings = parseSharedStrings(entries.get("xl/sharedStrings.xml")?.toString("utf8") ?? "");
  const sheetXml = entries.get(firstWorksheetPath(entries))?.toString("utf8") ?? "";
  const rows: string[][] = [];
  for (const rowMatch of sheetXml.matchAll(/<row\b[^>]*>([\s\S]*?)<\/row>/g)) {
    const row: string[] = [];
    for (const cellMatch of rowMatch[1].matchAll(/<c\b([^>]*)>([\s\S]*?)<\/c>/g)) {
      const attrs = cellMatch[1];
      const body = cellMatch[2];
      const ref = attrs.match(/\br="([^"]+)"/)?.[1] ?? "";
      const type = attrs.match(/\bt="([^"]+)"/)?.[1] ?? "";
      const value = body.match(/<v>([\s\S]*?)<\/v>/)?.[1] ?? body.match(/<t\b[^>]*>([\s\S]*?)<\/t>/)?.[1] ?? "";
      row[columnIndex(ref)] = type === "s" ? sharedStrings[Number(value)] ?? "" : decodeXmlText(value);
    }
    rows.push(row.slice(0, 8).map((cell) => String(cell ?? "")));
    if (rows.length >= 28) {
      break;
    }
  }
  return rows;
}

function parseCsvPreview(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let index = 0; index < text.length && rows.length < 28; index += 1) {
    const char = text[index];
    const next = text[index + 1];
    if (quoted && char === "\"" && next === "\"") {
      cell += "\"";
      index += 1;
    } else if (char === "\"") {
      quoted = !quoted;
    } else if (!quoted && char === ",") {
      row.push(cell);
      cell = "";
    } else if (!quoted && (char === "\n" || char === "\r")) {
      if (char === "\r" && next === "\n") {
        index += 1;
      }
      row.push(cell);
      rows.push(row.slice(0, 8));
      row = [];
      cell = "";
    } else {
      cell += char;
    }
  }
  if (cell || row.length > 0) {
    row.push(cell);
    rows.push(row.slice(0, 8));
  }
  return rows;
}

function ohlcTablePreview(rows: string[][]): string[][] {
  if (rows.length < 2) {
    return [];
  }
  const header = rows[0].map((cell) => cell.toLowerCase().trim());
  const openIndex = header.findIndex((cell) => cell === "open" || cell === "o");
  const highIndex = header.findIndex((cell) => cell === "high" || cell === "h");
  const lowIndex = header.findIndex((cell) => cell === "low" || cell === "l");
  const closeIndex = header.findIndex((cell) => cell === "close" || cell === "c");
  const timeIndex = header.findIndex((cell) => ["time", "date", "datetime", "timestamp"].includes(cell));
  if ([openIndex, highIndex, lowIndex, closeIndex].some((index) => index < 0)) {
    return [];
  }
  return [
    ["time", "open", "high", "low", "close"],
    ...rows.slice(1, 21).map((row, index) => [
      timeIndex >= 0 ? row[timeIndex] ?? `${index + 1}` : `${index + 1}`,
      row[openIndex] ?? "",
      row[highIndex] ?? "",
      row[lowIndex] ?? "",
      row[closeIndex] ?? ""
    ])
  ];
}

function jsonOhlcPreview(text: string): string[][] {
  try {
    const parsed = JSON.parse(text);
    const records: unknown[] = Array.isArray(parsed) ? parsed : Array.isArray(parsed?.data) ? parsed.data : Array.isArray(parsed?.candles) ? parsed.candles : [];
    if (!records.every((record: unknown) => record && typeof record === "object")) {
      return [];
    }
    const rows: string[][] = [];
    records.slice(0, 20).forEach((value: unknown, index: number) => {
      const record = value as Record<string, unknown>;
      rows.push([
        String(record.time ?? record.date ?? record.datetime ?? record.timestamp ?? index + 1),
        String(record.open ?? record.o ?? ""),
        String(record.high ?? record.h ?? ""),
        String(record.low ?? record.l ?? ""),
        String(record.close ?? record.c ?? "")
      ]);
    });
    return rows.some((row: string[]) => row.slice(1).some(Boolean)) ? [["time", "open", "high", "low", "close"], ...rows] : [];
  } catch {
    return [];
  }
}

async function tablePreviewForFile(filePath: string, kind: ComposerUploadPreview["kind"]): Promise<string[][]> {
  if (kind !== "spreadsheet" && kind !== "chart") {
    return [];
  }
  try {
    const bytes = await readFile(filePath);
    if (extname(filePath).toLowerCase() === ".csv") {
      return parseCsvPreview(bytes.subarray(0, 64 * 1024).toString("utf8"));
    }
    return parseXlsxPreview(bytes);
  } catch (error) {
    console.error("Spreadsheet preview failed.", error);
    return [["Spreadsheet preview unavailable"]];
  }
}

async function composerUploadItem(filePath: string): Promise<ComposerUploadItem> {
  let kind = uploadPreviewKindForPath(filePath);
  const textPreview = await textPreviewForFile(filePath, kind);
  let tablePreview = await tablePreviewForFile(filePath, kind);
  const extension = extname(filePath).toLowerCase();
  const chartPreview =
    kind === "spreadsheet"
      ? ohlcTablePreview(tablePreview)
      : extension === ".json"
        ? jsonOhlcPreview(textPreview)
        : [];
  if (chartPreview.length > 0) {
    kind = "chart";
    tablePreview = chartPreview;
  }
  const id = hashJson({ filePath, created: Date.now() }).slice(0, 20);
  return {
    id,
    path: filePath,
    name: basename(filePath),
    kind,
    url: uploadPreviewUrl(id, basename(filePath)),
    mimeType: uploadPreviewMimeType(filePath, kind),
    textPreview,
    tablePreview
  };
}

async function validDroppedFilePaths(filePaths: string[]): Promise<string[]> {
  const normalized = Array.from(new Set(
    filePaths
      .map((filePath) => (typeof filePath === "string" ? filePath.trim() : ""))
      .filter(Boolean)
      .map((filePath) => resolve(filePath))
  ));
  const accepted: string[] = [];
  for (const filePath of normalized) {
    try {
      const metadata = await stat(filePath);
      if (metadata.isFile()) {
        accepted.push(filePath);
      }
    } catch {
      // Drag payloads can contain virtual items from browser download shelves.
      // Only real local files can be attached by the native preview pipeline.
    }
  }
  return accepted;
}

async function attachComposerFilePaths(filePaths: string[]): Promise<{ accepted: boolean; error?: IpcError }> {
  const acceptedPaths = await validDroppedFilePaths(filePaths);
  if (acceptedPaths.length === 0) {
    panelsChatBottomState.uploadErrorText = "DROP_EMPTY: no local file found.";
    return {
      accepted: false,
      error: {
        code: "bad_payload",
        message: panelsChatBottomState.uploadErrorText,
        proofHash: hashJson({ filePaths })
      }
    };
  }

  const existingPaths = new Set(panelsChatBottomState.uploadItems.map((item) => item.path));
  const newPaths = acceptedPaths.filter((filePath) => !existingPaths.has(filePath));
  const nextCount = panelsChatBottomState.uploadItems.length + newPaths.length;
  if (nextCount > PANELS_CHAT_BOTTOM_MAX_UPLOADS) {
    const proofHash = hashJson({
      selected: acceptedPaths.length,
      existing: panelsChatBottomState.uploadItems.length,
      max: PANELS_CHAT_BOTTOM_MAX_UPLOADS
    });
    panelsChatBottomState.uploadErrorText = `UPLOAD_LIMIT: ${PANELS_CHAT_BOTTOM_MAX_UPLOADS} files max.`;
    return {
      accepted: false,
      error: {
        code: "bad_payload",
        message: panelsChatBottomState.uploadErrorText,
        proofHash
      }
    };
  }

  const newItems = await Promise.all(newPaths.map((filePath) => composerUploadItem(filePath)));
  for (const item of newItems) {
    composerUploadPreviewItems.set(item.id, item);
  }
  panelsChatBottomState.uploadItems = [...panelsChatBottomState.uploadItems, ...newItems];
  panelsChatBottomState.uploadCount = panelsChatBottomState.uploadItems.length;
  panelsChatBottomState.uploadEditTargetId = "";
  panelsChatBottomState.uploadErrorText = "";
  return { accepted: true };
}

async function attachComposerFiles(event: Electron.IpcMainInvokeEvent): Promise<{ accepted: boolean; error?: IpcError }> {
  const owner = BrowserWindow.fromWebContents(event.sender);
  const options: Electron.OpenDialogOptions = {
    title: "Attach files",
    buttonLabel: "Upload",
    properties: ["openFile", "multiSelections"],
    filters: [{ name: "All Files", extensions: ["*"] }]
  };
  const result = owner ? await dialog.showOpenDialog(owner, options) : await dialog.showOpenDialog(options);
  if (result.canceled || result.filePaths.length === 0) {
    return { accepted: true };
  }

  return attachComposerFilePaths(result.filePaths);
}

function commandResult(command: HeaderCommand, accepted: boolean, mode: FrontSliceMode): HeaderCommandResult {
  const result: HeaderCommandResult = {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted,
    mode,
    event: accepted ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: ""
  };
  if (!accepted && mode === "shadow") {
    result.error = {
      code: "shadow_only",
      message: "Header slice is locked in explicit shadow rollback mode.",
      proofHash: hashJson({ mode, command })
    };
  }
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

async function submitPanelsChatDraft(
  command: PanelsChatBottomCommand,
  draft: string,
  moduleId: string,
  pendingUploadItems: ComposerUploadItem[]
): Promise<void> {
  updateBrainIdentityContext(command);
  const parallelSessionIndex =
    typeof command.parallelSessionIndex === "number" && Number.isInteger(command.parallelSessionIndex)
      ? command.parallelSessionIndex
      : 0;
  const activeSession = ensureActiveChatSession(draft || "Attached files");
  const internalPrompt = command.internalPrompt === true;
  const replaceAssistantMessageId =
    typeof command.replaceAssistantMessageId === "string" ? command.replaceAssistantMessageId.trim() : "";
  // Audit anchors: const requestSessionId = activeSession.sessionId
  // Audit anchors: const requestTranscriptBeforeSend = [...panelsChatBottomState.transcript]
  await submitChatDraftForSession(
    draft,
    moduleId,
    pendingUploadItems,
    activeSession,
    panelsChatBottomState.transcript,
    parallelSessionIndex,
    internalPrompt,
    replaceAssistantMessageId,
    (nextTranscript) => {
      panelsChatBottomState.transcript = nextTranscript;
      emitPanelsChatBottomSnapshotEvent("transcript_committed", activeSession.sessionId);
    }
  );
}

async function submitChatDraftForSession(
  draft: string,
  moduleId: string,
  pendingUploadItems: ComposerUploadItem[],
  session: SidebarSessionItem,
  transcript: TranscriptMessage[],
  parallelSessionIndex: number,
  internalPrompt: boolean,
  replaceAssistantMessageId: string,
  commitTranscript: (transcript: TranscriptMessage[]) => void
): Promise<void> {
  session.working = true;
  session.date = todayIsoDate();
  const assistantRun = startAssistantRunControl(session.sessionId);
  let latestCommittedTranscript = transcript;
  const trackedCommitTranscript = (nextTranscript: TranscriptMessage[]) => {
    latestCommittedTranscript = nextTranscript;
    commitTranscript(nextTranscript);
  };
  try {
    await submitChatDraftForSessionInner(draft, moduleId, pendingUploadItems, session, transcript, parallelSessionIndex, internalPrompt, replaceAssistantMessageId, assistantRun, trackedCommitTranscript);
  } catch (error) {
    if (!(error instanceof AssistantRunCancelledError)) {
      throw error;
    }
    const cancelledMessage: TranscriptMessage = {
      id: `assistant-cancelled-${Date.now()}`,
      role: "assistant",
      text: "Stopped by user.",
      proofHash: hashJson({ assistantRunCancelled: assistantRun.id, sessionId: assistantRun.sessionId, cancelledAt: assistantRun.cancelledAt ?? new Date().toISOString() })
    };
    latestCommittedTranscript = transcriptWithMessage(latestCommittedTranscript, cancelledMessage);
    commitTranscript(latestCommittedTranscript);
  } finally {
    finishAssistantRunControl(assistantRun);
    session.working = transcriptHasOpenQuestionnaire(latestCommittedTranscript);
    session.date = todayIsoDate();
  }
}

interface UniversalLoopOrchestratorPassResult {
  assistantMessage: TranscriptMessage;
  previousBrainSegment: BrainSegmentId;
  activatedBrainSegment?: ActiveBrainSegmentId;
}

async function executeUniversalLoopOrchestratorPass(params: {
  assistantMessage: TranscriptMessage;
  baseTranscript: TranscriptMessage[];
  originalUserText: string;
  providerAttachments: ProviderAttachment[];
  userMessageId: string;
  moduleId: string;
  parallelSessionIndex: number;
  session: SidebarSessionItem;
  agentEvents: AgentRuntimeEventQueue;
  assistantRun: AssistantRunControl;
  commitTranscript: (transcript: TranscriptMessage[]) => void;
  replaceAssistantMessageId?: string;
}): Promise<UniversalLoopOrchestratorPassResult> {
  let assistantMessage = await executeAssistantAgentActionLoop({
    assistantMessage: params.assistantMessage,
    baseTranscript: params.baseTranscript,
    originalUserText: params.originalUserText,
    providerAttachments: params.providerAttachments,
    userMessageId: params.userMessageId,
    moduleId: params.moduleId,
    agentEvents: params.agentEvents,
    assistantRun: params.assistantRun,
    commitTranscript: params.commitTranscript
  });
  throwIfAssistantRunCancelled(params.assistantRun);
  assistantMessage = applyGeographicTravelAirbnbFallback(assistantMessage, params.originalUserText, params.moduleId);
  assistantMessage = await applyGeographicMapsFallback(assistantMessage, params.originalUserText, params.moduleId, params.parallelSessionIndex);
  throwIfAssistantRunCancelled(params.assistantRun);
  assistantMessage = await executeAssistantModuleCodeActs(assistantMessage, params.moduleId, params.parallelSessionIndex, params.originalUserText, params.session);
  assistantMessage = executeAssistantDomainBrainCodeActs(assistantMessage);
  assistantMessage = executeAssistantRenameSessionCodeAct(assistantMessage, params.session);
  assistantMessage = sanitizeAssistantRenameChatter(assistantMessage);
  assistantMessage = enforceQuestionnaireLoopPause(assistantMessage);
  assistantMessage = suppressRepeatedBrainSegmentCodeAct(assistantMessage, panelsChatBottomState.activeBrainSegment);
  const previousBrainSegment = panelsChatBottomState.activeBrainSegment;
  const activatedBrainSegment = brainSegmentFromAssistantText(assistantMessage.text);
  assistantMessage = executeAssistantBrainSegmentCodeAct(assistantMessage);
  assistantMessage = enforceQuestionnaireLoopPause(assistantMessage);
  if (params.replaceAssistantMessageId) {
    assistantMessage = {
      ...assistantMessage,
      id: params.replaceAssistantMessageId,
      proofHash: hashJson({ replaceAssistantMessageId: params.replaceAssistantMessageId, text: assistantMessage.text, previousProofHash: assistantMessage.proofHash })
    };
  }
  await injectAssistantGeoEntityIntoNativeMapsBeforeDisplay(assistantMessage);
  return {
    assistantMessage,
    previousBrainSegment,
    activatedBrainSegment
  };
}

function universalLoopContinuationUserText(params: {
  originalUserText: string;
  assistantText: string;
  searchArchiveActive: boolean;
  previousBrainSegment: BrainSegmentId;
  activatedBrainSegment?: ActiveBrainSegmentId;
}): { text: string; idSuffix: string } | undefined {
  if (params.searchArchiveActive) {
    return undefined;
  }
  if (params.activatedBrainSegment && params.activatedBrainSegment !== params.previousBrainSegment) {
    return {
      text: brainSegmentContinuationUserText(params.originalUserText, params.activatedBrainSegment),
      idSuffix: params.activatedBrainSegment
    };
  }
  if (
    shouldContinueAfterBrainCodeAct({
      originalUserText: params.originalUserText,
      assistantText: params.assistantText,
      activatedBrainSegment: params.activatedBrainSegment,
      previousBrainSegment: params.previousBrainSegment
    })
  ) {
    return {
      text: brainCodeActLoopContinuationUserText(
        params.originalUserText,
        params.assistantText,
        brainCodeActCommandsFromAssistantText(params.assistantText)
      ),
      idSuffix: "brain-codeact"
    };
  }
  return undefined;
}

function universalLoopContinuationKey(params: {
  continuation: { text: string; idSuffix: string };
  pass: UniversalLoopOrchestratorPassResult;
}): string {
  const codeActs = brainCodeActCommandsFromAssistantText(params.pass.assistantMessage.text).join("|");
  return [
    params.continuation.idSuffix,
    params.pass.activatedBrainSegment ?? "",
    codeActs || "no-codeact"
  ].join(":");
}

async function executeUniversalLoopContinuation(params: {
  nextTranscript: TranscriptMessage[];
  requestSessionId: string;
  originalUserText: string;
  providerAttachments: ProviderAttachment[];
  userMessageId: string;
  moduleId: string;
  parallelSessionIndex: number;
  session: SidebarSessionItem;
  agentEvents: AgentRuntimeEventQueue;
  assistantRun: AssistantRunControl;
  commitTranscript: (transcript: TranscriptMessage[]) => void;
  searchArchiveActive: boolean;
  pass: UniversalLoopOrchestratorPassResult;
}): Promise<TranscriptMessage[]> {
  let nextTranscript = params.nextTranscript;
  let pass = params.pass;
  const seenContinuationKeys = new Set<string>();

  while (true) {
    const continuation = universalLoopContinuationUserText({
      originalUserText: params.originalUserText,
      assistantText: pass.assistantMessage.text,
      searchArchiveActive: params.searchArchiveActive,
      previousBrainSegment: pass.previousBrainSegment,
      activatedBrainSegment: pass.activatedBrainSegment
    });
    if (!continuation) {
      return nextTranscript;
    }
    const continuationKey = universalLoopContinuationKey({ continuation, pass });
    if (seenContinuationKeys.has(continuationKey)) {
      return nextTranscript;
    }
    seenContinuationKeys.add(continuationKey);

    const continuationAssistantMessageId = `assistant-response-${Date.now()}-${continuation.idSuffix}`;
    const continuationLiveSink = createAssistantLiveTextSink({
      baseTranscript: nextTranscript,
      assistantMessageId: continuationAssistantMessageId,
      agentEvents: params.agentEvents,
      commitTranscript: params.commitTranscript,
      assistantRun: params.assistantRun,
      renameSession: params.session
    });
    const rawContinuationMessage = await buildAssistantTranscriptMessage(
      continuation.text,
      params.providerAttachments,
      params.userMessageId,
      params.moduleId,
      nextTranscript,
      continuationLiveSink,
      continuationAssistantMessageId
    );
    const continuationPass = await executeUniversalLoopOrchestratorPass({
      assistantMessage: rawContinuationMessage,
      baseTranscript: nextTranscript,
      originalUserText: continuation.text,
      providerAttachments: params.providerAttachments,
      userMessageId: params.userMessageId,
      moduleId: params.moduleId,
      parallelSessionIndex: params.parallelSessionIndex,
      session: params.session,
      agentEvents: params.agentEvents,
      assistantRun: params.assistantRun,
      commitTranscript: params.commitTranscript
    });
    nextTranscript = await commitAssistantMessageWithProgressiveSeed(
      nextTranscript,
      continuationPass.assistantMessage,
      params.requestSessionId,
      params.commitTranscript
    );
    archiveTranscriptMessage(params.session, continuationPass.assistantMessage);
    pass = continuationPass;
  }
}

async function submitChatDraftForSessionInner(
  draft: string,
  moduleId: string,
  pendingUploadItems: ComposerUploadItem[],
  session: SidebarSessionItem,
  transcript: TranscriptMessage[],
  parallelSessionIndex: number,
  internalPrompt: boolean,
  replaceAssistantMessageId: string,
  assistantRun: AssistantRunControl,
  commitTranscript: (transcript: TranscriptMessage[]) => void
): Promise<void> {
  throwIfAssistantRunCancelled(assistantRun);
  const requestSessionId = session.sessionId;
  const requestTranscriptBeforeSend = transcriptWithoutMessage([...transcript], replaceAssistantMessageId);
  const searchArchiveCodeAct = readSearchArchiveCodeAct(draft);
  const searchArchiveRequest = searchArchiveCodeAct?.kind === "request" ? searchArchiveCodeAct.request : undefined;
  let providerAttachments: ProviderAttachment[] = [];
  const editTargetIdForTurn = panelsChatBottomState.uploadEditTargetId;
  let attachmentPreviews = pendingUploadItems.map(publicUploadPreview);
  let attachmentProofs: unknown = attachmentPreviews.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    kind: attachment.kind,
    proofHash: hashJson(attachment)
  }));
  if (!searchArchiveCodeAct) {
    // Audit anchor: const providerUploadItems = providerUploadItemsForCommand(pendingUploadItems)
    const providerUploadItems = providerUploadItemsForCommand(pendingUploadItems, transcript);
    providerAttachments = await providerAttachmentsFromUploads(providerUploadItems);
    if (editTargetIdForTurn) {
      providerAttachments = providerAttachments.map((attachment) =>
        attachment.id === editTargetIdForTurn ? { ...attachment, editRole: "editable_input" } : attachment
      );
    }
    const currentAttachmentIds = new Set(pendingUploadItems.map((item) => item.id));
    const currentAttachments = providerAttachments.filter((attachment) => currentAttachmentIds.has(attachment.id));
    attachmentPreviews = currentAttachments.map(publicUploadPreview);
    attachmentProofs = attachmentProofSummary(currentAttachments);
  }
  const message: TranscriptMessage = {
    id: `${internalPrompt ? "internal-user" : "user"}-${Date.now()}`,
    role: "user",
    text: draft,
    attachments: attachmentPreviews,
    proofHash: hashJson({ draft, attachments: attachmentProofs })
  };
  const requestTranscriptWithUser = [...requestTranscriptBeforeSend, message];
  let nextTranscript = transcriptWithoutMessage(transcript, replaceAssistantMessageId);
  nextTranscript = internalPrompt ? nextTranscript : transcriptWithMessage(nextTranscript, message);
  if (replaceAssistantMessageId) {
    commitTranscript(nextTranscript);
  }
  if (!internalPrompt) {
    commitTranscript(nextTranscript);
    archiveTranscriptMessage(session, message);
  }
  if (panelsChatBottomState.activeSessionId === requestSessionId) {
    panelsChatBottomState.uploadItems = [];
    panelsChatBottomState.uploadCount = 0;
    panelsChatBottomState.uploadErrorText = "";
    panelsChatBottomState.uploadEditTargetId = "";
  }
  const contextPlan = searchArchiveCodeAct ? undefined : conversationContextPlan(message.id, requestTranscriptWithUser);
  const contextCompactionSeed = `${Date.now()}-${hashJson({ requestSessionId, userMessageId: message.id, estimatedTokens: contextPlan?.estimatedTokens ?? 0 }).slice(0, 8)}`;
  const contextCompactionEvent = contextPlan?.shouldCompact ? contextCompactionEventMessage("compressing", contextCompactionSeed) : undefined;
  const agentEvents = createAgentRuntimeEventQueue({ sessionId: requestSessionId });
  if (contextCompactionEvent) {
    nextTranscript = transcriptWithMessage(nextTranscript, contextCompactionEvent);
    commitTranscript(nextTranscript);
    agentEvents.emit({
      kind: "compaction_started",
      messageId: contextCompactionEvent.id,
      compaction: {
        state: "compressing",
        seed: contextCompactionSeed,
        estimatedTokens: contextPlan?.estimatedTokens
      }
    });
    emitPanelsChatBottomSnapshotEvent("context_compaction_started", requestSessionId);
  }
  let assistantMessage: TranscriptMessage;
  if (searchArchiveCodeAct?.kind === "template") {
    assistantMessage = localSearchArchiveTemplateStatus(searchArchiveCodeAct.result.reason);
  } else if (searchArchiveRequest) {
    assistantMessage = await localSearchArchiveStatus(searchArchiveRequest, session);
  } else {
    const liveAssistantMessageId = replaceAssistantMessageId || `assistant-response-${Date.now()}`;
    const liveTextSink = createAssistantLiveTextSink({
      baseTranscript: nextTranscript,
      assistantMessageId: liveAssistantMessageId,
      agentEvents,
      commitTranscript,
      assistantRun,
      renameSession: session
    });
    // Audit anchor: await buildAssistantTranscriptMessage(draft, providerAttachments, message.id, moduleId)
    assistantMessage = await buildAssistantTranscriptMessage(
      draft,
      providerAttachments,
      message.id,
      moduleId,
      requestTranscriptWithUser,
      liveTextSink,
      liveAssistantMessageId
    );
    throwIfAssistantRunCancelled(assistantRun);
  }
  const orchestratorPass = await executeUniversalLoopOrchestratorPass({
    assistantMessage,
    baseTranscript: nextTranscript,
    originalUserText: draft,
    providerAttachments,
    userMessageId: message.id,
    moduleId,
    parallelSessionIndex,
    session,
    agentEvents,
    assistantRun,
    commitTranscript,
    replaceAssistantMessageId
  });
  assistantMessage = orchestratorPass.assistantMessage;
  nextTranscript = await commitAssistantMessageWithProgressiveSeed(nextTranscript, assistantMessage, requestSessionId, commitTranscript);
  if (contextCompactionEvent) {
    nextTranscript = transcriptWithReplacedMessage(nextTranscript, contextCompactionEventMessage("compressed", contextCompactionSeed));
    commitTranscript(nextTranscript);
    agentEvents.emit({
      kind: "compaction_completed",
      messageId: contextCompactionEvent.id,
      compaction: {
        state: "compressed",
        seed: contextCompactionSeed,
        estimatedTokens: contextPlan?.estimatedTokens
      }
    });
    emitPanelsChatBottomSnapshotEvent("context_compaction_completed", requestSessionId);
  }
  // Audit anchor: archiveTranscriptMessage(activeSession, assistantMessage)
  archiveTranscriptMessage(session, assistantMessage);
  nextTranscript = await executeUniversalLoopContinuation({
    nextTranscript,
    requestSessionId,
    originalUserText: draft,
    providerAttachments,
    userMessageId: message.id,
    moduleId,
    parallelSessionIndex,
    session,
    agentEvents,
    assistantRun,
    commitTranscript,
    searchArchiveActive: Boolean(searchArchiveCodeAct),
    pass: orchestratorPass
  });
}

async function submitParallelPanelsChatDraft(
  command: PanelsChatBottomCommand,
  laneIndex: number,
  draft: string,
  moduleId: string,
  pendingUploadItems: ComposerUploadItem[]
): Promise<void> {
  updateBrainIdentityContext(command);
  const lane = ensureParallelChatLane(laneIndex, draft || "Attached files");
  const session = localChatSessions.find((item) => item.sessionId === lane.sessionId);
  if (!session) {
    return;
  }
  await submitChatDraftForSession(
    draft,
    moduleId,
    pendingUploadItems,
    session,
    lane.transcript,
    laneIndex,
    command.internalPrompt === true,
    typeof command.replaceAssistantMessageId === "string" ? command.replaceAssistantMessageId.trim() : "",
    (nextTranscript) => {
      lane.transcript = nextTranscript;
      emitPanelsChatBottomSnapshotEvent("transcript_committed", lane.sessionId);
    }
  );
}

async function applyPanelsChatBottomCommand(command: PanelsChatBottomCommand): Promise<void> {
  panelsChatBottomState.lastControl = command.kind;
  switch (command.kind) {
    case "new_session":
      resetPanelsChatSessionView();
      panelsChatBottomState.composerResetRequestTime = panelsChatBottomCommandTime(command);
      sidebarState.recentSessionId = "";
      break;
    case "chat_text_edited": {
      if (panelsChatBottomCommandTime(command) <= panelsChatBottomState.composerResetRequestTime) {
        break;
      }
      panelsChatBottomState.chatText = command.value ?? panelsChatBottomState.chatText;
      break;
    }
    case "send_chat": {
      const draft = (command.value ?? panelsChatBottomState.chatText).trim();
      const moduleId = typeof command.moduleId === "string" ? command.moduleId : "";
      const pendingUploadItems = composerUploadItemsForCommand(command);
      panelsChatBottomState.composerResetRequestTime = panelsChatBottomCommandTime(command);
      panelsChatBottomState.chatText = "";
      if (draft || pendingUploadItems.length > 0) {
        const parallelSessionIndex =
          typeof command.parallelSessionIndex === "number" && Number.isInteger(command.parallelSessionIndex)
            ? command.parallelSessionIndex
            : 0;
        if (parallelSessionIndex > 0) {
          await submitParallelPanelsChatDraft(command, parallelSessionIndex, draft, moduleId, pendingUploadItems);
        } else {
          await submitPanelsChatDraft(command, draft, moduleId, pendingUploadItems);
        }
      }
      panelsChatBottomState.chatText = "";
      panelsChatBottomState.uploadEditTargetId = "";
      break;
    }
    case "send_parallel_chat_batch": {
      const moduleId = typeof command.moduleId === "string" ? command.moduleId : "";
      const pendingUploadItems = composerUploadItemsForCommand(command);
      panelsChatBottomState.composerResetRequestTime = panelsChatBottomCommandTime(command);
      panelsChatBottomState.chatText = "";
      const drafts = (command.parallelDrafts ?? [])
        .map((draft) => ({
          parallelSessionIndex: draft.parallelSessionIndex,
          value: draft.value.trim()
        }))
        .filter((draft) =>
          draft.value.length > 0 &&
          Number.isInteger(draft.parallelSessionIndex) &&
          draft.parallelSessionIndex >= 0 &&
          draft.parallelSessionIndex <= 3
        );
      if (drafts.length > 0) {
        await Promise.all(drafts.map((draft) =>
          draft.parallelSessionIndex > 0
            ? submitParallelPanelsChatDraft(command, draft.parallelSessionIndex, draft.value, moduleId, pendingUploadItems)
            : submitPanelsChatDraft({ ...command, value: draft.value, parallelSessionIndex: 0 }, draft.value, moduleId, pendingUploadItems)
        ));
      }
      panelsChatBottomState.chatText = "";
      panelsChatBottomState.uploadEditTargetId = "";
      break;
    }
    case "open_archive_parallel": {
      await loadChatArchive();
      const sessionId = typeof command.sessionId === "string" ? command.sessionId.trim() : "";
      const turnId = typeof command.turnId === "string" ? command.turnId.trim() : "";
      const focusSnippet = typeof command.focusSnippet === "string" ? command.focusSnippet : "";
      const parallelSessionIndex =
        typeof command.parallelSessionIndex === "number" && Number.isInteger(command.parallelSessionIndex)
          ? command.parallelSessionIndex
          : 1;
      if (sessionId) {
        openArchiveSessionInParallel(sessionId, turnId, parallelSessionIndex, focusSnippet);
      }
      break;
    }
    case "close_parallel_lane": {
      const parallelSessionIndex =
        typeof command.parallelSessionIndex === "number" && Number.isInteger(command.parallelSessionIndex)
          ? command.parallelSessionIndex
          : 1;
      closeParallelChatLane(parallelSessionIndex);
      break;
    }
    case "stop_assistant":
      stopActiveAssistantRunControl();
      break;
    case "assistant_write_complete":
      markAssistantWriteComplete(command.value ?? "");
      break;
    case "update_brain_identity":
      updateBrainIdentityContext(command);
      break;
    case "permission_mode_selected":
      if (
        command.value === "ask-permissions" ||
        command.value === "auto-accept-edits" ||
        command.value === "full-autonomy" ||
        command.value === "self-directed"
      ) {
        panelsChatBottomState.permissionMode = command.value;
        panelsChatBottomState.permissionModeOpen = false;
      }
      break;
    case "stage_attachment_for_edit":
      break;
    case "select_llm":
    case "open_llm_providers":
      if (command.provider) {
        panelsChatBottomState.selectedProvider = command.provider;
        panelsChatBottomState.modelIndex = 0;
        panelsChatBottomState.reasoningIndex = Math.min(
          panelsChatBottomState.reasoningIndex,
          Math.max(0, providerProfileFromComposer(command.provider).reasoning.length - 1)
        );
      }
      break;
    case "cycle_llm_model": {
      const profile = providerProfileFromComposer(panelsChatBottomState.selectedProvider);
      panelsChatBottomState.modelIndex = (panelsChatBottomState.modelIndex + 1) % Math.max(1, profile.models.length);
      break;
    }
    case "cycle_llm_reasoning": {
      const profile = providerProfileFromComposer(panelsChatBottomState.selectedProvider);
      panelsChatBottomState.reasoningIndex = (panelsChatBottomState.reasoningIndex + 1) % Math.max(1, profile.reasoning.length);
      break;
    }
    case "upload_preview_scroll":
    case "refresh_probes":
    case "activate_control":
    case "attach_files":
    case "attach_dropped_files":
      break;
  }
}

function panelsChatBottomCommandResult(
  command: PanelsChatBottomCommand,
  accepted: boolean,
  mode: FrontSliceMode,
  error?: IpcError
): PanelsChatBottomCommandResult {
  const result: PanelsChatBottomCommandResult = {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted,
    mode,
    event: accepted ? "electron_command_applied" : mode === "shadow" ? "shadow_manifest_recorded" : "rejected",
    proofHash: ""
  };
  if (error) {
    result.error = error;
  }
  if (!accepted && mode === "shadow") {
    result.error = {
      code: "shadow_only",
      message: "Panels/chat/bottom slice is locked in explicit shadow rollback mode.",
      proofHash: hashJson({ mode, command })
    };
  }
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function installIpc(): void {
  ipcMain.handle("forge:get-hardware-telemetry-snapshot", async (event): Promise<HardwareTelemetrySnapshot> => {
    const snapshot = await hardwareTelemetrySnapshot();
    if (!validateSender(event)) {
      return {
        ...snapshot,
        governor: {
          ...snapshot.governor,
          notes: ["Hardware telemetry rejected by sender validation."]
        },
        proofHash: hashJson({ rejected: "bad_sender", sampledAt: snapshot.sampledAt })
      };
    }
    return snapshot;
  });

  ipcMain.handle("forge:connect-llm-provider", async (event, provider: unknown): Promise<LlmProviderConnectResult> => {
    if (!validateSender(event)) {
      const proofHash = hashJson({ provider, accepted: false, reason: "bad_sender" });
      return {
        provider: "codex",
        accepted: false,
        events: ["rejected bad sender"],
        models: [],
        reasoning: [],
        quotaLabel: "unavailable",
        error: {
          code: "bad_sender",
          message: "LLM provider connection rejected by sender validation.",
          proofHash
        },
        proofHash
      };
    }
    return connectLlmProvider(provider);
  });

  ipcMain.handle("forge:reset-llm-provider", async (event, provider: unknown): Promise<LlmProviderConnectResult> => {
    if (!validateSender(event) || !isLlmProviderConnectId(provider)) {
      const proofHash = hashJson({ provider, accepted: false, reason: "bad_sender" });
      return {
        provider: "codex",
        accepted: false,
        events: ["provider reset rejected", "not ready"],
        models: [],
        reasoning: [],
        quotaLabel: "unavailable",
        error: {
          code: "bad_payload",
          message: "LLM provider reset rejected by sender validation.",
          proofHash
        },
        proofHash
      };
    }
    return resetLlmProviderRuntime(provider);
  });

  ipcMain.handle("forge:get-llm-provider-runtime-snapshot", (event): LlmProviderRuntimeSnapshot => {
    if (!validateSender(event)) {
      return {
        codex: runtimeEventFromProviderProfile(providerRuntime.codex),
        claude: runtimeEventFromProviderProfile(providerRuntime.claude),
        openrouter: runtimeEventFromProviderProfile(providerRuntime.openrouter)
      };
    }
    return llmProviderRuntimeSnapshot();
  });

  ipcMain.handle("forge:search-archive", async (event, request: unknown): Promise<SearchArchiveResult> => {
    if (!validateSender(event) || !isSearchArchiveRequest(request)) {
      return searchArchiveSessions([], {
        query: "",
        scope: "all",
        topK: 0,
        contextTurns: 0
      });
    }
    return searchChatArchive(request);
  });

  ipcMain.handle("forge:get-session-files-snapshot", async (event): Promise<SessionFilesSnapshot> => {
    if (!validateSender(event)) {
      return {
        schema: "ingen.electron.session_files.snapshot.v1",
        groups: [],
        fileCount: 0,
        proofHash: stableSearchArchiveHash([])
      };
    }
    await loadChatArchive();
    return sessionFilesSnapshot();
  });

  ipcMain.handle("forge:get-brain-memory-snapshot", async (event): Promise<BrainCanonicalMemorySnapshot> => {
    if (!validateSender(event)) {
      return blankBrainCanonicalMemorySnapshot();
    }
    return readBrainCanonicalMemorySnapshotFromDisk();
  });

  ipcMain.handle("forge:merge-brain-memory-snapshot", async (event, snapshot: unknown): Promise<BrainCanonicalMemoryResult> => {
    const current = await readBrainCanonicalMemorySnapshotFromDisk();
    if (!validateSender(event)) {
      return {
        accepted: false,
        snapshot: current,
        error: {
          code: "bad_sender",
          message: "Rejected Brain memory snapshot from an untrusted sender.",
          proofHash: stableSearchArchiveHash({ sender: event.senderFrame?.url ?? "" })
        }
      };
    }
    const merged = await mergeBrainCanonicalMemorySnapshotToDisk(snapshot);
    return {
      accepted: true,
      snapshot: merged
    };
  });

  ipcMain.handle("forge:search-city-suggestions", async (event, query: unknown): Promise<CitySuggestionResult> => {
    if (!validateSender(event)) {
      return citySuggestionError("", "City lookup rejected by sender validation.");
    }
    return searchCitySuggestions(query);
  });

  ipcMain.handle("forge:maps-open-geo-entity", async (event, query: unknown): Promise<NativeWebExplorerResult> => {
    if (!validateSender(event)) {
      return nativeMapsResult(false, {
        code: "bad_sender",
        message: "Maps geo entity navigation rejected by sender validation.",
        proofHash: hashJson({ query, sender: event.senderFrame?.url ?? "" })
      });
    }
    return openAssistantGeoEntityMaps(query);
  });

  ipcMain.handle("forge:get-cutover", (event, slice: string): FrontSliceMode => {
    if (
      !validateSender(event) ||
      (slice !== "header" &&
        slice !== "sidebar" &&
        slice !== "panels_chat_bottom" &&
        slice !== "canvas_surfaces" &&
        slice !== "right_panel")
    ) {
      return "electron";
    }
    return cutoverMode(slice);
  });
  ipcMain.handle("forge:get-header-snapshot", async (event): Promise<HeaderSnapshot> => {
    if (!validateSender(event)) {
      return { ...headerSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return headerSnapshot();
  });
  ipcMain.handle("forge:get-header-surface-snapshot", async (event): Promise<HeaderSurfaceSnapshot> => {
    if (!validateSender(event)) {
      return { ...headerSurfaceSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return headerSurfaceSnapshot();
  });
  ipcMain.handle("forge:get-banger-preview-frame", async (event): Promise<RustBangerPreviewFrame> => {
    if (!validateSender(event)) {
      return loadRustBangerPreviewFrame(shellRoot);
    }
    return loadRustBangerPreviewFrame(shellRoot);
  });
  ipcMain.handle("forge:get-banger-present-loop-bootstrap", async (event, request: unknown): Promise<RustBangerPresentLoopBootstrap> => {
    const owner = BrowserWindow.fromWebContents(event.sender);
    const bounds = owner?.getBounds();
    const parentWindowHandle = owner ? nativeWindowHandleDecimal(owner) : undefined;
    const requestedBounds = normalizeBangerViewportRequest(request);
    if (!validateSender(event)) {
      return loadRustBangerPresentLoopBootstrap(shellRoot, {
        parentWindowHandle,
        x: requestedBounds.x,
        y: requestedBounds.y,
        width: requestedBounds.width ?? bounds?.width,
        height: requestedBounds.height ?? bounds?.height,
        sceneKind: requestedBounds.sceneKind
      });
    }
    return loadRustBangerPresentLoopBootstrap(shellRoot, {
      parentWindowHandle,
      x: requestedBounds.x,
      y: requestedBounds.y,
      width: requestedBounds.width ?? bounds?.width,
      height: requestedBounds.height ?? bounds?.height,
      sceneKind: requestedBounds.sceneKind
    });
  });
  ipcMain.handle("forge:get-banger-google-tiles-config", async (event): Promise<BangerGoogleTilesConfigResult> => {
    const schema = "forge.banger.google_photorealistic_tiles_config.v1" as const;
    const initialView = {
      longitude: -122.08409,
      latitude: 37.42207,
      heightMeters: 2600,
      headingDegrees: 0,
      pitchDegrees: -38,
      rollDegrees: 0
    };
    const base = {
      schema,
      source: "cesium-ion-direct" as const,
      accessMode: "cesium-ion" as const,
      provider: "google_photorealistic_3d_tiles" as const,
      rendererModel: "cesium_for_unreal_style_3d_tileset" as const,
      rootTilesetUrl: "",
      nativeStreamer: {
        schema: "forge.banger.native_3d_tiles_streamer.v1" as const,
        authority: "banger_native_engine" as const,
        status: "contract_ready_direct_cesium_required",
        rootIngestionStage: "3d_tiles_root_json_manifest_ingestion",
        traversalStage: "screen_space_error_priority_queue_with_tile_budget",
        contentDecodeStage: "b3dm_glb_gltf_mesh_material_texture_decode",
        georeferenceStage: "wgs84_ecef_to_enu_floating_origin",
        gpuSubmissionStage: "meshlet_or_indexed_mesh_upload_pending",
        visualFallback: "none_direct_cesium_required",
        blocker: "cesium_or_google_direct_credentials_required"
      },
      directRootTilesetEndpoint: "https://tile.googleapis.com/v1/3dtiles/root.json" as const,
      requestBudget: 18,
      rootRequestTtlHours: 3 as const,
      showCreditsOnScreen: true as const,
      attribution: {
        required: true as const,
        mode: "visible_on_screen" as const,
        policy: "google_maps_platform_terms" as const
      },
      georeference: {
        ellipsoid: "WGS84" as const,
        originLatitude: initialView.latitude,
        originLongitude: initialView.longitude,
        originHeightMeters: 0
      },
      lod: {
        policy: "screen_space_error" as const,
        maxScreenSpaceError: 16,
        skipLevelOfDetail: true,
        maxSimultaneousTileLoads: 18
      },
      cache: {
        authority: "banger_tileset_residency_cache" as const,
        maxResidentTileBytes: 512 * 1024 * 1024
      },
      interopFloor: {
        cesiumForUnreal: "1.12+" as const,
        cesiumJs: "1.91+" as const,
        tileset: "OGC_3D_Tiles" as const
      },
      initialView
    };
    if (!validateSender(event)) {
      return {
        ...base,
        accepted: false,
        error: {
          code: "bad_sender",
          message: "Banger Google Tiles config rejected sender.",
          proofHash: hashJson({ schema, rejected: "bad_sender" })
        },
        proofHash: hashJson({ schema, accepted: false, reason: "bad_sender" })
      };
    }
    const cesiumIonAccessToken = (process.env.CESIUM_ACCESS_TOKEN ?? process.env.VITE_CESIUM_ACCESS_TOKEN ?? "").trim();
    const apiKey = (process.env.GOOGLE_MAP_TILES_API_KEY ?? process.env.VITE_GOOGLE_MAP_TILES_API_KEY ?? "").trim();
    const explicitCesiumIonAccessTokenUrl = (
      process.env.FORGE_BANGER_CESIUM_ION_TOKEN_URL ??
      process.env.FORGE_CESIUM_ION_TOKEN_URL ??
      ""
    ).trim();
    const renderTokenBrokerBaseUrl = [
      process.env.FORGE_REAL_ESTATE_BACKEND_URL,
      process.env.FORGE_RENDER_BACKEND_URL,
      "https://forge-6cai.onrender.com"
    ]
      .map((value) => value?.trim().replace(/\/+$/, ""))
      .find((value): value is string => Boolean(value));
    const cesiumIonAccessTokenUrl = explicitCesiumIonAccessTokenUrl ||
      (renderTokenBrokerBaseUrl ? `${renderTokenBrokerBaseUrl}/api/banger/cesium-ion-token` : "");
    if (cesiumIonAccessToken) {
      const result = {
        ...base,
        accepted: true,
        source: "cesium-ion-direct" as const,
        accessMode: "cesium-ion" as const,
        rootTilesetUrl: "ion://google-photorealistic-3d-tiles",
        cesiumIonAccessToken
      };
      return {
        ...result,
        proofHash: hashJson({
          schema,
          accepted: true,
          provider: result.provider,
          rendererModel: result.rendererModel,
          source: result.source,
          accessMode: result.accessMode,
          tokenFingerprint: hashJson(cesiumIonAccessToken).slice(0, 16),
          requestBudget: result.requestBudget,
          lod: result.lod,
          georeference: result.georeference,
          attribution: result.attribution,
          initialView
        })
      };
    }
    if (cesiumIonAccessTokenUrl) {
      const result = {
        ...base,
        accepted: true,
        source: "cesium-ion-token-broker" as const,
        accessMode: "cesium-ion" as const,
        rootTilesetUrl: "ion://google-photorealistic-3d-tiles",
        cesiumIonAccessTokenUrl
      };
      return {
        ...result,
        proofHash: hashJson({
          schema,
          accepted: true,
          provider: result.provider,
          rendererModel: result.rendererModel,
          source: result.source,
          accessMode: result.accessMode,
          tokenEndpointFingerprint: hashJson(cesiumIonAccessTokenUrl).slice(0, 16),
          requestBudget: result.requestBudget,
          lod: result.lod,
          georeference: result.georeference,
          attribution: result.attribution,
          initialView
        })
      };
    }
    if (apiKey) {
      const result = {
        ...base,
        source: "google-map-tiles-direct" as const,
        accessMode: "google-map-tiles-api-key" as const,
        accepted: true,
        rootTilesetUrl: `https://tile.googleapis.com/v1/3dtiles/root.json?key=${encodeURIComponent(apiKey)}`
      };
      return {
        ...result,
        proofHash: hashJson({
          schema,
          accepted: true,
          endpoint: "https://tile.googleapis.com/v1/3dtiles/root.json",
          source: result.source,
          accessMode: result.accessMode,
          keyFingerprint: hashJson(apiKey).slice(0, 16),
          requestBudget: result.requestBudget,
          lod: result.lod,
          georeference: result.georeference,
          attribution: result.attribution,
          initialView
        })
      };
    }
    return {
      ...base,
      accepted: false,
      error: {
        code: "shadow_only",
        message: "Set CESIUM_ACCESS_TOKEN, FORGE_BANGER_CESIUM_ION_TOKEN_URL, or GOOGLE_MAP_TILES_API_KEY.",
        proofHash: hashJson({ schema, configured: false })
      },
      proofHash: hashJson({ schema, accepted: false, configured: false })
    };
  });
  ipcMain.handle("forge:dispatch-header-command", (event, command: unknown): HeaderCommandResult => {
    const mode = cutoverMode();
    if (!validateSender(event) || !isHeaderCommand(command)) {
      const requestId =
        command && typeof command === "object" && "requestId" in command
          ? String((command as { requestId: unknown }).requestId)
          : "rejected";
      return {
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId,
        accepted: false,
        mode,
        event: "rejected",
        error: {
          code: "bad_payload",
          message: "Header command failed IPC validation.",
          proofHash: hashJson(command)
        },
        proofHash: hashJson({ requestId, mode, rejected: true })
      };
    }
    if (applyWindowHeaderCommand(event, command)) {
      return commandResult(command, true, mode);
    }
    if (mode === "electron") {
      applyHeaderCommand(command);
      return commandResult(command, true, mode);
    }
    return commandResult(command, false, mode);
  });
  ipcMain.handle("forge:get-sidebar-snapshot", async (event): Promise<SidebarSnapshot> => {
    if (!validateSender(event)) {
      return { ...sidebarSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return sidebarSnapshot();
  });
  ipcMain.handle("forge:dispatch-sidebar-command", async (event, command: unknown): Promise<SidebarCommandResult> => {
    const mode = cutoverMode("sidebar");
    if (!validateSender(event) || !isSidebarCommand(command)) {
      const requestId =
        command && typeof command === "object" && "requestId" in command
          ? String((command as { requestId: unknown }).requestId)
          : "rejected";
      return {
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId,
        accepted: false,
        mode,
        event: "rejected",
        error: {
          code: "bad_payload",
          message: "Sidebar command failed IPC validation.",
          proofHash: hashJson(command)
        },
        proofHash: hashJson({ requestId, mode, rejected: true })
      };
    }
    if (mode === "electron") {
      await applySidebarCommand(command);
      return sidebarCommandResult(command, true, mode);
    }
    return sidebarCommandResult(command, false, mode);
  });
  ipcMain.handle("forge:get-panels-chat-bottom-snapshot", async (event): Promise<PanelsChatBottomSnapshot> => {
    if (!validateSender(event)) {
      return { ...panelsChatBottomSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return panelsChatBottomSnapshot();
  });
  ipcMain.handle(
    "forge:dispatch-panels-chat-bottom-command",
    async (event, command: unknown): Promise<PanelsChatBottomCommandResult> => {
      const mode = cutoverMode("panels_chat_bottom");
      if (!validateSender(event) || !isPanelsChatBottomCommand(command)) {
        const requestId =
          command && typeof command === "object" && "requestId" in command
            ? String((command as { requestId: unknown }).requestId)
            : "rejected";
        return {
          version: FORGE_ELECTRON_IPC_VERSION,
          requestId,
          accepted: false,
          mode,
          event: "rejected",
          error: {
            code: "bad_payload",
            message: "Panels/chat/bottom command failed IPC validation.",
            proofHash: hashJson(command)
          },
          proofHash: hashJson({ requestId, mode, rejected: true })
        };
      }
      if (mode === "electron") {
        if (command.kind === "attach_files") {
          const attachResult = await attachComposerFiles(event);
          return panelsChatBottomCommandResult(command, attachResult.accepted, mode, attachResult.error);
        }
        if (command.kind === "attach_dropped_files") {
          const attachResult = await attachComposerFilePaths(command.filePaths ?? []);
          return panelsChatBottomCommandResult(command, attachResult.accepted, mode, attachResult.error);
        }
        if (command.kind === "stage_attachment_for_edit") {
          const stageResult = stageAttachmentForImageEdit(command);
          return panelsChatBottomCommandResult(command, stageResult.accepted, mode, stageResult.error);
        }
        await applyPanelsChatBottomCommand(command);
        return panelsChatBottomCommandResult(command, true, mode);
      }
      return panelsChatBottomCommandResult(command, false, mode);
    }
  );
  ipcMain.handle("forge:get-canvas-surfaces-snapshot", async (event): Promise<CanvasSurfacesSnapshot> => {
    if (!validateSender(event)) {
      return { ...canvasSurfacesSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return canvasSurfacesSnapshot();
  });
  ipcMain.handle("forge:dispatch-canvas-surfaces-command", (event, command: unknown): CanvasSurfacesCommandResult => {
    const mode = cutoverMode("canvas_surfaces");
    if (!validateSender(event) || !isCanvasSurfacesCommand(command)) {
      const requestId =
        command && typeof command === "object" && "requestId" in command
          ? String((command as { requestId: unknown }).requestId)
          : "rejected";
      return {
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId,
        accepted: false,
        mode,
        event: "rejected",
        error: {
          code: "bad_payload",
          message: "Canvas surfaces command failed IPC validation.",
          proofHash: hashJson(command)
        },
        proofHash: hashJson({ requestId, mode, rejected: true })
      };
    }
    if (mode === "electron") {
      applyCanvasSurfacesCommand(command);
      return canvasSurfacesCommandResult(command, true, mode);
    }
    return canvasSurfacesCommandResult(command, false, mode);
  });
  ipcMain.handle("forge:get-right-panel-snapshot", async (event): Promise<RightPanelSnapshot> => {
    if (!validateSender(event)) {
      return { ...rightPanelSnapshot(), mode: "electron" };
    }
    void refreshRustBackendProjection(shellRoot);
    return rightPanelSnapshot();
  });
  ipcMain.handle("forge:dispatch-right-panel-command", (event, command: unknown): RightPanelCommandResult => {
    const mode = cutoverMode("right_panel");
    if (!validateSender(event) || !isRightPanelCommand(command)) {
      const requestId =
        command && typeof command === "object" && "requestId" in command
          ? String((command as { requestId: unknown }).requestId)
          : "rejected";
      return {
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId,
        accepted: false,
        mode,
        event: "rejected",
        error: {
          code: "bad_payload",
          message: "Right panel command failed IPC validation.",
          proofHash: hashJson(command)
        },
        proofHash: hashJson({ requestId, mode, rejected: true })
      };
    }
    if (mode === "electron") {
      applyRightPanelCommand(command);
      return rightPanelCommandResult(command, true, mode);
    }
    return rightPanelCommandResult(command, false, mode);
  });
}

function contentType(pathname: string): string {
  switch (extname(pathname)) {
    case ".html":
      return "text/html";
    case ".js":
    case ".mjs":
      return "text/javascript";
    case ".css":
      return "text/css";
    case ".svg":
      return "image/svg+xml";
    case ".png":
      return "image/png";
    case ".ttf":
      return "font/ttf";
    default:
      return "application/octet-stream";
  }
}

function parseRangeHeader(rangeHeader: string | null, size: number): { start: number; end: number } | undefined {
  if (!rangeHeader) {
    return undefined;
  }
  const match = /^bytes=(\d*)-(\d*)$/.exec(rangeHeader.trim());
  if (!match) {
    return undefined;
  }
  const [, rawStart, rawEnd] = match;
  if (!rawStart && !rawEnd) {
    return undefined;
  }
  if (!rawStart) {
    const suffixLength = Number(rawEnd);
    if (!Number.isFinite(suffixLength) || suffixLength <= 0) {
      return undefined;
    }
    return {
      start: Math.max(0, size - suffixLength),
      end: Math.max(0, size - 1)
    };
  }
  const start = Number(rawStart);
  const end = rawEnd ? Number(rawEnd) : size - 1;
  if (!Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end < start || start >= size) {
    return undefined;
  }
  return { start, end: Math.min(end, size - 1) };
}

async function uploadPreviewResponse(request: Request, item: ComposerUploadItem): Promise<Response> {
  const fileStat = await stat(item.path);
  const size = fileStat.size;
  const headers = new Headers({
    "Access-Control-Allow-Origin": "*",
    "Accept-Ranges": "bytes",
    "Cache-Control": "no-store",
    "Content-Type": item.mimeType
  });

  if (size <= 0) {
    headers.set("Content-Length", "0");
    return new Response(null, { status: 204, headers });
  }

  const rangeHeader = request.headers.get("range");
  const range = parseRangeHeader(rangeHeader, size);
  if (rangeHeader && !range) {
    headers.set("Content-Range", `bytes */${size}`);
    return new Response(null, { status: 416, headers });
  }

  const start = range?.start ?? 0;
  const end = range?.end ?? size - 1;
  headers.set("Content-Length", String(end - start + 1));
  if (range) {
    headers.set("Content-Range", `bytes ${start}-${end}/${size}`);
  }

  if (request.method === "HEAD") {
    return new Response(null, { status: range ? 206 : 200, headers });
  }

  const stream = Readable.toWeb(createReadStream(item.path, { start, end })) as ReadableStream<Uint8Array>;
  return new Response(stream, {
    status: range ? 206 : 200,
    headers
  });
}

function installProtocol(): void {
  protocol.handle("ingen", async (request) => {
    const url = new URL(request.url);
    if (url.host === "upload-preview") {
      const id = decodeURIComponent(url.pathname.slice(1).split("/")[0] ?? "");
      const item = panelsChatBottomState.uploadItems.find((candidate) => candidate.id === id) ?? composerUploadPreviewItems.get(id);
      if (!item) {
        return new Response("Upload preview not found.", {
          status: 404,
          headers: { "Content-Type": "text/plain; charset=utf-8" }
        });
      }
      if (!existsSync(item.path)) {
        return new Response("Upload preview source file not found.", {
          status: 404,
          headers: { "Content-Type": "text/plain; charset=utf-8" }
        });
      }
      return uploadPreviewResponse(request, item);
    }

    const relative = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
    const bytes = await readFile(join(rendererDist, relative));
    return new Response(bytes, {
      headers: {
        "Content-Type": contentType(relative)
      }
    });
  });
}

async function createWindow(): Promise<void> {
  const labWindow = eventTextLabMode;
  const window = new BrowserWindow({
    width: labWindow ? 1220 : 1535,
    height: labWindow ? 820 : 786,
    minWidth: labWindow ? 760 : 1180,
    minHeight: labWindow ? 560 : 760,
    frame: labWindow ? true : false,
    thickFrame: labWindow ? true : false,
    resizable: true,
    minimizable: true,
    maximizable: true,
    closable: true,
    focusable: true,
    skipTaskbar: false,
    autoHideMenuBar: labWindow ? true : false,
    title: labWindow ? "InGen Event Text Lab" : "InGen",
    show: false,
    transparent: !labWindow,
    backgroundColor: labWindow ? "#101112" : TRANSPARENT_WINDOW_BACKGROUND,
    webPreferences: {
      preload: join(shellRoot, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      webviewTag: true,
      backgroundThrottling: false,
      offscreen: false
    }
  });
  primaryWindow = window;
  window.setTitle(labWindow ? "InGen Event Text Lab" : "InGen");
  installRendererCpuProfiler(window);

  const showWindow = () => {
    if (window.isDestroyed()) {
      return;
    }
    if (!labWindow) {
      window.maximize();
    }
    window.show();
    window.focus();
  };

  window.once("ready-to-show", () => {
    showWindow();
  });
  window.webContents.once("did-finish-load", () => {
    window.setTitle(labWindow ? "InGen Event Text Lab" : "InGen");
    showWindow();
    emitProviderRuntimeSnapshot();
  });
  window.webContents.once("did-fail-load", () => {
    showWindow();
  });
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith("https://")) {
      void shell.openExternal(url);
    }
    return { action: "deny" };
  });
  window.webContents.on("will-navigate", (event, url) => {
    if (!url.startsWith("ingen://renderer/") && !url.startsWith("ingen://upload-preview/") && !url.startsWith("http://127.0.0.1:")) {
      event.preventDefault();
    }
  });
  (window.webContents as Electron.WebContents & {
    on(event: "did-attach-webview", listener: (event: Electron.Event, webContents: Electron.WebContents, params?: { src?: unknown; partition?: unknown }) => void): Electron.WebContents;
  }).on("did-attach-webview", (_event: Electron.Event, webContents: Electron.WebContents, params?: { src?: unknown; partition?: unknown }) => {
    const attachmentParams = params ?? {};
    const src = typeof attachmentParams.src === "string" ? attachmentParams.src : "";
    const partition = typeof attachmentParams.partition === "string" ? attachmentParams.partition : "";
    if (isMapsWebviewAttachment(src, partition)) {
      rememberMapsDomWebviewGuest(webContents, src);
      console.info("Google Earth DOM/RAM cartography guest attached.", { src, partition });
    }
  });
  window.on("closed", () => {
    finalizeWidgetTaskbarState("window-closed");
    if (primaryWindow === window) {
      primaryWindow = null;
    }
    if (BrowserWindow.getAllWindows().length === 0) {
      app.quit();
    }
  });

  if (process.env.VITE_DEV_SERVER_URL) {
    await window.loadURL(labWindow ? `${process.env.VITE_DEV_SERVER_URL}/event-text-lab.html` : process.env.VITE_DEV_SERVER_URL);
  } else {
    await window.loadURL(labWindow ? "ingen://renderer/event-text-lab.html" : "ingen://renderer/index.html");
  }
  showWindow();
}

function installRendererCpuProfiler(window: BrowserWindow): void {
  const durationMs = Number(process.env.INGEN_RENDERER_PROFILE_MS ?? "0");
  if (!Number.isFinite(durationMs) || durationMs <= 0) {
    return;
  }
  window.webContents.once("did-finish-load", () => {
    const debug = window.webContents.debugger;
    void (async () => {
      try {
        debug.attach("1.3");
        await debug.sendCommand("Profiler.enable");
        await debug.sendCommand("Profiler.start");
        setTimeout(() => {
          void (async () => {
            try {
              const result = await debug.sendCommand("Profiler.stop");
              await writeFile("C:\\tmp\\ingen-renderer-profile.cpuprofile", JSON.stringify(result, null, 2));
              console.info("Renderer CPU profile written to C:\\tmp\\ingen-renderer-profile.cpuprofile");
            } catch (error) {
              console.error("Renderer CPU profiler stop failed.", error);
            } finally {
              if (debug.isAttached()) {
                debug.detach();
              }
            }
          })();
        }, durationMs);
      } catch (error) {
        console.error("Renderer CPU profiler failed.", error);
        if (debug.isAttached()) {
          debug.detach();
        }
      }
    })();
  });
}

function restorePrimaryWindow(): void {
  const window = primaryWindow ?? BrowserWindow.getAllWindows()[0];
  if (!window || window.isDestroyed()) {
    return;
  }
  if (window.isMinimized()) {
    window.restore();
  }
  window.show();
  window.focus();
}

async function warmRustBackendProjection(): Promise<void> {
  await refreshRustBackendProjection(shellRoot);
}

app.whenReady().then(async () => {
  installProtocol();
  installWindowControlIpc();
  installNativeWebExplorerIpc();
  installTerminalIpc();
  installAppMetricsLogger();
  if (!eventTextLabMode) {
    installIpc();
  }
  await Promise.race([
    refreshHardwareProfile(),
    new Promise<void>((resolve) => {
      setTimeout(resolve, 450);
    })
  ]);
  await restoreProviderRuntimeFromDisk();
  await reconcileCanonicalMemoryStore();
  await restoreBrainIdentityContextFromDisk();
  await restoreWorkspaceDirFromDisk();
  if (!eventTextLabMode) {
    await loadChatArchive();
    resetPanelsChatSessionView();
  }
  await createWindow();
  void validatePersistedCodexSession();
  void validatePersistedClaudeSession();
  void validatePersistedOpenRouterSession();
  void refreshHardwareProfile();
  if (!eventTextLabMode) {
    void warmRustBackendProjection().catch((error: unknown) => {
      console.error("Rust backend projection refresh failed after window creation.", error);
    });
  }
});

function installAppMetricsLogger(): void {
  const durationMs = Number(process.env.INGEN_APP_METRICS_MS ?? "0");
  if (!Number.isFinite(durationMs) || durationMs <= 0) {
    return;
  }
  const start = Date.now();
  const rows: unknown[] = [];
  const timer = setInterval(() => {
    rows.push({
      atMs: Date.now() - start,
      metrics: app.getAppMetrics().map((metric) => ({
        pid: metric.pid,
        type: metric.type,
        cpu: metric.cpu,
        memory: metric.memory
      }))
    });
    if (Date.now() - start >= durationMs) {
      clearInterval(timer);
      void writeFile("C:\\tmp\\ingen-app-metrics.json", JSON.stringify(rows, null, 2)).catch((error: unknown) => {
        console.error("Failed to write app metrics.", error);
      });
    }
  }, 1000);
}

app.on("gpu-info-update", () => {
  void refreshHardwareProfile();
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    void createWindow();
    return;
  }
  restorePrimaryWindow();
});

app.on("before-quit", () => {
  finalizeWidgetTaskbarState("before-quit");
  destroyAttachmentSnapshotWindow();
});

app.on("window-all-closed", () => {
  finalizeWidgetTaskbarState("window-all-closed");
  if (process.platform !== "darwin") {
    app.quit();
  }
});

process.once("exit", () => {
  finalizeWidgetTaskbarState("process-exit");
});

process.once("SIGINT", () => {
  finalizeWidgetTaskbarState("sigint");
  process.exit(130);
});

process.once("SIGTERM", () => {
  finalizeWidgetTaskbarState("sigterm");
  process.exit(143);
});

process.once("uncaughtException", (error) => {
  finalizeWidgetTaskbarState("uncaught-exception");
  console.error("Uncaught main process exception.", error);
  process.exit(1);
});
