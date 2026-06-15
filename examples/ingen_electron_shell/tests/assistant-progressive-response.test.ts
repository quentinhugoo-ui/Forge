import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(root, "src", "preload", "preload.ts"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const ipcContractSource = readFileSync(join(root, "src", "shared", "ipc-contract.ts"), "utf8");
const animationSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");
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
    expect(mainSource).not.toContain('outcome: "max_steps"');
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
    expect(mainSource).toContain("shouldStop: (text) => Boolean(params.assistantRun?.cancelled || extractAgentActionJsonRequest(text))");
    expect(mainSource).toContain("liveTextSink");
    expect(mainSource).toContain("continuationLiveSink");
    expect(mainSource).toContain('kind: "text_delta"');
  });

  it("lets the composer stop an active assistant run", () => {
    expect(animationSource).toContain("snapshot.composer.assistantBusy");
    expect(animationSource).toContain('activityText = "is thinking"');
    expect(animationSource).toContain("ASSISTANT_WORKING_STATUS_DELAY_MS");
    expect(animationSource).toContain("ASSISTANT_WORKING_STATUS_EXIT_MS");
    expect(animationSource).toContain("function AssistantWorkingStatus");
    expect(animationSource).toContain("function agentWorkingStatusText");
    expect(animationSource).toContain("function canShowAssistantWorkingStatus");
    expect(animationSource).toContain("assistantWorkingCandidate && canShowAssistantWorkingStatus(assistantWorkingEvent)");
    expect(animationSource).toContain("is continuing in ${brainSegmentName(event.command)}");
    expect(animationSource).toContain("return true;");
    expect(animationSource).toContain("sessionRow__loaderViewbox assistantThinkingEvent__loaderViewbox");
    expect(animationSource).toContain("latestTranscriptEvent(message.text)");
    expect(animationSource).toContain("is running a confirmed shell command");
    expect(animationSource).toContain("is inspecting files");
    expect(animationSource).toContain("is applying a local action");
    expect(stylesSource).toContain("assistantThinkingEvent__loaderViewbox");
    expect(stylesSource).toContain(".assistantText--pending .loader");
    expect(stylesSource).toContain("assistantWorkingEvent--leaving");
    expect(stylesSource).toContain("assistantWorkingStatusIn");
    expect(stylesSource).toContain("assistantWorkingStatusOut");
    expect(animationSource).not.toContain("formatAssistantActivityElapsed");
    expect(animationSource).not.toContain("assistantThinkingEvent__elapsed");
    expect(animationSource).not.toContain("is switching to");
    expect(stylesSource).not.toContain("assistantThinkingEvent__elapsed");
    expect(animationSource).not.toContain("tokens");
    expect(animationSource).toContain("assistantWorking");
    expect(animationSource).toContain("assistantWorkingEvent");
    expect(animationSource).toContain("assistantStopActive");
    expect(animationSource).toContain('kind: "stop_assistant"');
    expect(animationSource).toContain("Stop assistant");
    expect(animationSource).toContain("composer__send--stop");
    expect(animationSource).toContain("disabled={!canSend && !assistantStopActive}");
    expect(mainSource).toContain("interface AssistantRunControl");
    expect(mainSource).toContain("stopActiveAssistantRunControl");
    expect(mainSource).toContain("throwIfAssistantRunCancelled");
    expect(mainSource).toContain('text: "Stopped by user."');
  });

  it("injects the full local action atlas at boot and keeps per-turn reminders compact", () => {
    expect(mainSource).toContain("function shouldInjectFullAgentActionManifest");
    expect(mainSource).toContain("function shouldInjectCompactAgentActionManifest");
    expect(mainSource).toContain("function agentActionContinuationManifest");
    expect(mainSource).toContain("function brainRuntimeReminderManifest");
    expect(mainSource).toContain("LOCAL_ACTION_ATLAS_BOOT v1");
    expect(mainSource).toContain("full local action atlas is injected at session boot and after conversation compaction only");
    expect(mainSource).toContain("boot_manifest=already_injected_once_for_this_session_or_reinjected_after_compaction");
    expect(mainSource).toContain("codeact_loop_rule=Use Brain CodeActs as loop-stream events");
    expect(mainSource).toContain("codeact_loop_rule=For non-trivial work, Brain CodeActs are loop-stream events");
    expect(mainSource).toContain("function brainCodeActLoopRulesManifest");
    expect(mainSource).toContain("brainCodeActLoopRulesManifest()");
    expect(mainSource).toContain("capabilities_codeact=");
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
    expect(mainSource).toContain("full_atlas_location=brain_boot_manifest_or_post_compaction_reinjection");
    expect(mainSource).toContain("agentActionContinuationManifest()");
    expect(mainSource).toContain("[agentActionRoutingHint(), agentActionContinuationManifest()].join(\"\\n\")");
    expect(mainSource).not.toContain("return [agentActionRoutingHint(), agentActionHostPromptManifest(agentActionHostConfig())].join(\"\\n\")");
    expect(mainSource).toContain(": \"\"");
    expect(mainSource).toContain("agentActionContextManifest(userText, transcript)");
  });

  it("keeps Brain CodeActs inside the agentic loop instead of final chatbot prose", () => {
    expect(mainSource).toContain("const BRAIN_CODEACT_COMMANDS_BY_LENGTH");
    expect(mainSource).toContain("function brainCodeActCommandsFromAssistantText");
    expect(mainSource).toContain("function isBrainCodeActUserPauseCommand");
    expect(mainSource).toContain("function isBrainCodeActSurfaceCommand");
    expect(mainSource).toContain("function shouldContinueAfterBrainCodeAct");
    expect(mainSource).toContain("function brainCodeActLoopContinuationUserText");
    expect(mainSource).toContain("BRAIN_CODEACT_LOOP_CONTINUATION v1");
    expect(mainSource).toContain("Le ou les CodeActs Brain precedents sont des evenements de loop stream");
    expect(mainSource).toContain("previous_visible_progress=");
    expect(mainSource).toContain("assistant-response-${Date.now()}-brain-codeact");
    expect(mainSource).toContain("shouldContinueAfterBrainCodeAct({");
    expect(mainSource).toContain("continuationMessage = await executeAssistantAgentActionLoop({");
    expect(mainSource).toContain("archiveTranscriptMessage(session, continuationMessage)");
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
    expect(mainSource).toContain("Je commence le tri par les elements les plus evidents.");
    expect(mainSource).toContain("Je deplace ${sourceName} vers ${targetFolder}");
    expect(mainSource).not.toContain("Etape ${index + 1}/${total}");
    expect(mainSource).not.toContain("sans toucher aux raccourcis d'application");
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
    expect(mainSource).not.toContain("AGENT_ACTION_LOOP_DEFAULT_MAX_STEPS");
    expect(mainSource).not.toContain("AGENT_ACTION_LOOP_FILE_MUTATION_MAX_STEPS");
    expect(mainSource).not.toContain("function agentActionLoopMaxStepsForObjective");
    expect(mainSource).toContain("function agentActionShouldRunFileOrganizationFallback");
    expect(mainSource).not.toContain("La limite de boucle approche");
    expect(mainSource).not.toContain("No further action was executed after this guard fired.");
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
    expect(mainSource).not.toContain("Final summary: agent loop max_steps");
    expect(mainSource).toContain("while (true)");
    expect(mainSource).toContain("stopActiveAssistantRunControl");
    expect(mainSource).toContain('case "stop_assistant"');
    expect(mainSource).toContain('outcome: reportableAgentActionLoopOutcome(loopState, "blocked")');
    expect(mainSource).not.toContain('outcome: "deterministic_fallback"');
  });

  it("keeps filesystem move events separate from file content edit counters", () => {
    expect(animationSource).toContain("function isAgentFileModificationCommand");
    expect(animationSource).toContain("return command === AGENT_COPY_PATH_COMMAND");
    expect(animationSource).toContain("return delta.addedChars > 0 || delta.removedChars > 0 ? delta : undefined");
    expect(animationSource).toContain("is applying a local action");
    expect(animationSource).not.toContain("is using ${command}");
    expect(animationSource).toContain("Modified");
    expect(animationSource).not.toContain("Modification de");
  });

  it("keeps shell payloads out of the readable assistant transcript", () => {
    expect(animationSource).toContain("function isShellPayloadLine");
    expect(animationSource).toContain("isShellCodeActCommand(lastEvent?.command)");
    expect(animationSource).toContain("lastEvent.detail = shellPayloadDisplayText(line)");
    expect(animationSource).toContain("Copy-Item|ConvertTo-Json|ForEach-Object|Get-ChildItem");
  });
});
