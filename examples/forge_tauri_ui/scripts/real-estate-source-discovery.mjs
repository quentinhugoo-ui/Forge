import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultRegistryPath = join(root, "source-registry", "real-estate-public-sources.json");
const defaultStorePath = join(root, ".forge-data");

const args = new Set(process.argv.slice(2));
const live = args.has("--live");
const pretty = args.has("--pretty");
const registryPath = argValue("--registry") ?? defaultRegistryPath;
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? defaultStorePath;
const timeoutMs = Number(argValue("--timeout-ms") ?? 12000);
const maxBytes = Number(argValue("--max-bytes") ?? 524288);
const maxResourceLinks = Number(argValue("--max-resource-links") ?? 80);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const manifestPath = join(dataDir, "source_manifest.jsonl");
const latestPath = join(dataDir, "source_discovery_latest.json");

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function fail(message) {
  console.error(`[real-estate-source-discovery] ${message}`);
  process.exit(1);
}

if (!existsSync(registryPath)) fail(`registry not found: ${registryPath}`);

const registry = JSON.parse(readFileSync(registryPath, "utf8"));
const parserMap = registry.parsers ?? {};
const parserNames = new Set(Object.keys(parserMap));
const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${registryPath}:${live}:${maxBytes}`).slice(0, 16);
const records = [];
const failures = [];

validateRegistry(registry);

for (const source of registry.sources ?? []) {
  for (const urlEntry of source.urls ?? []) {
    const record = live
      ? await discoverUrl(source, urlEntry)
      : plannedRecord(source, urlEntry);
    records.push(record);
  }
}

const summary = buildSummary();

mkdirSync(dataDir, { recursive: true });
writeFileSync(manifestPath, "");
for (const record of records) {
  appendFileSync(manifestPath, `${JSON.stringify(record)}\n`);
}
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));

if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function validateRegistry(value) {
  if (!value || typeof value !== "object") fail("registry root must be an object");
  if (!Array.isArray(value.sources)) fail("registry.sources must be an array");
  for (const source of value.sources) {
    if (!source.id) pushFailure("error", "source_missing_id", "unknown", "");
    if (!Array.isArray(source.urls) || source.urls.length === 0) {
      pushFailure("error", "source_without_urls", source.id ?? "unknown", "");
    }
  }
}

function plannedRecord(source, urlEntry) {
  const url = urlEntry.url ?? "";
  return recordBase(source, urlEntry, {
    status: "planned",
    finalUrl: url,
    contentType: "",
    contentLength: "",
    detectedFormat: detectFormat(url, "", Buffer.alloc(0)),
    previewHash: "",
    bodyHash: "",
    discoveredResources: [],
    parserProof: parserProof(urlEntry.parser, urlEntry.expectedFormats ?? []),
  });
}

async function discoverUrl(source, urlEntry) {
  const base = recordBase(source, urlEntry, {
    status: "pending",
    finalUrl: urlEntry.url ?? "",
    contentType: "",
    contentLength: "",
    detectedFormat: "",
    previewHash: "",
    bodyHash: "",
    discoveredResources: [],
    parserProof: parserProof(urlEntry.parser, urlEntry.expectedFormats ?? []),
  });
  if (!urlEntry.url) {
    pushFailure("error", "url_missing", source.id, "");
    return { ...base, status: "invalid" };
  }
  if (typeof fetch !== "function") {
    pushFailure("error", "fetch_unavailable", source.id, urlEntry.url);
    return { ...base, status: "fetch_unavailable" };
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(urlEntry.url, {
      method: urlEntry.method ?? "GET",
      redirect: "follow",
      signal: controller.signal,
      headers: {
        "user-agent": "ForgeSourceDiscovery/0.1 (+official metadata discovery; bounded preview)",
        accept: acceptHeader(urlEntry),
      },
    });
    const contentType = response.headers.get("content-type") ?? "";
    const contentLength = response.headers.get("content-length") ?? "";
    const bytes = await readPreviewBytes(response, maxBytes);
    const detectedFormat = detectFormat(response.url || urlEntry.url, contentType, bytes);
    const text = bytesToText(bytes, contentType, detectedFormat);
    const discoveredResources = discoverResources({
      source,
      parentUrl: response.url || urlEntry.url,
      detectedFormat,
      contentType,
      text,
    }).slice(0, maxResourceLinks);
    const status = response.ok ? "ok" : `http_${response.status}`;
    if (!response.ok) {
      pushFailure("warning", "url_http_non_ok", source.id, `${urlEntry.url} -> ${response.status}`);
    }
    return finalizeRecord({
      ...base,
      status,
      finalUrl: response.url || urlEntry.url,
      contentType,
      contentLength,
      detectedFormat,
      previewBytes: bytes.length,
      previewHash: sha256(bytes),
      bodyHash: sha256(bytes),
      discoveredResources,
      parserProof: parserProof(urlEntry.parser, urlEntry.expectedFormats ?? [], detectedFormat),
    });
  } catch (error) {
    const status = error?.name === "AbortError" ? "timeout" : "fetch_error";
    pushFailure("warning", status, source.id, `${urlEntry.url} -> ${error?.message ?? error}`);
    return finalizeRecord({
      ...base,
      status,
      error: String(error?.message ?? error),
    });
  } finally {
    clearTimeout(timer);
  }
}

function recordBase(source, urlEntry, payload) {
  const record = {
    kind: "real_estate_source_manifest",
    schemaVersion: 1,
    runId,
    discoveredAt: new Date().toISOString(),
    sourceId: source.id,
    sourceLabel: source.label,
    priority: source.priority,
    domain: source.domain,
    collector: source.collector,
    access: source.access,
    license: source.license,
    cadence: source.cadence,
    seedKind: urlEntry.kind ?? "unknown",
    seedUrl: urlEntry.url ?? "",
    method: urlEntry.method ?? "GET",
    expectedFormats: urlEntry.expectedFormats ?? [],
    expectedParser: urlEntry.parser ?? "",
    extract: source.extract ?? [],
    serves: source.serves ?? [],
    ...payload,
  };
  record.recordHash = sha256(JSON.stringify({ ...record, recordHash: "" }));
  return record;
}

function finalizeRecord(record) {
  record.recordHash = sha256(JSON.stringify({ ...record, recordHash: "" }));
  return record;
}

function discoverResources({ source, parentUrl, detectedFormat, contentType, text }) {
  if (!text) return [];
  if (detectedFormat === "json" || detectedFormat === "geojson" || contentType.toLowerCase().includes("json")) {
    try {
      const json = JSON.parse(text);
      return discoverJsonResources(json, parentUrl, source.id);
    } catch {
      return discoverTextUrls(text, parentUrl, source.id);
    }
  }
  if (detectedFormat === "html" || detectedFormat === "xml") {
    return discoverTextUrls(text, parentUrl, source.id);
  }
  return [];
}

function discoverJsonResources(value, parentUrl, sourceId) {
  const found = [];
  const seen = new Set();
  walkJson(value, (node, path) => {
    if (!node || typeof node !== "object" || Array.isArray(node)) return;
    const url = stringValue(node.url) ?? stringValue(node.latest) ?? stringValue(node.href) ?? stringValue(node.download_url);
    if (!url || !isHttpUrl(url)) return;
    const absoluteUrl = absolutize(url, parentUrl);
    if (seen.has(absoluteUrl)) return;
    seen.add(absoluteUrl);
    const format = normalizeFormat(
      stringValue(node.format)
        ?? stringValue(node.mime)
        ?? stringValue(node.mime_type)
        ?? detectFormat(absoluteUrl, stringValue(node.mime) ?? "", Buffer.alloc(0)),
    );
    found.push({
      sourceId,
      parentUrl,
      url: absoluteUrl,
      title: stringValue(node.title) ?? stringValue(node.name) ?? "",
      format,
      parser: parserForFormat(format),
      mime: stringValue(node.mime) ?? stringValue(node.mime_type) ?? "",
      filesize: numberValue(node.filesize) ?? numberValue(node.size) ?? null,
      checksum: stringValue(node.checksum?.value) ?? stringValue(node.checksum) ?? "",
      lastModified: stringValue(node.last_modified) ?? stringValue(node.modified) ?? "",
      jsonPath: path.join("."),
      resourceHash: sha256(`${sourceId}:${absoluteUrl}:${format}`),
    });
  });
  return found;
}

function discoverTextUrls(text, parentUrl, sourceId) {
  const found = [];
  const seen = new Set();
  const hrefRegex = /\b(?:href|src)=["']([^"']+)["']/gi;
  const nakedUrlRegex = /https?:\/\/[^\s"'<>]+/gi;
  for (const regex of [hrefRegex, nakedUrlRegex]) {
    let match;
    while ((match = regex.exec(text)) && found.length < maxResourceLinks) {
      const rawUrl = match[1] ?? match[0];
      const url = absolutize(rawUrl, parentUrl);
      if (!isHttpUrl(url) || seen.has(url)) continue;
      const format = detectFormat(url, "", Buffer.alloc(0));
      if (!isUsefulResourceUrl(url, format)) continue;
      seen.add(url);
      found.push({
        sourceId,
        parentUrl,
        url,
        title: "",
        format,
        parser: parserForFormat(format),
        mime: "",
        filesize: null,
        checksum: "",
        lastModified: "",
        jsonPath: "",
        resourceHash: sha256(`${sourceId}:${url}:${format}`),
      });
    }
  }
  return found;
}

function buildSummary() {
  const discoveredResourceCount = records.reduce((sum, record) => sum + (record.discoveredResources?.length ?? 0), 0);
  const statuses = {};
  const formats = {};
  for (const record of records) {
    statuses[record.status] = (statuses[record.status] ?? 0) + 1;
    if (record.detectedFormat) formats[record.detectedFormat] = (formats[record.detectedFormat] ?? 0) + 1;
    for (const resource of record.discoveredResources ?? []) {
      if (resource.format) formats[resource.format] = (formats[resource.format] ?? 0) + 1;
    }
  }
  const summary = {
    kind: "real_estate_source_discovery_summary",
    mode: live ? "live" : "plan_only",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    registryPath: registryPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    manifestPath: manifestPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    sourceCount: registry.sources?.length ?? 0,
    seedUrlCount: records.length,
    discoveredResourceCount,
    statuses,
    formats,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    mode: summary.mode,
    sourceCount: summary.sourceCount,
    seedUrlCount: summary.seedUrlCount,
    discoveredResourceCount: summary.discoveredResourceCount,
    statuses,
    formats,
    failures,
    records: records.map((record) => record.recordHash),
  }));
  return summary;
}

async function readPreviewBytes(response, limit) {
  const reader = response.body?.getReader?.();
  if (!reader) {
    const buffer = Buffer.from(await response.arrayBuffer());
    return buffer.subarray(0, limit);
  }
  const chunks = [];
  let size = 0;
  while (size < limit) {
    const { done, value } = await reader.read();
    if (done) break;
    const chunk = Buffer.from(value);
    const keep = Math.min(chunk.length, limit - size);
    chunks.push(chunk.subarray(0, keep));
    size += keep;
    if (keep < chunk.length) break;
  }
  try {
    await reader.cancel();
  } catch {
    // Bounded preview only; cancellation is best effort.
  }
  return Buffer.concat(chunks, size);
}

function walkJson(value, visit, path = []) {
  if (!value || typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, index) => walkJson(item, visit, path.concat(String(index))));
    return;
  }
  visit(value, path);
  for (const [key, child] of Object.entries(value)) {
    walkJson(child, visit, path.concat(key));
  }
}

function parserProof(expectedParser, expectedFormats, detectedFormat = "") {
  const missing = [];
  if (expectedParser && !parserNames.has(expectedParser)) missing.push(expectedParser);
  for (const format of expectedFormats) {
    if (!parserNames.has(format) && format !== "protobuf") missing.push(format);
  }
  const detectedParser = parserForFormat(detectedFormat);
  return {
    expectedParser,
    expectedFormats,
    detectedFormat,
    detectedParser,
    supported: missing.length === 0 && (!detectedFormat || detectedParser !== "unknown"),
    missing,
  };
}

function parserForFormat(format) {
  const normalized = normalizeFormat(format);
  return parserMap[normalized] ? normalized : "unknown";
}

function normalizeFormat(value) {
  const format = String(value ?? "").trim().toLowerCase();
  if (!format) return "unknown";
  if (format.includes("geo+json")) return "geojson";
  if (format.includes("json")) return "json";
  if (format.includes("pdf")) return "pdf";
  if (format.includes("csv") && format.includes("gzip")) return "csv.gz";
  if (format.includes("csv")) return "csv";
  if (format.includes("excel") || format.includes("xlsx")) return "xlsx";
  if (format.includes("xls")) return "xls";
  if (format.includes("parquet")) return "parquet";
  if (format.includes("xml")) return "xml";
  if (format.includes("zip")) return "zip";
  if (format.includes("html")) return "html";
  if (format.includes("kml")) return "xml";
  if (format.includes("geojson")) return "geojson";
  if (format.includes("shp") || format.includes("shapefile")) return "zip";
  if (format.includes("arcgis") || format.includes("rest api")) return "json";
  if (format === "url" || format === "web page" || format === "page web") return "html";
  if (format === "txt" || format.includes("plain")) return "txt";
  return format.replace(/^\./, "");
}

function detectFormat(url, contentType, bytes) {
  const lowerUrl = String(url ?? "").toLowerCase();
  const lowerType = String(contentType ?? "").toLowerCase();
  const parsed = safeUrl(lowerUrl);
  const path = parsed?.pathname ?? lowerUrl;
  if (lowerType.includes("application/geo+json")) return "geojson";
  if (lowerType.includes("application/json")) return "json";
  if (lowerType.includes("application/pdf") || bytes.subarray(0, 4).toString() === "%PDF") return "pdf";
  if (lowerType.includes("text/csv") || lowerUrl.endsWith(".csv")) return "csv";
  if (lowerUrl.endsWith(".csv.gz")) return "csv.gz";
  if (lowerUrl.endsWith(".xlsx")) return "xlsx";
  if (lowerUrl.endsWith(".xls")) return "xls";
  if (lowerUrl.endsWith(".parquet")) return "parquet";
  if (lowerUrl.endsWith(".zip")) return "zip";
  if (lowerUrl.endsWith(".geojson")) return "geojson";
  if (lowerType.includes("xml") || lowerUrl.endsWith(".xml") || lowerUrl.endsWith(".rss") || lowerUrl.endsWith(".kml")) return "xml";
  if (lowerType.includes("html")) return "html";
  if (lowerType.includes("text/plain") || lowerUrl.endsWith(".txt") || lowerUrl.endsWith(".md")) return "txt";
  if (path.includes("/api/") || path.endsWith("/api")) return "json";
  if (isMetadataPageUrl(lowerUrl)) return "html";
  return "unknown";
}

function acceptHeader(entry) {
  const formats = new Set(entry.expectedFormats ?? []);
  if (formats.has("json") || formats.has("geojson")) return "application/json, application/geo+json, */*;q=0.2";
  if (formats.has("pdf")) return "application/pdf, */*;q=0.2";
  if (formats.has("csv") || formats.has("txt")) return "text/csv, text/plain, */*;q=0.2";
  return "*/*";
}

function isUsefulResourceUrl(url, format) {
  const lower = url.toLowerCase();
  if (isDecorativeOrNavigationUrl(lower)) return false;
  if (["csv", "csv.gz", "txt", "xlsx", "xls", "parquet", "xml", "pdf", "zip", "json", "geojson"].includes(format)) {
    return true;
  }
  return isMetadataPageUrl(lower) || lower.includes("/api/") || lower.includes("opendata");
}

function isMetadataPageUrl(lowerUrl) {
  return lowerUrl.includes("data.gouv.fr/datasets/")
    || lowerUrl.includes("data.gouv.fr/dataservices/")
    || lowerUrl.includes("transport.data.gouv.fr/datasets/")
    || lowerUrl.includes("meteo.data.gouv.fr/")
    || lowerUrl.includes("echanges.dila.gouv.fr/opendata/");
}

function isDecorativeOrNavigationUrl(lowerUrl) {
  return lowerUrl.includes("~gitbook/image")
    || lowerUrl.includes("favicon")
    || lowerUrl.includes("logo")
    || lowerUrl.includes("support.data.gouv.fr")
    || lowerUrl.includes("forum.data.gouv.fr")
    || lowerUrl.includes("github.com/opendatateam/udata")
    || lowerUrl.endsWith(".png")
    || lowerUrl.endsWith(".jpg")
    || lowerUrl.endsWith(".jpeg")
    || lowerUrl.endsWith(".svg")
    || lowerUrl.endsWith(".webp")
    || lowerUrl.endsWith(".ico");
}

function isHttpUrl(value) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function absolutize(value, parentUrl) {
  try {
    return new URL(value, parentUrl).toString();
  } catch {
    return String(value ?? "");
  }
}

function safeUrl(value) {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

function bytesToText(bytes, contentType, detectedFormat) {
  if (!bytes.length) return "";
  if (["pdf", "zip", "xlsx", "xls", "parquet"].includes(detectedFormat)) return "";
  const type = contentType.toLowerCase();
  if (
    detectedFormat === "json"
    || detectedFormat === "geojson"
    || detectedFormat === "html"
    || detectedFormat === "xml"
    || detectedFormat === "csv"
    || detectedFormat === "txt"
    || type.includes("text")
    || type.includes("json")
    || type.includes("xml")
  ) {
    return bytes.toString("utf8");
  }
  return "";
}

function stringValue(value) {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function numberValue(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

function pushFailure(severity, code, sourceId, detail) {
  failures.push({ severity, code, sourceId, detail });
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
