import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  type ForgeShellApi,
  type NativeSection,
  type ProfileCanvas,
  type SessionsMenuMode,
  type SidebarCommand,
  type SidebarCommandKind,
  type SidebarCommandResult,
  type SidebarSessionItem,
  type SidebarSnapshot,
  makeSidebarRequestId
} from "../shared/ipc-contract";

export interface SidebarShadowManifest {
  schema: "ingen.electron.sidebar_shadow_manifest.v1";
  sliceId: "sidebar_sessions";
  mode: SidebarSnapshot["mode"];
  activeSection: NativeSection;
  profileCanvas: ProfileCanvas;
  activeDrawer: string;
  sessionsMenuMode: SessionsMenuMode;
  visibleToolIds: string[];
  visibleRecentIds: string[];
  archivedCount: number;
  availableActions: SidebarCommandKind[];
  focusTarget: string;
  lastEvent: SidebarCommandResult["event"] | "boot";
  lastRequestId: string;
  snapshotProofHash: string;
  ipcProofHash: string;
  interactionCount: number;
}

export interface SidebarShadowState {
  snapshot: SidebarSnapshot;
  manifest: SidebarShadowManifest;
  lastProof: string;
  booted: boolean;
}

export const fallbackSidebarSnapshot: SidebarSnapshot = {
  schema: "ingen.electron.sidebar.snapshot.v1",
  version: FORGE_ELECTRON_IPC_VERSION,
  mode: "shadow",
  activeSection: "forge",
  profileCanvas: "",
  activeDrawer: "",
  profileOpen: false,
  sessionsMenuMode: "recents",
  recentSessionId: "",
  hasArchivedSession: false,
  recentItems: [],
  archivedItems: [],
  toolControls: [
    { id: "new-session", label: "New Session", icon: "inline_svg_0008.svg", drawer: "", visible: true, hidden: false, selected: true, nativeAuthority: "electron-shadow" },
    { id: "pool", label: "Pool", icon: "inline_svg_0018.svg", drawer: "pool", visible: true, hidden: false, selected: false, nativeAuthority: "electron-shadow" },
    { id: "modules", label: "Modules", icon: "inline_svg_0019.svg", drawer: "modules", visible: true, hidden: false, selected: false, nativeAuthority: "electron-shadow" },
    { id: "assets", label: "My Assets", icon: "inline_svg_0005.svg", drawer: "assets", visible: true, hidden: false, selected: false, nativeAuthority: "electron-shadow" },
    { id: "automations", label: "Automations", icon: "inline_svg_0021.svg", drawer: "", visible: true, hidden: false, selected: false, nativeAuthority: "electron-shadow" },
    { id: "brain", label: "Brain", icon: "inline_svg_0020.svg", drawer: "", visible: true, hidden: false, selected: false, nativeAuthority: "electron-shadow" }
  ],
  profileMenuItems: [
    { id: "llm", label: "LLM providers", detail: "Keys, models and local routing", iconLabel: "AI" },
    { id: "profile", label: "Profile", detail: "Public canvas replica", iconLabel: "Q" }
  ],
  archiveConfirm: {
    open: false,
    candidateId: "",
    candidateLabel: "",
    candidateDate: "",
    candidateSection: "forge"
  },
  profileCanvasSummary: "workspace",
  proofHash: "fallback-sidebar"
};

interface StoreInternals {
  snapshot: SidebarSnapshot;
  lastProof: string;
  lastEvent: SidebarShadowManifest["lastEvent"];
  lastRequestId: string;
  focusTarget: string;
  interactionCount: number;
  booted: boolean;
}

type SidebarCommandDraft = SidebarCommand extends infer Command
  ? Command extends SidebarCommand
    ? Omit<Command, "version" | "requestId">
    : never
  : never;

function manifestFrom(internals: StoreInternals): SidebarShadowManifest {
  const visibleTools = internals.snapshot.toolControls.filter((tool) => tool.visible);
  return {
    schema: "ingen.electron.sidebar_shadow_manifest.v1",
    sliceId: "sidebar_sessions",
    mode: internals.snapshot.mode,
    activeSection: internals.snapshot.activeSection,
    profileCanvas: internals.snapshot.profileCanvas,
    activeDrawer: internals.snapshot.activeDrawer,
    sessionsMenuMode: internals.snapshot.sessionsMenuMode,
    visibleToolIds: visibleTools.map((tool) => tool.id),
    visibleRecentIds: internals.snapshot.recentItems.filter((item) => item.rowVisible).map((item) => item.sessionId || item.label),
    archivedCount: internals.snapshot.archivedItems.filter((item) => item.rowVisible).length,
    availableActions: [
      "navigate",
      "open_session",
      "rename_session",
      "open_profile_canvas",
      "archive_session",
      "activate_control",
      "switch_sessions_mode",
      "toggle_profile_menu",
      "set_active_drawer",
      "hide_tool",
      "restore_tool",
      "pin_session",
      "confirm_archive",
      "cancel_archive"
    ],
    focusTarget: internals.focusTarget,
    lastEvent: internals.lastEvent,
    lastRequestId: internals.lastRequestId,
    snapshotProofHash: internals.snapshot.proofHash,
    ipcProofHash: internals.lastProof,
    interactionCount: internals.interactionCount
  };
}

function publicState(internals: StoreInternals): SidebarShadowState {
  return {
    snapshot: internals.snapshot,
    manifest: manifestFrom(internals),
    lastProof: internals.lastProof,
    booted: internals.booted
  };
}

function sessionMatches(item: SidebarSessionItem, sessionId: string): boolean {
  return item.sessionId === sessionId || item.parallelGroupId === sessionId || (!item.sessionId && item.label === sessionId);
}

function localPreviewSnapshot(snapshot: SidebarSnapshot, command: SidebarCommand): SidebarSnapshot {
  const next: SidebarSnapshot = {
    ...snapshot,
    recentItems: snapshot.recentItems.map((item) => ({ ...item })),
    archivedItems: snapshot.archivedItems.map((item) => ({ ...item })),
    toolControls: snapshot.toolControls.map((tool) => ({ ...tool })),
    profileMenuItems: snapshot.profileMenuItems.map((item) => ({ ...item })),
    archiveConfirm: { ...snapshot.archiveConfirm }
  };
  if (command.kind === "switch_sessions_mode") {
    next.profileCanvas = "sessions";
    next.sessionsMenuMode = command.mode;
  } else if (command.kind === "open_profile_canvas") {
    next.profileCanvas = command.canvas;
    next.profileOpen = false;
  } else if (command.kind === "toggle_profile_menu") {
    next.profileCanvas = "";
    next.profileOpen = !next.profileOpen;
  } else if (command.kind === "archive_session") {
    const candidate = next.recentItems.find((item) => sessionMatches(item, command.sessionId));
    next.archiveConfirm = {
      open: true,
      candidateId: command.sessionId,
      candidateLabel: candidate?.label ?? "New session",
      candidateDate: candidate?.date ?? "2026-06-09",
      candidateSection: candidate?.section ?? "forge"
    };
  } else if (command.kind === "rename_session") {
    const label = command.label.replace(/\s+/g, " ").trim();
    if (label) {
      next.recentItems = next.recentItems.map((item) => (sessionMatches(item, command.sessionId) ? { ...item, label } : item));
      next.archivedItems = next.archivedItems.map((item) => (sessionMatches(item, command.sessionId) ? { ...item, label } : item));
      if (next.archiveConfirm.candidateId === command.sessionId) {
        next.archiveConfirm = { ...next.archiveConfirm, candidateLabel: label };
      }
    }
  } else if (command.kind === "confirm_archive") {
    const archivedId = next.archiveConfirm.candidateId;
    const archived = next.recentItems.find((item) => sessionMatches(item, archivedId));
    next.recentItems = next.recentItems.map((item) =>
      sessionMatches(item, archivedId) ? { ...item, archived: true, rowVisible: false } : item
    );
    next.archivedItems = archived ? [{ ...archived, archived: true, rowVisible: true }, ...next.archivedItems] : next.archivedItems;
    next.hasArchivedSession = true;
    next.archiveConfirm = { open: false, candidateId: "", candidateLabel: "", candidateDate: "", candidateSection: "forge" };
  } else if (command.kind === "set_active_drawer") {
    next.profileCanvas = "";
    next.profileOpen = false;
    next.activeDrawer = next.activeDrawer === command.drawer ? "" : command.drawer;
  } else if (command.kind === "pin_session") {
    next.profileCanvas = "";
    next.profileOpen = false;
    next.recentItems = next.recentItems.map((item) =>
      item.sessionId === "native-front-migration"
        ? { ...item, label: command.label, section: command.section, pinned: true }
        : { ...item, pinned: false }
    );
  } else if (command.kind === "navigate") {
    next.activeSection = command.section;
    next.profileCanvas = "";
    next.recentSessionId = "";
  } else if (command.kind === "open_session") {
    next.activeSection = command.section;
    next.profileCanvas = "";
    next.recentSessionId = command.sessionId;
  } else if (command.kind === "activate_control" || command.kind === "hide_tool" || command.kind === "restore_tool") {
    next.profileCanvas = "";
    next.profileOpen = false;
  } else if (command.kind === "cancel_archive") {
    next.archiveConfirm = { open: false, candidateId: "", candidateLabel: "", candidateDate: "", candidateSection: "forge" };
  }
  next.profileCanvasSummary =
    next.profileCanvas === "sessions"
      ? `sessions:${next.sessionsMenuMode}`
      : next.profileCanvas === "profile"
        ? "profile-canvas"
        : next.profileCanvas === "brain"
          ? "brain-canvas"
          : next.profileCanvas === "llm"
            ? "llm-providers"
            : "workspace";
  return next;
}

function previewSnapshotFromLocation(): SidebarSnapshot {
  if (typeof window === "undefined") return fallbackSidebarSnapshot;
  const state = new URL(window.location.href).searchParams.get("sidebar-state");
  if (state === "sessions-recents") {
    return localPreviewSnapshot(fallbackSidebarSnapshot, {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "preview-sessions-recents",
      kind: "switch_sessions_mode",
      mode: "recents"
    });
  }
  if (state === "sessions-archived") {
    const archived = localPreviewSnapshot(fallbackSidebarSnapshot, {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "preview-archive",
      kind: "archive_session",
      sessionId: "native-front-migration"
    });
    const confirmed = localPreviewSnapshot(archived, {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "preview-confirm",
      kind: "confirm_archive"
    });
    return localPreviewSnapshot(confirmed, {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "preview-sessions-archived",
      kind: "switch_sessions_mode",
      mode: "archived"
    });
  }
  if (state === "profile") {
    return localPreviewSnapshot(fallbackSidebarSnapshot, {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "preview-profile",
      kind: "open_profile_canvas",
      canvas: "profile"
    });
  }
  return fallbackSidebarSnapshot;
}

type SidebarShellApiProvider = ForgeShellApi | (() => ForgeShellApi | undefined);

function browserApi(): ForgeShellApi | undefined {
  return typeof window === "undefined" ? undefined : window.forgeShell;
}

export function createSidebarShadowStore(apiProvider?: SidebarShellApiProvider) {
  const resolveApi = () =>
    typeof apiProvider === "function"
      ? apiProvider()
      : apiProvider ?? browserApi();
  let pendingChatSession: SidebarSessionItem | null = null;
  let internals: StoreInternals = {
    snapshot: fallbackSidebarSnapshot,
    lastProof: "sidebar shadow boot",
    lastEvent: "boot",
    lastRequestId: "boot",
    focusTarget: "new-session",
    interactionCount: 0,
    booted: false
  };
  let state = publicState(internals);
  const listeners = new Set<() => void>();

  function emit(next: Partial<StoreInternals>): SidebarShadowState {
    internals = { ...internals, ...next };
    state = publicState(internals);
    listeners.forEach((listener) => listener());
    return state;
  }

  function matchesPendingChatSession(item: SidebarSessionItem, pending: SidebarSessionItem): boolean {
    return (
      item.sessionId !== pending.sessionId &&
      item.label === pending.label &&
      (item.workspaceLabel ?? "") === (pending.workspaceLabel ?? "")
    );
  }

  function withPendingChatSession(snapshot: SidebarSnapshot): SidebarSnapshot {
    if (!pendingChatSession) {
      return snapshot;
    }
    const backendSelectedDifferentSession =
      snapshot.recentSessionId &&
      snapshot.recentSessionId !== pendingChatSession.sessionId;
    const backendSelectedParallelGroup = snapshot.recentItems.some((item) =>
      item.sessionId === snapshot.recentSessionId &&
      (item.parallelGroupId || (item.parallelLaneCount ?? 0) > 1)
    );
    if (backendSelectedDifferentSession || backendSelectedParallelGroup) {
      pendingChatSession = null;
      return snapshot;
    }
    if (snapshot.recentItems.some((item) => matchesPendingChatSession(item, pendingChatSession as SidebarSessionItem))) {
      pendingChatSession = null;
      return snapshot;
    }
    return {
      ...snapshot,
      recentSessionId: pendingChatSession.sessionId,
      recentItems: [
        pendingChatSession,
        ...snapshot.recentItems.filter((item) => item.sessionId !== pendingChatSession?.sessionId)
      ],
      proofHash: "optimistic-chat-session"
    };
  }

  async function dispatch(command: SidebarCommand, focusTarget: string): Promise<SidebarShadowState> {
    const api = resolveApi();
    emit({
      snapshot: localPreviewSnapshot(internals.snapshot, command),
      lastEvent: "shadow_manifest_recorded",
      lastRequestId: command.requestId,
      focusTarget,
      interactionCount: internals.interactionCount + 1
    });
    const result = await api?.dispatchSidebarCommand(command);
    const remoteSnapshot = await api?.getSidebarSnapshot();
    const snapshot =
      result?.accepted === false && remoteSnapshot
        ? localPreviewSnapshot(remoteSnapshot, command)
        : !remoteSnapshot
          ? internals.snapshot
          : remoteSnapshot;
    return emit({
      snapshot: withPendingChatSession(snapshot),
      lastProof: result?.proofHash ?? "renderer-only",
      lastEvent: result?.event ?? "rejected",
      lastRequestId: command.requestId,
      focusTarget,
      interactionCount: internals.interactionCount + 1
    });
  }

  function commandWithEnvelope(command: SidebarCommandDraft): SidebarCommand {
    return {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: makeSidebarRequestId(),
      ...command
    } as SidebarCommand;
  }

  function beginChatSessionPreview(label: string, workspaceLabel = "Forge"): SidebarShadowState {
    const compactLabel = label.replace(/\s+/g, " ").trim() || "New session";
    const previewLabel = compactLabel.length <= 42 ? compactLabel : `${compactLabel.slice(0, 39).trimEnd()}...`;
    const compactWorkspace = workspaceLabel.trim() || "Forge";
    const previewId = "optimistic-chat-session";
    const previewSession: SidebarSessionItem = {
      sessionId: previewId,
      label: previewLabel,
      date: new Date().toISOString().slice(0, 10),
      section: internals.snapshot.activeSection === "shell" ? "forge" : internals.snapshot.activeSection,
      workspaceLabel: compactWorkspace,
      rowVisible: true,
      pinned: false,
      working: true,
      automated: false,
      archived: false
    };
    pendingChatSession = previewSession;
    return emit({
      snapshot: withPendingChatSession(internals.snapshot),
      lastEvent: "shadow_manifest_recorded",
      lastRequestId: previewId,
      focusTarget: previewId,
      interactionCount: internals.interactionCount + 1
    });
  }

  function finishChatSessionPreview(): SidebarShadowState {
    if (!pendingChatSession) {
      return state;
    }
    pendingChatSession = {
      ...pendingChatSession,
      working: false
    };
    return emit({
      snapshot: withPendingChatSession(internals.snapshot),
      lastEvent: "shadow_manifest_recorded",
      lastRequestId: pendingChatSession.sessionId,
      focusTarget: pendingChatSession.sessionId,
      interactionCount: internals.interactionCount + 1
    });
  }

  return {
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot(): SidebarShadowState {
      return state;
    },
    async boot(): Promise<SidebarShadowState> {
      const api = resolveApi();
      const snapshot = (await api?.getSidebarSnapshot()) ?? previewSnapshotFromLocation();
      return emit({
        snapshot: withPendingChatSession(snapshot),
        lastProof: snapshot.proofHash,
        lastEvent: "boot",
        lastRequestId: "boot",
        booted: true
      });
    },
    beginChatSessionPreview,
    finishChatSessionPreview,
    dispatch,
    async archiveSession(sessionId: string, focusTarget: string): Promise<SidebarShadowState> {
      await dispatch(commandWithEnvelope({ kind: "archive_session", sessionId }), focusTarget);
      return dispatch(commandWithEnvelope({ kind: "confirm_archive" }), focusTarget);
    },
    command(command: SidebarCommandDraft): SidebarCommand {
      return commandWithEnvelope(command);
    }
  };
}

export const sidebarShadowStore = createSidebarShadowStore(browserApi);

export function useSidebarShadowStore(): SidebarShadowState {
  return useSyncExternalStore(sidebarShadowStore.subscribe, sidebarShadowStore.getSnapshot, sidebarShadowStore.getSnapshot);
}
