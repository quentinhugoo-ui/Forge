import { describe, expect, it } from "vitest";
import {
  AGENT_BROWSER_DOWNLOAD_COMMAND,
  AGENT_BROWSER_INSPECT_COMMAND,
  AGENT_BROWSER_OPEN_COMMAND,
  AGENT_COPY_PATH_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
  AGENT_DOCUMENT_CONVERT_COMMAND,
  AGENT_DOCUMENT_INSPECT_COMMAND,
  AGENT_DOCUMENT_WRITE_COMMAND,
  AGENT_APPSHOT_COMMAND,
  AGENT_COMPUTER_INSPECT_COMMAND,
  AGENT_FOCUS_WINDOW_COMMAND,
  AGENT_LIST_COMMAND,
  AGENT_READONLY_SHELL_COMMAND,
  AGENT_SEARCH_COMMAND,
  AGENT_SHELL_COMMAND,
  agentActionEventFromLine
} from "../src/renderer/agent-action-events";

describe("agent action transcript events", () => {
  it("maps direct event commands to transcript event copy", () => {
    expect(agentActionEventFromLine(`${AGENT_SEARCH_COMMAND} query="needle"`)).toEqual({
      command: AGENT_SEARCH_COMMAND,
      text: "computer search returned bounded matches"
    });
  });

  it("maps structured action kinds and tool ids to event commands", () => {
    expect(agentActionEventFromLine('AGENT_ACTION action="list" path="."')?.command).toBe(AGENT_LIST_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="fs.copy" path="a" toPath="b"')?.command).toBe(AGENT_COPY_PATH_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="shell.readonly" command="rg"')?.command).toBe(AGENT_READONLY_SHELL_COMMAND);
    expect(agentActionEventFromLine("executeAgentAction fs.delete_tree confirmed=true")?.command).toBe(AGENT_DELETE_TREE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="run_command" command="powershell.exe"')?.command).toBe(AGENT_SHELL_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_inspect"')?.command).toBe(AGENT_COMPUTER_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="computer.appshot"')?.command).toBe(AGENT_APPSHOT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="computer.focus_window"')?.command).toBe(AGENT_FOCUS_WINDOW_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_inspect_url"')?.command).toBe(AGENT_BROWSER_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="browser.download"')?.command).toBe(AGENT_BROWSER_DOWNLOAD_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="browser.open_url"')?.command).toBe(AGENT_BROWSER_OPEN_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="document_inspect" path="report.md"')?.command).toBe(AGENT_DOCUMENT_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="document.write_json" path="data.json"')?.command).toBe(AGENT_DOCUMENT_WRITE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="document.convert_text"')?.command).toBe(AGENT_DOCUMENT_CONVERT_COMMAND);
  });

  it("keeps path metadata for file modification event cards", () => {
    expect(agentActionEventFromLine(`${AGENT_COPY_PATH_COMMAND} path="src/App.tsx" toPath="src/App.copy.tsx"`)).toMatchObject({
      command: AGENT_COPY_PATH_COMMAND,
      path: "src/App.tsx",
      toPath: "src/App.copy.tsx"
    });
  });

  it("ignores unrelated prose", () => {
    expect(agentActionEventFromLine("Je vais chercher dans les fichiers avant de modifier.")).toBeUndefined();
  });
});
