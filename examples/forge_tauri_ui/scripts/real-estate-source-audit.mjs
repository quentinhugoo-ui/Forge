import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultRegistryPath = join(root, "source-registry", "real-estate-public-sources.json");

const args = new Set(process.argv.slice(2));
const live = args.has("--live");
const pretty = args.has("--pretty");
const registryPath = argValue("--registry") ?? defaultRegistryPath;
const outputPath = argValue("--output");
const timeoutMs = Number(argValue("--timeout-ms") ?? 12000);
const maxBytes = Number(argValue("--max-bytes") ?? 65536);

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function fail(message) {
  console.error(`[real-estate-source-audit] ${message}`);
  process.exit(1);
}

if (!existsSync(registryPath)) fail(`registry not found: ${registryPath}`);

const registry = JSON.parse(readFileSync(registryPath, "utf8"));
const parserNames = new Set(Object.keys(registry.parsers ?? {}));
const sourceResults = [];
const failures = [];

validateRegistry(registry);

for (const source of registry.sources) {
  const urls = Array.isArray(source.urls) ? source.urls : [];
  const urlResults = [];
  for (const entry of urls) {
    const staticResult = validateUrlEntry(source, entry);
    if (!live) {
      urlResults.push(staticResult);
      continue;
    }
    urlResults.push(await probeUrl(source, entry, staticResult));
  }
  sourceResults.push({
    id: source.id,
    priority: source.priority,
    domain: source.domain,
    collector: source.collector,
    urlCount: urls.length,
    urls: urlResults,
  });
}

const manifest = {
  kind: "real_estate_source_registry_audit",
  mode: live ? "live" : "plan_only",
  generatedAt: new Date().toISOString(),
  registryPath: registryPath.replaceAll("\\", "/"),
  sourceCount: registry.sources.length,
  urlCount: sourceResults.reduce((sum, source) => sum + source.urlCount, 0),
  parserCount: parserNames.size,
  failures,
  sources: sourceResults,
};
manifest.proofHash = sha256(JSON.stringify({
  mode: manifest.mode,
  sourceCount: manifest.sourceCount,
  urlCount: manifest.urlCount,
  parserCount: manifest.parserCount,
  failures: manifest.failures,
  sources: manifest.sources,
}));

const text = pretty ? JSON.stringify(manifest, null, 2) : JSON.stringify(manifest);
if (outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${text}\n`);
}

if (pretty || !outputPath) console.log(text);

const hardFailures = failures.filter((failure) => failure.severity === "error");
if (hardFailures.length) process.exit(1);

function validateRegistry(value) {
  if (!value || typeof value !== "object") fail("registry JSON root must be an object");
  if (!Array.isArray(value.sources)) fail("registry.sources must be an array");
  if (!value.parsers || typeof value.parsers !== "object") fail("registry.parsers must be an object");
  const seen = new Set();
  for (const source of value.sources) {
    if (!source.id) pushFailure("error", "source_missing_id", "unknown", "");
    if (seen.has(source.id)) pushFailure("error", "source_duplicate_id", source.id, "");
    seen.add(source.id);
    if (!Array.isArray(source.urls) || source.urls.length === 0) {
      pushFailure("error", "source_without_urls", source.id, "");
    }
    if (!source.collector) pushFailure("warning", "source_without_collector", source.id, "");
    if (!source.priority) pushFailure("warning", "source_without_priority", source.id, "");
  }
}

function validateUrlEntry(source, entry) {
  const result = {
    kind: entry.kind ?? "unknown",
    url: entry.url ?? "",
    method: entry.method ?? "GET",
    expectedFormats: entry.expectedFormats ?? [],
    parser: entry.parser ?? "",
    status: "planned",
    detectedFormat: "",
    contentType: "",
    contentLength: "",
    previewHash: "",
    notes: [],
  };
  if (!entry.url) {
    pushFailure("error", "url_missing", source.id, "");
    result.status = "invalid";
    return result;
  }
  try {
    new URL(entry.url);
  } catch {
    pushFailure("error", "url_invalid", source.id, entry.url);
    result.status = "invalid";
  }
  if (!parserNames.has(result.parser)) {
    pushFailure("error", "parser_not_registered", source.id, `${entry.url} -> ${result.parser}`);
    result.notes.push("parser_not_registered");
  }
  for (const format of result.expectedFormats) {
    if (!parserNames.has(format) && !["protobuf"].includes(format)) {
      pushFailure("warning", "expected_format_without_parser", source.id, `${entry.url} -> ${format}`);
    }
  }
  return result;
}

async function probeUrl(source, entry, planned) {
  if (planned.status === "invalid") return planned;
  if (typeof fetch !== "function") {
    pushFailure("error", "fetch_unavailable", source.id, entry.url);
    return { ...planned, status: "fetch_unavailable" };
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(entry.url, {
      method: entry.method ?? "GET",
      redirect: "follow",
      signal: controller.signal,
      headers: {
        "user-agent": "ForgeSourceAudit/0.1 (+metadata-first source registry validation)",
        accept: acceptHeader(entry),
      },
    });
    const contentType = response.headers.get("content-type") ?? "";
    const contentLength = response.headers.get("content-length") ?? "";
    const bytes = await readPreviewBytes(response, maxBytes);
    const detectedFormat = detectFormat(entry.url, contentType, bytes);
    const status = response.ok ? "ok" : `http_${response.status}`;
    if (!response.ok) pushFailure("warning", "url_http_non_ok", source.id, `${entry.url} -> ${response.status}`);
    if (entry.expectedFormats?.length && !entry.expectedFormats.includes(detectedFormat)) {
      planned.notes.push(`detected_format:${detectedFormat}`);
    }
    return {
      ...planned,
      status,
      finalUrl: response.url,
      detectedFormat,
      contentType,
      contentLength,
      previewHash: sha256(bytes),
      previewBytes: bytes.length,
    };
  } catch (error) {
    const status = error?.name === "AbortError" ? "timeout" : "fetch_error";
    pushFailure("warning", status, source.id, `${entry.url} -> ${error?.message ?? error}`);
    return {
      ...planned,
      status,
      error: String(error?.message ?? error),
    };
  } finally {
    clearTimeout(timer);
  }
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
    // Best effort: the audit only needs a bounded preview.
  }
  return Buffer.concat(chunks, size);
}

function acceptHeader(entry) {
  const formats = new Set(entry.expectedFormats ?? []);
  if (formats.has("json") || formats.has("geojson")) return "application/json, application/geo+json, */*;q=0.2";
  if (formats.has("pdf")) return "application/pdf, */*;q=0.2";
  if (formats.has("csv")) return "text/csv, */*;q=0.2";
  return "*/*";
}

function detectFormat(url, contentType, bytes) {
  const lowerUrl = url.toLowerCase();
  const lowerType = contentType.toLowerCase();
  if (lowerType.includes("application/geo+json")) return "geojson";
  if (lowerType.includes("application/json")) return "json";
  if (lowerType.includes("application/pdf") || bytes.subarray(0, 4).toString() === "%PDF") return "pdf";
  if (lowerType.includes("text/csv") || lowerUrl.endsWith(".csv")) return "csv";
  if (lowerUrl.endsWith(".csv.gz")) return "csv.gz";
  if (lowerUrl.endsWith(".xlsx")) return "xlsx";
  if (lowerUrl.endsWith(".xls")) return "xls";
  if (lowerUrl.endsWith(".parquet")) return "parquet";
  if (lowerUrl.endsWith(".zip")) return "zip";
  if (lowerType.includes("xml") || lowerUrl.endsWith(".xml") || lowerUrl.endsWith(".rss")) return "xml";
  if (lowerType.includes("html")) return "html";
  return "unknown";
}

function pushFailure(severity, code, sourceId, detail) {
  failures.push({ severity, code, sourceId, detail });
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
