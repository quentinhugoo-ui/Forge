import type { ForgeShellActionName, ForgeShellActionPayload } from "./shell-actions.js";

export interface ForgeShellActionRuntime {
  runAction(name: ForgeShellActionName, payload?: ForgeShellActionPayload): boolean;
}

const shellClickActions = Object.freeze([
  ["#alphaSidebarToggle", "toggle-sidebar"],
  ["#alphaProofToggle", "toggle-right-panel"],
  ["#alphaProofClose", "close-right-panel"],
  ["#profileBtn", "profile-toggle"],
  ["[data-profile-action]", "profile-action"],
  ["#docsClose", "docs-close"],
  ["#navForge", "nav-forge"],
  ["#navAlpha", "nav-alpha"],
  ["#realEstateModeBtn", "toggle-real-estate"],
  ["#realEstateHomeSectionBtn", "real-estate-home"],
  ["#webexplorer", "toggle-webexplorer"],
  ["#tradingWorkspaceBtn", "open-trading"],
  ["#bangerBoomBtn", "toggle-banger"],
  ["#bangerExitBtn", "close-banger"],
  ["#forgeSearchBtn", "open-search"],
  ["#forgeSearchClose", "close-search"],
  ["#forgeSearchBackdrop", "close-search"],
  ["#realEstateNewSessionPanelBtn", "real-estate-new-session"],
  ["#realEstateToolsPanelBtn", "toggle-real-estate-tools"],
  ["#realEstateToolsCloseBtn", "close-real-estate-tools"],
  ["#realEstateCrmPanelBtn", "toggle-real-estate-contacts"],
  ["#realEstateCrmCloseBtn", "close-real-estate-contacts"],
  ["#realEstateAutomationsPanelBtn", "real-estate-automations"],
  ["#realEstatePropertiesPanelBtn", "real-estate-properties"],
  ["#providerClose", "provider-close"],
  ["#providerLauncherCodex", "provider-launch-codex"],
  ["#providerLauncherGemini", "provider-launch-gemini"],
  ["#providerLauncherClaude", "provider-launch-claude"],
  ["#providerLauncherOanda", "provider-launch-oanda"],
  ["#providerWorkbenchLaunch", "provider-workbench-launch"],
  ["#providerWorkbenchRefresh", "provider-workbench-refresh"],
  ["#openAiProviderRefresh", "provider-refresh-all"],
  ["#oandaProviderTerminalSend", "provider-oanda-send"],
  ["#oandaProviderTerminalReset", "provider-oanda-reset"],
  ["#openAiProviderConnect", "provider-openai-connect"],
  ["#geminiProviderSaveKey", "provider-gemini-save-key"],
  ["#geminiProviderConnect", "provider-gemini-connect"],
  ["#geminiProviderRefresh", "provider-gemini-refresh"],
  ["#geminiProviderClearKey", "provider-gemini-clear-key"],
  ["#claudeProviderConnect", "provider-claude-connect"],
  ["#claudeProviderRefresh", "provider-claude-refresh"],
  ["#voiceElevenSaveKey", "provider-voice-save-key"],
  ["#voiceElevenClearKey", "provider-voice-clear-key"],
  ["#navLibraryBtn", "library-toggle"],
  ["#libraryClose", "library-close"],
  ["#navMcpBtn", "mcp-toggle"],
  ["#mcpClose", "mcp-close"],
  ["#navProgramsBtn", "programs-toggle"],
  ["#programsClose", "programs-close"],
  ["#programCreateToggle", "program-create-toggle"],
  ["#programCreateCancel", "program-create-cancel"],
  ["#myAtlasRefresh", "atlas-refresh"],
  ["#alpha3dToggle", "alpha-3d-toggle"],
  ["#marsLensToggle", "mars-lens-toggle"],
  ["#marsLensClose", "mars-lens-close"],
  ["#alphaSplitViewToggle", "alpha-split-toggle"],
  ["#alpha3dNewMap", "alpha-3d-new-map"],
  [".lib-filter-btn", "library-filter"],
  [".mcp-filter-btn", "mcp-filter"],
  [".programs-filter-btn", "programs-filter"],
  [".atlas-subtab", "atlas-subtab"],
] satisfies readonly (readonly [string, ForgeShellActionName])[]);

type ShellShortcutAction = {
  readonly key: string;
  readonly ctrlOrMeta?: boolean;
  readonly alt?: boolean;
  readonly shift?: boolean;
  readonly action: ForgeShellActionName;
};

const shellShortcutActions: readonly ShellShortcutAction[] = Object.freeze([
  { key: "k", ctrlOrMeta: true, action: "toggle-search" },
  { key: "escape", action: "escape-overlays" },
]);

export function installForgeShellUiRouter(runtime: ForgeShellActionRuntime): void {
  if (document.documentElement?.dataset.forgeShellUiRouterInstalled === "true") return;
  document.documentElement.dataset.forgeShellUiRouterInstalled = "true";
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    runtime.runAction("document-click", {
      source: "document",
      kind: "click",
      target,
    });
    for (const [selector, action] of shellClickActions) {
      const button = target.closest(selector);
      if (!button) continue;
      event.preventDefault();
      event.stopPropagation();
      runtime.runAction(action, {
        source: selector,
        kind: "click",
        dataset: datasetFor(button),
      });
      return;
    }
  });
  window.addEventListener("keydown", (event) => {
    for (const shortcut of shellShortcutActions) {
      if (!matchesShortcut(event, shortcut)) continue;
      event.preventDefault();
      event.stopPropagation();
      runtime.runAction(shortcut.action, {
        source: "keyboard",
        kind: "shortcut",
        key: shortcut.key,
      });
      return;
    }
  });
}

export const installForgeShellClickRouter = installForgeShellUiRouter;

function datasetFor(element: Element): Readonly<Record<string, string>> {
  if (!(element instanceof HTMLElement)) return Object.freeze({});
  const dataset: Record<string, string> = {};
  for (const [key, value] of Object.entries(element.dataset)) {
    if (typeof value === "string") dataset[key] = value;
  }
  return Object.freeze(dataset);
}

function matchesShortcut(
  event: KeyboardEvent,
  shortcut: ShellShortcutAction,
): boolean {
  const key = event.key.toLowerCase();
  if (key !== shortcut.key) return false;
  if (Boolean(shortcut.ctrlOrMeta) !== Boolean(event.ctrlKey || event.metaKey)) return false;
  if (Boolean(shortcut.alt) !== event.altKey) return false;
  if (Boolean(shortcut.shift) !== event.shiftKey) return false;
  return true;
}
