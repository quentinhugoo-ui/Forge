import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const packsPathArg = argValue("--packs");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePathFromPacks(packsPathArg);
const limit = Number(argValue("--limit") ?? 512);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const packsPath = packsPathArg ?? join(dataDir, "intel_packs.jsonl");
const seedsPath = join(dataDir, "kasm_metric_seeds.jsonl");
const latestPath = join(dataDir, "kasm_seed_builder_latest.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${packsPath}:${limit}`).slice(0, 16);
const failures = [];

if (!existsSync(packsPath)) fail(`intel packs file not found: ${packsPath}`);
mkdirSync(dataDir, { recursive: true });
writeFileSync(seedsPath, "");

const packs = readJsonl(packsPath).slice(0, limit);
const seeds = packs.flatMap((pack) => buildSeedsForPack(pack));

for (const seed of seeds) appendFileSync(seedsPath, `${JSON.stringify(finalizeSeed(seed))}\n`);

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function buildSeedsForPack(pack) {
  const vector = featureVectorForPack(pack);
  const priorityScore = priorityScoreForVector(vector);
  const base = {
    kind: "real_estate_kasm_metric_seed",
    schemaVersion: 1,
    runId,
    packTheme: pack.theme,
    packProofHash: pack.proofHash,
    packStatus: pack.summary?.status ?? "unknown",
    sourceIds: pack.sourceIds ?? [],
    evidenceRefs: pack.evidenceRefs ?? [],
    graphRefs: pack.graphRefs ?? [],
    featureVector: vector,
    priorityScore,
    simulationHints: simulationHintsForPack(pack, vector, priorityScore),
  };
  return [
    {
      ...base,
      seedType: "pack_priority",
      metricKey: `${pack.theme}.priority_score`,
      metricValue: priorityScore,
      unit: "score_0_1",
      confidence: confidenceForPack(pack, vector),
    },
    {
      ...base,
      seedType: "data_quality",
      metricKey: `${pack.theme}.data_quality_score`,
      metricValue: vector.evidence_quality_score,
      unit: "score_0_1",
      confidence: 0.82,
    },
    {
      ...base,
      seedType: "actionability",
      metricKey: `${pack.theme}.actionability_score`,
      metricValue: vector.actionability_score,
      unit: "score_0_1",
      confidence: confidenceForPack(pack, vector),
    },
  ];
}

function featureVectorForPack(pack) {
  const metrics = pack.metrics ?? {};
  const quality = pack.quality ?? {};
  const gapCount = pack.gaps?.length ?? 0;
  const signalCount = pack.signals?.length ?? 0;
  const actionCount = pack.recommendedActions?.length ?? 0;
  const sourceDensity = clamp((metrics.sourceCount ?? 0) / 4, 0, 1);
  const evidenceQuality = clamp(((quality.evidenceScore ?? 0) * 0.4) + ((quality.fieldScore ?? 0) * 0.25) + ((quality.graphScore ?? 0) * 0.25) + ((quality.sourceScore ?? 0) * 0.1), 0, 1);
  const graphDensity = clamp(((metrics.nodeCount ?? 0) + (metrics.edgeCount ?? 0) + ((metrics.clusterCount ?? 0) * 2)) / 24, 0, 1);
  const signalStrength = clamp((signalCount * 0.18) + ((metrics.clusterCount ?? 0) * 0.12) + ((metrics.recordCount ?? 0) * 0.03), 0, 1);
  const dataGapPenalty = clamp((quality.gapPenalty ?? 0) + (gapCount * 0.06), 0, 1);
  const actionability = clamp((quality.usabilityScore ?? 0) * 0.45 + signalStrength * 0.25 + graphDensity * 0.15 + Math.min(1, actionCount / 3) * 0.15 - dataGapPenalty * 0.25, 0, 1);
  return {
    source_density_score: round(sourceDensity),
    evidence_quality_score: round(evidenceQuality),
    graph_density_score: round(graphDensity),
    market_signal_score: round(pack.theme === "market" ? signalStrength : signalStrength * 0.45),
    local_signal_score: round(pack.theme === "local_signal" ? signalStrength : signalStrength * 0.35),
    economic_signal_score: round(pack.theme === "economic" ? signalStrength : signalStrength * 0.35),
    data_gap_penalty: round(dataGapPenalty),
    actionability_score: round(actionability),
  };
}

function priorityScoreForVector(vector) {
  return round(clamp(
    vector.actionability_score * 0.34
      + vector.evidence_quality_score * 0.22
      + vector.graph_density_score * 0.18
      + vector.source_density_score * 0.12
      + Math.max(vector.market_signal_score, vector.local_signal_score, vector.economic_signal_score) * 0.14
      - vector.data_gap_penalty * 0.22,
    0,
    1,
  ));
}

function confidenceForPack(pack, vector) {
  return round(clamp((pack.quality?.usabilityScore ?? 0) * 0.55 + vector.evidence_quality_score * 0.35 + vector.source_density_score * 0.1, 0.05, 0.98));
}

function simulationHintsForPack(pack, vector, priorityScore) {
  const hints = [];
  if (priorityScore >= 0.65) hints.push("deep_crawl_candidate");
  if (vector.data_gap_penalty >= 0.35) hints.push("fill_data_gaps_first");
  if (vector.graph_density_score >= 0.5) hints.push("graph_join_simulation");
  if (vector.local_signal_score >= 0.35) hints.push("local_signal_watch");
  if (pack.llmContract?.canAnswer) hints.push("llm_ready_compact_context");
  if (!hints.length) hints.push("low_priority_monitor");
  return hints;
}

function finalizeSeed(seed) {
  const finalized = {
    ...seed,
    generatedAt: new Date().toISOString(),
  };
  finalized.seedHash = sha256(JSON.stringify({
    seedType: finalized.seedType,
    metricKey: finalized.metricKey,
    metricValue: finalized.metricValue,
    featureVector: finalized.featureVector,
    packProofHash: finalized.packProofHash,
    evidenceRefs: finalized.evidenceRefs,
    graphRefs: finalized.graphRefs,
  }));
  return finalized;
}

function buildSummary() {
  const byType = {};
  const byTheme = {};
  let maxPriority = 0;
  for (const seed of seeds) {
    byType[seed.seedType] = (byType[seed.seedType] ?? 0) + 1;
    byTheme[seed.packTheme] = (byTheme[seed.packTheme] ?? 0) + 1;
    maxPriority = Math.max(maxPriority, seed.priorityScore ?? 0);
  }
  const summary = {
    kind: "real_estate_kasm_seed_builder_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    packsPath: packsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    seedsPath: seedsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    packCount: packs.length,
    seedCount: seeds.length,
    maxPriorityScore: round(maxPriority),
    byType,
    byTheme,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    packCount: summary.packCount,
    seedCount: summary.seedCount,
    maxPriorityScore: summary.maxPriorityScore,
    byType,
    byTheme,
    failures,
    seeds: seeds.map((seed) => `${seed.metricKey}:${seed.metricValue}:${seed.packProofHash}`).sort(),
  }));
  return summary;
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

function round(value) {
  return Math.round(value * 10000) / 10000;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? value : 0));
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function inferStorePathFromPacks(path) {
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/intel_packs.jsonl";
  if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  return dirname(dirname(dirname(path)));
}

function fail(message) {
  console.error(`[real-estate-kasm-seed-builder] ${message}`);
  process.exit(1);
}

function sha256(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}
