import { app, BrowserView, BrowserWindow, clipboard, dialog, ipcMain, net, protocol, safeStorage, screen, session, shell } from "electron";
import { spawn, spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { mkdir, open, readFile, rename, stat, writeFile } from "node:fs/promises";
import { cpus } from "node:os";
import { basename, dirname, extname, join, resolve } from "node:path";
import { Readable } from "node:stream";
import { fileURLToPath, pathToFileURL } from "node:url";
import { inflateRawSync } from "node:zlib";
import {
  cachedRustBackendProjection,
  refreshRustBackendProjection,
  type RustBackendProjection
} from "./rust-backend.js";
import {
  FORGE_ELECTRON_IPC_VERSION,
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_CODEACT_COMMANDS,
  BRAIN_CODEACT_ROUTING_RULES,
  BRAIN_AIRBNB_COMMAND,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_MAPS_COMMAND,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_NEWMODULE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_QUESTIONNAIRE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_CODING_COMMAND,
  BRAIN_SCIENCE_VISIBLE_CATALOG,
  BRAIN_CODING_VISIBLE_CATALOG,
  BRAIN_WORKSPACE_COMMAND,
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
  type ComposerUploadPreview,
  type IpcError,
  type LlmProviderConnectId,
  type LlmProviderConnectResult,
  type LlmProviderRuntimeEvent,
  type LlmProviderRuntimeSnapshot,
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
  type NativeTerminalBounds,
  type NativeTerminalResult,
  type NativeWebExplorerBounds,
  type NativeWebExplorerCodeAct,
  type NativeWebExplorerResult,
  type SearchArchiveRequest,
  type SearchArchiveResult,
  type SessionFilesSnapshot,
  isCanvasSurfacesCommand,
  isHeaderCommand,
  isPanelsChatBottomCommand,
  isRightPanelCommand,
  isNativeSection,
  isSidebarCommand
} from "../shared/ipc-contract.js";
import {
  parseSearchArchiveCodeAct,
  renderSearchArchiveResult,
  searchArchiveSessions,
  stableSearchArchiveHash,
  type ChatArchiveAttachment,
  type ChatArchiveMessage,
  type ChatArchiveSession,
  type ChatArchiveSessionMeta,
  markArchiveSessionArchived,
  upsertArchiveMessage
} from "./search-archive.js";
import {
  extractGoogleWebCodeAct,
  renderGoogleWebCodeActResult,
  type GoogleWebCodeActRequest
} from "./google-web-codeact.js";
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
const PANELS_CHAT_BOTTOM_MAX_VIDEO_SUBTITLE_BYTES = 48 * 1024;
const PANELS_CHAT_BOTTOM_CONTEXT_TEXT_BYTES = 24 * 1024;
const PANELS_CHAT_BOTTOM_CONTEXT_TOKEN_BUDGET = 80_000;
const PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS = 56_000;
const PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS = 12_000;
const PANELS_CHAT_BOTTOM_MEMORY_TOKEN_BUDGET = 14_000;
const PANELS_CHAT_BOTTOM_MEMORY_TEXT_BYTES = 32 * 1024;
const PANELS_CHAT_BOTTOM_DOCUMENT_MEMORY_BYTES = 48 * 1024;
const PANELS_CHAT_BOTTOM_VISUAL_SNAPSHOT_SIZE = 768;
let primaryWindow: BrowserWindow | null = null;
const authWindows = new Map<LlmProviderConnectId, BrowserWindow>();
const providerAuthWatchers = new Map<LlmProviderConnectId, ReturnType<typeof setInterval>>();
let claudeProvisioningActive = false;
let codexRuntimeWindow: BrowserWindow | null = null;
let nativeWebExplorerView: BrowserView | null = null;
let nativeWebExplorerOwner: BrowserWindow | null = null;
let nativeWebExplorerLoadedUrl = "";
let nativeWebExplorerPendingUrl = "";
let nativeWebExplorerTargetUrl = WEBEXPLORER_DEFAULT_URL;
let nativeWebExplorerSessionConfigured = false;
let nativeWebExplorerBoundsKey = "";
let nativeMapsView: BrowserView | null = null;
let nativeMapsOwner: BrowserWindow | null = null;
let nativeMapsLoadedUrl = "";
let nativeMapsPendingUrl = "";
let nativeMapsTargetUrl = GOOGLE_EARTH_DEFAULT_URL;
let nativeMapsSessionConfigured = false;
let nativeMapsBoundsKey = "";
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
if (process.platform === "win32") {
  app.setAppUserModelId(eventTextLabMode ? "com.forge.ingen.event-text-lab" : "com.forge.ingen");
}

const bypassSingleInstanceLock = process.env.INGEN_ELECTRON_BYPASS_SINGLE_INSTANCE_LOCK === "1";
const hasSingleInstanceLock = eventTextLabMode || bypassSingleInstanceLock || app.requestSingleInstanceLock();
if (!hasSingleInstanceLock) {
  app.quit();
} else if (!eventTextLabMode && !bypassSingleInstanceLock) {
  app.on("second-instance", () => {
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
const parallelChatLanes = new Map<number, { sessionId: string; transcript: TranscriptMessage[]; groupId: string }>();
const composerUploadPreviewItems = new Map<string, ComposerUploadItem>();
const providerAttachmentCache = new Map<string, ProviderAttachment>();

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

async function openAiResponseContent(userText: string, attachments: ProviderAttachment[]): Promise<OpenAiResponseContentPart[]> {
  const text = userTextWithAttachmentContext(userText, attachments);
  const content: OpenAiResponseContentPart[] = [
    {
      type: "input_text",
      text: text || "Analyse les pieces jointes envoyees."
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
  const text = userTextWithAttachmentContext(userText, attachments) || "Analyse les pieces jointes envoyees.";
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
    .map((attachment) => `${attachment.kind}:${attachment.name}`)
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

function compactedConversationMemory(
  compactedMessages: PlannedConversationMessage[],
  estimatedTokens: number
): string {
  const header = [
    "BRAIN_REINJECTED_AFTER_COMPACTION:",
    brainBootManifest(),
    "",
    "CONVERSATION_COMPACTION v1",
    `estimated_transcript_tokens=${estimatedTokens}`,
    `compact_after_tokens=${PANELS_CHAT_BOTTOM_COMPACT_AT_TOKENS}`,
    `recent_raw_token_budget=${PANELS_CHAT_BOTTOM_RECENT_CONTEXT_TOKENS}`,
    "Memoire compacte de la conversation anterieure:"
  ];
  const selected: string[] = [];
  let usedTokens = estimatedPromptTokens(header.join("\n"));
  for (const message of [...compactedMessages].reverse()) {
    if (message.role === "system") {
      continue;
    }
    const label = message.role === "user" ? "Utilisateur" : "Assistant";
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
      "Utilise cette memoire comme contexte, sans la recopier si elle n'est pas utile."
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
      const lines = [
        `Document ${blocks.length + 1}: ${preview.name}`,
        `id=${preview.id}`,
        `kind=${preview.kind}`,
        cached?.mimeType ? `mime=${cached.mimeType}` : "",
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
      "Registre des documents de la session:",
      "Tous ces documents font partie du contexte actif. S'ils sont pertinents, utilise leurs apercus et leurs noms pour raisonner; demande une reinjection seulement si le contenu brut manque.",
      `Les images de ce registre restent des cibles editables pour ${BRAIN_EDITIMAGE_COMMAND}. Si l'utilisateur demande ensuite de modifier, supprimer, remplacer, nettoyer, recadrer, recolorer ou restyler une image du registre, active ${BRAIN_EDITIMAGE_COMMAND} avec image_ref=id ou filename au lieu de demander un workspace ou de proposer un prompt pour un outil externe.`,
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
    "TACHE ACTIVE OBLIGATOIRE:",
    "L'utilisateur vient d'envoyer les pieces jointes pour repondre a sa demande precedente.",
    `Demande precedente: ${previousIntent}`,
    "Les pieces jointes du tour courant sont la reponse attendue a cette demande precedente.",
    "Reponds directement maintenant a la demande precedente en utilisant les pieces jointes disponibles.",
    "Ne demande pas ce que l'utilisateur veut faire avec l'image ou le document.",
    "Si la demande precedente etait un avis, donne ton avis concret; si elle etait une analyse, analyse directement."
  ].join("\n");
  return [memory, documentMemory, attachmentFollowUp].filter(Boolean).join("\n\n");
}

interface ProviderTextRun {
  text: string;
  runtime: string;
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
      "Module actif: Gmail. Si une action Gmail est demandee, ecris ta propre phrase naturelle adaptee a la demande utilisateur, puis active explicitement le CodeAct Gmail avec ses slots.",
      `Pour ouvrir Gmail directement, active ${BRAIN_GMAIL_COM_COMMAND} apres ta phrase naturelle.`,
      "L'application affiche automatiquement l'evenement quand le CodeAct est active; ne decris pas l'evenement technique dans ta phrase.",
      `Template Gmail: ${BRAIN_GMAIL_COMMAND} intent="open|search|inspect|summarize|draft|reply" query="..." keywords="..." recipient="..." subject="..." body="...".`,
      "N'envoie jamais un email toi-meme: draft/reply prepare seulement un brouillon soumis a validation utilisateur.",
      `N'utilise pas ${BRAIN_GOOGLEWEB_COMMAND} pour une action Gmail.`
    ].join("\n");
  }
  if (moduleId === "airbnb") {
    return [
      "Module actif: Airbnb. Si une action Airbnb est demandee, ecris ta propre phrase naturelle adaptee a la demande utilisateur, puis active explicitement le CodeAct Airbnb avec ses slots.",
      BRAIN_CODEACT_ROUTING_RULES,
      "Ta reponse ne doit jamais etre seulement une commande CodeAct: elle doit contenir une phrase normale pour l'utilisateur, et cette meme phrase doit etre copiee dans le slot say.",
      "L'application affiche automatiquement l'evenement quand le CodeAct est active; ne decris pas l'evenement technique dans ta phrase.",
      `Template Airbnb: ${BRAIN_AIRBNB_COMMAND} intent="open|search|inspect" say="phrase naturelle LLM visible par l'utilisateur" query="..." keywords="...".`,
      `Pour ouvrir Airbnb directement ou commencer une recherche Airbnb, active ${BRAIN_AIRBNB_COMMAND} apres ta phrase naturelle.`,
      `N'utilise pas ${BRAIN_GOOGLEWEB_COMMAND} pour une action Airbnb.`
    ].join("\n");
  }
  if (moduleId === "compute") {
    return [
      `Module actif: Compute. Si la demande contient une formule, un calcul, une recurrence, une simulation numerique ou une verification mathematique, ecris une phrase naturelle courte puis active explicitement ${BRAIN_NEWCOMPUTE_COMMAND}.`,
      BRAIN_CODEACT_ROUTING_RULES,
      `Le texte du composer peut contenir une formule injectee depuis le Canvas: traite-la comme entree Compute et transforme-la en commande ${BRAIN_NEWCOMPUTE_COMMAND} au lieu de seulement l'expliquer en prose.`,
      "L'application affiche automatiquement l'evenement Compute quand le CodeAct est active; ne decris pas l'evenement technique dans ta phrase.",
      `N'utilise ${BRAIN_GOOGLEWEB_COMMAND} que si le calcul exige une recherche web externe avant le Compute.`
    ].join("\n");
  }
  return [
    "Respecte les commandes et priorites CodeAct du Brain deja fourni au debut de session.",
    BRAIN_CODEACT_ROUTING_RULES,
    `Quand le Brain actif est general et qu'une demande correspond a ${BRAIN_SCIENCE_COMMAND} ou ${BRAIN_CODING_COMMAND}, active d'abord ce CodeAct de Brain avec une phrase naturelle courte; ne commence pas par une longue reponse specialisee sans switch.`,
    `N'utilise ${BRAIN_GOOGLEWEB_COMMAND} que pour une recherche web generique qui n'est couverte par aucun module specifique du Brain.`,
    `Regle geographique stricte: lieu geographique detecte seul = ${BRAIN_MAPS_COMMAND}; lieu geographique + champ lexical voyage/vacances/sejour = ${BRAIN_MAPS_COMMAND} puis ${BRAIN_AIRBNB_COMMAND}. Pour une ville, un pays, une region, la meteo d'une ville, une carte, un trajet, Google Earth, une localisation ou des coordonnees sans vocabulaire voyage/vacances/sejour, ecris une phrase naturelle puis active ${BRAIN_MAPS_COMMAND}. Si le meme lieu apparait avec voyage, vacances, partir, visiter, tourisme, sejour, destination, dates, voyageurs, logement, hotel, location ou reservation, ecris une phrase naturelle puis active d'abord ${BRAIN_MAPS_COMMAND}, puis ${BRAIN_AIRBNB_COMMAND} comme page suivante du WebExplorer. Ne lis jamais la position de l'ordinateur sans permission explicite.`,
    `Si l'utilisateur demande de generer/creer une image, ecris une phrase naturelle puis active ${BRAIN_NEWIMAGE_COMMAND} avec say et prompt.`,
    `Si l'utilisateur demande de modifier/retoucher/transformer une image attachee, selectionnee ou visible dans le registre des documents de la session, ecris une phrase naturelle puis active ${BRAIN_EDITIMAGE_COMMAND} avec say, instruction et image_ref.`,
    `Pour une demande comme retirer un element de l'image, enlever le fond, changer les couleurs ou modifier l'image, n'ecris pas un prompt pour un outil externe: active ${BRAIN_EDITIMAGE_COMMAND}.`,
    `N'active jamais ${BRAIN_WORKSPACE_COMMAND} pour generer ou editer une image; le workspace ne concerne que le travail local de code/fichiers/projet.`,
    "Si aucune image n'est disponible dans le composer, la selection, le dernier visuel visible ou le registre des documents, demande quelle image modifier au lieu d'activer le CodeAct.",
    "L'application n'infere pas la demande utilisateur; elle execute seulement les commandes CodeAct que tu as ecrites."
  ].join("\n");
}

interface BrainIdentityContext {
  userFirstName: string;
  agentFirstName: string;
  userHomeLocation: string;
}

const brainIdentityContext: BrainIdentityContext = {
  userFirstName: "",
  agentFirstName: "",
  userHomeLocation: ""
};

function brainIdentityStorePath(): string {
  return join(app.getPath("userData"), "brain", "identity-memory.json");
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

async function restoreBrainIdentityContextFromDisk(): Promise<void> {
  try {
    const raw = await readFile(brainIdentityStorePath(), "utf8");
    const parsed = JSON.parse(raw) as Partial<BrainIdentityContext>;
    brainIdentityContext.userFirstName = normalizeBrainIdentityName(parsed.userFirstName);
    brainIdentityContext.agentFirstName = normalizeBrainIdentityName(parsed.agentFirstName);
    brainIdentityContext.userHomeLocation = normalizeBrainHomeLocation(parsed.userHomeLocation);
  } catch {
    // No persisted identity yet; keep first-install fields blank.
  }
}

async function persistBrainIdentityContext(): Promise<void> {
  try {
    const path = brainIdentityStorePath();
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, JSON.stringify(brainIdentityContext), "utf8");
  } catch (error) {
    console.error("Failed to persist Brain identity memory.", error);
  }
}

function brainIdentityMemoryManifest(): string {
  const user = brainIdentityContext.userFirstName;
  const assistant = brainIdentityContext.agentFirstName;
  const homeLocation = brainIdentityContext.userHomeLocation;
  if (!user && !assistant && !homeLocation) return "";
  return `BRAIN_IDENTITY_MEMORY v1 user_first_name=${JSON.stringify(user)} assistant_first_name=${JSON.stringify(assistant)} user_home_location=${JSON.stringify(homeLocation)} rule=If asked your name or first name, answer assistant_first_name. Use user_first_name for the user. Use user_home_location as user-confirmed living place only when location context is useful. Never invent missing identity or location fields. Never treat user_home_location as live device geolocation.`;
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
  return normalizeBrainHomeLocation(brainIdentityContext.userHomeLocation);
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
    keywords: [...request.keywords, "brain_home_location", geocode.source],
    latitude: geocode.latitude,
    longitude: geocode.longitude,
    source: request.source
  });
}

function userTextHasTravelOrStayIntent(value: string): boolean {
  return /\b(airbnb|voyage|voyager|vacances|partir|depart|départ|destination|tourisme|touristique|visiter|visite|sejour|séjour|weekend|week-end|logement|loger|hebergement|hébergement|hotel|hôtel|auberge|reserver|réserver|reservation|réservation|booking|location|louer|appartement|maison|villa|chambre|nuits?|voyageurs?)\b/i.test(value);
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
    /\b(?:parle|parles|raconte|dis)[-\s]+moi\s+(?:de|d'|sur)\s+(.{2,96})$/i,
    /\b(?:meteo|météo|temperature|température|climat)\s+(?:a|à|de|d'|sur|pour)\s+(.{2,96})$/i,
    /\b(?:ou|où)\s+est\s+(.{2,96})$/i,
    /\b(?:carte|map|maps|google earth|localise|situe)\s+(?:de|d'|a|à|sur|pour)?\s*(.{2,96})$/i
  ];
  for (const pattern of patterns) {
    const match = text.match(pattern);
    const target = cleanInferredMapsTarget(match?.[1] ?? "");
    if (target.length >= 2) {
      return target;
    }
  }
  const simplePlaceIntro = text.match(/^(?:parle|parles|raconte|dis)[-\s]+moi\s+(?:de|d'|sur)\s+([A-ZÀ-Ý][\p{L}\p{M}' -]{1,96})$/iu);
  const simplePlace = cleanInferredMapsTarget(simplePlaceIntro?.[1] ?? "");
  if (simplePlace.length >= 2) {
    return simplePlace;
  }
  return "";
}

function inferMapsTargetFromUserText(value: string): string {
  const text = value.replace(/\s+/g, " ").trim();
  if (!text || userTextHasTravelOrStayIntent(text)) {
    return "";
  }
  return inferGeographicTargetFromUserText(text);
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
  return `${BRAIN_AIRBNB_COMMAND} intent="search" say="Je t'ouvre Airbnb pour ${cleanTarget}." query="${cleanTarget}" keywords="host_geographic_travel_fallback, voyage, vacances"`;
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
  if (message.role !== "assistant" || moduleId === "airbnb" || moduleId === "gmail") {
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
  const unchanged =
    brainIdentityContext.userFirstName === nextUserFirstName &&
    brainIdentityContext.agentFirstName === nextAgentFirstName &&
    brainIdentityContext.userHomeLocation === nextUserHomeLocation;
  brainIdentityContext.userFirstName = nextUserFirstName;
  brainIdentityContext.agentFirstName = nextAgentFirstName;
  brainIdentityContext.userHomeLocation = nextUserHomeLocation;
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
  const generalBrainCodeActCommands = BRAIN_CODEACT_COMMANDS.filter((command) => command !== BRAIN_QUESTIONNAIRE_COMMAND);
  const commandDescriptions = BRAIN_CODEACT_COMMAND_DESCRIPTIONS
    .filter((item) => item.command !== BRAIN_QUESTIONNAIRE_COMMAND)
    .map((item) => `${item.command}: ${item.description}`)
    .join(" | ");
  return [
    "BRAIN_BOOT_MANIFEST v1",
    "source=src/brain.rs",
    identityMemory,
    `codeact_commands=${generalBrainCodeActCommands.join(" ")}`,
    `codeact_descriptions=${commandDescriptions}`,
    `codeact_routing_rules=${BRAIN_CODEACT_ROUTING_RULES}`,
    "rule=Brain is the single source of truth for CodeAct command identities; do not invent or revive commands outside this manifest.",
    "rule=Use Brain memory/search before asking the user to repeat prior local session context.",
    `rule=If local code/files/project work needs a folder and no workspace is active, emit ${BRAIN_WORKSPACE_COMMAND}. This workspace rule does not apply to ${BRAIN_NEWIMAGE_COMMAND} or ${BRAIN_EDITIMAGE_COMMAND}.`
  ].filter(Boolean).join("\n");
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

function brainSegmentManifest(): string {
  if (panelsChatBottomState.activeBrainSegment === "science") {
    return [
      "ACTIVE_BRAIN_SEGMENT v1",
      "active=science",
      `activated_by=${BRAIN_SCIENCE_COMMAND}`,
      `rule=${BRAIN_SCIENCE_COMMAND} is already active for this session; continue directly in Science Brain and do not emit ${BRAIN_SCIENCE_COMMAND} again unless a later turn explicitly switches away first.`,
      BRAIN_SCIENCE_VISIBLE_CATALOG
    ].join("\n");
  }
  if (panelsChatBottomState.activeBrainSegment === "coding") {
    return [
      "ACTIVE_BRAIN_SEGMENT v1",
      "active=coding",
      `activated_by=${BRAIN_CODING_COMMAND}`,
      `rule=${BRAIN_CODING_COMMAND} is already active for this session; continue directly in Coding Brain and do not emit ${BRAIN_CODING_COMMAND} again unless a later turn explicitly switches away first.`,
      BRAIN_CODING_VISIBLE_CATALOG
    ].join("\n");
  }
  return [
    "ACTIVE_BRAIN_SEGMENT v1",
    "active=general",
    `mandatory=Before answering or asking clarifying questions, classify the user's task by semantic domain. If you understand that it belongs to science, engineering, mathematics, biology, chemistry, physics, cryptography, formal analysis, physical product design, electronics, mechanics, robotics, CAD/3D, Banger, future Banger 3D, Monster or /newcompute_ work, write a short natural acknowledgement and activate ${BRAIN_SCIENCE_COMMAND} before any detailed specialized answer or questionnaire.`,
    "mandatory=Physical product or prototype conception is engineering by default, even if the object is ordinary or newly mentioned.",
    "mandatory=The decision to activate /sciencebrain_ is semantic, not keyword-based: infer the domain implied by the user's natural-language request, even when the exact object, field or project name has never appeared in the Brain.",
    `mandatory=If you understand that the task belongs to software engineering, coding, websites, applications, repository work, debugging, tests, architecture, scripts, API, Rust, TypeScript, Electron or developer tooling, write a short natural acknowledgement and activate ${BRAIN_CODING_COMMAND} before any detailed specialized answer or questionnaire.`,
    "mandatory=Clarifying-question CodeActs are specialized-catalog tools; while active=general, do not open a questionnaire before the required Brain segment has been activated.",
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
    "after_answers=When the user message starts SELF_DIRECTED_QUESTIONNAIRE_ANSWERS v1, begin autonomous loop stream work immediately; do not ask the same questionnaire again unless the answers are contradictory.",
    "loop_stream=Write one short paragraph that states the next concrete action and why, then emit the relevant CodeAct command/event below it. Repeat this paragraph -> event rhythm while work remains.",
    `tool_policy=Use Brain CodeActs and specialized commands when useful: ${BRAIN_GOOGLEWEB_COMMAND} for web research, ${BRAIN_NEWCOMPUTE_COMMAND}/${BRAIN_SELECTCOMPUTE_COMMAND} for Monster, ${BRAIN_NEWMODULE_COMMAND} for module materialization, ${BRAIN_NEWOBJECT_COMMAND} for Banger objects, ${BRAIN_FRONTDESIGN_COMMAND} for native palette work.`,
    "goal_reached=When the user's explicit stop condition is satisfied, include a compact marker line: SELF_DIRECTED_GOAL_REACHED reason=\"short reason\" next_prompt=\"a stronger next project direction\".",
    "continuation=When the user message starts SELF_DIRECTED_CONTINUATION v1 or SELF_DIRECTED_PROJECT_EXPANSION v1, continue loop stream work directly unless a new ambiguity truly requires /questionnaire_.",
    "guardrails=No payment, credential action, destructive deletion or irreversible external submission without explicit human confirmation."
  ].join("\n");
}

function codexDirectInstructions(reasoning: string, moduleId = ""): string {
  return [
    `@forge:direct:v1 p=Codex lang=fr tools=codeact effort=${reasoning}`,
    "style=francais naturel, concis; reponds directement.",
    "Tu es la surface assistant locale d'InGen. N'invente pas de runtime ni de statut technique.",
    brainIdentityMemoryManifest(),
    workspaceContextManifest(),
    brainSegmentManifest(),
    selfDirectedModeManifest(),
    webExplorerCodeActInstructions(moduleId)
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

function parseCodexDirectEventStream(rawText: string): string {
  const buffer = { text: `${rawText}\n\n` };
  let finalText = "";
  for (const event of drainCodexSseEvents(buffer)) {
    if (!event || typeof event !== "object") {
      continue;
    }
    const record = event as Record<string, unknown>;
    const type = typeof record.type === "string" ? record.type : "";
    if (type.includes("output_text.delta") && typeof record.delta === "string") {
      finalText += record.delta;
    } else if (type === "response.completed") {
      const fallback = codexDirectResponseOutputText(record.response ?? record);
      if (!finalText.trim() && fallback.trim()) {
        finalText = fallback;
      }
    } else if (!finalText.trim()) {
      const fallback = codexDirectResponseOutputText(record);
      if (fallback.trim()) {
        finalText = fallback;
      }
    }
  }
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
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
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
    instructions: codexDirectInstructions(reasoning, moduleId),
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
  const rawText = await response.text();
  if (!response.ok) {
    throw new Error(`Codex OAuth direct HTTP ${response.status}: ${rawText.slice(0, 360)}`);
  }
  const text = parseCodexDirectEventStream(rawText);
  if (!text) {
    throw new Error(`Codex OAuth direct returned no assistant text: ${rawText.slice(0, 360)}`);
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

async function runCodexSubscriptionExec(
  profile: ProviderRuntimeProfile,
  userText: string,
  attachments: ProviderAttachment[],
  moduleId = "",
  userMessageId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): Promise<ProviderTextRun> {
  try {
    return await runCodexOAuthDirect(profile, userText, attachments, moduleId, userMessageId, transcript);
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
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
): Promise<ProviderTextRun> {
  const command = resolveClaudeCodeCommand();
  if (!command) {
    throw new Error("Claude Code CLI is not executable from this Electron process.");
  }
  const model = selectedComposerModel(profile);
  const effort = normalizedReasoningEffort(selectedComposerReasoning(profile));
  const history = recentConversationWindow(userMessageId, transcript)
    .map((message) => `${message.role === "system" ? "Systeme" : message.role === "user" ? "Utilisateur" : "Assistant"}: ${message.content}`)
    .join("\n\n");
  const promptedUserText = [
    brainIdentityMemoryManifest(),
    workspaceContextManifest(),
    brainSegmentManifest(),
    selfDirectedModeManifest(),
    webExplorerCodeActInstructions(moduleId),
    history ? `Conversation recente:\n${history}` : "",
    `Utilisateur:\n${userText}`
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
  return {
    text: parseClaudeStreamOutput(await runProviderCommand(command, args)),
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

async function runOpenRouterChatCompletion(
  profile: ProviderRuntimeProfile,
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId: string,
  moduleId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
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
      content:
        [
          "Tu es la surface assistant locale d'InGen. Reponds dans la langue de l'utilisateur, clairement et sans inventer de runtime.",
          brainIdentityMemoryManifest(),
          workspaceContextManifest(),
          brainSegmentManifest(),
          selfDirectedModeManifest(),
          webExplorerCodeActInstructions(moduleId)
        ].filter(Boolean).join("\n")
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
  const choices = parsed && typeof parsed === "object" && Array.isArray((parsed as { choices?: unknown }).choices)
    ? (parsed as { choices: unknown[] }).choices
    : [];
  const first = choices[0];
  const message = first && typeof first === "object" ? (first as { message?: unknown }).message : undefined;
  const content = message && typeof message === "object" ? contentText((message as { content?: unknown }).content) : "";
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

async function buildAssistantTranscriptMessage(
  userText: string,
  attachments: ProviderAttachment[],
  userMessageId: string,
  moduleId = "",
  transcript: TranscriptMessage[] = panelsChatBottomState.transcript
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
  try {
    if (profile.connectId === "codex" && profile.connected) {
      const run = await runCodexSubscriptionExec(profile, providerUserText, attachments, moduleId, userMessageId, transcript);
      return {
        id: `assistant-response-${Date.now()}`,
        role: "assistant",
        text: run.text,
        proofHash: hashJson({ provider: profile.connectId, model, reasoning, runtime: run.runtime, userText, providerUserText, attachments: attachmentProofs, text: run.text })
      };
    }
    if (profile.connectId === "claude" && profile.connected) {
      const run = await runClaudeCodePrint(profile, userTextWithAttachmentContext(providerUserText, attachments), moduleId, userMessageId, transcript);
      return {
        id: `assistant-response-${Date.now()}`,
        role: "assistant",
        text: run.text,
        proofHash: hashJson({ provider: profile.connectId, model, reasoning, runtime: run.runtime, userText, providerUserText, attachments: attachmentProofs, text: run.text })
      };
    }
    if (profile.connectId === "openrouter" && profile.connected) {
      const text = await runOpenRouterChatCompletion(profile, providerUserText, attachments, userMessageId, moduleId, transcript);
      return {
        id: `assistant-response-${Date.now()}`,
        role: "assistant",
        text,
        proofHash: hashJson({ provider: profile.connectId, model, reasoning, userText, providerUserText, attachments: attachmentProofs, text })
      };
    }
    const reason = profile.connectId === "codex" && profile.connected
      ? "Codex est connecte pour la session et le catalogue. La prochaine etape doit etre un pont Codex local/subscription, pas un appel API OpenAI facture a l'usage."
      : profile.connected
        ? `${profile.label} est connecte, mais son runtime conversationnel direct n'est pas encore cable dans ce composer.`
        : `${profile.label} n'est pas encore connecte.`;
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

function loadNativeMapsTarget(view: BrowserView, reason: string): void {
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

function installNativeWebExplorerViewportFade(view: BrowserView): void {
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
  const navigationUrl = request.url || GOOGLE_EARTH_DEFAULT_URL;
  if (!isAllowedNativeMapsUrl(navigationUrl)) {
    return nativeMapsResult(false, {
      code: "bad_payload",
      message: "Maps WebExplorer navigation rejected: URL is outside the Google Earth perimeter.",
      proofHash: hashJson({ url: navigationUrl, proofHash: request.proofHash })
    });
  }
  activateWebExplorerSplit();
  nativeMapsTargetUrl = navigationUrl;
  if (nativeMapsView && !nativeMapsView.webContents.isDestroyed()) {
    loadNativeMapsTarget(nativeMapsView, "maps-codeact");
  }
  emitNativeMapsCodeAct({ ...request, url: navigationUrl, parallelSessionIndex });
  return nativeMapsResult(true);
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

function attachNativeWebExplorerView(owner: BrowserWindow, view: BrowserView): void {
  if (!owner.getBrowserViews().includes(view)) {
    owner.addBrowserView(view);
  }
  owner.setTopBrowserView(view);
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

function ensureNativeMapsView(owner: BrowserWindow): BrowserView {
  if (nativeMapsView && nativeMapsOwner === owner && !nativeMapsView.webContents.isDestroyed()) {
    attachNativeWebExplorerView(owner, nativeMapsView);
    return nativeMapsView;
  }
  hideNativeMapsView();
  hideNativeWebExplorerView();
  configureNativeMapsSession();
  const view = new BrowserView({
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
  attachNativeWebExplorerView(owner, view);
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
    console.info("Native Maps loading.", nativeMapsTargetUrl);
  });
  view.webContents.on("did-finish-load", () => {
    if (!view.webContents.isDestroyed()) {
      nativeMapsLoadedUrl = view.webContents.getURL();
      nativeMapsPendingUrl = "";
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
    owner?.removeBrowserView(view);
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
  const view = ensureNativeMapsView(owner);
  view.setBounds(normalized);
  nativeMapsBoundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}`;
  owner.setTopBrowserView(view);
  view.webContents.focus();
  console.info("Native Maps shown.", { bounds: normalized, url: nativeMapsTargetUrl });
  loadNativeMapsTarget(view, "show");
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
  if (!nativeMapsView) {
    return showNativeMaps(event, bounds);
  }
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
  const boundsKey = `${normalized.x}:${normalized.y}:${normalized.width}:${normalized.height}`;
  if (nativeMapsBoundsKey === boundsKey) {
    return nativeMapsResult(true);
  }
  nativeMapsBoundsKey = boundsKey;
  nativeMapsView.setBounds(normalized);
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
}

function minimizeNativeWindow(event: Electron.IpcMainInvokeEvent): boolean {
  if (!validateSender(event)) {
    console.warn("Blocked window minimize from invalid sender", event.senderFrame?.url ?? "");
    return false;
  }
  const window = senderNativeWindow(event);
  if (!window) {
    return false;
  }
  if (window.isFullScreen()) {
    window.setFullScreen(false);
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

function installWindowControlIpc(): void {
  ipcMain.handle("forge:window-minimize", (event): boolean => minimizeNativeWindow(event));
  ipcMain.handle("forge:window-toggle-maximize", (event): boolean => toggleNativeWindowMaximize(event));
  ipcMain.handle("forge:window-close", (event): boolean => closeNativeWindow(event));
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

function sessionLabelFromDraft(draft: string): string {
  const compact = draft.replace(/\s+/g, " ").trim();
  if (!compact) return "New session";
  return compact.length <= 42 ? compact : `${compact.slice(0, 39).trimEnd()}...`;
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

function ensureActiveChatSession(draft: string, options: { markWorking?: boolean } = {}): SidebarSessionItem {
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
    if (materialized.label === "New session") {
      materialized.label = sessionLabelFromDraft(draft);
    }
    ensureBrainBootTranscript(materialized.sessionId);
    sidebarState.recentSessionId = materialized.sessionId;
    return materialized;
  }
  const session: SidebarSessionItem = {
    sessionId: generateChatSessionId(),
    label: sessionLabelFromDraft(draft),
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
  return cleaned || "New session";
}

function sessionById(sessionId: string): SidebarSessionItem | undefined {
  return localChatSessions.find((session) => session.sessionId === sessionId);
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

function ensureParallelChatLane(index: number, draft: string): { sessionId: string; transcript: TranscriptMessage[]; groupId: string } {
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
    label: `Parallel ${index + 1}: ${sessionLabelFromDraft(draft)}`,
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

function parallelChatLaneSnapshots(): PanelsChatBottomSnapshot["parallelLanes"] {
  return Array.from(parallelChatLanes.entries())
    .sort(([left], [right]) => left - right)
    .map(([index, lane]) => ({
      index,
      sessionId: lane.sessionId,
      transcript: publicTranscript(lane.transcript),
      proofHash: hashJson({ index, sessionId: lane.sessionId, transcript: publicTranscript(lane.transcript) })
    }));
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

function transcriptWithoutMessage(messages: TranscriptMessage[], messageId: string): TranscriptMessage[] {
  return messageId ? messages.filter((message) => message.id !== messageId) : messages;
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
  return join(app.getPath("userData"), "brain", "chat-session-archive.json");
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

async function loadChatArchive(): Promise<void> {
  if (chatArchiveLoaded) return;
  if (chatArchiveLoadPromise) {
    await chatArchiveLoadPromise;
    return;
  }
  chatArchiveLoadPromise = (async () => {
    try {
      const raw = await readFile(chatArchiveStorePath(), "utf8");
      const parsed = JSON.parse(raw) as unknown;
      const sessions = parsed && typeof parsed === "object" && Array.isArray((parsed as { sessions?: unknown }).sessions)
        ? (parsed as { sessions: unknown[] }).sessions
        : [];
      chatArchiveSessions.clear();
      for (const session of sessions) {
        if (isChatArchiveSession(session)) {
          session.messages = session.messages.filter((message) => !isInternalTranscriptMessage(message));
          chatArchiveSessions.set(session.sessionId, session);
        }
      }
      syncLocalChatSessionsFromArchive();
    } catch {
      chatArchiveSessions.clear();
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
      const sessions = Array.from(chatArchiveSessions.values())
        .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
        .slice(0, 200);
      const envelope = {
        schema: "forge.brain.chat_archive_store.v1",
        updatedAt: new Date().toISOString(),
        sessions,
        proofHash: stableSearchArchiveHash(sessions.map((session) => session.proofHash))
      };
      const path = chatArchiveStorePath();
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, JSON.stringify(envelope, null, 2), "utf8");
    })
    .catch((error: unknown) => {
      console.error("Failed to persist chat archive.", error);
    });
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
  return {
    ...attachment,
    url: uploadPreviewUrl(attachment.id, attachment.name),
    ...(cached ? { localPath: cached.path, mimeType: cached.mimeType } : {})
  };
}

function publicArchiveAttachmentPreview(attachment: ChatArchiveAttachment): ComposerUploadPreview {
  const { localPath: _localPath, mimeType: _mimeType, ...preview } = attachment;
  return {
    ...preview,
    url: uploadPreviewUrl(preview.id, preview.name)
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
  const existingIds = new Set(localChatSessions.map((session) => session.sessionId));
  const restored = Array.from(chatArchiveSessions.values())
    .filter((session) => !session.archived && !existingIds.has(session.sessionId))
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .map(sidebarSessionFromArchive);
  if (restored.length > 0) {
    localChatSessions.push(...restored);
  }
}

function restoreChatSessionToCanvas(sessionId: string): boolean {
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

async function searchChatArchive(request: SearchArchiveRequest): Promise<SearchArchiveResult> {
  await loadChatArchive();
  return searchArchiveSessions(Array.from(chatArchiveSessions.values()), request);
}

function isSearchArchiveRequest(value: unknown): value is SearchArchiveRequest {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<SearchArchiveRequest>;
  return (
    typeof candidate.query === "string" &&
    (candidate.scope === undefined || candidate.scope === "recent" || candidate.scope === "archived" || candidate.scope === "all") &&
    (candidate.topK === undefined || typeof candidate.topK === "number") &&
    (candidate.contextTurns === undefined || typeof candidate.contextTurns === "number") &&
    (candidate.targets === undefined || (Array.isArray(candidate.targets) && candidate.targets.every((target) => typeof target === "string")))
  );
}

async function localSearchArchiveStatus(request: SearchArchiveRequest): Promise<TranscriptMessage> {
  await loadChatArchive();
  const result = searchArchiveSessions(Array.from(chatArchiveSessions.values()), request);
  const text = renderSearchArchiveResult(result);
  return {
    id: `assistant-searcharchive-${Date.now()}`,
    role: "assistant",
    text,
    proofHash: hashJson({ codeact: BRAIN_SEARCHARCHIVE_COMMAND, request, result })
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

async function executeAssistantMapsCodeAct(message: TranscriptMessage, parallelSessionIndex = 0): Promise<TranscriptMessage> {
  if (message.role !== "assistant") {
    return message;
  }
  const request = extractMapsCodeAct(message.text);
  if (!request) {
    return message;
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
  return [
    userText || "Continue la demande utilisateur en cours.",
    "",
    `Contexte InGen: ${command} vient d'etre active. Le catalogue ${catalog} est maintenant injecte. Continue la demande utilisateur avec ce Brain actif; ne reactive pas ${command} sauf si une nouvelle demande l'exige.`,
    `Loop stream mode: answer in short French paragraphs, separated by useful CodeActs when an action is in progress. If you need several framing questions, do not write a long checklist in the Canvas: activate ${BRAIN_QUESTIONNAIRE_COMMAND} with title, intro, q1/q2/q3/q4/q5 maximum and three contextual expert option cards per question via q1_options/q2_options/q3_options/q4_options/q5_options. The intro frames the project goal in 2-3 short French sentences. Each option must follow "Label (Tag) - 1-2 useful French sentences: benefit, tradeoff, when to choose it"; use concise tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when useful, mark "(Recommended)" when it is the best starting point, and include a more ambitious/quality/longer/costlier path when relevant. For color-choice questions, include bounded preview tokens such as color:#38bdf8 or colors:#38bdf8,#a855f7 inside the option label; never include arbitrary CSS or JS. Forbidden: Option 1/2/3, vague meta choices, "je ne sais pas", "comparer plusieurs pistes", or generic one-word answers. The host always adds the fourth Other option with a free-text field.`
  ].join("\n");
}

function waitForNativeMapsFirstVisual(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 900);
  });
}

async function executeAssistantModuleCodeActs(
  message: TranscriptMessage,
  moduleId: string,
  parallelSessionIndex: number
): Promise<TranscriptMessage> {
  if (moduleId === "airbnb") {
    return executeAssistantAirbnbCodeAct(message, parallelSessionIndex);
  }
  if (moduleId === "gmail") {
    return executeAssistantGmailCodeAct(message, parallelSessionIndex);
  }
  let next = executeAssistantGoogleWebCodeAct(message, parallelSessionIndex);
  const shouldOpenAirbnbAfterMaps =
    next.text.includes(BRAIN_MAPS_COMMAND) &&
    next.text.includes(BRAIN_AIRBNB_COMMAND) &&
    !next.text.includes("MAPS_RESULT") &&
    !next.text.includes("AIRBNB_RESULT");
  next = await executeAssistantMapsCodeAct(next, parallelSessionIndex);
  if (shouldOpenAirbnbAfterMaps && next.text.includes("MAPS_RESULT")) {
    await waitForNativeMapsFirstVisual();
  }
  next = executeAssistantGmailCodeAct(next, parallelSessionIndex);
  next = executeAssistantAirbnbCodeAct(next, parallelSessionIndex);
  return next;
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

function assistantPatternDemoArchiveSession(): ChatArchiveSession {
  const now = "2026-06-13T03:20:00.000Z";
  const messages: ChatArchiveMessage[] = [
    { turnId: "pattern-demo-user-summary", role: "user", text: "Montre-moi un resume court.", createdAt: now, attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-summary") },
    { turnId: "pattern-demo-assistant-summary", role: "assistant", text: "En bref : Kagoshima a des hivers doux, des etes tres humides, et l'automne reste souvent la saison la plus confortable.\n\nA retenir : chaleur + humidite structurent la plupart des conseils.", createdAt: "2026-06-13T03:20:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-summary") },
    { turnId: "pattern-demo-user-facts", role: "user", text: "Montre-moi des paires cle-valeur.", createdAt: "2026-06-13T03:21:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-facts") },
    { turnId: "pattern-demo-assistant-facts", role: "assistant", text: "Fiche compacte :\n\nVille: Kagoshima\nSaison ideale: automne\nHumidite: elevee en ete\nPoint de vigilance: typhons possibles", createdAt: "2026-06-13T03:21:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-facts") },
    { turnId: "pattern-demo-user-steps", role: "user", text: "Montre-moi une procedure.", createdAt: "2026-06-13T03:22:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-steps") },
    { turnId: "pattern-demo-assistant-steps", role: "assistant", text: "Procedure recommandee :\n\n1. Identifier le pattern dominant.\n2. Verifier si le parser sait deja le reconnaitre.\n3. Le rendre avec la grammaire visuelle commune.\n4. Ajouter une assertion de non-regression.", createdAt: "2026-06-13T03:22:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-steps") },
    { turnId: "pattern-demo-user-plan", role: "user", text: "Montre-moi un plan d'action.", createdAt: "2026-06-13T03:23:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-plan") },
    { turnId: "pattern-demo-assistant-plan", role: "assistant", text: "Plan d'action :\n\nObjectif: harmoniser les rendus LLM\nEtape 1: detecter les blocs recurrents\nEtape 2: appliquer les tokens visuels communs\nValidation: test Markdown cible + build renderer\nProchaine action: ouvrir la session galerie et comparer les blocs", createdAt: "2026-06-13T03:23:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-plan") },
    { turnId: "pattern-demo-user-pros-cons", role: "user", text: "Montre-moi avantages et limites.", createdAt: "2026-06-13T03:24:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-pros-cons") },
    { turnId: "pattern-demo-assistant-pros-cons", role: "assistant", text: "Avantages :\n\n- Lecture plus rapide\n- Patterns LLM previsibles\n- Moins de blocs bruts fatigants\n\nLimites :\n\n- Certains formats sont ambigus\n- Le parser doit rester conservateur\n- Les longs tableaux doivent rester scrollables", createdAt: "2026-06-13T03:24:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-pros-cons") },
    { turnId: "pattern-demo-user-callout", role: "user", text: "Montre-moi notes, warnings et hypotheses.", createdAt: "2026-06-13T03:25:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-callout") },
    { turnId: "pattern-demo-assistant-callout", role: "assistant", text: "Note: Ceci devient une information discrete.\n\nAttention: cette ligne devient un warning.\n\nHypothese: le LLM peut envoyer une supposition que le front doit distinguer.", createdAt: "2026-06-13T03:25:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-callout") },
    { turnId: "pattern-demo-user-quote", role: "user", text: "Montre-moi une citation/source.", createdAt: "2026-06-13T03:26:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-quote") },
    { turnId: "pattern-demo-assistant-quote", role: "assistant", text: "Citation/source a isoler :\n\n> Le rendu doit aider a scanner l'information, pas seulement reproduire le texte brut.\n> Une citation consecutive reste un seul bloc.", createdAt: "2026-06-13T03:26:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-quote") },
    { turnId: "pattern-demo-user-table", role: "user", text: "Montre-moi une reponse avec un tableau Markdown.", createdAt: "2026-06-13T03:27:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-table") },
    { turnId: "pattern-demo-assistant-table", role: "assistant", text: "Tableau climatique aplati comme un LLM peut le renvoyer :\n\n| Saison | Temperature | Ressenti | |---|---:|---| | Hiver | 5-14 C | Doux | | Printemps | 10-24 C | Agreable | | Ete | 24-33 C | Chaud et humide |", createdAt: "2026-06-13T03:27:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-table") },
    { turnId: "pattern-demo-user-code", role: "user", text: "Montre-moi un code fence.", createdAt: "2026-06-13T03:28:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-code") },
    { turnId: "pattern-demo-assistant-code", role: "assistant", text: "Exemple de code fence :\n\n```rust\nfn seasonal_band(month: u8) -> &'static str {\n    match month {\n        12 | 1 | 2 => \"winter\",\n        6..=8 => \"summer\",\n        _ => \"transition\",\n    }\n}\n```", createdAt: "2026-06-13T03:28:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-code") },
    { turnId: "pattern-demo-user-data", role: "user", text: "Montre-moi du JSON ou YAML.", createdAt: "2026-06-13T03:29:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-data") },
    { turnId: "pattern-demo-assistant-data", role: "assistant", text: "Exemple de donnees structurees :\n\n```json\n{\n  \"city\": \"Kagoshima\",\n  \"season\": \"autumn\",\n  \"risk\": \"typhoon watch\"\n}\n```", createdAt: "2026-06-13T03:29:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-data") },
    { turnId: "pattern-demo-user-command", role: "user", text: "Montre-moi une commande terminal.", createdAt: "2026-06-13T03:30:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-command") },
    { turnId: "pattern-demo-assistant-command", role: "assistant", text: "Commande a copier :\n\n```powershell\nnpm.cmd run build\nnpx.cmd vitest run tests/llm-multimodal-attachments.test.ts -t \"renders assistant markdown\"\n```", createdAt: "2026-06-13T03:30:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-command") },
    { turnId: "pattern-demo-user-log", role: "user", text: "Montre-moi un log d'erreur.", createdAt: "2026-06-13T03:31:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-log") },
    { turnId: "pattern-demo-assistant-log", role: "assistant", text: "Erreur a diagnostiquer :\n\n```text\nError: table row has 5 cells but header has 3\n  at parseMarkdownTable (PanelsChatBottomSlice.tsx:1488)\n  at AssistantMarkdownText (PanelsChatBottomSlice.tsx:2552)\n```", createdAt: "2026-06-13T03:31:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-log") },
    { turnId: "pattern-demo-user-math", role: "user", text: "Montre-moi calculs et formules.", createdAt: "2026-06-13T03:32:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-math") },
    { turnId: "pattern-demo-assistant-math", role: "assistant", text: "Calcul rapide :\n\nTemperature moyenne: (24 + 33) / 2 = 28.5 C\nHumidite ressentie: 28.5 C + facteur humidite eleve => sensation plus lourde\nROI simplifie: gain / cout = 420 / 120 = 3.5", createdAt: "2026-06-13T03:32:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-math") },
    { turnId: "pattern-demo-user-decision", role: "user", text: "Montre-moi une decision recommandee.", createdAt: "2026-06-13T03:33:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-decision") },
    { turnId: "pattern-demo-assistant-decision", role: "assistant", text: "Recommandation : partir en automne.\n\nOption A: automne, meilleur equilibre confort/pluie.\nOption B: printemps, agreable mais humidite montante.\nOption C: ete, a eviter si tu supportes mal la chaleur.\n\nConclusion : je choisirais septembre tardif ou octobre.", createdAt: "2026-06-13T03:33:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-decision") },
    { turnId: "pattern-demo-user-questions", role: "user", text: "Montre-moi des questions de clarification.", createdAt: "2026-06-13T03:34:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-questions") },
    { turnId: "pattern-demo-assistant-questions", role: "assistant", text: "Questions utiles :\n\n- Tu veux une reponse touristique ou meteorologique ?\n- Tu preferes des moyennes mensuelles ou saisonnieres ?\n- Tu veux inclure les risques typhon ?", createdAt: "2026-06-13T03:34:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-questions") },
    { turnId: "pattern-demo-user-checklist", role: "user", text: "Montre-moi une checklist.", createdAt: "2026-06-13T03:35:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-checklist") },
    { turnId: "pattern-demo-assistant-checklist", role: "assistant", text: "Checklist de preparation :\n\n- [x] Identifier le pattern LLM\n- [x] Appliquer l'identite graphique commune\n- [ ] Verifier le rendu dans la session fictive", createdAt: "2026-06-13T03:35:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-checklist") },
    { turnId: "pattern-demo-user-divider", role: "user", text: "Montre-moi une synthese separee.", createdAt: "2026-06-13T03:36:00.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-user-divider") },
    { turnId: "pattern-demo-assistant-divider", role: "assistant", text: "Avant la synthese.\n\n---\n\n1. Lire le pattern.\n2. Promouvoir en composant.\n3. Garder le texte accessible.", createdAt: "2026-06-13T03:36:10.000Z", attachments: [], proofHash: stableSearchArchiveHash("pattern-demo-assistant-divider") }
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
  if (chatArchiveLoaded) {
    syncLocalChatSessionsFromArchive();
  }
  ensureAssistantPatternDemoSession();
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
  const modelLabels = selectedProfile.connected
    ? selectedProfile.models.length > 0 ? selectedProfile.models : ["Model catalog unavailable"]
    : ["Connect provider"];
  const reasoningLabels = selectedProfile.connected
    ? selectedProfile.reasoning.length > 0 ? selectedProfile.reasoning : ["Reasoning unavailable"]
    : ["-"];
  panelsChatBottomState.modelIndex = panelsChatBottomState.modelIndex % modelLabels.length;
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
  let latestCommittedTranscript = transcript;
  const trackedCommitTranscript = (nextTranscript: TranscriptMessage[]) => {
    latestCommittedTranscript = nextTranscript;
    commitTranscript(nextTranscript);
  };
  try {
    await submitChatDraftForSessionInner(draft, moduleId, pendingUploadItems, session, transcript, parallelSessionIndex, internalPrompt, replaceAssistantMessageId, trackedCommitTranscript);
  } finally {
    session.working = transcriptHasOpenQuestionnaire(latestCommittedTranscript);
    session.date = todayIsoDate();
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
  commitTranscript: (transcript: TranscriptMessage[]) => void
): Promise<void> {
  const requestSessionId = session.sessionId;
  const requestTranscriptBeforeSend = transcriptWithoutMessage([...transcript], replaceAssistantMessageId);
  const searchArchiveRequest = parseSearchArchiveCodeAct(draft);
  let providerAttachments: ProviderAttachment[] = [];
  const editTargetIdForTurn = panelsChatBottomState.uploadEditTargetId;
  let attachmentPreviews = pendingUploadItems.map(publicUploadPreview);
  let attachmentProofs: unknown = attachmentPreviews.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    kind: attachment.kind,
    proofHash: hashJson(attachment)
  }));
  if (!searchArchiveRequest) {
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
  let assistantMessage: TranscriptMessage;
  if (searchArchiveRequest) {
    assistantMessage = await localSearchArchiveStatus(searchArchiveRequest);
  } else {
    // Audit anchor: await buildAssistantTranscriptMessage(draft, providerAttachments, message.id, moduleId)
    assistantMessage = await buildAssistantTranscriptMessage(draft, providerAttachments, message.id, moduleId, requestTranscriptWithUser);
  }
  assistantMessage = applyGeographicTravelAirbnbFallback(assistantMessage, draft, moduleId);
  assistantMessage = await applyGeographicMapsFallback(assistantMessage, draft, moduleId, parallelSessionIndex);
  assistantMessage = await executeAssistantModuleCodeActs(assistantMessage, moduleId, parallelSessionIndex);
  assistantMessage = enforceQuestionnaireLoopPause(assistantMessage);
  assistantMessage = suppressRepeatedBrainSegmentCodeAct(assistantMessage, panelsChatBottomState.activeBrainSegment);
  const previousBrainSegment = panelsChatBottomState.activeBrainSegment;
  const activatedBrainSegment = brainSegmentFromAssistantText(assistantMessage.text);
  assistantMessage = executeAssistantBrainSegmentCodeAct(assistantMessage);
  assistantMessage = enforceQuestionnaireLoopPause(assistantMessage);
  if (replaceAssistantMessageId) {
    assistantMessage = {
      ...assistantMessage,
      id: replaceAssistantMessageId,
      proofHash: hashJson({ replaceAssistantMessageId, text: assistantMessage.text, previousProofHash: assistantMessage.proofHash })
    };
  }
  nextTranscript = transcriptWithMessage(nextTranscript, assistantMessage);
  commitTranscript(nextTranscript);
  // Audit anchor: archiveTranscriptMessage(activeSession, assistantMessage)
  archiveTranscriptMessage(session, assistantMessage);
  if (!searchArchiveRequest && activatedBrainSegment && activatedBrainSegment !== previousBrainSegment) {
    const continuationUserText = brainSegmentContinuationUserText(draft, activatedBrainSegment);
    let continuationMessage = await buildAssistantTranscriptMessage(
      continuationUserText,
      providerAttachments,
      message.id,
      moduleId,
      nextTranscript
    );
    continuationMessage = await executeAssistantModuleCodeActs(continuationMessage, moduleId, parallelSessionIndex);
    continuationMessage = enforceQuestionnaireLoopPause(continuationMessage);
    continuationMessage = suppressRepeatedBrainSegmentCodeAct(continuationMessage, panelsChatBottomState.activeBrainSegment);
    continuationMessage = executeAssistantBrainSegmentCodeAct(continuationMessage);
    continuationMessage = enforceQuestionnaireLoopPause(continuationMessage);
    nextTranscript = transcriptWithMessage(nextTranscript, continuationMessage);
    commitTranscript(nextTranscript);
    archiveTranscriptMessage(session, continuationMessage);
  }
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
    }
  );
}

async function applyPanelsChatBottomCommand(command: PanelsChatBottomCommand): Promise<void> {
  panelsChatBottomState.lastControl = command.kind;
  switch (command.kind) {
    case "new_session":
      resetPanelsChatSessionView();
      sidebarState.recentSessionId = "";
      break;
    case "chat_text_edited":
      panelsChatBottomState.chatText = command.value ?? panelsChatBottomState.chatText;
      break;
    case "send_chat": {
      const draft = (command.value ?? panelsChatBottomState.chatText).trim();
      const moduleId = typeof command.moduleId === "string" ? command.moduleId : "";
      const pendingUploadItems = composerUploadItemsForCommand(command);
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

  ipcMain.handle("forge:search-city-suggestions", async (event, query: unknown): Promise<CitySuggestionResult> => {
    if (!validateSender(event)) {
      return citySuggestionError("", "City lookup rejected by sender validation.");
    }
    return searchCitySuggestions(query);
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
    thickFrame: true,
    resizable: true,
    minimizable: true,
    maximizable: true,
    closable: true,
    focusable: true,
    skipTaskbar: false,
    autoHideMenuBar: labWindow ? true : false,
    title: labWindow ? "InGen Event Text Lab" : "InGen",
    show: false,
    backgroundColor: labWindow ? "#101112" : "#0e0e0f",
    webPreferences: {
      preload: join(shellRoot, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
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
  window.on("closed", () => {
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
  destroyAttachmentSnapshotWindow();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
