import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const generatedSource = readFileSync(join(process.cwd(), "src", "shared", "generated", "forge-ipc.generated.ts"), "utf8");
const brainSource = readFileSync(join(process.cwd(), "..", "..", "src", "brain.rs"), "utf8");

describe("session rename CodeAct", () => {
  it("injects and executes a Brain-owned short title command", () => {
    expect(brainSource).toContain('pub const BRAIN_RENAME_SESSION_COMMAND: &str = "/rename_session_"');
    expect(brainSource).toContain("brain_rename_session_codeact_template()");
    expect(brainSource).toContain("On the first user message of a session");
    expect(generatedSource).toContain('export const BRAIN_RENAME_SESSION_COMMAND = "/rename_session_" as const;');
    expect(generatedSource).toContain("BRAIN_RENAME_SESSION_COMMAND_DESCRIPTION");
    expect(mainSource).toContain("Au premier message utilisateur de cette session");
    expect(mainSource).toContain("function firstTurnSessionRenameInput");
    expect(mainSource).toContain("function transcriptHasAssistantResponse");
    expect(mainSource).toContain("SESSION_TITLE_CODEACT_REQUIRED v1");
    expect(mainSource).toContain("N'ecris aucune phrase visible sur le renommage");
    expect(mainSource).toContain("pas seulement un nom propre");
    expect(mainSource).toContain("Decouverte de Kagoshima");
    expect(mainSource).toContain("...firstTurnSessionRenameInput(userMessageId, transcript)");
    expect(mainSource).toContain('const PENDING_LLM_SESSION_TITLE = "New session"');
    expect(mainSource).toContain("label: PENDING_LLM_SESSION_TITLE");
    expect(mainSource).not.toContain("function sessionLabelFromDraft");
    expect(mainSource).toContain("extractRenameSessionCodeAct");
    expect(mainSource).toContain('[^"\\r\\n]{0,120}');
    expect(mainSource).toContain("function polishedSessionTitle");
    expect(mainSource).toContain("Decouverte de ${compact}");
    expect(mainSource).toContain("Histoire de ${copiedHistory[1]}");
    expect(mainSource).toContain("renameChatSession(session, request)");
    expect(mainSource).toContain("archiveSession.title = request.title");
    expect(mainSource).toContain("removeRenameSessionChatter(removeRenameSessionCodeActLines(message.text))");
    expect(mainSource).not.toContain("renderRenameSessionCodeActResult");
    expect(mainSource).not.toContain("RENAME_SESSION_RESULT");
    expect(mainSource).toContain("executeAssistantRenameSessionCodeAct(assistantMessage, session)");
  });
});
