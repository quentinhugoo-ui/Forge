import { describe, expect, it } from "vitest";
import type {
  CanvasSurfacesCommand,
  CanvasSurfacesCommandResult,
  CanvasSurfacesSnapshot,
  ForgeShellApi
} from "../src/shared/ipc-contract";
import {
  FORGE_ELECTRON_IPC_VERSION,
  isCanvasSurfacesCommand,
  isCanvasSurfacesSnapshot
} from "../src/shared/ipc-contract";
import {
  createCanvasSurfacesStore,
  fallbackCanvasSurfacesSnapshot
} from "../src/renderer/canvas-surfaces-store";

function canvasSnapshot(overrides: Partial<CanvasSurfacesSnapshot> = {}): CanvasSurfacesSnapshot {
  return {
    ...fallbackCanvasSurfacesSnapshot,
    ...overrides,
    surfaces: overrides.surfaces ?? fallbackCanvasSurfacesSnapshot.surfaces.map((surface) => ({ ...surface }))
  };
}

describe("canvas surfaces store", () => {
  it("keeps the fallback Forge canvas IPC-ready and native policies explicit", () => {
    expect(isCanvasSurfacesSnapshot(fallbackCanvasSurfacesSnapshot)).toBe(true);
    expect(fallbackCanvasSurfacesSnapshot.activeSurfaceId).toBe("forge-drop-canvas");
    expect(fallbackCanvasSurfacesSnapshot.nativeSurfacePolicy.banger).toBe("child-window");
    expect(fallbackCanvasSurfacesSnapshot.nativeSurfacePolicy.webexplorer).toBe("rust-owned-webview");
  });

  it("refreshes central Banger/WebExplorer contracts from the shell API", async () => {
    const snapshot = canvasSnapshot({
      activeSection: "banger",
      activeSurfaceId: "banger-native-child-surface",
      surfaces: [
        {
          id: "banger-native-child-surface",
          kind: "banger_native_child",
          label: "Banger native viewport",
          route: "banger",
          status: "native_pending",
          sourceComponent: "BangerNativeViewport",
          nativeContract: "wgpu-child-window-frame-hash",
          authority: "electron-shadow",
          headline: "Banger child window slot",
          detail: "Banger owns rendering.",
          proofHash: "banger-proof"
        }
      ],
      proofHash: "banger-snapshot-proof"
    });
    const api: Pick<ForgeShellApi, "getCanvasSurfacesSnapshot" | "dispatchCanvasSurfacesCommand"> = {
      getCanvasSurfacesSnapshot: async () => snapshot,
      dispatchCanvasSurfacesCommand: async (command) => ({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: command.requestId,
        accepted: false,
        mode: "shadow",
        event: "shadow_manifest_recorded",
        proofHash: "result-proof"
      })
    };
    const store = createCanvasSurfacesStore(api);

    const state = await store.refresh();

    expect(state.snapshot.activeSurfaceId).toBe("banger-native-child-surface");
    expect(state.manifest.surfaceCount).toBe(1);
    expect(state.snapshot.surfaces[0]?.nativeContract).toBe("wgpu-child-window-frame-hash");
    expect(state.booted).toBe(true);
  });

  it("dispatches versioned commands and records the last result", async () => {
    const results: CanvasSurfacesCommand[] = [];
    const api: Pick<ForgeShellApi, "getCanvasSurfacesSnapshot" | "dispatchCanvasSurfacesCommand"> = {
      getCanvasSurfacesSnapshot: async () => fallbackCanvasSurfacesSnapshot,
      dispatchCanvasSurfacesCommand: async (command) => {
        results.push(command);
        const result: CanvasSurfacesCommandResult = {
          version: FORGE_ELECTRON_IPC_VERSION,
          requestId: command.requestId,
          accepted: false,
          mode: "shadow",
          event: "shadow_manifest_recorded",
          proofHash: "dispatch-proof"
        };
        return result;
      }
    };
    const store = createCanvasSurfacesStore(api);

    const result = await store.dispatch({ kind: "request_native_surface", section: "webexplorer" });

    expect(result?.event).toBe("shadow_manifest_recorded");
    expect(results[0]?.version).toBe(FORGE_ELECTRON_IPC_VERSION);
    expect(isCanvasSurfacesCommand(results[0])).toBe(true);
    expect(store.getSnapshot().lastResult?.proofHash).toBe("dispatch-proof");
  });
});
