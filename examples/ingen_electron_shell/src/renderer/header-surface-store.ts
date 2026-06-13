import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  type ForgeShellApi,
  type HeaderSurfaceSnapshot
} from "../shared/ipc-contract";

export interface HeaderSurfaceState {
  snapshot: HeaderSurfaceSnapshot;
  booted: boolean;
}

export const fallbackSurfaceSnapshot: HeaderSurfaceSnapshot = {
  schema: "ingen.electron.header.surface_snapshot.v1",
  version: FORGE_ELECTRON_IPC_VERSION,
  mode: "shadow",
  activeSection: "forge",
  profileCanvas: "",
  surfaces: [
    {
      id: "forge-drop-canvas",
      kind: "drop_canvas",
      label: "Forge drop canvas",
      route: "forge",
      authority: "electron-shadow",
      status: "shadow",
      slot: { x: 287, y: 96, width: 1248, height: 690 },
      nativeContract: "forge-canvas-shadow-placeholder",
      sourceComponent: "DropCanvas",
      summary: "Default Forge canvas remains shadow-only until central-zone migration.",
      proofHash: "fallback"
    }
  ],
  proofHash: "fallback"
};

export function createHeaderSurfaceStore(api?: Pick<ForgeShellApi, "getHeaderSurfaceSnapshot">) {
  let state: HeaderSurfaceState = {
    snapshot: fallbackSurfaceSnapshot,
    booted: false
  };
  const listeners = new Set<() => void>();

  function emit(next: HeaderSurfaceState): HeaderSurfaceState {
    state = next;
    listeners.forEach((listener) => listener());
    return state;
  }

  async function refresh(): Promise<HeaderSurfaceState> {
    const snapshot = (await api?.getHeaderSurfaceSnapshot()) ?? fallbackSurfaceSnapshot;
    return emit({ snapshot, booted: true });
  }

  return {
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot(): HeaderSurfaceState {
      return state;
    },
    refresh
  };
}

const browserApi = typeof window === "undefined" ? undefined : window.forgeShell;

export const headerSurfaceStore = createHeaderSurfaceStore(browserApi);

export function useHeaderSurfaceStore(): HeaderSurfaceState {
  return useSyncExternalStore(
    headerSurfaceStore.subscribe,
    headerSurfaceStore.getSnapshot,
    headerSurfaceStore.getSnapshot
  );
}
