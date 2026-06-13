import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const shellRoot = join(here, "..");
const repoRoot = join(shellRoot, "..", "..");
const generatedTs = join(shellRoot, "src", "shared", "generated", "forge-ipc.generated.ts");
const manifestPath = join(shellRoot, "src", "shared", "generated", "forge-ipc.manifest.generated.json");

const result = spawnSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-Command",
    `& { & '${join(repoRoot, "scripts", "forge-cargo.ps1").replace(/'/g, "''")}' @args }`,
    "run",
    "--manifest-path",
    "examples\\ingen_electron_shell\\contract\\Cargo.toml",
    "--",
    generatedTs
  ],
  {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe"
  }
);

if (result.status !== 0) {
  process.stderr.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}

if (!existsSync(generatedTs)) {
  throw new Error(`Rust IPC generator did not create ${generatedTs}`);
}

const generated = readFileSync(generatedTs, "utf8");
const proofHash = createHash("sha256").update(generated).digest("hex");
const manifest = {
  schema: "ingen.electron.ipc_contract_manifest.v1",
  source: "examples/ingen_electron_shell/contract/src/main.rs",
  generated: "examples/ingen_electron_shell/src/shared/generated/forge-ipc.generated.ts",
  generator: "rust:ingen-electron-ipc-contract",
  version: 1,
  proof_hash: proofHash
};

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify(manifest, null, 2));
