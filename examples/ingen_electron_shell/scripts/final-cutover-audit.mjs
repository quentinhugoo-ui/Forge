import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const shellRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const generatedRoot = join(shellRoot, "src", "shared", "generated");

function readJson(path) {
  return JSON.parse(readFileSync(join(shellRoot, path), "utf8"));
}

function readText(path) {
  return readFileSync(join(shellRoot, path), "utf8");
}

function sha256(value) {
  return createHash("sha256").update(typeof value === "string" ? value : JSON.stringify(value)).digest("hex");
}

function check(id, passed, evidence, severity = "blocker") {
  return {
    id,
    passed: Boolean(passed),
    severity,
    evidence
  };
}

const packageJson = readJson("package.json");
const ipcManifest = readJson("src/shared/generated/forge-ipc.manifest.generated.json");
const generatedIpc = readText("src/shared/generated/forge-ipc.generated.ts");
const main = readText("src/main/main.ts");
const rustBackend = readText("src/main/rust-backend.ts");
const preload = readText("src/preload/preload.ts");
const ipcContract = readText("src/shared/ipc-contract.ts");
const app = readText("src/renderer/App.tsx");
const styles = readText("src/renderer/styles.css");
const sidebar = readText("src/renderer/SidebarSlice.tsx");
const rendererFiles = readdirSync(join(shellRoot, "src", "renderer"));
const scripts = readdirSync(join(shellRoot, "scripts"));

const combinedProductText = [
  JSON.stringify(packageJson),
  generatedIpc,
  main,
  rustBackend,
  preload,
  ipcContract,
  app,
  styles,
  sidebar,
  scripts.join("\n")
].join("\n");
const forbiddenRuntimeName = "sli" + "nt";

const expectedApiMethods = [
  "getHeaderSnapshot",
  "dispatchHeaderCommand",
  "getSidebarSnapshot",
  "dispatchSidebarCommand",
  "getPanelsChatBottomSnapshot",
  "dispatchPanelsChatBottomCommand",
  "getCanvasSurfacesSnapshot",
  "dispatchCanvasSurfacesCommand",
  "getRightPanelSnapshot",
  "dispatchRightPanelCommand"
];

const checks = [
  check(
    "electron_product_shell_only",
    !combinedProductText.toLowerCase().includes(forbiddenRuntimeName) &&
      !existsSync(join(shellRoot, "..", "ingen_native_front")) &&
      existsSync(join(shellRoot, "public", "shell-assets")),
    {
      forbiddenTokenInProductText: combinedProductText.toLowerCase().includes(forbiddenRuntimeName),
      removedNativeFront: !existsSync(join(shellRoot, "..", "ingen_native_front")),
      shellAssets: existsSync(join(shellRoot, "public", "shell-assets"))
    }
  ),
  check(
    "typed_ipc_contract_generated",
    ipcManifest.schema === "ingen.electron.ipc_contract_manifest.v1" &&
      generatedIpc.includes('export type FrontSliceMode = "electron" | "shadow"'),
    {
      schema: ipcManifest.schema,
      productModes: generatedIpc.includes('export type FrontSliceMode = "electron" | "shadow"')
    }
  ),
  check(
    "rust_backend_projection_required",
    main.includes("await refreshRustBackendProjection(shellRoot)") &&
      main.includes("cachedRustBackendProjection") &&
      rustBackend.includes("ingen.native_services.electron_backend_projection.v1") &&
      rustBackend.includes("forge-cargo.ps1"),
    {
      startupRefresh: main.includes("await refreshRustBackendProjection(shellRoot)"),
      cachedProjection: main.includes("cachedRustBackendProjection"),
      backendSchema: rustBackend.includes("ingen.native_services.electron_backend_projection.v1"),
      forgeCargoBridge: rustBackend.includes("forge-cargo.ps1")
    }
  ),
  check(
    "electron_launcher_present",
    existsSync(join(shellRoot, "run_ingen_electron_shell.cmd")) &&
      existsSync(join(shellRoot, "run_ingen_electron_shell.vbs")) &&
      readText("run_ingen_electron_shell.cmd").includes("FORGE_FRONT_SLICE_HEADER=electron") &&
      (readText("run_ingen_electron_shell.cmd").includes("npm.cmd run launch") ||
        readText("run_ingen_electron_shell.cmd").includes("node_modules\\electron\\dist\\electron.exe")),
    {
      cmd: existsSync(join(shellRoot, "run_ingen_electron_shell.cmd")),
      vbs: existsSync(join(shellRoot, "run_ingen_electron_shell.vbs"))
    }
  ),
  check(
    "electron_security_defaults",
    main.includes("contextIsolation: true") &&
      main.includes("nodeIntegration: false") &&
      main.includes("sandbox: true") &&
      main.includes("webSecurity: true") &&
      main.includes("setWindowOpenHandler") &&
      main.includes("will-navigate"),
    {
      contextIsolation: main.includes("contextIsolation: true"),
      nodeIntegrationFalse: main.includes("nodeIntegration: false"),
      sandbox: main.includes("sandbox: true"),
      webSecurity: main.includes("webSecurity: true"),
      windowOpenGuard: main.includes("setWindowOpenHandler"),
      navigationGuard: main.includes("will-navigate")
    }
  ),
  check(
    "strict_context_bridge_ipc",
    preload.includes("contextBridge.exposeInMainWorld(\"forgeShell\", forgeShell)") &&
      !preload.includes("sendSync") &&
      !preload.includes("ipcRenderer.send(") &&
      expectedApiMethods.every((method) => preload.includes(method)),
    {
      rawSend: preload.includes("ipcRenderer.send("),
      sendSync: preload.includes("sendSync"),
      methods: expectedApiMethods.filter((method) => preload.includes(method))
    }
  ),
  check(
    "renderer_slice_files_present",
    [
      "App.tsx",
      "SidebarSlice.tsx",
      "CanvasSurfacesSlice.tsx",
      "RightPanelSlice.tsx",
      "PanelsChatBottomSlice.tsx"
    ].every((file) => rendererFiles.includes(file)),
    { rendererFiles }
  ),
  check(
    "current_frontier_stack_pinned",
    packageJson.dependencies.react === "19.2.7" &&
      packageJson.dependencies["react-dom"] === "19.2.7" &&
      packageJson.devDependencies.electron === "42.3.3" &&
      packageJson.devDependencies.vite === "8.0.16" &&
      packageJson.devDependencies["@playwright/test"] === "1.60.0",
    {
      react: packageJson.dependencies.react,
      electron: packageJson.devDependencies.electron,
      vite: packageJson.devDependencies.vite,
      playwright: packageJson.devDependencies["@playwright/test"]
    },
    "warning"
  )
];

const blockers = checks.filter((entry) => entry.severity === "blocker" && !entry.passed);
const warnings = checks.filter((entry) => entry.severity === "warning" && !entry.passed);
const manifest = {
  schema: "ingen.electron.final_cutover_audit.v2",
  date: "2026-06-10",
  status: blockers.length === 0 ? "cutover_complete" : "blocked",
  inputs: {
    ipc_manifest_hash: ipcManifest.proof_hash
  },
  checks,
  blockers,
  warnings,
  gates_to_run: ["npm.cmd run typecheck", "npm.cmd test", "npm.cmd run build"],
  proof_hash: ""
};
manifest.proof_hash = sha256({ ...manifest, proof_hash: "" });

writeFileSync(join(generatedRoot, "final-cutover-audit.generated.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify({ status: manifest.status, blockers: blockers.length, warnings: warnings.length, proof_hash: manifest.proof_hash }, null, 2));

if (blockers.length > 0) {
  process.exitCode = 1;
}
