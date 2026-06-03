import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const strict = process.argv.includes("--strict");
const failures = [];
const warnings = [];

function read(path) {
  const abs = join(root, path);
  if (!existsSync(abs)) {
    failures.push(`missing ${path}`);
    return "";
  }
  return readFileSync(abs, "utf8");
}

function readJson(path) {
  const source = read(path);
  if (!source) return {};
  try {
    return JSON.parse(source);
  } catch (err) {
    failures.push(`invalid JSON ${path}: ${err.message || err}`);
    return {};
  }
}

function expect(label, condition) {
  if (!condition) failures.push(label);
}

function warn(label, condition) {
  if (!condition) warnings.push(label);
}

function count(source, pattern) {
  return [...source.matchAll(pattern)].length;
}

function listFiles(dir, acc = []) {
  if (!existsSync(dir)) return acc;
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      if (["dist", "node_modules", "assets"].includes(entry)) continue;
      listFiles(path, acc);
    } else {
      acc.push(path);
    }
  }
  return acc;
}

const appTs = read("ui/src/shell/surface.ts");
const bootJs = read("ui/src/shell/boot.ts");
const indexHtml = read("ui/index.html");
const kernelRs = read("src-tauri/src/forge_kernel.rs");
const jobRuntimeRs = read("src-tauri/src/forge_job_runtime.rs");
const mainTs = read("ui/src/main.ts");
const intentSurfaceTs = read("ui/src/shell/intent-surface.ts");
const shellMachineTs = read("ui/src/shell/shell-machine.ts");
const shellTypesTs = read("ui/src/shell/types.ts");
const tauriClientTs = read("ui/src/shell/tauri-client.ts");
const tauriBridgeTs = read("ui/src/shell/tauri-bridge.ts");
const windowControlsTs = read("ui/src/shell/window-controls.ts");
const clickRouterTs = read("ui/src/shell/click-router.ts");
const sectionRegistryTs = read("ui/src/shell/section-registry.ts");
const manifestsTs = read("ui/src/sections/manifests.ts");
const dataflow = read("scripts/real-estate-living-dataflow-graph.mjs");
const toolCells = read("source-registry/real-estate-tool-cells.json");
const evidenceMemory = read("scripts/real-estate-evidence-memory-builder.mjs");
const pipeline = read("scripts/real-estate-source-pipeline.mjs");
const manualJsLock = read("ui/src/MANUAL_JS_LOCK.md");
const ownership = read("ui/SECTION_OWNERSHIP.json");
const tauriConfig = readJson("src-tauri/tauri.conf.json");
const defaultCapability = readJson("src-tauri/capabilities/default.json");
const webexplorerRemoteCapability = readJson("src-tauri/capabilities/webexplorer-remote.json");

const surfaceTsSourceBudget = Object.freeze({
  "ui/src/shell/surface.ts": { bytes: 1189163, listeners: 218 },
  "ui/src/sections/banger/surface.ts": { bytes: 399081, listeners: 20 },
  "ui/src/sections/trading/surface.ts": { bytes: 318167, listeners: 41 },
});

expect("1 Event Kernel must own append-only shell history", kernelRs.includes("forge_kernel_events.jsonl") && kernelRs.includes("pub fn forge_kernel") && kernelRs.includes("fn replay"));
expect("2 Shell State Machine must fuse navigation/panels/window/onboarding", shellTypesTs.includes("ForgeShellState") && shellTypesTs.includes("SET_WINDOW_COMMAND") && shellTypesTs.includes("SET_ONBOARDING") && shellMachineTs.includes("reduceForgeShellState"));
expect("3 Runtime Bridge must centralize typed Tauri contracts", tauriClientTs.includes("forgeTauriCommandContracts") && tauriClientTs.includes("window.ForgeTauriBridge.invoke") && tauriClientTs.includes("forge_kernel"));
expect("4 UI Projection must render from compact kernel state", mainTs.includes("acceptProjection") && mainTs.includes("ForgeShellProjection") && kernelRs.includes("pub canvas: JsonValue") && kernelRs.includes("pub right_panel: JsonValue"));
expect(
  "5 Section Cells must expose lifecycle/permissions/commands",
  shellTypesTs.includes("ForgeSectionCell") &&
    (sectionRegistryTs.includes("applyState(state: ForgeShellState)") || mainTs.includes("registry.applyState(state)")) &&
    manifestsTs.includes("permissions:") &&
    manifestsTs.includes("commands:"),
);
expect("6 Durable Job Runtime must have unified job ledger", jobRuntimeRs.includes("ForgeUnifiedJob") && jobRuntimeRs.includes("forge_job_ledger.jsonl") && jobRuntimeRs.includes("recover_job_ledger"));
expect("7 Living Dataflow Graph must materialize content-addressed views", dataflow.includes("living_dataflow_graph.json") && dataflow.includes("inputHash") && dataflow.includes("source -> raw -> event -> entity/signal -> intelPack -> score -> action -> memoryFact"));
expect("8 Tool Cells must manifest schemas, permissions and proofs", toolCells.includes('"inputSchema"') && toolCells.includes('"outputSchema"') && toolCells.includes('"permissions"'));
expect("9 Evidence Memory must produce sourced, contradictory/supersedable facts", evidenceMemory.includes("evidence_memory_facts.jsonl") && evidenceMemory.includes("supersedes") && evidenceMemory.includes("contradicts") && evidenceMemory.includes("sourceHash") && evidenceMemory.includes("proofHash"));
expect("10 Pipeline must run dataflow and evidence stages", pipeline.includes("runStep(\"dataflow\"") && pipeline.includes("runStep(\"evidence\""));
expect(
  "11 Intent UI contract must be TS-owned and bundled through the shell runtime",
  intentSurfaceTs.includes("step_16_ts_owned_contract") &&
    intentSurfaceTs.includes("FORGE_AGENT_COMPACT_SURFACE=1") &&
    intentSurfaceTs.includes("FORGE_INTENT_ActCode_SURFACE=1") &&
    intentSurfaceTs.includes("direct JS listeners") &&
    intentSurfaceTs.includes("duplicate intent runners outside BrainCommand/FORGE_AGENT.rs") &&
    mainTs.includes("forgeIntentSurfaceContract") &&
    mainTs.includes("ForgeIntentSurfaceContract"),
);

expect("forge-boot must not bypass the Tauri bus", !bootJs.includes("__TAURI__?.core?.invoke") && !bootJs.includes(".core.invoke"));
expect("index must load generated bridge before typed runtime", indexHtml.indexOf('src="./dist/forge-tauri-bridge.js"') > -1 && indexHtml.indexOf('src="./dist/forge-tauri-bridge.js"') < indexHtml.indexOf('src="./dist/forge-shell-runtime.js"'));
expect("index must load typed runtime before generated app surface", indexHtml.indexOf('src="./dist/forge-shell-runtime.js"') > -1 && indexHtml.indexOf('src="./dist/forge-shell-runtime.js"') < indexHtml.indexOf('src="./dist/forge-app.js"'));
expect("manual JS lock must forbid hand-written JS and document TS surface cells", manualJsLock.includes("None.") && manualJsLock.includes("ui/src/shell/surface.ts") && manualJsLock.includes("ui/dist/**/*.js"));
expect("section ownership must mention generated TS bundles", ownership.includes("ui/dist/forge-shell-runtime.js") && ownership.includes("ui/src/shell/section-registry.ts"));
expect("Tauri global API must stay disabled", tauriConfig?.app?.withGlobalTauri === false);
expect("Tauri bridge must use explicit API imports", tauriBridgeTs.includes('from "@tauri-apps/api/core"') && tauriBridgeTs.includes('from "@tauri-apps/api/event"') && windowControlsTs.includes('from "@tauri-apps/api/window"'));
expect("UI source must not depend on window.__TAURI__", ![tauriBridgeTs, tauriClientTs, windowControlsTs, appTs, bootJs, mainTs].some((source) => source.includes("__TAURI__")));
expect("Tauri security must use explicit capabilities", JSON.stringify(tauriConfig?.app?.security?.capabilities || []) === JSON.stringify(["default", "webexplorer-remote"]));
expect("Tauri CSP must be enabled and deny object/base injection", !!tauriConfig?.app?.security?.csp && tauriConfig.app.security.csp !== null && tauriConfig.app.security.csp["object-src"] === "'none'" && tauriConfig.app.security.csp["base-uri"] === "'none'");
expect("main UI capability must stay window-scoped", JSON.stringify(defaultCapability?.windows || []) === JSON.stringify(["main"]) && !defaultCapability.remote);
expect("remote webexplorer capability must stay isolated to its backend webview", JSON.stringify(webexplorerRemoteCapability?.webviews || []) === JSON.stringify(["webexplorer-native"]) && !!webexplorerRemoteCapability?.remote && !(webexplorerRemoteCapability?.windows || []).includes("main"));

const manualJsFiles = listFiles(join(root, "ui"))
  .filter((file) => file.endsWith(".js"))
  .filter((file) => !file.includes(`${join("ui", "dist")}${"\\"}`))
  .map((file) => `ui/${relative(join(root, "ui"), file).replaceAll("\\", "/")}`)
  .sort();
for (const file of manualJsFiles) {
  expect(`manual JS file forbidden outside dist: ${file}`, false);
}
expect("manual JS source debt must be zero", manualJsFiles.length === 0);
const manualJsFileSizes = Object.fromEntries(manualJsFiles.map((file) => {
  const abs = join(root, file.replace(/\//g, "\\"));
  return [file, statSync(abs).size];
}));
const manualJsDirectListeners = Object.fromEntries(manualJsFiles.map((file) => {
  const source = read(file);
  return [file, count(source, /addEventListener\(/g)];
}));

const surfaceTsSourceSizes = {};
const surfaceTsDirectListeners = {};
for (const [file, budget] of Object.entries(surfaceTsSourceBudget)) {
  const source = read(file);
  const abs = join(root, file.replace(/\//g, "\\"));
  const size = statSync(abs).size;
  const listenerCount = count(source, /addEventListener\(/g);
  surfaceTsSourceSizes[file] = size;
  surfaceTsDirectListeners[file] = listenerCount;
  expect(`shell TS surface may shrink or disappear but not grow: ${file} ${size}/${budget.bytes}`, size <= budget.bytes);
  expect(`surface TS listeners may shrink or disappear but not grow: ${file} ${listenerCount}/${budget.listeners}`, listenerCount <= budget.listeners);
}

const rawIpcCount = count(appTs, /(__TAURI__[\s\S]{0,60}core[\s\S]{0,20}invoke|\.core\.invoke|tauriApi\.core\.invoke|tauri\.core\.invoke)/g);
const rawTauriEventCount = count(appTs, /(__TAURI__[\s\S]{0,60}event[\s\S]{0,20}listen|tauri\.event\.listen)/g);
const directListenerCount = count(appTs, /addEventListener\(/g);
const querySelectorCount = count(appTs, /querySelector\(/g);
warn(`shell raw IPC budget exceeded in surface.ts: ${rawIpcCount}/0`, rawIpcCount <= 0);
warn(`shell raw Tauri event budget exceeded in surface.ts: ${rawTauriEventCount}/0`, rawTauriEventCount <= 0);
warn(`shell listener budget exceeded in surface.ts: ${directListenerCount}/221`, directListenerCount <= 221);
warn(`shell querySelector budget exceeded in surface.ts: ${querySelectorCount}/55`, querySelectorCount <= 55);

const summary = {
  kind: "forge_full_app_cutover_audit",
  status: failures.length ? "failed" : warnings.length && strict ? "warning_failed" : "ok",
  pillarsChecked: 11,
  surfaceDebtBudget: {
    appRawIpc: { count: rawIpcCount, max: 0 },
    appRawTauriEvents: { count: rawTauriEventCount, max: 0 },
    appDirectListeners: { count: directListenerCount, max: 221 },
    appQuerySelector: { count: querySelectorCount, max: 55 },
    manualJsFiles,
    manualJsFileSizes,
    manualJsDirectListeners,
    surfaceTsSourceBudget,
    surfaceTsSourceSizes,
    surfaceTsDirectListeners,
  },
  failures,
  warnings,
};

console.log(JSON.stringify(summary, null, 2));
if (failures.length || (strict && warnings.length)) process.exit(1);
