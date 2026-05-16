import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const seedsPathArg = argValue("--seeds");
const storePath = argValue("--store") ?? process.env.FORGE_STORE_PATH ?? inferStorePathFromSeeds(seedsPathArg);
const limit = Number(argValue("--limit") ?? 512);
const rustEnabled = !args.has("--no-rust");
const rustRepeat = Number(argValue("--rust-repeat") ?? 2);
const rustFocus = argValue("--rust-focus") ?? "scenario-dag";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

const harvesterDir = join(storePath, "real-estate-harvester");
const dataDir = join(harvesterDir, "data");
const seedsPath = seedsPathArg ?? join(dataDir, "kasm_metric_seeds.jsonl");
const resultsPath = join(dataDir, "kasm_simulation_results.jsonl");
const rankedActionsPath = join(dataDir, "ranked_actions.json");
const latestPath = join(dataDir, "kasm_simulator_latest.json");
const rustComputePath = join(dataDir, "kasm_rust_compute.json");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${seedsPath}:${limit}`).slice(0, 16);
const failures = [];

if (!existsSync(seedsPath)) fail(`KASM seeds file not found: ${seedsPath}`);
mkdirSync(dataDir, { recursive: true });
writeFileSync(resultsPath, "");

const seeds = readJsonl(seedsPath).slice(0, limit);
const packContexts = groupSeedsByPack(seeds);
const rustCompute = rustEnabled ? runRustKasmCompute(packContexts) : disabledRustCompute();
const simulations = [];

for (const context of packContexts.values()) {
  for (const scenario of scenariosForContext(context)) {
    simulations.push(simulateScenario(context, scenario));
  }
}

const rankedActions = rankActions(simulations);
for (const result of simulations) appendFileSync(resultsPath, `${JSON.stringify(finalizeSimulation(result))}\n`);

const summary = buildSummary();
writeFileSync(rankedActionsPath, `${JSON.stringify({
  kind: "real_estate_ranked_actions",
  runId,
  generatedAt: new Date().toISOString(),
  computeEngine: rustCompute.engine,
  rustComputeRef: rustCompute.proofHash,
  workItems: rustCompute.workItems,
  rankedActions,
  proofHash: sha256(JSON.stringify(rankedActions.map((action) => action.actionHash))),
}, null, 2)}\n`);
writeFileSync(latestPath, `${JSON.stringify(summary, null, 2)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
if (failures.some((failure) => failure.severity === "error")) process.exit(1);

function runRustKasmCompute(contexts) {
  const properties = Number(argValue("--rust-properties") ?? Math.max(24_000, contexts.size * 4096, seeds.length * 1024));
  const scenarios = Number(argValue("--rust-scenarios") ?? Math.max(512, contexts.size * 96, seeds.length * 32));
  const candidateLimit = Number(argValue("--rust-candidates") ?? Math.min(properties, Math.max(8_000, contexts.size * 2048, seeds.length * 512)));
  const started = new Date().toISOString();
  const cargoArgs = [
    "run",
    "--quiet",
    "--example",
    "lab_runner_immo",
    "--",
    "--properties",
    String(properties),
    "--scenarios",
    String(scenarios),
    "--repeat",
    String(Math.max(2, rustRepeat)),
    "--candidate-limit",
    String(candidateLimit),
    "--focus",
    rustFocus,
    "--seeds",
    seedsPath,
  ];
  const result = spawnSync("cargo", cargoArgs, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 32,
    env: {
      ...process.env,
      CARGO_TARGET_DIR: argValue("--rust-target-dir") ?? join(root, "target"),
    },
  });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const parsed = parseRustLabOutput(stdout);
  const compute = {
    kind: "real_estate_kasm_rust_compute",
    engine: "cargo:lab_runner_immo",
    startedAt: started,
    finishedAt: new Date().toISOString(),
    status: result.status === 0 ? "ok" : "failed",
    exitCode: result.status,
    command: `cargo ${cargoArgs.join(" ")}`,
    properties,
    scenarios,
    repeat: Math.max(2, rustRepeat),
    candidateLimit,
    focus: rustFocus,
    metrics: parsed.metrics,
    horizons: parsed.horizons,
    workItems: parsed.workItems,
    bestScore: parsed.bestScore,
    bestSellerProbability: parsed.bestSellerProbability,
    bestExpectedFee: parsed.bestExpectedFee,
    bestSignal: parsed.bestSignal,
    intelPackRef: parsed.intelPackRef,
    evidenceRef: parsed.evidenceRef,
    speedupX: parsed.speedupX,
    warmAvoidedUnits: parsed.warmAvoidedUnits,
    stdoutHash: sha256(stdout),
    stderrHash: sha256(stderr),
    stdoutTail: tailLines(stdout, 24),
    stderrTail: tailLines(stderr, 12),
  };
  compute.proofHash = sha256(JSON.stringify({
    engine: compute.engine,
    status: compute.status,
    properties,
    scenarios,
    repeat: compute.repeat,
    candidateLimit,
    focus: rustFocus,
    metrics: compute.metrics,
    horizons: compute.horizons,
    workItems: compute.workItems,
    bestScore: compute.bestScore,
    bestSellerProbability: compute.bestSellerProbability,
    bestExpectedFee: compute.bestExpectedFee,
    bestSignal: compute.bestSignal,
    stdoutHash: compute.stdoutHash,
  }));
  writeFileSync(rustComputePath, `${JSON.stringify(compute, null, 2)}\n`);
  if (compute.status !== "ok") {
    failures.push({
      severity: "error",
      code: "rust_kasm_compute_failed",
      detail: compute.stderrTail || compute.stdoutTail || "cargo lab_runner_immo failed",
    });
  }
  return compute;
}

function disabledRustCompute() {
  const compute = {
    kind: "real_estate_kasm_rust_compute",
    engine: "disabled",
    status: "disabled",
    workItems: 0,
    bestScore: 0,
    proofHash: sha256("real-estate-kasm-rust-disabled"),
  };
  writeFileSync(rustComputePath, `${JSON.stringify(compute, null, 2)}\n`);
  return compute;
}

function parseRustLabOutput(stdout) {
  const first = stdout.match(/properties=(\d+)\s+metrics=(\d+)\s+scenarios=(\d+)\s+horizons=(\d+)/);
  const passLines = [...stdout.matchAll(/work_items=(\d+).*?best_property=(\d+).*?zone=(\d+).*?scenario=(\d+).*?horizon=(\d+)d\s+score=([0-9.]+)\s+seller_prob=([0-9.]+)\s+expected_fee=([0-9.]+)\s+strongest_signal=([^\s]+)\s+evidence=([^\s]+)\s+intel_pack=([^\s]+)/g)];
  const bestPass = passLines
    .map((match) => ({
      workItems: Number(match[1]),
      bestPropertyId: Number(match[2]),
      bestZoneId: Number(match[3]),
      bestScenarioId: Number(match[4]),
      bestHorizonDays: Number(match[5]),
      bestScore: Number(match[6]),
      bestSellerProbability: Number(match[7]),
      bestExpectedFee: Number(match[8]),
      bestSignal: match[9],
      evidenceRef: match[10],
      intelPackRef: match[11],
    }))
    .sort((a, b) => b.bestScore - a.bestScore)[0] ?? {};
  const summary = stdout.match(/summary target=.*?speedup_x=([0-9.]+).*?warm_avoided_units=(\d+)/);
  return {
    metrics: Number(first?.[2] ?? 0),
    horizons: Number(first?.[4] ?? 0),
    workItems: Number(bestPass.workItems ?? 0),
    bestScore: Number(bestPass.bestScore ?? 0),
    bestSellerProbability: Number(bestPass.bestSellerProbability ?? 0),
    bestExpectedFee: Number(bestPass.bestExpectedFee ?? 0),
    bestSignal: bestPass.bestSignal ?? "",
    evidenceRef: bestPass.evidenceRef ?? "",
    intelPackRef: bestPass.intelPackRef ?? "",
    speedupX: summary ? Number(summary[1]) : 0,
    warmAvoidedUnits: summary ? Number(summary[2]) : 0,
  };
}

function groupSeedsByPack(inputSeeds) {
  const out = new Map();
  for (const seed of inputSeeds) {
    const key = `${seed.packTheme}:${seed.packProofHash}`;
    const current = out.get(key) ?? {
      packTheme: seed.packTheme,
      packProofHash: seed.packProofHash,
      packStatus: seed.packStatus,
      sourceIds: seed.sourceIds ?? [],
      evidenceRefs: seed.evidenceRefs ?? [],
      graphRefs: seed.graphRefs ?? [],
      featureVector: seed.featureVector ?? {},
      priorityScore: seed.priorityScore ?? 0,
      simulationHints: new Set(),
      seeds: [],
    };
    current.seeds.push(seed);
    current.priorityScore = Math.max(current.priorityScore, seed.priorityScore ?? 0);
    current.featureVector = mergeVector(current.featureVector, seed.featureVector ?? {});
    for (const hint of seed.simulationHints ?? []) current.simulationHints.add(hint);
    out.set(key, current);
  }
  return out;
}

function scenariosForContext(context) {
  const base = [
    "deep_crawl",
    "fill_gap",
    "graph_join",
    "local_watch",
    "llm_ready",
  ];
  const selected = base.filter((scenario) => {
    const vector = context.featureVector;
    if (scenario === "deep_crawl") return context.priorityScore >= 0.35 || vector.evidence_quality_score >= 0.55;
    if (scenario === "fill_gap") return vector.data_gap_penalty >= 0.2;
    if (scenario === "graph_join") return vector.graph_density_score >= 0.15;
    if (scenario === "local_watch") return context.packTheme === "local_signal" || vector.local_signal_score >= 0.2;
    if (scenario === "llm_ready") return context.simulationHints.has("llm_ready_compact_context") || vector.actionability_score >= 0.35;
    return true;
  });
  return selected.length ? selected : ["llm_ready"];
}

function simulateScenario(context, scenario) {
  const vector = context.featureVector;
  const profile = scenarioProfile(scenario);
  const computeBoost = clamp(Math.log10((rustCompute.workItems ?? 1) + 1) / 10, 0, 0.2);
  const expectedGain = clamp(
    profile.gainBase
      + vector.actionability_score * profile.actionabilityWeight
      + vector.evidence_quality_score * profile.evidenceWeight
      + vector.graph_density_score * profile.graphWeight
      + Math.max(vector.market_signal_score, vector.local_signal_score, vector.economic_signal_score) * profile.signalWeight,
    0,
    1,
  );
  const cost = clamp(profile.costBase + vector.data_gap_penalty * profile.gapCost + (1 - vector.source_density_score) * 0.08 + computeBoost * 0.22, 0, 1);
  const risk = clamp(profile.riskBase + vector.data_gap_penalty * profile.gapRisk + (1 - vector.evidence_quality_score) * 0.12, 0, 1);
  const explorationBonus = context.simulationHints.has("deep_crawl_candidate") || context.simulationHints.has("local_signal_watch") ? 0.05 : 0;
  const utilityScore = round(clamp(expectedGain * 0.47 + context.priorityScore * 0.22 + rustCompute.bestScore * 0.18 + explorationBonus - cost * 0.14 - risk * 0.15, 0, 1));
  return {
    kind: "real_estate_kasm_simulation_result",
    schemaVersion: 1,
    runId,
    scenario,
    actionId: actionIdForScenario(context, scenario),
    packTheme: context.packTheme,
    packProofHash: context.packProofHash,
    sourceIds: context.sourceIds,
    evidenceRefs: context.evidenceRefs.slice(0, 12),
    graphRefs: context.graphRefs.slice(0, 12),
    inputSeedHashes: context.seeds.map((seed) => seed.seedHash).filter(Boolean),
    featureVector: vector,
    expectedGain: round(expectedGain),
    cost: round(cost),
    risk: round(risk),
    rustComputeRef: rustCompute.proofHash,
    workItems: rustCompute.workItems,
    utilityScore,
    decision: decisionForScore(utilityScore, risk),
    rationale: rationaleForScenario(scenario, context, { expectedGain, cost, risk, utilityScore }),
  };
}

function scenarioProfile(scenario) {
  return {
    deep_crawl: { gainBase: 0.18, costBase: 0.38, riskBase: 0.22, actionabilityWeight: 0.28, evidenceWeight: 0.18, graphWeight: 0.1, signalWeight: 0.26, gapCost: 0.22, gapRisk: 0.16 },
    fill_gap: { gainBase: 0.14, costBase: 0.22, riskBase: 0.12, actionabilityWeight: 0.12, evidenceWeight: 0.1, graphWeight: 0.08, signalWeight: 0.08, gapCost: 0.1, gapRisk: 0.08 },
    graph_join: { gainBase: 0.16, costBase: 0.26, riskBase: 0.16, actionabilityWeight: 0.18, evidenceWeight: 0.2, graphWeight: 0.38, signalWeight: 0.12, gapCost: 0.14, gapRisk: 0.12 },
    local_watch: { gainBase: 0.15, costBase: 0.14, riskBase: 0.1, actionabilityWeight: 0.18, evidenceWeight: 0.12, graphWeight: 0.06, signalWeight: 0.34, gapCost: 0.1, gapRisk: 0.1 },
    llm_ready: { gainBase: 0.2, costBase: 0.08, riskBase: 0.14, actionabilityWeight: 0.34, evidenceWeight: 0.26, graphWeight: 0.08, signalWeight: 0.12, gapCost: 0.08, gapRisk: 0.2 },
  }[scenario];
}

function rankActions(results) {
  return results
    .map((result) => {
      const action = {
        actionId: result.actionId,
        scenario: result.scenario,
        packTheme: result.packTheme,
        utilityScore: result.utilityScore,
        expectedGain: result.expectedGain,
        cost: result.cost,
        risk: result.risk,
        rustComputeRef: result.rustComputeRef,
        workItems: result.workItems,
        decision: result.decision,
        sourceIds: result.sourceIds,
        evidenceRefs: result.evidenceRefs,
        graphRefs: result.graphRefs,
        rationale: result.rationale,
        inputSeedHashes: result.inputSeedHashes,
      };
      action.actionHash = sha256(JSON.stringify(action));
      return action;
    })
    .sort((a, b) => b.utilityScore - a.utilityScore || a.risk - b.risk || a.cost - b.cost)
    .slice(0, 25);
}

function finalizeSimulation(result) {
  const finalized = {
    ...result,
    simulatedAt: new Date().toISOString(),
  };
  finalized.resultHash = sha256(JSON.stringify({
    scenario: finalized.scenario,
    actionId: finalized.actionId,
    utilityScore: finalized.utilityScore,
    expectedGain: finalized.expectedGain,
    cost: finalized.cost,
    risk: finalized.risk,
    rustComputeRef: finalized.rustComputeRef,
    workItems: finalized.workItems,
    featureVector: finalized.featureVector,
    inputSeedHashes: finalized.inputSeedHashes,
  }));
  return finalized;
}

function buildSummary() {
  const byScenario = {};
  const byDecision = {};
  let maxUtility = 0;
  for (const result of simulations) {
    byScenario[result.scenario] = (byScenario[result.scenario] ?? 0) + 1;
    byDecision[result.decision] = (byDecision[result.decision] ?? 0) + 1;
    maxUtility = Math.max(maxUtility, result.utilityScore);
  }
  const summary = {
    kind: "real_estate_kasm_simulator_summary",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    seedsPath: seedsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    resultsPath: resultsPath.replaceAll("\\", "/"),
    rankedActionsPath: rankedActionsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    rustComputePath: rustComputePath.replaceAll("\\", "/"),
    computeEngine: rustCompute.engine,
    rustWorkItems: rustCompute.workItems,
    rustBestScore: round(rustCompute.bestScore),
    rustSpeedupX: rustCompute.speedupX,
    rustProofHash: rustCompute.proofHash,
    seedCount: seeds.length,
    packContextCount: packContexts.size,
    simulationCount: simulations.length,
    rankedActionCount: rankedActions.length,
    maxUtilityScore: round(maxUtility),
    byScenario,
    byDecision,
    failures,
  };
  summary.proofHash = sha256(JSON.stringify({
    seedCount: summary.seedCount,
    packContextCount: summary.packContextCount,
    simulationCount: summary.simulationCount,
    rankedActionCount: summary.rankedActionCount,
    maxUtilityScore: summary.maxUtilityScore,
    byScenario,
    byDecision,
    failures,
    actions: rankedActions.map((action) => action.actionHash),
    rustProofHash: rustCompute.proofHash,
    rustWorkItems: rustCompute.workItems,
  }));
  return summary;
}

function actionIdForScenario(context, scenario) {
  return `${scenario}:${context.packTheme}:${sha256(context.packProofHash).slice(0, 16)}`;
}

function decisionForScore(score, risk) {
  if (score >= 0.62 && risk <= 0.55) return "run_now";
  if (score >= 0.42) return "queue_next";
  if (risk >= 0.55) return "needs_guardrail";
  return "monitor";
}

function rationaleForScenario(scenario, context, scores) {
  const labels = {
    deep_crawl: "Approfondir les sources les plus prometteuses.",
    fill_gap: "Combler les trous de donnees avant decision.",
    graph_join: "Tenter des jointures graphe supplementaires.",
    local_watch: "Surveiller le signal local dans le temps.",
    llm_ready: "Exposer un contexte compact au LLM.",
  };
  return {
    label: labels[scenario],
    packTheme: context.packTheme,
    hints: [...context.simulationHints],
    expectedGain: round(scores.expectedGain),
    cost: round(scores.cost),
    risk: round(scores.risk),
    utilityScore: round(scores.utilityScore),
  };
}

function mergeVector(left, right) {
  const out = { ...left };
  for (const [key, value] of Object.entries(right)) out[key] = Math.max(out[key] ?? 0, value ?? 0);
  return out;
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

function inferStorePathFromSeeds(path) {
  if (!path) return ".";
  const normalized = path.replaceAll("\\", "/");
  const marker = "/real-estate-harvester/data/kasm_metric_seeds.jsonl";
  if (normalized.endsWith(marker)) return normalized.slice(0, -marker.length);
  return dirname(dirname(dirname(path)));
}

function fail(message) {
  console.error(`[real-estate-kasm-simulator] ${message}`);
  process.exit(1);
}

function tailLines(value, count) {
  return String(value)
    .split(/\r?\n/)
    .filter(Boolean)
    .slice(-count)
    .join("\n");
}

function sha256(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}
