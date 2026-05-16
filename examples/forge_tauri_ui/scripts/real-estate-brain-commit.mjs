import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePath(argValue("--ranked"));
const limit = Number(argValue("--limit") ?? 64);

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const rankedPath = argValue("--ranked") ?? join(dataDir, "ranked_actions.json");
const rustComputePath = argValue("--compute") ?? join(dataDir, "kasm_rust_compute.json");
const intelPacksPath = argValue("--packs") ?? join(dataDir, "intel_packs.jsonl");
const commitsPath = join(dataDir, "real_estate_memory_commits.jsonl");
const latestPath = join(dataDir, "real_estate_memory_latest.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${rankedPath}:${rustComputePath}:${intelPacksPath}`).slice(0, 16);
const failures = [];

if (!existsSync(rankedPath)) fail(`ranked actions file not found: ${rankedPath}`);
if (!existsSync(rustComputePath)) fail(`Rust compute file not found: ${rustComputePath}`);
mkdirSync(dataDir, { recursive: true });
writeFileSync(commitsPath, "");

const ranked = readJson(rankedPath);
const rustCompute = readJson(rustComputePath);
const intelPacks = existsSync(intelPacksPath) ? readJsonl(intelPacksPath) : [];
const packIndex = buildPackIndex(intelPacks);
const actions = (ranked.rankedActions ?? []).slice(0, limit);
const commits = actions.map((action, index) => finalizeCommit(buildCommit(action, index)));

for (const commit of commits) appendFileSync(commitsPath, `${JSON.stringify(commit)}\n`);

const summary = buildSummary();
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function buildCommit(action, index) {
  const pack = packForAction(action);
  const confidence = confidenceForAction(action);
  const noteText = brainNoteText(action, pack, confidence);
  const textHash = sha256(noteText);
  return {
    kind: "real_estate_memory_commit",
    schemaVersion: 1,
    runId,
    scope: "agence_immo",
    section: "real-estate",
    memoryLayer: "semantic",
    source: "real_estate_kasm_pipeline",
    commitKind: "ranked_action",
    rank: index + 1,
    actionId: action.actionId,
    scenario: action.scenario,
    decision: action.decision,
    confidence,
    trustScore: confidence,
    textHash,
    noteText,
    evidence: {
      actionHash: action.actionHash,
      rankedActionsProofHash: ranked.proofHash,
      rustComputeProofHash: rustCompute.proofHash,
      rustComputeRef: action.rustComputeRef ?? ranked.rustComputeRef,
      workItems: action.workItems ?? ranked.workItems ?? rustCompute.workItems,
      packTheme: action.packTheme,
      sourceIds: action.sourceIds ?? [],
      evidenceRefs: action.evidenceRefs ?? [],
      graphRefs: action.graphRefs ?? [],
      packProofHash: pack?.proofHash ?? pack?.packProofHash ?? null,
      packSummary: pack?.summary ?? null,
    },
    brainCommitRequest: {
      scope: "agence_immo",
      section: "real-estate",
      kind: "ranked_action",
      source: "real_estate_kasm_pipeline",
      confidence,
      text: noteText,
    },
    toolRouting: toolRoutingForAction(action),
  };
}

function brainNoteText(action, pack, confidence) {
  const label = action.rationale?.label ?? "Action classee par KASM.";
  const evidenceCount = (action.evidenceRefs ?? []).length;
  const graphCount = (action.graphRefs ?? []).length;
  const packStatus = pack?.summary?.status ?? "unknown";
  const topSignals = pack?.summary?.topSignals?.slice(0, 3).join(" | ") || rustCompute.bestSignal || "none";
  return [
    "forge-brain-llm-note-v1",
    "scope=agence-immo",
    "kind=ranked_action",
    "memory_layer=semantic",
    "source=real_estate_kasm_pipeline",
    "verification_status=anchored",
    `trust_score=${confidence.toFixed(3)}`,
    `confidence=${confidence.toFixed(3)}`,
    `action_id=${action.actionId}`,
    `scenario=${action.scenario}`,
    `decision=${action.decision}`,
    `utility_score=${num(action.utilityScore)}`,
    `expected_gain=${num(action.expectedGain)}`,
    `cost=${num(action.cost)}`,
    `risk=${num(action.risk)}`,
    `work_items=${action.workItems ?? ranked.workItems ?? rustCompute.workItems ?? 0}`,
    `rust_proof_hash=${rustCompute.proofHash ?? ""}`,
    `ranked_actions_hash=${ranked.proofHash ?? ""}`,
    `action_hash=${action.actionHash ?? ""}`,
    `pack_theme=${action.packTheme ?? ""}`,
    `pack_status=${packStatus}`,
    `evidence_refs=${evidenceCount}`,
    `graph_refs=${graphCount}`,
    `top_signals=${topSignals}`,
    "",
    `${label} Utiliser cette note avant de relancer un calcul LLM natif. Les raws restent locaux; citer les preuves par actionHash, rustProofHash et rankedActionsHash.`,
  ].join("\n");
}

function finalizeCommit(commit) {
  commit.commitHash = sha256(JSON.stringify({
    scope: commit.scope,
    memoryLayer: commit.memoryLayer,
    actionId: commit.actionId,
    scenario: commit.scenario,
    confidence: commit.confidence,
    textHash: commit.textHash,
    evidence: commit.evidence,
  }));
  commit.brainRef = `refs/brain/llm/agence-immo/ranked-action/${commit.commitHash.slice(0, 24)}`;
  commit.createdAt = new Date().toISOString();
  return commit;
}

function confidenceForAction(action) {
  const utility = Number(action.utilityScore ?? 0);
  const risk = Number(action.risk ?? 0);
  const computeTrust = rustCompute.status === "ok" && rustCompute.workItems > 0 ? 0.16 : 0;
  const proofTrust = action.actionHash && rustCompute.proofHash ? 0.12 : 0;
  return round(clamp(0.42 + utility * 0.34 - risk * 0.12 + computeTrust + proofTrust, 0.1, 0.96));
}

function toolRoutingForAction(action) {
  const scenarioMap = {
    deep_crawl: ["/marche_veille_", "/concurrence_"],
    fill_gap: ["/marche_veille_", "/back_office_"],
    graph_join: ["/pilotage_agence_", "/estimation_"],
    local_watch: ["/veille_locale_", "/marche_veille_"],
    llm_ready: ["/pilotage_agence_", "/rapport_vendeur_"],
  };
  return {
    primaryCommands: scenarioMap[action.scenario] ?? ["/pilotage_agence_"],
    brainFirst: true,
    recallScope: "agence_immo",
    memoryLayer: "semantic",
  };
}

function packForAction(action) {
  const byHash = action.packProofHash ? packIndex.byProofHash.get(action.packProofHash) : null;
  if (byHash) return byHash;
  const byTheme = action.packTheme ? packIndex.byTheme.get(action.packTheme) : null;
  return byTheme ?? null;
}

function buildPackIndex(packs) {
  const byProofHash = new Map();
  const byTheme = new Map();
  for (const pack of packs) {
    if (pack.proofHash) byProofHash.set(pack.proofHash, pack);
    if (pack.packProofHash) byProofHash.set(pack.packProofHash, pack);
    if (pack.theme && !byTheme.has(pack.theme)) byTheme.set(pack.theme, pack);
  }
  return { byProofHash, byTheme };
}

function buildSummary() {
  const byScenario = {};
  const byDecision = {};
  for (const commit of commits) {
    byScenario[commit.scenario] = (byScenario[commit.scenario] ?? 0) + 1;
    byDecision[commit.decision] = (byDecision[commit.decision] ?? 0) + 1;
  }
  const summary = {
    kind: "real_estate_memory_commit_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    rankedPath: rankedPath.replaceAll("\\", "/"),
    rustComputePath: rustComputePath.replaceAll("\\", "/"),
    intelPacksPath: intelPacksPath.replaceAll("\\", "/"),
    commitsPath: commitsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    rankedActionCount: ranked.rankedActions?.length ?? 0,
    commitCount: commits.length,
    maxConfidence: round(Math.max(0, ...commits.map((commit) => commit.confidence))),
    rustWorkItems: rustCompute.workItems ?? 0,
    rustProofHash: rustCompute.proofHash ?? "",
    byScenario,
    byDecision,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    rankedProofHash: ranked.proofHash,
    rustProofHash: rustCompute.proofHash,
    commitHashes: commits.map((commit) => commit.commitHash),
    byScenario,
    byDecision,
    failures,
  }));
  return summary;
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`cannot read JSON ${path}: ${error.message}`);
  }
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
        failures.push({ severity: "warning", code: "jsonl_decode_failed", detail: `${path} line ${index + 1}: ${error.message}` });
        return null;
      }
    })
    .filter(Boolean);
}

function num(value) {
  return Number.isFinite(Number(value)) ? Number(value).toFixed(4) : "0.0000";
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

function inferStorePath(path) {
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/ranked_actions.json";
  if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  return dirname(dirname(dirname(path)));
}

function fail(message) {
  console.error(`[real-estate-brain-commit] ${message}`);
  process.exit(1);
}

function sha256(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}
