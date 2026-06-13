import { describe, expect, it } from "vitest";
import {
  AGENT_COPY_PATH_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
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
  });

  it("ignores unrelated prose", () => {
    expect(agentActionEventFromLine("Je vais chercher dans les fichiers avant de modifier.")).toBeUndefined();
  });
});
