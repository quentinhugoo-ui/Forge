import { createHash } from "node:crypto";
import {
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_GOOGLEWEB_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const GOOGLEWEB_COMMAND = BRAIN_GOOGLEWEB_COMMAND;
export const GOOGLEWEB_RESULT_SCHEMA = BRAIN_GOOGLEWEB_RESULT_SCHEMA;
export const GOOGLEWEB_HOME_URL = "https://www.google.com/";
const GOOGLEWEB_HOME_QUERY = "Google";

const MAX_QUERY_CHARS = 240;
const MAX_KEYWORDS = 10;
const MAX_KEYWORD_CHARS = 48;

export interface GoogleWebCodeActRequest {
  schema: "forge.webexplorer.googleweb.request.v1";
  command: typeof GOOGLEWEB_COMMAND;
  query: string;
  keywords: string[];
  url: string;
  source: "explicit_codeact";
  proofHash: string;
}

export function parseGoogleWebCodeAct(input: string): GoogleWebCodeActRequest | undefined {
  const trimmed = input.trim();
  if (!trimmed.startsWith(GOOGLEWEB_COMMAND)) {
    return undefined;
  }
  const body = trimmed.slice(GOOGLEWEB_COMMAND.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? body : "";
  const explicitQuery = (fields.get("query") ?? fields.get("q") ?? fields.get("topic") ?? freeform).trim();
  const query = clampText((explicitQuery || GOOGLEWEB_HOME_QUERY).trim(), MAX_QUERY_CHARS);
  const keywords = uniqueKeywords([
    ...splitKeywords(fields.get("keywords")),
    ...splitKeywords(fields.get("keyword")),
    ...splitKeywords(fields.get("must")),
    ...splitKeywords(fields.get("focus"))
  ]);
  return buildGoogleWebCodeActRequest(query, keywords, "explicit_codeact", explicitQuery ? undefined : GOOGLEWEB_HOME_URL);
}

export function extractGoogleWebCodeAct(input: string): GoogleWebCodeActRequest | undefined {
  const explicit = parseGoogleWebCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => item.startsWith(GOOGLEWEB_COMMAND));
  return line ? parseGoogleWebCodeAct(line) : undefined;
}

export function buildGoogleSearchUrl(query: string, keywords: string[]): string {
  const url = new URL("https://www.google.com/search");
  const searchQuery = uniqueKeywords([query, ...keywords]).join(" ").trim();
  url.searchParams.set("q", searchQuery || query);
  url.searchParams.set("hl", "fr");
  return url.toString();
}

export function renderGoogleWebCodeActResult(request: GoogleWebCodeActRequest): string {
  const actionText = request.url === GOOGLEWEB_HOME_URL
    ? "J'ouvre Google dans Web Explorer."
    : `Je lance la recherche Google pour "${request.query}" dans Web Explorer.`;
  return [
    actionText,
    "",
    "GOOGLEWEB_RESULT",
    `schema=${GOOGLEWEB_RESULT_SCHEMA}`,
    `command=${GOOGLEWEB_COMMAND}`,
    `query=${JSON.stringify(request.query)}`,
    `keywords=${JSON.stringify(request.keywords)}`,
    `url=${JSON.stringify(request.url)}`,
    `source=${request.source}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

function buildGoogleWebCodeActRequest(
  query: string,
  keywords: string[],
  source: GoogleWebCodeActRequest["source"],
  urlOverride?: string
): GoogleWebCodeActRequest | undefined {
  const cleanQuery = clampText(query, MAX_QUERY_CHARS);
  if (!cleanQuery) {
    return undefined;
  }
  const cleanKeywords = uniqueKeywords(keywords);
  const request: GoogleWebCodeActRequest = {
    schema: "forge.webexplorer.googleweb.request.v1",
    command: GOOGLEWEB_COMMAND,
    query: cleanQuery,
    keywords: cleanKeywords,
    url: urlOverride ?? buildGoogleSearchUrl(cleanQuery, cleanKeywords),
    source,
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
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
    const clean = clampText(value, MAX_KEYWORD_CHARS);
    const key = normalizeText(clean);
    if (!clean || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(clean);
    if (result.length >= MAX_KEYWORDS) {
      break;
    }
  }
  return result;
}

function normalizeText(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase()
    .replace(/\s+/g, " ")
    .trim();
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
