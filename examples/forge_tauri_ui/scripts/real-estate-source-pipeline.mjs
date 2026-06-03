import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scriptsDir = join(root, "scripts");
const defaultRegistryPath = join(root, "source-registry", "real-estate-public-sources.json");
const defaultAdaptersPath = join(root, "source-registry", "real-estate-parser-adapters.json");
const defaultToolCellsPath = join(root, "source-registry", "real-estate-tool-cells.json");
const defaultStorePath = join(root, ".forge-data");

const args = new Set(process.argv.slice(2));
const live = args.has("--live");
const pretty = args.has("--pretty");
const registryPath = resolve(argValue("--registry") ?? defaultRegistryPath);
const adaptersPath = resolve(argValue("--adapters") ?? defaultAdaptersPath);
const toolCellsPath = resolve(argValue("--tool-cells") ?? defaultToolCellsPath);
const storePath = resolve(argValue("--store") ?? process.env.FORGE_STORE_PATH ?? defaultStorePath);
const timeoutMs = Number(argValue("--timeout-ms") ?? 8000);
const maxBytes = Number(argValue("--max-bytes") ?? 1024 * 1024);
const discoveryBytes = Number(argValue("--discovery-bytes") ?? 512 * 1024);
const downloadLimit = Number(argValue("--download-limit") ?? 32);
const parserLimit = Number(argValue("--parser-limit") ?? 64);
const stages = new Set((argValue("--stages") ?? "audit,discovery,download,parse,resolve,intel,seeds,memory,dataflow,evidence").split(",").map((it) => it.trim()).filter(Boolean));

const dataDir = join(storePath, "real-estate-harvester", "data");
const sourceManifestPath = join(dataDir, "source_manifest.jsonl");
const rawDownloadsPath = join(dataDir, "raw_downloads.jsonl");
const normalizedEventsPath = join(dataDir, "normalized_events.jsonl");
const entityGraphPath = join(dataDir, "entity_graph.jsonl");
const intelPacksPath = join(dataDir, "intel_packs.jsonl");
const kasmMetricSeedsPath = join(dataDir, "kasm_metric_seeds.jsonl");
const kasmSimulationResultsPath = join(dataDir, "kasm_simulation_results.jsonl");
const rankedActionsPath = join(dataDir, "ranked_actions.json");
const kasmRustComputePath = join(dataDir, "kasm_rust_compute.json");
const memoryCommitsPath = join(dataDir, "real_estate_memory_commits.jsonl");
const livingDataflowGraphPath = join(dataDir, "living_dataflow_graph.json");
const livingDataflowGraphJsonlPath = join(dataDir, "living_dataflow_graph.jsonl");
const toolCellOutputsDir = join(dataDir, "tool_cell_outputs");
const evidenceMemoryFactsPath = join(dataDir, "evidence_memory_facts.jsonl");
const evidenceMemoryLatestPath = join(dataDir, "evidence_memory_latest.json");
const pipelineLatestPath = join(dataDir, "pipeline_run.json");
const pipelineLedgerPath = join(dataDir, "pipeline_runs.jsonl");

const startedAt = new Date().toISOString();
const runId = sha256(`${startedAt}:${registryPath}:${storePath}:${live}`).slice(0, 16);
const stepResults = [];

if (!existsSync(registryPath)) fail(`registry not found: ${registryPath}`);
if (!existsSync(adaptersPath)) fail(`parser adapters registry not found: ${adaptersPath}`);
mkdirSync(dataDir, { recursive: true });

if (stages.has("audit")) {
  stepResults.push(runStep("audit", "real-estate-source-audit.mjs", [
    `--registry=${registryPath}`,
    "--pretty",
    ...(live ? ["--live"] : []),
    `--timeout-ms=${timeoutMs}`,
    `--max-bytes=${Math.min(discoveryBytes, maxBytes)}`,
  ]));
}

if (stages.has("discovery")) {
  stepResults.push(runStep("discovery", "real-estate-source-discovery.mjs", [
    `--registry=${registryPath}`,
    `--store=${storePath}`,
    "--pretty",
    ...(live ? ["--live"] : []),
    `--timeout-ms=${timeoutMs}`,
    `--max-bytes=${discoveryBytes}`,
  ]));
}

if (stages.has("download")) {
  if (!existsSync(sourceManifestPath)) {
    stepResults.push(skippedStep("download", "missing_source_manifest", sourceManifestPath));
  } else {
    stepResults.push(runStep("download", "real-estate-raw-downloader.mjs", [
      `--manifest=${sourceManifestPath}`,
      `--store=${storePath}`,
      "--pretty",
      ...(live ? ["--live"] : []),
      `--limit=${downloadLimit}`,
      `--timeout-ms=${timeoutMs}`,
      `--max-bytes=${maxBytes}`,
    ]));
  }
}

if (stages.has("parse")) {
  if (!existsSync(rawDownloadsPath)) {
    stepResults.push(skippedStep("parse", "missing_raw_downloads", rawDownloadsPath));
  } else {
    stepResults.push(runStep("parse", "real-estate-parser-router.mjs", [
      `--downloads=${rawDownloadsPath}`,
      `--store=${storePath}`,
      `--adapters=${adaptersPath}`,
      "--pretty",
      `--limit=${parserLimit}`,
    ]));
  }
}

if (stages.has("resolve")) {
  if (!existsSync(normalizedEventsPath)) {
    stepResults.push(skippedStep("resolve", "missing_normalized_events", normalizedEventsPath));
  } else {
    stepResults.push(runStep("resolve", "real-estate-entity-resolver.mjs", [
      `--events=${normalizedEventsPath}`,
      `--store=${storePath}`,
      "--pretty",
      `--limit=${parserLimit}`,
    ]));
  }
}

if (stages.has("intel")) {
  if (!existsSync(entityGraphPath) || !existsSync(normalizedEventsPath)) {
    stepResults.push(skippedStep("intel", "missing_graph_or_events", `${entityGraphPath} | ${normalizedEventsPath}`));
  } else {
    stepResults.push(runStep("intel", "real-estate-intel-pack-builder.mjs", [
      `--graph=${entityGraphPath}`,
      `--events=${normalizedEventsPath}`,
      `--store=${storePath}`,
      "--pretty",
      `--limit=${parserLimit}`,
    ]));
  }
}

if (stages.has("seeds")) {
  if (!existsSync(intelPacksPath)) {
    stepResults.push(skippedStep("seeds", "missing_intel_packs", intelPacksPath));
  } else {
    stepResults.push(runStep("seeds", "real-estate-kasm-seed-builder.mjs", [
      `--packs=${intelPacksPath}`,
      `--store=${storePath}`,
      "--pretty",
      `--limit=${parserLimit}`,
    ]));
  }
}

if (stages.has("memory")) {
  if (!existsSync(rankedActionsPath) || !existsSync(kasmRustComputePath)) {
    stepResults.push(skippedStep("memory", "missing_ranked_actions_or_compute", `${rankedActionsPath} | ${kasmRustComputePath}`));
  } else {
    stepResults.push(runStep("memory", "real-estate-brain-commit.mjs", [
      `--ranked=${rankedActionsPath}`,
      `--compute=${kasmRustComputePath}`,
      `--packs=${intelPacksPath}`,
      `--store=${storePath}`,
      "--pretty",
      `--limit=${parserLimit}`,
    ]));
  }
}

if (stages.has("dataflow")) {
  stepResults.push(runStep("dataflow", "real-estate-living-dataflow-graph.mjs", [
    `--store=${storePath}`,
    "--pretty",
    `--limit=${parserLimit}`,
  ]));
}

if (stages.has("evidence")) {
  stepResults.push(runStep("evidence", "real-estate-evidence-memory-builder.mjs", [
    `--store=${storePath}`,
    "--pretty",
    `--limit=${parserLimit}`,
  ]));
}

const pipeline = buildPipelineReport();
writeFileSync(pipelineLatestPath, `${JSON.stringify(pipeline, null, 2)}\n`);
appendFileSync(pipelineLedgerPath, `${JSON.stringify(pipeline)}\n`);

if (pretty || !args.has("--quiet")) console.log(JSON.stringify(pipeline, null, pretty ? 2 : 0));
if (pipeline.status === "failed") process.exit(1);

function runStep(name, scriptName, scriptArgs) {
  const scriptPath = join(scriptsDir, scriptName);
  const started = new Date().toISOString();
  const result = spawnSync(process.execPath, [scriptPath, ...scriptArgs], {
    cwd: resolve(root, "..", ".."),
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 16,
  });
  const stdout = result.stdout?.trim() ?? "";
  const stderr = result.stderr?.trim() ?? "";
  const summary = parseJsonSummary(stdout);
  return {
    name,
    script: scriptName,
    status: result.status === 0 ? "ok" : "failed",
    exitCode: result.status,
    startedAt: started,
    finishedAt: new Date().toISOString(),
    proofHash: summary?.proofHash ?? sha256(`${stdout}\n${stderr}`),
    summary,
    stderr: stderr.slice(0, 4000),
  };
}

function skippedStep(name, reason, path) {
  return {
    name,
    script: "",
    status: "skipped",
    exitCode: 0,
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    proofHash: sha256(`${name}:${reason}:${path}`),
    summary: { reason, path: path.replaceAll("\\", "/") },
    stderr: "",
  };
}

function parseJsonSummary(stdout) {
  if (!stdout) return null;
  try {
    return JSON.parse(stdout);
  } catch {
    const start = stdout.indexOf("{");
    const end = stdout.lastIndexOf("}");
    if (start >= 0 && end > start) {
      try {
        return JSON.parse(stdout.slice(start, end + 1));
      } catch {
        return null;
      }
    }
    return null;
  }
}

function buildPipelineReport() {
  const failed = stepResults.some((step) => step.status === "failed");
  const summary = {
    kind: "real_estate_source_pipeline_run",
    runId,
    mode: live ? "live" : "plan_only",
    status: failed ? "failed" : "ok",
    startedAt,
    finishedAt: new Date().toISOString(),
    registryPath: registryPath.replaceAll("\\", "/"),
    adaptersPath: adaptersPath.replaceAll("\\", "/"),
    toolCellsPath: toolCellsPath.replaceAll("\\", "/"),
    storePath: storePath.replaceAll("\\", "/"),
    artifacts: {
      sourceManifest: sourceManifestPath.replaceAll("\\", "/"),
      rawDownloads: rawDownloadsPath.replaceAll("\\", "/"),
      normalizedEvents: normalizedEventsPath.replaceAll("\\", "/"),
      entityGraph: entityGraphPath.replaceAll("\\", "/"),
      intelPacks: intelPacksPath.replaceAll("\\", "/"),
      kasmMetricSeeds: kasmMetricSeedsPath.replaceAll("\\", "/"),
      kasmRustCompute: kasmRustComputePath.replaceAll("\\", "/"),
      kasmSimulationResults: kasmSimulationResultsPath.replaceAll("\\", "/"),
      rankedActions: rankedActionsPath.replaceAll("\\", "/"),
      memoryCommits: memoryCommitsPath.replaceAll("\\", "/"),
      livingDataflowGraph: livingDataflowGraphPath.replaceAll("\\", "/"),
      livingDataflowGraphJsonl: livingDataflowGraphJsonlPath.replaceAll("\\", "/"),
      toolCellOutputs: toolCellOutputsDir.replaceAll("\\", "/"),
      evidenceMemoryFacts: evidenceMemoryFactsPath.replaceAll("\\", "/"),
      evidenceMemoryLatest: evidenceMemoryLatestPath.replaceAll("\\", "/"),
      latest: pipelineLatestPath.replaceAll("\\", "/"),
      ledger: pipelineLedgerPath.replaceAll("\\", "/"),
    },
    budget: {
      timeoutMs,
      discoveryBytes,
      maxBytes,
      downloadLimit,
      parserLimit,
    },
    steps: stepResults,
  };
  summary.proofHash = sha256(JSON.stringify({
    mode: summary.mode,
    status: summary.status,
    registryPath: summary.registryPath,
    adaptersPath: summary.adaptersPath,
    artifacts: summary.artifacts,
    budget: summary.budget,
    steps: stepResults.map((step) => ({
      name: step.name,
      status: step.status,
      proofHash: step.proofHash,
      summaryProofHash: step.summary?.proofHash ?? "",
    })),
  }));
  return summary;
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function fail(message) {
  console.error(`[real-estate-source-pipeline] ${message}`);
  process.exit(1);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
