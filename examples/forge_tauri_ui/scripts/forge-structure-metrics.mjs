import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(__dirname, "..", "..", "..");
const protectedPath = "C:\\Users\\quent\\Documents\\EVE\\MAP";
const excludedDirNames = new Set([
  ".git",
  ".vs",
  "node_modules",
  "target",
  "dist",
  "ui\\dist",
  ".forge-store",
  ".forge-data",
  ".codex-target",
  ".codex-schema",
]);
const excludedExactRelativeDirs = new Set([
  "examples/forge_tauri_ui/ui/dist",
]);
const sourceExtensions = new Set([
  ".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".json", ".toml", ".md", ".css", ".html", ".wit", ".td", ".yml", ".yaml",
]);
const docExtensions = new Set([".md"]);

function pathWithinWorkspace(path) {
  const rel = relative(workspaceRoot, path);
  return rel && !rel.startsWith("..") && !rel.includes(":") ? rel : rel === "" ? "" : null;
}

function shouldSkipDir(path) {
  const rel = pathWithinWorkspace(path);
  if (rel == null) return true;
  const normalized = rel.replaceAll("\\", "/");
  const basename = normalized.split("/").pop() || "";
  if (excludedDirNames.has(basename)) return true;
  if (excludedExactRelativeDirs.has(normalized)) return true;
  return false;
}

function countLines(text) {
  if (!text) return 0;
  return text.split(/\r?\n/).length;
}

function bucketForFile(path) {
  const ext = extname(path).toLowerCase();
  if (docExtensions.has(ext)) return "docs";
  if (sourceExtensions.has(ext)) return "source";
  return "other";
}

const metrics = {
  schema: "forge.structure.metrics.v1",
  workspaceRoot,
  protectedPath,
  excludedDirs: [],
  scannedDirs: 0,
  files: {
    total: 0,
    source: 0,
    docs: 0,
    other: 0,
  },
  lines: {
    total: 0,
    source: 0,
    docs: 0,
    other: 0,
  },
};

function walk(dir) {
  if (shouldSkipDir(dir)) {
    const rel = pathWithinWorkspace(dir);
    if (rel) metrics.excludedDirs.push(rel.replaceAll("\\", "/"));
    return;
  }
  metrics.scannedDirs += 1;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const fullPath = join(dir, entry.name);
    const rel = pathWithinWorkspace(fullPath);
    if (rel == null) continue;
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) {
      walk(fullPath);
      continue;
    }
    if (!entry.isFile()) continue;
    const bucket = bucketForFile(fullPath);
    const lines = bucket === "other" ? 0 : countLines(readFileSync(fullPath, "utf8"));
    metrics.files.total += 1;
    metrics.files[bucket] += 1;
    metrics.lines.total += lines;
    metrics.lines[bucket] += lines;
  }
}

walk(workspaceRoot);
metrics.excludedDirs = Array.from(new Set(metrics.excludedDirs)).sort();
metrics.proofHash = createHash("sha256")
  .update(JSON.stringify(metrics))
  .digest("hex");

console.log(JSON.stringify(metrics, null, 2));
