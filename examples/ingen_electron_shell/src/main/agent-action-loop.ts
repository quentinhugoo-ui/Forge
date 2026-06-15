import {
  type AgentActionKind,
  isAgentActionRequest,
  type AgentActionRequest
} from "../shared/ipc-contract.js";

export const AGENT_ACTION_JSON_PREFIX = "AGENT_ACTION_JSON";

export interface ExtractedAgentAction {
  request: AgentActionRequest;
  fragment: string;
  startIndex: number;
  endIndex: number;
}

export interface ExtractedAgentActionControlFragment {
  fragment: string;
  startIndex: number;
  endIndex: number;
}

const AGENT_ACTION_ALIASES: Partial<Record<string, AgentActionKind>> = {
  list_directory: "list",
  list_dir: "list",
  mkdir: "create_directory",
  create_folder: "create_directory",
  rename: "rename_path",
  move: "move_path",
  copy: "copy_path",
  delete_directory: "delete_empty_directory",
  delete_recursive: "delete_tree",
  shell: "run_command",
  readonly_shell: "run_readonly_command"
};

function normalizedAgentActionRequest(value: unknown): AgentActionRequest | undefined {
  if (isAgentActionRequest(value)) {
    return value;
  }
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  const action = typeof record.action === "string" ? AGENT_ACTION_ALIASES[record.action] ?? record.action : undefined;
  const path = typeof record.path === "string" ? record.path : "";
  const rawScope = record.scope;
  const scope =
    (action === "list" || action === "search") && record.scope === "workspace" && /^[A-Za-z]:[\\/]/.test(path)
      ? "computer"
      : (action === "run_command" || action === "run_readonly_command") && rawScope === "shell"
        ? "computer"
        : rawScope;
  const normalized = { ...record, action, scope };
  return isAgentActionRequest(normalized) ? normalized : undefined;
}

export function jsonObjectEndIndex(text: string, objectStart: number): number | undefined {
  if (text[objectStart] !== "{") {
    return undefined;
  }
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = objectStart; index < text.length; index += 1) {
    const char = text[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (char === "\\") {
        escaped = true;
      } else if (char === "\"") {
        inString = false;
      }
      continue;
    }
    if (char === "\"") {
      inString = true;
      continue;
    }
    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return index + 1;
      }
    }
  }
  return undefined;
}

export function extractAgentActionJsonControlFragment(text: string): ExtractedAgentActionControlFragment | undefined {
  const markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX);
  if (markerIndex < 0) {
    return undefined;
  }
  const afterMarker = markerIndex + AGENT_ACTION_JSON_PREFIX.length;
  const jsonStartOffset = text.slice(afterMarker).search(/\S/);
  if (jsonStartOffset < 0) {
    return undefined;
  }
  const jsonStart = afterMarker + jsonStartOffset;
  const jsonEnd = jsonObjectEndIndex(text, jsonStart);
  if (!jsonEnd) {
    return undefined;
  }
  return {
    fragment: text.slice(markerIndex, jsonEnd),
    startIndex: markerIndex,
    endIndex: jsonEnd
  };
}

export function extractAgentActionJsonRequest(text: string): ExtractedAgentAction | undefined {
  let markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX);
  while (markerIndex >= 0) {
    const afterMarker = markerIndex + AGENT_ACTION_JSON_PREFIX.length;
    const jsonStartOffset = text.slice(afterMarker).search(/\S/);
    if (jsonStartOffset < 0) {
      return undefined;
    }
    const jsonStart = afterMarker + jsonStartOffset;
    const jsonEnd = jsonObjectEndIndex(text, jsonStart);
    if (!jsonEnd) {
      return undefined;
    }
    const rawJson = text.slice(jsonStart, jsonEnd);
    try {
      const parsed = JSON.parse(rawJson) as unknown;
      const request = normalizedAgentActionRequest(parsed);
      if (request) {
        return {
          request,
          fragment: text.slice(markerIndex, jsonEnd),
          startIndex: markerIndex,
          endIndex: jsonEnd
        };
      }
    } catch {
      // Keep scanning in case this mention was prose and a later marker is valid.
    }
    markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX, afterMarker);
  }
  return undefined;
}

export function removeAgentActionJsonFragment(text: string, extracted: ExtractedAgentActionControlFragment): string {
  const before = text.slice(0, extracted.startIndex).trimEnd();
  const after = text.slice(extracted.endIndex).trimStart();
  return [before, after]
    .filter((part) => part.length > 0)
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

export function removeAgentActionJsonFragments(text: string): string {
  let visibleText = text;
  while (true) {
    const extracted = extractAgentActionJsonRequest(visibleText) ?? extractAgentActionJsonControlFragment(visibleText);
    if (!extracted) {
      return visibleText;
    }
    visibleText = removeAgentActionJsonFragment(visibleText, extracted);
  }
}

export function agentActionLiveVisibleText(text: string): string {
  const extracted = extractAgentActionJsonRequest(text) ?? extractAgentActionJsonControlFragment(text);
  if (extracted) {
    return removeAgentActionJsonFragment(text, extracted);
  }
  const markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX);
  return markerIndex >= 0 ? text.slice(0, markerIndex).trimEnd() : text;
}
