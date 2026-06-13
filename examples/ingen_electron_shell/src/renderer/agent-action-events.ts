export const AGENT_LIST_COMMAND = "/agent_list_";
export const AGENT_SEARCH_COMMAND = "/agent_search_";
export const AGENT_CREATE_DIRECTORY_COMMAND = "/agent_create_directory_";
export const AGENT_RENAME_PATH_COMMAND = "/agent_rename_path_";
export const AGENT_MOVE_PATH_COMMAND = "/agent_move_path_";
export const AGENT_COPY_PATH_COMMAND = "/agent_copy_path_";
export const AGENT_DELETE_EMPTY_DIRECTORY_COMMAND = "/agent_delete_empty_directory_";
export const AGENT_DELETE_TREE_COMMAND = "/agent_delete_tree_";
export const AGENT_READONLY_SHELL_COMMAND = "/agent_readonly_shell_";
export const AGENT_SHELL_COMMAND = "/agent_shell_";

export const AGENT_ACTION_EVENT_COMMANDS = [
  AGENT_LIST_COMMAND,
  AGENT_SEARCH_COMMAND,
  AGENT_CREATE_DIRECTORY_COMMAND,
  AGENT_RENAME_PATH_COMMAND,
  AGENT_MOVE_PATH_COMMAND,
  AGENT_COPY_PATH_COMMAND,
  AGENT_DELETE_EMPTY_DIRECTORY_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
  AGENT_READONLY_SHELL_COMMAND,
  AGENT_SHELL_COMMAND
] as const;

export type AgentActionEventCommand = (typeof AGENT_ACTION_EVENT_COMMANDS)[number];

export interface AgentActionTranscriptEvent {
  command: AgentActionEventCommand;
  text: string;
}

const AGENT_ACTION_EVENT_TEXT = new Map<AgentActionEventCommand, string>([
  [AGENT_LIST_COMMAND, "file system listing returned"],
  [AGENT_SEARCH_COMMAND, "computer search returned bounded matches"],
  [AGENT_CREATE_DIRECTORY_COMMAND, "directory created"],
  [AGENT_RENAME_PATH_COMMAND, "path renamed"],
  [AGENT_MOVE_PATH_COMMAND, "path moved"],
  [AGENT_COPY_PATH_COMMAND, "path copied"],
  [AGENT_DELETE_EMPTY_DIRECTORY_COMMAND, "empty directory deleted"],
  [AGENT_DELETE_TREE_COMMAND, "directory tree deleted"],
  [AGENT_READONLY_SHELL_COMMAND, "read-only shell command inspected the workspace"],
  [AGENT_SHELL_COMMAND, "confirmed shell command executed"]
]);

export const AGENT_ACTION_EVENT_HINTS: readonly [string, AgentActionEventCommand][] = [
  ["fs.list", AGENT_LIST_COMMAND],
  ["fs.search", AGENT_SEARCH_COMMAND],
  ["fs.create_directory", AGENT_CREATE_DIRECTORY_COMMAND],
  ["fs.rename", AGENT_RENAME_PATH_COMMAND],
  ["fs.move", AGENT_MOVE_PATH_COMMAND],
  ["fs.copy", AGENT_COPY_PATH_COMMAND],
  ["fs.delete_empty_directory", AGENT_DELETE_EMPTY_DIRECTORY_COMMAND],
  ["fs.delete_tree", AGENT_DELETE_TREE_COMMAND],
  ["shell.readonly", AGENT_READONLY_SHELL_COMMAND],
  ["shell.full", AGENT_SHELL_COMMAND]
] as const;

const AGENT_ACTION_EVENT_BY_ACTION = new Map<string, AgentActionEventCommand>([
  ["list", AGENT_LIST_COMMAND],
  ["search", AGENT_SEARCH_COMMAND],
  ["create_directory", AGENT_CREATE_DIRECTORY_COMMAND],
  ["rename_path", AGENT_RENAME_PATH_COMMAND],
  ["move_path", AGENT_MOVE_PATH_COMMAND],
  ["copy_path", AGENT_COPY_PATH_COMMAND],
  ["delete_empty_directory", AGENT_DELETE_EMPTY_DIRECTORY_COMMAND],
  ["delete_tree", AGENT_DELETE_TREE_COMMAND],
  ["run_readonly_command", AGENT_READONLY_SHELL_COMMAND],
  ["run_command", AGENT_SHELL_COMMAND]
]);

const AGENT_ACTION_EVENT_BY_TOOL = new Map<string, AgentActionEventCommand>(AGENT_ACTION_EVENT_HINTS);
const AGENT_ACTION_EVENT_COMMAND_SET = new Set<string>(AGENT_ACTION_EVENT_COMMANDS);

function cleanToken(value: string): string {
  return value.trim().replace(/^["'`{[(]+|["'`,;})\]]+$/g, "");
}

export function isAgentActionEventCommand(value: string): value is AgentActionEventCommand {
  return AGENT_ACTION_EVENT_COMMAND_SET.has(value);
}

export function agentActionEventText(command: AgentActionEventCommand): string {
  return AGENT_ACTION_EVENT_TEXT.get(command) ?? "agent action executed";
}

export function agentActionEventCommandFromToken(value: string): AgentActionEventCommand | undefined {
  const token = cleanToken(value);
  if (isAgentActionEventCommand(token)) {
    return token;
  }
  return AGENT_ACTION_EVENT_BY_ACTION.get(token) ?? AGENT_ACTION_EVENT_BY_TOOL.get(token);
}

function assignmentValues(line: string): string[] {
  const values: string[] = [];
  const pattern = /(?:^|[\s,{])(?:agent_action|action|tool|capability)\s*[:=]\s*("([^"]+)"|'([^']+)'|([^\s,;}]+))/g;
  for (const match of line.matchAll(pattern)) {
    values.push(match[2] ?? match[3] ?? match[4] ?? "");
  }
  return values;
}

export function agentActionEventFromLine(line: string): AgentActionTranscriptEvent | undefined {
  const trimmed = line.trim();
  const firstToken = trimmed.split(/\s+/, 1)[0] ?? "";
  const directCommand = agentActionEventCommandFromToken(firstToken);
  if (directCommand) {
    return { command: directCommand, text: agentActionEventText(directCommand) };
  }
  for (const value of assignmentValues(trimmed)) {
    const assignedCommand = agentActionEventCommandFromToken(value);
    if (assignedCommand) {
      return { command: assignedCommand, text: agentActionEventText(assignedCommand) };
    }
  }
  for (const [toolId, command] of AGENT_ACTION_EVENT_HINTS) {
    if (trimmed.includes(toolId)) {
      return { command, text: agentActionEventText(command) };
    }
  }
  return undefined;
}
