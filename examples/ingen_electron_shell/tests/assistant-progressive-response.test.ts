import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(root, "src", "preload", "preload.ts"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const ipcContractSource = readFileSync(join(root, "src", "shared", "ipc-contract.ts"), "utf8");
const animationSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const agentActionLoopSource = readFileSync(join(root, "src", "main", "agent-action-loop.ts"), "utf8");

describe("assistant progressive response feed", () => {
  it("pushes transcript snapshot events while a chat command is still running", () => {
    expect(ipcContractSource).toContain("export interface PanelsChatBottomSnapshotEvent");
    expect(ipcContractSource).toContain("export interface AgentRuntimeEvent");
    expect(ipcContractSource).toContain('schema: "ingen.agent_runtime.event.v1"');
    expect(preloadSource).toContain('ipcRenderer.on("forge:panels-chat-bottom-snapshot-event"');
    expect(preloadSource).toContain('ipcRenderer.on("forge:agent-runtime-event"');
    expect(storeSource).toContain("api?.onPanelsChatBottomSnapshotEvent?.");
    expect(mainSource).toContain('webContents.send("forge:panels-chat-bottom-snapshot-event"');
    expect(mainSource).toContain('webContents.send("forge:agent-runtime-event"');
  });

  it("seeds large assistant messages before committing the full final response", () => {
    expect(mainSource).toContain("ASSISTANT_PROGRESSIVE_SEED_MIN_CHARS");
    expect(mainSource).toContain("commitAssistantMessageWithProgressiveSeed");
    expect(mainSource).toContain('emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed"');
    expect(storeSource).toContain("incomingHasFreshAssistantResponse");
  });

  it("keeps the existing assistant writing animation implementation untouched", () => {
    expect(animationSource).toContain("function assistantRenderableText");
    expect(animationSource).toContain("_renamechat_");
    expect(animationSource).toContain("_renamechat_?)?");
    expect(animationSource).toContain("\\s*$");
    expect(animationSource).toContain("assistantRenderableText(text)");
    expect(animationSource).toContain("function AnimatedAssistantText");
    expect(animationSource).toContain("const renderableText = useMemo(() => assistantRenderableText(message.text), [message.text]);");
    expect(animationSource).toContain("const animationSource = useMemo(() => assistantVisibleAnimationSource(renderableText), [renderableText]);");
    expect(animationSource).toContain("const visibleText = animationSource.slice(0, visibleCharacters);");
  });

  it("keeps pending-response animation live across session materialization", () => {
    expect(animationSource).toContain("const keepDraftResponseLive = previous.hadPending");
    expect(animationSource).not.toContain('previous.sessionId === "" && previous.hadPending');
  });

  it("runs agent action requests as a visible loop stream", () => {
    expect(agentActionLoopSource).toContain('export const AGENT_ACTION_JSON_PREFIX = "AGENT_ACTION_JSON"');
    expect(mainSource).toContain('const AGENT_ACTION_RESULT_PREFIX = "AGENT_ACTION_RESULT v1"');
    expect(mainSource).toContain("function executeAssistantAgentActionLoop");
    expect(mainSource).toContain("interface AgentRuntimeEventQueue");
    expect(mainSource).toContain("function createAgentRuntimeEventQueue");
    expect(mainSource).toContain("agentEvents: AgentRuntimeEventQueue");
    expect(mainSource).toContain("extractAgentActionJsonRequest(assistantMessage.text)");
    expect(mainSource).toContain("executeAgentActionRequest(agentActionHostConfig(), extracted.request)");
    expect(mainSource).toContain("emitAgentRuntimeToolCallStarted");
    expect(mainSource).toContain('kind: "tool_call_started"');
    expect(mainSource).toContain('kind: "tool_result"');
    expect(mainSource).toContain('kind: "tool_call_completed"');
    expect(mainSource).toContain("agentActionLoopContinuationUserText");
    expect(mainSource).toContain("agentActionStepNeedsMutationFollowUp");
    expect(mainSource).toContain("agentActionForcedContinuationUserText");
    expect(mainSource).toContain("AGENT_ACTION_FORCED_CONTINUATION v1");
    expect(mainSource).toContain("applyDeterministicOrganizationFallback");
    expect(mainSource).toContain("deterministicOrganizationRequestsFromList");
    expect(mainSource).toContain("function renderPendingAgentActionText");
    expect(mainSource).toContain("function renderCompletedPendingAgentActionText");
    expect(mainSource).toContain("agentActionLoopPendingStep");
    expect(mainSource).toContain("deterministicAgentActionFallbackPending");
    expect(mainSource).toContain("const fallbackMessage = await applyDeterministicOrganizationFallback");
    expect(mainSource).toContain("params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage))");
    expect(mainSource).toContain("assistantMessage = await executeAssistantAgentActionLoop({");
    expect(agentActionLoopSource).toContain("let markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX)");
    expect(agentActionLoopSource).toContain("export function jsonObjectEndIndex");
    expect(agentActionLoopSource).toContain("export function removeAgentActionJsonFragment");
    expect(agentActionLoopSource).toContain("export function agentActionLiveVisibleText");
  });

  it("streams provider text into the transcript and cuts over to tools live", () => {
    expect(mainSource).toContain("interface ProviderLiveTextSink");
    expect(mainSource).toContain("interface AssistantProviderAdapter");
    expect(mainSource).toContain("const ASSISTANT_PROVIDER_ADAPTERS");
    expect(mainSource).toContain("function assistantProviderAdapterFor");
    expect(mainSource).toContain("const adapter = assistantProviderAdapterFor(profile)");
    expect(mainSource).toContain("const run = await adapter.run");
    expect(mainSource).toContain("async function readCodexDirectEventStream(response: Response, liveSink?: ProviderLiveTextSink)");
    expect(mainSource).toContain("liveSink?.onText(finalText.trimEnd())");
    expect(mainSource).toContain("liveSink?.shouldStop?.(finalText)");
    expect(mainSource).toContain("await reader.cancel().catch(() => undefined)");
    expect(mainSource).toContain("function runProviderCommandStreamingText");
    expect(mainSource).toContain("claudeStreamLineText");
    expect(mainSource).toContain("async function readOpenRouterChatCompletionStream(response: Response, liveSink?: ProviderLiveTextSink)");
    expect(mainSource).toContain("function openRouterStreamDeltaText");
    expect(mainSource).toContain("body.stream = true");
    expect(mainSource).toContain("runOpenRouterChatCompletion(");
    expect(mainSource).toContain("params.profile");
    expect(mainSource).toContain("params.providerUserText");
    expect(mainSource).toContain("function createAssistantLiveTextSink");
    expect(mainSource).toContain("removeRenameSessionCodeActLines(agentActionLiveVisibleText(text))");
    expect(mainSource).toContain("transcriptWithReplacedMessage(params.baseTranscript, liveMessage)");
    expect(mainSource).toContain("shouldStop: (text) => Boolean(extractAgentActionJsonRequest(text))");
    expect(mainSource).toContain("liveTextSink");
    expect(mainSource).toContain("continuationLiveSink");
    expect(mainSource).toContain('kind: "text_delta"');
  });

  it("injects the heavy local action manifest lazily", () => {
    expect(mainSource).toContain("function shouldInjectFullAgentActionManifest");
    expect(mainSource).toContain("textLooksLikeLocalActionIntent(userText)");
    expect(mainSource).toContain("transcriptHasRecentAgentActionLoop(transcript)");
    expect(mainSource).toContain("shouldInjectFullAgentActionManifest(userText, transcript)");
    expect(mainSource).toContain("? [agentActionRoutingHint(), agentActionHostPromptManifest(agentActionHostConfig())].join(\"\\n\")");
    expect(mainSource).toContain(": \"\"");
    expect(mainSource).toContain("agentActionContextManifest(userText, transcript)");
  });
});
