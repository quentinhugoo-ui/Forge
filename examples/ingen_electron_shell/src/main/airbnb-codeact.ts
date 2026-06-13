import { createHash } from "node:crypto";
import {
  BRAIN_AIRBNB_COMMAND,
  BRAIN_AIRBNB_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const AIRBNB_COMMAND = BRAIN_AIRBNB_COMMAND;
export const AIRBNB_RESULT_SCHEMA = BRAIN_AIRBNB_RESULT_SCHEMA;
export const AIRBNB_HOME_URL = "https://www.airbnb.com/";

const MAX_FIELD_CHARS = 420;

export interface AirbnbCodeActRequest {
  schema: "forge.webexplorer.airbnb.request.v1";
  command: typeof AIRBNB_COMMAND;
  intent: "open" | "search" | "inspect";
  say: string;
  query: string;
  keywords: string[];
  url: string;
  proofHash: string;
}

export function parseAirbnbCodeAct(input: string): AirbnbCodeActRequest | undefined {
  const trimmed = input.trim();
  if (!readAirbnbCommand(trimmed)) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(AIRBNB_COMMAND.length).trim());
  const intent = readIntent(fields.get("intent"));
  const say = clampText(fields.get("say") ?? fields.get("message") ?? "", MAX_FIELD_CHARS);
  const query = clampText(fields.get("query") ?? fields.get("q") ?? "", MAX_FIELD_CHARS);
  const keywords = uniqueKeywords(splitKeywords(fields.get("keywords") ?? fields.get("keyword")));
  return buildAirbnbCodeActRequest({ command: AIRBNB_COMMAND, intent, say, query, keywords });
}

export function extractAirbnbCodeAct(input: string): AirbnbCodeActRequest | undefined {
  const explicit = parseAirbnbCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => Boolean(readAirbnbCommand(item)));
  return line ? parseAirbnbCodeAct(line) : undefined;
}

export function renderAirbnbCodeActResult(request: AirbnbCodeActRequest): string {
  return [
    "AIRBNB_RESULT",
    `schema=${AIRBNB_RESULT_SCHEMA}`,
    `command=${request.command}`,
    `intent=${request.intent}`,
    `say=${JSON.stringify(request.say)}`,
    `query=${JSON.stringify(request.query)}`,
    `keywords=${JSON.stringify(request.keywords)}`,
    `url=${JSON.stringify(request.url)}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

function buildAirbnbCodeActRequest(params: Omit<AirbnbCodeActRequest, "schema" | "url" | "proofHash">): AirbnbCodeActRequest {
  const request: AirbnbCodeActRequest = {
    schema: "forge.webexplorer.airbnb.request.v1",
    ...params,
    url: airbnbUrl(params),
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

function airbnbUrl(params: Pick<AirbnbCodeActRequest, "query" | "keywords">): string {
  const search = uniqueKeywords([params.query, ...params.keywords]).join(" ").trim();
  const url = new URL(AIRBNB_HOME_URL);
  if (search) {
    url.searchParams.set("query", search);
  }
  return url.toString();
}

function readAirbnbCommand(value: string): typeof AIRBNB_COMMAND | undefined {
  const trimmed = value.trim();
  return trimmed === AIRBNB_COMMAND || trimmed.startsWith(`${AIRBNB_COMMAND} `) ? AIRBNB_COMMAND : undefined;
}

function readIntent(value: unknown): AirbnbCodeActRequest["intent"] {
  if (value === "open" || value === "search" || value === "inspect") {
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
