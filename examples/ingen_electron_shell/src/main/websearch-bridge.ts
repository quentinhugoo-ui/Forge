import {
  WEBSEARCH_COMMAND,
  WEBSEARCH_RESULT_SCHEMA,
  stableWebSearchHash,
  type WebSearchBridgeResult,
  type WebSearchCitation,
  type WebSearchCodeActRequest,
  type WebSearchMediaCandidate,
  type WebSearchProviderId,
  type WebSearchProviderResult,
  type WebSearchProviderRoute,
  type WebSearchStatus,
  type WebSearchUrlCandidate
} from "./websearch-codeact.js";

const OPENAI_WEBSEARCH_ENDPOINT = "https://api.openai.com/v1/responses";
const CLAUDE_WEBSEARCH_ENDPOINT = "https://api.anthropic.com/v1/messages";
const DEFAULT_OPENAI_WEBSEARCH_MODEL = "gpt-5.5";
const DEFAULT_CLAUDE_WEBSEARCH_MODEL = "claude-sonnet-4-5-20250929";
const WEBSEARCH_TIMEOUT_MS = 45_000;
const MAX_ANSWER_CHARS = 3_000;
const MAX_SNIPPET_CHARS = 500;

export async function runWebSearchBridge(request: WebSearchCodeActRequest): Promise<WebSearchBridgeResult> {
  const startedAt = new Date().toISOString();
  const started = Date.now();
  const route = providerRoute(request.providers);
  const providers = await Promise.all(route.map((provider) => runProvider(provider, request)));
  const finishedAt = new Date().toISOString();
  const okCount = providers.filter((provider) => provider.status === "ok").length;
  const status: WebSearchStatus = okCount === providers.length && providers.length > 0 ? "ok" : okCount > 0 ? "partial" : "error";
  const urls = dedupeUrls(providers.flatMap((provider) => provider.urls)).slice(0, request.topKUrls);
  const citations = dedupeCitations(providers.flatMap((provider) => provider.citations)).slice(0, request.topKUrls * 2);
  const media = dedupeMedia(providers.flatMap((provider) => provider.media)).slice(0, request.imageMaxResults || request.topKUrls);
  const warnings = providers.flatMap((provider) => provider.warnings);
  const suggestedScraperUrls = uniqueStrings([
    ...urls.map((item) => item.url),
    ...media.map((item) => item.sourceUrl || item.url).filter(Boolean)
  ]).slice(0, request.topKUrls);
  const answer = compactAnswer(providers);
  const result: WebSearchBridgeResult = {
    schema: WEBSEARCH_RESULT_SCHEMA,
    command: WEBSEARCH_COMMAND,
    status,
    requestHash: request.proofHash,
    startedAt,
    finishedAt,
    durationMs: Date.now() - started,
    providers,
    answer,
    urls,
    citations,
    media,
    suggestedScraperUrls,
    warnings,
    proofHash: ""
  };
  result.proofHash = stableWebSearchHash({ ...result, proofHash: "" });
  return result;
}

function providerRoute(route: WebSearchProviderRoute): WebSearchProviderId[] {
  switch (route) {
    case "openai":
      return ["openai"];
    case "claude":
      return ["claude"];
    case "openai_then_claude":
      return ["openai", "claude"];
    case "claude_then_openai":
      return ["claude", "openai"];
    case "both_parallel":
      return ["openai", "claude"];
    case "auto":
    default: {
      const providers: WebSearchProviderId[] = [];
      if (process.env.OPENAI_API_KEY || process.env.INGEN_OPENAI_API_KEY) providers.push("openai");
      if (process.env.ANTHROPIC_API_KEY || process.env.CLAUDE_API_KEY || process.env.INGEN_ANTHROPIC_API_KEY) providers.push("claude");
      return providers.length > 0 ? providers : ["openai", "claude"];
    }
  }
}

async function runProvider(provider: WebSearchProviderId, request: WebSearchCodeActRequest): Promise<WebSearchProviderResult> {
  if (provider === "openai") {
    return runOpenAiWebSearch(request);
  }
  return runClaudeWebSearch(request);
}

async function runOpenAiWebSearch(request: WebSearchCodeActRequest): Promise<WebSearchProviderResult> {
  const apiKey = process.env.OPENAI_API_KEY || process.env.INGEN_OPENAI_API_KEY || "";
  const started = Date.now();
  const model = process.env.INGEN_WEBSEARCH_OPENAI_MODEL || DEFAULT_OPENAI_WEBSEARCH_MODEL;
  if (!apiKey) {
    return providerError("openai", started, "OPENAI_API_KEY or INGEN_OPENAI_API_KEY is not configured.", model);
  }
  try {
    const tool: Record<string, unknown> = {
      type: "web_search"
    };
    const contentTypes = openAiSearchContentTypes(request);
    if (contentTypes.length > 0) {
      tool.search_content_types = contentTypes;
    }
    if (contentTypes.includes("image")) {
      tool.image_settings = {
        max_results: Math.max(1, request.imageMaxResults || 6),
        caption: true
      };
    }
    const filters: Record<string, string[]> = {};
    if (request.allowedDomains.length > 0) filters.allowed_domains = request.allowedDomains;
    if (request.blockedDomains.length > 0) filters.blocked_domains = request.blockedDomains;
    if (Object.keys(filters).length > 0) tool.filters = filters;
    const body: Record<string, unknown> = {
      model,
      reasoning: { effort: "low" },
      tools: [tool],
      tool_choice: request.toolChoice,
      include: ["web_search_call.action.sources", "web_search_call.results"],
      input: webSearchPrompt(request)
    };
    const parsed = await fetchJsonWithTimeout(OPENAI_WEBSEARCH_ENDPOINT, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json"
      },
      body: JSON.stringify(body)
    });
    const answer = compactText(readOpenAiOutputText(parsed), MAX_ANSWER_CHARS);
    const urls = collectOpenAiUrls(parsed).slice(0, request.topKUrls).map((item) => ({ ...item, provider: "openai" as const }));
    const citations = collectOpenAiCitations(parsed).slice(0, request.topKUrls * 2).map((item) => ({
      ...item,
      provider: "openai" as const
    }));
    const media = collectOpenAiMedia(parsed).slice(0, request.imageMaxResults || request.topKUrls).map((item) => ({
      ...item,
      provider: "openai" as const
    }));
    return providerOk("openai", started, model, {
      answer,
      urls,
      citations,
      media,
      searchedQueries: collectOpenAiQueries(parsed)
    });
  } catch (error) {
    return providerError("openai", started, friendlyError(error), model);
  }
}

async function runClaudeWebSearch(request: WebSearchCodeActRequest): Promise<WebSearchProviderResult> {
  const apiKey = process.env.ANTHROPIC_API_KEY || process.env.CLAUDE_API_KEY || process.env.INGEN_ANTHROPIC_API_KEY || "";
  const started = Date.now();
  const model = process.env.INGEN_WEBSEARCH_CLAUDE_MODEL || DEFAULT_CLAUDE_WEBSEARCH_MODEL;
  if (!apiKey) {
    return providerError("claude", started, "ANTHROPIC_API_KEY, CLAUDE_API_KEY or INGEN_ANTHROPIC_API_KEY is not configured.", model);
  }
  try {
    const tool: Record<string, unknown> = {
      type: "web_search_20250305",
      name: "web_search",
      max_uses: request.maxSearches
    };
    if (request.allowedDomains.length > 0) tool.allowed_domains = request.allowedDomains;
    if (request.blockedDomains.length > 0) tool.blocked_domains = request.blockedDomains;
    const location = parseApproximateLocation(request.userLocation);
    if (location) tool.user_location = location;
    const parsed = await fetchJsonWithTimeout(CLAUDE_WEBSEARCH_ENDPOINT, {
      method: "POST",
      headers: {
        "x-api-key": apiKey,
        "anthropic-version": "2023-06-01",
        "content-type": "application/json"
      },
      body: JSON.stringify({
        model,
        max_tokens: 1800,
        tools: [tool],
        messages: [{ role: "user", content: webSearchPrompt(request) }]
      })
    });
    const answer = compactText(readClaudeText(parsed), MAX_ANSWER_CHARS);
    const urls = collectClaudeUrls(parsed).slice(0, request.topKUrls).map((item) => ({ ...item, provider: "claude" as const }));
    const citations = collectClaudeCitations(parsed).slice(0, request.topKUrls * 2).map((item) => ({
      ...item,
      provider: "claude" as const
    }));
    const media = collectClaudeMediaPageCandidates(parsed, request).slice(0, request.imageMaxResults || request.topKUrls).map((item) => ({
      ...item,
      provider: "claude" as const
    }));
    return providerOk("claude", started, model, {
      answer,
      urls,
      citations,
      media,
      searchedQueries: collectClaudeQueries(parsed)
    });
  } catch (error) {
    return providerError("claude", started, friendlyError(error), model);
  }
}

function webSearchPrompt(request: WebSearchCodeActRequest): string {
  return [
    `Research query: ${request.query}`,
    `Goal: ${request.goal}`,
    `Freshness: ${request.freshness}`,
    `Locale: ${request.locale}`,
    request.allowedDomains.length > 0 ? `Allowed domains: ${request.allowedDomains.join(", ")}` : "",
    request.blockedDomains.length > 0 ? `Blocked domains: ${request.blockedDomains.join(", ")}` : "",
    "Return a concise source-backed answer and rank the most useful URLs.",
    request.mediaIntent !== "none"
      ? `Media enrichment requested: ${request.mediaIntent}. Return/source media candidates conservatively, with captions and source pages when available. Do not fabricate media URLs.`
      : "",
    "Do not include raw HTML or long page dumps.",
    request.extractIntent !== "none"
      ? "If extraction is needed, identify the exact URLs that should be passed to /scrapers_ next."
      : ""
  ].filter(Boolean).join("\n");
}

async function fetchJsonWithTimeout(url: string, init: RequestInit): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), WEBSEARCH_TIMEOUT_MS);
  try {
    const response = await fetch(url, { ...init, signal: controller.signal });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(text || `${url} returned ${response.status}`);
    }
    return text ? JSON.parse(text) : {};
  } finally {
    clearTimeout(timeout);
  }
}

function providerOk(
  provider: WebSearchProviderId,
  started: number,
  model: string,
  result: Pick<WebSearchProviderResult, "answer" | "urls" | "citations" | "media" | "searchedQueries">
): WebSearchProviderResult {
  return {
    provider,
    status: "ok",
    durationMs: Date.now() - started,
    model,
    warnings: [],
    ...result
  };
}

function providerError(
  provider: WebSearchProviderId,
  started: number,
  error: string,
  model?: string
): WebSearchProviderResult {
  return {
    provider,
    status: "error",
    durationMs: Date.now() - started,
    model,
    searchedQueries: [],
    answer: "",
    urls: [],
    citations: [],
    media: [],
    warnings: [error],
    error
  };
}

function openAiSearchContentTypes(request: WebSearchCodeActRequest): string[] {
  if (request.searchContentTypes === "image") {
    return ["image"];
  }
  if (
    request.searchContentTypes === "text_image" ||
    request.mediaIntent === "image_enrichment" ||
    request.mediaIntent === "image_video_audio_enrichment" ||
    request.output === "media_manifest"
  ) {
    return ["image", "text"];
  }
  return [];
}

function collectOpenAiUrls(parsed: unknown): Array<Omit<WebSearchUrlCandidate, "provider">> {
  const candidates: Array<Omit<WebSearchUrlCandidate, "provider">> = [];
  walkObjects(parsed, (record) => {
    const url = firstString(record, ["url", "uri", "source_url", "source_website_url"]);
    if (!url || !/^https?:\/\//i.test(url)) return;
    candidates.push({
      url,
      title: firstString(record, ["title", "source_title"]),
      snippet: compactText(firstString(record, ["snippet", "text", "caption"]), MAX_SNIPPET_CHARS)
    });
  });
  return dedupeUrlShape(candidates);
}

function collectOpenAiCitations(parsed: unknown): Array<Omit<WebSearchCitation, "provider">> {
  const citations: Array<Omit<WebSearchCitation, "provider">> = [];
  walkObjects(parsed, (record) => {
    if (record.type !== "url_citation" && !("url" in record)) return;
    const url = firstString(record, ["url"]);
    if (!url || !/^https?:\/\//i.test(url)) return;
    citations.push({
      url,
      title: firstString(record, ["title"]),
      citedText: compactText(firstString(record, ["cited_text", "text"]), 180)
    });
  });
  return dedupeCitationShape(citations);
}

function collectOpenAiMedia(parsed: unknown): Array<Omit<WebSearchMediaCandidate, "provider">> {
  const media: Array<Omit<WebSearchMediaCandidate, "provider">> = [];
  walkObjects(parsed, (record) => {
    const type = firstString(record, ["type"]);
    const imageUrl = firstString(record, ["image_url", "imageUrl", "url"]);
    if (type !== "image_result" && !firstString(record, ["image_url", "imageUrl"])) {
      return;
    }
    if (!imageUrl || !/^https?:\/\//i.test(imageUrl)) return;
    media.push({
      kind: "image",
      url: imageUrl,
      thumbnailUrl: firstString(record, ["thumbnail_url", "thumbnailUrl"]),
      sourceUrl: firstString(record, ["source_website_url", "sourceWebsiteUrl", "source_url", "sourceUrl"]),
      title: firstString(record, ["title"]),
      caption: compactText(firstString(record, ["caption", "alt", "description"]), 260),
      mimeType: mediaMimeType(imageUrl)
    });
  });
  return dedupeMediaShape(media);
}

function collectOpenAiQueries(parsed: unknown): string[] {
  const queries: string[] = [];
  walkObjects(parsed, (record) => {
    const query = firstString(record, ["query"]);
    if (query) queries.push(query);
  });
  return uniqueStrings(queries).slice(0, 20);
}

function readOpenAiOutputText(parsed: unknown): string {
  const root = parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {};
  if (typeof root.output_text === "string") {
    return root.output_text;
  }
  const texts: string[] = [];
  walkObjects(parsed, (record) => {
    if ((record.type === "output_text" || record.type === "text") && typeof record.text === "string") {
      texts.push(record.text);
    }
  });
  return texts.join("\n").trim();
}

function collectClaudeUrls(parsed: unknown): Array<Omit<WebSearchUrlCandidate, "provider">> {
  const urls: Array<Omit<WebSearchUrlCandidate, "provider">> = [];
  walkObjects(parsed, (record) => {
    if (record.type !== "web_search_result") return;
    const url = firstString(record, ["url"]);
    if (!url || !/^https?:\/\//i.test(url)) return;
    urls.push({
      url,
      title: firstString(record, ["title"]),
      pageAge: firstString(record, ["page_age"])
    });
  });
  return dedupeUrlShape(urls);
}

function collectClaudeCitations(parsed: unknown): Array<Omit<WebSearchCitation, "provider">> {
  const citations: Array<Omit<WebSearchCitation, "provider">> = [];
  walkObjects(parsed, (record) => {
    if (record.type !== "web_search_result_location") return;
    const url = firstString(record, ["url"]);
    if (!url || !/^https?:\/\//i.test(url)) return;
    citations.push({
      url,
      title: firstString(record, ["title"]),
      citedText: compactText(firstString(record, ["cited_text"]), 180)
    });
  });
  return dedupeCitationShape(citations);
}

function collectClaudeMediaPageCandidates(
  parsed: unknown,
  request: WebSearchCodeActRequest
): Array<Omit<WebSearchMediaCandidate, "provider">> {
  if (request.mediaIntent === "none" && request.output !== "media_manifest") {
    return [];
  }
  return collectClaudeUrls(parsed)
    .filter((item) => {
      const haystack = `${item.title ?? ""} ${item.snippet ?? ""}`.toLowerCase();
      if (request.mediaIntent === "audio_enrichment") return /\b(audio|sound|music|podcast|ost|voice|clip)\b/u.test(haystack);
      if (request.mediaIntent === "video_enrichment") return /\b(video|trailer|clip|gameplay|youtube|vimeo)\b/u.test(haystack);
      return true;
    })
    .map((item) => ({
      kind: "page" as const,
      url: item.url,
      sourceUrl: item.url,
      title: item.title,
      caption: compactText(item.snippet ?? item.pageAge ?? "", 260)
    }));
}

function collectClaudeQueries(parsed: unknown): string[] {
  const queries: string[] = [];
  walkObjects(parsed, (record) => {
    if (record.name === "web_search" && record.input && typeof record.input === "object") {
      const query = firstString(record.input as Record<string, unknown>, ["query"]);
      if (query) queries.push(query);
    }
  });
  return uniqueStrings(queries).slice(0, 20);
}

function readClaudeText(parsed: unknown): string {
  const texts: string[] = [];
  walkObjects(parsed, (record) => {
    if (record.type === "text" && typeof record.text === "string") {
      texts.push(record.text);
    }
  });
  return texts.join("\n").trim();
}

function parseApproximateLocation(value: string): Record<string, string> | undefined {
  if (!value || value === "none") return undefined;
  try {
    const parsed = JSON.parse(value) as unknown;
    if (parsed && typeof parsed === "object") {
      return { type: "approximate", ...(parsed as Record<string, string>) };
    }
  } catch {
    // Treat as plain city/country text below.
  }
  return { type: "approximate", city: value };
}

function compactAnswer(providers: WebSearchProviderResult[]): string {
  const answer = providers.find((provider) => provider.answer.trim())?.answer ?? "";
  return compactText(answer, MAX_ANSWER_CHARS);
}

function dedupeUrls(values: WebSearchUrlCandidate[]): WebSearchUrlCandidate[] {
  return dedupeBy(values, (item) => canonicalUrl(item.url));
}

function dedupeCitations(values: WebSearchCitation[]): WebSearchCitation[] {
  return dedupeBy(values, (item) => `${canonicalUrl(item.url)}:${item.citedText ?? ""}`);
}

function dedupeMedia(values: WebSearchMediaCandidate[]): WebSearchMediaCandidate[] {
  return dedupeBy(values, (item) => canonicalUrl(item.url));
}

function dedupeUrlShape<T extends { url: string }>(values: T[]): T[] {
  return dedupeBy(values, (item) => canonicalUrl(item.url));
}

function dedupeCitationShape<T extends { url: string; citedText?: string }>(values: T[]): T[] {
  return dedupeBy(values, (item) => `${canonicalUrl(item.url)}:${item.citedText ?? ""}`);
}

function dedupeMediaShape<T extends { url: string }>(values: T[]): T[] {
  return dedupeBy(values, (item) => canonicalUrl(item.url));
}

function dedupeBy<T>(values: T[], keyFor: (value: T) => string): T[] {
  const seen = new Set<string>();
  const result: T[] = [];
  for (const value of values) {
    const key = keyFor(value);
    if (!key || seen.has(key)) continue;
    seen.add(key);
    result.push(value);
  }
  return result;
}

function canonicalUrl(value: string): string {
  try {
    const url = new URL(value);
    url.hash = "";
    return url.toString();
  } catch {
    return value.trim();
  }
}

function uniqueStrings(values: string[]): string[] {
  return dedupeBy(values.map((item) => item.trim()).filter(Boolean), (item) => item.toLowerCase());
}

function walkObjects(value: unknown, visitor: (record: Record<string, unknown>) => void): void {
  if (!value || typeof value !== "object") return;
  if (Array.isArray(value)) {
    for (const item of value) walkObjects(item, visitor);
    return;
  }
  const record = value as Record<string, unknown>;
  visitor(record);
  for (const item of Object.values(record)) {
    walkObjects(item, visitor);
  }
}

function firstString(record: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function compactText(text: string, maxChars: number): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function mediaMimeType(url: string): string | undefined {
  const clean = url.split(/[?#]/u, 1)[0]?.toLowerCase() ?? "";
  if (clean.endsWith(".png")) return "image/png";
  if (clean.endsWith(".jpg") || clean.endsWith(".jpeg")) return "image/jpeg";
  if (clean.endsWith(".webp")) return "image/webp";
  if (clean.endsWith(".gif")) return "image/gif";
  if (clean.endsWith(".avif")) return "image/avif";
  if (clean.endsWith(".svg")) return "image/svg+xml";
  if (clean.endsWith(".mp4")) return "video/mp4";
  if (clean.endsWith(".webm")) return "video/webm";
  if (clean.endsWith(".mp3")) return "audio/mpeg";
  if (clean.endsWith(".wav")) return "audio/wav";
  return undefined;
}

function friendlyError(error: unknown): string {
  if (error instanceof Error) {
    return error.name === "AbortError" ? `Web search timed out after ${WEBSEARCH_TIMEOUT_MS}ms.` : error.message;
  }
  return String(error);
}
