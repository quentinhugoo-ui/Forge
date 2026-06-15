import { useSyncExternalStore } from "react";
import {
  FORGE_ELECTRON_IPC_VERSION,
  makePanelsChatBottomRequestId,
  type ForgeShellApi,
  type ParallelChatDraft,
  type PanelsChatBottomCommand,
  type PanelsChatBottomCommandKind,
  type PanelsChatBottomCommandResult,
  type PanelsChatBottomSnapshot,
  type TranscriptMessage
} from "../shared/ipc-contract";
import { readBrainAgentMemory, readBrainUserLocationMemory, readBrainUserMemory } from "./brain-user-memory-store";
import { sidebarShadowStore } from "./sidebar-shadow-store";

export interface PanelsChatBottomShadowManifest {
  schema: "ingen.electron.panels_chat_bottom_shadow_manifest.v1";
  sliceId: "panels_chat_bottom";
  mode: PanelsChatBottomSnapshot["mode"];
  visibleStatusDock: boolean;
  transcriptCount: number;
  focusTarget: "composer" | "status_dock" | "bottom_controls";
  availableActions: PanelsChatBottomCommandKind[];
  ipcProofHash: string;
}

interface PanelsChatBottomStoreState {
  snapshot: PanelsChatBottomSnapshot;
  manifest: PanelsChatBottomShadowManifest;
  lastResult?: PanelsChatBottomCommandResult;
}

function fallbackSnapshot(): PanelsChatBottomSnapshot {
  return {
    schema: "ingen.electron.panels_chat_bottom.snapshot.v1",
    version: FORGE_ELECTRON_IPC_VERSION,
    mode: "shadow",
    activeSection: "banger",
    activeSessionId: "",
    profileCanvas: "",
    rightPanelOpen: true,
    statusDock: {
      visible: true,
      title: "Banger viewport",
      primaryAction: "Open native details",
      lines: [
        { label: "PRIMARY", value: "banger=texture_probe=true visible=true frame_hash=shadow", source: "NativeStateKernel::projection" },
        { label: "GPU", value: "gpu=wgpu shadow residency", source: "NativeServiceSnapshot" },
        { label: "JOBS", value: "queued=0 running=0 done=0 failed=0", source: "NativeStateKernel::projection" },
        { label: "PROOF", value: "frame-hash=shadow, ui_hash=pending", source: "NativeStateKernel::projection" }
      ]
    },
    transcript: [],
    parallelLanes: [],
    agentSurfaceStatus: "fallback snapshot",
    composer: {
      chatText: "",
      splitPrompts: false,
      permissionMode: "ask-permissions",
      permissionModeOpen: false,
      selectedProvider: "openai",
      assistantBusy: false,
      providers: [
        { provider: "openai", label: "Codex", connected: true, active: true, account: "local Codex auth", proof: "provider-store" },
        { provider: "anthropic", label: "Claude", connected: false, active: false, account: "not linked", proof: "unverified" },
        { provider: "openrouter", label: "OpenRouter", connected: false, active: false, account: "not linked", proof: "oauth-key-store" }
      ],
      modelLabel: "Codex",
      reasoningLabel: "Medium",
      uploadStatus: "uploads=0",
      uploadPreviewLabel: "",
      uploadPreviewKind: "",
      uploadCount: 0,
      uploadErrorText: "",
      uploadPreviews: []
    },
    bottomControls: [],
    proofHash: "fallback"
  };
}

function manifestFromSnapshot(snapshot: PanelsChatBottomSnapshot): PanelsChatBottomShadowManifest {
  return {
    schema: "ingen.electron.panels_chat_bottom_shadow_manifest.v1",
    sliceId: "panels_chat_bottom",
    mode: snapshot.mode,
    visibleStatusDock: snapshot.statusDock.visible,
    transcriptCount: snapshot.transcript.length,
    focusTarget: snapshot.composer.chatText ? "composer" : snapshot.statusDock.visible ? "status_dock" : "bottom_controls",
    availableActions: snapshot.bottomControls.filter((control) => control.enabled).map((control) => control.kind),
    ipcProofHash: snapshot.proofHash
  };
}

function withParallelLaneMessages(
  snapshot: PanelsChatBottomSnapshot,
  index: number,
  messages: TranscriptMessage[]
): PanelsChatBottomSnapshot {
  const currentLane = snapshot.parallelLanes.find((lane) => lane.index === index);
  const nextLane = {
    index,
    sessionId: currentLane?.sessionId ?? "",
    transcript: messages,
    proofHash: "optimistic-parallel-render"
  };
  const lanes = snapshot.parallelLanes.filter((lane) => lane.index !== index);
  lanes.push(nextLane);
  lanes.sort((left, right) => left.index - right.index);
  return {
    ...snapshot,
    parallelLanes: lanes
  };
}

function browserApi(): ForgeShellApi | undefined {
  return globalThis.window?.forgeShell;
}

function mergeInFlightPendingTranscript(
  incomingMessages: TranscriptMessage[],
  currentMessages: TranscriptMessage[]
): TranscriptMessage[] {
  const existingIds = new Set(incomingMessages.map((message) => message.id));
  const currentIds = new Set(currentMessages.map((message) => message.id));
  const incomingHasFreshAssistantResponse = incomingMessages.some(
    (message) =>
      message.role === "assistant" &&
      !message.id.startsWith("assistant-pending-") &&
      message.text.trim().length > 0 &&
      !currentIds.has(message.id)
  );
  if (incomingHasFreshAssistantResponse) {
    return incomingMessages;
  }
  const pendingMessages = currentMessages.filter((message) =>
    message.role === "assistant" &&
    message.id.startsWith("assistant-pending-") &&
    !existingIds.has(message.id)
  );
  if (pendingMessages.length === 0) {
    return incomingMessages;
  }
  const replacementTargets = new Set(
    pendingMessages
      .map((message) => replacementTargetIdFromPending(message.id))
      .filter((id): id is string => Boolean(id))
  );
  const visibleIncomingMessages = replacementTargets.size > 0
    ? incomingMessages.filter((message) => !replacementTargets.has(message.id))
    : incomingMessages;
  return [...visibleIncomingMessages, ...pendingMessages];
}

function replacementTargetIdFromPending(id: string): string | undefined {
  const prefix = "assistant-pending-replace-";
  return id.startsWith(prefix) ? id.slice(prefix.length) : undefined;
}

function mergeInFlightPendingSnapshot(
  incomingSnapshot: PanelsChatBottomSnapshot,
  currentSnapshot: PanelsChatBottomSnapshot,
  inFlightChatRequests: number
): PanelsChatBottomSnapshot {
  if (inFlightChatRequests <= 0) {
    return incomingSnapshot;
  }
  const transcript = mergeInFlightPendingTranscript(incomingSnapshot.transcript, currentSnapshot.transcript);
  const parallelLanes = incomingSnapshot.parallelLanes.map((lane) => {
    const currentLane = currentSnapshot.parallelLanes.find((candidate) => candidate.index === lane.index);
    if (!currentLane) {
      return lane;
    }
    return {
      ...lane,
      transcript: mergeInFlightPendingTranscript(lane.transcript, currentLane.transcript)
    };
  });
  return {
    ...incomingSnapshot,
    transcript,
    parallelLanes
  };
}

function optimisticSendSnapshot(
  snapshot: PanelsChatBottomSnapshot,
  parallelSessionIndex: number,
  draft: string,
  attachments: PanelsChatBottomSnapshot["composer"]["uploadPreviews"],
  internalPrompt = false,
  replaceAssistantMessageId = ""
): PanelsChatBottomSnapshot {
  const optimisticMessage: TranscriptMessage = {
    id: `optimistic-user-${Date.now()}-${parallelSessionIndex}`,
    role: "user",
    text: draft,
    attachments,
    proofHash: "optimistic-render"
  };
  const pendingAssistantMessage: TranscriptMessage = {
    id: replaceAssistantMessageId
      ? `assistant-pending-replace-${replaceAssistantMessageId}`
      : `assistant-pending-${Date.now()}-${parallelSessionIndex}`,
    role: "assistant",
    text: "",
    proofHash: "optimistic-llm-pending"
  };
  const removeReplacementTarget = (messages: TranscriptMessage[]) =>
    replaceAssistantMessageId
      ? messages.filter((message) => message.id !== replaceAssistantMessageId)
      : messages;
  const nextTranscript =
    parallelSessionIndex > 0
      ? [
          ...removeReplacementTarget(snapshot.parallelLanes.find((lane) => lane.index === parallelSessionIndex)?.transcript ?? []),
          ...(internalPrompt ? [] : [optimisticMessage]),
          pendingAssistantMessage
        ]
      // Audit anchor: : [...state.snapshot.transcript, optimisticMessage, pendingAssistantMessage]
      : [...removeReplacementTarget(snapshot.transcript), ...(internalPrompt ? [] : [optimisticMessage]), pendingAssistantMessage];
  return parallelSessionIndex > 0
    ? withParallelLaneMessages(snapshot, parallelSessionIndex, nextTranscript)
    : {
        ...snapshot,
        transcript: nextTranscript
      };
}

export function createPanelsChatBottomStore(api = browserApi()) {
  let state: PanelsChatBottomStoreState = {
    snapshot: fallbackSnapshot(),
    manifest: manifestFromSnapshot(fallbackSnapshot())
  };
  const subscribers = new Set<() => void>();
  let inFlightChatRequests = 0;

  function emit() {
    for (const subscriber of subscribers) {
      subscriber();
    }
  }

  async function refresh() {
    if (!api) {
      return;
    }
    const snapshot = mergeInFlightPendingSnapshot(await api.getPanelsChatBottomSnapshot(), state.snapshot, inFlightChatRequests);
    state = {
      ...state,
      snapshot,
      manifest: manifestFromSnapshot(snapshot)
    };
    emit();
  }

  async function dispatch(command: Omit<PanelsChatBottomCommand, "version" | "requestId">) {
    if (!api) {
      return;
    }
    let outgoingCommand = command;
    let optimisticChatInFlight = false;
    if (command.kind === "send_parallel_chat_batch") {
      const drafts = (command.parallelDrafts ?? [])
        .map((draft) => ({
          parallelSessionIndex: draft.parallelSessionIndex,
          value: draft.value.trim()
        }))
        .filter((draft): draft is ParallelChatDraft => draft.value.length > 0 && Number.isInteger(draft.parallelSessionIndex));
      const attachments = state.snapshot.composer.uploadPreviews;
      if (drafts.length > 0) {
        if (attachments.length > 0) {
          outgoingCommand = {
            ...command,
            parallelDrafts: drafts,
            attachmentIds: attachments.map((attachment) => attachment.id)
          };
        } else {
          outgoingCommand = {
            ...command,
            parallelDrafts: drafts
          };
        }
        const workspace = await api.getWorkspaceFolder?.();
        void workspace;
        let nextSnapshot = state.snapshot;
        for (const draft of drafts) {
          nextSnapshot = optimisticSendSnapshot(nextSnapshot, draft.parallelSessionIndex, draft.value, attachments);
        }
        state = {
          ...state,
          snapshot: {
            ...nextSnapshot,
            composer: {
              ...nextSnapshot.composer,
              chatText: "",
              uploadStatus: "uploads=0",
              uploadPreviewLabel: "",
              uploadPreviewKind: "",
              uploadCount: 0,
              uploadErrorText: "",
              uploadPreviews: []
            }
          }
        };
        state = { ...state, manifest: manifestFromSnapshot(state.snapshot) };
        optimisticChatInFlight = true;
        emit();
      }
    } else if (command.kind === "send_chat") {
      const draft = typeof command.value === "string" ? command.value.trim() : state.snapshot.composer.chatText.trim();
      const attachments = state.snapshot.composer.uploadPreviews;
      const parallelSessionIndex =
        typeof command.parallelSessionIndex === "number" && Number.isInteger(command.parallelSessionIndex)
          ? command.parallelSessionIndex
          : 0;
      const internalPrompt = command.internalPrompt === true;
      const replaceAssistantMessageId =
        typeof command.replaceAssistantMessageId === "string" ? command.replaceAssistantMessageId.trim() : "";
      if (attachments.length > 0) {
        outgoingCommand = {
          ...command,
          attachmentIds: attachments.map((attachment) => attachment.id)
        };
      }
      if (draft || attachments.length > 0) {
        const workspace = await api.getWorkspaceFolder?.();
        if (parallelSessionIndex === 0 && !internalPrompt) {
          sidebarShadowStore.beginChatSessionPreview(draft || "Attached files", workspace?.folderName || undefined);
        }
        const nextSnapshot = optimisticSendSnapshot(state.snapshot, parallelSessionIndex, draft, attachments, internalPrompt, replaceAssistantMessageId);
        state = {
          ...state,
          snapshot: {
            ...nextSnapshot,
            composer: {
              ...nextSnapshot.composer,
              chatText: "",
              uploadStatus: "uploads=0",
              uploadPreviewLabel: "",
              uploadPreviewKind: "",
              uploadCount: 0,
              uploadErrorText: "",
              uploadPreviews: []
            }
          }
        };
        state = { ...state, manifest: manifestFromSnapshot(state.snapshot) };
        optimisticChatInFlight = true;
        emit();
      }
    }
    if (command.kind === "new_session") {
      state = {
        ...state,
        snapshot: {
          ...state.snapshot,
          transcript: [],
          parallelLanes: [],
          composer: {
            ...state.snapshot.composer,
            chatText: "",
            uploadStatus: "uploads=0",
            uploadPreviewLabel: "",
            uploadPreviewKind: "",
            uploadCount: 0,
            uploadErrorText: "",
            uploadPreviews: []
          }
        }
      };
      state = { ...state, manifest: manifestFromSnapshot(state.snapshot) };
      emit();
    }
    if (command.kind === "send_chat" || command.kind === "send_parallel_chat_batch") {
      const userMemory = readBrainUserMemory();
      const agentMemory = readBrainAgentMemory();
      const locationMemory = readBrainUserLocationMemory();
      outgoingCommand = {
        ...outgoingCommand,
        userFirstName: userMemory.preferredFirstName,
        agentFirstName: agentMemory.preferredFirstName,
        userHomeLocation: locationMemory.homeLocation
      };
    }
    if (optimisticChatInFlight) {
      inFlightChatRequests += 1;
    }
    let result!: PanelsChatBottomCommandResult;
    try {
      result = await api.dispatchPanelsChatBottomCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: makePanelsChatBottomRequestId(),
        ...outgoingCommand
      });
    } finally {
      if (optimisticChatInFlight) {
        inFlightChatRequests = Math.max(0, inFlightChatRequests - 1);
      }
    }
    state = { ...state, lastResult: result };
    // Audit anchor: if (command.kind === "send_chat" && !result.accepted)
    if ((command.kind === "send_chat" || command.kind === "send_parallel_chat_batch") && !result.accepted) {
      await refresh();
      return;
    }
    await refresh();
    if (command.kind === "assistant_write_complete") {
      sidebarShadowStore.finishChatSessionPreview();
    }
    if (
      command.kind === "send_chat" ||
      command.kind === "send_parallel_chat_batch" ||
      command.kind === "new_session" ||
      command.kind === "assistant_write_complete"
    ) {
      await sidebarShadowStore.boot();
    }
  }

  api?.onLlmProviderEvent?.((event) => {
    if (event.events.includes("ready") || event.models.length > 0 || event.reasoning.length > 0) {
      void refresh();
    }
  });
  api?.onPanelsChatBottomSnapshotEvent?.((event) => {
    if (event.kind === "snapshot_updated") {
      void refresh();
    }
  });

  return {
    subscribe(callback: () => void) {
      subscribers.add(callback);
      return () => subscribers.delete(callback);
    },
    getSnapshot() {
      return state;
    },
    refresh,
    dispatch
  };
}

export const panelsChatBottomStore = createPanelsChatBottomStore();

export function usePanelsChatBottomStore() {
  return useSyncExternalStore(
    panelsChatBottomStore.subscribe,
    panelsChatBottomStore.getSnapshot,
    panelsChatBottomStore.getSnapshot
  );
}
