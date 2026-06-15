import { createHash } from "node:crypto";
import {
  BRAIN_CODEDOCS_COMMAND,
  BRAIN_CODEDOCS_RESULT_SCHEMA,
  BRAIN_GITHUB_MCP_COMMAND,
  BRAIN_GITHUB_MCP_RESULT_SCHEMA,
  BRAIN_SECURITYSCAN_COMMAND,
  BRAIN_SECURITYSCAN_RESULT_SCHEMA,
  BRAIN_WEBACT_COMMAND,
  BRAIN_WEBACT_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export type PriorityMcpCommand =
  | typeof BRAIN_CODEDOCS_COMMAND
  | typeof BRAIN_GITHUB_MCP_COMMAND
  | typeof BRAIN_WEBACT_COMMAND
  | typeof BRAIN_SECURITYSCAN_COMMAND;

export type PriorityMcpKind = "codedocs" | "github" | "webact" | "securityscan";
export type PriorityMcpProvider = "context7" | "github" | "playwright" | "semgrep";
export type PriorityMcpStatus = "ok" | "partial" | "error";

export interface PriorityMcpCodeActRequest {
  schema: "forge.priority_mcp.request.v1";
  command: PriorityMcpCommand;
  kind: PriorityMcpKind;
  provider: PriorityMcpProvider;
  operation: string;
  library: string;
  libraryId: string;
  version: string;
  query: string;
  repo: string;
  number: string;
  ref: string;
  url: string;
  instruction: string;
  selector: string;
  text: string;
  target: string;
  config: string;
  rule: string;
  language: string;
  severityThreshold: "info" | "warning" | "error" | "critical";
  mode: string;
  output: string;
  maxTokens: number;
  timeoutMs: number;
  rawFields: Record<string, string>;
  source: "explicit_codeact";
  proofHash: string;
}

export interface PriorityMcpToolResult {
  tool: string;
  arguments: Record<string, unknown>;
  content: unknown;
}

export interface PriorityMcpBridgeResult {
  schema:
    | typeof BRAIN_CODEDOCS_RESULT_SCHEMA
    | typeof BRAIN_GITHUB_MCP_RESULT_SCHEMA
    | typeof BRAIN_WEBACT_RESULT_SCHEMA
    | typeof BRAIN_SECURITYSCAN_RESULT_SCHEMA;
  command: PriorityMcpCommand;
  status: PriorityMcpStatus;
  requestHash: string;
  startedAt: string;
  finishedAt: string;
  durationMs: number;
  provider: PriorityMcpProvider;
  operation: string;
  tool?: string;
  availableTools: string[];
  content: unknown;
  toolResults: PriorityMcpToolResult[];
  warnings: string[];
  error?: string;
  proofHash: string;
}

const MAX_TEXT_CHARS = 2_000;
const MAX_FIELD_CHARS = 1_200;
const DEFAULT_TIMEOUT_MS = 45_000;

const COMMAND_CONFIG: Record<PriorityMcpCommand, {
  kind: PriorityMcpKind;
  provider: PriorityMcpProvider;
  schema: PriorityMcpBridgeResult["schema"];
  resultMarker: string;
}> = {
  [BRAIN_CODEDOCS_COMMAND]: {
    kind: "codedocs",
    provider: "context7",
    schema: BRAIN_CODEDOCS_RESULT_SCHEMA,
    resultMarker: "CODEDOCS_RESULT"
  },
  [BRAIN_GITHUB_MCP_COMMAND]: {
    kind: "github",
    provider: "github",
    schema: BRAIN_GITHUB_MCP_RESULT_SCHEMA,
    resultMarker: "GITHUB_MCP_RESULT"
  },
  [BRAIN_WEBACT_COMMAND]: {
    kind: "webact",
    provider: "playwright",
    schema: BRAIN_WEBACT_RESULT_SCHEMA,
    resultMarker: "WEBACT_RESULT"
  },
  [BRAIN_SECURITYSCAN_COMMAND]: {
    kind: "securityscan",
    provider: "semgrep",
    schema: BRAIN_SECURITYSCAN_RESULT_SCHEMA,
    resultMarker: "SECURITYSCAN_RESULT"
  }
};

const PRIORITY_MCP_COMMANDS = Object.keys(COMMAND_CONFIG) as PriorityMcpCommand[];

export function parsePriorityMcpCodeAct(input: string): PriorityMcpCodeActRequest | undefined {
  const trimmed = input.trim();
  const command = readPriorityMcpCommand(trimmed);
  if (!command) {
    return undefined;
  }
  const config = COMMAND_CONFIG[command];
  const body = trimmed.slice(command.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? clampText(body, MAX_TEXT_CHARS) : "";
  const request: PriorityMcpCodeActRequest = {
    schema: "forge.priority_mcp.request.v1",
    command,
    kind: config.kind,
    provider: config.provider,
    operation: readOperation(config.kind, fields),
    library: clampText(fields.get("library") ?? fields.get("package") ?? "", MAX_FIELD_CHARS),
    libraryId: clampText(fields.get("library_id") ?? fields.get("libraryId") ?? "", MAX_FIELD_CHARS),
    version: clampText(fields.get("version") ?? "auto", MAX_FIELD_CHARS),
    query: clampText(fields.get("query") ?? fields.get("q") ?? fields.get("topic") ?? freeform, MAX_TEXT_CHARS),
    repo: clampText(fields.get("repo") ?? fields.get("repository") ?? "", MAX_FIELD_CHARS),
    number: clampText(fields.get("number") ?? fields.get("issue") ?? fields.get("pr") ?? "", 80),
    ref: clampText(fields.get("ref") ?? fields.get("branch") ?? fields.get("sha") ?? "", MAX_FIELD_CHARS),
    url: clampText(fields.get("url") ?? "", MAX_FIELD_CHARS),
    instruction: clampText(fields.get("instruction") ?? fields.get("goal") ?? fields.get("assertion") ?? "", MAX_TEXT_CHARS),
    selector: clampText(fields.get("selector") ?? fields.get("element") ?? fields.get("ref") ?? "", MAX_FIELD_CHARS),
    text: clampText(fields.get("text") ?? fields.get("value") ?? "", MAX_TEXT_CHARS),
    target: clampText(fields.get("target") ?? fields.get("path") ?? fields.get("file") ?? freeform, MAX_FIELD_CHARS),
    config: clampText(fields.get("config") ?? "auto", MAX_FIELD_CHARS),
    rule: clampText(fields.get("rule") ?? "", MAX_TEXT_CHARS),
    language: clampText(fields.get("language") ?? fields.get("lang") ?? "auto", 80),
    severityThreshold: readChoice(fields.get("severity_threshold") ?? fields.get("severity"), ["info", "warning", "error", "critical"], "warning"),
    mode: readChoice(fields.get("mode"), ["read_only", "write_requires_confirmation"], "read_only"),
    output: clampText(fields.get("output") ?? defaultOutput(config.kind), MAX_FIELD_CHARS),
    maxTokens: clampNumber(fields.get("max_tokens") ?? fields.get("maxTokens"), 500, 20_000, 6_000),
    timeoutMs: clampNumber(fields.get("timeout_ms") ?? fields.get("timeoutMs"), 3_000, 180_000, DEFAULT_TIMEOUT_MS),
    rawFields: Object.fromEntries(fields.entries()),
    source: "explicit_codeact",
    proofHash: ""
  };
  if (!hasRequiredFields(request)) {
    return undefined;
  }
  request.proofHash = stablePriorityMcpHash({ ...request, proofHash: "" });
  return request;
}

export function extractPriorityMcpCodeAct(input: string): PriorityMcpCodeActRequest | undefined {
  const explicit = parsePriorityMcpCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const lines = input.split(/\r?\n/);
  const startIndex = lines.findIndex((line) => Boolean(readPriorityMcpCommand(line)));
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
  return parsePriorityMcpCodeAct(block.join("\n"));
}

export function renderPriorityMcpCodeActResult(result: PriorityMcpBridgeResult): string {
  const marker = priorityMcpResultMarkerForCommand(result.command);
  const toolSummary = result.toolResults.map((toolResult) => ({
    tool: toolResult.tool,
    arguments: toolResult.arguments
  }));
  return [
    marker,
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `request_hash=sha256:${result.requestHash}`,
    `duration_ms=${result.durationMs}`,
    `provider=${result.provider}`,
    `operation=${result.operation}`,
    result.tool ? `tool=${result.tool}` : "",
    `available_tools=${JSON.stringify(result.availableTools.slice(0, 80))}`,
    `tool_calls=${JSON.stringify(toolSummary.slice(0, 12))}`,
    `content=${JSON.stringify(compactForManifest(result.content))}`,
    `warnings=${JSON.stringify(result.warnings)}`,
    result.error ? `error=${JSON.stringify(result.error)}` : "",
    `proof_hash=sha256:${result.proofHash}`
  ].filter(Boolean).join("\n");
}

export function priorityMcpResultMarkerForCommand(command: PriorityMcpCommand): string {
  return COMMAND_CONFIG[command].resultMarker;
}

export function priorityMcpSchemaForCommand(command: PriorityMcpCommand): PriorityMcpBridgeResult["schema"] {
  return COMMAND_CONFIG[command].schema;
}

export function stablePriorityMcpHash(value: unknown): string {
  return createHash("sha256").update(stableStringify(value)).digest("hex");
}

function readPriorityMcpCommand(value: string): PriorityMcpCommand | undefined {
  const trimmed = value.trim().toLowerCase();
  for (const command of PRIORITY_MCP_COMMANDS) {
    if (trimmed === command) {
      return command;
    }
    if (trimmed.startsWith(command) && /\s/u.test(trimmed.charAt(command.length))) {
      return command;
    }
  }
  return undefined;
}

function readOperation(kind: PriorityMcpKind, fields: Map<string, string>): string {
  switch (kind) {
    case "codedocs":
      return "query_docs";
    case "github":
      return readChoice(fields.get("operation") ?? fields.get("op"), [
        "repo_context",
        "code_search",
        "issue_pr_triage",
        "ci_status",
        "release_notes",
        "create_issue",
        "comment_pr"
      ], "repo_context");
    case "webact":
      return readChoice(fields.get("action") ?? fields.get("operation"), [
        "navigate",
        "snapshot",
        "click",
        "type",
        "screenshot",
        "evaluate",
        "test_flow"
      ], "snapshot");
    case "securityscan":
      return readChoice(fields.get("mode") ?? fields.get("operation"), [
        "security_check",
        "semgrep_scan",
        "custom_rule",
        "ast"
      ], "security_check");
  }
}

function defaultOutput(kind: PriorityMcpKind): string {
  switch (kind) {
    case "codedocs":
      return "docs_manifest";
    case "github":
      return "github_manifest";
    case "webact":
      return "browser_action_manifest";
    case "securityscan":
      return "security_manifest";
  }
}

function hasRequiredFields(request: PriorityMcpCodeActRequest): boolean {
  switch (request.kind) {
    case "codedocs":
      return Boolean(request.library || request.libraryId) && Boolean(request.query);
    case "github":
      return Boolean(request.repo || request.query || request.number);
    case "webact":
      return request.operation !== "navigate" || Boolean(request.url);
    case "securityscan":
      return Boolean(request.target);
  }
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"((?:\\.|[^"])*)"|'((?:\\.|[^'])*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    const value = decodeTemplateValue(match[2] ?? match[3] ?? match[4] ?? "").trim();
    fields.set(key, value);
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

function readChoice<const T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return allowed.includes(normalized as T) ? normalized as T : fallback;
}

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "string" || !value.trim()) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.max(min, Math.min(max, parsed));
}

function clampText(value: unknown, maxChars: number): string {
  if (typeof value !== "string") {
    return "";
  }
  const clean = value.trim();
  return clean.length > maxChars ? clean.slice(0, maxChars) : clean;
}

function compactForManifest(value: unknown): unknown {
  if (typeof value === "string") {
    return clampText(value, 12_000);
  }
  const serialized = JSON.stringify(value);
  if (!serialized || serialized.length <= 24_000) {
    return value;
  }
  return {
    compacted: true,
    preview: serialized.slice(0, 24_000)
  };
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(",")}]`;
  }
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${stableStringify(object[key])}`)
    .join(",")}}`;
}
