import { createHash } from "node:crypto";
import {
  BRAIN_LOCAL_ACTIONS_COMMAND,
  BRAIN_LOCAL_ACTIONS_RESULT_SCHEMA,
  type AgentActionRequest,
  type AgentActionResult
} from "../shared/ipc-contract.js";

export const LOCAL_ACTIONS_COMMAND = BRAIN_LOCAL_ACTIONS_COMMAND;
export const LOCAL_ACTIONS_RESULT_SCHEMA = BRAIN_LOCAL_ACTIONS_RESULT_SCHEMA;

const MAX_QUERY_CHARS = 400;
const MAX_RESULTS_MIN = 1;
const MAX_RESULTS_MAX = 120;

type LocalActionsScope =
  | "all"
  | "workspace"
  | "computer"
  | "coding"
  | "browser"
  | "documents"
  | "windows"
  | "cloud"
  | "automation";

export interface LocalActionsCodeActRequest {
  schema: "forge.agent.local_actions.request.v1";
  command: typeof BRAIN_LOCAL_ACTIONS_COMMAND;
  action: "capabilities";
  scope: LocalActionsScope;
  query: string;
  maxResults: number;
  output: "agent_action_manifest";
  source: "explicit_codeact";
  proofHash: string;
}

export function parseLocalActionsCodeAct(input: string): LocalActionsCodeActRequest | undefined {
  const trimmed = input.trim();
  const command = readLocalActionsCommand(trimmed);
  if (!command) {
    return undefined;
  }
  const body = trimmed.slice(command.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? body : "";
  const request: LocalActionsCodeActRequest = {
    schema: "forge.agent.local_actions.request.v1",
    command,
    action: "capabilities",
    scope: readScope(fields.get("scope")),
    query: clampText(fields.get("query") ?? fields.get("q") ?? fields.get("topic") ?? freeform, MAX_QUERY_CHARS),
    maxResults: clampNumber(fields.get("maxResults") ?? fields.get("max_results"), MAX_RESULTS_MIN, MAX_RESULTS_MAX, 40),
    output: "agent_action_manifest",
    source: "explicit_codeact",
    proofHash: ""
  };
  request.proofHash = stableLocalActionsHash({ ...request, proofHash: "" });
  return request;
}

export function extractLocalActionsCodeAct(input: string): LocalActionsCodeActRequest | undefined {
  if (input.includes("LOCAL_ACTIONS_RESULT")) {
    return undefined;
  }
  const explicit = parseLocalActionsCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const lines = input.split(/\r?\n/);
  const startIndex = lines.findIndex((line) => Boolean(readLocalActionsCommand(line)));
  if (startIndex < 0) {
    return undefined;
  }
  const block: string[] = [];
  for (let index = startIndex; index < lines.length; index += 1) {
    const line = lines[index]?.trim() ?? "";
    if (index > startIndex && (!line || line.startsWith("/") || /^[A-Z_]+_RESULT\b/.test(line))) {
      break;
    }
    block.push(line);
  }
  return parseLocalActionsCodeAct(block.join("\n"));
}

export function localActionsCodeActToAgentActionRequest(request: LocalActionsCodeActRequest): AgentActionRequest {
  return {
    action: "capabilities",
    scope: request.scope,
    query: request.query,
    maxResults: request.maxResults
  };
}

export function renderLocalActionsCodeActResult(
  request: LocalActionsCodeActRequest,
  result: AgentActionResult,
  compactResult: string
): string {
  return [
    "LOCAL_ACTIONS_RESULT v1",
    `schema=${LOCAL_ACTIONS_RESULT_SCHEMA}`,
    `command=${request.command}`,
    `action=${request.action}`,
    `status=${result.accepted ? "ok" : "error"}`,
    `scope=${request.scope}`,
    `query=${JSON.stringify(request.query)}`,
    `max_results=${request.maxResults}`,
    `request_hash=sha256:${request.proofHash}`,
    `agent_action_route=${result.routeId ?? "agent.capabilities"}`,
    `agent_action_result=${compactResult}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

function readLocalActionsCommand(value: string): typeof BRAIN_LOCAL_ACTIONS_COMMAND | undefined {
  const trimmed = value.trim().toLowerCase();
  const command = BRAIN_LOCAL_ACTIONS_COMMAND;
  if (trimmed === command) {
    return command;
  }
  if (trimmed.startsWith(command) && /\s/u.test(trimmed.charAt(command.length))) {
    return command;
  }
  return undefined;
}

function readScope(value: unknown): LocalActionsScope {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : "";
  const scopes: LocalActionsScope[] = [
    "all",
    "workspace",
    "computer",
    "coding",
    "browser",
    "documents",
    "windows",
    "cloud",
    "automation"
  ];
  return scopes.find((scope) => scope === normalized) ?? "all";
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"((?:\\.|[^"])*)"|'((?:\\.|[^'])*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    fields.set(key, decodeTemplateValue(match[2] ?? match[3] ?? match[4] ?? "").trim());
  }
  return fields;
}

function decodeTemplateValue(value: string): string {
  return value
    .replace(/\\"/gu, "\"")
    .replace(/\\'/gu, "'")
    .replace(/\\n/gu, "\n")
    .replace(/\\t/gu, "\t")
    .replace(/\\\\/gu, "\\");
}

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
  const numeric = typeof value === "number" ? value : typeof value === "string" ? Number.parseInt(value, 10) : NaN;
  if (!Number.isFinite(numeric)) {
    return fallback;
  }
  return Math.max(min, Math.min(max, Math.trunc(numeric)));
}

function clampText(text: string | undefined, maxChars: number): string {
  const clean = (text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return clean.slice(0, maxChars).trim();
}

function stableLocalActionsHash(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(stableJson(value))).digest("hex");
}

function stableJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stableJson);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stableJson(item)])
    );
  }
  return value;
}
