import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultAdaptersPath = join(root, "source-registry", "real-estate-parser-adapters.json");
const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const downloadsPath = argValue("--downloads");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePathFromDownloads(downloadsPath);
const adaptersPath = argValue("--adapters") ?? defaultAdaptersPath;
const limit = Number(argValue("--limit") ?? 128);
const maxTextBytes = Number(argValue("--max-text-bytes") ?? 1024 * 1024);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const eventsPath = join(dataDir, "normalized_events.jsonl");
const latestPath = join(dataDir, "parser_router_latest.json");

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function fail(message) {
  console.error(`[real-estate-parser-router] ${message}`);
  process.exit(1);
}

if (!downloadsPath) fail("missing --downloads=<raw_downloads.jsonl>");
if (!existsSync(downloadsPath)) fail(`downloads file not found: ${downloadsPath}`);
if (!existsSync(adaptersPath)) fail(`parser adapters registry not found: ${adaptersPath}`);

const startedAt = new Date().toISOString();
const adapterRegistry = JSON.parse(readFileSync(adaptersPath, "utf8"));
const availableAdapters = detectAvailableAdapters(adapterRegistry);
const runId = sha256(`${startedAt}:${downloadsPath}:${adaptersPath}:${limit}`).slice(0, 16);
const failures = [];
const rawRecords = readJsonl(downloadsPath)
  .filter((record) => record.status === "ok" && record.rawPath && existsSync(record.rawPath))
  .slice(0, limit);
const events = [];

mkdirSync(dataDir, { recursive: true });

for (const raw of rawRecords) {
  const produced = routeRecord(raw);
  for (const event of produced) {
    const finalized = finalizeEvent(event);
    events.push(finalized);
    appendFileSync(eventsPath, `${JSON.stringify(finalized)}\n`);
  }
}

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function routeRecord(raw) {
  const format = normalizeFormat(raw.detectedFormat || raw.format);
  const bytes = readFileSync(raw.rawPath);
  const adapterPlan = selectAdapter(format);
  if (["json", "geojson"].includes(format)) return parseJsonLike(raw, bytes, format, adapterPlan);
  if (["csv", "txt"].includes(format)) return parseDelimitedLike(raw, bytes, format, adapterPlan);
  if (format === "xml") return parseXmlLike(raw, bytes, adapterPlan);
  if (format === "html") return parseHtmlLike(raw, bytes, adapterPlan);
  if (["pdf", "zip", "xlsx", "xls", "parquet", "csv.gz"].includes(format)) return [binaryPlaceholder(raw, bytes, format, adapterPlan)];
  return [unsupportedPlaceholder(raw, bytes, format, adapterPlan)];
}

function parseJsonLike(raw, bytes, format, adapterPlan) {
  const text = boundedText(bytes);
  try {
    const value = JSON.parse(text);
    const resources = [];
    const geo = [];
    const records = [];
    walkJson(value, (node, path) => {
      if (!node || typeof node !== "object" || Array.isArray(node)) return;
      const url = stringValue(node.url) ?? stringValue(node.latest) ?? stringValue(node.href) ?? stringValue(node.download_url);
      if (url && isHttpUrl(url)) {
        resources.push({
          url,
          format: normalizeFormat(stringValue(node.format) ?? stringValue(node.mime) ?? ""),
          title: stringValue(node.title) ?? stringValue(node.name) ?? "",
          path: path.join("."),
        });
      }
      const coordinates = node.geometry?.coordinates ?? node.coordinates;
      if (Array.isArray(coordinates)) {
        geo.push({ path: path.join("."), preview: JSON.stringify(coordinates).slice(0, 200) });
      }
      const keys = Object.keys(node);
      if (keys.length >= 2 && records.length < 16) {
        records.push(keys.slice(0, 24));
      }
    });
    return [
      eventBase(raw, adapterPlan, {
        eventType: format === "geojson" ? "geojson_summary" : "json_summary",
        parser: format,
        recordCount: estimateRecordCount(value),
        discoveredUrls: resources.slice(0, 64),
        geoHints: geo.slice(0, 32),
        fieldHints: uniqueFieldHints(records).slice(0, 64),
        textPreview: "",
        parseStatus: "ok",
      }),
    ];
  } catch (error) {
    pushFailure("warning", "json_parse_failed", raw.sourceId, `${raw.rawPath}: ${error.message}`);
    return [
      eventBase(raw, adapterPlan, {
        eventType: "json_parse_failed",
        parser: format,
        recordCount: 0,
        discoveredUrls: [],
        geoHints: [],
        fieldHints: [],
        textPreview: text.slice(0, 500),
        parseStatus: "failed",
        error: error.message,
      }),
    ];
  }
}

function parseDelimitedLike(raw, bytes, format, adapterPlan) {
  const text = boundedText(bytes);
  const lines = text.split(/\r?\n/).filter(Boolean);
  const delimiter = guessDelimiter(lines.slice(0, 5));
  const header = splitDelimited(lines[0] ?? "", delimiter).map(cleanCell).filter(Boolean);
  return [
    eventBase(raw, adapterPlan, {
      eventType: format === "csv" ? "csv_summary" : "text_summary",
      parser: format,
      recordCount: Math.max(0, lines.length - (header.length ? 1 : 0)),
      discoveredUrls: discoverUrls(text).slice(0, 64),
      geoHints: [],
      fieldHints: header.slice(0, 128),
      textPreview: lines.slice(0, 6).join("\n").slice(0, 1000),
      parseStatus: "ok",
      delimiter,
    }),
  ];
}

function parseXmlLike(raw, bytes, adapterPlan) {
  const text = boundedText(bytes);
  const tags = [...text.matchAll(/<([A-Za-z_][\w:.-]*)\b/g)].map((match) => match[1]);
  return [
    eventBase(raw, adapterPlan, {
      eventType: "xml_summary",
      parser: "xml",
      recordCount: countRepeatedTags(tags),
      discoveredUrls: discoverUrls(text).slice(0, 64),
      geoHints: [],
      fieldHints: topValues(tags, 64),
      textPreview: text.replace(/\s+/g, " ").slice(0, 1000),
      parseStatus: "ok",
    }),
  ];
}

function parseHtmlLike(raw, bytes, adapterPlan) {
  const text = boundedText(bytes);
  const title = text.match(/<title[^>]*>(.*?)<\/title>/is)?.[1]?.replace(/\s+/g, " ").trim() ?? "";
  return [
    eventBase(raw, adapterPlan, {
      eventType: "html_summary",
      parser: "html",
      recordCount: 0,
      discoveredUrls: discoverUrls(text).slice(0, 64),
      geoHints: [],
      fieldHints: [],
      textPreview: title || text.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").slice(0, 1000),
      parseStatus: "ok",
    }),
  ];
}

function binaryPlaceholder(raw, bytes, format, adapterPlan) {
  return eventBase(raw, adapterPlan, {
    eventType: `${format}_raw_ready`,
    parser: format,
    recordCount: 0,
    discoveredUrls: [],
    geoHints: [],
    fieldHints: [],
    textPreview: "",
    parseStatus: "deferred_binary_parser",
    rawBytes: bytes.length,
  });
}

function unsupportedPlaceholder(raw, bytes, format, adapterPlan) {
  pushFailure("warning", "unsupported_format", raw.sourceId, `${format}: ${raw.rawPath}`);
  return eventBase(raw, adapterPlan, {
    eventType: "unsupported_raw",
    parser: "unknown",
    recordCount: 0,
    discoveredUrls: [],
    geoHints: [],
    fieldHints: [],
    textPreview: "",
    parseStatus: "unsupported",
    rawBytes: bytes.length,
  });
}

function eventBase(raw, adapterPlan, payload) {
  return {
    kind: "real_estate_normalized_event",
    schemaVersion: 1,
    runId,
    parsedAt: new Date().toISOString(),
    sourceId: raw.sourceId,
    sourceLabel: raw.sourceLabel,
    sourceLicense: raw.sourceLicense,
    sourcePriority: raw.sourcePriority,
    collector: raw.collector,
    resourceUrl: raw.url,
    parentUrl: raw.parentUrl,
    rawHash: raw.rawHash,
    rawPath: raw.rawPath,
    format: normalizeFormat(raw.detectedFormat || raw.format),
    adapterPlan,
    freshness: raw.freshness,
    ...payload,
  };
}

function finalizeEvent(event) {
  event.eventHash = sha256(JSON.stringify({ ...event, eventHash: "" }));
  return event;
}

function buildSummary() {
  const eventTypes = {};
  const parsers = {};
  const statuses = {};
  const adapters = {};
  let recordCount = 0;
  for (const event of events) {
    eventTypes[event.eventType] = (eventTypes[event.eventType] ?? 0) + 1;
    parsers[event.parser] = (parsers[event.parser] ?? 0) + 1;
    statuses[event.parseStatus] = (statuses[event.parseStatus] ?? 0) + 1;
    adapters[event.adapterPlan?.selected ?? "unknown"] = (adapters[event.adapterPlan?.selected ?? "unknown"] ?? 0) + 1;
    recordCount += event.recordCount ?? 0;
  }
  const summary = {
    kind: "real_estate_parser_router_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    downloadsPath: downloadsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    adaptersPath: adaptersPath.replaceAll("\\", "/"),
    eventsPath: eventsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    rawRecordCount: rawRecords.length,
    eventCount: events.length,
    recordCount,
    eventTypes,
    parsers,
    statuses,
    adapters,
    availableAdapters,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    rawRecordCount: summary.rawRecordCount,
    eventCount: summary.eventCount,
    recordCount,
    eventTypes,
    parsers,
    statuses,
    adapters,
    availableAdapters,
    failures,
    events: events.map((event) => event.eventHash),
  }));
  return summary;
}

function selectAdapter(format) {
  const route = adapterRegistry.routes?.[format] ?? adapterRegistry.routes?.unknown ?? ["native"];
  const selected = route.find((adapter) => adapter === "native" || availableAdapters[adapter]) ?? "native";
  const preferred = route[0] ?? "native";
  const selectedMeta = adapterRegistry.adapters?.[selected] ?? {};
  return {
    format,
    preferred,
    selected,
    route,
    mode: selected === preferred ? "preferred" : "fallback",
    reason: selectedMeta.sotaReason ?? "deterministic local fallback",
    available: selected === "native" || Boolean(availableAdapters[selected]),
    externalAdaptersPending: route.filter((adapter) => adapter !== "native" && !availableAdapters[adapter]),
    universalModel: Object.keys(adapterRegistry.universalEventModel ?? {}),
  };
}

function detectAvailableAdapters(registry) {
  const out = { native: true };
  for (const [name, adapter] of Object.entries(registry.adapters ?? {})) {
    if (name === "native") {
      out.native = true;
      continue;
    }
    out[name] = commandExists(adapter.command);
  }
  return out;
}

function commandExists(command) {
  if (!command) return false;
  const result = spawnSync(command, ["--version"], {
    encoding: "utf8",
    stdio: "ignore",
    timeout: 1500,
    windowsHide: true,
  });
  return !result.error && typeof result.status === "number";
}

function readJsonl(path) {
  return readFileSync(path, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        pushFailure("error", "jsonl_decode_failed", path, `line ${index + 1}: ${error.message}`);
        return null;
      }
    })
    .filter(Boolean);
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

function estimateRecordCount(value) {
  if (Array.isArray(value)) return value.length;
  if (Array.isArray(value?.data)) return value.data.length;
  if (Array.isArray(value?.features)) return value.features.length;
  if (Array.isArray(value?.resources)) return value.resources.length;
  if (value && typeof value === "object") return Object.keys(value).length;
  return 0;
}

function uniqueFieldHints(rows) {
  const seen = new Set();
  const out = [];
  for (const row of rows) {
    for (const key of row) {
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(key);
    }
  }
  return out;
}

function guessDelimiter(lines) {
  const candidates = [",", ";", "\t", "|"];
  let best = ",";
  let bestScore = -1;
  for (const candidate of candidates) {
    const score = lines.reduce((sum, line) => sum + line.split(candidate).length, 0);
    if (score > bestScore) {
      best = candidate;
      bestScore = score;
    }
  }
  return best;
}

function splitDelimited(line, delimiter) {
  if (delimiter === "\t") return line.split("\t");
  return line.split(delimiter);
}

function cleanCell(value) {
  return String(value ?? "").replace(/^"|"$/g, "").trim();
}

function countRepeatedTags(tags) {
  const counts = tagCounts(tags);
  return Math.max(0, ...Object.values(counts));
}

function topValues(values, limit) {
  const counts = tagCounts(values);
  return Object.entries(counts)
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([value]) => value);
}

function tagCounts(values) {
  const counts = {};
  for (const value of values) counts[value] = (counts[value] ?? 0) + 1;
  return counts;
}

function discoverUrls(text) {
  return [...text.matchAll(/https?:\/\/[^\s"'<>]+/gi)]
    .map((match) => match[0])
    .filter((url, index, all) => all.indexOf(url) === index)
    .map((url) => ({ url, format: normalizeFormatFromUrl(url) }));
}

function normalizeFormatFromUrl(url) {
  const lower = url.toLowerCase();
  if (lower.endsWith(".geojson")) return "geojson";
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".csv")) return "csv";
  if (lower.endsWith(".csv.gz")) return "csv.gz";
  if (lower.endsWith(".xlsx")) return "xlsx";
  if (lower.endsWith(".xls")) return "xls";
  if (lower.endsWith(".parquet")) return "parquet";
  if (lower.endsWith(".xml") || lower.endsWith(".rss") || lower.endsWith(".kml")) return "xml";
  if (lower.endsWith(".pdf")) return "pdf";
  if (lower.endsWith(".zip")) return "zip";
  return "html";
}

function normalizeFormat(value) {
  const format = String(value ?? "").trim().toLowerCase();
  if (!format) return "unknown";
  if (format.includes("geojson") || format.includes("geo+json")) return "geojson";
  if (format.includes("json")) return "json";
  if (format.includes("csv") && format.includes("gzip")) return "csv.gz";
  if (format.includes("csv")) return "csv";
  if (format.includes("xlsx")) return "xlsx";
  if (format.includes("xls")) return "xls";
  if (format.includes("parquet")) return "parquet";
  if (format.includes("xml") || format.includes("kml")) return "xml";
  if (format.includes("pdf")) return "pdf";
  if (format.includes("zip")) return "zip";
  if (format.includes("html")) return "html";
  if (format === "txt" || format.includes("plain")) return "txt";
  return format;
}

function boundedText(bytes) {
  return bytes.subarray(0, maxTextBytes).toString("utf8");
}

function stringValue(value) {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function isHttpUrl(value) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function inferStorePathFromDownloads(path) {
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/raw_downloads.jsonl";
  if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  return dirname(dirname(dirname(path)));
}

function pushFailure(severity, code, sourceId, detail) {
  failures.push({ severity, code, sourceId, detail });
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
