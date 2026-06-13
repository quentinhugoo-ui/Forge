import { describe, expect, it } from "vitest";
import type { ForgeShellApi, RightPanelCommand, RightPanelCommandResult, RightPanelSnapshot } from "../src/shared/ipc-contract";
import { FORGE_ELECTRON_IPC_VERSION, isRightPanelCommand, isRightPanelSnapshot } from "../src/shared/ipc-contract";
import { createRightPanelStore, fallbackRightPanelSnapshot } from "../src/renderer/right-panel-store";

function rightPanelSnapshot(overrides: Partial<RightPanelSnapshot> = {}): RightPanelSnapshot {
  return {
    ...fallbackRightPanelSnapshot,
    ...overrides,
    tabs: overrides.tabs ?? fallbackRightPanelSnapshot.tabs.map((tab) => ({ ...tab })),
    lines: overrides.lines ?? fallbackRightPanelSnapshot.lines.map((line) => ({ ...line })),
    actions: overrides.actions ?? fallbackRightPanelSnapshot.actions.map((action) => ({ ...action }))
  };
}

describe("right panel store", () => {
  it("keeps a valid fallback status dock snapshot", () => {
    expect(isRightPanelSnapshot(fallbackRightPanelSnapshot)).toBe(true);
    expect(fallbackRightPanelSnapshot.activeTab).toBe("status");
    expect(fallbackRightPanelSnapshot.lines.length).toBeGreaterThan(0);
  });

  it("refreshes proof lines from the shell API", async () => {
    const snapshot = rightPanelSnapshot({
      activeSection: "banger",
      activeTab: "native",
      title: "Banger proof dock",
      lines: [{ label: "Banger", value: "child-window native_pending", severity: "warn", proofHash: "banger-proof" }],
      proofHash: "right-panel-proof"
    });
    const api: Pick<ForgeShellApi, "getRightPanelSnapshot" | "dispatchRightPanelCommand"> = {
      getRightPanelSnapshot: async () => snapshot,
      dispatchRightPanelCommand: async (command) => ({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: command.requestId,
        accepted: false,
        mode: "shadow",
        event: "shadow_manifest_recorded",
        proofHash: "result-proof"
      })
    };
    const store = createRightPanelStore(api);

    const state = await store.refresh();

    expect(state.snapshot.title).toBe("Banger proof dock");
    expect(state.manifest.lineCount).toBe(1);
    expect(state.snapshot.lines[0]?.severity).toBe("warn");
  });

  it("dispatches versioned tab commands and records the result", async () => {
    const commands: RightPanelCommand[] = [];
    const api: Pick<ForgeShellApi, "getRightPanelSnapshot" | "dispatchRightPanelCommand"> = {
      getRightPanelSnapshot: async () => fallbackRightPanelSnapshot,
      dispatchRightPanelCommand: async (command) => {
        commands.push(command);
        const result: RightPanelCommandResult = {
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
    const store = createRightPanelStore(api);

    const result = await store.dispatch({ kind: "select_tab", target: "proofs" });

    expect(result?.event).toBe("shadow_manifest_recorded");
    expect(commands[0]?.version).toBe(FORGE_ELECTRON_IPC_VERSION);
    expect(isRightPanelCommand(commands[0])).toBe(true);
    expect(store.getSnapshot().lastResult?.proofHash).toBe("dispatch-proof");
  });
});
