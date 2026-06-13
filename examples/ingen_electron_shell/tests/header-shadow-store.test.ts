import { describe, expect, it } from "vitest";
import type {
  FrontSliceMode,
  HeaderCommand,
  HeaderCommandResult,
  HeaderSnapshot
} from "../src/shared/ipc-contract";
import { FORGE_ELECTRON_IPC_VERSION } from "../src/shared/ipc-contract";
import { createHeaderShadowStore, fallbackSnapshot } from "../src/renderer/header-shadow-store";

function cloneSnapshot(overrides: Partial<HeaderSnapshot> = {}): HeaderSnapshot {
  return {
    ...fallbackSnapshot,
    ...overrides,
    topControls: fallbackSnapshot.topControls.map((control) => ({ ...control })),
    workspaceControls: fallbackSnapshot.workspaceControls.map((control) => ({ ...control })),
    nativeSurfaceContracts: { ...fallbackSnapshot.nativeSurfaceContracts }
  };
}

function commandResult(command: HeaderCommand, mode: FrontSliceMode): HeaderCommandResult {
  return {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted: mode === "electron",
    mode,
    event: mode === "electron" ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: `proof-${command.kind}`
  };
}

describe("header shadow store", () => {
  it("boots from the typed shell API and emits a compact manifest", async () => {
    const api = {
      getHeaderSnapshot: async () => cloneSnapshot({ proofHash: "snapshot-proof" }),
      dispatchHeaderCommand: async (command) => commandResult(command, "shadow")
    } satisfies Parameters<typeof createHeaderShadowStore>[0];
    const store = createHeaderShadowStore(api);

    const state = await store.boot();

    expect(state.booted).toBe(true);
    expect(state.manifest.schema).toBe("ingen.electron.header_shadow_manifest.v1");
    expect(state.manifest.visibleControlIds).toContain("webexplorer-top");
    expect(state.manifest.availableActions).toContain("open_banger");
    expect(state.manifest.snapshotProofHash).toBe("snapshot-proof");
  });

  it("dispatches navigation with generated IPC types and preserves focus target", async () => {
    const commands: HeaderCommand[] = [];
    let snapshot = cloneSnapshot({ proofHash: "boot-proof" });
    const api = {
      getHeaderSnapshot: async () => snapshot,
      dispatchHeaderCommand: async (command) => {
        commands.push(command);
        if (command.kind === "navigate_workspace") {
          snapshot = cloneSnapshot({
            mode: "electron",
            activeSection: command.section,
            sectionTitle: command.section === "webexplorer" ? "RAM DOM Atlas" : "Forge",
            proofHash: "web-proof"
          });
        }
        return commandResult(command, "electron");
      }
    } satisfies Parameters<typeof createHeaderShadowStore>[0];
    const store = createHeaderShadowStore(api);
    await store.boot();

    const state = await store.dispatchControl({
      id: "webexplorer-workspace",
      command: "navigate_workspace",
      route: "webexplorer"
    });

    expect(commands).toHaveLength(1);
    expect(commands[0]).toMatchObject({ kind: "navigate_workspace", section: "webexplorer" });
    expect(state.snapshot.activeSection).toBe("webexplorer");
    expect(state.manifest.focusTarget).toBe("webexplorer-workspace");
    expect(state.manifest.lastEvent).toBe("electron_command_applied");
    expect(state.manifest.interactionCount).toBe(1);
  });

  it("dispatches left panel and canvas split toggles through the same typed header circuit", async () => {
    const commands: HeaderCommand[] = [];
    let snapshot = cloneSnapshot({
      mode: "electron",
      leftPanelOpen: true,
      rightPanelOpen: false,
      proofHash: "panel-boot-proof"
    });
    const api = {
      getHeaderSnapshot: async () => snapshot,
      dispatchHeaderCommand: async (command) => {
        commands.push(command);
        if (command.kind === "toggle_left_panel") {
          snapshot = cloneSnapshot({ ...snapshot, leftPanelOpen: !snapshot.leftPanelOpen });
        }
        if (command.kind === "toggle_right_panel") {
          snapshot = cloneSnapshot({ ...snapshot, rightPanelOpen: !snapshot.rightPanelOpen });
        }
        return commandResult(command, "electron");
      }
    } satisfies Parameters<typeof createHeaderShadowStore>[0];
    const store = createHeaderShadowStore(api);
    await store.boot();

    const leftClosed = await store.dispatchControl({
      id: "left-panel",
      command: "toggle_left_panel"
    });
    const splitOpen = await store.dispatchControl({
      id: "right-panel",
      command: "toggle_right_panel",
      route: "right-panel"
    });

    expect(commands.map((command) => command.kind)).toEqual(["toggle_left_panel", "toggle_right_panel"]);
    expect(leftClosed.snapshot.leftPanelOpen).toBe(false);
    expect(splitOpen.snapshot.rightPanelOpen).toBe(true);
    expect(splitOpen.manifest.focusTarget).toBe("right-panel");
  });
});
