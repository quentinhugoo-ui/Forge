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
export const AGENT_COMPUTER_INSPECT_COMMAND = "/agent_computer_inspect_";
export const AGENT_APPSHOT_COMMAND = "/agent_appshot_";
export const AGENT_FOCUS_WINDOW_COMMAND = "/agent_focus_window_";
export const AGENT_CLIPBOARD_READ_COMMAND = "/agent_clipboard_read_";
export const AGENT_CLIPBOARD_WRITE_COMMAND = "/agent_clipboard_write_";
export const AGENT_BROWSER_INSPECT_COMMAND = "/agent_browser_inspect_";
export const AGENT_BROWSER_DOWNLOAD_COMMAND = "/agent_browser_download_";
export const AGENT_BROWSER_OPEN_COMMAND = "/agent_browser_open_";
export const AGENT_DOCUMENT_INSPECT_COMMAND = "/agent_document_inspect_";
export const AGENT_DOCUMENT_WRITE_COMMAND = "/agent_document_write_";
export const AGENT_DOCUMENT_CONVERT_COMMAND = "/agent_document_convert_";
export const AGENT_DEV_STATUS_COMMAND = "/agent_dev_status_";
export const AGENT_DEV_DIFF_COMMAND = "/agent_dev_diff_";
export const AGENT_DEV_CHECK_COMMAND = "/agent_dev_check_";
export const AGENT_AUTOMATION_RECORD_COMMAND = "/agent_automation_record_";

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
  AGENT_SHELL_COMMAND,
  AGENT_COMPUTER_INSPECT_COMMAND,
  AGENT_APPSHOT_COMMAND,
  AGENT_FOCUS_WINDOW_COMMAND,
  AGENT_CLIPBOARD_READ_COMMAND,
  AGENT_CLIPBOARD_WRITE_COMMAND,
  AGENT_BROWSER_INSPECT_COMMAND,
  AGENT_BROWSER_DOWNLOAD_COMMAND,
  AGENT_BROWSER_OPEN_COMMAND,
  AGENT_DOCUMENT_INSPECT_COMMAND,
  AGENT_DOCUMENT_WRITE_COMMAND,
  AGENT_DOCUMENT_CONVERT_COMMAND,
  AGENT_DEV_STATUS_COMMAND,
  AGENT_DEV_DIFF_COMMAND,
  AGENT_DEV_CHECK_COMMAND,
  AGENT_AUTOMATION_RECORD_COMMAND
] as const;

export type AgentActionEventCommand = (typeof AGENT_ACTION_EVENT_COMMANDS)[number];

export interface AgentActionTranscriptEvent {
  command: AgentActionEventCommand;
  text: string;
  path?: string;
  toPath?: string;
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
  [AGENT_SHELL_COMMAND, "confirmed shell command executed"],
  [AGENT_COMPUTER_INSPECT_COMMAND, "computer GUI state inspected"],
  [AGENT_APPSHOT_COMMAND, "confirmed appshot captured"],
  [AGENT_FOCUS_WINDOW_COMMAND, "confirmed window focus requested"],
  [AGENT_CLIPBOARD_READ_COMMAND, "confirmed clipboard text inspected"],
  [AGENT_CLIPBOARD_WRITE_COMMAND, "confirmed clipboard text replaced"],
  [AGENT_BROWSER_INSPECT_COMMAND, "web page state inspected"],
  [AGENT_BROWSER_DOWNLOAD_COMMAND, "confirmed web download verified"],
  [AGENT_BROWSER_OPEN_COMMAND, "confirmed browser navigation requested"],
  [AGENT_DOCUMENT_INSPECT_COMMAND, "document/media artifact inspected"],
  [AGENT_DOCUMENT_WRITE_COMMAND, "document/data artifact written and verified"],
  [AGENT_DOCUMENT_CONVERT_COMMAND, "document text converted and verified"],
  [AGENT_DEV_STATUS_COMMAND, "repository status inspected"],
  [AGENT_DEV_DIFF_COMMAND, "repository diff inspected"],
  [AGENT_DEV_CHECK_COMMAND, "confirmed developer check completed"],
  [AGENT_AUTOMATION_RECORD_COMMAND, "confirmed automation goal recorded"]
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
  ["shell.full", AGENT_SHELL_COMMAND],
  ["computer.inspect", AGENT_COMPUTER_INSPECT_COMMAND],
  ["computer.appshot", AGENT_APPSHOT_COMMAND],
  ["computer.focus_window", AGENT_FOCUS_WINDOW_COMMAND],
  ["computer.clipboard_read", AGENT_CLIPBOARD_READ_COMMAND],
  ["computer.clipboard_write", AGENT_CLIPBOARD_WRITE_COMMAND],
  ["browser.inspect_url", AGENT_BROWSER_INSPECT_COMMAND],
  ["browser.download", AGENT_BROWSER_DOWNLOAD_COMMAND],
  ["browser.open_url", AGENT_BROWSER_OPEN_COMMAND],
  ["document.inspect", AGENT_DOCUMENT_INSPECT_COMMAND],
  ["document.write_text", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document.write_json", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document.write_csv", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document.convert_text", AGENT_DOCUMENT_CONVERT_COMMAND],
  ["dev.repo_status", AGENT_DEV_STATUS_COMMAND],
  ["dev.git_diff", AGENT_DEV_DIFF_COMMAND],
  ["dev.run_check", AGENT_DEV_CHECK_COMMAND],
  ["automation.record", AGENT_AUTOMATION_RECORD_COMMAND]
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
  ["run_command", AGENT_SHELL_COMMAND],
  ["computer_inspect", AGENT_COMPUTER_INSPECT_COMMAND],
  ["computer_appshot", AGENT_APPSHOT_COMMAND],
  ["computer_focus_window", AGENT_FOCUS_WINDOW_COMMAND],
  ["computer_clipboard_read", AGENT_CLIPBOARD_READ_COMMAND],
  ["computer_clipboard_write", AGENT_CLIPBOARD_WRITE_COMMAND],
  ["browser_inspect_url", AGENT_BROWSER_INSPECT_COMMAND],
  ["browser_download", AGENT_BROWSER_DOWNLOAD_COMMAND],
  ["browser_open_url", AGENT_BROWSER_OPEN_COMMAND],
  ["document_inspect", AGENT_DOCUMENT_INSPECT_COMMAND],
  ["document_write_text", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document_write_json", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document_write_csv", AGENT_DOCUMENT_WRITE_COMMAND],
  ["document_convert_text", AGENT_DOCUMENT_CONVERT_COMMAND],
  ["dev_repo_status", AGENT_DEV_STATUS_COMMAND],
  ["dev_git_diff", AGENT_DEV_DIFF_COMMAND],
  ["dev_run_check", AGENT_DEV_CHECK_COMMAND],
  ["automation_record", AGENT_AUTOMATION_RECORD_COMMAND]
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

function assignmentValue(line: string, key: string): string | undefined {
  const pattern = new RegExp(`(?:^|[\\s,{])${key}\\s*[:=]\\s*("((?:\\\\.|[^"])*)"|'([^']*)'|([^\\s,;}]+))`);
  const match = pattern.exec(line);
  const value = match?.[2] ?? match?.[3] ?? match?.[4];
  if (!value) {
    return undefined;
  }
  if (match?.[2] !== undefined) {
    try {
      return JSON.parse(`"${value}"`) as string;
    } catch {
      return value;
    }
  }
  return value;
}

function eventWithPathMetadata(command: AgentActionEventCommand, line: string): AgentActionTranscriptEvent {
  const event: AgentActionTranscriptEvent = {
    command,
    text: agentActionEventText(command)
  };
  const path = assignmentValue(line, "path");
  const toPath = assignmentValue(line, "toPath");
  if (path) {
    event.path = path;
  }
  if (toPath) {
    event.toPath = toPath;
  }
  return event;
}

export function agentActionEventFromLine(line: string): AgentActionTranscriptEvent | undefined {
  const trimmed = line.trim();
  const firstToken = trimmed.split(/\s+/, 1)[0] ?? "";
  const directCommand = agentActionEventCommandFromToken(firstToken);
  if (directCommand) {
    return eventWithPathMetadata(directCommand, trimmed);
  }
  for (const value of assignmentValues(trimmed)) {
    const assignedCommand = agentActionEventCommandFromToken(value);
    if (assignedCommand) {
      return eventWithPathMetadata(assignedCommand, trimmed);
    }
  }
  for (const [toolId, command] of AGENT_ACTION_EVENT_HINTS) {
    if (trimmed.includes(toolId)) {
      return eventWithPathMetadata(command, trimmed);
    }
  }
  return undefined;
}
