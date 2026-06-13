import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  type ForgeShellApi,
  type HeaderCommand,
  type HeaderCommandKind,
  type HeaderCommandResult,
  type HeaderControl,
  type HeaderSnapshot,
  type NativeSection,
  isNativeSection,
  makeRequestId
} from "../shared/ipc-contract";

export interface HeaderShadowManifest {
  schema: "ingen.electron.header_shadow_manifest.v1";
  sliceId: "header";
  mode: HeaderSnapshot["mode"];
  activeSection: NativeSection;
  sectionTitle: string;
  profileCanvas: HeaderSnapshot["profileCanvas"];
  leftPanelOpen: boolean;
  rightPanelOpen: boolean;
  visibleControlIds: string[];
  selectedControlIds: string[];
  availableActions: HeaderCommandKind[];
  focusTarget: string;
  lastEvent: HeaderCommandResult["event"] | "boot";
  lastRequestId: string;
  snapshotProofHash: string;
  ipcProofHash: string;
  interactionCount: number;
}

export interface HeaderShadowState {
  snapshot: HeaderSnapshot;
  manifest: HeaderShadowManifest;
  lastProof: string;
  booted: boolean;
}

export const fallbackSnapshot: HeaderSnapshot = {
  schema: "ingen.electron.header.snapshot.v1",
  version: FORGE_ELECTRON_IPC_VERSION,
  mode: "shadow",
  activeSection: "forge",
  sectionTitle: "Forge",
  profileCanvas: "",
  leftPanelOpen: true,
  rightPanelOpen: false,
  macChrome: false,
  cpuLabel: "local",
  gpuLabel: "native",
  topControls: [
    {
      id: "left-panel",
      label: "Toggle left panel",
      icon: "panel-left",
      command: "toggle_left_panel",
      selected: true,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "sessions",
      label: "Search sessions",
      icon: "search",
      command: "open_sessions_canvas",
      route: "sessions",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "webexplorer-top",
      label: "Open WebExplorer",
      icon: "globe",
      command: "open_webexplorer",
      route: "webexplorer",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "banger-top",
      label: "Open Banger",
      icon: "box",
      command: "open_banger",
      route: "banger",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "trading-top",
      label: "Open Trading",
      icon: "chart",
      command: "open_trading",
      route: "trading",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "window-minimize",
      label: "Minimize",
      icon: "minus",
      command: "window_minimize",
      selected: false,
      visible: true,
      nativeAuthority: "window"
    },
    {
      id: "window-maximize",
      label: "Maximize",
      icon: "square",
      command: "window_toggle_maximize",
      selected: false,
      visible: true,
      nativeAuthority: "window"
    },
    {
      id: "window-close",
      label: "Close",
      icon: "x",
      command: "window_close",
      selected: false,
      visible: true,
      nativeAuthority: "window"
    }
  ],
  workspaceControls: [
    {
      id: "plan",
      label: "Plan",
      icon: "plan",
      command: "toggle_right_panel",
      route: "right-panel",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "webexplorer-workspace",
      label: "WebExplorer workspace",
      icon: "globe",
      command: "navigate_workspace",
      route: "webexplorer",
      selected: false,
      visible: true,
      nativeAuthority: "electron-shadow"
    },
    {
      id: "right-panel",
      label: "Split canvas",
      icon: "panel-right",
      command: "toggle_right_panel",
      route: "right-panel",
      selected: true,
      visible: true,
      nativeAuthority: "electron-shadow"
    }
  ],
  nativeSurfaceContracts: {
    banger: "native-child-surface",
    webexplorer: "rust-owned-webview"
  },
  proofHash: "fallback"
};

interface StoreInternals {
  snapshot: HeaderSnapshot;
  lastProof: string;
  lastEvent: HeaderShadowManifest["lastEvent"];
  lastRequestId: string;
  focusTarget: string;
  interactionCount: number;
  booted: boolean;
}

function controls(snapshot: HeaderSnapshot): HeaderControl[] {
  return [...snapshot.topControls, ...snapshot.workspaceControls].filter((control) => control.visible);
}

function nativeSectionFromRoute(route: HeaderControl["route"], fallback: NativeSection): NativeSection {
  return isNativeSection(route) ? route : fallback;
}

function manifestFrom(internals: StoreInternals): HeaderShadowManifest {
  const visibleControls = controls(internals.snapshot);
  return {
    schema: "ingen.electron.header_shadow_manifest.v1",
    sliceId: "header",
    mode: internals.snapshot.mode,
    activeSection: internals.snapshot.activeSection,
    sectionTitle: internals.snapshot.sectionTitle,
    profileCanvas: internals.snapshot.profileCanvas,
    leftPanelOpen: internals.snapshot.leftPanelOpen,
    rightPanelOpen: internals.snapshot.rightPanelOpen,
    visibleControlIds: visibleControls.map((control) => control.id),
    selectedControlIds: visibleControls.filter((control) => control.selected).map((control) => control.id),
    availableActions: visibleControls.map((control) => control.command),
    focusTarget: internals.focusTarget,
    lastEvent: internals.lastEvent,
    lastRequestId: internals.lastRequestId,
    snapshotProofHash: internals.snapshot.proofHash,
    ipcProofHash: internals.lastProof,
    interactionCount: internals.interactionCount
  };
}

function publicState(internals: StoreInternals): HeaderShadowState {
  return {
    snapshot: internals.snapshot,
    manifest: manifestFrom(internals),
    lastProof: internals.lastProof,
    booted: internals.booted
  };
}

function buildCommand(kind: HeaderCommandKind, route: HeaderControl["route"], current: HeaderSnapshot): HeaderCommand {
  const requestId = makeRequestId();
  if (kind === "navigate_workspace") {
    return {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId,
      kind,
      section: nativeSectionFromRoute(route, current.activeSection)
    };
  }
  return {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId,
    kind
  };
}

function withPanelToggleFallback(snapshot: HeaderSnapshot, command: HeaderCommand, accepted: boolean): HeaderSnapshot {
  if (accepted) {
    return snapshot;
  }
  const leftPanelOpen = command.kind === "toggle_left_panel" ? !snapshot.leftPanelOpen : snapshot.leftPanelOpen;
  const rightPanelOpen = command.kind === "toggle_right_panel" ? !snapshot.rightPanelOpen : snapshot.rightPanelOpen;
  const profileCanvas =
    command.kind === "open_sessions_canvas"
      ? snapshot.profileCanvas === "sessions" ? "" : "sessions"
      : command.kind === "navigate_workspace" ||
          command.kind === "toggle_left_panel" ||
          command.kind === "toggle_right_panel" ||
          command.kind === "open_webexplorer" ||
          command.kind === "open_banger" ||
          command.kind === "open_trading"
        ? ""
        : snapshot.profileCanvas;
  const activeSection =
    command.kind === "navigate_workspace"
      ? command.section
      : command.kind === "open_webexplorer"
        ? "webexplorer"
        : command.kind === "open_banger"
          ? "banger"
          : command.kind === "open_trading"
            ? "trading"
            : snapshot.activeSection;
  const sectionTitle =
    command.kind === "navigate_workspace"
      ? command.section === "webexplorer" ? "RAM DOM Atlas" : "Forge"
      : command.kind === "open_webexplorer"
        ? "RAM DOM Atlas"
        : command.kind === "open_banger"
          ? "New object"
          : command.kind === "open_trading"
            ? "Market"
      : snapshot.sectionTitle;
  return {
    ...snapshot,
    activeSection,
    sectionTitle,
    profileCanvas,
    leftPanelOpen,
    rightPanelOpen,
    topControls: snapshot.topControls.map((control) =>
      control.id === "left-panel" ? { ...control, selected: leftPanelOpen } : control
    ),
    workspaceControls: snapshot.workspaceControls.map((control) =>
      control.id === "right-panel" ? { ...control, selected: rightPanelOpen } : control
    )
  };
}

export function createHeaderShadowStore(
  api?: Pick<ForgeShellApi, "getHeaderSnapshot" | "dispatchHeaderCommand">
) {
  let internals: StoreInternals = {
    snapshot: fallbackSnapshot,
    lastProof: "shadow boot",
    lastEvent: "boot",
    lastRequestId: "boot",
    focusTarget: "left-panel",
    interactionCount: 0,
    booted: false
  };
  let state = publicState(internals);
  const listeners = new Set<() => void>();

  function emit(next: Partial<StoreInternals>): HeaderShadowState {
    internals = { ...internals, ...next };
    state = publicState(internals);
    listeners.forEach((listener) => listener());
    return state;
  }

  return {
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot(): HeaderShadowState {
      return state;
    },
    async boot(): Promise<HeaderShadowState> {
      const snapshot = (await api?.getHeaderSnapshot()) ?? fallbackSnapshot;
      const nextState = emit({
        snapshot,
        lastProof: snapshot.proofHash,
        lastEvent: "boot",
        lastRequestId: "boot",
        booted: true
      });
      if (api && /detecting/i.test(snapshot.gpuLabel)) {
        window.setTimeout(() => {
          void api.getHeaderSnapshot().then((refreshed) => {
            emit({
              snapshot: refreshed,
              lastProof: refreshed.proofHash,
              lastEvent: "boot",
              lastRequestId: "hardware-refresh",
              booted: true
            });
          });
        }, 900);
      }
      return nextState;
    },
    async dispatchControl(control: Pick<HeaderControl, "id" | "command" | "route">): Promise<HeaderShadowState> {
      const command = buildCommand(control.command, control.route, internals.snapshot);
      const interactionCount = internals.interactionCount + 1;
      if (command.kind !== "window_minimize" && command.kind !== "window_toggle_maximize" && command.kind !== "window_close") {
        emit({
          snapshot: withPanelToggleFallback(internals.snapshot, command, false),
          lastEvent: "shadow_manifest_recorded",
          lastRequestId: command.requestId,
          focusTarget: control.id,
          interactionCount
        });
      }
      const result = await api?.dispatchHeaderCommand(command);
      const remoteSnapshot = await api?.getHeaderSnapshot();
      const snapshot = remoteSnapshot ?? internals.snapshot;
      const accepted = result?.accepted === true;
      return emit({
        snapshot: remoteSnapshot ? withPanelToggleFallback(snapshot, command, accepted) : snapshot,
        lastProof: result?.proofHash ?? "renderer-only",
        lastEvent: result?.event ?? "rejected",
        lastRequestId: command.requestId,
        focusTarget: control.id,
        interactionCount
      });
    }
  };
}

const browserApi = typeof window === "undefined" ? undefined : window.forgeShell;

export const headerShadowStore = createHeaderShadowStore(browserApi);

export function useHeaderShadowStore(): HeaderShadowState {
  return useSyncExternalStore(headerShadowStore.subscribe, headerShadowStore.getSnapshot, headerShadowStore.getSnapshot);
}
