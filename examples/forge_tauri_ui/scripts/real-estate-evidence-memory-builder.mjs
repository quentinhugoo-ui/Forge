import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const storePath = resolve(argValue("--store") ?? process.env.FORGE_STORE_PATH ?? join(root, ".forge-data"));
const limit = Number(argValue("--limit") ?? 512);

const dataDir = join(storePath, "real-estate-harvester", "data");
const graphLatestPath = join(dataDir, "living_dataflow_graph.json");
const toolOutputsDir = join(dataDir, "tool_cell_outputs");
const rankedActionsPath = join(dataDir, "ranked_actions.json");
const memoryCommitsPath = join(dataDir, "real_estate_memory_commits.jsonl");
const factsPath = join(dataDir, "evidence_memory_facts.jsonl");
const latestPath = join(dataDir, "evidence_memory_latest.json");

const startedAt = new Date().toISOString();
const facts = [];

mkdirSync(dataDir, { recursive: true });
writeFileSync(factsPath, "");

const graph = readJson(graphLatestPath);
if (graph) {
  pushFact({
    scope: "agence_immo:dataflow",
    claim: `Living dataflow graph ${graph.status ?? "unknown"}: ${graph.nodeCount ?? 0} nodes, ${graph.edgeCount ?? 0} edges.`,
    sourceHash: graph.inputHash ?? "",
    proofHash: graph.proofHash ?? "",
    confidence: confidenceFromCounts(graph.nodeCount, graph.edgeCount, 0.64),
    sourceKind: "living_dataflow_graph",
    evidence: {
      graphPath: graph.graphPath,
      inputHash: graph.inputHash,
      nodeCount: graph.nodeCount,
      edgeCount: graph.edgeCount,
      byNodeType: graph.byNodeType,
      byRelation: graph.byRelation,
    },
  });
}

for (const output of readToolOutputs().slice(0, limit)) {
  const selected = output.summary?.selectedNodeCount ?? 0;
  const ranked = output.rankedActions?.length ?? 0;
  pushFact({
    scope: `agence_immo:tool:${output.toolId}`,
    claim: `${output.command} returned ${selected} graph nodes and ${ranked} ranked actions.`,
    sourceHash: output.graphProofHash ?? "",
    proofHash: output.proofHash ?? "",
    confidence: confidenceFromCounts(selected, ranked, output.status === "ok" ? 0.58 : 0.32),
    sourceKind: "tool_cell_output",
    evidence: {
      toolId: output.toolId,
      command: output.command,
      status: output.status,
      graphProofHash: output.graphProofHash,
      summary: output.summary,
      evidenceRefs: output.evidenceRefs?.slice?.(0, 24) ?? [],
    },
  });
  for (const action of output.rankedActions ?? []) {
    pushFact({
      scope: `agence_immo:tool:${output.toolId}:action`,
      claim: `${output.command} suggests ${action.label ?? action.id} (${action.reason ?? "no reason"}).`,
      sourceHash: output.proofHash ?? "",
      proofHash: action.id ? sha256(`${output.proofHash}:${action.id}:${action.reason ?? ""}`) : output.proofHash,
      confidence: clamp(Number(action.confidence ?? output.summary?.selectedNodeCount ? 0.52 : 0.3), 0.05, 0.98),
      sourceKind: "tool_cell_ranked_action",
      evidence: { toolId: output.toolId, command: output.command, action },
    });
  }
}

for (const action of readRankedActions(rankedActionsPath).slice(0, limit)) {
  pushFact({
    scope: "agence_immo:kasm:ranked_action",
    claim: `KASM ranked action ${action.actionId ?? action.id ?? action.scenario ?? "unknown"}: ${action.decision ?? action.rationale?.label ?? "no decision"}.`,
    sourceHash: action.packProofHash ?? action.seedHash ?? "",
    proofHash: action.actionHash ?? sha256(JSON.stringify(action)),
    confidence: clamp(Number(action.confidence ?? action.utilityScore ?? 0.5), 0.05, 0.98),
    sourceKind: "kasm_ranked_action",
    evidence: action,
  });
}

for (const commit of readJsonl(memoryCommitsPath).slice(0, limit)) {
  pushFact({
    scope: `agence_immo:brain:${commit.memoryLayer ?? "semantic"}`,
    claim: commit.noteText?.split("\n").filter(Boolean).slice(-1)[0] ?? commit.decision ?? "Memory commit",
    sourceHash: commit.textHash ?? commit.evidence?.actionHash ?? "",
    proofHash: commit.commitHash ?? sha256(JSON.stringify(commit)),
    confidence: clamp(Number(commit.trustScore ?? commit.confidence ?? 0.5), 0.05, 0.98),
    sourceKind: "brain_memory_commit",
    evidence: {
      brainRef: commit.brainRef,
      actionId: commit.actionId,
      scenario: commit.scenario,
      evidence: commit.evidence,
      toolRouting: commit.toolRouting,
    },
  });
}

const finalized = finalizeFacts(facts);
for (const fact of finalized) appendFileSync(factsPath, `${JSON.stringify(fact)}\n`);
const latest = buildLatest(finalized);
writeFileSync(latestPath, `${JSON.stringify(latest, null, 2)}\n`);
if (pretty || !args.has("--quiet")) console.log(JSON.stringify(latest, null, pretty ? 2 : 0));

function pushFact(input) {
  const observedAt = startedAt;
  const stableKey = `${input.scope}:${normalizeClaim(input.claim)}`;
  const evidence = input.evidence ?? {};
  const noteText = String(input.claim || "").trim();
  const noteHash = sha256(stableJson({
    kind: "real_estate_memory_note_v1",
    scope: input.scope,
    noteText,
    sourceKind: input.sourceKind,
    sourceHash: input.sourceHash || "",
    proofHash: input.proofHash || "",
  }));
  const evidenceHash = sha256(stableJson({
    kind: "real_estate_memory_evidence_v1",
    scope: input.scope,
    sourceKind: input.sourceKind,
    sourceHash: input.sourceHash || "",
    proofHash: input.proofHash || "",
    evidence,
  }));
  facts.push({
    kind: "evidence_fact",
    schemaVersion: 1,
    factId: `fact:${sha256(stableKey).slice(0, 32)}`,
    stableKey,
    scope: input.scope,
    claim: input.claim,
    sourceKind: input.sourceKind,
    sourceHash: input.sourceHash || "",
    proofHash: input.proofHash || "",
    noteText,
    noteHash,
    evidenceHash,
    confidence: clamp(Number(input.confidence ?? 0.5), 0.01, 0.99),
    observedAt,
    supersedes: [],
    contradicts: [],
    evidence,
    memoryNote: {
      kind: "anchored_memory_note",
      scope: input.scope,
      noteText,
      noteHash,
      evidenceHash,
      sourceHash: input.sourceHash || "",
      proofHash: input.proofHash || "",
      observedAt,
    },
  });
}

function finalizeFacts(inputFacts) {
  const byStable = new Map();
  const contradictions = new Map();
  for (const fact of inputFacts) {
    const current = byStable.get(fact.stableKey);
    if (!current || current.confidence <= fact.confidence) byStable.set(fact.stableKey, fact);
  }
  const deduped = [...byStable.values()].sort((a, b) => a.scope.localeCompare(b.scope) || b.confidence - a.confidence);
  const byScope = groupBy(deduped, (fact) => fact.scope);
  for (const group of byScope.values()) {
    for (const a of group) {
      for (const b of group) {
        if (a.factId === b.factId) continue;
        if (claimsContradict(a.claim, b.claim)) {
          if (!contradictions.has(a.factId)) contradictions.set(a.factId, new Set());
          contradictions.get(a.factId).add(b.factId);
        }
      }
    }
  }
  const latestByClaim = new Map();
  for (const fact of deduped) {
    const claimKey = `${fact.scope}:${normalizeNumbersAway(fact.claim)}`;
    const previous = latestByClaim.get(claimKey);
    if (previous && previous.factId !== fact.factId) fact.supersedes.push(previous.factId);
    latestByClaim.set(claimKey, fact);
  }
  return deduped.map((fact) => {
    const out = {
      ...fact,
      contradicts: [...(contradictions.get(fact.factId) ?? [])],
    };
    out.factHash = sha256(JSON.stringify({
      factId: out.factId,
      scope: out.scope,
      claim: out.claim,
      noteHash: out.noteHash,
      evidenceHash: out.evidenceHash,
      sourceHash: out.sourceHash,
      proofHash: out.proofHash,
      confidence: out.confidence,
      observedAt: out.observedAt,
      supersedes: out.supersedes,
      contradicts: out.contradicts,
    }));
    return out;
  });
}

function buildLatest(facts) {
  const bySourceKind = countBy(facts, (fact) => fact.sourceKind);
  const byScope = countBy(facts, (fact) => fact.scope);
  const proofHash = sha256(JSON.stringify(facts.map((fact) => fact.factHash).sort()));
  return {
    kind: "evidence_memory_latest",
    schemaVersion: 1,
    status: facts.length ? "ok" : "empty",
    startedAt,
    finishedAt: new Date().toISOString(),
    storePath: storePath.replaceAll("\\", "/"),
    factsPath: factsPath.replaceAll("\\", "/"),
    latestPath: latestPath.replaceAll("\\", "/"),
    factCount: facts.length,
    contradictionCount: facts.reduce((sum, fact) => sum + fact.contradicts.length, 0),
    supersessionCount: facts.reduce((sum, fact) => sum + fact.supersedes.length, 0),
    maxConfidence: round(Math.max(0, ...facts.map((fact) => fact.confidence))),
    bySourceKind,
    byScope,
    proofHash,
    noteHashCount: facts.filter((fact) => fact.noteHash).length,
    evidenceHashCount: facts.filter((fact) => fact.evidenceHash).length,
    llmContract: {
      rule: "Use anchored memory notes instead of raw context. Cite factId/noteHash/evidenceHash/sourceHash/proofHash. Treat contradictions as unresolved until a newer higher-confidence fact supersedes them.",
      requiredFields: ["factId", "scope", "claim", "noteHash", "evidenceHash", "sourceHash", "proofHash", "confidence", "observedAt", "supersedes", "contradicts"],
      rawDataPolicy: "Do not read raw client files from memory; follow source/proof hashes only when explicitly requested.",
    },
  };
}

function readToolOutputs() {
  if (!existsSync(toolOutputsDir)) return [];
  return readdirSync(toolOutputsDir)
    .filter((name) => name.endsWith(".json"))
    .map((name) => readJson(join(toolOutputsDir, name)))
    .filter((output) => output?.toolId && output?.command && Array.isArray(output?.rankedActions));
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

function claimsContradict(a, b) {
  const left = a.toLowerCase();
  const right = b.toLowerCase();
  return (left.includes("empty") && right.includes(" ok"))
    || (right.includes("empty") && left.includes(" ok"))
    || (left.includes("no decision") && !right.includes("no decision"))
    || (right.includes("no decision") && !left.includes("no decision"));
}

function normalizeClaim(value) {
  return String(value).toLowerCase().replace(/\s+/g, " ").trim();
}

function normalizeNumbersAway(value) {
  return normalizeClaim(value).replace(/\d+(\.\d+)?/g, "#");
}

function confidenceFromCounts(a = 0, b = 0, base = 0.5) {
  return clamp(base + Math.min(0.28, Number(a || 0) * 0.025 + Number(b || 0) * 0.035), 0.05, 0.96);
}

function groupBy(items, fn) {
  const map = new Map();
  for (const item of items) {
    const key = fn(item);
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(item);
  }
  return map;
}

function countBy(items, fn) {
  const out = {};
  for (const item of items) {
    const key = fn(item) ?? "unknown";
    out[key] = (out[key] ?? 0) + 1;
  }
  return out;
}

function round(value) {
  return Math.round(Number(value || 0) * 1000) / 1000;
}

function clamp(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, value));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function stableJson(value) {
  return JSON.stringify(stableValue(value));
}

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (value && typeof value === "object") {
    const out = {};
    for (const key of Object.keys(value).sort()) out[key] = stableValue(value[key]);
    return out;
  }
  return value;
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}
