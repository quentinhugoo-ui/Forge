import {
  FORGE_ELECTRON_IPC_VERSION,
  BRAIN_AIRBNB_COMMAND,
  BRAIN_AIRBNB_RESULT_SCHEMA,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_IMAGE_RESULT_SCHEMA,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GMAIL_RESULT_SCHEMA,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_GOOGLEWEB_RESULT_SCHEMA,
  BRAIN_MAPS_COMMAND,
  BRAIN_MAPS_RESULT_SCHEMA,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_QUESTIONNAIRE_COMMAND,
  BRAIN_QUESTIONNAIRE_RESULT_SCHEMA,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_CODING_COMMAND,
  BRAIN_SCIENCE_VISIBLE_CATALOG,
  BRAIN_CODING_VISIBLE_CATALOG,
  BRAIN_WORKSPACE_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SEARCHARCHIVE_RESULT_SCHEMA,
  BRAIN_RENAME_SESSION_COMMAND,
  BRAIN_RENAME_SESSION_RESULT_SCHEMA,
  CANVAS_SURFACES_COMMAND_KIND,
  HEADER_COMMAND_KIND,
  PANELS_CHAT_BOTTOM_COMMAND_KIND,
  PROFILE_CANVAS,
  RIGHT_PANEL_COMMAND_KIND,
  SESSIONS_MENU_MODE,
  SIDEBAR_COMMAND_KIND,
  NATIVE_SECTION
} from "./generated/forge-ipc.generated.js";
import type {
  ForgeShellApi as GeneratedForgeShellApi,
  FrontSliceMode,
  CanvasSurfacesCommand,
  CanvasSurfacesCommandKind,
  CanvasSurfacesSnapshot,
  ComposerUploadPreview,
  HeaderCommand,
  HeaderCommandKind,
  HeaderCommandResult,
  HeaderControl,
  HeaderSurfaceSnapshot,
  HeaderSnapshot,
  IpcError,
  IpcErrorCode,
  NativeAuthority,
  NativeSection,
  ParallelChatDraft,
  PanelsChatBottomCommand,
  PanelsChatBottomCommandKind,
  PanelsChatBottomCommandResult,
  PanelsChatBottomSnapshot,
  ProfileCanvas,
  RightPanelCommand,
  RightPanelCommandKind,
  RightPanelSnapshot,
  SessionsMenuMode,
  SidebarCommand,
  SidebarCommandKind,
  SidebarCommandResult,
  SidebarSnapshot,
  SidebarSessionItem,
  SidebarToolControl
} from "./generated/forge-ipc.generated.js";

export {
  BRAIN_AIRBNB_COMMAND,
  BRAIN_AIRBNB_COMMAND_DESCRIPTION,
  BRAIN_AIRBNB_RESULT_SCHEMA,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_EDITIMAGE_COMMAND_DESCRIPTION,
  BRAIN_IMAGE_RESULT_SCHEMA,
  BRAIN_BRAIN_COMMAND,
  BRAIN_BRAIN_COMMAND_DESCRIPTION,
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_CODEACT_COMMANDS,
  BRAIN_CODEACT_ROUTING_RULES,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_FRONTDESIGN_COMMAND_DESCRIPTION,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COMMAND_DESCRIPTION,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GMAIL_COM_COMMAND_DESCRIPTION,
  BRAIN_GMAIL_RESULT_SCHEMA,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_GOOGLEWEB_COMMAND_DESCRIPTION,
  BRAIN_GOOGLEWEB_RESULT_SCHEMA,
  BRAIN_MAPS_COMMAND,
  BRAIN_MAPS_COMMAND_DESCRIPTION,
  BRAIN_MAPS_RESULT_SCHEMA,
  BRAIN_WORKSPACE_COMMAND,
  BRAIN_WORKSPACE_COMMAND_DESCRIPTION,
  BRAIN_GOOGLE_AGENDA_COMMAND,
  BRAIN_GOOGLE_AGENDA_COMMAND_DESCRIPTION,
  BRAIN_NAMED_COMPUTE_COMMAND,
  BRAIN_NAMED_COMPUTE_COMMAND_DESCRIPTION,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND_DESCRIPTION,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_NEWIMAGE_COMMAND_DESCRIPTION,
  BRAIN_QUESTIONNAIRE_COMMAND,
  BRAIN_QUESTIONNAIRE_COMMAND_DESCRIPTION,
  BRAIN_QUESTIONNAIRE_RESULT_SCHEMA,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_SCIENCE_COMMAND_DESCRIPTION,
  BRAIN_CODING_COMMAND,
  BRAIN_CODING_COMMAND_DESCRIPTION,
  BRAIN_SCIENCE_VISIBLE_CATALOG,
  BRAIN_CODING_VISIBLE_CATALOG,
  BRAIN_NEWMODULE_COMMAND,
  BRAIN_NEWMODULE_COMMAND_DESCRIPTION,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_NEWOBJECT_COMMAND_DESCRIPTION,
  BRAIN_RUST_PORT_ADAPTER_COMMAND,
  BRAIN_RUST_PORT_ADAPTER_COMMAND_DESCRIPTION,
  BRAIN_RUST_STATE_STORE_COMMAND,
  BRAIN_RUST_STATE_STORE_COMMAND_DESCRIPTION,
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND_DESCRIPTION,
  BRAIN_SEARCHARCHIVE_RESULT_SCHEMA,
  BRAIN_RENAME_SESSION_COMMAND,
  BRAIN_RENAME_SESSION_COMMAND_DESCRIPTION,
  BRAIN_RENAME_SESSION_RESULT_SCHEMA,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND_DESCRIPTION,
  BRAIN_WEB_COMMAND,
  BRAIN_WEB_COMMAND_DESCRIPTION,
  CANVAS_SURFACES_COMMAND_KIND,
  CANVAS_SURFACES_COMMAND_RESULT_EVENT,
  CANVAS_SURFACE_KIND,
  CANVAS_SURFACE_STATUS,
  FORGE_ELECTRON_IPC_CONTRACT_SOURCE,
  FORGE_ELECTRON_IPC_VERSION,
  FRONT_SLICE_MODE,
  HEADER_COMMAND_KIND,
  HEADER_COMMAND_RESULT_EVENT,
  HEADER_SURFACE_KIND,
  HEADER_SURFACE_STATUS,
  IPC_ERROR_CODE,
  NATIVE_AUTHORITY,
  NATIVE_SECTION,
  PANELS_CHAT_BOTTOM_COMMAND_KIND,
  PANELS_CHAT_BOTTOM_COMMAND_RESULT_EVENT,
  PROFILE_CANVAS,
  RIGHT_PANEL_COMMAND_KIND,
  RIGHT_PANEL_COMMAND_RESULT_EVENT,
  SESSIONS_MENU_MODE,
  SIDEBAR_COMMAND_KIND,
  SIDEBAR_COMMAND_RESULT_EVENT
} from "./generated/forge-ipc.generated.js";
export type {
  FrontSliceMode,
  HeaderCommand,
  HeaderCommandKind,
  HeaderCommandResult,
  HeaderControl,
  HeaderSurfaceContract,
  HeaderSurfaceKind,
  HeaderSurfaceSnapshot,
  HeaderSurfaceSlot,
  HeaderSurfaceStatus,
  HeaderSnapshot,
  IpcError,
  IpcErrorCode,
  NativeAuthority,
  NativeSection,
  BottomControl,
  BrainCodeActCommand,
  CanvasSurfaceKind,
  CanvasSurfaceStatus,
  CanvasSurfaceSummary,
  CanvasSurfacesCommand,
  CanvasSurfacesCommandKind,
  CanvasSurfacesCommandResult,
  CanvasSurfacesSnapshot,
  ComposerSnapshot,
  ComposerUploadPreview,
  LlmProviderState,
  ParallelChatDraft,
  PanelsChatBottomCommand,
  PanelsChatBottomCommandKind,
  PanelsChatBottomCommandResult,
  PanelsChatBottomSnapshot,
  ProfileCanvas,
  RightPanelAction,
  RightPanelCommand,
  RightPanelCommandKind,
  RightPanelCommandResult,
  RightPanelLine,
  RightPanelSnapshot,
  RightPanelTab,
  SessionsMenuMode,
  SidebarCommand,
  SidebarCommandKind,
  SidebarCommandResult,
  SidebarSnapshot,
  SidebarSessionItem,
  SidebarToolControl,
  StatusDockLine,
  StatusDockSnapshot,
  TranscriptMessage
} from "./generated/forge-ipc.generated.js";

export type LlmProviderConnectId = "codex" | "claude" | "openrouter";

export interface LlmProviderConnectResult {
  provider: LlmProviderConnectId;
  accepted: boolean;
  events: string[];
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  proofHash: string;
  error?: IpcError;
}

export interface LlmProviderRuntimeEvent {
  provider: LlmProviderConnectId;
  events: string[];
  models: string[];
  reasoning: string[];
  quotaLabel: string;
  proofHash: string;
}

export type LlmProviderRuntimeSnapshot = Record<LlmProviderConnectId, LlmProviderRuntimeEvent>;

export interface PanelsChatBottomSnapshotEvent {
  kind: "snapshot_updated";
  reason: "transcript_committed" | "assistant_progressive_seed";
  sessionId: string;
  proofHash: string;
}

export type TerminalRuntimeEvent =
  | {
      kind: "ready";
      shell: string;
      cwd: string;
      prompt: string;
      subtitle: string;
      proofHash: string;
    }
  | {
      kind: "output";
      stream: "stdout" | "stderr";
      data: string;
      proofHash: string;
    }
  | {
      kind: "exit";
      code: number | null;
      signal: string | null;
      proofHash: string;
    };

export interface TerminalStartResult {
  accepted: boolean;
  shell: string;
  cwd: string;
  prompt: string;
  subtitle: string;
  proofHash: string;
  error?: IpcError;
}

export interface ForgeTerminalApi {
  start: () => Promise<TerminalStartResult>;
  write: (data: string) => Promise<boolean>;
  resize: (cols: number, rows: number) => Promise<boolean>;
  stop: () => Promise<boolean>;
  onEvent: (listener: (event: TerminalRuntimeEvent) => void) => () => void;
}

export interface WorkspaceChoiceResult {
  canceled: boolean;
  path: string;
  folderName: string;
  proofHash: string;
  error?: IpcError;
}

export interface WorkspaceActionResult {
  accepted: boolean;
  path: string;
  value: string;
  proofHash: string;
  error?: IpcError;
}

export type AgentActionCapabilityId =
  | "fs.list"
  | "fs.search"
  | "fs.create_directory"
  | "fs.rename"
  | "fs.move"
  | "fs.copy"
  | "fs.delete_empty_directory"
  | "fs.delete_tree"
  | "shell.readonly"
  | "shell.full"
  | "browser.playwright"
  | "computer_use"
  | "mcp";

export type AgentActionRisk = "read" | "workspace_write" | "computer_write" | "destructive" | "external_ui" | "blocked";

export interface AgentActionCapability {
  id: AgentActionCapabilityId;
  title: string;
  status: "available" | "planned" | "blocked";
  risk: AgentActionRisk;
  underlyingTools: string[];
  requiresApproval: boolean;
  writes: boolean;
  description: string;
}

export interface AgentActionHostManifest {
  schema: "ingen.agent_action_host.manifest.v1";
  workspace: {
    active: boolean;
    root: string;
    cwd: string;
    protectedRoots: string[];
  };
  permissions: {
    sandbox: "workspace_or_confirmed_computer";
    recursiveDelete: "confirmed_with_absolute_path_guard";
    shell: "readonly_allowlist_or_confirmed_full";
    browser: "contained_webexplorer";
    computerUse: "planned_confirmation_required";
  };
  capabilities: AgentActionCapability[];
  proofHash: string;
}

export type AgentActionKind =
  | "list"
  | "search"
  | "create_directory"
  | "rename_path"
  | "move_path"
  | "copy_path"
  | "delete_empty_directory"
  | "delete_tree"
  | "run_readonly_command"
  | "run_command";

export type AgentActionScope = "workspace" | "computer";

export interface AgentActionRequest {
  action: AgentActionKind;
  scope?: AgentActionScope;
  path?: string;
  toPath?: string;
  query?: string;
  command?: string;
  args?: string[];
  maxResults?: number;
  confirmed?: boolean;
  recursive?: boolean;
  timeoutMs?: number;
}

export interface AgentActionPathEntry {
  name: string;
  path: string;
  kind: "file" | "directory" | "other";
}

export interface AgentActionSearchMatch {
  path: string;
  line: number;
  text: string;
}

export interface AgentActionResult {
  schema: "ingen.agent_action_host.result.v1";
  accepted: boolean;
  action: AgentActionKind;
  cwd: string;
  path?: string;
  toPath?: string;
  items?: AgentActionPathEntry[];
  matches?: AgentActionSearchMatch[];
  commandLine?: string;
  exitCode?: number | null;
  stdoutPreview?: string;
  stderrPreview?: string;
  value?: string;
  proofHash: string;
  error?: IpcError;
}

export function isAgentActionRequest(value: unknown): value is AgentActionRequest {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<AgentActionRequest>;
  const actions: AgentActionKind[] = [
    "list",
    "search",
    "create_directory",
    "rename_path",
    "move_path",
    "copy_path",
    "delete_empty_directory",
    "delete_tree",
    "run_readonly_command",
    "run_command"
  ];
  if (!actions.includes(candidate.action as AgentActionKind)) {
    return false;
  }
  if (candidate.scope !== undefined && candidate.scope !== "workspace" && candidate.scope !== "computer") {
    return false;
  }
  for (const key of ["path", "toPath", "query", "command"] as const) {
    if (candidate[key] !== undefined && typeof candidate[key] !== "string") {
      return false;
    }
  }
  if (candidate.args !== undefined && (!Array.isArray(candidate.args) || !candidate.args.every((arg) => typeof arg === "string"))) {
    return false;
  }
  if (
    candidate.maxResults !== undefined &&
    (!Number.isInteger(candidate.maxResults) || candidate.maxResults < 1 || candidate.maxResults > 500)
  ) {
    return false;
  }
  if (candidate.confirmed !== undefined && typeof candidate.confirmed !== "boolean") {
    return false;
  }
  if (candidate.recursive !== undefined && typeof candidate.recursive !== "boolean") {
    return false;
  }
  if (
    candidate.timeoutMs !== undefined &&
    (!Number.isInteger(candidate.timeoutMs) || candidate.timeoutMs < 100 || candidate.timeoutMs > 600_000)
  ) {
    return false;
  }
  return true;
}

export interface NativeWebExplorerBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface NativeWebExplorerResult {
  accepted: boolean;
  url: string;
  proofHash: string;
  error?: IpcError;
}

export interface NativeDomRamArtifactSummary {
  kind: "dom_graph_page" | "ram_region_table" | "browser_event_loop_slice";
  layout: string;
  liveCapturePolicy: string;
  liveBackpressurePolicy: string;
  liveSectionOwner: string;
  liveSliceHash: string;
  byteLength: number;
  recordCount: number;
}

export interface NativeDomRamUiTreeNode {
  nodeId: string;
  parentNodeId: string;
  depth: number;
  backendNodeId: number;
  nodeType: number;
  nodeName: string;
  nodeValue: string;
  attributes: Record<string, string>;
  layout?: {
    x: number;
    y: number;
    width: number;
    height: number;
    paintOrder: number;
    text: string;
  };
  visible: boolean;
}

export interface NativeDomRamUiTreeLandmark {
  role: "google_earth_search_bar";
  nodeId: string;
  backendNodeId: number;
  confidence: number;
  label: string;
  reason: string;
  layout?: NativeDomRamUiTreeNode["layout"];
}

export interface NativeDomRamCartographyResult {
  accepted: boolean;
  schema: "forge.webexplorer.dom_ram_cartography.v1";
  target: "google_earth";
  url: string;
  lane: "native_tandem_dom_ram";
  nativeDomain: "dom_ram";
  engine: "monster_native_tandem";
  snapshot: {
    source: "cdp_domsnapshot";
    documentCount: number;
    nodeCount: number;
    layoutCount: number;
    textBoxCount: number;
    scrollOffsetX: number;
    scrollOffsetY: number;
    captureHash: string;
  };
  uiTree: {
    schema: "forge.webexplorer.dom_ram_ui_tree.v1";
    nodeCount: number;
    nodes: NativeDomRamUiTreeNode[];
    landmarks: {
      googleEarthSearchBar?: NativeDomRamUiTreeLandmark;
      searchCandidates: NativeDomRamUiTreeLandmark[];
    };
    treeHash: string;
  };
  memory: {
    source: "electron_webcontents";
    workingSetSizeKb: number;
    peakWorkingSetSizeKb: number;
    privateBytesKb: number;
    sharedBytesKb: number;
    processId: number;
    processType: string;
    regionTableHash: string;
  };
  artifacts: NativeDomRamArtifactSummary[];
  manifestHash: string;
  proofHash: string;
  error?: IpcError;
}

export interface NativeWebExplorerCodeAct {
  schema: "forge.webexplorer.googleweb.request.v1" | "forge.webexplorer.maps.request.v1" | "forge.webexplorer.gmail.request.v1" | "forge.webexplorer.airbnb.request.v1";
  command: typeof BRAIN_GOOGLEWEB_COMMAND | typeof BRAIN_MAPS_COMMAND | typeof BRAIN_GMAIL_COMMAND | typeof BRAIN_GMAIL_COM_COMMAND | typeof BRAIN_AIRBNB_COMMAND;
  parallelSessionIndex?: number;
  intent?: "open" | "search" | "inspect" | "summarize" | "draft" | "reply";
  target?: string;
  latitude?: number;
  longitude?: number;
  query: string;
  keywords: string[];
  url: string;
  source?: "explicit_codeact";
  recipient?: string;
  subject?: string;
  body?: string;
  proofHash: string;
}

export interface NativeTerminalBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface NativeTerminalResult {
  accepted: boolean;
  path: string;
  proofHash: string;
  error?: IpcError;
}

export interface CitySuggestion {
  label: string;
  city: string;
  country: string;
  latitude?: number;
  longitude?: number;
  source: "google_places" | "photon";
}

export interface CitySuggestionResult {
  schema: "ingen.brain.memory.city_suggestions.v1";
  query: string;
  suggestions: CitySuggestion[];
  proofHash: string;
  error?: IpcError;
}

export type SearchArchiveScope = "recent" | "archived" | "all";
export type SearchArchiveSourceType = "session_message" | "attachment";

export interface SearchArchiveRequest {
  query: string;
  scope?: SearchArchiveScope;
  topK?: number;
  contextTurns?: number;
  targets?: string[];
}

export interface SearchArchiveContextLine {
  role: "user" | "assistant" | "system";
  turnId: string;
  text: string;
  proofHash: string;
}

export interface SearchArchiveAttachmentRef {
  id: string;
  name: string;
  kind: ComposerUploadPreview["kind"];
  textPreview: string;
  proofHash: string;
  openRef: string;
}

export interface SearchArchiveHit {
  rank: number;
  sourceType: SearchArchiveSourceType;
  sessionId: string;
  sessionTitle: string;
  turnId: string;
  role: "user" | "assistant" | "system";
  createdAt: string;
  matchedField: "message_text" | "attachment_name" | "attachment_text";
  snippet: string;
  contextBefore: SearchArchiveContextLine[];
  contextAfter: SearchArchiveContextLine[];
  attachments: SearchArchiveAttachmentRef[];
  score: number;
  evidenceHash: string;
  openRef: string;
  fetchMoreRef: string;
}

export interface SearchArchiveResult {
  schema: typeof BRAIN_SEARCHARCHIVE_RESULT_SCHEMA;
  query: string;
  scope: SearchArchiveScope;
  matchCount: number;
  returnedCount: number;
  truncated: boolean;
  tokenBudgetUsedEstimate: number;
  indexSnapshotHash: string;
  hits: SearchArchiveHit[];
  proofHash: string;
}

export interface SessionFilesGroup {
  sessionId: string;
  sessionName: string;
  date: string;
  archived: boolean;
  files: ComposerUploadPreview[];
  proofHash: string;
}

export interface SessionFilesSnapshot {
  schema: "ingen.electron.session_files.snapshot.v1";
  groups: SessionFilesGroup[];
  fileCount: number;
  proofHash: string;
}

export interface HardwareMetric {
  label: string;
  value: number | null;
  unit: "%" | "GB" | "MB" | "MHz" | "RPM" | "C" | "W" | "count" | "text";
  status: "ok" | "warning" | "critical" | "unavailable";
}

export interface HardwareGpuSnapshot {
  name: string;
  vendor: "nvidia" | "amd" | "intel" | "apple" | "unknown";
  source: "nvidia-smi" | "linux-drm" | "system" | "unavailable";
  utilization: HardwareMetric;
  memoryUsed: HardwareMetric;
  memoryTotal: HardwareMetric;
  temperature: HardwareMetric;
  fanSpeed: HardwareMetric;
  powerDraw: HardwareMetric;
}

export interface HardwareProcessSnapshot {
  pid: number;
  name: string;
  cpuPercent: number;
  memoryMb: number;
}

export interface HardwareTelemetrySnapshot {
  schema: "ingen.hardware.telemetry.snapshot.v1";
  platform: NodeJS.Platform | "unknown";
  arch: string;
  hostname: string;
  sampledAt: string;
  cpu: {
    model: string;
    cores: number;
    utilization: HardwareMetric;
    loadAverage: HardwareMetric;
  };
  memory: {
    used: HardwareMetric;
    total: HardwareMetric;
    utilization: HardwareMetric;
  };
  thermal: {
    systemTemperature: HardwareMetric;
    source: "linux-thermal" | "windows-acpi" | "unavailable";
  };
  gpus: HardwareGpuSnapshot[];
  topProcesses: HardwareProcessSnapshot[];
  governor: {
    profile: "quiet" | "balanced" | "performance";
    monsterBudgetPercent: number;
    bangerBudgetPercent: number;
    controlAuthority: "app-budget-only" | "native-driver-ready";
    fanControl: "locked";
    notes: string[];
  };
  proofHash: string;
}

export interface ForgeShellApi extends GeneratedForgeShellApi {
  connectLlmProvider: (provider: LlmProviderConnectId) => Promise<LlmProviderConnectResult>;
  resetLlmProvider?: (provider: LlmProviderConnectId) => Promise<LlmProviderConnectResult>;
  getLlmProviderRuntimeSnapshot?: () => Promise<LlmProviderRuntimeSnapshot>;
  onLlmProviderEvent?: (listener: (event: LlmProviderRuntimeEvent) => void) => () => void;
  onPanelsChatBottomSnapshotEvent?: (listener: (event: PanelsChatBottomSnapshotEvent) => void) => () => void;
  chooseWorkspaceFolder?: () => Promise<WorkspaceChoiceResult>;
  getWorkspaceFolder?: () => Promise<WorkspaceChoiceResult>;
  getAgentActionHostManifest?: () => Promise<AgentActionHostManifest>;
  executeAgentAction?: (request: AgentActionRequest) => Promise<AgentActionResult>;
  getHardwareTelemetrySnapshot?: () => Promise<HardwareTelemetrySnapshot>;
  showWorkspaceInExplorer?: () => Promise<WorkspaceActionResult>;
  copyWorkspacePath?: () => Promise<WorkspaceActionResult>;
  copyWorkspaceBranchName?: () => Promise<WorkspaceActionResult>;
  searchArchive?: (request: SearchArchiveRequest) => Promise<SearchArchiveResult>;
  getSessionFilesSnapshot?: () => Promise<SessionFilesSnapshot>;
  showNativeWebExplorer?: (bounds: NativeWebExplorerBounds) => Promise<NativeWebExplorerResult>;
  updateNativeWebExplorerBounds?: (bounds: NativeWebExplorerBounds) => Promise<NativeWebExplorerResult>;
  hideNativeWebExplorer?: () => Promise<NativeWebExplorerResult>;
  onNativeWebExplorerCodeAct?: (listener: (event: NativeWebExplorerCodeAct) => void) => () => void;
  showNativeMaps?: (bounds: NativeWebExplorerBounds) => Promise<NativeWebExplorerResult>;
  updateNativeMapsBounds?: (bounds: NativeWebExplorerBounds) => Promise<NativeWebExplorerResult>;
  hideNativeMaps?: () => Promise<NativeWebExplorerResult>;
  onNativeMapsCodeAct?: (listener: (event: NativeWebExplorerCodeAct) => void) => () => void;
  captureMapsDomRamCartography?: () => Promise<NativeDomRamCartographyResult>;
  searchCitySuggestions?: (query: string) => Promise<CitySuggestionResult>;
  openGeoEntity?: (query: string) => Promise<NativeWebExplorerResult>;
  showNativeTerminal?: (bounds: NativeTerminalBounds) => Promise<NativeTerminalResult>;
  updateNativeTerminalBounds?: (bounds: NativeTerminalBounds) => Promise<NativeTerminalResult>;
  hideNativeTerminal?: () => Promise<NativeTerminalResult>;
}

export function makeRequestId(prefix = "hdr"): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export function makeSidebarRequestId(): string {
  return makeRequestId("sbar");
}

export function makePanelsChatBottomRequestId(): string {
  return makeRequestId("pcb");
}

export function makeCanvasSurfacesRequestId(): string {
  return makeRequestId("cvs");
}

export function makeRightPanelRequestId(): string {
  return makeRequestId("rpanel");
}

export function isHeaderCommand(value: unknown): value is HeaderCommand {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<HeaderCommand>;
  if (candidate.version !== FORGE_ELECTRON_IPC_VERSION || typeof candidate.requestId !== "string") {
    return false;
  }
  if (candidate.cancelToken !== undefined && typeof candidate.cancelToken !== "string") {
    return false;
  }
  if (!HEADER_COMMAND_KIND.includes(candidate.kind as HeaderCommandKind)) {
    return false;
  }
  if (candidate.kind === "navigate_workspace") {
    return isNativeSection((candidate as { section?: unknown }).section);
  }
  return !("section" in candidate);
}

export function isNativeSection(value: unknown): value is NativeSection {
  return typeof value === "string" && NATIVE_SECTION.includes(value as NativeSection);
}

export function isProfileCanvas(value: unknown): value is ProfileCanvas {
  return typeof value === "string" && PROFILE_CANVAS.includes(value as ProfileCanvas);
}

export function isSessionsMenuMode(value: unknown): value is SessionsMenuMode {
  return typeof value === "string" && SESSIONS_MENU_MODE.includes(value as SessionsMenuMode);
}

export function isSidebarCommand(value: unknown): value is SidebarCommand {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<SidebarCommand>;
  if (candidate.version !== FORGE_ELECTRON_IPC_VERSION || typeof candidate.requestId !== "string") {
    return false;
  }
  if (candidate.cancelToken !== undefined && typeof candidate.cancelToken !== "string") {
    return false;
  }
  if (!SIDEBAR_COMMAND_KIND.includes(candidate.kind as SidebarCommandKind)) {
    return false;
  }
  switch (candidate.kind) {
    case "navigate":
      return isNativeSection((candidate as { section?: unknown }).section);
    case "open_session":
      return (
        typeof (candidate as { sessionId?: unknown }).sessionId === "string" &&
        isNativeSection((candidate as { section?: unknown }).section)
      );
    case "open_profile_canvas":
      return isProfileCanvas((candidate as { canvas?: unknown }).canvas);
    case "archive_session":
      return typeof (candidate as { sessionId?: unknown }).sessionId === "string";
    case "activate_control":
      return typeof (candidate as { label?: unknown }).label === "string";
    case "switch_sessions_mode":
      return isSessionsMenuMode((candidate as { mode?: unknown }).mode);
    case "set_active_drawer":
      return typeof (candidate as { drawer?: unknown }).drawer === "string";
    case "hide_tool":
    case "restore_tool":
      return typeof (candidate as { toolId?: unknown }).toolId === "string";
    case "pin_session":
      return (
        typeof (candidate as { sessionId?: unknown }).sessionId === "string" &&
        typeof (candidate as { label?: unknown }).label === "string" &&
        isNativeSection((candidate as { section?: unknown }).section)
      );
    case "toggle_profile_menu":
    case "confirm_archive":
    case "cancel_archive":
      return true;
    default:
      return false;
  }
}

export function isHeaderSurfaceSnapshot(value: unknown): value is HeaderSurfaceSnapshot {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<HeaderSurfaceSnapshot>;
  return (
    candidate.schema === "ingen.electron.header.surface_snapshot.v1" &&
    candidate.version === FORGE_ELECTRON_IPC_VERSION &&
    isNativeSection(candidate.activeSection) &&
    Array.isArray(candidate.surfaces) &&
    typeof candidate.proofHash === "string"
  );
}

export function isPanelsChatBottomCommand(value: unknown): value is PanelsChatBottomCommand {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<PanelsChatBottomCommand>;
  if (candidate.version !== FORGE_ELECTRON_IPC_VERSION || typeof candidate.requestId !== "string") {
    return false;
  }
  if (candidate.cancelToken !== undefined && typeof candidate.cancelToken !== "string") {
    return false;
  }
  if (!PANELS_CHAT_BOTTOM_COMMAND_KIND.includes(candidate.kind as PanelsChatBottomCommandKind)) {
    return false;
  }
  if (candidate.value !== undefined && typeof candidate.value !== "string") {
    return false;
  }
  if (candidate.userFirstName !== undefined && typeof candidate.userFirstName !== "string") {
    return false;
  }
  if (candidate.agentFirstName !== undefined && typeof candidate.agentFirstName !== "string") {
    return false;
  }
  if (candidate.moduleId !== undefined && typeof candidate.moduleId !== "string") {
    return false;
  }
  if (
    candidate.parallelSessionIndex !== undefined &&
    (!Number.isInteger(candidate.parallelSessionIndex) || candidate.parallelSessionIndex < 0 || candidate.parallelSessionIndex > 3)
  ) {
    return false;
  }
  if (
    candidate.parallelDrafts !== undefined &&
    (!Array.isArray(candidate.parallelDrafts) ||
      candidate.parallelDrafts.length === 0 ||
      candidate.parallelDrafts.length > 4 ||
      !candidate.parallelDrafts.every((draft: Partial<ParallelChatDraft>) => {
        if (!draft || typeof draft !== "object" || typeof draft.value !== "string") {
          return false;
        }
        const index = draft.parallelSessionIndex;
        return Number.isInteger(index) && typeof index === "number" && index >= 0 && index <= 3;
      }))
  ) {
    return false;
  }
  if (candidate.provider !== undefined && !["openai", "anthropic", "openrouter"].includes(candidate.provider)) {
    return false;
  }
  if (candidate.direction !== undefined && typeof candidate.direction !== "number") {
    return false;
  }
  if (
    candidate.attachmentIds !== undefined &&
    (!Array.isArray(candidate.attachmentIds) || !candidate.attachmentIds.every((id) => typeof id === "string"))
  ) {
    return false;
  }
  if (
    candidate.filePaths !== undefined &&
    (!Array.isArray(candidate.filePaths) || !candidate.filePaths.every((filePath) => typeof filePath === "string"))
  ) {
    return false;
  }
  return true;
}

export function isCanvasSurfacesCommand(value: unknown): value is CanvasSurfacesCommand {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<CanvasSurfacesCommand>;
  if (candidate.version !== FORGE_ELECTRON_IPC_VERSION || typeof candidate.requestId !== "string") {
    return false;
  }
  if (candidate.cancelToken !== undefined && typeof candidate.cancelToken !== "string") {
    return false;
  }
  if (!CANVAS_SURFACES_COMMAND_KIND.includes(candidate.kind as CanvasSurfacesCommandKind)) {
    return false;
  }
  if (candidate.target !== undefined && typeof candidate.target !== "string") {
    return false;
  }
  if (candidate.value !== undefined && typeof candidate.value !== "string") {
    return false;
  }
  if (candidate.section !== undefined && !isNativeSection(candidate.section)) {
    return false;
  }
  if (candidate.canvas !== undefined && !isProfileCanvas(candidate.canvas)) {
    return false;
  }
  return true;
}

export function isCanvasSurfacesSnapshot(value: unknown): value is CanvasSurfacesSnapshot {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<CanvasSurfacesSnapshot>;
  return (
    candidate.schema === "ingen.electron.canvas_surfaces.snapshot.v1" &&
    candidate.version === FORGE_ELECTRON_IPC_VERSION &&
    isNativeSection(candidate.activeSection) &&
    isProfileCanvas(candidate.profileCanvas) &&
    Array.isArray(candidate.surfaces) &&
    typeof candidate.activeSurfaceId === "string" &&
    typeof candidate.proofHash === "string"
  );
}

export function isRightPanelCommand(value: unknown): value is RightPanelCommand {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<RightPanelCommand>;
  if (candidate.version !== FORGE_ELECTRON_IPC_VERSION || typeof candidate.requestId !== "string") {
    return false;
  }
  if (candidate.cancelToken !== undefined && typeof candidate.cancelToken !== "string") {
    return false;
  }
  if (!RIGHT_PANEL_COMMAND_KIND.includes(candidate.kind as RightPanelCommandKind)) {
    return false;
  }
  if (candidate.target !== undefined && typeof candidate.target !== "string") {
    return false;
  }
  if (candidate.value !== undefined && typeof candidate.value !== "string") {
    return false;
  }
  return true;
}

export function isRightPanelSnapshot(value: unknown): value is RightPanelSnapshot {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<RightPanelSnapshot>;
  return (
    candidate.schema === "ingen.electron.right_panel.snapshot.v1" &&
    candidate.version === FORGE_ELECTRON_IPC_VERSION &&
    isNativeSection(candidate.activeSection) &&
    isProfileCanvas(candidate.profileCanvas) &&
    typeof candidate.open === "boolean" &&
    typeof candidate.activeTab === "string" &&
    Array.isArray(candidate.tabs) &&
    Array.isArray(candidate.lines) &&
    Array.isArray(candidate.actions) &&
    typeof candidate.proofHash === "string"
  );
}
