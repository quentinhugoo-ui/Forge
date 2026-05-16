import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(appRoot, "..", "..");
const appRepoPrefix = slash(relative(repoRoot, appRoot));
const checkMode = process.argv.includes("--check");
const jsonMode = process.argv.includes("--json");
const failures = [];
const warnings = [];
const vendorArtifactSpecs = [
  {
    id: "three.module",
    file: "ui/assets/vendor/three/build/three.module.js",
    package: "three",
    version: "0.170.0",
    source: "npm:three@0.170.0/build/three.module.min.js",
    expectedSha256: "08fd7545d13d2c7fb65ab691530a802dafefd638596501854f267d0fb13c39e7",
  },
  {
    id: "three.GLTFLoader",
    file: "ui/assets/vendor/three/examples/jsm/loaders/GLTFLoader.js",
    package: "three",
    version: "0.170.0",
    source: "npm:three@0.170.0/examples/jsm/loaders/GLTFLoader.js minified with esbuild@0.27.4 --format=esm",
    expectedSha256: "90e6b59228df29a7a746b1de6ec248caaed7ebde241b822674346d4d2b6ad810",
  },
  {
    id: "three.BufferGeometryUtils",
    file: "ui/assets/vendor/three/examples/jsm/utils/BufferGeometryUtils.js",
    package: "three",
    version: "0.170.0",
    source: "npm:three@0.170.0/examples/jsm/utils/BufferGeometryUtils.js minified with esbuild@0.27.4 --format=esm",
    expectedSha256: "2100c6759b2a8a7da9f4f427e07dea3b5f426da1c1b4a3234b0d20ec8d630790",
  },
  {
    id: "xterm.core",
    file: "ui/assets/vendor/xterm/xterm.js",
    package: "xterm",
    version: "hash-pinned",
    source: "local vendored UMD bundle; replace only with a package-lock-backed artifact",
    expectedSha256: "14903579ff54664cd72f8e8699e6961a6272c21863ec1c3b118cdc8af5d4a972",
  },
  {
    id: "xterm.fit",
    file: "ui/assets/vendor/xterm/addon-fit.js",
    package: "xterm-addon-fit",
    version: "hash-pinned",
    source: "local vendored UMD bundle; replace only with a package-lock-backed artifact",
    expectedSha256: "ba3ea256ce0620a0992a197d6c9baea64823fc93d8da07a9e366ca9943c18527",
  },
  {
    id: "xterm.css",
    file: "ui/assets/vendor/xterm/xterm.css",
    package: "xterm",
    version: "hash-pinned",
    source: "local vendored stylesheet minified by whitespace/comment compression; replace only with a package-lock-backed artifact",
    expectedSha256: "1201c9b40de19cbd68d30571befe8ef53a1903f54ebbe839b01bb77c64983ff8",
  },
  {
    id: "xterm.license",
    file: "ui/assets/vendor/xterm/LICENSE.xterm",
    package: "xterm",
    version: "hash-pinned",
    source: "local vendored MIT license",
    expectedSha256: "b569f629d00f2626a8100df2a1798210535621e42164dfd426a6fe5aac7b0ccd",
  },
  {
    id: "xterm.fit.license",
    file: "ui/assets/vendor/xterm/LICENSE.addon-fit",
    package: "xterm-addon-fit",
    version: "hash-pinned",
    source: "local vendored MIT license",
    expectedSha256: "e256f01188af527e4d06d21d06fbf785ae9c50d4b328bf03cbe0ba7f0aa4228f",
  },
];
const reductionGoal = {
  baselineLines: 298000,
  targetLines: 60000,
};
const lineBudgets = {
  "examples/forge_tauri_ui/src-tauri/src/main.rs": 8000,
  "examples/forge_tauri_ui/ui/src/shell/surface.ts": 7000,
  "examples/forge_tauri_ui/ui/styles.css": 3500,
  "examples/lab_runner_trading.rs": 2500,
  "examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs": 3000,
  "examples/forge_tauri_ui/src-tauri/src/trading.rs": 3500,
  "examples/forge_tauri_ui/ui/src/sections/banger/surface.ts": 2500,
  "examples/forge_tauri_ui/ui/src/sections/trading/surface.ts": 3000,
  "examples/lab_runner_banger.rs": 2000,
  "examples/forge_tauri_ui/src-tauri/src/forge_agent_tools.rs": 2500,
};

function slash(path) {
  return String(path).replace(/\\/g, "/");
}

function appFileToRepoPath(file) {
  return slash(join(appRepoPrefix, file));
}

function read(relativePath) {
  const path = join(appRoot, relativePath);
  if (!existsSync(path)) {
    failures.push(`missing file: ${relativePath}`);
    return "";
  }
  return readFileSync(path, "utf8");
}

function lineOf(source, index) {
  return source.slice(0, index).split(/\r?\n/).length;
}

function uniqueSorted(values) {
  return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function round1(value) {
  return Math.round(value * 10) / 10;
}

function lineCountFile(path) {
  const text = readFileSync(path, "utf8");
  return text.length ? text.split(/\r?\n/).length : 0;
}

function git(args) {
  try {
    return execFileSync("git", args, { cwd: repoRoot, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return "";
  }
}

function trackedFiles() {
  const out = git(["ls-files"]);
  return out ? out.split(/\r?\n/).filter(Boolean) : [];
}

function workspaceFiles() {
  const untracked = git(["ls-files", "--others", "--exclude-standard"]);
  return uniqueSorted([...trackedFiles(), ...(untracked ? untracked.split(/\r?\n/).filter(Boolean) : [])]);
}

function sourceLineMetrics() {
  const sourceExt = /\.(rs|js|css|html|md|toml|json|mjs)$/i;
  const artifactFiles = new Set(vendorArtifactSpecs.map((artifact) => appFileToRepoPath(artifact.file)));
  const rows = [];
  for (const file of workspaceFiles().filter((path) => sourceExt.test(path))) {
    const abs = join(repoRoot, file);
    if (!existsSync(abs)) continue;
    const text = readFileSync(abs, "utf8");
    const lines = text.length ? text.split(/\r?\n/).length : 0;
    rows.push({
      file: slash(file),
      lines,
      bytes: Buffer.byteLength(text),
      ext: file.slice(file.lastIndexOf(".")).toLowerCase(),
      managedArtifact: artifactFiles.has(slash(file)),
    });
  }
  const total = rows.reduce((sum, row) => sum + row.lines, 0);
  const productRows = rows.filter((row) => !row.managedArtifact);
  const artifactRows = rows.filter((row) => row.managedArtifact);
  const productLines = productRows.reduce((sum, row) => sum + row.lines, 0);
  const artifactLines = artifactRows.reduce((sum, row) => sum + row.lines, 0);
  const artifactBytes = artifactRows.reduce((sum, row) => sum + row.bytes, 0);
  const byExt = Object.fromEntries(
    Object.entries(
      rows.reduce((acc, row) => {
        acc[row.ext] ||= { files: 0, lines: 0, productLines: 0 };
        acc[row.ext].files += 1;
        acc[row.ext].lines += row.lines;
        if (!row.managedArtifact) acc[row.ext].productLines += row.lines;
        return acc;
      }, {}),
    ).sort(([a], [b]) => a.localeCompare(b)),
  );
  const totalCut = Math.max(1, reductionGoal.baselineLines - reductionGoal.targetLines);
  const removedSinceBaseline = reductionGoal.baselineLines - total;
  const budgetPressure = Object.entries(lineBudgets)
    .map(([file, targetLines]) => {
      const lines = rows.find((row) => row.file === file)?.lines || 0;
      return {
        file: slash(file),
        lines,
        targetLines,
        overTargetLines: Math.max(0, lines - targetLines),
        cutNeededPercent: lines > 0 ? round1((Math.max(0, lines - targetLines) / lines) * 100) : 0,
      };
    })
    .sort((a, b) => b.overTargetLines - a.overTargetLines);
  return {
    files: rows.length,
    lines: total,
    productLines,
    managedArtifactLines: artifactLines,
    managedArtifactBytes: artifactBytes,
    targetLinesFor80PercentCut: Math.round(total * 0.2),
    reductionGoal: {
      ...reductionGoal,
      removedSinceBaseline,
      remainingToTarget: Math.max(0, total - reductionGoal.targetLines),
      currentCutPercent: round1((removedSinceBaseline / reductionGoal.baselineLines) * 100),
      progressPercent: round1((removedSinceBaseline / totalCut) * 100),
      currentMultipleOfTarget: round1(total / reductionGoal.targetLines),
    },
    byExt,
    managedArtifacts: artifactRows.map((row) => ({ file: row.file, lines: row.lines, bytes: row.bytes })),
    budgetPressure,
    largest: rows.sort((a, b) => b.lines - a.lines).slice(0, 20),
  };
}

function trackedDirSet(files) {
  const dirs = new Set();
  for (const file of files) {
    const parts = slash(file).split("/");
    for (let i = 1; i < parts.length; i += 1) {
      dirs.add(parts.slice(0, i).join("/"));
    }
  }
  return dirs;
}

function walkPhysicalTree(root, rel = "") {
  let files = 0;
  let dirs = 0;
  let bytes = 0;
  const byTop = new Map();
  for (const entry of readdirSync(join(root, rel), { withFileTypes: true })) {
    const childRel = slash(rel ? join(rel, entry.name) : entry.name);
    if (childRel === ".git" || childRel.startsWith(".git/")) continue;
    const top = childRel.split("/")[0] || ".";
    const current = byTop.get(top) || { files: 0, dirs: 0, bytes: 0 };
    const childAbs = join(root, childRel);
    if (entry.isDirectory()) {
      dirs += 1;
      current.dirs += 1;
      const nested = walkPhysicalTree(root, childRel);
      files += nested.files;
      dirs += nested.dirs;
      bytes += nested.bytes;
      current.files += nested.files;
      current.dirs += nested.dirs;
      current.bytes += nested.bytes;
      byTop.set(top, current);
    } else if (entry.isFile()) {
      const size = statSync(childAbs).size;
      files += 1;
      bytes += size;
      current.files += 1;
      current.bytes += size;
      byTop.set(top, current);
    }
  }
  return {
    files,
    dirs,
    bytes,
    byTop: Object.fromEntries(
      [...byTop.entries()].sort((a, b) => b[1].files - a[1].files || b[1].dirs - a[1].dirs),
    ),
  };
}

function structureMetrics() {
  const tracked = trackedFiles().map(slash);
  const trackedDirs = trackedDirSet(tracked);
  const physical = walkPhysicalTree(repoRoot);
  const ignored = git(["ls-files", "--others", "--ignored", "--exclude-standard"]);
  const ignoredFiles = ignored ? ignored.split(/\r?\n/).filter(Boolean).map(slash) : [];
  return {
    trackedFiles: tracked.length,
    trackedDirs: trackedDirs.size,
    physicalFiles: physical.files,
    physicalDirs: physical.dirs,
    physicalBytes: physical.bytes,
    ignoredFiles: ignoredFiles.length,
    ignoredTop: Object.fromEntries(
      Object.entries(
        ignoredFiles.reduce((acc, file) => {
          const top = file.split("/")[0] || ".";
          acc[top] ||= 0;
          acc[top] += 1;
          return acc;
        }, {}),
      ).sort((a, b) => b[1] - a[1]),
    ),
    physicalTop: physical.byTop,
    trackedTop: Object.fromEntries(
      Object.entries(
        tracked.reduce((acc, file) => {
          const top = file.split("/")[0] || ".";
          acc[top] ||= 0;
          acc[top] += 1;
          return acc;
        }, {}),
      ).sort((a, b) => b[1] - a[1]),
    ),
  };
}

function vendorArtifacts() {
  return vendorArtifactSpecs.map((artifact) => {
    const abs = join(appRoot, artifact.file);
    if (!existsSync(abs)) {
      failures.push(`missing vendor artifact: ${artifact.file}`);
      return { ...artifact, missing: true };
    }
    return {
      ...artifact,
      sha256: sha256File(abs),
      bytes: readFileSync(abs).byteLength,
      lines: lineCountFile(abs),
    };
  });
}

function trackedVendorFiles() {
  return workspaceFiles().filter((file) => slash(file).startsWith(`${appRepoPrefix}/ui/assets/vendor/`)).map(slash);
}

function extractTauriCommands() {
  const rustFiles = [
    "src-tauri/src/main.rs",
    "src-tauri/src/trading.rs",
    "src-tauri/src/trading_pressure.rs",
    "src-tauri/src/banger.rs",
    "src-tauri/src/real_estate_harvester.rs",
  ];
  const annotated = [];
  for (const file of rustFiles) {
    const text = read(file);
    const pattern = /#\[tauri::command\][\s\S]{0,160}?\b(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;
    for (let match; (match = pattern.exec(text));) {
      annotated.push({ name: match[1], file, line: lineOf(text, match.index) });
    }
  }

  const main = read("src-tauri/src/main.rs");
  const handlerStart = main.indexOf("tauri::generate_handler![");
  const registered = [];
  if (handlerStart >= 0) {
    const open = main.indexOf("[", handlerStart);
    const close = main.indexOf("])", open);
    const block = close >= 0 ? main.slice(open + 1, close) : main.slice(open + 1);
    for (const raw of block.split(",")) {
      const entry = raw.trim().replace(/\s+/g, " ");
      if (!entry || entry.startsWith("//")) continue;
      if (/^[A-Za-z_][A-Za-z0-9_:]*$/.test(entry)) {
        registered.push({
          name: entry,
          basename: entry.split("::").pop(),
          file: "src-tauri/src/main.rs",
          line: lineOf(main, main.indexOf(entry, handlerStart)),
        });
      }
    }
  } else {
    failures.push("missing tauri::generate_handler! block");
  }

  const registeredBasenames = new Set(registered.map((entry) => entry.basename));
  const annotatedButUnregistered = annotated
    .filter((entry) => !registeredBasenames.has(entry.name))
    .map((entry) => `${entry.name} (${entry.file}:${entry.line})`);

  return {
    annotated,
    registered,
    annotatedCount: annotated.length,
    registeredCount: registered.length,
    annotatedButUnregistered,
  };
}

function extractMcpTools() {
  const text = read("src-tauri/src/bin/forge_mcp.rs");
  const toolsStart = text.indexOf("fn tools_list()");
  const callStart = text.indexOf("fn handle_tool_call");
  const toolsBlock = toolsStart >= 0 && callStart > toolsStart ? text.slice(toolsStart, callStart) : "";
  const compactStart = text.indexOf("fn compact_tools_list()");
  const compactEnd = text.indexOf("fn env_flag", compactStart);
  const compactBlock = compactStart >= 0 && compactEnd > compactStart ? text.slice(compactStart, compactEnd) : "";
  const visible = [];
  const defaultVisibleBlock = compactBlock || toolsBlock;
  const namePattern = /"name"\s*:\s*"([a-z][a-z0-9_.]+)"/g;
  for (let match; (match = namePattern.exec(defaultVisibleBlock));) {
    visible.push(match[1]);
  }
  const helperPattern = /mcp_tool\(\s*"([a-z][a-z0-9_.]+)"/g;
  for (let match; (match = helperPattern.exec(defaultVisibleBlock));) {
    visible.push(match[1]);
  }

  const callEnd = text.indexOf("fn mcp_internal_tool_response", callStart);
  const callBlock = callStart >= 0 && callEnd > callStart ? text.slice(callStart, callEnd) : "";
  const handled = [];
  const armPattern = /^\s*((?:"[^"]+"\s*(?:\|\s*)?)+)=>/gm;
  for (let match; (match = armPattern.exec(callBlock));) {
    const names = [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    handled.push(...names);
  }
  const aliasesStart = text.indexOf("const MCP_TOOL_ALIASES");
  const aliasesEnd = text.indexOf("const MCP_INTERNAL_TOOL_ROUTES", aliasesStart);
  const aliasesBlock = aliasesStart >= 0 && aliasesEnd > aliasesStart ? text.slice(aliasesStart, aliasesEnd) : "";
  for (const match of aliasesBlock.matchAll(/"([^"]+)"/g)) {
    handled.push(match[1]);
  }

  const missingHandlers = uniqueSorted(visible.filter((name) => !handled.includes(name)));
  return {
    visible: uniqueSorted(visible),
    handled: uniqueSorted(handled),
    visibleCount: uniqueSorted(visible).length,
    handledAliasCount: uniqueSorted(handled).length,
    visibleWithoutHandler: missingHandlers,
  };
}

function extractUiSurface(registeredTauriCommands) {
  const ownership = JSON.parse(read("ui/SECTION_OWNERSHIP.json") || "{}");
  const entrypoints = ownership.entrypoints || [
    "ui/dist/forge-section-registry.js",
    "ui/dist/forge-tauri-bridge.js",
    "ui/dist/forge-boot.js",
    "ui/dist/forge-window-controls.js",
    "ui/dist/forge-webexplorer-config.js",
    "ui/src/shell/surface.ts",
    "ui/src/sections/trading/surface.ts",
    "ui/src/sections/banger/surface.ts",
  ];
  const sources = entrypoints.map((file) => ({ file, text: read(file) }));
  const invocations = [];
  const unknownInvocations = [];
  const eventListeners = [];
  const customEvents = [];
  const storageKeys = [];
  const storageDynamic = [];
  const constantStrings = new Map();

  for (const source of sources) {
    const constPattern = /\bconst\s+([A-Z][A-Z0-9_]+)\s*=\s*["']([^"']+)["']/g;
    for (let match; (match = constPattern.exec(source.text));) {
      constantStrings.set(match[1], match[2]);
    }
  }

  for (const source of sources) {
    const invokePattern = /(?<callee>[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*)\s*\(\s*["'](?<command>[a-z][a-z0-9_]{2,})["']/g;
    for (let match; (match = invokePattern.exec(source.text));) {
      const command = match.groups.command;
      const callee = match.groups.callee;
      const isIpcCallee = [
        "invoke",
        "forgeTauri.invoke",
        "window.__TAURI__.core.invoke",
        "tauri.core.invoke",
        "tauriApi.core.invoke",
        "backendInvoke",
        "fallbackTauriWindowControl",
        "invokeWindowCommand",
      ].includes(callee);
      if (!isIpcCallee) continue;
      const entry = {
        command,
        callee,
        file: source.file,
        line: lineOf(source.text, match.index),
      };
      if (registeredTauriCommands.has(command)) invocations.push(entry);
      else unknownInvocations.push(entry);
    }

    const dynamicProviderCommandPattern = /["']((?:codex|gemini|claude)_terminal_(?:snapshot|start|send_input|resize|stop|clear))["']/g;
    for (let match; (match = dynamicProviderCommandPattern.exec(source.text));) {
      const command = match[1];
      if (!registeredTauriCommands.has(command)) continue;
      invocations.push({
        command,
        callee: "providerTerminalInvokeName",
        file: source.file,
        line: lineOf(source.text, match.index),
      });
    }

    const addListenerPattern = /(?:window|document|[A-Za-z_$][\w$]*)\.addEventListener\(\s*["']([^"']+)["']/g;
    for (let match; (match = addListenerPattern.exec(source.text));) {
      eventListeners.push({ event: match[1], file: source.file, line: lineOf(source.text, match.index) });
    }

    const customEventPattern = /new\s+CustomEvent\(\s*["']([^"']+)["']/g;
    for (let match; (match = customEventPattern.exec(source.text));) {
      customEvents.push({ event: match[1], file: source.file, line: lineOf(source.text, match.index) });
    }

    const storagePattern = /localStorage\.(?:getItem|setItem|removeItem)\(\s*([^,)]+)/g;
    for (let match; (match = storagePattern.exec(source.text));) {
      const rawArg = match[1].trim();
      const quoted = rawArg.match(/^["']([^"']+)["']$/);
      const resolved = quoted?.[1] || constantStrings.get(rawArg);
      const entry = { key: resolved || rawArg, file: source.file, line: lineOf(source.text, match.index) };
      if (resolved) storageKeys.push(entry);
      else storageDynamic.push(entry);
    }
  }

  const tauriEvents = [];
  const eventSources = [
    { file: "src-tauri/src/main.rs", text: read("src-tauri/src/main.rs") },
    ...sources,
  ];
  for (const source of eventSources) {
    const emitPattern = /\.emit\(\s*["']([^"']+)["']/g;
    for (let match; (match = emitPattern.exec(source.text));) {
      tauriEvents.push({ direction: "emit", event: match[1], file: source.file, line: lineOf(source.text, match.index) });
    }
    const listenPattern = /\.listen\(\s*["']([^"']+)["']/g;
    for (let match; (match = listenPattern.exec(source.text));) {
      tauriEvents.push({ direction: "listen", event: match[1], file: source.file, line: lineOf(source.text, match.index) });
    }
  }

  const byCommand = {};
  for (const hit of invocations) {
    byCommand[hit.command] ||= [];
    byCommand[hit.command].push(`${hit.file}:${hit.line}:${hit.callee}`);
  }

  return {
    sections: (ownership.sections || []).map((section) => ({
      id: section.id,
      owner: section.owner,
      lifecycle: section.lifecycle,
      files: section.files || [],
      nativePresentCommands: section.nativePresentCommands || [],
      nativeHideCommands: section.nativeHideCommands || [],
    })),
    sensitiveCommands: ownership.sensitiveCommands || [],
    entrypoints,
    frontendCommandInvocations: Object.fromEntries(Object.entries(byCommand).sort(([a], [b]) => a.localeCompare(b))),
    frontendCommandCount: Object.keys(byCommand).length,
    frontendUnknownIpcInvocations: unknownInvocations.map((hit) => `${hit.command} (${hit.file}:${hit.line}:${hit.callee})`),
    customEvents: uniqueSorted(customEvents.map((entry) => entry.event)),
    eventListeners: uniqueSorted(eventListeners.map((entry) => entry.event)),
    tauriEvents: uniqueSorted(tauriEvents.map((entry) => `${entry.direction}:${entry.event}`)),
    localStorageKeys: uniqueSorted(storageKeys.map((entry) => entry.key)),
    dynamicLocalStorageKeys: uniqueSorted(storageDynamic.map((entry) => `${entry.key} (${entry.file}:${entry.line})`)),
  };
}

function validate(manifest) {
  const expectedMcp = [
    "forge.search",
    "forge.execute",
    "forge.read_projection",
    "forge.cancel",
  ];
  const expectedSections = ["shell", "alpha", "forge", "webexplorer", "real-estate", "real-estate-main", "trading", "banger"];
  const expectedSensitive = ["webexplorer_native_present", "bloomberg_live_native_present"];
  const mcpVisible = new Set(manifest.mcp.visible);
  const sections = new Set(manifest.ui.sections.map((section) => section.id));
  const tauriRegistered = new Set(manifest.tauri.registered.map((entry) => entry.basename));

  for (const tool of expectedMcp) {
    if (!mcpVisible.has(tool)) failures.push(`missing visible MCP tool: ${tool}`);
  }
  for (const section of expectedSections) {
    if (!sections.has(section)) failures.push(`missing UI section: ${section}`);
  }
  for (const command of expectedSensitive) {
    if (!tauriRegistered.has(command)) failures.push(`sensitive command not registered with Tauri: ${command}`);
  }
  for (const missing of manifest.tauri.annotatedButUnregistered) {
    warnings.push(`tauri command annotated but not registered: ${missing}`);
  }
  for (const missing of manifest.mcp.visibleWithoutHandler) {
    failures.push(`visible MCP tool has no handler arm: ${missing}`);
  }
  for (const hit of manifest.ui.frontendUnknownIpcInvocations) {
    warnings.push(`frontend IPC command is not registered with Tauri: ${hit}`);
  }
  for (const artifact of manifest.vendorArtifacts || []) {
    if (artifact.missing) continue;
    if (artifact.sha256 !== artifact.expectedSha256) {
      failures.push(`vendor artifact hash mismatch: ${artifact.file}`);
    }
  }
  const managedVendorFiles = new Set(vendorArtifactSpecs.map((artifact) => appFileToRepoPath(artifact.file)));
  for (const file of trackedVendorFiles()) {
    if (!managedVendorFiles.has(file)) failures.push(`tracked vendor file is not hash-pinned: ${file}`);
  }
}

const manifest = {
  schema: "forge.surface-manifest.v1",
  repoHead: git(["rev-parse", "--short", "HEAD"]) || null,
  sourceMetrics: sourceLineMetrics(),
  structure: structureMetrics(),
  vendorArtifacts: vendorArtifacts(),
  tauri: extractTauriCommands(),
  mcp: extractMcpTools(),
};
manifest.ui = extractUiSurface(new Set(manifest.tauri.registered.map((entry) => entry.basename)));
manifest.proofHash = sha256(JSON.stringify(manifest));
validate(manifest);

if (jsonMode) {
  console.log(JSON.stringify({ ...manifest, warnings, failures }, null, 2));
} else {
  const goal = manifest.sourceMetrics.reductionGoal;
  const pressure = manifest.sourceMetrics.budgetPressure
    .filter((row) => row.overTargetLines > 0)
    .slice(0, 5)
    .map((row) => `${row.file}:${row.lines}->${row.targetLines}`)
    .join(" ");
  console.log(`[forge-surface-manifest] proof=${manifest.proofHash}`);
  console.log(`[forge-surface-manifest] lines=${manifest.sourceMetrics.lines} target20=${manifest.sourceMetrics.targetLinesFor80PercentCut}`);
  console.log(`[forge-surface-manifest] productLines=${manifest.sourceMetrics.productLines} managedArtifactLines=${manifest.sourceMetrics.managedArtifactLines} managedArtifactBytes=${manifest.sourceMetrics.managedArtifactBytes}`);
  console.log(`[forge-surface-manifest] reduction baseline=${goal.baselineLines} target=${goal.targetLines} progress=${goal.progressPercent}% remaining=${goal.remainingToTarget} current=${goal.currentMultipleOfTarget}x-target`);
  console.log(`[forge-surface-manifest] structure trackedFiles=${manifest.structure.trackedFiles} trackedDirs=${manifest.structure.trackedDirs} physicalFiles=${manifest.structure.physicalFiles} physicalDirs=${manifest.structure.physicalDirs} ignoredFiles=${manifest.structure.ignoredFiles}`);
  console.log(`[forge-surface-manifest] structurePressure ${
    Object.entries(manifest.structure.physicalTop)
      .slice(0, 5)
      .map(([name, row]) => `${name}:${row.files}f/${row.dirs}d`)
      .join(" ") || "none"
  }`);
  console.log(`[forge-surface-manifest] pressure ${pressure || "none"}`);
  console.log(`[forge-surface-manifest] tauri registered=${manifest.tauri.registeredCount} annotated=${manifest.tauri.annotatedCount}`);
  console.log(`[forge-surface-manifest] mcp visible=${manifest.mcp.visibleCount} handledAliases=${manifest.mcp.handledAliasCount}`);
  console.log(`[forge-surface-manifest] ui sections=${manifest.ui.sections.length} frontendCommands=${manifest.ui.frontendCommandCount}`);
  console.log(`[forge-surface-manifest] vendorArtifacts=${manifest.vendorArtifacts.length}`);
}

if (warnings.length && !process.argv.includes("--quiet")) {
  console.warn("[forge-surface-manifest] warnings");
  for (const warning of warnings) console.warn(`- ${warning}`);
}

if (failures.length) {
  console.error("[forge-surface-manifest] failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
