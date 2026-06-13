import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  makeCanvasSurfacesRequestId,
  type CanvasSurfacesCommand,
  type CanvasSurfacesCommandResult,
  type CanvasSurfacesSnapshot,
  type ForgeShellApi
} from "../shared/ipc-contract";

export interface CanvasSurfacesManifest {
  sliceId: "canvas_surfaces";
  mode: CanvasSurfacesSnapshot["mode"];
  activeSurfaceId: string;
  surfaceCount: number;
  nativePolicy: CanvasSurfacesSnapshot["nativeSurfacePolicy"];
  proofHash: string;
}

export interface CanvasSurfacesState {
  snapshot: CanvasSurfacesSnapshot;
  manifest: CanvasSurfacesManifest;
  lastResult?: CanvasSurfacesCommandResult;
  booted: boolean;
}

export const fallbackCanvasSurfacesSnapshot: CanvasSurfacesSnapshot = {
  schema: "ingen.electron.canvas_surfaces.snapshot.v1",
  version: FORGE_ELECTRON_IPC_VERSION,
  mode: "shadow",
  activeSection: "forge",
  profileCanvas: "",
  activeSurfaceId: "forge-drop-canvas",
  surfaces: [
    {
      id: "forge-drop-canvas",
      kind: "drop_canvas",
      label: "Forge drop canvas",
      route: "forge",
      status: "ipc_ready",
      sourceComponent: "DropCanvas",
      nativeContract: "forge-canvas-shadow-placeholder",
      authority: "electron-shadow",
      headline: "Forge canvas shadow",
      detail: "Default CodeAct drop surface remains metadata-only until central canvas promotion.",
      proofHash: "fallback"
    }
  ],
  nativeSurfacePolicy: {
    banger: "child-window",
    webexplorer: "rust-owned-webview"
  },
  proofHash: "fallback"
};

function manifestFrom(snapshot: CanvasSurfacesSnapshot): CanvasSurfacesManifest {
  return {
    sliceId: "canvas_surfaces",
    mode: snapshot.mode,
    activeSurfaceId: snapshot.activeSurfaceId,
    surfaceCount: snapshot.surfaces.length,
    nativePolicy: snapshot.nativeSurfacePolicy,
    proofHash: snapshot.proofHash
  };
}

function commandWithEnvelope(command: Omit<CanvasSurfacesCommand, "version" | "requestId">): CanvasSurfacesCommand {
  return {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: makeCanvasSurfacesRequestId(),
    ...command
  };
}

export function createCanvasSurfacesStore(
  api?: Pick<ForgeShellApi, "getCanvasSurfacesSnapshot" | "dispatchCanvasSurfacesCommand">
) {
  let state: CanvasSurfacesState = {
    snapshot: fallbackCanvasSurfacesSnapshot,
    manifest: manifestFrom(fallbackCanvasSurfacesSnapshot),
    booted: false
  };
  const listeners = new Set<() => void>();

  function emit(next: CanvasSurfacesState): CanvasSurfacesState {
    state = next;
    listeners.forEach((listener) => listener());
    return state;
  }

  async function refresh(): Promise<CanvasSurfacesState> {
    const snapshot = (await api?.getCanvasSurfacesSnapshot()) ?? fallbackCanvasSurfacesSnapshot;
    return emit({
      ...state,
      snapshot,
      manifest: manifestFrom(snapshot),
      booted: true
    });
  }

  async function dispatch(
    command: Omit<CanvasSurfacesCommand, "version" | "requestId">
  ): Promise<CanvasSurfacesCommandResult | undefined> {
    const result = await api?.dispatchCanvasSurfacesCommand(commandWithEnvelope(command));
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
    getSnapshot(): CanvasSurfacesState {
      return state;
    },
    refresh,
    dispatch
  };
}

const browserApi = typeof window === "undefined" ? undefined : window.forgeShell;

export const canvasSurfacesStore = createCanvasSurfacesStore(browserApi);

export function useCanvasSurfacesStore(): CanvasSurfacesState {
  return useSyncExternalStore(
    canvasSurfacesStore.subscribe,
    canvasSurfacesStore.getSnapshot,
    canvasSurfacesStore.getSnapshot
  );
}
