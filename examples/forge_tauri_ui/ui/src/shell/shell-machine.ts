import type { ForgeKernelProjection, ForgeSectionId, ForgeShellEvent, ForgeShellMode, ForgeShellState } from "./types.js";

export const initialForgeShellState: ForgeShellState = Object.freeze({
  phase: "boot",
  mode: "forge",
  activeSection: "alpha",
  activeSections: Object.freeze({
    alpha: true,
  }),
  leftPanelCollapsed: false,
  canvas: Object.freeze({}),
  chatbar: Object.freeze({}),
  rightPanel: Object.freeze({}),
  jobs: Object.freeze({}),
  hardware: null,
  panels: Object.freeze({}),
  overlays: Object.freeze({}),
  window: Object.freeze({ lastCommand: "", label: "" }),
  onboarding: Object.freeze({ scope: "", status: "idle", questionId: "" }),
});

function homeSectionForMode(mode: ForgeShellMode): ForgeSectionId {
  return mode === "agence-immo" ? "real-estate-main" : "alpha";
}

export function reduceForgeShellState(
  state: ForgeShellState,
  event: ForgeShellEvent,
): ForgeShellState {
  switch (event.type) {
    case "BOOT_READY":
      return { ...state, phase: "ready" };
    case "BOOT_ERROR":
      return { ...state, phase: "error" };
    case "SET_MODE": {
      const activeSection = event.mode === state.mode ? state.activeSection : homeSectionForMode(event.mode);
      return {
        ...state,
        mode: event.mode,
        activeSection,
        activeSections: Object.freeze({
          ...state.activeSections,
          "real-estate": event.mode === "agence-immo",
          "real-estate-main": event.mode === "agence-immo",
        }),
        panels: event.mode === "agence-immo" ? state.panels : closeRealEstatePanels(state.panels),
        overlays: event.mode === "agence-immo" ? state.overlays : closeRealEstatePanels(state.overlays),
        onboarding: event.mode === "agence-immo" ? state.onboarding : Object.freeze({ scope: "real-estate", status: "idle", questionId: "" }),
      };
    }
    case "SET_REAL_ESTATE_MODE": {
      const mode: ForgeShellMode = event.active ? "agence-immo" : "forge";
      const activeSection = event.active
        ? (event.webExplorerActive ? "webexplorer" : "real-estate-main")
        : "alpha";
      return {
        ...state,
        mode,
        activeSection,
        activeSections: Object.freeze({
          ...state.activeSections,
          "real-estate": event.active,
          "real-estate-main": event.active && !event.webExplorerActive,
          alpha: !event.active,
          [activeSection]: true,
        }),
        panels: event.active ? state.panels : closeRealEstatePanels(state.panels),
        overlays: event.active ? state.overlays : closeRealEstatePanels(state.overlays),
        onboarding: event.active ? state.onboarding : Object.freeze({ scope: "real-estate", status: "idle", questionId: "" }),
      };
    }
    case "ACTIVATE_SECTION":
      return {
        ...state,
        activeSection: event.section,
        activeSections: Object.freeze({
          ...state.activeSections,
          [event.section]: true,
        }),
      };
    case "SET_SECTION_ACTIVE":
      return {
        ...state,
        activeSections: Object.freeze({
          ...state.activeSections,
          [event.section]: event.active,
        }),
      };
    case "SET_SURFACE_ACTIVE": {
      const activeSection = event.active
        ? event.section
        : event.fallbackSection || homeSectionForMode(state.mode);
      return {
        ...state,
        activeSection,
        activeSections: Object.freeze({
          ...state.activeSections,
          [event.section]: event.active,
          [activeSection]: true,
        }),
      };
    }
    case "TOGGLE_LEFT_PANEL":
      return { ...state, leftPanelCollapsed: !state.leftPanelCollapsed };
    case "SET_CANVAS":
      return {
        ...state,
        canvas: Object.freeze({ ...state.canvas, ...event.patch }),
      };
    case "SET_CHATBAR":
      return {
        ...state,
        chatbar: Object.freeze({ ...state.chatbar, ...event.patch }),
      };
    case "SET_RIGHT_PANEL":
      return {
        ...state,
        rightPanel: Object.freeze({ ...state.rightPanel, ...event.patch }),
      };
    case "SET_JOBS":
      return {
        ...state,
        jobs: Object.freeze({ ...state.jobs, ...event.patch }),
      };
    case "SET_HARDWARE":
      return {
        ...state,
        hardware: event.hardware,
      };
    case "SET_PANEL":
      return {
        ...state,
        panels: Object.freeze({ ...state.panels, [event.panel]: event.open }),
      };
    case "SET_OVERLAY":
      return {
        ...state,
        overlays: Object.freeze({ ...state.overlays, [event.overlay]: event.open }),
      };
    case "SET_WINDOW_COMMAND":
      return {
        ...state,
        window: Object.freeze({ lastCommand: event.command, label: event.label || "" }),
      };
    case "SET_ONBOARDING":
      return {
        ...state,
        onboarding: Object.freeze({
          scope: event.scope,
          status: event.status,
          questionId: event.questionId || "",
        }),
      };
    default: {
      const neverEvent: never = event;
      return neverEvent;
    }
  }
}

export function forgeShellStateFromKernelProjection(
  state: ForgeShellState,
  projection: ForgeKernelProjection,
): ForgeShellState {
  const projectionShell = projection.shell || {};
  const projectionSection = projection.section || {};
  const projectionLeftPanel = projection.leftPanel || projection.left_panel || {};
  const mode: ForgeShellMode = projectionShell.mode === "agence-immo" || projection.mode === "agence-immo" ? "agence-immo" : "forge";
  const phase = normalizePhase(projectionShell.phase || state.phase);
  const section = normalizeSectionId(
    projectionSection.active
      || projectionSection.activeSection
      || projectionSection.active_section
      || projection.activeSection
      || projection.active_section
      || state.activeSection,
  );
  const activeSectionList = projectionSection.activeSections
    || projectionSection.active_sections
    || projection.activeSections
    || projection.active_sections
    || [];
  const activeSections = Object.freeze(
    activeSectionList.reduce<Record<string, boolean>>((acc, item) => {
      acc[item] = true;
      return acc;
    }, { [section]: true }),
  );
  return {
    ...state,
    phase,
    mode,
    activeSection: section,
    activeSections,
    leftPanelCollapsed: Boolean(
      projectionLeftPanel.collapsed
        ?? projection.leftPanelCollapsed
        ?? projection.left_panel_collapsed
        ?? state.leftPanelCollapsed,
    ),
    canvas: Object.freeze({ ...state.canvas, ...(projection.canvas || {}) }),
    chatbar: Object.freeze({ ...state.chatbar, ...(projection.chatbar || {}) }),
    rightPanel: Object.freeze({ ...state.rightPanel, ...(projection.rightPanel || projection.right_panel || {}) }),
    jobs: Object.freeze({ ...state.jobs, ...(projection.jobs || {}) }),
    hardware: projection.hardware ?? state.hardware,
    panels: Object.freeze({ ...state.panels, ...(projection.panels || {}) }),
    overlays: Object.freeze({ ...state.overlays, ...(projection.overlays || {}) }),
    window: Object.freeze({
      lastCommand: projection.lastWindowControl || projection.last_window_control || state.window.lastCommand,
      label: projection.lastWindowLabel || projection.last_window_label || state.window.label,
    }),
    onboarding: Object.freeze({
      scope: projection.onboarding?.scope || state.onboarding.scope,
      status: normalizeOnboardingStatus(projection.onboarding?.status || state.onboarding.status),
      questionId: projection.onboarding?.questionId || projection.onboarding?.question_id || state.onboarding.questionId,
    }),
  };
}

function normalizePhase(phase: string): ForgeShellState["phase"] {
  if (phase === "ready" || phase === "error") return phase;
  return "boot";
}

function closeRealEstatePanels(panels: Readonly<Record<string, boolean>>): Readonly<Record<string, boolean>> {
  return Object.freeze({
    ...panels,
    "real-estate-tools": false,
    "real-estate-contacts": false,
  });
}

function normalizeOnboardingStatus(status: string): ForgeShellState["onboarding"]["status"] {
  if (status === "initializing" || status === "asking" || status === "complete" || status === "error") return status;
  return "idle";
}

function normalizeSectionId(section: string): ForgeSectionId {
  if (
    section === "forge"
    || section === "webexplorer"
    || section === "real-estate"
    || section === "real-estate-main"
    || section === "trading"
    || section === "banger"
  ) {
    return section;
  }
  return "alpha";
}
