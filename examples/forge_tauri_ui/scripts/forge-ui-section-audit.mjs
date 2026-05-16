import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];
const warnings = [];

function read(relativePath) {
  const path = join(root, relativePath);
  if (!existsSync(path)) {
    failures.push(`missing file: ${relativePath}`);
    return "";
  }
  return readFileSync(path, "utf8");
}

function expect(label, condition) {
  if (!condition) failures.push(label);
}

function escapeRegExp(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function lineOf(source, index) {
  return source.slice(0, index).split(/\r?\n/).length;
}

function scriptOrder(html, scripts) {
  let cursor = -1;
  for (const script of scripts) {
    const index = html.indexOf(`src="./${script}"`, cursor + 1);
    if (index < 0) return false;
    cursor = index;
  }
  return true;
}

function sectionRegistered(sources, sectionId) {
  const registerPattern = new RegExp(
    `(?:ForgeSectionRegistry|forgeSections)\\?\\.register\\?\\.\\(\\s*\\{[\\s\\S]{0,260}?id:\\s*["']${escapeRegExp(sectionId)}["']`,
  );
  const manifestPattern = new RegExp(`\\{\\s*id:\\s*["']${escapeRegExp(sectionId)}["']`);
  return sources.some((source) => registerPattern.test(source.text) || manifestPattern.test(source.text));
}

function sectionLifecycleControlled(sources, sectionId) {
  const patterns = [
    new RegExp(`ForgeSectionRegistry\\?\\.(?:activate|deactivate)\\?\\.\\(\\s*["']${escapeRegExp(sectionId)}["']`),
    new RegExp(`forgeSections\\?\\.setActive\\?\\.\\(\\s*["']${escapeRegExp(sectionId)}["']`),
    new RegExp(`setActive\\?\\.\\(\\s*["']${escapeRegExp(sectionId)}["']`),
    new RegExp(`ForgeSectionRegistry\\?\\.setActive\\?\\.\\(\\s*["']${escapeRegExp(sectionId)}["']`),
    new RegExp(`section:\\s*["']${escapeRegExp(sectionId)}["'][\\s\\S]{0,180}?active:`),
    new RegExp(`type:\\s*["']SET_SURFACE_ACTIVE["'][\\s\\S]{0,180}?section:\\s*["']${escapeRegExp(sectionId)}["']`),
  ];
  return sources.some((source) => patterns.some((pattern) => pattern.test(source.text)));
}

function findInvocations(source) {
  const invocations = [];
  const pattern = /(?<callee>[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*)\s*(?:<[^>]+>)?\s*\(\s*["'](?<command>[a-z][a-z0-9_]{2,})["']/g;
  for (let match; (match = pattern.exec(source.text));) {
    invocations.push({
      file: source.path,
      line: lineOf(source.text, match.index),
      callee: match.groups.callee,
      command: match.groups.command,
      context: source.text.slice(Math.max(0, match.index - 700), match.index + 1200),
    });
  }
  return invocations;
}

function findGlobalWrites(source) {
  const writes = [];
  const pattern = /window\.(__forge[A-Za-z0-9_]+)\s*=(?!=)/g;
  for (let match; (match = pattern.exec(source.text));) {
    writes.push({
      file: source.path,
      line: lineOf(source.text, match.index),
      global: match[1],
    });
  }
  return writes;
}

const manifest = JSON.parse(read("ui/SECTION_OWNERSHIP.json"));
const indexHtml = read("ui/index.html");
const manifestFiles = Array.from(new Set([
  ...manifest.entrypoints,
  ...manifest.sections.flatMap((section) => section.files || []),
]));
const sources = Object.fromEntries(
  manifestFiles.map((relativePath) => [relativePath, { path: relativePath, text: read(relativePath) }]),
);
const sourceList = Object.values(sources);

expect(
  "section registry and bridge must load before app.js",
  scriptOrder(indexHtml, ["dist/forge-section-registry.js", "dist/forge-tauri-bridge.js", "dist/forge-boot.js", "dist/forge-window-controls.js", "dist/forge-webexplorer-config.js", "dist/forge-app.js"]),
);

for (const section of manifest.sections) {
  const sectionSources = section.files.map((file) => sources[file]).filter(Boolean);
  expect(`${section.id}: every owned file must exist`, sectionSources.length === section.files.length);
  expect(`${section.id}: section must be registered`, sectionRegistered(sourceList, section.id));
  if (section.lifecycle !== "always-active" && section.lifecycle !== "shell-section") {
    expect(`${section.id}: lifecycle must be mirrored in ForgeSectionRegistry`, sectionLifecycleControlled(sourceList, section.id));
  }
}

const invocations = sourceList.flatMap(findInvocations);
const invocationsByCommand = new Map();
for (const invocation of invocations) {
  if (!invocationsByCommand.has(invocation.command)) invocationsByCommand.set(invocation.command, []);
  invocationsByCommand.get(invocation.command).push(invocation);
}

for (const rule of manifest.sensitiveCommands) {
  const hits = invocationsByCommand.get(rule.command) || [];
  expect(`${rule.command}: command must be referenced`, hits.length > 0);
  const bridgeHits = hits.filter((hit) => hit.callee === "forgeTauri.invoke" || hit.callee === "forgeInvoke" || hit.callee === "deps.invoke");
  if (rule.requiresBridge) {
    expect(`${rule.command}: command must have a ForgeTauriBridge path`, bridgeHits.length > 0);
  }
  for (const hit of hits) {
    const isBridgeInvoke = hit.callee === "forgeTauri.invoke" || hit.callee === "forgeInvoke" || hit.callee === "deps.invoke";
    if (rule.requiresBridge && !rule.allowRawFallback) {
      expect(
        `${rule.command}: ${hit.file}:${hit.line} must use ForgeTauriBridge`,
        isBridgeInvoke,
      );
    }
    if (rule.forbidDirectInvoke) {
      expect(
        `${rule.command}: ${hit.file}:${hit.line} must not use raw invoke fallback`,
        isBridgeInvoke,
      );
    }
    if (rule.requiresActiveSection) {
      if (!isBridgeInvoke) continue;
      expect(
        `${rule.command}: ${hit.file}:${hit.line} must require the ${rule.owner} section to be active`,
        hit.context.includes(`section: "${rule.owner}"`) && hit.context.includes("requiresActiveSection: true"),
      );
    }
    if (rule.bootSafe) {
      if (!isBridgeInvoke) continue;
      expect(
        `${rule.command}: ${hit.file}:${hit.line} must be bootSafe`,
        hit.context.includes("bootSafe: true"),
      );
    }
  }
}

const globalWrites = sourceList
  .filter((source) => !source.path.startsWith("ui/dist/"))
  .flatMap(findGlobalWrites);
const writesByGlobal = new Map();
for (const write of globalWrites) {
  if (!writesByGlobal.has(write.global)) writesByGlobal.set(write.global, new Set());
  writesByGlobal.get(write.global).add(write.file);
}
for (const [global, files] of writesByGlobal.entries()) {
  if (files.size > 1) {
    warnings.push(`${global} is written by multiple files: ${[...files].join(", ")}`);
  }
}

const commandSummary = {};
for (const [command, hits] of invocationsByCommand.entries()) {
  commandSummary[command] = hits.map((hit) => `${hit.file}:${hit.line}`);
}

if (process.argv.includes("--json")) {
  console.log(JSON.stringify({
    sections: manifest.sections.map((section) => ({
      id: section.id,
      files: section.files,
      lifecycle: section.lifecycle,
      registered: sectionRegistered(section.files.map((file) => sources[file]).filter(Boolean), section.id),
    })),
    commandSummary,
    sharedGlobalWrites: Object.fromEntries(
      [...writesByGlobal.entries()].map(([global, files]) => [global, [...files]]),
    ),
    warnings,
    failures,
  }, null, 2));
}

if (warnings.length && !process.argv.includes("--quiet")) {
  console.warn("[forge-ui-section-audit] warnings");
  for (const warning of warnings) console.warn(`- ${warning}`);
}

if (failures.length) {
  console.error("[forge-ui-section-audit] failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[forge-ui-section-audit] ok");
