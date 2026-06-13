import { describe, expect, it } from "vitest";
import type { ForgeShellApi, HeaderSurfaceSnapshot } from "../src/shared/ipc-contract";
import { FORGE_ELECTRON_IPC_VERSION } from "../src/shared/ipc-contract";
import { createHeaderSurfaceStore, fallbackSurfaceSnapshot } from "../src/renderer/header-surface-store";

function surfaceSnapshot(overrides: Partial<HeaderSurfaceSnapshot> = {}): HeaderSurfaceSnapshot {
  return {
    ...fallbackSurfaceSnapshot,
    ...overrides,
    surfaces: overrides.surfaces ?? fallbackSurfaceSnapshot.surfaces.map((surface) => ({ ...surface }))
  };
}

describe("header opened surface store", () => {
  it("boots with the default Forge drop canvas contract", () => {
    const store = createHeaderSurfaceStore();

    const state = store.getSnapshot();

    expect(state.snapshot.schema).toBe("ingen.electron.header.surface_snapshot.v1");
    expect(state.snapshot.surfaces[0]?.kind).toBe("drop_canvas");
    expect(state.snapshot.surfaces[0]?.sourceComponent).toBe("DropCanvas");
  });

  it("refreshes WebExplorer/Banger/Trading contracts from the typed shell API", async () => {
    const snapshots: HeaderSurfaceSnapshot[] = [
      surfaceSnapshot({
        activeSection: "webexplorer",
        surfaces: [
          {
            id: "webexplorer-webview-host",
            kind: "webexplorer_webview",
            label: "Rust WebView host",
            route: "webexplorer",
            authority: "electron-shadow",
            status: "native_pending",
            slot: { x: 287, y: 96, width: 1248, height: 690 },
            nativeContract: "rust-owned-webview-policy-host",
            sourceComponent: "GoogleWebViewCanvas",
            summary: "Rust owns navigation policy.",
            proofHash: "web-proof"
          }
        ],
        proofHash: "web-snapshot-proof"
      }),
      surfaceSnapshot({
        activeSection: "banger",
        surfaces: [
          {
            id: "banger-native-child-surface",
            kind: "banger_native_child",
            label: "Banger native viewport",
            route: "banger",
            authority: "electron-shadow",
            status: "native_pending",
            slot: { x: 287, y: 96, width: 1248, height: 690 },
            nativeContract: "wgpu-child-window-frame-hash",
            sourceComponent: "BangerNativeViewport",
            summary: "Banger owns rendering.",
            proofHash: "banger-proof"
          }
        ],
        proofHash: "banger-snapshot-proof"
      }),
      surfaceSnapshot({
        activeSection: "trading",
        surfaces: [
          {
            id: "trading-product-section",
            kind: "product_section",
            label: "Product section surface",
            route: "trading",
            authority: "electron-shadow",
            status: "shadow",
            slot: { x: 287, y: 96, width: 1248, height: 690 },
            nativeContract: "domain-service-proof-projection",
            sourceComponent: "ProductSectionSurface",
            summary: "Rust service proof projection.",
            proofHash: "trading-proof"
          }
        ],
        proofHash: "trading-snapshot-proof"
      })
    ];
    const api: Pick<ForgeShellApi, "getHeaderSurfaceSnapshot"> = {
      getHeaderSurfaceSnapshot: async () => snapshots.shift() ?? fallbackSurfaceSnapshot
    };
    const store = createHeaderSurfaceStore(api);

    expect((await store.refresh()).snapshot.surfaces[0]?.sourceComponent).toBe("GoogleWebViewCanvas");
    expect((await store.refresh()).snapshot.surfaces[0]?.nativeContract).toBe("wgpu-child-window-frame-hash");
    expect((await store.refresh()).snapshot.surfaces[0]?.route).toBe("trading");
    expect(store.getSnapshot().booted).toBe(true);
  });

  it("keeps generated version compatibility in fallback snapshots", () => {
    expect(fallbackSurfaceSnapshot.version).toBe(FORGE_ELECTRON_IPC_VERSION);
  });
});
