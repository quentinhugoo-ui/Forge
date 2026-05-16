import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const graphPathArg = argValue("--graph");
const eventsPathArg = argValue("--events");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePath(graphPathArg, eventsPathArg);
const limit = Number(argValue("--limit") ?? 512);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const graphPath = graphPathArg ?? join(dataDir, "entity_graph.jsonl");
const eventsPath = eventsPathArg ?? join(dataDir, "normalized_events.jsonl");
const packsPath = join(dataDir, "intel_packs.jsonl");
const latestPath = join(dataDir, "intel_pack_builder_latest.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${graphPath}:${eventsPath}:${limit}`).slice(0, 16);
const failures = [];

if (!existsSync(graphPath)) fail(`graph file not found: ${graphPath}`);
if (!existsSync(eventsPath)) fail(`events file not found: ${eventsPath}`);
mkdirSync(dataDir, { recursive: true });
writeFileSync(packsPath, "");

const graphRecords = readJsonl(graphPath).slice(0, limit);
const events = readJsonl(eventsPath).slice(0, limit);
const packs = buildIntelPacks(graphRecords, events);
const finalizedPacks = packs.map((pack) => finalizePack(pack));

for (const pack of finalizedPacks) appendFileSync(packsPath, `${JSON.stringify(pack)}\n`);

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function buildIntelPacks(graph, normalizedEvents) {
  const byTheme = new Map();
  const graphStats = summarizeGraph(graph);
  for (const event of normalizedEvents) {
    const theme = themeForEvent(event);
    const bucket = byTheme.get(theme) ?? newThemePack(theme);
    byTheme.set(theme, bucket);
    absorbEvent(bucket, event);
  }
  for (const record of graph) absorbGraphRecord(byTheme, record);
  const packs = [...byTheme.values()];
  packs.push(buildExecutivePack(packs, graphStats));
  return packs.map((pack) => enrichPack(pack, graphStats));
}

function newThemePack(theme) {
  return {
    kind: "real_estate_intel_pack",
    schemaVersion: 1,
    runId,
    theme,
    scope: "public_harvester",
    title: titleForTheme(theme),
    sourceIds: new Set(),
    evidenceRefs: [],
    graphRefs: [],
    metrics: {
      eventCount: 0,
      rawCount: 0,
      nodeCount: 0,
      edgeCount: 0,
      clusterCount: 0,
      recordCount: 0,
      fieldCount: 0,
      sourceCount: 0,
    },
    signals: [],
    gaps: [],
    recommendedActions: [],
    quality: {},
  };
}

function absorbEvent(pack, event) {
  pack.metrics.eventCount += 1;
  pack.metrics.recordCount += event.recordCount ?? event.universal?.quality?.recordCount ?? 0;
  pack.metrics.fieldCount += event.universal?.quality?.fieldCount ?? event.fieldHints?.length ?? 0;
  if (event.rawHash) pack.metrics.rawCount += 1;
  if (event.sourceId) pack.sourceIds.add(event.sourceId);
  pack.evidenceRefs.push({
    eventHash: event.eventHash,
    rawHash: event.rawHash,
    sourceId: event.sourceId,
    url: event.resourceUrl,
    parser: event.parser,
    adapter: event.adapterPlan?.selected,
  });
  for (const metric of event.universal?.metricSeeds ?? []) {
    if (metric.key === "external_url_count" && metric.value > 0) {
      pack.signals.push(signal("source_links", `Ressource avec ${metric.value} URL(s) exploitables`, metric.confidence, event));
    }
  }
  if ((event.universal?.quality?.fieldCount ?? 0) === 0) {
    pack.gaps.push(gap("missing_fields", "Aucun champ exploitable detecte dans un raw", "medium", event));
  }
  if (event.adapterPlan?.externalAdaptersPending?.length) {
    pack.gaps.push(gap("adapter_pending", `Adapter SOTA absent: ${event.adapterPlan.externalAdaptersPending.join(", ")}`, "low", event));
  }
}

function absorbGraphRecord(byTheme, record) {
  const theme = themeForGraphRecord(record);
  const pack = byTheme.get(theme) ?? newThemePack(theme);
  byTheme.set(theme, pack);
  pack.graphRefs.push({ id: record.id, kind: record.kind, nodeType: record.nodeType, relation: record.relation, hash: record.recordHash });
  if (record.kind === "entity_node") pack.metrics.nodeCount += 1;
  if (record.kind === "entity_edge") pack.metrics.edgeCount += 1;
  if (record.kind === "entity_cluster") pack.metrics.clusterCount += 1;
  if (record.sourceId) pack.sourceIds.add(record.sourceId);
  if (record.kind === "entity_cluster" && record.entityKind) {
    pack.signals.push(signal("cluster_detected", `Cluster ${record.entityKind}: ${record.nodeIds?.length ?? 0} noeud(s)`, record.confidence ?? 0.5, record));
  }
}

function buildExecutivePack(packs, graphStats) {
  const pack = newThemePack("executive");
  pack.title = "Pack executif harvester immo";
  for (const child of packs) {
    pack.metrics.eventCount += child.metrics.eventCount;
    pack.metrics.rawCount += child.metrics.rawCount;
    pack.metrics.nodeCount += child.metrics.nodeCount;
    pack.metrics.edgeCount += child.metrics.edgeCount;
    pack.metrics.clusterCount += child.metrics.clusterCount;
    pack.metrics.recordCount += child.metrics.recordCount;
    pack.metrics.fieldCount += child.metrics.fieldCount;
    for (const sourceId of child.sourceIds) pack.sourceIds.add(sourceId);
  }
  pack.signals.push({
    type: "graph_density",
    label: `${graphStats.nodeCount} nodes, ${graphStats.edgeCount} edges, ${graphStats.clusterCount} clusters`,
    confidence: graphStats.nodeCount ? 0.78 : 0.2,
  });
  return pack;
}

function enrichPack(pack, graphStats) {
  pack.metrics.sourceCount = pack.sourceIds.size;
  pack.sourceIds = [...pack.sourceIds].sort();
  pack.evidenceRefs = dedupeBy(pack.evidenceRefs, (item) => `${item.eventHash}:${item.rawHash}`).slice(0, 32);
  pack.graphRefs = dedupeBy(pack.graphRefs, (item) => `${item.id}:${item.hash}`).slice(0, 48);
  pack.signals = dedupeBy(pack.signals, (item) => `${item.type}:${item.label}`).slice(0, 16);
  pack.gaps = dedupeBy(pack.gaps, (item) => `${item.type}:${item.label}`).slice(0, 16);
  pack.quality = qualityForPack(pack, graphStats);
  pack.recommendedActions = recommendedActionsForPack(pack);
  pack.llmContract = {
    rule: "Do not read raw files unless explicitly requested; use evidenceRefs, graphRefs and proofHash.",
    maxContextHint: "compact",
    canAnswer: pack.quality.usabilityScore >= 0.35,
  };
  pack.summary = summaryForPack(pack);
  return pack;
}

function qualityForPack(pack, graphStats) {
  const sourceScore = Math.min(1, pack.metrics.sourceCount / 3);
  const evidenceScore = Math.min(1, pack.evidenceRefs.length / 5);
  const graphScore = Math.min(1, (pack.metrics.nodeCount + pack.metrics.edgeCount + pack.metrics.clusterCount) / 10);
  const fieldScore = Math.min(1, pack.metrics.fieldCount / 12);
  const gapPenalty = Math.min(0.45, pack.gaps.length * 0.08);
  const usabilityScore = clamp((sourceScore * 0.25) + (evidenceScore * 0.25) + (graphScore * 0.25) + (fieldScore * 0.25) - gapPenalty, 0, 1);
  return {
    sourceScore,
    evidenceScore,
    graphScore,
    fieldScore,
    gapPenalty,
    usabilityScore,
    graphDensityReference: graphStats.nodeCount ? graphStats.edgeCount / graphStats.nodeCount : 0,
  };
}

function recommendedActionsForPack(pack) {
  const actions = [];
  if (pack.quality.usabilityScore < 0.35) actions.push("Collecter plus de ressources data-bearing avant exposition au LLM.");
  if (pack.gaps.some((item) => item.type === "adapter_pending")) actions.push("Installer ou brancher l'adapter SOTA manquant pour ce format.");
  if (pack.metrics.clusterCount > 0) actions.push("Promouvoir les clusters en candidats de jointure KASM.");
  if (pack.metrics.fieldCount > 0 && pack.metrics.recordCount === 0) actions.push("Relancer avec limite de telechargement plus large ou endpoint pagine.");
  if (!actions.length) actions.push("Pack exploitable par KASM/brain pour scoring local.");
  return actions.slice(0, 6);
}

function summaryForPack(pack) {
  return {
    title: pack.title,
    status: pack.quality.usabilityScore >= 0.65 ? "usable" : pack.quality.usabilityScore >= 0.35 ? "partial" : "thin",
    metrics: pack.metrics,
    topSignals: pack.signals.slice(0, 4).map((item) => item.label),
    topGaps: pack.gaps.slice(0, 4).map((item) => item.label),
  };
}

function themeForEvent(event) {
  const text = `${event.sourceId ?? ""} ${event.collector ?? ""} ${event.sourceLabel ?? ""}`.toLowerCase();
  if (/(dvf|foncier|mutation|market|price|valeur)/.test(text)) return "market";
  if (/(dpe|energy|energie|renovation|ademe)/.test(text)) return "energy";
  if (/(risque|georisque|argile|radon|flood|inondation)/.test(text)) return "risk";
  if (/(urbanisme|plu|cadastre|parcelle|rnb|building)/.test(text)) return "land";
  if (/(transport|mobilite|mobility)/.test(text)) return "mobility";
  if (/(news|actualite|perception|local)/.test(text)) return "local_signal";
  if (/(education|school|sante|health|service)/.test(text)) return "services";
  if (/(sirene|business|emploi|jobs|boamp|procurement)/.test(text)) return "economic";
  return "general";
}

function themeForGraphRecord(record) {
  const text = `${record.nodeType ?? ""} ${record.entityKind ?? ""} ${record.label ?? ""}`.toLowerCase();
  if (/(market|price|value|prix)/.test(text)) return "market";
  if (/(energy|dpe|diagnostic)/.test(text)) return "energy";
  if (/(risk|risque)/.test(text)) return "risk";
  if (/(urbanism|parcel|building|cadastre|land)/.test(text)) return "land";
  if (/(local_news|external_reference)/.test(text)) return "local_signal";
  if (/(admin_area|address|geo_coordinate)/.test(text)) return "geography";
  return "general";
}

function titleForTheme(theme) {
  return {
    executive: "Pack executif harvester immo",
    market: "Marche et valeurs",
    energy: "Energie, DPE et renovation",
    risk: "Risques et assurabilite",
    land: "Foncier, cadastre et urbanisme",
    mobility: "Mobilite et accessibilite",
    local_signal: "Veille locale et perception",
    services: "Services publics et attractivite",
    economic: "Activite economique locale",
    geography: "Geographie et jointures",
    general: "Sources generales",
  }[theme] ?? theme;
}

function summarizeGraph(records) {
  const out = { nodeCount: 0, edgeCount: 0, clusterCount: 0 };
  for (const record of records) {
    if (record.kind === "entity_node") out.nodeCount += 1;
    if (record.kind === "entity_edge") out.edgeCount += 1;
    if (record.kind === "entity_cluster") out.clusterCount += 1;
  }
  return out;
}

function finalizePack(pack) {
  const prepared = {
    ...pack,
    generatedAt: new Date().toISOString(),
  };
  prepared.proofHash = sha256(JSON.stringify({
    theme: prepared.theme,
    metrics: prepared.metrics,
    quality: prepared.quality,
    evidenceRefs: prepared.evidenceRefs,
    graphRefs: prepared.graphRefs,
    signals: prepared.signals,
    gaps: prepared.gaps,
  }));
  return prepared;
}

function buildSummary() {
  const themes = {};
  let usable = 0;
  for (const pack of packs) {
    themes[pack.theme] = (themes[pack.theme] ?? 0) + 1;
    if (pack.quality?.usabilityScore >= 0.35) usable += 1;
  }
  const summary = {
    kind: "real_estate_intel_pack_builder_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    graphPath: graphPath.replaceAll("\\", "/"),
    eventsPath: eventsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    packsPath: packsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    graphRecordCount: graphRecords.length,
    eventCount: events.length,
    packCount: packs.length,
    usablePackCount: usable,
    themes,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    graphRecordCount: summary.graphRecordCount,
    eventCount: summary.eventCount,
    packCount: summary.packCount,
    usablePackCount: usable,
    themes,
    failures,
    packs: finalizedPacks.map((pack) => pack.proofHash),
  }));
  return summary;
}

function signal(type, label, confidence, evidence) {
  return {
    type,
    label,
    confidence: clamp(confidence ?? 0.5, 0, 1),
    evidenceHash: evidence.eventHash ?? evidence.recordHash ?? "",
  };
}

function gap(type, label, severity, evidence) {
  return {
    type,
    label,
    severity,
    evidenceHash: evidence.eventHash ?? evidence.recordHash ?? "",
  };
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
        failures.push({ severity: "error", code: "jsonl_decode_failed", detail: `${path} line ${index + 1}: ${error.message}` });
        return null;
      }
    })
    .filter(Boolean);
}

function dedupeBy(items, keyFn) {
  const seen = new Set();
  const out = [];
  for (const item of items) {
    const key = keyFn(item);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(item);
  }
  return out;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function inferStorePath(graph, events) {
  const path = graph ?? events;
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const markers = ["/real-estate-harvester/data/entity_graph.jsonl", "/real-estate-harvester/data/normalized_events.jsonl"];
  for (const marker of markers) {
    if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  }
  return dirname(dirname(dirname(path)));
}

function fail(message) {
  console.error(`[real-estate-intel-pack-builder] ${message}`);
  process.exit(1);
}

function sha256(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}
