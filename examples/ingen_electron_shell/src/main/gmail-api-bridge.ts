import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import type { GmailCodeActRequest, GmailIntent } from "./gmail-codeact.js";
import { GMAIL_COMMAND, GMAIL_RESULT_SCHEMA } from "./gmail-codeact.js";

export const GMAIL_API_RESULT_SCHEMA = "forge.gmail.api_result.v1";
export const GMAIL_API_BASE_URL = "https://gmail.googleapis.com/gmail/v1";

export type GmailApiStatus = "ok" | "auth_required" | "blocked" | "error";
export type GmailApiOperation = "search" | "inspect" | "summarize" | "draft" | "open";
export type GmailApiScope =
  | "https://www.googleapis.com/auth/gmail.metadata"
  | "https://www.googleapis.com/auth/gmail.readonly"
  | "https://www.googleapis.com/auth/gmail.compose"
  | "https://www.googleapis.com/auth/gmail.send";

export interface GmailApiMessageSummary {
  id: string;
  threadId: string;
  from?: string;
  to?: string;
  subject?: string;
  date?: string;
  snippet?: string;
  labelIds: string[];
  proofHash: string;
}

export interface GmailApiDraftSummary {
  id: string;
  messageId?: string;
  threadId?: string;
  proofHash: string;
}

export interface GmailApiBridgeResult {
  schema: typeof GMAIL_API_RESULT_SCHEMA;
  command: typeof GMAIL_COMMAND;
  status: GmailApiStatus;
  operation: GmailApiOperation;
  execution: "gmail_rest_api";
  requestHash: string;
  requiredScopes: GmailApiScope[];
  endpoints: string[];
  messages: GmailApiMessageSummary[];
  draft?: GmailApiDraftSummary;
  resultSizeEstimate?: number;
  warnings: string[];
  error?: string;
  proofHash: string;
}

interface GmailApiCredentials {
  accessToken: string;
}

interface GmailApiListResponse {
  messages?: Array<{ id?: string; threadId?: string }>;
  resultSizeEstimate?: number;
}

interface GmailApiMessage {
  id?: string;
  threadId?: string;
  labelIds?: string[];
  snippet?: string;
  payload?: {
    headers?: Array<{ name?: string; value?: string }>;
  };
}

interface GmailApiDraftResponse {
  id?: string;
  message?: {
    id?: string;
    threadId?: string;
  };
}

const METADATA_HEADERS = ["From", "To", "Subject", "Date"];
const GMAIL_API_TIMEOUT_MS = 30_000;

export async function runGmailApiBridge(request: GmailCodeActRequest): Promise<GmailApiBridgeResult> {
  const operation = gmailApiOperation(request.intent);
  const requiredScopes = gmailApiRequiredScopes(request);
  const endpoints = gmailApiEndpoints(request, operation);
  const credentials = gmailApiCredentialsFromEnv();
  if (!credentials) {
    return gmailApiResult(request, {
      status: "auth_required",
      operation,
      requiredScopes,
      endpoints,
      warnings: [
        "Gmail API OAuth is not connected. Configure a user OAuth access token outside the LLM with INGEN_GMAIL_ACCESS_TOKEN for the current bridge.",
        "The LLM receives this proof result only; access tokens are never echoed."
      ]
    });
  }
  if (operation === "open") {
    return gmailApiResult(request, {
      status: "blocked",
      operation,
      requiredScopes,
      endpoints,
      warnings: ["Opening Gmail is a WebExplorer action. Use mode=\"split_webexplorer\" for visual Gmail navigation."]
    });
  }
  if (request.sendPolicy !== "user_approval_required") {
    return gmailApiResult(request, {
      status: "blocked",
      operation,
      requiredScopes,
      endpoints,
      warnings: ["Gmail write actions require explicit user approval. Silent sending is blocked."]
    });
  }
  try {
    if (operation === "draft") {
      const draft = await createGmailDraft(request, credentials);
      return gmailApiResult(request, {
        status: "ok",
        operation,
        requiredScopes,
        endpoints,
        draft,
        warnings: ["Draft created only; no email was sent."]
      });
    }
    if (operation === "inspect" || request.messageId) {
      const message = await getGmailMessage(request.messageId || request.query, credentials);
      return gmailApiResult(request, {
        status: "ok",
        operation,
        requiredScopes,
        endpoints,
        messages: [message],
        warnings: []
      });
    }
    const list = await listGmailMessages(request, credentials);
    const messageIds = list.messages.map((message) => message.id).filter((id): id is string => Boolean(id));
    const details = await Promise.all(messageIds.map((messageId) => getGmailMessage(messageId, credentials)));
    return gmailApiResult(request, {
      status: "ok",
      operation,
      requiredScopes,
      endpoints,
      messages: details,
      resultSizeEstimate: list.resultSizeEstimate,
      warnings: []
    });
  } catch (error) {
    return gmailApiResult(request, {
      status: "error",
      operation,
      requiredScopes,
      endpoints,
      warnings: [],
      error: friendlyError(error)
    });
  }
}

export function renderGmailApiBridgeResult(result: GmailApiBridgeResult): string {
  return [
    "GMAIL_RESULT",
    `schema=${GMAIL_RESULT_SCHEMA}`,
    `api_schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `operation=${result.operation}`,
    `execution=${JSON.stringify(result.execution)}`,
    `request_hash=sha256:${result.requestHash}`,
    `required_scopes=${JSON.stringify(result.requiredScopes)}`,
    `endpoints=${JSON.stringify(result.endpoints)}`,
    `result_size_estimate=${result.resultSizeEstimate ?? result.messages.length}`,
    `messages=${JSON.stringify(result.messages)}`,
    result.draft ? `draft=${JSON.stringify(result.draft)}` : "",
    result.warnings.length > 0 ? `warnings=${JSON.stringify(result.warnings)}` : "",
    result.error ? `error=${JSON.stringify(result.error)}` : "",
    `proof_hash=sha256:${result.proofHash}`
  ].filter(Boolean).join("\n");
}

export function gmailApiRequiredScopes(request: GmailCodeActRequest): GmailApiScope[] {
  if (request.intent === "draft" || request.intent === "reply") {
    return ["https://www.googleapis.com/auth/gmail.compose"];
  }
  if (request.query || request.intent === "summarize" || request.intent === "inspect") {
    return ["https://www.googleapis.com/auth/gmail.readonly"];
  }
  return ["https://www.googleapis.com/auth/gmail.metadata"];
}

function gmailApiOperation(intent: GmailIntent): GmailApiOperation {
  if (intent === "reply") return "draft";
  if (intent === "summarize") return "summarize";
  if (intent === "inspect") return "inspect";
  if (intent === "draft") return "draft";
  if (intent === "search") return "search";
  return "open";
}

function gmailApiEndpoints(request: GmailCodeActRequest, operation: GmailApiOperation): string[] {
  if (operation === "draft") return [`${GMAIL_API_BASE_URL}/users/me/drafts`];
  if (operation === "inspect" || request.messageId) return [`${GMAIL_API_BASE_URL}/users/me/messages/{id}`];
  if (operation === "open") return [];
  return [`${GMAIL_API_BASE_URL}/users/me/messages`, `${GMAIL_API_BASE_URL}/users/me/messages/{id}`];
}

function gmailApiCredentialsFromEnv(): GmailApiCredentials | undefined {
  const accessToken = (process.env.INGEN_GMAIL_ACCESS_TOKEN ?? process.env.GMAIL_ACCESS_TOKEN ?? "").trim();
  return accessToken ? { accessToken } : undefined;
}

async function listGmailMessages(request: GmailCodeActRequest, credentials: GmailApiCredentials): Promise<Required<GmailApiListResponse>> {
  const url = new URL(`${GMAIL_API_BASE_URL}/users/me/messages`);
  url.searchParams.set("maxResults", String(Math.max(1, Math.min(500, request.maxResults))));
  const query = gmailSearchQuery(request);
  if (query) url.searchParams.set("q", query);
  const response = await gmailFetchJson(url.toString(), credentials);
  const list = readRecord(response) as GmailApiListResponse;
  return {
    messages: (Array.isArray(list.messages) ? list.messages : [])
      .map((message) => ({
        id: String(message.id ?? ""),
        threadId: String(message.threadId ?? "")
      }))
      .filter((message) => message.id),
    resultSizeEstimate: Number.isFinite(list.resultSizeEstimate) ? Number(list.resultSizeEstimate) : 0
  };
}

async function getGmailMessage(id: string, credentials: GmailApiCredentials): Promise<GmailApiMessageSummary> {
  const messageId = id.trim();
  if (!messageId) {
    throw new Error("Gmail message_id is required for inspect.");
  }
  const url = new URL(`${GMAIL_API_BASE_URL}/users/me/messages/${encodeURIComponent(messageId)}`);
  url.searchParams.set("format", "METADATA");
  for (const header of METADATA_HEADERS) {
    url.searchParams.append("metadataHeaders", header);
  }
  const parsed = readRecord(await gmailFetchJson(url.toString(), credentials)) as GmailApiMessage;
  return summarizeGmailMessage(parsed);
}

async function createGmailDraft(request: GmailCodeActRequest, credentials: GmailApiCredentials): Promise<GmailApiDraftSummary> {
  const raw = rawEmailMessage(request);
  const parsed = readRecord(await gmailFetchJson(`${GMAIL_API_BASE_URL}/users/me/drafts`, credentials, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message: { raw } })
  })) as GmailApiDraftResponse;
  const draft: GmailApiDraftSummary = {
    id: String(parsed.id ?? ""),
    messageId: parsed.message?.id,
    threadId: parsed.message?.threadId,
    proofHash: ""
  };
  draft.proofHash = stableHash({ ...draft, proofHash: "" });
  return draft;
}

async function gmailFetchJson(url: string, credentials: GmailApiCredentials, init: RequestInit = {}): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), GMAIL_API_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      ...init,
      signal: controller.signal,
      headers: {
        accept: "application/json",
        ...(init.headers ?? {}),
        authorization: `Bearer ${credentials.accessToken}`
      }
    });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(`Gmail API HTTP ${response.status}: ${compactText(text, 700)}`);
    }
    return text ? JSON.parse(text) : {};
  } finally {
    clearTimeout(timeout);
  }
}

function gmailSearchQuery(request: GmailCodeActRequest): string {
  return [request.query, ...request.keywords].map((item) => item.trim()).filter(Boolean).join(" ").trim();
}

function summarizeGmailMessage(message: GmailApiMessage): GmailApiMessageSummary {
  const headers = new Map((message.payload?.headers ?? []).map((header) => [String(header.name ?? "").toLowerCase(), String(header.value ?? "")]));
  const summary: GmailApiMessageSummary = {
    id: String(message.id ?? ""),
    threadId: String(message.threadId ?? ""),
    from: headers.get("from"),
    to: headers.get("to"),
    subject: headers.get("subject"),
    date: headers.get("date"),
    snippet: compactText(String(message.snippet ?? ""), 280),
    labelIds: Array.isArray(message.labelIds) ? message.labelIds.map(String) : [],
    proofHash: ""
  };
  summary.proofHash = stableHash({ ...summary, proofHash: "" });
  return summary;
}

function rawEmailMessage(request: GmailCodeActRequest): string {
  const lines = [
    request.recipient ? `To: ${request.recipient}` : "",
    request.subject ? `Subject: ${request.subject}` : "",
    "Content-Type: text/plain; charset=\"UTF-8\"",
    "MIME-Version: 1.0",
    "",
    request.body
  ].filter((line, index) => line || index >= 4);
  return Buffer.from(lines.join("\r\n"), "utf8").toString("base64url");
}

function gmailApiResult(
  request: GmailCodeActRequest,
  partial: Omit<Partial<GmailApiBridgeResult>, "schema" | "command" | "execution" | "requestHash" | "proofHash">
): GmailApiBridgeResult {
  const result: GmailApiBridgeResult = {
    schema: GMAIL_API_RESULT_SCHEMA,
    command: GMAIL_COMMAND,
    status: partial.status ?? "error",
    operation: partial.operation ?? gmailApiOperation(request.intent),
    execution: "gmail_rest_api",
    requestHash: request.proofHash,
    requiredScopes: partial.requiredScopes ?? gmailApiRequiredScopes(request),
    endpoints: partial.endpoints ?? [],
    messages: partial.messages ?? [],
    draft: partial.draft,
    resultSizeEstimate: partial.resultSizeEstimate,
    warnings: partial.warnings ?? [],
    error: partial.error,
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function compactText(value: string, maxChars: number): string {
  const clean = value.replace(/\s+/gu, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function friendlyError(error: unknown): string {
  if (error instanceof Error) {
    return error.name === "AbortError" ? `Gmail API timed out after ${GMAIL_API_TIMEOUT_MS}ms.` : error.message;
  }
  return String(error);
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
