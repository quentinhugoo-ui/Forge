import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  makeRightPanelRequestId,
  type ForgeShellApi,
  type RightPanelCommand,
  type RightPanelCommandResult,
  type RightPanelSnapshot
} from "../shared/ipc-contract";

export interface RightPanelManifest {
  sliceId: "right_panel";
  mode: RightPanelSnapshot["mode"];
  activeTab: string;
  lineCount: number;
  proofHash: string;
}

export interface RightPanelState {
  snapshot: RightPanelSnapshot;
  manifest: RightPanelManifest;
  lastResult?: RightPanelCommandResult;
  booted: boolean;
}

export const fallbackRightPanelSnapshot: RightPanelSnapshot = {
  schema: "ingen.electron.right_panel.snapshot.v1",
  version: FORGE_ELECTRON_IPC_VERSION,
  mode: "shadow",
  activeSection: "forge",
  profileCanvas: "",
  open: true,
  activeTab: "status",
  title: "Section status dock",
  summary: "Right panel shadow projection waiting for Rust probe promotion.",
  tabs: [
    { id: "status", label: "Status", selected: true, count: 4 },
    { id: "proofs", label: "Proofs", selected: false, count: 3 },
    { id: "native", label: "Native", selected: false, count: 1 }
  ],
  lines: [
    { label: "Section", value: "forge", severity: "ok", proofHash: "fallback-section" },
    { label: "Jobs", value: "queued=0 running=0", severity: "ok", proofHash: "fallback-jobs" }
  ],
  actions: [
    { id: "refresh", label: "Refresh", command: "refresh", enabled: true },
    { id: "native", label: "Native", command: "select_tab", enabled: true }
  ],
  proofHash: "fallback"
};

function manifestFrom(snapshot: RightPanelSnapshot): RightPanelManifest {
  return {
    sliceId: "right_panel",
    mode: snapshot.mode,
    activeTab: snapshot.activeTab,
    lineCount: snapshot.lines.length,
    proofHash: snapshot.proofHash
  };
}

function withEnvelope(command: Omit<RightPanelCommand, "version" | "requestId">): RightPanelCommand {
  return {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: makeRightPanelRequestId(),
    ...command
  };
}

export function createRightPanelStore(
  api?: Pick<ForgeShellApi, "getRightPanelSnapshot" | "dispatchRightPanelCommand">
) {
  let state: RightPanelState = {
    snapshot: fallbackRightPanelSnapshot,
    manifest: manifestFrom(fallbackRightPanelSnapshot),
    booted: false
  };
  const listeners = new Set<() => void>();

  function emit(next: RightPanelState): RightPanelState {
    state = next;
    listeners.forEach((listener) => listener());
    return state;
  }

  async function refresh(): Promise<RightPanelState> {
    const snapshot = (await api?.getRightPanelSnapshot()) ?? fallbackRightPanelSnapshot;
    return emit({ ...state, snapshot, manifest: manifestFrom(snapshot), booted: true });
  }

  async function dispatch(
    command: Omit<RightPanelCommand, "version" | "requestId">
  ): Promise<RightPanelCommandResult | undefined> {
    const result = await api?.dispatchRightPanelCommand(withEnvelope(command));
    if (!result) return undefined;
    const refreshed = await refresh();
    emit({ ...refreshed, lastResult: result });
    return result;
  }

  return {
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot(): RightPanelState {
      return state;
    },
    refresh,
    dispatch
  };
}

const browserApi = typeof window === "undefined" ? undefined : window.forgeShell;

export const rightPanelStore = createRightPanelStore(browserApi);

export function useRightPanelStore(): RightPanelState {
  return useSyncExternalStore(rightPanelStore.subscribe, rightPanelStore.getSnapshot, rightPanelStore.getSnapshot);
}
