import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const generatedSource = readFileSync(join(process.cwd(), "src", "shared", "generated", "forge-ipc.generated.ts"), "utf8");
const brainSource = readFileSync(join(process.cwd(), "..", "..", "src", "brain.rs"), "utf8");
const rendererSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");

describe("session rename CodeAct", () => {
  it("uses only the silent /rename_session_<title>_ prefix tag", () => {
    expect(brainSource).toContain('pub const BRAIN_RENAME_SESSION_COMMAND: &str = "/rename_session_"');
    expect(brainSource).toContain("/rename_session_<short title>_");
    expect(brainSource).toContain("The app strips the tag before display");
    expect(brainSource).toContain("Never mention, explain, echo, format, or discuss this rename action");
    expect(generatedSource).toContain('export const BRAIN_RENAME_SESSION_COMMAND = "/rename_session_" as const;');
    expect(generatedSource).toContain("/rename_session_<short title>_");
    expect(generatedSource).toContain("The app strips the tag before display");
    expect(mainSource).toContain("On the first assistant response in this session");
    expect(mainSource).toContain("/rename_session_<short title>_");
    expect(mainSource).toContain("The app strips the tag before display");
    expect(mainSource).toContain("const RENAME_SESSION_TAG_PATTERN");
    expect(mainSource).toContain("const RENAME_SESSION_TAG_FRAGMENT_PATTERN");
    expect(mainSource).toContain("trimmed.match(RENAME_SESSION_TAG_PATTERN)");
    expect(mainSource).toContain("brain_rename_session_tag");
    expect(mainSource).toContain("renameChatSession(session, request)");
    expect(mainSource).toContain("archiveSession.title = request.title");
    expect(rendererSource).toContain("BRAIN_RENAME_SESSION_COMMAND");
    expect(rendererSource).toContain("SILENT_TRANSCRIPT_CODEACT_COMMANDS");
    expect(rendererSource).toContain("function isSilentTranscriptCodeActLine");
    expect(rendererSource).toContain("line.replace(RENAME_SESSION_TAG_FRAGMENT_PATTERN");
    expect(rendererSource).toContain("!SILENT_TRANSCRIPT_CODEACT_COMMANDS.has(command)");
  });

  it("removes the old compact parser and runtime title guesser", () => {
    const combined = [brainSource, generatedSource, mainSource, rendererSource].join("\n");
    expect(combined).not.toContain("rename" + "chat");
    expect(combined).not.toContain("nomdu" + "chat");
    expect(combined).not.toContain("RENAME" + "_CHAT" + "_CODEACT_SUFFIX");
    expect(combined).not.toContain("COMPACT" + "_RENAME" + "_CHAT" + "_CODEACT_PATTERN");
    expect(combined).not.toContain("brain_compact_" + "rename" + "chat");
    expect(combined).not.toContain("parseCodeAct" + "TemplateFields");
    expect(combined).not.toContain("sessionTitle" + "SubjectFromUserText");
    expect(combined).not.toContain("firstTurn" + "RuntimeSessionTitle");
    expect(combined).not.toContain("applyFirstTurn" + "RuntimeSessionTitle");
    expect(combined).not.toContain("runtime" + "_first_turn_title");
    expect(combined).not.toContain("SESSION_TITLE" + "_CODEACT_REQUIRED v1");
    expect(combined).not.toContain("renderRenameSession" + "CodeActResult");
  });
});
