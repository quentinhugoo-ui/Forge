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
const normalizedMainSource = mainSource.replace(/\r\n/g, "\n");

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
    expect(mainSource).toContain("function emitAgentLoopDiagnosticSummary");
    expect(mainSource).toContain('outcome: "completed"');
    expect(mainSource).toContain('outcome: reportableAgentActionLoopOutcome(loopState, "blocked")');
    expect(mainSource).toContain('outcome: "max_steps"');
    expect(mainSource).toContain("tool_steps=");
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
    expect(mainSource).toContain("function shouldInjectCompactAgentActionManifest");
    expect(mainSource).toContain("function agentActionContinuationManifest");
    expect(mainSource).toContain("manifest.runtime.manifestHash");
    expect(mainSource).toContain("manifest.runtime.atlasHash");
    expect(mainSource).toContain("delta_policy=${manifest.runtime.injectionPolicy}");
    expect(mainSource).toContain("prompt_budget=${manifest.runtime.promptBudget}");
    expect(mainSource).toContain("token_estimate_compact=${manifest.runtime.promptTokenEstimate.compactContinuation}");
    expect(mainSource).toContain("installed_tools_delta=${manifest.runtime.installedToolIds.join(\"|\")}");
    expect(mainSource).toContain("missing_tools_delta=${manifest.runtime.missingToolIds.join(\"|\")}");
    expect(mainSource).toContain("selected_capability_policy=Use AGENT_ACTION_SELECTED_CAPABILITY");
    expect(mainSource).toContain("textLooksLikeLocalActionIntent(userText)");
    expect(mainSource).toContain("textIsAgentActionContinuation(userText)");
    expect(mainSource).toContain("transcriptHasRecentAgentActionLoop(transcript)");
    expect(mainSource).toContain("shouldInjectFullAgentActionManifest(userText, transcript)");
    expect(mainSource).toContain("shouldInjectCompactAgentActionManifest(userText, transcript)");
    expect(mainSource).toContain("return [agentActionRoutingHint(), agentActionHostPromptManifest(agentActionHostConfig())].join(\"\\n\")");
    expect(mainSource).toContain("[agentActionRoutingHint(), agentActionContinuationManifest()].join(\"\\n\")");
    expect(mainSource).toContain(": \"\"");
    expect(mainSource).toContain("agentActionContextManifest(userText, transcript)");
  });

  it("keeps loop continuation results compact before reinjection", () => {
    expect(mainSource).toContain("const AGENT_ACTION_RESULT_ITEM_LIMIT = 10");
    expect(mainSource).toContain("const AGENT_ACTION_RESULT_MATCH_LIMIT = 8");
    expect(mainSource).toContain("const AGENT_ACTION_RESULT_PREVIEW_BYTES = 6_000");
    expect(mainSource).toContain("function compactAgentActionItems");
    expect(mainSource).toContain("function compactAgentActionMatches");
    expect(mainSource).toContain("omittedItems");
    expect(mainSource).toContain("omittedMatches");
    expect(mainSource).toContain("trimUtf8Bytes(result.stdoutPreview, AGENT_ACTION_RESULT_PREVIEW_BYTES)");
  });

  it("has diagnostics for desktop organization, tool-result resumption, compaction, and event/prompt separation", () => {
    expect(mainSource).toContain("function deterministicOrganizationRequestsFromList");
    expect(mainSource).toContain("function agentActionFileOrganizationRequests");
    expect(mainSource).toContain("AGENT_ACTION_VISIBLE_SHORTCUT_EXTENSIONS");
    expect(mainSource).toContain("AGENT_ACTION_DEFAULT_COLLECTION_FOLDER_NAME");
    expect(mainSource).toContain("AGENT_ACTION_DETERMINISTIC_STEP_DELAY_MS");
    expect(mainSource).toContain("function agentActionShouldLeavePathInPlace");
    expect(mainSource).toContain("function agentActionFileOrganizationProgressText");
    expect(mainSource).toContain("function agentActionFileOrganizationFinalText");
    expect(mainSource).toContain("await waitAgentActionVisualStep()");
    expect(mainSource).toContain("Principe universel d'agent local");
    expect(mainSource).toContain("si un outil echoue");
    expect(mainSource).toContain("Je range ${sourceName} dans son dossier");
    expect(mainSource).toContain("agentActionOrganizeCategory(item)");
    expect(mainSource).toContain("action: \"create_directory\"");
    expect(mainSource).toContain("action: \"move_path\"");
    expect(mainSource).toContain("agentActionStepNeedsMutationFollowUp(params.originalUserText, extracted.request, result)");
    expect(mainSource).toContain("agentActionLoopContinuationUserText(params.originalUserText, extracted.request, result, step)");
    expect(mainSource).toContain("function agentActionFailureContinuationUserText");
    expect(mainSource).toContain("PowerShell, cmd.exe, winget, reg.exe, schtasks, netsh, DISM");
    expect(mainSource).toContain("agentActionLoopFailureContinuationStep");
    expect(mainSource).toContain("AGENT_ACTION_RESULT_PREFIX");
    expect(mainSource).toContain("compactAgentActionResult(result)");
    expect(mainSource).toContain("AGENT_ACTION_LOOP_DEFAULT_MAX_STEPS");
    expect(mainSource).toContain("AGENT_ACTION_LOOP_FILE_MUTATION_MAX_STEPS");
    expect(mainSource).toContain("function agentActionLoopMaxStepsForObjective");
    expect(mainSource).toContain("function agentActionShouldRunFileOrganizationFallback");
    expect(mainSource).toContain("La limite de boucle approche");
    expect(mainSource).toContain("No further action was executed after this guard fired.");
    expect(mainSource).toContain("function agentActionSelectedCapabilityContext");
    expect(mainSource).toContain("agentActionCapabilityDetailManifest(agentActionHostConfig(), capabilityId)");
    expect(mainSource).toContain("AGENT_ACTION_COMPACTION_STATE v1");
    expect(mainSource).toContain("last_tool_ground_truth=");
    expect(mainSource).toContain("After compaction, continue the same observe-act-verify-retry loop");
    expect(normalizedMainSource).toContain("result.accepted\n      ? \"next=verify whether the user objective is now satisfied");
    expect(mainSource).toContain("agentActionSelectedCapabilityContext(request, result)");
    expect(normalizedMainSource).toContain("agentEvents.emit({\n      kind: \"compaction_started\"");
    expect(normalizedMainSource).toContain("agentEvents.emit({\n      kind: \"compaction_completed\"");
    expect(mainSource).toContain("window.webContents.send(\"forge:agent-runtime-event\", runtimeEvent)");
    expect(mainSource).toContain("!message.id.startsWith(\"assistant-status-\")");
    expect(mainSource).not.toContain("providerConversationMessages(\n  event");
  });

  it("uses a universal action-loop state instead of the desktop fallback as the normal path", () => {
    expect(ipcContractSource).toContain('schema: "ingen.agent_action_loop.state.v1"');
    expect(ipcContractSource).toContain("export type AgentActionLoopOutcome");
    expect(ipcContractSource).toContain("export interface AgentActionLoopState");
    expect(mainSource).toContain("type AgentActionLoopState");
    expect(mainSource).toContain("const AGENT_ACTION_COMPAT_DETERMINISTIC_FALLBACK");
    expect(mainSource).toContain("function createAgentActionLoopState");
    expect(mainSource).toContain("function agentActionLoopWithResult");
    expect(mainSource).toContain("function agentActionLoopWithStatus");
    expect(mainSource).toContain("function reportableAgentActionLoopOutcome");
    expect(mainSource).toContain("function ensureAgentActionLoopFinalSummary");
    expect(mainSource).toContain("Final summary: agent loop ${state.finalStatus}");
    expect(mainSource).toContain("AGENT_ACTION_COMPAT_DETERMINISTIC_FALLBACK || agentActionShouldRunFileOrganizationFallback");
    expect(mainSource).toContain("Final summary: agent loop max_steps; incomplete after ${maxSteps} local-action steps.");
    expect(mainSource).toContain('outcome: reportableAgentActionLoopOutcome(loopState, "blocked")');
    expect(mainSource).not.toContain('outcome: "deterministic_fallback"');
  });

  it("keeps filesystem move events separate from file content edit counters", () => {
    expect(animationSource).toContain("function isAgentFileModificationCommand");
    expect(animationSource).toContain("return command === AGENT_COPY_PATH_COMMAND");
    expect(animationSource).toContain("return delta.addedChars > 0 || delta.removedChars > 0 ? delta : undefined");
    expect(animationSource).toContain("Modified");
    expect(animationSource).not.toContain("Modification de");
  });
});
