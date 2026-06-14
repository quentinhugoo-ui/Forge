import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(root, "src", "preload", "preload.ts"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const ipcContractSource = readFileSync(join(root, "src", "shared", "ipc-contract.ts"), "utf8");
const animationSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");

describe("assistant progressive response feed", () => {
  it("pushes transcript snapshot events while a chat command is still running", () => {
    expect(ipcContractSource).toContain("export interface PanelsChatBottomSnapshotEvent");
    expect(preloadSource).toContain('ipcRenderer.on("forge:panels-chat-bottom-snapshot-event"');
    expect(storeSource).toContain("api?.onPanelsChatBottomSnapshotEvent?.");
    expect(mainSource).toContain('webContents.send("forge:panels-chat-bottom-snapshot-event"');
  });

  it("seeds large assistant messages before committing the full final response", () => {
    expect(mainSource).toContain("ASSISTANT_PROGRESSIVE_SEED_MIN_CHARS");
    expect(mainSource).toContain("commitAssistantMessageWithProgressiveSeed");
    expect(mainSource).toContain('emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed"');
    expect(storeSource).toContain("incomingHasFreshAssistantResponse");
  });

  it("keeps the existing assistant writing animation implementation untouched", () => {
    expect(animationSource).toContain("function AnimatedAssistantText");
    expect(animationSource).toContain("const animationSource = useMemo(() => assistantVisibleAnimationSource(message.text), [message.text]);");
    expect(animationSource).toContain("const visibleText = animationSource.slice(0, visibleCharacters);");
  });

  it("keeps pending-response animation live across session materialization", () => {
    expect(animationSource).toContain("const keepDraftResponseLive = previous.hadPending");
    expect(animationSource).not.toContain('previous.sessionId === "" && previous.hadPending');
  });

  it("runs agent action requests as a visible loop stream", () => {
    expect(mainSource).toContain('const AGENT_ACTION_JSON_PREFIX = "AGENT_ACTION_JSON"');
    expect(mainSource).toContain('const AGENT_ACTION_RESULT_PREFIX = "AGENT_ACTION_RESULT v1"');
    expect(mainSource).toContain("function executeAssistantAgentActionLoop");
    expect(mainSource).toContain("extractAgentActionJsonRequest(assistantMessage.text)");
    expect(mainSource).toContain("executeAgentActionRequest(agentActionHostConfig(), extracted.request)");
    expect(mainSource).toContain("agentActionLoopContinuationUserText");
    expect(mainSource).toContain("params.commitTranscript(transcriptWithMessage(params.baseTranscript, assistantMessage))");
    expect(mainSource).toContain("assistantMessage = await executeAssistantAgentActionLoop({");
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
