import { forgeSectionManifests } from "./sections/manifests.js";
import { installForgeShellUiRouter } from "./shell/click-router.js";
import { forgeIntentSurfaceContract } from "./shell/intent-surface.js";
import { ForgeTypedSectionRegistry } from "./shell/section-registry.js";
import { forgeShellStateFromKernelProjection, initialForgeShellState, reduceForgeShellState } from "./shell/shell-machine.js";
import { createForgeTauriClient } from "./shell/tauri-client.js";
import { forgeTauriCommandContracts } from "./shell/tauri-client.js";
import type { ForgeShellActionHandler, ForgeShellActionName, ForgeShellActionPayload } from "./shell/shell-actions.js";
import type { ForgeKernelProjection, ForgeSectionCell, ForgeSectionDefinition, ForgeShellEvent, ForgeShellState, ForgeTauriClient } from "./shell/types.js";

export interface ForgeShellRuntime {
  readonly tauri: ForgeTauriClient;
  register(section: ForgeSectionDefinition): ForgeSectionDefinition;
  dispatch(event: ForgeShellEvent): ForgeShellState;
  registerAction(name: ForgeShellActionName, handler: ForgeShellActionHandler): ForgeShellActionName;
  runAction(name: ForgeShellActionName, payload?: ForgeShellActionPayload): boolean;
  subscribe(listener: (state: ForgeShellState) => void): () => void;
  snapshot(): ForgeShellState & { readonly sections: readonly ForgeSectionDefinition[]; readonly cells: readonly ForgeSectionCell[] };
}

declare global {
  interface Window {
    ForgeShellRuntime?: ForgeShellRuntime;
    ForgeIntentSurfaceContract?: typeof forgeIntentSurfaceContract;
    ForgeTauriCommandContracts?: typeof forgeTauriCommandContracts;
    ForgeShellProjection?: ForgeShellState;
    ForgeSectionCells?: readonly ForgeSectionCell[];
  }
}

export function createForgeShellRuntime(): ForgeShellRuntime {
  const registry = new ForgeTypedSectionRegistry();
  const actions = new Map<ForgeShellActionName, ForgeShellActionHandler>();
  const listeners = new Set<(state: ForgeShellState) => void>();
  const tauri = createForgeTauriClient();
  let state = initialForgeShellState;
  const renderProjection = (): void => {
    document.documentElement.dataset.forgeMode = state.mode;
    document.documentElement.dataset.forgeSection = state.activeSection;
    document.documentElement.dataset.forgePhase = state.phase;
    document.documentElement.dataset.forgeLeftPanelCollapsed = state.leftPanelCollapsed ? "true" : "false";
    document.documentElement.dataset.forgeRightPanelOpen = state.rightPanel.open === true ? "true" : "false";
    document.documentElement.dataset.forgeJobsStatus = String(state.jobs.status || "");
    window.ForgeSectionCells = registry.applyState(state);
    window.ForgeShellProjection = Object.freeze({ ...state });
    for (const listener of listeners) listener(state);
  };
  const acceptProjection = (projection: ForgeKernelProjection): void => {
    state = forgeShellStateFromKernelProjection(state, projection);
    renderProjection();
  };
  syncShellFromKernel(tauri, acceptProjection);

  return Object.freeze({
    tauri,
    register: (section: ForgeSectionDefinition) => registry.register(section),
    dispatch: (event: ForgeShellEvent) => {
      dispatchShellIntentToKernel(tauri, event, acceptProjection, () => {
        state = reduceForgeShellState(state, event);
        renderProjection();
      });
      return state;
    },
    registerAction: (name: ForgeShellActionName, handler: ForgeShellActionHandler) => {
      actions.set(name, handler);
      return name;
    },
    runAction: (name: ForgeShellActionName, payload: ForgeShellActionPayload = {}) => {
      const handler = actions.get(name);
      if (!handler) return false;
      handler(payload);
      return true;
    },
    subscribe: (listener: (state: ForgeShellState) => void) => {
      listeners.add(listener);
      listener(state);
      return () => {
        listeners.delete(listener);
      };
    },
    snapshot: () => Object.freeze({ ...state, sections: registry.list(), cells: registry.cells() }),
  });
}

function dispatchShellIntentToKernel(
  tauri: ForgeTauriClient,
  event: ForgeShellEvent,
  acceptProjection: (projection: ForgeKernelProjection) => void,
  fallbackRender: () => void,
): void {
  let op = "";
  let payload: Record<string, unknown> = {};
  if (event.type === "ACTIVATE_SECTION") {
    op = "activate_section";
    payload = { section: event.section };
  } else if (event.type === "SET_SECTION_ACTIVE") {
    op = "set_section_active";
    payload = { section: event.section, active: event.active };
  } else if (event.type === "SET_SURFACE_ACTIVE") {
    op = "set_surface_active";
    payload = { section: event.section, active: event.active, fallbackSection: event.fallbackSection || "" };
  } else if (event.type === "SET_MODE") {
    op = "set_mode";
    payload = { mode: event.mode };
  } else if (event.type === "SET_REAL_ESTATE_MODE") {
    op = "set_real_estate_mode";
    payload = { active: event.active, webExplorerActive: event.webExplorerActive === true };
  } else if (event.type === "BOOT_READY") {
    op = "boot_ready";
  } else if (event.type === "BOOT_ERROR") {
    op = "boot_error";
  } else if (event.type === "TOGGLE_LEFT_PANEL") {
    op = "toggle_left_panel";
  } else if (event.type === "SET_CANVAS") {
    op = "set_canvas";
    payload = { ...event.patch };
  } else if (event.type === "SET_CHATBAR") {
    op = "set_chatbar";
    payload = { ...event.patch };
  } else if (event.type === "SET_RIGHT_PANEL") {
    op = "set_right_panel";
    payload = { ...event.patch };
  } else if (event.type === "SET_JOBS") {
    op = "set_jobs";
    payload = { ...event.patch };
  } else if (event.type === "SET_HARDWARE") {
    op = "hardware_observed";
    payload = { hardware: event.hardware };
  } else if (event.type === "SET_PANEL") {
    op = "set_panel";
    payload = { panel: event.panel, open: event.open };
  } else if (event.type === "SET_OVERLAY") {
    op = "set_overlay";
    payload = { overlay: event.overlay, open: event.open };
  } else if (event.type === "SET_WINDOW_COMMAND") {
    op = "window_control";
    payload = { command: event.command, label: event.label || "" };
  } else if (event.type === "SET_ONBOARDING") {
    op = "set_onboarding";
    payload = { scope: event.scope, status: event.status, questionId: event.questionId || "" };
  } else {
    return;
  }
  void tauri
    .invoke<ForgeKernelProjection>(
      "forge_kernel",
      { op, payload, source: "ui-intent", eventType: event.type },
      { bootSafe: true, requiresActiveSection: false, trace: true },
    )
    .then(acceptProjection)
    .catch(fallbackRender);
}

function syncShellFromKernel(
  tauri: ForgeTauriClient,
  acceptProjection: (projection: ForgeKernelProjection) => void,
): void {
  void tauri
    .invoke<ForgeKernelProjection>("forge_kernel", { op: "snapshot", payload: {} }, { bootSafe: true, requiresActiveSection: false })
    .then(acceptProjection)
    .catch(() => {});
}

window.ForgeShellRuntime = createForgeShellRuntime();
window.ForgeIntentSurfaceContract = forgeIntentSurfaceContract;
window.ForgeTauriCommandContracts = forgeTauriCommandContracts;
window.ForgeSectionManifests = forgeSectionManifests;
for (const section of forgeSectionManifests) {
  window.ForgeShellRuntime.register(section);
}
window.__forgeActiveSection = () => window.ForgeShellRuntime?.snapshot().activeSection || "alpha";
window.__forgeSwitchSection = (section) => {
  if (
    section === "alpha"
    || section === "forge"
    || section === "webexplorer"
    || section === "real-estate"
    || section === "real-estate-main"
    || section === "trading"
    || section === "banger"
  ) {
    window.ForgeShellRuntime?.dispatch({ type: "ACTIVATE_SECTION", section });
  }
};
installForgeShellUiRouter(window.ForgeShellRuntime);
