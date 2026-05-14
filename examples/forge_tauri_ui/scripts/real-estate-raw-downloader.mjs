import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, extname, join } from "node:path";

const args = new Set(process.argv.slice(2));
const live = args.has("--live");
const pretty = args.has("--pretty");
const allowLarge = args.has("--allow-large");
const includeHtml = args.has("--include-html");
const manifestPath = argValue("--manifest");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH;
const timeoutMs = Number(argValue("--timeout-ms") ?? 12000);
const maxBytes = Number(argValue("--max-bytes") ?? 5 * 1024 * 1024);
const limit = Number(argValue("--limit") ?? 64);
const allowedFormats = new Set(
  (argValue("--formats") ?? "json,geojson,csv,csv.gz,txt,xlsx,xls,parquet,xml,pdf,zip")
    .split(",")
    .map((it) => it.trim().toLowerCase())
    .filter(Boolean),
);
if (includeHtml) allowedFormats.add("html");

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function fail(message) {
  console.error(`[real-estate-raw-downloader] ${message}`);
  process.exit(1);
}

if (!manifestPath) fail("missing --manifest=<source_manifest.jsonl>");
if (!existsSync(manifestPath)) fail(`manifest not found: ${manifestPath}`);

const resolvedStore = storePath ?? inferStorePathFromManifest(manifestPath);
const harvesterDir = join(resolvedStore, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const rawDir = join(harvesterDir, "raw");
const downloadsPath = join(dataDir, "raw_downloads.jsonl");
const latestPath = join(dataDir, "raw_downloader_latest.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${manifestPath}:${live}:${maxBytes}:${limit}`).slice(0, 16);
const records = readJsonl(manifestPath);
const resources = selectResources(records);
const failures = [];
const downloadRecords = [];

mkdirSync(dataDir, { recursive: true });
mkdirSync(rawDir, { recursive: true });

for (const resource of resources.slice(0, limit)) {
  const record = live ? await downloadResource(resource) : plannedRecord(resource);
  downloadRecords.push(record);
  appendFileSync(downloadsPath, `${JSON.stringify(record)}\n`);
}

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function readJsonl(path) {
  return readFileSync(path, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        pushFailure("error", "manifest_json_decode_failed", path, `line ${index + 1}: ${error.message}`);
        return null;
      }
    })
    .filter(Boolean);
}

function selectResources(sourceRecords) {
  const selected = [];
  const seen = new Set();
  for (const sourceRecord of sourceRecords) {
    for (const resource of sourceRecord.discoveredResources ?? []) {
      const format = normalizeFormat(resource.format);
      if (!resource.url || seen.has(resource.url)) continue;
      if (!allowedFormats.has(format)) continue;
      if (format === "html" && !includeHtml) continue;
      seen.add(resource.url);
      selected.push({
        runId,
        sourceManifestRunId: sourceRecord.runId,
        sourceRecordHash: sourceRecord.recordHash,
        sourceId: resource.sourceId ?? sourceRecord.sourceId,
        sourceLabel: sourceRecord.sourceLabel,
        sourceLicense: sourceRecord.license,
        sourceCadence: sourceRecord.cadence,
        sourcePriority: sourceRecord.priority,
        collector: sourceRecord.collector,
        parentUrl: resource.parentUrl,
        url: resource.url,
        title: resource.title ?? "",
        format,
        parser: resource.parser && resource.parser !== "unknown" ? resource.parser : parserForFormat(format),
        mime: resource.mime ?? "",
        expectedSize: resource.filesize ?? null,
        checksum: resource.checksum ?? "",
        lastModified: resource.lastModified ?? "",
        resourceHash: resource.resourceHash ?? sha256(`${sourceRecord.sourceId}:${resource.url}:${format}`),
      });
    }
  }
  return selected.sort((a, b) =>
    String(a.sourcePriority).localeCompare(String(b.sourcePriority))
    || a.sourceId.localeCompare(b.sourceId)
    || a.url.localeCompare(b.url)
  );
}

function plannedRecord(resource) {
  return finalizeRecord({
    kind: "real_estate_raw_download",
    schemaVersion: 1,
    runId,
    mode: "plan_only",
    downloadedAt: new Date().toISOString(),
    status: "planned",
    ...resource,
    contentType: "",
    contentLength: "",
    rawHash: "",
    rawBytes: 0,
    rawPath: "",
    cacheStatus: "not_checked",
    freshness: freshnessForCadence(resource.sourceCadence),
  });
}

async function downloadResource(resource) {
  const base = {
    kind: "real_estate_raw_download",
    schemaVersion: 1,
    runId,
    mode: "live",
    downloadedAt: new Date().toISOString(),
    ...resource,
    freshness: freshnessForCadence(resource.sourceCadence),
  };
  if (resource.expectedSize && resource.expectedSize > maxBytes && !allowLarge) {
    return finalizeRecord({
      ...base,
      status: "skipped_too_large_manifest",
      contentType: resource.mime ?? "",
      contentLength: String(resource.expectedSize),
      rawHash: "",
      rawBytes: 0,
      rawPath: "",
      cacheStatus: "skipped",
      note: `expected size ${resource.expectedSize} exceeds maxBytes ${maxBytes}`,
    });
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(resource.url, {
      method: "GET",
      redirect: "follow",
      signal: controller.signal,
      headers: {
        "user-agent": "ForgeRawDownloader/0.1 (+official public data; content-addressed cache)",
        accept: acceptHeader(resource.format),
      },
    });
    const contentType = response.headers.get("content-type") ?? "";
    const contentLength = response.headers.get("content-length") ?? "";
    const lengthNumber = Number(contentLength);
    if (Number.isFinite(lengthNumber) && lengthNumber > maxBytes && !allowLarge) {
      await cancelBody(response);
      return finalizeRecord({
        ...base,
        status: "skipped_too_large_header",
        contentType,
        contentLength,
        rawHash: "",
        rawBytes: 0,
        rawPath: "",
        cacheStatus: "skipped",
        note: `content-length ${contentLength} exceeds maxBytes ${maxBytes}`,
      });
    }
    if (!response.ok) {
      pushFailure("warning", "download_http_non_ok", resource.sourceId, `${resource.url} -> ${response.status}`);
      await cancelBody(response);
      return finalizeRecord({
        ...base,
        status: `http_${response.status}`,
        contentType,
        contentLength,
        rawHash: "",
        rawBytes: 0,
        rawPath: "",
        cacheStatus: "miss",
      });
    }
    const bytes = await readBoundedBytes(response, maxBytes, allowLarge);
    if (bytes.tooLarge) {
      return finalizeRecord({
        ...base,
        status: "skipped_too_large_stream",
        contentType,
        contentLength,
        rawHash: "",
        rawBytes: bytes.size,
        rawPath: "",
        cacheStatus: "skipped",
        note: `stream exceeded maxBytes ${maxBytes}`,
      });
    }
    const actualFormat = detectFormat(response.url || resource.url, contentType, bytes.buffer, resource.format);
    const rawHash = sha256(bytes.buffer);
    const rawPath = rawPathForHash(rawHash, actualFormat, resource.url);
    const cacheStatus = existsSync(rawPath) ? "hit" : "miss";
    if (cacheStatus === "miss") {
      mkdirSync(dirname(rawPath), { recursive: true });
      writeFileSync(rawPath, bytes.buffer);
    }
    return finalizeRecord({
      ...base,
      status: "ok",
      finalUrl: response.url || resource.url,
      contentType,
      contentLength,
      detectedFormat: actualFormat,
      rawHash,
      rawBytes: bytes.buffer.length,
      rawPath: rawPath.replaceAll("\\", "/"),
      cacheStatus,
    });
  } catch (error) {
    const status = error?.name === "AbortError" ? "timeout" : "fetch_error";
    pushFailure("warning", status, resource.sourceId, `${resource.url} -> ${error?.message ?? error}`);
    return finalizeRecord({
      ...base,
      status,
      contentType: "",
      contentLength: "",
      rawHash: "",
      rawBytes: 0,
      rawPath: "",
      cacheStatus: "miss",
      error: String(error?.message ?? error),
    });
  } finally {
    clearTimeout(timer);
  }
}

async function readBoundedBytes(response, max, allow) {
  const reader = response.body?.getReader?.();
  if (!reader) {
    const buffer = Buffer.from(await response.arrayBuffer());
    return { buffer, size: buffer.length, tooLarge: !allow && buffer.length > max };
  }
  const chunks = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    const chunk = Buffer.from(value);
    size += chunk.length;
    if (!allow && size > max) {
      try {
        await reader.cancel();
      } catch {
        // Best effort cancel.
      }
      return { buffer: Buffer.alloc(0), size, tooLarge: true };
    }
    chunks.push(chunk);
  }
  return { buffer: Buffer.concat(chunks, size), size, tooLarge: false };
}

async function cancelBody(response) {
  try {
    await response.body?.cancel?.();
  } catch {
    // Best effort.
  }
}

function buildSummary() {
  const statuses = {};
  const formats = {};
  const cacheStatuses = {};
  let rawBytes = 0;
  let downloaded = 0;
  let skipped = 0;
  for (const record of downloadRecords) {
    statuses[record.status] = (statuses[record.status] ?? 0) + 1;
    formats[record.format] = (formats[record.format] ?? 0) + 1;
    cacheStatuses[record.cacheStatus] = (cacheStatuses[record.cacheStatus] ?? 0) + 1;
    rawBytes += record.rawBytes ?? 0;
    if (record.status === "ok") downloaded += 1;
    if (record.status.startsWith("skipped")) skipped += 1;
  }
  const summary = {
    kind: "real_estate_raw_downloader_summary",
    mode: live ? "live" : "plan_only",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    manifestPath: manifestPath.replaceAll("\\", "/"),
    storePath: resolvedStore.replaceAll("\\", "/"),
    downloadsPath: downloadsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    selectedResourceCount: resources.length,
    processedResourceCount: downloadRecords.length,
    downloaded,
    skipped,
    rawBytes,
    maxBytes,
    allowLarge,
    formats,
    statuses,
    cacheStatuses,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    mode: summary.mode,
    selectedResourceCount: summary.selectedResourceCount,
    processedResourceCount: summary.processedResourceCount,
    downloaded,
    skipped,
    rawBytes,
    maxBytes,
    allowLarge,
    formats,
    statuses,
    cacheStatuses,
    failures,
    records: downloadRecords.map((record) => record.recordHash),
  }));
  return summary;
}

function rawPathForHash(hash, format, url) {
  const extension = extensionFor(format, url);
  return join(rawDir, hash.slice(0, 2), `${hash}${extension}`);
}

function extensionFor(format, url) {
  const ext = extname(new URL(url).pathname).toLowerCase();
  if (ext && ext.length <= 12) return ext;
  const map = {
    "csv.gz": ".csv.gz",
    geojson: ".geojson",
    json: ".json",
    csv: ".csv",
    txt: ".txt",
    xlsx: ".xlsx",
    xls: ".xls",
    parquet: ".parquet",
    xml: ".xml",
    pdf: ".pdf",
    zip: ".zip",
    html: ".html",
  };
  return map[format] ?? ".bin";
}

function detectFormat(url, contentType, bytes, fallback) {
  const lowerUrl = String(url ?? "").toLowerCase();
  const lowerType = String(contentType ?? "").toLowerCase();
  const head = bytes.subarray(0, 64).toString("utf8").trimStart().toLowerCase();
  if (lowerType.includes("application/geo+json")) return "geojson";
  if (lowerType.includes("application/json") || head.startsWith("{") || head.startsWith("[")) return "json";
  if (lowerType.includes("application/pdf") || bytes.subarray(0, 4).toString() === "%PDF") return "pdf";
  if (lowerType.includes("text/html") || head.startsWith("<!doctype") || head.startsWith("<html")) return "html";
  if (lowerType.includes("javascript") || lowerUrl.endsWith(".js") || head.startsWith("/*!") || head.startsWith("//")) return "txt";
  if (lowerType.includes("text/csv") || lowerUrl.endsWith(".csv")) return "csv";
  if (lowerUrl.endsWith(".csv.gz")) return "csv.gz";
  if (lowerUrl.endsWith(".geojson")) return "geojson";
  if (lowerUrl.endsWith(".xlsx")) return "xlsx";
  if (lowerUrl.endsWith(".xls")) return "xls";
  if (lowerUrl.endsWith(".parquet")) return "parquet";
  if (lowerUrl.endsWith(".zip")) return "zip";
  if (lowerType.includes("xml") || lowerUrl.endsWith(".xml") || lowerUrl.endsWith(".rss") || lowerUrl.endsWith(".kml")) return "xml";
  if (lowerType.includes("text/plain") || lowerUrl.endsWith(".txt") || lowerUrl.endsWith(".md")) return "txt";
  return normalizeFormat(fallback);
}

function parserForFormat(format) {
  const normalized = normalizeFormat(format);
  if (normalized === "csv.gz") return "csv.gz";
  return normalized;
}

function normalizeFormat(value) {
  const format = String(value ?? "").trim().toLowerCase();
  if (!format) return "unknown";
  if (format.includes("geo+json") || format.includes("geojson")) return "geojson";
  if (format.includes("json")) return "json";
  if (format.includes("pdf")) return "pdf";
  if (format.includes("csv") && format.includes("gzip")) return "csv.gz";
  if (format.includes("csv")) return "csv";
  if (format.includes("xlsx") || format.includes("excel")) return "xlsx";
  if (format.includes("xls")) return "xls";
  if (format.includes("parquet")) return "parquet";
  if (format.includes("xml") || format.includes("kml")) return "xml";
  if (format.includes("zip") || format.includes("shp")) return "zip";
  if (format.includes("html") || format === "url" || format === "web page") return "html";
  if (format === "txt" || format.includes("plain")) return "txt";
  return format.replace(/^\./, "");
}

function freshnessForCadence(cadence) {
  const value = String(cadence ?? "").toLowerCase();
  if (value.includes("hour")) return { cadence, staleAfterHours: 2 };
  if (value.includes("daily")) return { cadence, staleAfterHours: 36 };
  if (value.includes("week")) return { cadence, staleAfterHours: 24 * 10 };
  if (value.includes("month")) return { cadence, staleAfterHours: 24 * 45 };
  return { cadence, staleAfterHours: 24 * 7 };
}

function acceptHeader(format) {
  if (format === "json" || format === "geojson") return "application/json, application/geo+json, */*;q=0.2";
  if (format === "pdf") return "application/pdf, */*;q=0.2";
  if (format === "csv" || format === "txt") return "text/csv, text/plain, */*;q=0.2";
  if (format === "xml") return "application/xml, text/xml, */*;q=0.2";
  return "*/*";
}

function inferStorePathFromManifest(path) {
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/source_manifest.jsonl";
  if (normalized.endsWith(marker)) {
    return normalized.slice(0, -marker.length);
  }
  return dirname(dirname(dirname(path)));
}

function finalizeRecord(record) {
  record.recordHash = sha256(JSON.stringify({ ...record, recordHash: "" }));
  return record;
}

function pushFailure(severity, code, sourceId, detail) {
  failures.push({ severity, code, sourceId, detail });
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
