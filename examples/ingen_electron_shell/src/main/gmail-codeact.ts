import { createHash } from "node:crypto";
import {
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GMAIL_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const GMAIL_COMMAND = BRAIN_GMAIL_COMMAND;
export const GMAIL_COM_COMMAND = BRAIN_GMAIL_COM_COMMAND;
export const GMAIL_RESULT_SCHEMA = BRAIN_GMAIL_RESULT_SCHEMA;

const MAX_FIELD_CHARS = 420;
export const GMAIL_SIGN_IN_URL =
  "https://accounts.google.com/v3/signin/identifier?continue=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F&dsh=S774374132%3A1781221713252777&emr=1&followup=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F&osid=1&passive=1209600&service=mail&flowName=GlifWebSignIn&flowEntry=ServiceLogin&ifkv=AcDsRvwvE6OEtCrlcF2xeomhVr5CW3eg8--rcWsKGX7WFKgGHEXXRQbs2nX53njSKjZ6B3o5MvId5w";

export interface GmailCodeActRequest {
  schema: "forge.webexplorer.gmail.request.v1";
  command: typeof GMAIL_COMMAND | typeof GMAIL_COM_COMMAND;
  intent: "open" | "search" | "inspect" | "summarize" | "draft" | "reply";
  query: string;
  keywords: string[];
  recipient: string;
  subject: string;
  body: string;
  url: string;
  proofHash: string;
}

export function parseGmailCodeAct(input: string): GmailCodeActRequest | undefined {
  const trimmed = input.trim();
  const command = readGmailCommand(trimmed);
  if (!command) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(command.length).trim());
  const intent = readIntent(fields.get("intent"));
  const query = clampText(fields.get("query") ?? fields.get("q") ?? "", MAX_FIELD_CHARS);
  const keywords = uniqueKeywords(splitKeywords(fields.get("keywords") ?? fields.get("keyword")));
  const recipient = clampText(fields.get("recipient") ?? fields.get("to") ?? "", MAX_FIELD_CHARS);
  const subject = clampText(fields.get("subject") ?? "", MAX_FIELD_CHARS);
  const body = clampText(fields.get("body") ?? "", MAX_FIELD_CHARS);
  return buildGmailCodeActRequest({ command, intent, query, keywords, recipient, subject, body });
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
    `intent=${request.intent}`,
    `query=${JSON.stringify(request.query)}`,
    `keywords=${JSON.stringify(request.keywords)}`,
    `recipient=${JSON.stringify(request.recipient)}`,
    `subject=${JSON.stringify(request.subject)}`,
    `url=${JSON.stringify(request.url)}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

export function gmailWebExplorerNavigationUrl(request: GmailCodeActRequest): string {
  if (request.command === GMAIL_COM_COMMAND || request.intent === "open") {
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
  if (params.command === GMAIL_COM_COMMAND) {
    return GMAIL_SIGN_IN_URL;
  }
  if (params.intent === "open" && !params.query && params.keywords.length === 0) {
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

function readGmailCommand(value: string): typeof GMAIL_COMMAND | typeof GMAIL_COM_COMMAND | undefined {
  const trimmed = value.trim();
  if (trimmed === GMAIL_COM_COMMAND || trimmed.startsWith(`${GMAIL_COM_COMMAND} `)) {
    return GMAIL_COM_COMMAND;
  }
  if (trimmed === GMAIL_COMMAND || trimmed.startsWith(`${GMAIL_COMMAND} `)) {
    return GMAIL_COMMAND;
  }
  return undefined;
}

function readIntent(value: unknown): GmailCodeActRequest["intent"] {
  if (value === "open" || value === "search" || value === "inspect" || value === "summarize" || value === "draft" || value === "reply") {
    return value;
  }
  return "open";
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    const value = (match[2] ?? match[3] ?? match[4] ?? "").trim();
    fields.set(key, value);
  }
  return fields;
}

function splitKeywords(value: unknown): string[] {
  if (typeof value !== "string" || !value.trim()) {
    return [];
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
  }
  return result.slice(0, 12);
}

function clampText(text: string, maxChars: number): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
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
