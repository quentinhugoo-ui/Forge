import { createHash } from "node:crypto";
import {
  BRAIN_SCRAPERS_COMMAND,
  BRAIN_SCRAPERS_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const SCRAPERS_COMMAND = BRAIN_SCRAPERS_COMMAND;
export const SCRAPERS_RESULT_SCHEMA = BRAIN_SCRAPERS_RESULT_SCHEMA;

const MAX_GOAL_CHARS = 600;
const MAX_URLS = 12;
const MAX_SELECTOR_CHARS = 220;
const MAX_SELECTORS = 24;
const DEFAULT_LIMITS: ScrapersLimits = {
  pages: 1,
  links: 50,
  images: 50,
  bytes: 5_242_880,
  timeoutMs: 30_000,
  concurrency: 4
};

export type ScrapersBackendId = "scrapling" | "crawl4ai";
export type ScrapersStatus = "ok" | "partial" | "error";

export interface ScrapersLimits {
  pages: number;
  links: number;
  images: number;
  bytes: number;
  timeoutMs: number;
  concurrency: number;
}

export interface ScrapersFieldContract {
  name: string;
  selector?: string;
  attr?: string;
  multiple?: boolean;
  required?: boolean;
  description?: string;
}

export interface ScrapersCodeActRequest {
  schema: "forge.scrapers.mcp.request.v1";
  command: typeof SCRAPERS_COMMAND;
  urls: string[];
  goal: string;
  fieldsSchema: ScrapersFieldContract[];
  selectors: string[];
  extractionMode:
    | "structured_markdown_media"
    | "structured_only"
    | "markdown_only"
    | "media_catalog"
    | "screenshot"
    | "archive_snapshot";
  fetchMode: "auto" | "http" | "dynamic" | "stealth" | "session";
  session: string;
  waitUntil: "dom_ready" | "network_idle" | "selector" | "images_loaded" | "virtual_scroll_complete";
  waitSelector: string;
  markdownQuery: string;
  contentFilter: "none" | "fit_markdown" | "bm25" | "pruning" | "fit_markdown_bm25_pruning";
  crawlDepth: "single_page" | "same_domain_depth_1" | "same_domain_depth_2" | "docs_site_bounded";
  links: "none" | "internal" | "external" | "both" | "both_scored";
  media: "none" | "image_urls" | "image_urls_and_metadata" | "all_media_urls" | "download_image_artifacts";
  imagePolicy: "urls_only" | "urls_and_metadata" | "download_artifacts" | "screenshot_only";
  artifacts: "none" | "screenshot" | "full_page_screenshot" | "pdf" | "mhtml" | "downloaded_files";
  artifactFormat: "png" | "jpeg" | "pdf" | "mhtml" | "original";
  limits: ScrapersLimits;
  dedupeKey:
    | "canonical_url"
    | "canonical_url+selector_path"
    | "canonical_url+selector_path+content_hash"
    | "media_src";
  backends: ScrapersBackendId[];
  mergePolicy:
    | "structured_fields_then_fit_markdown"
    | "markdown_then_structured_evidence"
    | "compare_and_dedupe";
  provenance: string[];
  source: "explicit_codeact";
  proofHash: string;
}

export interface ScrapersArtifactRef {
  backend: ScrapersBackendId;
  kind: string;
  url?: string;
  mimeType?: string;
  bytes?: number;
  sha256?: string;
  preview?: string;
}

export interface ScrapersProviderResult {
  backend: ScrapersBackendId;
  status: ScrapersStatus;
  transport: "mcp_stdio" | "crawl4ai_http";
  durationMs: number;
  tool?: string;
  endpoint?: string;
  error?: string;
  warnings: string[];
  counts: {
    urls: number;
    fields: number;
    markdownChars: number;
    links: number;
    media: number;
    artifacts: number;
  };
  data: {
    urls: string[];
    fields: unknown[];
    markdown: string[];
    links: unknown[];
    media: unknown[];
    artifacts: ScrapersArtifactRef[];
    rawPreview?: unknown;
  };
}

export interface ScrapersBridgeResult {
  schema: typeof SCRAPERS_RESULT_SCHEMA;
  command: typeof SCRAPERS_COMMAND;
  status: ScrapersStatus;
  requestHash: string;
  startedAt: string;
  finishedAt: string;
  durationMs: number;
  providers: ScrapersProviderResult[];
  merged: {
    urls: string[];
    fields: unknown[];
    markdown: string[];
    links: unknown[];
    media: unknown[];
    artifacts: ScrapersArtifactRef[];
    warnings: string[];
    provenance: Array<Record<string, unknown>>;
  };
  proofHash: string;
}

export function parseScrapersCodeAct(input: string): ScrapersCodeActRequest | undefined {
  const trimmed = input.trim();
  if (!readScrapersCommand(trimmed)) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(SCRAPERS_COMMAND.length).trim());
  const urls = parseUrls(fields.get("urls") ?? fields.get("url"));
  if (urls.length === 0) {
    return undefined;
  }

  const goal = clampText(
    fields.get("goal") ?? fields.get("markdown_query") ?? fields.get("query") ?? "bounded web collection",
    MAX_GOAL_CHARS
  );
  const markdownQuery = clampText(fields.get("markdown_query") ?? goal, MAX_GOAL_CHARS);
  const request: ScrapersCodeActRequest = {
    schema: "forge.scrapers.mcp.request.v1",
    command: SCRAPERS_COMMAND,
    urls,
    goal,
    fieldsSchema: parseFieldsSchema(fields.get("fields_schema")),
    selectors: parseSelectors(fields.get("selectors")),
    extractionMode: readChoice(fields.get("extraction_mode"), [
      "structured_markdown_media",
      "structured_only",
      "markdown_only",
      "media_catalog",
      "screenshot",
      "archive_snapshot"
    ], "structured_markdown_media"),
    fetchMode: readChoice(fields.get("fetch_mode"), ["auto", "http", "dynamic", "stealth", "session"], "auto"),
    session: clampText(fields.get("session") ?? "", 120),
    waitUntil: readChoice(fields.get("wait_until"), [
      "dom_ready",
      "network_idle",
      "selector",
      "images_loaded",
      "virtual_scroll_complete"
    ], "dom_ready"),
    waitSelector: clampText(fields.get("wait_selector") ?? "", MAX_SELECTOR_CHARS),
    markdownQuery,
    contentFilter: readChoice(fields.get("content_filter"), [
      "none",
      "fit_markdown",
      "bm25",
      "pruning",
      "fit_markdown_bm25_pruning"
    ], "fit_markdown_bm25_pruning"),
    crawlDepth: readChoice(fields.get("crawl_depth"), [
      "single_page",
      "same_domain_depth_1",
      "same_domain_depth_2",
      "docs_site_bounded"
    ], "single_page"),
    links: readChoice(fields.get("links"), ["none", "internal", "external", "both", "both_scored"], "both_scored"),
    media: readChoice(fields.get("media"), [
      "none",
      "image_urls",
      "image_urls_and_metadata",
      "all_media_urls",
      "download_image_artifacts"
    ], "image_urls_and_metadata"),
    imagePolicy: readChoice(fields.get("image_policy"), [
      "urls_only",
      "urls_and_metadata",
      "download_artifacts",
      "screenshot_only"
    ], "urls_only"),
    artifacts: readChoice(fields.get("artifacts"), [
      "none",
      "screenshot",
      "full_page_screenshot",
      "pdf",
      "mhtml",
      "downloaded_files"
    ], "none"),
    artifactFormat: readChoice(fields.get("artifact_format"), ["png", "jpeg", "pdf", "mhtml", "original"], "png"),
    limits: parseLimits(fields.get("limits")),
    dedupeKey: readChoice(fields.get("dedupe_key"), [
      "canonical_url",
      "canonical_url+selector_path",
      "canonical_url+selector_path+content_hash",
      "media_src"
    ], "canonical_url+selector_path+content_hash"),
    backends: parseBackends(fields.get("backends")),
    mergePolicy: readChoice(fields.get("merge_policy"), [
      "structured_fields_then_fit_markdown",
      "markdown_then_structured_evidence",
      "compare_and_dedupe"
    ], "structured_fields_then_fit_markdown"),
    provenance: parseList(fields.get("provenance"), 20, 80),
    source: "explicit_codeact",
    proofHash: ""
  };
  if (request.provenance.length === 0) {
    request.provenance = ["url", "final_url", "status", "timestamp", "selector_or_chunk", "backend", "artifact_hash"];
  }
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function extractScrapersCodeAct(input: string): ScrapersCodeActRequest | undefined {
  const explicit = parseScrapersCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => Boolean(readScrapersCommand(item)));
  return line ? parseScrapersCodeAct(line) : undefined;
}

export function renderScrapersCodeActResult(result: ScrapersBridgeResult): string {
  const providerSummary = result.providers.map((provider) => ({
    backend: provider.backend,
    status: provider.status,
    transport: provider.transport,
    tool: provider.tool,
    endpoint: provider.endpoint,
    duration_ms: provider.durationMs,
    counts: provider.counts,
    error: provider.error
  }));
  const manifest = {
    schema: result.schema,
    status: result.status,
    request_hash: `sha256:${result.requestHash}`,
    providers: providerSummary,
    merged: result.merged
  };
  return [
    "SCRAPERS_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `request_hash=sha256:${result.requestHash}`,
    `duration_ms=${result.durationMs}`,
    `providers=${JSON.stringify(providerSummary)}`,
    `manifest=${boundedJson(manifest, 60_000)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function stableScrapersHash(value: unknown): string {
  return stableHash(value);
}

function readScrapersCommand(value: string): typeof SCRAPERS_COMMAND | undefined {
  const trimmed = value.trim();
  const lower = trimmed.toLowerCase();
  return lower === SCRAPERS_COMMAND || lower.startsWith(`${SCRAPERS_COMMAND} `) ? SCRAPERS_COMMAND : undefined;
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

function parseUrls(value: unknown): string[] {
  const candidates = parseList(value, MAX_URLS, 2_000);
  const seen = new Set<string>();
  const urls: string[] = [];
  for (const candidate of candidates) {
    try {
      const url = new URL(candidate);
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        continue;
      }
      const normalized = url.toString();
      if (seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      urls.push(normalized);
      if (urls.length >= MAX_URLS) {
        break;
      }
    } catch {
      continue;
    }
  }
  return urls;
}

function parseFieldsSchema(value: unknown): ScrapersFieldContract[] {
  if (typeof value !== "string" || !value.trim()) {
    return [];
  }
  try {
    const parsed = JSON.parse(value) as unknown;
    const items = Array.isArray(parsed) ? parsed : [parsed];
    return items
      .map((item) => normalizeFieldContract(item))
      .filter((item): item is ScrapersFieldContract => Boolean(item))
      .slice(0, MAX_SELECTORS);
  } catch {
    return [];
  }
}

function normalizeFieldContract(value: unknown): ScrapersFieldContract | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const source = value as Record<string, unknown>;
  const name = clampText(String(source.name ?? source.key ?? ""), 80);
  if (!name) {
    return undefined;
  }
  const field: ScrapersFieldContract = { name };
  const selector = clampText(String(source.selector ?? source.css ?? source.xpath ?? ""), MAX_SELECTOR_CHARS);
  const attr = clampText(String(source.attr ?? source.attribute ?? ""), 60);
  const description = clampText(String(source.description ?? ""), 180);
  if (selector) field.selector = selector;
  if (attr) field.attr = attr;
  if (typeof source.multiple === "boolean") field.multiple = source.multiple;
  if (typeof source.required === "boolean") field.required = source.required;
  if (description) field.description = description;
  return field;
}

function parseSelectors(value: unknown): string[] {
  return parseList(value, MAX_SELECTORS, MAX_SELECTOR_CHARS);
}

function parseBackends(value: unknown): ScrapersBackendId[] {
  const selected = parseList(value, 4, 40);
  const wanted = new Set<ScrapersBackendId>();
  for (const item of selected) {
    const key = item.toLowerCase();
    if (key === "scrapling") wanted.add("scrapling");
    if (key === "crawl4ai" || key === "crawl4-ai" || key === "crawl") wanted.add("crawl4ai");
  }
  if (wanted.size === 0) {
    return ["scrapling", "crawl4ai"];
  }
  const backends: ScrapersBackendId[] = ["scrapling", "crawl4ai"];
  return backends.filter((backend) => wanted.has(backend));
}

function parseLimits(value: unknown): ScrapersLimits {
  const limits = { ...DEFAULT_LIMITS };
  if (typeof value !== "string" || !value.trim()) {
    return limits;
  }
  const pairs = value.split(/[,;\n]+/).map((item) => item.trim()).filter(Boolean);
  for (const pair of pairs) {
    const match = /^([a-zA-Z_][\w-]*)\s*=\s*(\d+)$/u.exec(pair);
    if (!match) {
      continue;
    }
    const key = match[1].toLowerCase();
    const amount = Number(match[2]);
    if (key === "pages") limits.pages = clampNumber(amount, 1, 20);
    if (key === "links") limits.links = clampNumber(amount, 0, 250);
    if (key === "images" || key === "media") limits.images = clampNumber(amount, 0, 250);
    if (key === "bytes") limits.bytes = clampNumber(amount, 0, 52_428_800);
    if (key === "timeout_ms" || key === "timeout") limits.timeoutMs = clampNumber(amount, 2_000, 120_000);
    if (key === "concurrency") limits.concurrency = clampNumber(amount, 1, 8);
  }
  return limits;
}

function parseList(value: unknown, maxItems: number, maxChars: number): string[] {
  if (typeof value !== "string" || !value.trim()) {
    return [];
  }
  const trimmed = value.trim();
  let rawItems: unknown[] | undefined;
  if (trimmed.startsWith("[") || trimmed.startsWith("{")) {
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      rawItems = Array.isArray(parsed) ? parsed : [parsed];
    } catch {
      rawItems = undefined;
    }
  }
  const items = rawItems ?? trimmed.split(/[,|\n]+/);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of items) {
    const clean = clampText(String(item), maxChars);
    const key = normalizeText(clean);
    if (!clean || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(clean);
    if (result.length >= maxItems) {
      break;
    }
  }
  return result;
}

function readChoice<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return allowed.find((item) => item === normalized) ?? fallback;
}

function clampText(text: string, maxChars: number): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function clampNumber(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, Math.floor(value)));
}

function normalizeText(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .replace(/\s+/g, " ")
    .trim();
}

function boundedJson(value: unknown, maxChars: number): string {
  const raw = JSON.stringify(value);
  if (raw.length <= maxChars) {
    return raw;
  }
  return JSON.stringify({
    truncated: true,
    max_chars: maxChars,
    preview: raw.slice(0, Math.max(0, maxChars - 200)),
    full_sha256: stableHash(raw)
  });
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
