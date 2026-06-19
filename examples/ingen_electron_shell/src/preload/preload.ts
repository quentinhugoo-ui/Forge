import { contextBridge, ipcRenderer } from "electron";
import type {
  CanvasSurfacesCommand,
  ForgeShellApi,
  ForgeTerminalApi,
  HeaderCommand,
  LlmProviderConnectId,
  AgentRuntimeEvent,
  PanelsChatBottomCommand,
  PanelsChatBottomSnapshotEvent,
  RightPanelCommand,
  AgentActionRequest,
  SearchArchiveRequest,
  SidebarCommand,
  NativeTerminalBounds,
  NativeWebExplorerCodeAct,
  NativeWebExplorerBounds,
  WidgetWallpaperSampleBounds,
  BrainCanonicalMemorySnapshot
} from "../shared/ipc-contract.js";

const forgeShell: ForgeShellApi = {
  getCutover(slice) {
    return ipcRenderer.invoke("forge:get-cutover", slice);
  },
  getHeaderSnapshot() {
    return ipcRenderer.invoke("forge:get-header-snapshot");
  },
  getHeaderSurfaceSnapshot() {
    return ipcRenderer.invoke("forge:get-header-surface-snapshot");
  },
  dispatchHeaderCommand(command: HeaderCommand) {
    return ipcRenderer.invoke("forge:dispatch-header-command", command);
  },
  getSidebarSnapshot() {
    return ipcRenderer.invoke("forge:get-sidebar-snapshot");
  },
  dispatchSidebarCommand(command: SidebarCommand) {
    return ipcRenderer.invoke("forge:dispatch-sidebar-command", command);
  },
  getPanelsChatBottomSnapshot() {
    return ipcRenderer.invoke("forge:get-panels-chat-bottom-snapshot");
  },
  dispatchPanelsChatBottomCommand(command: PanelsChatBottomCommand) {
    return ipcRenderer.invoke("forge:dispatch-panels-chat-bottom-command", command);
  },
  getCanvasSurfacesSnapshot() {
    return ipcRenderer.invoke("forge:get-canvas-surfaces-snapshot");
  },
  getBangerPreviewFrame() {
    return ipcRenderer.invoke("forge:get-banger-preview-frame");
  },
  getBangerPresentLoopBootstrap(request?: unknown) {
    return ipcRenderer.invoke("forge:get-banger-present-loop-bootstrap", request);
  },
  getBangerGoogleTilesConfig() {
    return ipcRenderer.invoke("forge:get-banger-google-tiles-config");
  },
  showNativeBanger(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:banger-show", bounds);
  },
  updateNativeBangerBounds(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:banger-bounds", bounds);
  },
  hideNativeBanger() {
    return ipcRenderer.invoke("forge:banger-hide");
  },
  dispatchCanvasSurfacesCommand(command: CanvasSurfacesCommand) {
    return ipcRenderer.invoke("forge:dispatch-canvas-surfaces-command", command);
  },
  getRightPanelSnapshot() {
    return ipcRenderer.invoke("forge:get-right-panel-snapshot");
  },
  dispatchRightPanelCommand(command: RightPanelCommand) {
    return ipcRenderer.invoke("forge:dispatch-right-panel-command", command);
  },
  connectLlmProvider(provider: LlmProviderConnectId) {
    return ipcRenderer.invoke("forge:connect-llm-provider", provider);
  },
  resetLlmProvider(provider: LlmProviderConnectId) {
    return ipcRenderer.invoke("forge:reset-llm-provider", provider);
  },
  getLlmProviderRuntimeSnapshot() {
    return ipcRenderer.invoke("forge:get-llm-provider-runtime-snapshot");
  },
  searchArchive(request: SearchArchiveRequest) {
    return ipcRenderer.invoke("forge:search-archive", request);
  },
  getSessionFilesSnapshot() {
    return ipcRenderer.invoke("forge:get-session-files-snapshot");
  },
  getBrainMemorySnapshot() {
    return ipcRenderer.invoke("forge:get-brain-memory-snapshot");
  },
  mergeBrainMemorySnapshot(snapshot: BrainCanonicalMemorySnapshot) {
    return ipcRenderer.invoke("forge:merge-brain-memory-snapshot", snapshot);
  },
  onLlmProviderEvent(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as Parameters<typeof listener>[0]);
    };
    ipcRenderer.on("forge:llm-provider-event", handler);
    return () => ipcRenderer.removeListener("forge:llm-provider-event", handler);
  },
  onAgentRuntimeEvent(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as AgentRuntimeEvent);
    };
    ipcRenderer.on("forge:agent-runtime-event", handler);
    return () => ipcRenderer.removeListener("forge:agent-runtime-event", handler);
  },
  onPanelsChatBottomSnapshotEvent(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as PanelsChatBottomSnapshotEvent);
    };
    ipcRenderer.on("forge:panels-chat-bottom-snapshot-event", handler);
    return () => ipcRenderer.removeListener("forge:panels-chat-bottom-snapshot-event", handler);
  },
  chooseWorkspaceFolder() {
    return ipcRenderer.invoke("forge:choose-workspace-folder");
  },
  getWorkspaceFolder() {
    return ipcRenderer.invoke("forge:get-workspace-folder");
  },
  getAgentActionHostManifest() {
    return ipcRenderer.invoke("forge:get-agent-action-host-manifest");
  },
  executeAgentAction(request: AgentActionRequest) {
    return ipcRenderer.invoke("forge:execute-agent-action", request);
  },
  getHardwareTelemetrySnapshot() {
    return ipcRenderer.invoke("forge:get-hardware-telemetry-snapshot");
  },
  showWorkspaceInExplorer() {
    return ipcRenderer.invoke("forge:show-workspace-in-explorer");
  },
  copyWorkspacePath() {
    return ipcRenderer.invoke("forge:copy-workspace-path");
  },
  copyWorkspaceBranchName() {
    return ipcRenderer.invoke("forge:copy-workspace-branch-name");
  },
  showNativeWebExplorer(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:webexplorer-show", bounds);
  },
  updateNativeWebExplorerBounds(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:webexplorer-bounds", bounds);
  },
  hideNativeWebExplorer() {
    return ipcRenderer.invoke("forge:webexplorer-hide");
  },
  onNativeWebExplorerCodeAct(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as NativeWebExplorerCodeAct);
    };
    ipcRenderer.on("forge:webexplorer-codeact", handler);
    return () => ipcRenderer.removeListener("forge:webexplorer-codeact", handler);
  },
  showNativeMaps(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:maps-show", bounds);
  },
  updateNativeMapsBounds(bounds: NativeWebExplorerBounds) {
    return ipcRenderer.invoke("forge:maps-bounds", bounds);
  },
  hideNativeMaps() {
    return ipcRenderer.invoke("forge:maps-hide");
  },
  captureMapsDomRamCartography() {
    return ipcRenderer.invoke("forge:maps-dom-ram-cartography-capture");
  },
  onNativeMapsCodeAct(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as NativeWebExplorerCodeAct);
    };
    ipcRenderer.on("forge:maps-codeact", handler);
    return () => ipcRenderer.removeListener("forge:maps-codeact", handler);
  },
  searchCitySuggestions(query: string) {
    return ipcRenderer.invoke("forge:search-city-suggestions", query);
  },
  openGeoEntity(query: string) {
    return ipcRenderer.invoke("forge:maps-open-geo-entity", query);
  },
  showNativeTerminal(bounds: NativeTerminalBounds) {
    return ipcRenderer.invoke("forge:terminal-show-native", bounds);
  },
  updateNativeTerminalBounds(bounds: NativeTerminalBounds) {
    return ipcRenderer.invoke("forge:terminal-bounds-native", bounds);
  },
  hideNativeTerminal() {
    return ipcRenderer.invoke("forge:terminal-hide-native");
  }
};

const forgeTerminal: ForgeTerminalApi = {
  start() {
    return ipcRenderer.invoke("forge:terminal-start");
  },
  write(data: string) {
    return ipcRenderer.invoke("forge:terminal-write", data);
  },
  resize(cols: number, rows: number) {
    return ipcRenderer.invoke("forge:terminal-resize", { cols, rows });
  },
  stop() {
    return ipcRenderer.invoke("forge:terminal-stop");
  },
  onEvent(listener) {
    const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
      listener(payload as Parameters<typeof listener>[0]);
    };
    ipcRenderer.on("forge:terminal-event", handler);
    return () => ipcRenderer.removeListener("forge:terminal-event", handler);
  }
};

const forgeWindowControls = {
  minimize(): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-minimize");
  },
  toggleMaximize(): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-toggle-maximize");
  },
  setWidgetMode(enabled: boolean, delayMs?: number): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-mode", enabled, delayMs);
  },
  setWidgetHitRegions(regions: Array<{ x: number; y: number; width: number; height: number }>): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-hit-regions", regions);
  },
  setWidgetPanelExpanded(enabled: boolean): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-panel-expanded", enabled);
  },
  setWidgetClickThrough(enabled: boolean): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-click-through", enabled);
  },
  setWidgetTaskbarAutoHide(enabled: boolean): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-taskbar-autohide", enabled);
  },
  toggleWidgetTaskbar(): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-widget-taskbar-toggle");
  },
  sampleWidgetWallpaper(bounds: WidgetWallpaperSampleBounds) {
    return ipcRenderer.invoke("forge:window-widget-wallpaper-sample", bounds);
  },
  close(): Promise<boolean> {
    return ipcRenderer.invoke("forge:window-close");
  }
};

contextBridge.exposeInMainWorld("forgeShell", forgeShell);
contextBridge.exposeInMainWorld("forgeTerminal", forgeTerminal);
contextBridge.exposeInMainWorld("forgeWindowControls", forgeWindowControls);
