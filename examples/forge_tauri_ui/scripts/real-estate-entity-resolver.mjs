import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const eventsPath = argValue("--events");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePathFromEvents(eventsPath);
const limit = Number(argValue("--limit") ?? 512);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const graphPath = join(dataDir, "entity_graph.jsonl");
const latestPath = join(dataDir, "entity_resolver_latest.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${eventsPath}:${limit}`).slice(0, 16);
const failures = [];
const nodes = new Map();
const edges = new Map();
const clusters = new Map();

if (!eventsPath) fail("missing --events=<normalized_events.jsonl>");
if (!existsSync(eventsPath)) fail(`events file not found: ${eventsPath}`);
mkdirSync(dataDir, { recursive: true });
writeFileSync(graphPath, "");

const events = readJsonl(eventsPath).slice(0, limit);
for (const event of events) resolveEvent(event);

for (const node of nodes.values()) appendFileSync(graphPath, `${JSON.stringify(finalizeGraphRecord(node))}\n`);
for (const edge of edges.values()) appendFileSync(graphPath, `${JSON.stringify(finalizeGraphRecord(edge))}\n`);
for (const cluster of clusters.values()) appendFileSync(graphPath, `${JSON.stringify(finalizeGraphRecord(cluster))}\n`);

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function resolveEvent(event) {
  const rawHash = event.rawHash ?? event.universal?.rawArtifact?.hash ?? "";
  const datasetId = event.universal?.dataset?.id ?? `dataset:${sha256(event.sourceId ?? "unknown").slice(0, 24)}`;
  upsertNode({
    kind: "entity_node",
    nodeType: "dataset",
    id: datasetId,
    label: event.universal?.dataset?.label ?? event.sourceLabel ?? event.sourceId,
    sourceId: event.sourceId,
    confidence: 1,
    evidence: [{ eventHash: event.eventHash, rawHash }],
  });
  upsertNode({
    kind: "entity_node",
    nodeType: "raw_artifact",
    id: `raw:${rawHash}`,
    label: rawHash.slice(0, 12),
    sourceId: event.sourceId,
    confidence: 1,
    evidence: [{ eventHash: event.eventHash, rawHash }],
  });
  upsertEdge(datasetId, `raw:${rawHash}`, "produced_raw", 1, event);

  for (const candidate of event.universal?.entityCandidates ?? []) {
    const id = canonicalEntityId(candidate.kind, candidate.field, "schema");
    upsertNode({
      kind: "entity_node",
      nodeType: candidate.kind,
      id,
      label: candidate.field,
      sourceId: event.sourceId,
      confidence: candidate.confidence ?? 0.5,
      scope: "schema_field",
      evidence: [{ eventHash: event.eventHash, rawHash, field: candidate.field }],
    });
    upsertEdge(`raw:${rawHash}`, id, "mentions_schema_entity", candidate.confidence ?? 0.5, event);
    upsertCluster(candidate.kind, normalizeToken(candidate.field), id, candidate.confidence ?? 0.5, event);
  }

  for (const element of event.universal?.parsedElements ?? []) {
    if (element.kind !== "table") continue;
    resolveTableElement(event, rawHash, element);
  }
}

function resolveTableElement(event, rawHash, element) {
  const columns = element.columns ?? [];
  const columnKinds = new Map(columns.map((column) => [column, entityKindForField(column)]).filter(([, kind]) => kind));
  for (const [column, kind] of columnKinds) {
    const fieldNodeId = canonicalEntityId(kind, column, "schema");
    upsertEdge(`raw:${rawHash}`, fieldNodeId, "has_field_entity", 0.62, event);
  }
  for (const row of element.rows ?? []) {
    const rowHash = sha256(JSON.stringify(row)).slice(0, 24);
    const rowNodeId = `row:${rawHash.slice(0, 12)}:${rowHash}`;
    upsertNode({
      kind: "entity_node",
      nodeType: "record",
      id: rowNodeId,
      label: rowHash,
      sourceId: event.sourceId,
      confidence: 0.7,
      evidence: [{ eventHash: event.eventHash, rawHash }],
    });
    upsertEdge(`raw:${rawHash}`, rowNodeId, "contains_record_sample", 0.7, event);
    for (const [column, value] of Object.entries(row)) {
      const entityKind = columnKinds.get(column);
      if (!entityKind || !hasUsableValue(value)) continue;
      const id = canonicalEntityId(entityKind, value, "value");
      const confidence = valueConfidence(entityKind, value);
      upsertNode({
        kind: "entity_node",
        nodeType: entityKind,
        id,
        label: String(value).slice(0, 160),
        sourceId: event.sourceId,
        confidence,
        scope: "sample_value",
        evidence: [{ eventHash: event.eventHash, rawHash, field: column }],
      });
      upsertEdge(rowNodeId, id, "has_value_entity", confidence, event);
      upsertEdge(`raw:${rawHash}`, id, "mentions_value_entity", confidence, event);
      upsertCluster(entityKind, normalizeToken(value), id, confidence, event);
    }
  }
}

function upsertNode(next) {
  const current = nodes.get(next.id);
  if (!current) {
    nodes.set(next.id, { ...next, firstSeenAt: startedAt, evidence: next.evidence ?? [] });
    return;
  }
  current.confidence = Math.max(current.confidence ?? 0, next.confidence ?? 0);
  current.evidence = mergeEvidence(current.evidence, next.evidence);
}

function upsertEdge(from, to, relation, confidence, event) {
  if (!from || !to) return;
  const id = `edge:${sha256(`${from}:${relation}:${to}`).slice(0, 24)}`;
  const current = edges.get(id);
  const evidence = [{ eventHash: event.eventHash, rawHash: event.rawHash }];
  if (!current) {
    edges.set(id, {
      kind: "entity_edge",
      id,
      from,
      to,
      relation,
      confidence,
      evidence,
    });
    return;
  }
  current.confidence = Math.max(current.confidence ?? 0, confidence ?? 0);
  current.evidence = mergeEvidence(current.evidence, evidence);
}

function upsertCluster(entityKind, token, nodeId, confidence, event) {
  if (!token) return;
  const id = `cluster:${entityKind}:${sha256(token).slice(0, 24)}`;
  const current = clusters.get(id);
  const evidence = [{ eventHash: event.eventHash, rawHash: event.rawHash, nodeId }];
  if (!current) {
    clusters.set(id, {
      kind: "entity_cluster",
      id,
      entityKind,
      canonicalToken: token,
      nodeIds: [nodeId],
      confidence,
      evidence,
    });
    return;
  }
  if (!current.nodeIds.includes(nodeId)) current.nodeIds.push(nodeId);
  current.confidence = Math.max(current.confidence ?? 0, confidence ?? 0);
  current.evidence = mergeEvidence(current.evidence, evidence);
}

function buildSummary() {
  const nodeTypes = {};
  const edgeRelations = {};
  const clusterKinds = {};
  for (const node of nodes.values()) nodeTypes[node.nodeType] = (nodeTypes[node.nodeType] ?? 0) + 1;
  for (const edge of edges.values()) edgeRelations[edge.relation] = (edgeRelations[edge.relation] ?? 0) + 1;
  for (const cluster of clusters.values()) clusterKinds[cluster.entityKind] = (clusterKinds[cluster.entityKind] ?? 0) + 1;
  const summary = {
    kind: "real_estate_entity_resolver_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    eventsPath: eventsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    graphPath: graphPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    eventCount: events.length,
    nodeCount: nodes.size,
    edgeCount: edges.size,
    clusterCount: clusters.size,
    nodeTypes,
    edgeRelations,
    clusterKinds,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    eventCount: summary.eventCount,
    nodeCount: summary.nodeCount,
    edgeCount: summary.edgeCount,
    clusterCount: summary.clusterCount,
    nodeTypes,
    edgeRelations,
    clusterKinds,
    failures,
    graph: [...nodes.keys(), ...edges.keys(), ...clusters.keys()].sort(),
  }));
  return summary;
}

function canonicalEntityId(kind, value, scope) {
  return `${kind}:${scope}:${sha256(normalizeToken(value)).slice(0, 24)}`;
}

function entityKindForField(field) {
  const normalized = String(field ?? "").toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  if (/\b(adresse|address|numero_voie|nom_voie|voie|street)\b/.test(normalized)) return "address";
  if (/\b(commune|code_insee|insee|departement|department|territory|region|code_postal|postal)\b/.test(normalized)) return "admin_area";
  if (/\b(latitude|longitude|lat|lon|lng|coord|geometry|geometrie|geo)\b/.test(normalized)) return "geo_coordinate";
  if (/\b(parcelle|cadastre|section|numero_parcelle)\b/.test(normalized)) return "parcel";
  if (/\b(batiment|building|rnb|immeuble)\b/.test(normalized)) return "building";
  if (/\b(dpe|energie|ges|chauffage|consommation)\b/.test(normalized)) return "energy_diagnostic";
  if (/\b(prix|valeur|mutation|loyer|rent|price|montant)\b/.test(normalized)) return "market_value";
  if (/\b(date|annee|year|timestamp|created|updated)\b/.test(normalized)) return "time";
  if (/\b(siren|siret|naf|entreprise|company|etablissement)\b/.test(normalized)) return "business";
  if (/\b(risque|alea|inondation|argile|radon|icpe|catnat)\b/.test(normalized)) return "risk";
  if (/\b(plu|urbanisme|servitude|zonage|zone)\b/.test(normalized)) return "urbanism";
  if (/\b(article|news|title|headline|publication)\b/.test(normalized)) return "local_news";
  if (/\b(url|link|href|image)\b/.test(normalized)) return "external_reference";
  return "";
}

function valueConfidence(kind, value) {
  const text = String(value ?? "");
  if (kind === "geo_coordinate" && /^-?\d+(\.\d+)?$/.test(text)) return 0.82;
  if (kind === "time" && /\d{4}/.test(text)) return 0.78;
  if (kind === "admin_area" && text.length >= 2) return 0.74;
  if (kind === "external_reference" && /^https?:\/\//i.test(text)) return 0.86;
  return 0.68;
}

function hasUsableValue(value) {
  if (value === null || value === undefined) return false;
  const text = String(value).trim();
  if (!text || text === "[object]" || text.startsWith("[array:")) return false;
  return text.length <= 240;
}

function normalizeToken(value) {
  return String(value ?? "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .replace(/https?:\/\//g, "")
    .replace(/[^a-z0-9]+/g, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function mergeEvidence(left = [], right = []) {
  const seen = new Set(left.map((item) => JSON.stringify(item)));
  const out = [...left];
  for (const item of right ?? []) {
    const key = JSON.stringify(item);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(item);
  }
  return out.slice(0, 32);
}

function finalizeGraphRecord(record) {
  return {
    ...record,
    runId,
    resolvedAt: new Date().toISOString(),
    recordHash: sha256(JSON.stringify({ ...record, recordHash: "" })),
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
        failures.push({ severity: "error", code: "events_json_decode_failed", detail: `line ${index + 1}: ${error.message}` });
        return null;
      }
    })
    .filter(Boolean);
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function inferStorePathFromEvents(path) {
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/normalized_events.jsonl";
  if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  return dirname(dirname(dirname(path)));
}

function fail(message) {
  console.error(`[real-estate-entity-resolver] ${message}`);
  process.exit(1);
}

function sha256(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}
