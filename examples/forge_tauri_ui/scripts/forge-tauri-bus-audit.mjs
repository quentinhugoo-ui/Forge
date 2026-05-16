import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const strict = process.argv.includes("--strict");
const allowedBusOwners = new Set([
  "ui/src/shell/tauri-bridge.ts",
  "ui/src/shell/tauri-client.ts",
]);
const rawInvokePatterns = [
  ".core.invoke",
  "tauri.core.invoke",
  "tauriApi.core.invoke",
  "window.__TAURI__?.core?.invoke",
  "window.__TAURI__.core.invoke",
];

function listFiles(dir, acc = []) {
  if (!existsSync(dir)) return acc;
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      if (entry === "dist" || entry === "node_modules") continue;
      listFiles(path, acc);
    } else if (/\.(js|ts)$/.test(entry)) {
      acc.push(path);
    }
  }
  return acc;
}

const suspects = [];
for (const file of listFiles(join(root, "ui"))) {
  const rel = `ui/${relative(join(root, "ui"), file).replaceAll("\\", "/")}`;
  if (allowedBusOwners.has(rel)) continue;
  const source = readFileSync(file, "utf8");
  const matched = rawInvokePatterns.filter((pattern) => source.includes(pattern));
  if (matched.length) suspects.push({ rel, matched });
}

if (suspects.length) {
  console.log(`[forge-tauri-bus-audit] ${suspects.length} legacy files still bypass the bus:`);
  for (const item of suspects) {
    console.log(`- ${item.rel}: ${item.matched.join(", ")}`);
  }
  if (strict) process.exit(1);
} else {
  console.log("[forge-tauri-bus-audit] ok");
}
