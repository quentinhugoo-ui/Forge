import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..", "..", "..");

function read(rel) {
  return readFileSync(resolve(root, rel), "utf8");
}

const AGENTS = read("AGENTS.md");
const ARCH = read("FORGE_RUNTIME_ARCHITECTURE.md");
const ROADMAP = read("ROADMAP.md");
const SECTION_CONTRACT = read("examples/forge_tauri_ui/ui/SECTION_CONTRACT.md");
const ownership = JSON.parse(read("examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json"));

const failures = [];

function expect(label, condition) {
  if (!condition) failures.push(label);
}

const sectionIds = Array.isArray(ownership.sections) ? ownership.sections.map((section) => section.id) : [];

expect(
  "AGENTS lists collection_os and generated ui/dist policy",
  AGENTS.includes("examples/forge_tauri_ui/src-tauri/src/collection_os.rs")
    && AGENTS.includes("ui/dist/**/*.js")
    && AGENTS.includes("tauri-bridge.ts"),
);

expect(
  "SECTION_CONTRACT keeps shared bridge and generated bundle policy",
  SECTION_CONTRACT.includes("SECTION_OWNERSHIP.json")
    && SECTION_CONTRACT.includes("ui/src/shell/tauri-bridge.ts")
    && SECTION_CONTRACT.includes("ui/dist/**/*.js"),
);

expect(
  "FORGE_RUNTIME_ARCHITECTURE keeps state kernel and ownership bridge",
  ARCH.includes("## Canonical State Kernel")
    && ARCH.includes("collection_os.rs")
    && ARCH.includes("SECTION_OWNERSHIP.json")
    && ARCH.includes("ui/dist/**/*.js"),
);

expect(
  "ROADMAP keeps repo compression block and verified state",
  ROADMAP.includes("### 11. Repo Compression And Safety")
    && ROADMAP.includes("## Current Verified State"),
);

expect(
  "All active sections appear in AGENTS or SECTION_CONTRACT",
  sectionIds.every((id) => AGENTS.includes(id) || SECTION_CONTRACT.includes(id) || ARCH.includes(id)),
);

if (failures.length) {
  console.error("[forge-doc-code-map-smoke] failed");
  for (const failure of failures) console.error(` - ${failure}`);
  process.exit(1);
}

console.log("[forge-doc-code-map-smoke] ok");
