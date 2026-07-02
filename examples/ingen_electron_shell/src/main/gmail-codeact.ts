import { createHash } from "node:crypto";
import {
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const GMAIL_COMMAND = BRAIN_GMAIL_COMMAND;
export const GMAIL_RESULT_SCHEMA = BRAIN_GMAIL_RESULT_SCHEMA;

const MAX_FIELD_CHARS = 420;
const MAX_KEYWORDS = 12;
const GMAIL_TEMPLATE_RESULT_SCHEMA = "forge.gmail.template_result.v1";
export const GMAIL_SIGN_IN_URL =
  "https://accounts.google.com/v3/signin/identifier?continue=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F&dsh=S774374132%3A1781221713252777&emr=1&followup=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F&osid=1&passive=1209600&service=mail&flowName=GlifWebSignIn&flowEntry=ServiceLogin&ifkv=AcDsRvwvE6OEtCrlcF2xeomhVr5CW3eg8--rcWsKGX7WFKgGHEXXRQbs2nX53njSKjZ6B3o5MvId5w";

export type GmailIntent = "open" | "search" | "inspect" | "summarize" | "draft" | "reply";
export type GmailMode = "gmail_api" | "split_webexplorer";
export type GmailSendPolicy = "user_approval_required";

export interface GmailCodeActRequest {
  schema: "forge.webexplorer.gmail.request.v1";
  command: typeof GMAIL_COMMAND;
  templateProofHash: string;
  intent: GmailIntent;
  query: string;
  keywords: string[];
  recipient: string;
  subject: string;
  body: string;
  messageId: string;
  maxResults: number;
  mode: GmailMode;
  sendPolicy: GmailSendPolicy;
  url: string;
  source: "explicit_codeact";
  proofHash: string;
}

export interface GmailTemplateResult {
  schema: typeof GMAIL_TEMPLATE_RESULT_SCHEMA;
  command: typeof GMAIL_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    intent: GmailIntent[];
    mode: GmailMode[];
    sendPolicy: GmailSendPolicy[];
  };
  proofHash: string;
}

export type GmailCodeAct =
  | { kind: "template"; result: GmailTemplateResult }
  | { kind: "request"; request: GmailCodeActRequest };

const GMAIL_INTENTS: GmailIntent[] = ["open", "search", "inspect", "summarize", "draft", "reply"];
const GMAIL_MODES: GmailMode[] = ["gmail_api", "split_webexplorer"];
const GMAIL_SEND_POLICIES: GmailSendPolicy[] = ["user_approval_required"];

export function gmailTemplateResult(reason: GmailTemplateResult["reason"] = "empty_command"): GmailTemplateResult {
  const templateProofHash = gmailTemplateProofHash();
  const template = [
    `${GMAIL_COMMAND}`,
    `template_proof_hash="sha256:${templateProofHash}"`,
    'intent="open|search|inspect|summarize|draft|reply"',
    'query=""',
    'keywords=[]',
    'recipient=""',
    'subject=""',
    'body=""',
    'message_id=""',
    'max_results=10',
    'mode="gmail_api"',
    'send_policy="user_approval_required"'
  ].join("\n");
  const result: GmailTemplateResult = {
    schema: GMAIL_TEMPLATE_RESULT_SCHEMA,
    command: GMAIL_COMMAND,
    status: "template",
    reason,
    template,
    allowedValues: {
      intent: GMAIL_INTENTS,
      mode: GMAIL_MODES,
      sendPolicy: GMAIL_SEND_POLICIES
    },
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function renderGmailTemplateResult(result: GmailTemplateResult): string {
  return [
    "GMAIL_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${gmailTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function readGmailCodeAct(input: string): GmailCodeAct | undefined {
  const trimmed = gmailCodeActText(input).trim();
  if (!trimmed) {
    return undefined;
  }
  const command = readGmailCommand(trimmed);
  if (!command) {
    return undefined;
  }
  const body = trimmed.slice(command.length).trim();
  if (!body) {
    return { kind: "template", result: gmailTemplateResult("empty_command") };
  }
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: gmailTemplateResult("template_required") };
  }
  const request = parseGmailCodeAct(trimmed);
  if (!request) {
    return { kind: "template", result: gmailTemplateResult("template_required") };
  }
  return { kind: "request", request };
}

export function parseGmailCodeAct(input: string): GmailCodeActRequest | undefined {
  const trimmed = gmailCodeActText(input).trim();
  const command = readGmailCommand(trimmed);
  if (!command) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(command.length).trim());
  const intent = readChoice(fields.get("intent"), GMAIL_INTENTS, "open");
  const query = clampText(fields.get("query") ?? fields.get("q") ?? "", MAX_FIELD_CHARS);
  const keywords = uniqueKeywords(splitKeywords(fields.get("keywords") ?? fields.get("keyword")));
  const recipient = clampText(fields.get("recipient") ?? fields.get("to") ?? "", MAX_FIELD_CHARS);
  const subject = clampText(fields.get("subject") ?? "", MAX_FIELD_CHARS);
  const body = clampText(fields.get("body") ?? "", MAX_FIELD_CHARS);
  const messageId = clampText(fields.get("message_id") ?? fields.get("messageId") ?? fields.get("id") ?? "", 160);
  const defaultMode: GmailMode = intent === "open" ? "split_webexplorer" : "gmail_api";
  return buildGmailCodeActRequest({
    command,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    intent,
    query,
    keywords,
    recipient,
    subject,
    body,
    messageId,
    maxResults: clampNumber(fields.get("max_results") ?? fields.get("limit"), 1, 50, 10),
    mode: readChoice(fields.get("mode") ?? fields.get("open_mode"), GMAIL_MODES, defaultMode),
    sendPolicy: readChoice(fields.get("send_policy"), GMAIL_SEND_POLICIES, "user_approval_required"),
    source: "explicit_codeact"
  });
}

export function extractGmailCodeAct(input: string): GmailCodeActRequest | undefined {
  const explicit = parseGmailCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => Boolean(readGmailCommand(item)));
  return line ? parseGmailCodeAct(line) : undefined;
}

export function renderGmailCodeActResult(request: GmailCodeActRequest): string {
  return [
    "GMAIL_RESULT",
    `schema=${GMAIL_RESULT_SCHEMA}`,
    `command=${request.command}`,
    `status=ok`,
    `intent=${request.intent}`,
    `execution="split_webexplorer_navigation"`,
    `query=${JSON.stringify(request.query)}`,
    `keywords=${JSON.stringify(request.keywords)}`,
    `recipient=${JSON.stringify(request.recipient)}`,
    `subject=${JSON.stringify(request.subject)}`,
    `message_id=${JSON.stringify(request.messageId)}`,
    `max_results=${request.maxResults}`,
    `mode=${request.mode}`,
    `send_policy=${request.sendPolicy}`,
    `url=${JSON.stringify(request.url)}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

export function gmailWebExplorerNavigationUrl(request: GmailCodeActRequest): string {
  if (request.intent === "open") {
    return GMAIL_SIGN_IN_URL;
  }
  return request.url;
}

function buildGmailCodeActRequest(params: Omit<GmailCodeActRequest, "schema" | "url" | "proofHash">): GmailCodeActRequest {
  const request: GmailCodeActRequest = {
    schema: "forge.webexplorer.gmail.request.v1",
    ...params,
    url: gmailUrl(params),
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

function gmailUrl(params: Pick<GmailCodeActRequest, "command" | "intent" | "query" | "keywords" | "recipient" | "subject" | "body">): string {
  if (params.intent === "open") {
    return GMAIL_SIGN_IN_URL;
  }
  if (params.intent === "draft" || params.intent === "reply") {
    const url = new URL("https://mail.google.com/mail/");
    const compose = new URLSearchParams();
    if (params.recipient) compose.set("to", params.recipient);
    if (params.subject) compose.set("su", params.subject);
    if (params.body) compose.set("body", params.body);
    const composeQuery = compose.toString();
    url.hash = composeQuery ? `compose?${composeQuery}` : "inbox";
    return url.toString();
  }
  const search = uniqueKeywords([params.query, ...params.keywords]).join(" ").trim();
  const url = new URL("https://mail.google.com/mail/");
  url.hash = search ? `search/${encodeURIComponent(search)}` : "inbox";
  return url.toString();
}

function readGmailCommand(value: string): typeof GMAIL_COMMAND | undefined {
  const trimmed = value.trim();
  if (trimmed === GMAIL_COMMAND || trimmed.startsWith(`${GMAIL_COMMAND} `)) {
    return GMAIL_COMMAND;
  }
  return undefined;
}

function gmailCodeActText(input: string): string {
  const commandIndex = input.indexOf(GMAIL_COMMAND);
  if (commandIndex < 0) {
    return "";
  }
  const lines = input.slice(commandIndex).split(/\r?\n/);
  const block: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]?.trim() ?? "";
    if (index > 0 && (!line || line.startsWith("/") || /^[A-Z_]+_RESULT\b/.test(line))) {
      break;
    }
    block.push(line);
  }
  return block.join("\n");
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

function splitKeywords(value: unknown): string[] {
  if (typeof value !== "string" || !value.trim()) {
    return [];
  }
  const text = value.trim();
  if (text.startsWith("[") && text.endsWith("]")) {
    try {
      const parsed = JSON.parse(text) as unknown;
      if (Array.isArray(parsed)) {
        return parsed.filter((item): item is string => typeof item === "string");
      }
    } catch {
      // Fall back to separator parsing.
    }
  }
  return value.split(/[,|;\n]+/).map((item) => item.trim()).filter(Boolean);
}

function uniqueKeywords(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    const clean = clampText(value, 80);
    const key = clean.toLocaleLowerCase();
    if (!clean || seen.has(key)) continue;
    seen.add(key);
    result.push(clean);
    if (result.length >= MAX_KEYWORDS) break;
  }
  return result;
}

function readChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return choices.find((choice) => choice === normalized) ?? fallback;
}

function clampText(text: string | undefined, maxChars: number): string {
  const clean = (text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
  const numeric = typeof value === "number" ? value : typeof value === "string" ? Number.parseInt(value, 10) : NaN;
  if (!Number.isFinite(numeric)) {
    return fallback;
  }
  return Math.max(min, Math.min(max, Math.trunc(numeric)));
}

function gmailTemplateProofHash(): string {
  return stableHash({
    command: GMAIL_COMMAND,
    schema: GMAIL_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "intent",
      "query",
      "keywords",
      "recipient",
      "subject",
      "body",
      "message_id",
      "max_results",
      "mode",
      "send_policy"
    ],
    allowedValues: {
      intent: GMAIL_INTENTS,
      mode: GMAIL_MODES,
      sendPolicy: GMAIL_SEND_POLICIES
    }
  });
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === gmailTemplateProofHash();
}

function normalizeProofHash(value: unknown): string {
  return String(value ?? "").trim().replace(/^sha256:/i, "");
}

function indentBlock(value: string, prefix: string): string {
  return value.split(/\r?\n/).map((line) => `${prefix}${line}`).join("\n");
}

function stableHash(value: unknown): string {
  return createHash("sha256").update(stableJson(value)).digest("hex");
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`).join(",")}}`;
}
