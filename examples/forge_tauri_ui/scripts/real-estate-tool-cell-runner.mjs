import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(root, "..", "..");
const args = new Set(process.argv.slice(2));
const pretty = args.has("--pretty");
const refreshFbc = args.has("--refresh-fbc") || args.has("--materialize-fbc");
const toolId = argValue("--tool");
const engineOverride = argValue("--engine");
const storePath = resolve(argValue("--store") ?? process.env.FORGE_STORE_PATH ?? join(root, ".forge-data"));
const registryPath = resolve(argValue("--registry") ?? join(root, "source-registry", "real-estate-tool-cells.json"));
const toolsTsPath = join(root, "ui", "src", "sections", "real-estate", "tools.ts");
const dataDir = join(storePath, "real-estate-harvester", "data");
const graphPath = join(dataDir, "living_dataflow_graph.json");
const graphJsonlPath = join(dataDir, "living_dataflow_graph.jsonl");
const outputDir = join(dataDir, "tool_cell_outputs");
const fbcRegistryManifestPath = join(outputDir, "fbc_registry_batch.json");

if (!existsSync(registryPath)) fail(`tool cell registry not found: ${registryPath}`);
if (!existsSync(toolsTsPath)) fail(`real estate tools source not found: ${toolsTsPath}`);

const registry = JSON.parse(readFileSync(registryPath, "utf8"));
const uiTools = readUiToolIds();
const cells = expandCells(registry);
const validation = validateCells(cells, uiTools, registry);

if (validation.failures.length) {
  const summary = buildValidationSummary(validation, cells);
  if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
  process.exit(1);
}

if (refreshFbc || (engineOverride === "forge_bytecode_v0" && args.has("--ensure-fbc"))) {
  materializeForgeBytecodeRegistry();
}

if (!toolId) {
  const summary = buildValidationSummary(validation, cells);
  if (pretty || !args.has("--quiet")) console.log(JSON.stringify(summary, null, pretty ? 2 : 0));
  process.exit(0);
}

const cell = cells.find((entry) => entry.id === toolId);
if (!cell) fail(`unknown tool cell: ${toolId}`);

mkdirSync(outputDir, { recursive: true });
const output = runToolCell(cell);
const outputPath = join(outputDir, `${cell.command.slice(1, -1)}.json`);
writeFileSync(outputPath, `${JSON.stringify(output, null, 2)}\n`);
if (pretty || !args.has("--quiet")) console.log(JSON.stringify(output, null, pretty ? 2 : 0));

function runToolCell(cell) {
  if (cell.engine === "forge_bytecode_v0") return runForgeBytecodeToolCell(cell);

  const graph = readJson(graphPath) ?? {};
  const graphRecords = readJsonl(graphJsonlPath);
  const selectedNodes = graphRecords
    .filter((record) => record.kind === "dataflow_node" && cell.focus.includes(record.type))
    .slice(0, 80);
  const selectedEdges = graphRecords
    .filter((record) => record.kind === "dataflow_edge" && selectedNodes.some((node) => node.id === record.from || node.id === record.to))
    .slice(0, 80);
  const rankedActions = selectedNodes
    .filter((node) => node.type === "action" || node.type === "score" || node.type === "intelPack")
    .slice(0, 12)
    .map((node, index) => ({
      rank: index + 1,
      id: node.id,
      label: node.label,
      confidence: node.confidence,
      reason: `${cell.query}:${node.type}`,
    }));
  const evidenceRefs = [...selectedNodes, ...selectedEdges].slice(0, 96).map((record) => ({
    id: record.id,
    type: record.type ?? record.relation,
    recordHash: record.recordHash,
    confidence: record.confidence,
  }));
  const payload = {
    toolId: cell.id,
    command: cell.command,
    status: graph.proofHash ? "ok" : "empty_graph",
    graphProofHash: graph.proofHash ?? "",
    summary: {
      engine: cell.engine,
      query: cell.query,
      graphStatus: graph.status ?? "missing",
      selectedNodeCount: selectedNodes.length,
      selectedEdgeCount: selectedEdges.length,
      graphNodeCount: graph.nodeCount ?? 0,
      graphEdgeCount: graph.edgeCount ?? 0,
      permissions: cell.permissions,
      denied: cell.denied,
    },
    evidenceRefs,
    rankedActions,
  };
  payload.proofHash = sha256(JSON.stringify({
    manifestHash: cell.manifestHash,
    graphProofHash: payload.graphProofHash,
    evidenceRefs,
    rankedActions,
  }));
  return payload;
}

function runForgeBytecodeToolCell(cell) {
  const artifact = readForgeBytecodeArtifact(cell);
  if (artifact) return artifact;

  const graph = readJson(graphPath) ?? {};
  const graphRecords = readJsonl(graphJsonlPath);
  const selectedNodes = graphRecords
    .filter((record) => record.kind === "dataflow_node" && cell.focus.includes(record.type))
    .slice(0, 80);
  const selectedEdges = graphRecords
    .filter((record) => record.kind === "dataflow_edge" && selectedNodes.some((node) => node.id === record.from || node.id === record.to))
    .slice(0, 80);
  const rankedActions = selectedNodes
    .filter((node) => node.type === "action" || node.type === "score" || node.type === "intelPack")
    .slice(0, 12)
    .map((node, index) => ({
      rank: index + 1,
      id: node.id,
      label: node.label,
      confidence: node.confidence,
      reason: `${cell.query}:${node.type}`,
    }));
  const evidenceRefs = [...selectedNodes, ...selectedEdges].slice(0, 96).map((record) => ({
    id: record.id,
    type: record.type ?? record.relation,
    recordHash: record.recordHash,
    confidence: record.confidence,
  }));
  const program = buildForgeBytecodeProgram(cell);
  const verifier = verifyForgeBytecodeProgram(program);
  const outputProjection = {
    kind: "forge_bytecode_tool_cell_output_v0",
    toolId: cell.id,
    command: cell.command,
    query: cell.query,
    graphProofHash: graph.proofHash ?? "",
    selectedNodeCount: selectedNodes.length,
    selectedEdgeCount: selectedEdges.length,
    evidenceRefs,
    rankedActions,
  };
  const outputBytes = JSON.stringify(outputProjection);
  const fuelUsed = verifier.ok ? program.ops.length : 0;
  const memoryPeak = verifier.ok ? outputBytes.length : 0;
  const proof = buildForgeBytecodeProof(program, verifier, outputBytes, fuelUsed, memoryPeak);
  return {
    toolId: cell.id,
    command: cell.command,
    status: verifier.ok ? (graph.proofHash ? "ok" : "empty_graph") : "verifier_denied",
    graphProofHash: graph.proofHash ?? "",
    proofHash: proof.proofHash,
    summary: {
      engine: cell.engine,
      query: cell.query,
      graphStatus: graph.status ?? "missing",
      selectedNodeCount: selectedNodes.length,
      selectedEdgeCount: selectedEdges.length,
      graphNodeCount: graph.nodeCount ?? 0,
      graphEdgeCount: graph.edgeCount ?? 0,
      permissions: cell.permissions,
      denied: cell.denied,
      fbc: {
        programHash: proof.programHash,
        verifierStatus: verifier.ok ? "ok" : "denied",
        verifierHash: verifier.verifierHash,
        capabilitySummary: program.capabilities.map((cap) => `${cap.kind}:${cap.scope}:${cap.sealedHash}`),
        fuelUsed,
        memoryPeak,
        replayResult: proof.proofHash === buildForgeBytecodeProof(program, verifier, outputBytes, fuelUsed, memoryPeak).proofHash ? "stable" : "drift",
        verifierErrors: verifier.errors,
      },
    },
    evidenceRefs,
    rankedActions,
  };
}

function readForgeBytecodeArtifact(cell) {
  const path = join(outputDir, `${cell.command.slice(1, -1)}.fbc.json`);
  const artifact = readJson(path);
  if (!artifact || artifact.toolId !== cell.id || artifact.summary?.engine !== "forge_bytecode_v0") {
    return null;
  }
  return {
    ...artifact,
    summary: {
      ...(artifact.summary ?? {}),
      query: cell.query,
      graphStatus: readJson(graphPath)?.status ?? "missing",
      permissions: cell.permissions,
      denied: cell.denied,
      source: "rust_fbc_registry_write",
    },
  };
}

function materializeForgeBytecodeRegistry() {
  const result = spawnSync("cargo", ["run", "--example", "lab_runner_fbc", "--", "registry-write"], {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 32,
  });
  if (result.status !== 0) {
    if (existsSync(fbcRegistryManifestPath)) return;
    fail(`FBC registry materialization failed (${result.status}): ${(result.stderr || result.stdout || "").slice(0, 4000)}`);
  }
}

function expandCells(registry) {
  const defaults = registry.defaults ?? {};
  return (registry.toolCells ?? []).map((cell) => {
    const command = `/${cell.id.replaceAll("-", "_")}_`;
    const expanded = {
      engine: engineOverride ?? defaults.engine,
      permissions: defaults.permissions ?? [],
      denied: defaults.denied ?? [],
      inputSchema: defaults.inputSchema,
      outputSchema: defaults.outputSchema,
      ...cell,
      command,
    };
    expanded.manifestHash = sha256(JSON.stringify({
      id: expanded.id,
      command: expanded.command,
      group: expanded.group,
      engine: expanded.engine,
      permissions: expanded.permissions,
      denied: expanded.denied,
      inputSchema: expanded.inputSchema,
      outputSchema: expanded.outputSchema,
      focus: expanded.focus,
      query: expanded.query,
    }));
    return expanded;
  });
}

function validateCells(cells, uiTools, registry) {
  const failures = [];
  const cellIds = new Set(cells.map((cell) => cell.id));
  const duplicateIds = cells.map((cell) => cell.id).filter((id, index, arr) => arr.indexOf(id) !== index);
  for (const id of duplicateIds) failures.push(`duplicate tool cell: ${id}`);
  for (const tool of uiTools) {
    if (!cellIds.has(tool.id)) failures.push(`missing tool cell for UI tool: ${tool.id}`);
  }
  for (const cell of cells) {
    if (!uiTools.some((tool) => tool.id === cell.id)) failures.push(`orphan tool cell not present in UI: ${cell.id}`);
    if (cell.command !== `/${cell.id.replaceAll("-", "_")}_`) failures.push(`bad command for ${cell.id}: ${cell.command}`);
    if (!cell.engine) failures.push(`missing engine for ${cell.id}`);
    if (!Array.isArray(cell.permissions) || !cell.permissions.length) failures.push(`missing permissions for ${cell.id}`);
    if (!cell.inputSchema || cell.inputSchema.type !== "object") failures.push(`missing input schema for ${cell.id}`);
    if (!cell.outputSchema || cell.outputSchema.type !== "object") failures.push(`missing output schema for ${cell.id}`);
    if (!Array.isArray(cell.focus) || !cell.focus.length) failures.push(`missing dataflow focus for ${cell.id}`);
    if (!cell.query) failures.push(`missing query for ${cell.id}`);
  }
  if (!registry.defaults?.denied?.includes("filesystem:raw_client_files")) failures.push("default denied permissions must block raw client files");
  return { failures, uiToolCount: uiTools.length, cellCount: cells.length };
}

function buildValidationSummary(validation, cells) {
  const byGroup = {};
  for (const cell of cells) byGroup[cell.group] = (byGroup[cell.group] ?? 0) + 1;
  const summary = {
    kind: "real_estate_tool_cell_registry",
    status: validation.failures.length ? "failed" : "ok",
    registryPath: registryPath.replaceAll("\\", "/"),
    toolCount: validation.uiToolCount,
    cellCount: validation.cellCount,
    byGroup,
    failures: validation.failures,
    manifestHash: sha256(JSON.stringify(cells.map((cell) => cell.manifestHash).sort())),
    contract: "ToolCell { manifest, inputSchema, outputSchema, permissions, denied, proofHash }",
  };
  summary.proofHash = sha256(JSON.stringify(summary));
  return summary;
}

function readUiToolIds() {
  const source = readFileSync(toolsTsPath, "utf8");
  const matches = [...source.matchAll(/\["([^"]+)",\s*"([^"]+)",\s*"([^"]+)"\]/g)];
  return matches.map((match) => ({ id: match[1], label: match[2], icon: match[3] }));
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

function buildForgeBytecodeProgram(cell) {
  const capabilities = (cell.permissions ?? []).map((permission) => {
    const kind = permission.startsWith("read:")
      ? "memory_scope"
      : permission.startsWith("write:")
        ? "artifact_hash"
        : "event_schema";
    const scope = permission.replace(/[^a-zA-Z0-9:_-]/g, "_");
    return {
      kind,
      scope,
      sealedHash: sha256(JSON.stringify({ kind, scope, limitBytes: 65536 })),
      limitBytes: 65536,
    };
  });
  return {
    version: 0,
    name: `toolcell:${cell.id}`,
    deterministic: true,
    expectedOutputSchema: "forge_bytecode_tool_cell_output_v0",
    capabilities,
    denied: cell.denied ?? [],
    hostcalls: ["ui_project_event", "read_capability"],
    ops: [
      { op: "push_text", valueHash: sha256(cell.query) },
      { op: "ui_intent_transition", from: "real-estate-main", intent: cell.query },
      { op: "emit_projection", label: cell.command },
      { op: "end" },
    ],
  };
}

function verifyForgeBytecodeProgram(program) {
  const errors = [];
  if (program.version !== 0) errors.push(`unsupported version ${program.version}`);
  if (!program.ops.length) errors.push("program has no opcodes");
  if (program.ops.at(-1)?.op !== "end") errors.push("program must end with end");
  for (const capability of program.capabilities) {
    if (capability.kind === "raw_filesystem" || capability.kind === "raw_network" || capability.kind === "secret") {
      errors.push(`denied capability ${capability.kind}`);
    }
    if (/[\\/]/.test(capability.scope) || capability.scope.includes("..")) {
      errors.push(`raw path-like capability scope denied: ${capability.scope}`);
    }
  }
  for (const denied of program.denied) {
    if (denied === "filesystem:raw_client_files" || denied === "network:direct" || denied === "secret:read") continue;
    if (denied.includes("\\") || denied.startsWith("/") || denied.includes("..")) {
      errors.push(`raw denied scope is not a capability: ${denied}`);
    }
  }
  for (const op of program.ops) {
    if (op.op === "raw_filesystem_probe" || op.op === "raw_network_probe") {
      errors.push(`denied opcode ${op.op}`);
    }
  }
  const verifierHash = sha256(JSON.stringify({
    domain: "forge-fbc-toolcell-verifier-v0",
    programHash: forgeBytecodeProgramHash(program),
    errors,
  }));
  return { ok: errors.length === 0, errors, verifierHash };
}

function buildForgeBytecodeProof(program, verifier, outputBytes, fuelUsed, memoryPeak) {
  const programHash = forgeBytecodeProgramHash(program);
  const bytecodeHash = sha256(JSON.stringify(program));
  const inputHash = sha256(JSON.stringify({ name: program.name, ops: program.ops }));
  const outputHash = sha256(outputBytes);
  const capabilityHash = sha256(JSON.stringify(program.capabilities));
  const hostcallHash = sha256(JSON.stringify(program.hostcalls));
  const backend = "toolcell_fbc_interpreter";
  const deterministicReplayHash = sha256(JSON.stringify({
    domain: "forge-fbc-toolcell-replay-v0",
    programHash,
    inputHash,
    outputHash,
    fuelUsed,
    memoryPeak,
    backend,
  }));
  const proofHash = sha256(JSON.stringify({
    domain: "forge-fbc-toolcell-proof-v0",
    programHash,
    bytecodeHash,
    verifierHash: verifier.verifierHash,
    inputHash,
    outputHash,
    capabilityHash,
    hostcallHash,
    fuelUsed,
    memoryPeak,
    backend,
    deterministicReplayHash,
  }));
  return {
    programHash,
    bytecodeHash,
    verifierHash: verifier.verifierHash,
    inputHash,
    outputHash,
    capabilityHash,
    hostcallHash,
    fuelUsed,
    memoryPeak,
    backend,
    deterministicReplayHash,
    proofHash,
  };
}

function forgeBytecodeProgramHash(program) {
  return sha256(JSON.stringify({
    version: program.version,
    name: program.name,
    deterministic: program.deterministic,
    expectedOutputSchema: program.expectedOutputSchema,
    capabilities: program.capabilities,
    hostcalls: program.hostcalls,
    ops: program.ops,
  }));
}

function argValue(name) {
  const prefix = `${name}=`;
  const found = process.argv.slice(2).find((arg) => arg.startsWith(prefix));
  return found ? found.slice(prefix.length) : undefined;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fail(message) {
  console.error(`[real-estate-tool-cell-runner] ${message}`);
  process.exit(1);
}
