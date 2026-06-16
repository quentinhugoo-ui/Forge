import { createHash } from "node:crypto";
import {
  BRAIN_WEBSEARCH_COMMAND,
  BRAIN_WEBSEARCH_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const WEBSEARCH_COMMAND = BRAIN_WEBSEARCH_COMMAND;
export const WEBSEARCH_RESULT_SCHEMA = BRAIN_WEBSEARCH_RESULT_SCHEMA;

const MAX_QUERY_CHARS = 800;
const MAX_GOAL_CHARS = 300;
const MAX_DOMAINS = 16;
const MAX_DOMAIN_CHARS = 120;
const WEBSEARCH_TEMPLATE_RESULT_SCHEMA = "forge.websearch.template_result.v1";

export type WebSearchProviderId = "openai" | "claude";
export type WebSearchProviderRoute =
  | "auto"
  | "openai"
  | "claude"
  | "openai_then_claude"
  | "claude_then_openai"
  | "both_parallel";
export type WebSearchStatus = "ok" | "partial" | "error";
export type WebSearchMediaIntent =
  | "none"
  | "image_enrichment"
  | "video_enrichment"
  | "audio_enrichment"
  | "image_video_audio_enrichment";
export type WebSearchContentTypes = "text" | "text_image" | "image";
export type WebSearchMediaKind = "image" | "video" | "audio" | "page";

export interface WebSearchCodeActRequest {
  schema: "forge.websearch.request.v1";
  command: typeof WEBSEARCH_COMMAND;
  templateProofHash: string;
  query: string;
  goal:
    | "source_backed_answer_and_ranked_urls"
    | "url_discovery_for_scrapers"
    | "citation_verification"
    | "comparative_research"
    | "latest_status_check"
    | "media_enrichment";
  providers: WebSearchProviderRoute;
  toolChoice: "auto" | "required";
  freshness: "auto" | "latest" | "past_day" | "past_week" | "past_month" | "past_year" | "any_time";
  allowedDomains: string[];
  blockedDomains: string[];
  maxSearches: number;
  topKUrls: number;
  searchContextSize: "low" | "medium" | "high";
  mediaIntent: WebSearchMediaIntent;
  searchContentTypes: WebSearchContentTypes;
  imageMaxResults: number;
  mediaSafety: "urls_metadata_only_until_user_approval";
  userLocation: string;
  locale: "fr" | "en" | "auto";
  extractIntent: "none" | "suggest_scrapers_urls" | "next_loop_scrapers";
  output:
    | "compact_answer_url_citation_manifest"
    | "url_manifest_only"
    | "comparison_matrix"
    | "verification_report"
    | "media_manifest";
  source: "explicit_codeact";
  proofHash: string;
}

export interface WebSearchTemplateResult {
  schema: typeof WEBSEARCH_TEMPLATE_RESULT_SCHEMA;
  command: typeof WEBSEARCH_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    goal: WebSearchCodeActRequest["goal"][];
    providers: WebSearchProviderRoute[];
    toolChoice: WebSearchCodeActRequest["toolChoice"][];
    freshness: WebSearchCodeActRequest["freshness"][];
    searchContextSize: WebSearchCodeActRequest["searchContextSize"][];
    mediaIntent: WebSearchMediaIntent[];
    searchContentTypes: WebSearchContentTypes[];
    mediaSafety: WebSearchCodeActRequest["mediaSafety"][];
    locale: WebSearchCodeActRequest["locale"][];
    extractIntent: WebSearchCodeActRequest["extractIntent"][];
    output: WebSearchCodeActRequest["output"][];
  };
  proofHash: string;
}

export type WebSearchCodeAct =
  | { kind: "template"; result: WebSearchTemplateResult }
  | { kind: "request"; request: WebSearchCodeActRequest };

export interface WebSearchUrlCandidate {
  url: string;
  title?: string;
  snippet?: string;
  pageAge?: string;
  provider: WebSearchProviderId;
}

export interface WebSearchCitation {
  url: string;
  title?: string;
  citedText?: string;
  provider: WebSearchProviderId;
}

export interface WebSearchMediaCandidate {
  kind: WebSearchMediaKind;
  url: string;
  thumbnailUrl?: string;
  sourceUrl?: string;
  title?: string;
  caption?: string;
  mimeType?: string;
  provider: WebSearchProviderId;
}

export interface WebSearchProviderResult {
  provider: WebSearchProviderId;
  status: WebSearchStatus;
  durationMs: number;
  model?: string;
  searchedQueries: string[];
  answer: string;
  urls: WebSearchUrlCandidate[];
  citations: WebSearchCitation[];
  media: WebSearchMediaCandidate[];
  warnings: string[];
  error?: string;
}

export interface WebSearchBridgeResult {
  schema: typeof WEBSEARCH_RESULT_SCHEMA;
  command: typeof WEBSEARCH_COMMAND;
  status: WebSearchStatus;
  requestHash: string;
  startedAt: string;
  finishedAt: string;
  durationMs: number;
  providers: WebSearchProviderResult[];
  answer: string;
  urls: WebSearchUrlCandidate[];
  citations: WebSearchCitation[];
  media: WebSearchMediaCandidate[];
  suggestedScraperUrls: string[];
  warnings: string[];
  proofHash: string;
}

const WEBSEARCH_GOALS: WebSearchCodeActRequest["goal"][] = [
  "source_backed_answer_and_ranked_urls",
  "url_discovery_for_scrapers",
  "citation_verification",
  "comparative_research",
  "latest_status_check",
  "media_enrichment"
];
const WEBSEARCH_PROVIDER_ROUTES: WebSearchProviderRoute[] = [
  "auto",
  "openai",
  "claude",
  "openai_then_claude",
  "claude_then_openai",
  "both_parallel"
];
const WEBSEARCH_TOOL_CHOICES: WebSearchCodeActRequest["toolChoice"][] = ["auto", "required"];
const WEBSEARCH_FRESHNESS: WebSearchCodeActRequest["freshness"][] = [
  "auto",
  "latest",
  "past_day",
  "past_week",
  "past_month",
  "past_year",
  "any_time"
];
const WEBSEARCH_CONTEXT_SIZES: WebSearchCodeActRequest["searchContextSize"][] = ["low", "medium", "high"];
const WEBSEARCH_MEDIA_INTENTS: WebSearchMediaIntent[] = [
  "none",
  "image_enrichment",
  "video_enrichment",
  "audio_enrichment",
  "image_video_audio_enrichment"
];
const WEBSEARCH_CONTENT_TYPES: WebSearchContentTypes[] = ["text", "text_image", "image"];
const WEBSEARCH_MEDIA_SAFETY: WebSearchCodeActRequest["mediaSafety"][] = ["urls_metadata_only_until_user_approval"];
const WEBSEARCH_LOCALES: WebSearchCodeActRequest["locale"][] = ["fr", "en", "auto"];
const WEBSEARCH_EXTRACT_INTENTS: WebSearchCodeActRequest["extractIntent"][] = [
  "none",
  "suggest_scrapers_urls",
  "next_loop_scrapers"
];
const WEBSEARCH_OUTPUTS: WebSearchCodeActRequest["output"][] = [
  "compact_answer_url_citation_manifest",
  "url_manifest_only",
  "comparison_matrix",
  "verification_report",
  "media_manifest"
];

export function webSearchTemplateResult(reason: WebSearchTemplateResult["reason"] = "empty_command"): WebSearchTemplateResult {
  const templateProofHash = webSearchTemplateProofHash();
  const template = [
    `${WEBSEARCH_COMMAND}`,
    `template_proof_hash="sha256:${templateProofHash}"`,
    'query=""',
    'goal="source_backed_answer_and_ranked_urls|url_discovery_for_scrapers|citation_verification|comparative_research|latest_status_check|media_enrichment"',
    'providers="auto|openai|claude|openai_then_claude|claude_then_openai|both_parallel"',
    'tool_choice="required|auto"',
    'freshness="auto|latest|past_day|past_week|past_month|past_year|any_time"',
    "allowed_domains=[]",
    "blocked_domains=[]",
    "max_searches=5",
    "top_k_urls=8",
    'search_context_size="low|medium|high"',
    'media_intent="none|image_enrichment|video_enrichment|audio_enrichment|image_video_audio_enrichment"',
    'search_content_types="text|text_image|image"',
    "image_max_results=6",
    'media_safety="urls_metadata_only_until_user_approval"',
    'user_location="none"',
    'locale="fr|en|auto"',
    'extract_intent="none|suggest_scrapers_urls|next_loop_scrapers"',
    'output="compact_answer_url_citation_manifest|url_manifest_only|comparison_matrix|verification_report|media_manifest"'
  ].join("\n");
  const result: WebSearchTemplateResult = {
    schema: WEBSEARCH_TEMPLATE_RESULT_SCHEMA,
    command: WEBSEARCH_COMMAND,
    status: "template",
    reason,
    template,
    allowedValues: {
      goal: WEBSEARCH_GOALS,
      providers: WEBSEARCH_PROVIDER_ROUTES,
      toolChoice: WEBSEARCH_TOOL_CHOICES,
      freshness: WEBSEARCH_FRESHNESS,
      searchContextSize: WEBSEARCH_CONTEXT_SIZES,
      mediaIntent: WEBSEARCH_MEDIA_INTENTS,
      searchContentTypes: WEBSEARCH_CONTENT_TYPES,
      mediaSafety: WEBSEARCH_MEDIA_SAFETY,
      locale: WEBSEARCH_LOCALES,
      extractIntent: WEBSEARCH_EXTRACT_INTENTS,
      output: WEBSEARCH_OUTPUTS
    },
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function renderWebSearchTemplateResult(result: WebSearchTemplateResult): string {
  return [
    "WEBSEARCH_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${webSearchTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function readWebSearchCodeAct(input: string): WebSearchCodeAct | undefined {
  const trimmed = webSearchCodeActText(input).trim();
  if (!trimmed) {
    return undefined;
  }
  const body = trimmed.slice(WEBSEARCH_COMMAND.length).trim();
  if (!body) {
    return { kind: "template", result: webSearchTemplateResult("empty_command") };
  }
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: webSearchTemplateResult("template_required") };
  }
  const request = parseWebSearchCodeAct(trimmed);
  if (!request || !request.query.trim()) {
    return { kind: "template", result: webSearchTemplateResult("template_required") };
  }
  return { kind: "request", request };
}

export function parseWebSearchCodeAct(input: string): WebSearchCodeActRequest | undefined {
  const trimmed = webSearchCodeActText(input).trim();
  if (!readWebSearchCommand(trimmed)) {
    return undefined;
  }
  const body = trimmed.slice(WEBSEARCH_COMMAND.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? body : "";
  const query = clampText(fields.get("query") ?? fields.get("q") ?? fields.get("topic") ?? freeform, MAX_QUERY_CHARS);
  if (!query) {
    return undefined;
  }
  const request: WebSearchCodeActRequest = {
    schema: "forge.websearch.request.v1",
    command: WEBSEARCH_COMMAND,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    query,
    goal: readChoice(fields.get("goal"), WEBSEARCH_GOALS, "source_backed_answer_and_ranked_urls"),
    providers: readChoice(fields.get("providers") ?? fields.get("provider"), WEBSEARCH_PROVIDER_ROUTES, "auto"),
    toolChoice: readChoice(fields.get("tool_choice"), WEBSEARCH_TOOL_CHOICES, "required"),
    freshness: readChoice(fields.get("freshness") ?? fields.get("recency"), WEBSEARCH_FRESHNESS, "auto"),
    allowedDomains: parseDomains(fields.get("allowed_domains") ?? fields.get("domains")),
    blockedDomains: parseDomains(fields.get("blocked_domains")),
    maxSearches: clampNumber(fields.get("max_searches"), 1, 20, 5),
    topKUrls: clampNumber(fields.get("top_k_urls") ?? fields.get("top_k"), 1, 30, 8),
    searchContextSize: readChoice(fields.get("search_context_size"), WEBSEARCH_CONTEXT_SIZES, "medium"),
    mediaIntent: readChoice(fields.get("media_intent"), WEBSEARCH_MEDIA_INTENTS, readMediaIntentFallback(fields)),
    searchContentTypes: readChoice(fields.get("search_content_types"), WEBSEARCH_CONTENT_TYPES, readSearchContentTypesFallback(fields)),
    imageMaxResults: clampNumber(fields.get("image_max_results") ?? fields.get("max_images"), 0, 30, 6),
    mediaSafety: readChoice(fields.get("media_safety"), WEBSEARCH_MEDIA_SAFETY, "urls_metadata_only_until_user_approval"),
    userLocation: clampText(fields.get("user_location") ?? "", 240),
    locale: readChoice(fields.get("locale") ?? fields.get("lang"), WEBSEARCH_LOCALES, "fr"),
    extractIntent: readChoice(fields.get("extract_intent"), WEBSEARCH_EXTRACT_INTENTS, "none"),
    output: readChoice(fields.get("output"), WEBSEARCH_OUTPUTS, "compact_answer_url_citation_manifest"),
    source: "explicit_codeact",
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function extractWebSearchCodeAct(input: string): WebSearchCodeActRequest | undefined {
  const explicit = parseWebSearchCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const lines = input.split(/\r?\n/);
  const startIndex = lines.findIndex((line) => Boolean(readWebSearchCommand(line)));
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
  return parseWebSearchCodeAct(block.join("\n"));
}

export function renderWebSearchCodeActResult(result: WebSearchBridgeResult): string {
  const providerSummary = result.providers.map((provider) => ({
    provider: provider.provider,
    status: provider.status,
    model: provider.model,
    duration_ms: provider.durationMs,
    queries: provider.searchedQueries,
    urls: provider.urls.length,
    citations: provider.citations.length,
    media: provider.media.length,
    error: provider.error
  }));
  return [
    "WEBSEARCH_RESULT",
    `schema=${WEBSEARCH_RESULT_SCHEMA}`,
    `command=${WEBSEARCH_COMMAND}`,
    `status=${result.status}`,
    `request_hash=sha256:${result.requestHash}`,
    `duration_ms=${result.durationMs}`,
    `providers=${JSON.stringify(providerSummary)}`,
    `answer=${JSON.stringify(result.answer)}`,
    `urls=${JSON.stringify(result.urls.slice(0, 30))}`,
    `citations=${JSON.stringify(result.citations.slice(0, 30))}`,
    `media=${JSON.stringify(result.media.slice(0, 30))}`,
    `suggested_scraper_urls=${JSON.stringify(result.suggestedScraperUrls.slice(0, 30))}`,
    `warnings=${JSON.stringify(result.warnings)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function stableWebSearchHash(value: unknown): string {
  return stableHash(value);
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

function readWebSearchCommand(value: string): typeof WEBSEARCH_COMMAND | undefined {
  const trimmed = value.trim().toLowerCase();
  if (trimmed === WEBSEARCH_COMMAND) {
    return WEBSEARCH_COMMAND;
  }
  if (trimmed.startsWith(WEBSEARCH_COMMAND) && /\s/u.test(trimmed.charAt(WEBSEARCH_COMMAND.length))) {
    return WEBSEARCH_COMMAND;
  }
  return undefined;
}

function webSearchCodeActText(input: string): string {
  const commandIndex = input.indexOf(WEBSEARCH_COMMAND);
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

function webSearchTemplateProofHash(): string {
  return stableHash({
    command: WEBSEARCH_COMMAND,
    schema: WEBSEARCH_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "query",
      "goal",
      "providers",
      "tool_choice",
      "freshness",
      "allowed_domains",
      "blocked_domains",
      "max_searches",
      "top_k_urls",
      "search_context_size",
      "media_intent",
      "search_content_types",
      "image_max_results",
      "media_safety",
      "user_location",
      "locale",
      "extract_intent",
      "output"
    ],
    allowedValues: {
      goal: WEBSEARCH_GOALS,
      providers: WEBSEARCH_PROVIDER_ROUTES,
      toolChoice: WEBSEARCH_TOOL_CHOICES,
      freshness: WEBSEARCH_FRESHNESS,
      searchContextSize: WEBSEARCH_CONTEXT_SIZES,
      mediaIntent: WEBSEARCH_MEDIA_INTENTS,
      searchContentTypes: WEBSEARCH_CONTENT_TYPES,
      mediaSafety: WEBSEARCH_MEDIA_SAFETY,
      locale: WEBSEARCH_LOCALES,
      extractIntent: WEBSEARCH_EXTRACT_INTENTS,
      output: WEBSEARCH_OUTPUTS
    }
  });
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === webSearchTemplateProofHash();
}

function normalizeProofHash(value: unknown): string {
  return String(value ?? "").trim().replace(/^sha256:/i, "");
}

function indentBlock(value: string, prefix: string): string {
  return value.split(/\r?\n/).map((line) => `${prefix}${line}`).join("\n");
}

function decodeTemplateValue(value: string): string {
  return value
    .replace(/\\"/gu, "\"")
    .replace(/\\'/gu, "'")
    .replace(/\\n/gu, "\n")
    .replace(/\\t/gu, "\t")
    .replace(/\\\\/gu, "\\");
}

function parseDomains(value: unknown): string[] {
  const values = parseStringList(value);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of values) {
    const clean = normalizeDomain(item);
    if (!clean || seen.has(clean)) {
      continue;
    }
    seen.add(clean);
    result.push(clean);
    if (result.length >= MAX_DOMAINS) {
      break;
    }
  }
  return result;
}

function parseStringList(value: unknown): string[] {
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
  return text.split(/[,|;\n]+/).map((item) => item.trim()).filter(Boolean);
}

function normalizeDomain(value: string): string {
  const clean = clampText(value, MAX_DOMAIN_CHARS).replace(/^https?:\/\//i, "").split("/")[0]?.trim().toLowerCase() ?? "";
  return clean.replace(/^\.+|\.+$/g, "");
}

function readChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return choices.find((choice) => choice === normalized) ?? fallback;
}

function readMediaIntentFallback(fields: Map<string, string>): WebSearchMediaIntent {
  const explicit = fields.get("media_intent")?.toLowerCase() ?? "";
  if (
    explicit === "image_enrichment" ||
    explicit === "video_enrichment" ||
    explicit === "audio_enrichment" ||
    explicit === "image_video_audio_enrichment"
  ) {
    return explicit;
  }
  const output = fields.get("output")?.toLowerCase() ?? "";
  const goal = fields.get("goal")?.toLowerCase() ?? "";
  const query = fields.get("query")?.toLowerCase() ?? fields.get("q")?.toLowerCase() ?? fields.get("topic")?.toLowerCase() ?? "";
  if (output === "media_manifest" || goal === "media_enrichment") {
    return "image_enrichment";
  }
  if (/\b(video|audio|image|images|photo|photos|visual|thumbnail|poster)\b/u.test(query)) {
    if (/\bvideo\b/u.test(query) && /\baudio\b/u.test(query)) {
      return "image_video_audio_enrichment";
    }
    if (/\bvideo\b/u.test(query)) {
      return "video_enrichment";
    }
    if (/\baudio\b/u.test(query)) {
      return "audio_enrichment";
    }
    return "image_enrichment";
  }
  return "none";
}

function readSearchContentTypesFallback(fields: Map<string, string>): WebSearchContentTypes {
  const mediaIntent = readMediaIntentFallback(fields);
  if (mediaIntent === "image_enrichment" || mediaIntent === "image_video_audio_enrichment") {
    return "text_image";
  }
  if (fields.get("output")?.toLowerCase() === "media_manifest") {
    return "text_image";
  }
  return "text";
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
