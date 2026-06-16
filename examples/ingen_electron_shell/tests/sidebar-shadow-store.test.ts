import { describe, expect, it } from "vitest";
import type {
  ForgeShellApi,
  FrontSliceMode,
  SidebarCommand,
  SidebarCommandResult,
  SidebarSessionItem,
  SidebarSnapshot
} from "../src/shared/ipc-contract";
import { FORGE_ELECTRON_IPC_VERSION } from "../src/shared/ipc-contract";
import { createSidebarShadowStore, fallbackSidebarSnapshot } from "../src/renderer/sidebar-shadow-store";

const sessionFixtureItems: SidebarSessionItem[] = [
  {
    sessionId: "native-front-migration",
    label: "Electron cutover",
    date: "2026-06-09",
    section: "forge",
    workspaceLabel: "Forge",
    rowVisible: true,
    pinned: true,
    working: false,
    automated: false,
    archived: false
  },
  {
    sessionId: "",
    label: "LLM Act Codes discovery methods",
    date: "2026-06-08",
    section: "forge",
    workspaceLabel: "Forge",
    rowVisible: true,
    pinned: false,
    working: false,
    automated: true,
    archived: false
  }
];

function cloneSnapshot(overrides: Partial<SidebarSnapshot> = {}): SidebarSnapshot {
  return {
    ...fallbackSidebarSnapshot,
    ...overrides,
    recentItems: (overrides.recentItems ?? sessionFixtureItems).map((item) => ({ ...item })),
    archivedItems: (overrides.archivedItems ?? fallbackSidebarSnapshot.archivedItems).map((item) => ({ ...item })),
    toolControls: fallbackSidebarSnapshot.toolControls.map((tool) => ({ ...tool })),
    profileMenuItems: fallbackSidebarSnapshot.profileMenuItems.map((item) => ({ ...item })),
    archiveConfirm: { ...fallbackSidebarSnapshot.archiveConfirm, ...overrides.archiveConfirm }
  };
}

function commandResult(command: SidebarCommand, mode: FrontSliceMode): SidebarCommandResult {
  return {
    version: FORGE_ELECTRON_IPC_VERSION,
    requestId: command.requestId,
    accepted: mode === "electron",
    mode,
    event: mode === "electron" ? "electron_command_applied" : "shadow_manifest_recorded",
    proofHash: `sidebar-proof-${command.kind}`
  };
}

describe("sidebar shadow store", () => {
  it("never exposes demo sessions before the real sidebar snapshot arrives", async () => {
    const store = createSidebarShadowStore();
    const state = await store.boot();
    const labels = state.snapshot.recentItems.map((item) => item.label);

    expect(state.snapshot.recentItems).toEqual([]);
    expect(state.snapshot.archivedItems).toEqual([]);
    expect(labels).not.toContain("Electron cutover");
    expect(labels).not.toContain("test session example");
    expect(labels).not.toContain("LLM Act Codes discovery methods");
    expect(labels).not.toContain("PoolClaw agent profiles");
    expect(labels).not.toContain("Banger create 3D object");
    expect(state.manifest.visibleRecentIds).toEqual([]);
  });

  it("boots from the typed shell API and emits a migration manifest", async () => {
    const api: ForgeShellApi = {
      getCutover: async () => "shadow",
      getHeaderSnapshot: async () => {
        throw new Error("unused");
      },
      getHeaderSurfaceSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchHeaderCommand: async () => {
        throw new Error("unused");
      },
      getSidebarSnapshot: async () => cloneSnapshot({ proofHash: "sidebar-snapshot-proof" }),
      dispatchSidebarCommand: async (command) => commandResult(command, "shadow"),
      getPanelsChatBottomSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchPanelsChatBottomCommand: async () => {
        throw new Error("unused");
      },
      getCanvasSurfacesSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchCanvasSurfacesCommand: async () => {
        throw new Error("unused");
      },
      getRightPanelSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchRightPanelCommand: async () => {
        throw new Error("unused");
      },
      connectLlmProvider: async () => {
        throw new Error("unused");
      }
    };
    const store = createSidebarShadowStore(api);

    const state = await store.boot();

    expect(state.booted).toBe(true);
    expect(state.manifest.schema).toBe("ingen.electron.sidebar_shadow_manifest.v1");
    expect(state.manifest.sliceId).toBe("sidebar_sessions");
    expect(state.manifest.visibleToolIds).toContain("new-session");
    expect(state.manifest.visibleRecentIds).toContain("native-front-migration");
    expect(state.manifest.snapshotProofHash).toBe("sidebar-snapshot-proof");
  });

  it("dispatches session mode changes with generated IPC types and preserves focus", async () => {
    const commands: SidebarCommand[] = [];
    let snapshot = cloneSnapshot();
    const api: ForgeShellApi = {
      getCutover: async () => "electron",
      getHeaderSnapshot: async () => {
        throw new Error("unused");
      },
      getHeaderSurfaceSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchHeaderCommand: async () => {
        throw new Error("unused");
      },
      getSidebarSnapshot: async () => snapshot,
      dispatchSidebarCommand: async (command) => {
        commands.push(command);
        if (command.kind === "switch_sessions_mode") {
          snapshot = cloneSnapshot({
            mode: "electron",
            profileCanvas: "sessions",
            sessionsMenuMode: command.mode,
            proofHash: "archived-proof"
          });
        }
        return commandResult(command, "electron");
      },
      getPanelsChatBottomSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchPanelsChatBottomCommand: async () => {
        throw new Error("unused");
      },
      getCanvasSurfacesSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchCanvasSurfacesCommand: async () => {
        throw new Error("unused");
      },
      getRightPanelSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchRightPanelCommand: async () => {
        throw new Error("unused");
      },
      connectLlmProvider: async () => {
        throw new Error("unused");
      }
    };
    const store = createSidebarShadowStore(api);
    await store.boot();

    const state = await store.dispatch(
      store.command({ kind: "switch_sessions_mode", mode: "archived" }),
      "sessions-archived"
    );

    expect(commands).toHaveLength(1);
    expect(commands[0]).toMatchObject({ kind: "switch_sessions_mode", mode: "archived" });
    expect(state.snapshot.profileCanvas).toBe("sessions");
    expect(state.manifest.sessionsMenuMode).toBe("archived");
    expect(state.manifest.focusTarget).toBe("sessions-archived");
    expect(state.manifest.lastEvent).toBe("electron_command_applied");
  });

  it("archives a row by label when the Slint-shadow session has no stable id", async () => {
    let snapshot = cloneSnapshot();
    const api: ForgeShellApi = {
      getCutover: async () => "electron",
      getHeaderSnapshot: async () => {
        throw new Error("unused");
      },
      getHeaderSurfaceSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchHeaderCommand: async () => {
        throw new Error("unused");
      },
      getSidebarSnapshot: async () => snapshot,
      dispatchSidebarCommand: async (command) => {
        if (command.kind === "archive_session") {
          const candidate = snapshot.recentItems.find((item) => item.label === command.sessionId);
          snapshot = cloneSnapshot({
            recentItems: snapshot.recentItems,
            archiveConfirm: {
              open: true,
              candidateId: command.sessionId,
              candidateLabel: candidate?.label ?? "New session",
              candidateDate: candidate?.date ?? "2026-06-09",
              candidateSection: candidate?.section ?? "forge"
            }
          });
        } else if (command.kind === "confirm_archive") {
          const archivedId = snapshot.archiveConfirm.candidateId;
          const archived = snapshot.recentItems.find((item) => item.label === archivedId);
          snapshot = cloneSnapshot({
            recentItems: snapshot.recentItems.map((item) =>
              item.label === archivedId ? { ...item, archived: true, rowVisible: false } : item
            ),
            archivedItems: archived ? [{ ...archived, archived: true, rowVisible: true }] : [],
            hasArchivedSession: Boolean(archived),
            archiveConfirm: { open: false, candidateId: "", candidateLabel: "", candidateDate: "", candidateSection: "forge" }
          });
        }
        return commandResult(command, "electron");
      },
      getPanelsChatBottomSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchPanelsChatBottomCommand: async () => {
        throw new Error("unused");
      },
      getCanvasSurfacesSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchCanvasSurfacesCommand: async () => {
        throw new Error("unused");
      },
      getRightPanelSnapshot: async () => {
        throw new Error("unused");
      },
      dispatchRightPanelCommand: async () => {
        throw new Error("unused");
      },
      connectLlmProvider: async () => {
        throw new Error("unused");
      }
    };
    const store = createSidebarShadowStore(api);
    await store.boot();

    const state = await store.archiveSession("LLM Act Codes discovery methods", "archive-llm-act-codes");

    expect(state.snapshot.recentItems.find((item) => item.label === "LLM Act Codes discovery methods")?.rowVisible).toBe(false);
    expect(state.snapshot.archivedItems.some((item) => item.label === "LLM Act Codes discovery methods" && item.rowVisible)).toBe(true);
    expect(state.snapshot.archiveConfirm.open).toBe(false);
    expect(state.manifest.archivedCount).toBe(1);
    expect(state.manifest.focusTarget).toBe("archive-llm-act-codes");
  });
});
