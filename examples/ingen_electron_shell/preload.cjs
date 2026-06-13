const { contextBridge, ipcRenderer } = require("electron");

const forgeShell = {
  getCutover(slice) {
    return ipcRenderer.invoke("forge:get-cutover", slice);
  },
  getHeaderSnapshot() {
    return ipcRenderer.invoke("forge:get-header-snapshot");
  },
  getHeaderSurfaceSnapshot() {
    return ipcRenderer.invoke("forge:get-header-surface-snapshot");
  },
  dispatchHeaderCommand(command) {
    return ipcRenderer.invoke("forge:dispatch-header-command", command);
  },
  getSidebarSnapshot() {
    return ipcRenderer.invoke("forge:get-sidebar-snapshot");
  },
  dispatchSidebarCommand(command) {
    return ipcRenderer.invoke("forge:dispatch-sidebar-command", command);
  },
  getPanelsChatBottomSnapshot() {
    return ipcRenderer.invoke("forge:get-panels-chat-bottom-snapshot");
  },
  dispatchPanelsChatBottomCommand(command) {
    return ipcRenderer.invoke("forge:dispatch-panels-chat-bottom-command", command);
  },
  getCanvasSurfacesSnapshot() {
    return ipcRenderer.invoke("forge:get-canvas-surfaces-snapshot");
  },
  dispatchCanvasSurfacesCommand(command) {
    return ipcRenderer.invoke("forge:dispatch-canvas-surfaces-command", command);
  },
  getRightPanelSnapshot() {
    return ipcRenderer.invoke("forge:get-right-panel-snapshot");
  },
  dispatchRightPanelCommand(command) {
    return ipcRenderer.invoke("forge:dispatch-right-panel-command", command);
  },
  connectLlmProvider(provider) {
    return ipcRenderer.invoke("forge:connect-llm-provider", provider);
  },
  resetLlmProvider(provider) {
    return ipcRenderer.invoke("forge:reset-llm-provider", provider);
  },
  getLlmProviderRuntimeSnapshot() {
    return ipcRenderer.invoke("forge:get-llm-provider-runtime-snapshot");
  },
  searchArchive(request) {
    return ipcRenderer.invoke("forge:search-archive", request);
  },
  getSessionFilesSnapshot() {
    return ipcRenderer.invoke("forge:get-session-files-snapshot");
  },
  onLlmProviderEvent(listener) {
    const handler = (_event, payload) => listener(payload);
    ipcRenderer.on("forge:llm-provider-event", handler);
    return () => ipcRenderer.removeListener("forge:llm-provider-event", handler);
  },
  chooseWorkspaceFolder() {
    return ipcRenderer.invoke("forge:choose-workspace-folder");
  },
  getWorkspaceFolder() {
    return ipcRenderer.invoke("forge:get-workspace-folder");
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
  showNativeWebExplorer(bounds) {
    return ipcRenderer.invoke("forge:webexplorer-show", bounds);
  },
  updateNativeWebExplorerBounds(bounds) {
    return ipcRenderer.invoke("forge:webexplorer-bounds", bounds);
  },
  hideNativeWebExplorer() {
    return ipcRenderer.invoke("forge:webexplorer-hide");
  },
  onNativeWebExplorerCodeAct(listener) {
    const handler = (_event, payload) => listener(payload);
    ipcRenderer.on("forge:webexplorer-codeact", handler);
    return () => ipcRenderer.removeListener("forge:webexplorer-codeact", handler);
  },
  showNativeMaps(bounds) {
    return ipcRenderer.invoke("forge:maps-show", bounds);
  },
  updateNativeMapsBounds(bounds) {
    return ipcRenderer.invoke("forge:maps-bounds", bounds);
  },
  hideNativeMaps() {
    return ipcRenderer.invoke("forge:maps-hide");
  },
  onNativeMapsCodeAct(listener) {
    const handler = (_event, payload) => listener(payload);
    ipcRenderer.on("forge:maps-codeact", handler);
    return () => ipcRenderer.removeListener("forge:maps-codeact", handler);
  }
};

const forgeTerminal = {
  start() {
    return ipcRenderer.invoke("forge:terminal-start");
  },
  write(data) {
    return ipcRenderer.invoke("forge:terminal-write", data);
  },
  resize(cols, rows) {
    return ipcRenderer.invoke("forge:terminal-resize", { cols, rows });
  },
  stop() {
    return ipcRenderer.invoke("forge:terminal-stop");
  },
  onEvent(listener) {
    const handler = (_event, payload) => listener(payload);
    ipcRenderer.on("forge:terminal-event", handler);
    return () => ipcRenderer.removeListener("forge:terminal-event", handler);
  }
};

const forgeWindowControls = {
  minimize() {
    console.info("[preload] forge:window-minimize");
    return ipcRenderer.invoke("forge:window-minimize");
  },
  toggleMaximize() {
    console.info("[preload] forge:window-toggle-maximize");
    return ipcRenderer.invoke("forge:window-toggle-maximize");
  },
  close() {
    console.info("[preload] forge:window-close");
    return ipcRenderer.invoke("forge:window-close");
  }
};

contextBridge.exposeInMainWorld("forgeShell", forgeShell);
contextBridge.exposeInMainWorld("forgeTerminal", forgeTerminal);
contextBridge.exposeInMainWorld("forgeWindowControls", forgeWindowControls);
