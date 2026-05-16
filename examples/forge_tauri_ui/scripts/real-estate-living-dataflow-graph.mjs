import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? join(root, ".forge-data");
const limit = Number(argValue("--limit") ?? 1024);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const graphJsonlPath = join(dataDir, "living_dataflow_graph.jsonl");
const graphLatestPath = join(dataDir, "living_dataflow_graph.json");
const previousLatest = readJson(graphLatestPath);

const artifacts = {
  sourceManifest: join(dataDir, "source_manifest.jsonl"),
  rawDownloads: join(dataDir, "raw_downloads.jsonl"),
  normalizedEvents: join(dataDir, "normalized_events.jsonl"),
  entityGraph: join(dataDir, "entity_graph.jsonl"),
  intelPacks: join(dataDir, "intel_packs.jsonl"),
  kasmMetricSeeds: join(dataDir, "kasm_metric_seeds.jsonl"),
  rankedActions: join(dataDir, "ranked_actions.json"),
  memoryCommits: join(dataDir, "real_estate_memory_commits.jsonl"),
};

mkdirSync(dataDir, { recursive: true });

const startedAt = new Date().toISOString();
const inputManifest = buildInputManifest();
const inputHash = sha256(JSON.stringify(inputManifest));

if (previousLatest?.inputHash === inputHash && existsSync(graphJsonlPath) && !args.has("--force")) {
  const reused = {
    ...previousLatest,
    status: "reused",
    reusedAt: new Date().toISOString(),
    inputHash,
    graphPath: graphJsonlPath.replaceAll("\\", "/"),
    latestPath: graphLatestPath.replaceAll("\\", "/"),
  };
  writeFileSync(graphLatestPath, `${JSON.stringify(reused, null, 2)}\n`);
  if (pretty || !args.has("--quiet")) console.log(JSON.stringify(reused, null, pretty ? 2 : 0));
  process.exit(0);
}

const nodes = new Map();
const edges = new Map();
const counters = {
  sources: 0,
  rawArtifacts: 0,
  events: 0,
  entities: 0,
  signals: 0,
  scores: 0,
  intelPacks: 0,
  actions: 0,
  memoryFacts: 0,
};

for (const source of readJsonl(artifacts.sourceManifest).slice(0, limit)) ingestSource(source);
for (const raw of readJsonl(artifacts.rawDownloads).slice(0, limit)) ingestRaw(raw);
for (const event of readJsonl(artifacts.normalizedEvents).slice(0, limit)) ingestEvent(event);
for (const record of readJsonl(artifacts.entityGraph).slice(0, limit)) ingestEntityRecord(record);
for (const pack of readJsonl(artifacts.intelPacks).slice(0, limit)) ingestIntelPack(pack);
for (const seed of readJsonl(artifacts.kasmMetricSeeds).slice(0, limit)) ingestScore(seed);
for (const action of readRankedActions(artifacts.rankedActions).slice(0, limit)) ingestAction(action);
for (const fact of readJsonl(artifacts.memoryCommits).slice(0, limit)) ingestMemoryFact(fact);

writeFileSync(graphJsonlPath, "");
const sortedNodes = [...nodes.values()].sort((a, b) => a.id.localeCompare(b.id)).map(finalizeRecord);
const sortedEdges = [...edges.values()].sort((a, b) => a.id.localeCompare(b.id)).map(finalizeRecord);
for (const node of sortedNodes) appendFileSync(graphJsonlPath, `${JSON.stringify(node)}\n`);
for (const edge of sortedEdges) appendFileSync(graphJsonlPath, `${JSON.stringify(edge)}\n`);

const summary = buildSummary(sortedNodes, sortedEdges);
writeFileSync(graphLatestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));

function ingestSource(source) {
  const sourceId = source.sourceId ?? source.id ?? source.resourceId ?? `source:${hashAny(source).slice(0, 16)}`;
  counters.sources += 1;
  upsertNode("source", sourceId, {
    label: source.sourceLabel ?? source.label ?? sourceId,
    sourceId,
    confidence: 0.82,
    payload: pick(source, ["url", "resourceUrl", "sourceId", "sourceLabel", "format", "contentType", "downloadScore"]),
  });
}

function ingestRaw(raw) {
  const rawHash = raw.rawHash ?? raw.hash ?? raw.previewHash ?? hashAny(raw);
  const rawId = `raw:${rawHash}`;
  counters.rawArtifacts += 1;
  upsertNode("raw", rawId, {
    label: raw.path ?? raw.url ?? rawHash.slice(0, 12),
    sourceId: raw.sourceId,
    confidence: raw.cacheStatus === "hit" ? 0.86 : 0.74,
    payload: pick(raw, ["url", "path", "rawPath", "bytes", "cacheStatus", "contentType"]),
  });
  if (raw.sourceId) upsertEdge(`source:${raw.sourceId}`, rawId, "produces_raw", 0.86, rawHash);
}

function ingestEvent(event) {
  const eventHash = event.eventHash ?? hashAny(event);
  const eventId = `event:${eventHash}`;
  const rawHash = event.rawHash ?? event.universal?.rawArtifact?.hash;
  counters.events += 1;
  upsertNode("event", eventId, {
    label: event.sourceLabel ?? event.sourceId ?? eventHash.slice(0, 12),
    sourceId: event.sourceId,
    confidence: event.universal?.quality?.fieldCount ? 0.78 : 0.48,
    payload: {
      parser: event.parser,
      adapter: event.adapterPlan?.selected,
      recordCount: event.recordCount ?? event.universal?.quality?.recordCount ?? 0,
      fieldCount: event.universal?.quality?.fieldCount ?? 0,
    },
  });
  if (event.sourceId) upsertEdge(`source:${event.sourceId}`, eventId, "emits_event", 0.72, eventHash);
  if (rawHash) upsertEdge(`raw:${rawHash}`, eventId, "parsed_into", 0.8, eventHash);
  for (const metric of event.universal?.metricSeeds ?? []) {
    const signalId = `signal:${sha256(`${eventHash}:${metric.key}`).slice(0, 24)}`;
    counters.signals += 1;
    upsertNode("signal", signalId, {
      label: metric.key,
      sourceId: event.sourceId,
      confidence: metric.confidence ?? 0.5,
      payload: { key: metric.key, value: metric.value, unit: metric.unit },
    });
    upsertEdge(eventId, signalId, "extracts_signal", metric.confidence ?? 0.5, eventHash);
  }
}

function ingestEntityRecord(record) {
  const id = record.id ?? record.recordHash ?? hashAny(record);
  const nodeType = record.nodeType ?? record.entityKind ?? record.kind ?? "entity";
  if (record.kind === "entity_edge" && record.from && record.to) {
    upsertEdge(record.from, record.to, record.relation ?? "relates_to", record.confidence ?? 0.5, record.recordHash ?? id);
    return;
  }
  counters.entities += 1;
  upsertNode("entity", id, {
    label: record.label ?? record.canonicalToken ?? id,
    sourceId: record.sourceId,
    confidence: record.confidence ?? 0.55,
    payload: { nodeType, graphKind: record.kind, evidence: record.evidence?.slice?.(0, 6) ?? [] },
  });
  for (const evidence of record.evidence ?? []) {
    if (evidence.eventHash) upsertEdge(`event:${evidence.eventHash}`, id, "supports_entity", record.confidence ?? 0.55, evidence.eventHash);
  }
}

function ingestIntelPack(pack) {
  const packId = `pack:${pack.proofHash ?? hashAny(pack)}`;
  counters.intelPacks += 1;
  upsertNode("intelPack", packId, {
    label: pack.title ?? pack.theme ?? "intel pack",
    confidence: pack.quality?.usabilityScore ?? 0.5,
    payload: {
      theme: pack.theme,
      status: pack.summary?.status,
      metrics: pack.metrics,
      recommendedActions: pack.recommendedActions?.slice?.(0, 6) ?? [],
    },
  });
  for (const evidence of pack.evidenceRefs ?? []) {
    if (evidence.eventHash) upsertEdge(`event:${evidence.eventHash}`, packId, "feeds_pack", pack.quality?.usabilityScore ?? 0.5, evidence.eventHash);
  }
  for (const graphRef of pack.graphRefs ?? []) {
    if (graphRef.id) upsertEdge(graphRef.id, packId, "summarized_by_pack", pack.quality?.usabilityScore ?? 0.5, graphRef.hash ?? graphRef.id);
  }
  for (const signal of pack.signals ?? []) {
    const signalId = `signal:${sha256(`${packId}:${signal.type}:${signal.label}`).slice(0, 24)}`;
    counters.signals += 1;
    upsertNode("signal", signalId, {
      label: signal.label ?? signal.type,
      confidence: signal.confidence ?? 0.5,
      payload: pick(signal, ["type", "label", "confidence"]),
    });
    upsertEdge(signalId, packId, "supports_pack", signal.confidence ?? 0.5, packId);
  }
}

function ingestScore(seed) {
  const scoreId = `score:${seed.seedHash ?? hashAny(seed)}`;
  const packId = seed.packProofHash ? `pack:${seed.packProofHash}` : "";
  counters.scores += 1;
  upsertNode("score", scoreId, {
    label: seed.metricKey ?? seed.seedType ?? "score",
    confidence: seed.confidence ?? 0.55,
    payload: {
      metricValue: seed.metricValue,
      priorityScore: seed.priorityScore,
      featureVector: seed.featureVector,
      simulationHints: seed.simulationHints,
    },
  });
  if (packId) upsertEdge(packId, scoreId, "scores_into", seed.confidence ?? 0.55, scoreId);
}

function ingestAction(action) {
  const actionId = `action:${action.actionHash ?? action.id ?? hashAny(action)}`;
  counters.actions += 1;
  upsertNode("action", actionId, {
    label: action.title ?? action.label ?? action.recommendedAction ?? actionId,
    confidence: action.confidence ?? action.score ?? 0.5,
    payload: pick(action, ["rank", "score", "risk", "gain", "cost", "scenario", "toolId", "reason"]),
  });
  for (const key of [action.seedHash, action.packProofHash, action.proofHash].filter(Boolean)) {
    upsertEdge(key.startsWith("score:") ? key : `score:${key}`, actionId, "ranks_action", action.confidence ?? action.score ?? 0.5, key);
  }
}

function ingestMemoryFact(fact) {
  const factId = `memory:${fact.proofHash ?? fact.memoryHash ?? hashAny(fact)}`;
  counters.memoryFacts += 1;
  upsertNode("memoryFact", factId, {
    label: fact.title ?? fact.key ?? fact.kind ?? "memory fact",
    confidence: fact.trustScore ?? fact.confidence ?? 0.55,
    payload: pick(fact, ["scope", "layer", "trustScore", "evidenceHash", "brainCommitRequest"]),
  });
  const evidenceHash = fact.evidenceHash ?? fact.brainCommitRequest?.evidenceHash;
  if (evidenceHash) upsertEdge(evidenceHash.startsWith("pack:") ? evidenceHash : `pack:${evidenceHash}`, factId, "committed_to_memory", fact.trustScore ?? 0.55, evidenceHash);
}

function upsertNode(type, id, data) {
  if (!id) return;
  const normalizedId = id.includes(":") ? id : `${type}:${id}`;
  const current = nodes.get(normalizedId);
  if (!current) {
    nodes.set(normalizedId, {
      kind: "dataflow_node",
      id: normalizedId,
      type,
      label: String(data.label ?? normalizedId).slice(0, 220),
      sourceId: data.sourceId ?? "",
      confidence: clamp(Number(data.confidence ?? 0.5), 0, 1),
      payload: data.payload ?? {},
      evidenceHashes: [],
    });
    return;
  }
  current.confidence = Math.max(current.confidence, clamp(Number(data.confidence ?? 0.5), 0, 1));
  current.payload = { ...current.payload, ...(data.payload ?? {}) };
}

function upsertEdge(from, to, relation, confidence, evidenceHash) {
  if (!from || !to) return;
  const normalizedFrom = normalizeNodeRef(from);
  const normalizedTo = normalizeNodeRef(to);
  const id = `edge:${sha256(`${normalizedFrom}:${relation}:${normalizedTo}`).slice(0, 28)}`;
  const current = edges.get(id);
  if (!current) {
    edges.set(id, {
      kind: "dataflow_edge",
      id,
      from: normalizedFrom,
      to: normalizedTo,
      relation,
      confidence: clamp(Number(confidence ?? 0.5), 0, 1),
      evidenceHashes: evidenceHash ? [String(evidenceHash)] : [],
    });
    return;
  }
  current.confidence = Math.max(current.confidence, clamp(Number(confidence ?? 0.5), 0, 1));
  if (evidenceHash && !current.evidenceHashes.includes(String(evidenceHash))) current.evidenceHashes.push(String(evidenceHash));
}

function finalizeRecord(record) {
  const finalized = {
    ...record,
    generatedAt: startedAt,
  };
  finalized.recordHash = sha256(JSON.stringify({
    kind: finalized.kind,
    id: finalized.id,
    type: finalized.type,
    from: finalized.from,
    to: finalized.to,
    relation: finalized.relation,
    confidence: finalized.confidence,
    payload: finalized.payload,
    evidenceHashes: finalized.evidenceHashes,
  }));
  return finalized;
}

function buildSummary(sortedNodes, sortedEdges) {
  const byNodeType = countBy(sortedNodes, (node) => node.type);
  const byRelation = countBy(sortedEdges, (edge) => edge.relation);
  const graphProof = sha256(JSON.stringify({
    inputHash,
    nodes: sortedNodes.map((node) => node.recordHash),
    edges: sortedEdges.map((edge) => edge.recordHash),
  }));
  return {
    kind: "real_estate_living_dataflow_graph",
    schemaVersion: 1,
    status: "rebuilt",
    startedAt,
    finishedAt: new Date().toISOString(),
    storePath: storePath.replaceAll("\\", "/"),
    graphPath: graphJsonlPath.replaceAll("\\", "/"),
    latestPath: graphLatestPath.replaceAll("\\", "/"),
    inputHash,
    proofHash: graphProof,
    inputManifest,
    nodeCount: sortedNodes.length,
    edgeCount: sortedEdges.length,
    counters,
    byNodeType,
    byRelation,
    llmContract: {
      rule: "Use this graph before reading raw files. Nodes and edges are content-addressed; follow evidence hashes only when needed.",
      chain: "source -> raw -> event -> entity/signal -> intelPack -> score -> action -> memoryFact",
      incremental: "If inputHash is unchanged, Forge reuses this materialized graph.",
    },
  };
}

function buildInputManifest() {
  return Object.fromEntries(Object.entries(artifacts).map(([name, path]) => [name, fileManifest(path)]));
}

function fileManifest(path) {
  if (!existsSync(path)) return { exists: false, path: path.replaceAll("\\", "/"), bytes: 0, hash: "" };
  const bytes = readFileSync(path);
  const stat = statSync(path);
  return {
    exists: true,
    path: path.replaceAll("\\", "/"),
    bytes: bytes.length,
    modifiedMs: Math.floor(stat.mtimeMs),
    hash: sha256(bytes),
  };
}

function readJsonl(path) {
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

function readRankedActions(path) {
  const value = readJson(path);
  if (Array.isArray(value)) return value;
  if (Array.isArray(value?.rankedActions)) return value.rankedActions;
  if (Array.isArray(value?.actions)) return value.actions;
  return [];
}

function readJson(path) {
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function normalizeNodeRef(id) {
  const value = String(id);
  if (value.includes(":")) return value;
  if (/^[a-f0-9]{32,}$/i.test(value)) return `hash:${value}`;
  return `entity:${value}`;
}

function pick(value, keys) {
  const out = {};
  for (const key of keys) {
    if (value?.[key] !== undefined) out[key] = value[key];
  }
  return out;
}

function countBy(items, fn) {
  const out = {};
  for (const item of items) {
    const key = fn(item) ?? "unknown";
    out[key] = (out[key] ?? 0) + 1;
  }
  return out;
}

function hashAny(value) {
  return sha256(JSON.stringify(value ?? null));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function clamp(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, value));
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}
