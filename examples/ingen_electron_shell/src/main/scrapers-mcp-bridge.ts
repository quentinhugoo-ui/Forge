import { Buffer } from "node:buffer";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash } from "node:crypto";
import type {
  ScrapersArtifactRef,
  ScrapersBackendId,
  ScrapersBridgeResult,
  ScrapersCodeActRequest,
  ScrapersProviderResult,
  ScrapersStatus
} from "./scrapers-codeact.js";
import { SCRAPERS_COMMAND, SCRAPERS_RESULT_SCHEMA, stableScrapersHash } from "./scrapers-codeact.js";

const DEFAULT_MCP_PROTOCOL_VERSION = "2024-11-05";
const MAX_MARKDOWN_CHARS = 16_000;
const MAX_RAW_PREVIEW_CHARS = 8_000;

interface JsonRpcMessage {
  jsonrpc?: "2.0";
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code?: number; message?: string; data?: unknown };
}

interface McpTool {
  name: string;
  description?: string;
  inputSchema?: {
    type?: string;
    properties?: Record<string, unknown>;
    required?: string[];
  };
}

interface McpToolCallPlan {
  name: string;
  arguments: Record<string, unknown>;
}

interface NormalizedProviderData {
  urls: string[];
  fields: unknown[];
  markdown: string[];
  links: unknown[];
  media: unknown[];
  artifacts: ScrapersArtifactRef[];
  rawPreview?: unknown;
}

interface TimedFetchResult {
  endpoint: string;
  statusCode: number;
  body: unknown;
}

export async function runScrapersMcpBridge(request: ScrapersCodeActRequest): Promise<ScrapersBridgeResult> {
  const startedAt = new Date().toISOString();
  const started = Date.now();
  const providerResults = await Promise.all(
    request.backends.map((backend) => backend === "scrapling" ? runScraplingProvider(request) : runCrawl4AiProvider(request))
  );
  const merged = mergeProviderResults(request, providerResults);
  const okCount = providerResults.filter((provider) => provider.status === "ok").length;
  const status: ScrapersStatus = okCount === providerResults.length ? "ok" : okCount > 0 ? "partial" : "error";
  const finishedAt = new Date().toISOString();
  const result: ScrapersBridgeResult = {
    schema: SCRAPERS_RESULT_SCHEMA,
    command: SCRAPERS_COMMAND,
    status,
    requestHash: request.proofHash,
    startedAt,
    finishedAt,
    durationMs: Date.now() - started,
    providers: providerResults,
    merged,
    proofHash: ""
  };
  result.proofHash = stableScrapersHash({ ...result, proofHash: "" });
  return result;
}

async function runScraplingProvider(request: ScrapersCodeActRequest): Promise<ScrapersProviderResult> {
  const started = Date.now();
  const command = process.env.INGEN_SCRAPLING_MCP_COMMAND || "scrapling";
  const args = splitCommandArgs(process.env.INGEN_SCRAPLING_MCP_ARGS || "mcp");
  const client = new StdioMcpClient(command, args, request.limits.timeoutMs);
  try {
    await client.start();
    await client.request("initialize", {
      protocolVersion: process.env.INGEN_MCP_PROTOCOL_VERSION || DEFAULT_MCP_PROTOCOL_VERSION,
      capabilities: {},
      clientInfo: {
        name: "InGen Scrapers Bridge",
        version: "0.1.0"
      }
    });
    client.notify("notifications/initialized", {});
    const tools = readMcpTools(await client.request("tools/list", {}));
    const plans = buildScraplingToolPlans(request, tools);
    if (plans.length === 0) {
      return providerError("scrapling", "mcp_stdio", started, "Scrapling MCP is running but no compatible tool was listed.", [
        `command=${command} ${args.join(" ")}`,
        `tools=${tools.map((tool) => tool.name).join(",") || "none"}`
      ]);
    }
    const responses: unknown[] = [];
    for (const plan of plans) {
      responses.push(await client.request("tools/call", { name: plan.name, arguments: plan.arguments }));
    }
    const data = mergeNormalizedData(responses.map((response) => normalizeMcpResponse("scrapling", response, request)));
    return providerSuccess("scrapling", "mcp_stdio", started, data, {
      tool: plans.map((plan) => plan.name).join("+"),
      warnings: plans.length > 1 ? ["Scrapling executed extraction and artifact calls in one bounded MCP session."] : []
    });
  } catch (error) {
    return providerError("scrapling", "mcp_stdio", started, friendlyError(error), [
      `Start Scrapling MCP with: ${command} ${args.join(" ")}`.trim(),
      "Install/configure Scrapling so the command is available on PATH, or set INGEN_SCRAPLING_MCP_COMMAND and INGEN_SCRAPLING_MCP_ARGS."
    ]);
  } finally {
    client.close();
  }
}

async function runCrawl4AiProvider(request: ScrapersCodeActRequest): Promise<ScrapersProviderResult> {
  const started = Date.now();
  const baseUrl = (process.env.INGEN_CRAWL4AI_BASE_URL || "http://localhost:11235").replace(/\/+$/u, "");
  const warnings: string[] = [];
  try {
    const primary = await callCrawl4AiPrimary(baseUrl, request);
    const artifactResponses = await callCrawl4AiArtifacts(baseUrl, request, warnings);
    const data = mergeNormalizedData([
      normalizeCrawl4AiResponse(primary.body, request),
      ...artifactResponses.map((response) => normalizeCrawl4AiResponse(response.body, request, response.endpoint))
    ]);
    return providerSuccess("crawl4ai", "crawl4ai_http", started, data, {
      endpoint: [primary.endpoint, ...artifactResponses.map((response) => response.endpoint)].join("+"),
      warnings
    });
  } catch (error) {
    return providerError("crawl4ai", "crawl4ai_http", started, friendlyError(error), [
      `Expected Crawl4AI self-hosted server at ${baseUrl}.`,
      "Start Crawl4AI Docker/self-hosting with MCP/REST enabled, or set INGEN_CRAWL4AI_BASE_URL."
    ]);
  }
}

function buildScraplingToolPlans(request: ScrapersCodeActRequest, tools: McpTool[]): McpToolCallPlan[] {
  const plans: McpToolCallPlan[] = [];
  const primaryName = chooseScraplingPrimaryTool(request, tools);
  if (primaryName) {
    plans.push({
      name: primaryName,
      arguments: filterToolArguments(findTool(tools, primaryName), buildScraplingArguments(request, primaryName))
    });
  }
  const screenshotName = hasScreenshotArtifact(request) ? findTool(tools, "screenshot")?.name : undefined;
  if (screenshotName && screenshotName !== primaryName) {
    plans.push({
      name: screenshotName,
      arguments: filterToolArguments(findTool(tools, screenshotName), buildScraplingScreenshotArguments(request))
    });
  }
  return plans;
}

function chooseScraplingPrimaryTool(request: ScrapersCodeActRequest, tools: McpTool[]): string | undefined {
  if (request.extractionMode === "screenshot") {
    return findTool(tools, "screenshot")?.name;
  }
  const bulk = request.urls.length > 1;
  const dynamic = request.fetchMode === "dynamic" || request.fetchMode === "session" || request.waitUntil !== "dom_ready";
  const desired = request.fetchMode === "stealth"
    ? bulk ? "bulk_stealthy_fetch" : "stealthy_fetch"
    : dynamic
      ? bulk ? "bulk_fetch" : "fetch"
      : request.fetchMode === "http"
        ? bulk ? "bulk_get" : "get"
        : bulk ? "bulk_fetch" : "fetch";
  return findTool(tools, desired)?.name
    ?? findTool(tools, bulk ? "bulk_fetch" : "fetch")?.name
    ?? findTool(tools, bulk ? "bulk_get" : "get")?.name
    ?? findTool(tools, "stealthy_fetch")?.name;
}

function buildScraplingArguments(request: ScrapersCodeActRequest, toolName: string): Record<string, unknown> {
  const bulk = toolName.startsWith("bulk_") || request.urls.length > 1;
  const selectors = request.selectors.length > 0
    ? request.selectors
    : request.fieldsSchema.map((field) => field.selector).filter((selector): selector is string => Boolean(selector));
  const args: Record<string, unknown> = {
    url: request.urls[0],
    urls: request.urls,
    timeout: request.limits.timeoutMs,
    wait: request.waitUntil !== "dom_ready",
    wait_selector: request.waitSelector || undefined,
    network_idle: request.waitUntil === "network_idle",
    extraction_type: selectors.length > 0 ? "css" : "text",
    css_selector: selectors[0],
    selectors,
    fields_schema: request.fieldsSchema,
    main_content_only: request.extractionMode !== "archive_snapshot",
    session_id: request.session || undefined,
    session: request.session || undefined
  };
  if (!bulk) {
    delete args.urls;
  }
  return dropUndefined(args);
}

function buildScraplingScreenshotArguments(request: ScrapersCodeActRequest): Record<string, unknown> {
  return dropUndefined({
    url: request.urls[0],
    image_type: request.artifactFormat === "jpeg" ? "jpeg" : "png",
    full_page: request.artifacts === "full_page_screenshot",
    quality: request.artifactFormat === "jpeg" ? 85 : undefined,
    wait: request.waitUntil !== "dom_ready",
    wait_selector: request.waitSelector || undefined,
    network_idle: request.waitUntil === "network_idle",
    timeout: request.limits.timeoutMs,
    session_id: request.session || undefined,
    session: request.session || undefined
  });
}

function filterToolArguments(tool: McpTool | undefined, args: Record<string, unknown>): Record<string, unknown> {
  const properties = tool?.inputSchema?.properties;
  if (!properties || Object.keys(properties).length === 0) {
    return args;
  }
  const filtered: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    if (Object.prototype.hasOwnProperty.call(properties, key)) {
      filtered[key] = value;
    }
  }
  if (Object.keys(filtered).length === 0) {
    if (Array.isArray(args.urls)) {
      filtered.urls = args.urls;
    }
    if (typeof args.url === "string") {
      filtered.url = args.url;
    }
  }
  return filtered;
}

async function callCrawl4AiPrimary(baseUrl: string, request: ScrapersCodeActRequest): Promise<TimedFetchResult> {
  const candidates = [
    {
      endpoint: `${baseUrl}/crawl`,
      body: buildCrawl4AiCrawlPayload(request)
    },
    ...(request.urls.length === 1
      ? [
        {
          endpoint: `${baseUrl}/md`,
          body: {
            url: request.urls[0],
            query: request.markdownQuery || request.goal,
            screenshot: hasScreenshotArtifact(request),
            pdf: request.artifacts === "pdf"
          }
        },
        {
          endpoint: `${baseUrl}/html`,
          body: {
            url: request.urls[0],
            wait_for: request.waitSelector || undefined
          }
        }
      ]
      : [])
  ];
  const failures: string[] = [];
  for (const candidate of candidates) {
    try {
      const response = await fetchJson(candidate.endpoint, candidate.body, request.limits.timeoutMs);
      const taskId = readTaskId(response.body);
      if (taskId) {
        return await pollCrawl4AiTask(baseUrl, taskId, request.limits.timeoutMs, candidate.endpoint);
      }
      return response;
    } catch (error) {
      failures.push(`${candidate.endpoint}: ${friendlyError(error)}`);
    }
  }
  throw new Error(failures.join(" | "));
}

async function callCrawl4AiArtifacts(
  baseUrl: string,
  request: ScrapersCodeActRequest,
  warnings: string[]
): Promise<TimedFetchResult[]> {
  const artifactCalls: Array<{ endpoint: string; body: Record<string, unknown> }> = [];
  if (hasScreenshotArtifact(request)) {
    artifactCalls.push({
      endpoint: `${baseUrl}/screenshot`,
      body: {
        url: request.urls[0],
        full_page: request.artifacts === "full_page_screenshot",
        wait_for: request.waitSelector || undefined
      }
    });
  }
  if (request.artifacts === "pdf") {
    artifactCalls.push({
      endpoint: `${baseUrl}/pdf`,
      body: {
        url: request.urls[0],
        wait_for: request.waitSelector || undefined
      }
    });
  }
  if (request.artifacts === "mhtml") {
    warnings.push("Crawl4AI MHTML capture is requested; this bridge records it when present in crawl results but does not assume a direct /mhtml endpoint.");
  }
  const responses: TimedFetchResult[] = [];
  for (const call of artifactCalls) {
    try {
      responses.push(await fetchJson(call.endpoint, call.body, request.limits.timeoutMs));
    } catch (error) {
      warnings.push(`${call.endpoint} failed: ${friendlyError(error)}`);
    }
  }
  return responses;
}

function buildCrawl4AiCrawlPayload(request: ScrapersCodeActRequest): Record<string, unknown> {
  return dropUndefined({
    urls: request.urls.slice(0, request.limits.pages),
    priority: 10,
    crawler_config: dropUndefined({
      wait_for: request.waitSelector || undefined,
      page_timeout: request.limits.timeoutMs,
      scan_full_page: request.waitUntil === "images_loaded" || request.waitUntil === "virtual_scroll_complete",
      magic: request.fetchMode === "dynamic" || request.fetchMode === "auto",
      screenshot: hasScreenshotArtifact(request),
      pdf: request.artifacts === "pdf",
      verbose: false
    }),
    markdown_generator_config: dropUndefined({
      query: request.markdownQuery || request.goal,
      content_filter: request.contentFilter,
      fit_markdown: request.contentFilter.includes("fit_markdown")
    }),
    extraction_config: request.fieldsSchema.length > 0
      ? {
        type: "json_css",
        schema: {
          name: "ingen_scrapers_fields",
          baseSelector: "body",
          fields: request.fieldsSchema.map((field) => ({
            name: field.name,
            selector: field.selector,
            type: field.attr && field.attr !== "text" ? "attribute" : "text",
            attribute: field.attr && field.attr !== "text" ? field.attr : undefined
          }))
        }
      }
      : undefined,
    session_id: request.session || undefined,
    max_depth: crawlDepthToMaxDepth(request.crawlDepth),
    max_pages: request.limits.pages,
    include_links: request.links !== "none",
    include_media: request.media !== "none"
  });
}

async function pollCrawl4AiTask(
  baseUrl: string,
  taskId: string,
  timeoutMs: number,
  submittedEndpoint: string
): Promise<TimedFetchResult> {
  const started = Date.now();
  let lastBody: unknown;
  while (Date.now() - started < timeoutMs) {
    await delay(900);
    const response = await fetchJson(`${baseUrl}/task/${encodeURIComponent(taskId)}`, undefined, Math.min(10_000, timeoutMs));
    lastBody = response.body;
    const status = readStatus(response.body);
    if (!status || ["completed", "done", "ok", "success", "failed", "error"].includes(status)) {
      return { ...response, endpoint: `${submittedEndpoint}->/task/${taskId}` };
    }
  }
  throw new Error(`Crawl4AI task ${taskId} timed out after ${timeoutMs}ms; last_status=${readStatus(lastBody) || "unknown"}`);
}

async function fetchJson(endpoint: string, body: unknown, timeoutMs: number): Promise<TimedFetchResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const headers: Record<string, string> = {
      accept: "application/json"
    };
    if (body !== undefined) {
      headers["content-type"] = "application/json";
    }
    if (process.env.INGEN_CRAWL4AI_API_TOKEN) {
      headers.authorization = `Bearer ${process.env.INGEN_CRAWL4AI_API_TOKEN}`;
    }
    const response = await fetch(endpoint, {
      method: body === undefined ? "GET" : "POST",
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: controller.signal
    });
    const raw = await response.text();
    const parsed = parseMaybeJson(raw);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText}: ${compactText(raw, 700)}`);
    }
    return {
      endpoint,
      statusCode: response.status,
      body: parsed
    };
  } finally {
    clearTimeout(timeout);
  }
}

function normalizeMcpResponse(
  backend: ScrapersBackendId,
  response: unknown,
  request: ScrapersCodeActRequest
): NormalizedProviderData {
  const artifacts: ScrapersArtifactRef[] = [];
  const fields: unknown[] = [];
  const markdown: string[] = [];
  const values: unknown[] = [];
  const content = readRecord(response)?.content;
  if (Array.isArray(content)) {
    for (const item of content) {
      const record = readRecord(item);
      if (!record) continue;
      if (record.type === "text" && typeof record.text === "string") {
        markdown.push(compactText(record.text, MAX_MARKDOWN_CHARS));
        const parsed = parseMaybeJson(record.text);
        if (parsed !== record.text) {
          values.push(parsed);
        }
      } else if (record.type === "image" && typeof record.data === "string") {
        artifacts.push(binaryArtifactRef(backend, "image", record.data, String(record.mimeType ?? "image/png")));
      } else if (typeof record.uri === "string") {
        artifacts.push({ backend, kind: "resource", url: record.uri });
      } else {
        values.push(record);
      }
    }
  }
  const structured = readRecord(response)?.structuredContent ?? readRecord(response)?.structured_content;
  if (structured !== undefined) {
    values.push(structured);
    fields.push(structured);
  }
  if (values.length === 0) {
    values.push(response);
  }
  const discovered = collectEvidence(values, request);
  return {
    urls: request.urls,
    fields: fields.length > 0 ? fields : discovered.fields,
    markdown,
    links: discovered.links,
    media: discovered.media,
    artifacts: [...artifacts, ...discovered.artifacts],
    rawPreview: sanitizePreview(response)
  };
}

function normalizeCrawl4AiResponse(response: unknown, request: ScrapersCodeActRequest, endpoint?: string): NormalizedProviderData {
  const records = flattenCrawlResults(response);
  const markdown: string[] = [];
  const fields: unknown[] = [];
  const artifacts: ScrapersArtifactRef[] = [];
  const values: unknown[] = [];
  for (const record of records) {
    values.push(record);
    const markdownValue = readMarkdown(record);
    if (markdownValue) {
      markdown.push(markdownValue);
    }
    const extracted = readRecord(record)?.extracted_content ?? readRecord(record)?.extractedContent;
    if (extracted !== undefined) {
      fields.push(parseMaybeJson(String(extracted)));
    }
    for (const key of ["screenshot", "pdf", "mhtml", "downloaded_file", "downloaded_files"]) {
      const value = readRecord(record)?.[key];
      if (typeof value === "string" && value.length > 200) {
        artifacts.push(binaryArtifactRef("crawl4ai", key, value, artifactMimeType(key)));
      }
    }
  }
  const discovered = collectEvidence(values.length > 0 ? values : [response], request);
  return {
    urls: discovered.urls.length > 0 ? discovered.urls : request.urls,
    fields: fields.length > 0 ? fields : discovered.fields,
    markdown,
    links: discovered.links,
    media: discovered.media,
    artifacts: [...artifacts, ...discovered.artifacts.map((artifact) => ({ ...artifact, backend: "crawl4ai" as const }))],
    rawPreview: sanitizePreview({ endpoint, response })
  };
}

function flattenCrawlResults(value: unknown): unknown[] {
  const root = readRecord(value);
  if (!root) return [value];
  for (const key of ["results", "result", "data", "items"]) {
    const item = root[key];
    if (Array.isArray(item)) return item;
    if (item && typeof item === "object") return [item];
  }
  return [value];
}

function readMarkdown(value: unknown): string {
  const record = readRecord(value);
  if (!record) {
    return typeof value === "string" ? compactText(value, MAX_MARKDOWN_CHARS) : "";
  }
  const markdown = record.markdown;
  if (typeof markdown === "string") {
    return compactText(markdown, MAX_MARKDOWN_CHARS);
  }
  const markdownRecord = readRecord(markdown);
  if (markdownRecord) {
    const preferred = markdownRecord.fit_markdown ?? markdownRecord.markdown_with_citations ?? markdownRecord.raw_markdown;
    if (typeof preferred === "string") {
      return compactText(preferred, MAX_MARKDOWN_CHARS);
    }
  }
  for (const key of ["fit_markdown", "markdown_with_citations", "cleaned_html", "html"]) {
    if (typeof record[key] === "string") {
      return compactText(String(record[key]), MAX_MARKDOWN_CHARS);
    }
  }
  return "";
}

function collectEvidence(values: unknown[], request: ScrapersCodeActRequest): NormalizedProviderData {
  const urls = new Set<string>();
  const links: unknown[] = [];
  const media: unknown[] = [];
  const fields: unknown[] = [];
  const artifacts: ScrapersArtifactRef[] = [];

  const visit = (value: unknown, path: string[]) => {
    if (value === null || value === undefined) return;
    if (typeof value === "string") {
      collectStringEvidence(value, path, urls, links, media, artifacts, request);
      return;
    }
    if (Array.isArray(value)) {
      for (const item of value) visit(item, path);
      return;
    }
    if (typeof value !== "object") return;
    const record = value as Record<string, unknown>;
    const mediaRecord = normalizeMediaRecord(record, path, request);
    if (mediaRecord) {
      media.push(mediaRecord);
    }
    const keys = Object.keys(record);
    if (keys.some((key) => ["selector", "value", "name", "text", "attr"].includes(key))) {
      fields.push(sanitizePreview(record));
    }
    for (const [key, child] of Object.entries(record)) {
      visit(child, [...path, key]);
    }
  };

  values.forEach((value) => visit(value, []));
  return {
    urls: Array.from(urls).slice(0, request.limits.pages),
    fields: dedupeJson(fields).slice(0, 80),
    markdown: [],
    links: dedupeJson(links).slice(0, request.limits.links),
    media: dedupeJson(media).slice(0, request.limits.images),
    artifacts: artifacts.slice(0, 12)
  };
}

function collectStringEvidence(
  value: string,
  path: string[],
  urls: Set<string>,
  links: unknown[],
  media: unknown[],
  artifacts: ScrapersArtifactRef[],
  request: ScrapersCodeActRequest
): void {
  const key = path.at(-1)?.toLowerCase() ?? "";
  if (looksLikeBase64Artifact(key, value)) {
    artifacts.push(binaryArtifactRef("crawl4ai", key, value, artifactMimeType(key)));
    return;
  }
  for (const url of extractUrls(value)) {
    urls.add(url);
    const sourcePath = path.join(".") || "text";
    if (isMediaKey(key) || isMediaUrl(url)) {
      const item = {
        kind: mediaKindForUrl(url, key),
        url,
        mimeType: mediaMimeType(url),
        source_path: sourcePath
      };
      if (request.media !== "none") media.push(item);
    } else if (request.links !== "none") {
      const item = {
        url,
        source_path: sourcePath
      };
      links.push(item);
    }
  }
}

function mergeProviderResults(
  request: ScrapersCodeActRequest,
  providers: ScrapersProviderResult[]
): ScrapersBridgeResult["merged"] {
  const warnings = providers.flatMap((provider) => provider.warnings);
  const urls = uniqueStrings([...request.urls, ...providers.flatMap((provider) => provider.data.urls)]);
  const fields = dedupeJson(providers.flatMap((provider) => provider.data.fields)).slice(0, 100);
  const markdown = compactMarkdownChunks(providers.flatMap((provider) => provider.data.markdown));
  const links = dedupeJson(providers.flatMap((provider) => provider.data.links)).slice(0, request.limits.links);
  const media = dedupeJson(providers.flatMap((provider) => provider.data.media)).slice(0, request.limits.images);
  const artifacts = dedupeArtifacts(providers.flatMap((provider) => provider.data.artifacts));
  const provenance = providers.map((provider) => ({
    backend: provider.backend,
    status: provider.status,
    transport: provider.transport,
    tool: provider.tool,
    endpoint: provider.endpoint,
    duration_ms: provider.durationMs,
    counts: provider.counts,
    error: provider.error
  }));
  return {
    urls,
    fields,
    markdown,
    links,
    media,
    artifacts,
    warnings,
    provenance
  };
}

function providerSuccess(
  backend: ScrapersBackendId,
  transport: ScrapersProviderResult["transport"],
  started: number,
  data: NormalizedProviderData,
  options: { tool?: string; endpoint?: string; warnings?: string[] } = {}
): ScrapersProviderResult {
  return {
    backend,
    status: "ok",
    transport,
    durationMs: Date.now() - started,
    tool: options.tool,
    endpoint: options.endpoint,
    warnings: options.warnings ?? [],
    counts: countData(data),
    data
  };
}

function providerError(
  backend: ScrapersBackendId,
  transport: ScrapersProviderResult["transport"],
  started: number,
  error: string,
  warnings: string[] = []
): ScrapersProviderResult {
  const data: NormalizedProviderData = {
    urls: [],
    fields: [],
    markdown: [],
    links: [],
    media: [],
    artifacts: []
  };
  return {
    backend,
    status: "error",
    transport,
    durationMs: Date.now() - started,
    error,
    warnings,
    counts: countData(data),
    data
  };
}

function countData(data: NormalizedProviderData): ScrapersProviderResult["counts"] {
  return {
    urls: data.urls.length,
    fields: data.fields.length,
    markdownChars: data.markdown.reduce((sum, item) => sum + item.length, 0),
    links: data.links.length,
    media: data.media.length,
    artifacts: data.artifacts.length
  };
}

function mergeNormalizedData(items: NormalizedProviderData[]): NormalizedProviderData {
  return {
    urls: uniqueStrings(items.flatMap((item) => item.urls)),
    fields: dedupeJson(items.flatMap((item) => item.fields)),
    markdown: compactMarkdownChunks(items.flatMap((item) => item.markdown)),
    links: dedupeJson(items.flatMap((item) => item.links)),
    media: dedupeJson(items.flatMap((item) => item.media)),
    artifacts: dedupeArtifacts(items.flatMap((item) => item.artifacts)),
    rawPreview: items.map((item) => item.rawPreview).filter(Boolean)
  };
}

class StdioMcpClient {
  private child?: ChildProcessWithoutNullStreams;
  private nextId = 1;
  private stdoutBuffer = Buffer.alloc(0);
  private stderrTail = "";
  private pending = new Map<number, {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }>();

  constructor(
    private readonly command: string,
    private readonly args: string[],
    private readonly timeoutMs: number
  ) {}

  start(): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const child = spawn(this.command, this.args, {
        stdio: "pipe",
        windowsHide: true
      });
      this.child = child;
      child.stdout.on("data", (chunk: Buffer) => this.handleStdout(chunk));
      child.stderr.on("data", (chunk: Buffer) => {
        this.stderrTail = compactTail(`${this.stderrTail}${chunk.toString("utf8")}`, 2_000);
      });
      child.on("error", (error) => {
        this.rejectAll(error instanceof Error ? error : new Error(String(error)));
        if (!settled) {
          settled = true;
          reject(error);
        }
      });
      child.on("exit", (code, signal) => {
        const detail = this.stderrTail ? ` stderr=${this.stderrTail}` : "";
        this.rejectAll(new Error(`MCP process exited code=${code ?? "null"} signal=${signal ?? "null"}.${detail}`));
      });
      setTimeout(() => {
        if (!settled) {
          settled = true;
          resolve();
        }
      }, 0);
    });
  }

  request(method: string, params?: unknown): Promise<unknown> {
    const id = this.nextId++;
    const message: JsonRpcMessage = {
      jsonrpc: "2.0",
      id,
      method,
      params
    };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`MCP request timed out: ${method}`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.send(message);
    });
  }

  notify(method: string, params?: unknown): void {
    this.send({ jsonrpc: "2.0", method, params });
  }

  close(): void {
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(new Error(`MCP client closed before response id=${id}`));
    }
    this.pending.clear();
    const child = this.child;
    if (!child) return;
    try {
      child.stdin.end();
    } catch {
      // Best effort cleanup.
    }
    if (!child.killed) {
      setTimeout(() => {
        if (!child.killed) {
          child.kill();
        }
      }, 250);
    }
  }

  private send(message: JsonRpcMessage): void {
    if (!this.child || !this.child.stdin.writable) {
      throw new Error("MCP process is not writable.");
    }
    const body = Buffer.from(JSON.stringify(message), "utf8");
    const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
    this.child.stdin.write(Buffer.concat([header, body]));
  }

  private handleStdout(chunk: Buffer): void {
    this.stdoutBuffer = Buffer.concat([this.stdoutBuffer, chunk]);
    while (true) {
      const headerEnd = this.stdoutBuffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        return;
      }
      const header = this.stdoutBuffer.slice(0, headerEnd).toString("utf8");
      const lengthMatch = /content-length\s*:\s*(\d+)/iu.exec(header);
      if (!lengthMatch) {
        this.stdoutBuffer = this.stdoutBuffer.slice(headerEnd + 4);
        continue;
      }
      const length = Number(lengthMatch[1]);
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + length;
      if (this.stdoutBuffer.length < bodyEnd) {
        return;
      }
      const body = this.stdoutBuffer.slice(bodyStart, bodyEnd).toString("utf8");
      this.stdoutBuffer = this.stdoutBuffer.slice(bodyEnd);
      try {
        this.handleMessage(JSON.parse(body) as JsonRpcMessage);
      } catch (error) {
        this.rejectAll(new Error(`Invalid MCP JSON response: ${friendlyError(error)}`));
      }
    }
  }

  private handleMessage(message: JsonRpcMessage): void {
    if (typeof message.id !== "number") {
      return;
    }
    const pending = this.pending.get(message.id);
    if (!pending) {
      return;
    }
    this.pending.delete(message.id);
    clearTimeout(pending.timer);
    if (message.error) {
      pending.reject(new Error(message.error.message || `MCP error ${message.error.code ?? ""}`.trim()));
    } else {
      pending.resolve(message.result);
    }
  }

  private rejectAll(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

function readMcpTools(value: unknown): McpTool[] {
  const tools = readRecord(value)?.tools;
  if (!Array.isArray(tools)) {
    return [];
  }
  return tools
    .map((tool) => readRecord(tool))
    .filter((tool): tool is Record<string, unknown> => {
      if (!tool) return false;
      return typeof tool.name === "string";
    })
    .map((tool) => ({
      name: String(tool.name),
      description: typeof tool.description === "string" ? tool.description : undefined,
      inputSchema: readRecord(tool.inputSchema) as McpTool["inputSchema"]
    }));
}

function findTool(tools: McpTool[], name: string): McpTool | undefined {
  return tools.find((tool) => tool.name === name);
}

function hasScreenshotArtifact(request: ScrapersCodeActRequest): boolean {
  return request.extractionMode === "screenshot" ||
    request.artifacts === "screenshot" ||
    request.artifacts === "full_page_screenshot" ||
    request.imagePolicy === "screenshot_only";
}

function crawlDepthToMaxDepth(value: ScrapersCodeActRequest["crawlDepth"]): number {
  if (value === "same_domain_depth_2" || value === "docs_site_bounded") return 2;
  if (value === "same_domain_depth_1") return 1;
  return 0;
}

function readTaskId(value: unknown): string {
  const record = readRecord(value);
  const id = record?.task_id ?? record?.taskId ?? record?.id;
  return typeof id === "string" || typeof id === "number" ? String(id) : "";
}

function readStatus(value: unknown): string {
  const record = readRecord(value);
  const status = record?.status ?? record?.state;
  return typeof status === "string" ? status.toLowerCase() : "";
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function dropUndefined(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined && item !== ""));
}

function parseMaybeJson(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) {
    return value;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return value;
  }
}

function compactText(value: string, maxChars: number): string {
  const clean = value.replace(/\s+\n/g, "\n").replace(/\n{4,}/g, "\n\n\n").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 96)).trimEnd()}\n\n[truncated sha256:${sha256(clean)} chars:${clean.length}]`;
}

function compactTail(value: string, maxChars: number): string {
  return value.length <= maxChars ? value : value.slice(value.length - maxChars);
}

function sanitizePreview(value: unknown): unknown {
  if (value === null || value === undefined) return value;
  if (typeof value === "string") {
    if (value.length > 200 && looksLikeBase64Artifact("", value)) {
      return { binary: true, bytes_estimate: Math.floor(value.length * 0.75), sha256: sha256(value) };
    }
    return compactText(value, MAX_RAW_PREVIEW_CHARS);
  }
  if (Array.isArray(value)) {
    return value.slice(0, 20).map((item) => sanitizePreview(item));
  }
  if (typeof value === "object") {
    const result: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value as Record<string, unknown>).slice(0, 80)) {
      result[key] = sanitizePreview(item);
    }
    return result;
  }
  return value;
}

function splitCommandArgs(value: string): string[] {
  const args: string[] = [];
  const regex = /"([^"]*)"|'([^']*)'|(\S+)/gu;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(value)) !== null) {
    args.push(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return args.filter(Boolean);
}

function extractUrls(value: string): string[] {
  const urls: string[] = [];
  const matches = value.matchAll(/https?:\/\/[^\s"'<>),]+/giu);
  for (const match of matches) {
    try {
      urls.push(new URL(match[0]).toString());
    } catch {
      continue;
    }
  }
  return urls;
}

function isMediaKey(key: string): boolean {
  return /\b(?:image|img|src|srcset|media|thumbnail|poster|video|audio|sound)\b/iu.test(key);
}

function isMediaUrl(value: string): boolean {
  return /\.(?:png|jpe?g|webp|gif|avif|svg|mp4|webm|mov|m4v|mp3|wav|ogg|flac|m4a)(?:[?#].*)?$/iu.test(value);
}

function normalizeMediaRecord(
  record: Record<string, unknown>,
  path: string[],
  request: ScrapersCodeActRequest
): Record<string, unknown> | undefined {
  if (request.media === "none") {
    return undefined;
  }
  const sourcePath = path.join(".");
  const pathKey = sourcePath.toLowerCase();
  const url = firstRecordString(record, [
    "src",
    "url",
    "href",
    "image_url",
    "imageUrl",
    "video_url",
    "videoUrl",
    "audio_url",
    "audioUrl",
    "poster",
    "thumbnail",
    "thumbnail_url",
    "thumbnailUrl"
  ]);
  if (!url || !/^https?:\/\//iu.test(url)) {
    return undefined;
  }
  const kind = mediaKindForUrl(url, pathKey);
  if (!isMediaUrl(url) && kind === "page" && !/\b(?:media|image|images|audio|video|thumbnail|poster)\b/iu.test(pathKey)) {
    return undefined;
  }
  const item: Record<string, unknown> = {
    kind,
    url,
    mimeType: mediaMimeType(url),
    source_path: sourcePath || "record"
  };
  for (const [target, keys] of Object.entries({
    alt: ["alt", "alt_text", "altText"],
    title: ["title", "name"],
    caption: ["caption", "description", "desc"],
    thumbnailUrl: ["thumbnail_url", "thumbnailUrl", "thumbnail"],
    posterUrl: ["poster", "poster_url", "posterUrl"],
    sourceUrl: ["source_url", "sourceUrl", "source_website_url", "sourceWebsiteUrl", "page_url", "pageUrl"]
  })) {
    const value = firstRecordString(record, keys);
    if (value) {
      item[target] = value;
    }
  }
  for (const key of ["score", "width", "height", "duration", "size", "bytes"]) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) {
      item[key] = value;
    }
  }
  return item;
}

function firstRecordString(record: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function mediaKindForUrl(url: string, hint = ""): "image" | "video" | "audio" | "page" {
  const clean = url.split(/[?#]/u, 1)[0]?.toLowerCase() ?? "";
  if (/\b(?:audio|sound)\b/iu.test(hint) || /\.(?:mp3|wav|ogg|flac|m4a)$/iu.test(clean)) return "audio";
  if (/\bvideo\b/iu.test(hint) || /\.(?:mp4|webm|mov|m4v)$/iu.test(clean)) return "video";
  if (/\b(?:image|img|thumbnail|poster)\b/iu.test(hint) || /\.(?:png|jpe?g|webp|gif|avif|svg)$/iu.test(clean)) return "image";
  return "page";
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
  if (clean.endsWith(".mov")) return "video/quicktime";
  if (clean.endsWith(".mp3")) return "audio/mpeg";
  if (clean.endsWith(".wav")) return "audio/wav";
  if (clean.endsWith(".ogg")) return "audio/ogg";
  if (clean.endsWith(".flac")) return "audio/flac";
  return undefined;
}

function looksLikeBase64Artifact(key: string, value: string): boolean {
  if (value.length < 200) return false;
  if (/^data:[^;]+;base64,/iu.test(value)) return true;
  return /\b(?:screenshot|pdf|mhtml|image|downloaded)/iu.test(key) && /^[a-zA-Z0-9+/=\s]+$/u.test(value.slice(0, 500));
}

function binaryArtifactRef(backend: ScrapersBackendId, kind: string, value: string, mimeType: string): ScrapersArtifactRef {
  const payload = value.includes(",") && value.startsWith("data:") ? value.slice(value.indexOf(",") + 1) : value;
  return {
    backend,
    kind,
    mimeType,
    bytes: Math.floor(payload.replace(/\s+/gu, "").length * 0.75),
    sha256: sha256(payload),
    preview: value.startsWith("data:") ? value.slice(0, 80) : undefined
  };
}

function artifactMimeType(kind: string): string {
  if (/pdf/iu.test(kind)) return "application/pdf";
  if (/mhtml/iu.test(kind)) return "multipart/related";
  if (/jpeg|jpg/iu.test(kind)) return "image/jpeg";
  return "image/png";
}

function dedupeJson<T>(items: T[]): T[] {
  const seen = new Set<string>();
  const result: T[] = [];
  for (const item of items) {
    const key = stableScrapersHash(item);
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(item);
  }
  return result;
}

function dedupeArtifacts(items: ScrapersArtifactRef[]): ScrapersArtifactRef[] {
  const seen = new Set<string>();
  const result: ScrapersArtifactRef[] = [];
  for (const item of items) {
    const key = item.sha256 || item.url || `${item.backend}:${item.kind}:${item.preview ?? ""}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(item);
  }
  return result.slice(0, 20);
}

function compactMarkdownChunks(items: string[]): string[] {
  const result: string[] = [];
  let budget = MAX_MARKDOWN_CHARS;
  for (const item of items) {
    if (budget <= 0) break;
    const chunk = compactText(item, Math.min(budget, 8_000));
    if (!chunk) continue;
    result.push(chunk);
    budget -= chunk.length;
  }
  return result;
}

function uniqueStrings(items: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of items) {
    if (!item || seen.has(item)) continue;
    seen.add(item);
    result.push(item);
  }
  return result;
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function friendlyError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
