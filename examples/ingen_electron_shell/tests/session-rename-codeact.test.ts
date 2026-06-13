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
    expect(mainSource).toContain('const PENDING_LLM_SESSION_TITLE = "New session"');
    expect(mainSource).toContain("label: PENDING_LLM_SESSION_TITLE");
    expect(mainSource).not.toContain("function sessionLabelFromDraft");
    expect(mainSource).toContain("extractRenameSessionCodeAct");
    expect(mainSource).toContain("renameChatSession(session, request)");
    expect(mainSource).toContain("archiveSession.title = request.title");
    expect(mainSource).toContain("RENAME_SESSION_RESULT");
    expect(mainSource).toContain("executeAssistantRenameSessionCodeAct(assistantMessage, session)");
  });
});
