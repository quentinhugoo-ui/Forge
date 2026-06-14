import {
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
      if (isAgentActionRequest(parsed)) {
        return {
          request: parsed,
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

export function removeAgentActionJsonFragment(text: string, extracted: ExtractedAgentAction): string {
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
    const extracted = extractAgentActionJsonRequest(visibleText);
    if (!extracted) {
      return visibleText;
    }
    visibleText = removeAgentActionJsonFragment(visibleText, extracted);
  }
}

export function agentActionLiveVisibleText(text: string): string {
  const extracted = extractAgentActionJsonRequest(text);
  if (extracted) {
    return removeAgentActionJsonFragment(text, extracted);
  }
  const markerIndex = text.indexOf(AGENT_ACTION_JSON_PREFIX);
  return markerIndex >= 0 ? text.slice(0, markerIndex).trimEnd() : text;
}
