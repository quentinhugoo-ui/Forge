import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];

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

function occurrenceOrder(source, ...needles) {
  let cursor = -1;
  for (const needle of needles) {
    const next = source.indexOf(needle, cursor + 1);
    if (next < 0) return false;
    cursor = next;
  }
  return true;
}

function guardedCall(source, command, guard) {
  const at = source.indexOf(command);
  if (at < 0) return false;
  return source.slice(Math.max(0, at - 600), at + 1000).includes(guard);
}

function listFiles(dir, acc = []) {
  if (!existsSync(dir)) return acc;
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) listFiles(path, acc);
    else acc.push(path);
  }
  return acc;
}

const indexHtml = read("ui/index.html");
const appJs = read("ui/src/shell/surface.ts");
const registryJs = read("ui/dist/forge-section-registry.js");
const bridgeJs = read("ui/dist/forge-tauri-bridge.js");
const shellRuntimeJs = read("ui/dist/forge-shell-runtime.js");
const sectionManifestsJs = shellRuntimeJs;
const realEstateBundleJs = read("ui/dist/forge-real-estate.js");
const tradingBundleJs = read("ui/dist/forge-trading.js");
const bangerBundleJs = read("ui/dist/forge-banger.js");
const hardwareBundleJs = read("ui/dist/forge-hardware.js");
const shellMachineTs = read("ui/src/shell/shell-machine.ts");
const shellTypesTs = read("ui/src/shell/types.ts");
const shellActionsTs = read("ui/src/shell/shell-actions.ts");
const shellTauriClientTs = read("ui/src/shell/tauri-client.ts");
const shellTauriBridgeTs = read("ui/src/shell/tauri-bridge.ts");
const hardwareTs = read("ui/src/shell/hardware.ts");
const sectionManifestsTs = read("ui/src/sections/manifests.ts");
const realEstateToolsTs = read("ui/src/sections/real-estate/tools.ts");
const realEstateToolRendererTs = read("ui/src/sections/real-estate/tool-renderer.ts");
const realEstateOnboardingTs = read("ui/src/sections/real-estate/onboarding.ts");
const realEstateOnboardingRuntimeTs = read("ui/src/sections/real-estate/onboarding-runtime.ts");
const realEstateLanguageRuntimeTs = read("ui/src/sections/real-estate/language-runtime.ts");
const realEstateModeRuntimeTs = read("ui/src/sections/real-estate/mode-runtime.ts");
const realEstateRuntimeContextTs = read("ui/src/sections/real-estate/runtime-context.ts");
const realEstatePanelRuntimeTs = read("ui/src/sections/real-estate/panel-runtime.ts");
const tradingControllerTs = read("ui/src/sections/trading/controller.ts");
const tradingStateTs = read("ui/src/sections/trading/state.ts");
const tradingCatalogTs = read("ui/src/sections/trading/catalog.ts");
const bangerControllerTs = read("ui/src/sections/banger/controller.ts");
const bangerCatalogTs = read("ui/src/sections/banger/catalog.ts");
const bootJs = read("ui/src/shell/boot.ts");
const windowControlsJs = read("ui/src/shell/window-controls.ts");
const sidebarJs = read("ui/dist/forge-sidebar.js");
const shellGuardianJs = read("ui/dist/forge-shell-guardian.js");
const searchPaletteJs = read("ui/dist/forge-search-palette.js");
const webExplorerConfigJs = read("ui/dist/forge-webexplorer-config.js");
const tradingJs = read("ui/src/sections/trading/surface.ts");
const bangerJs = read("ui/src/sections/banger/surface.ts");
const mainRs = read("src-tauri/src/main.rs");
const forgeKernelRs = read("src-tauri/src/forge_kernel.rs");
const forgeJobRuntimeRs = read("src-tauri/src/forge_job_runtime.rs");
const realEstateHarvesterRs = read("src-tauri/src/real_estate_harvester.rs");
const realEstateDataSyncBinRs = read("src-tauri/src/bin/real_estate_data_sync.rs");
const realEstateDataSyncRs = read("src-tauri/src/bin/real_estate_data_sync.rs");
const realEstateSourceRegistry = read("source-registry/real-estate-public-sources.json");
const realEstateSourceAudit = read("scripts/real-estate-source-audit.mjs");
const realEstateSourceDiscovery = read("scripts/real-estate-source-discovery.mjs");
const realEstateRawDownloader = read("scripts/real-estate-raw-downloader.mjs");
const realEstateParserRouter = read("scripts/real-estate-parser-router.mjs");
const realEstateEntityResolver = read("scripts/real-estate-entity-resolver.mjs");
const realEstateIntelPackBuilder = read("scripts/real-estate-intel-pack-builder.mjs");
const realEstateKasmSeedBuilder = read("scripts/real-estate-kasm-seed-builder.mjs");
const realEstateBrainCommit = read("scripts/real-estate-brain-commit.mjs");
const realEstateLivingDataflowGraph = read("scripts/real-estate-living-dataflow-graph.mjs");
const realEstateEvidenceMemoryBuilder = read("scripts/real-estate-evidence-memory-builder.mjs");
const realEstateSourcePipeline = read("scripts/real-estate-source-pipeline.mjs");
const tauriBusAudit = read("scripts/forge-tauri-bus-audit.mjs");
const cutoverAudit = read("scripts/forge-cutover-audit.mjs");
const realEstateParserAdapters = read("source-registry/real-estate-parser-adapters.json");
const realEstateToolCells = read("source-registry/real-estate-tool-cells.json");
const sectionOwnership = read("ui/SECTION_OWNERSHIP.json");
const manualJsLock = read("ui/src/MANUAL_JS_LOCK.md");
const realEstateToolsPanelHtml = indexHtml.slice(
  indexHtml.indexOf('id="realEstateToolsPanel"'),
  indexHtml.indexOf('id="webExplorerHistoryPanel"'),
);
const realEstateInitLoopSource = appJs.slice(
  appJs.indexOf("function scheduleRealEstateLlmInitLoop"),
  appJs.indexOf("function realEstateOnboardingQuestionLine"),
);
const realEstateModeSyncSource = appJs.slice(
  appJs.indexOf("function syncRealEstateModeUi"),
  appJs.indexOf("function realEstateCommandFromText"),
);
const topLevelUiJsFiles = listFiles(join(root, "ui"))
  .filter((path) => path.endsWith(".js"))
  .filter((path) => !path.includes(`${join("ui", "assets")}${"\\"}`) && !path.includes(`${join("ui", "dist")}${"\\"}`))
  .map((path) => `ui/${path.slice(join(root, "ui").length + 1).replaceAll("\\", "/")}`);

expect(
  "section registry and tauri bridge must load before app.js",
  occurrenceOrder(
    indexHtml,
    'src="./dist/forge-section-registry.js"',
    'src="./dist/forge-tauri-bridge.js"',
    'src="./dist/forge-shell-runtime.js"',
    'src="./dist/forge-real-estate.js"',
    'src="./dist/forge-boot.js"',
    'src="./dist/forge-window-controls.js"',
    'src="./dist/forge-hardware.js"',
    'src="./dist/forge-sidebar.js"',
    'src="./dist/forge-shell-guardian.js"',
    'src="./dist/forge-search-palette.js"',
    'src="./dist/forge-webexplorer-config.js"',
    'src="./dist/forge-app.js"',
    'src="./dist/forge-trading.js"',
    'src="./dist/forge-banger.js"',
  ),
);
expect("titlebar must not use data-tauri-drag-region", !indexHtml.includes("data-tauri-drag-region"));
expect("section registry global is exposed", registryJs.includes("window.ForgeSectionRegistry"));
expect("tauri bridge global is exposed", bridgeJs.includes("window.ForgeTauriBridge"));
expect("typed shell runtime global is exposed", shellRuntimeJs.includes("window.ForgeShellRuntime"));
expect("manual JS is forbidden and surface cells are TypeScript", manualJsLock.includes("None.") && manualJsLock.includes("ui/src/shell/surface.ts") && manualJsLock.includes("ui/dist/**/*.js"));
for (const jsFile of topLevelUiJsFiles) {
  expect(`manual JS file is forbidden outside dist: ${jsFile}`, false);
}
expect("section manifests live outside app.js", sectionManifestsJs.includes("window.ForgeSectionManifests") && sectionManifestsTs.includes("forgeSectionManifests") && appJs.includes("window.ForgeSectionManifests") && !appJs.includes('registerForgeSection({ id: "shell"'));
expect("real estate tool catalog lives outside app.js", realEstateBundleJs.includes("ForgeRealEstateTools") && realEstateToolsTs.includes("realEstateToolGroups") && appJs.includes("window.ForgeRealEstateTools") && !appJs.includes("const REAL_ESTATE_TOOL_GROUPS = Object.freeze(["));
expect("real estate tool renderer lives outside app.js", realEstateBundleJs.includes("renderToolPanel") && realEstateBundleJs.includes("renderCrmPanel") && realEstateToolRendererTs.includes("commandForRealEstateToolId") && !appJs.includes("function createRealEstateToolButton") && !appJs.includes("function createRealEstateToolGroup"));
expect("real estate onboarding contract lives outside app.js", realEstateBundleJs.includes("FORGE_REAL_ESTATE_ONBOARDING") && realEstateBundleJs.includes("assistant_contract=") && realEstateOnboardingTs.includes("realEstateOnboardingReplyLooksUsable") && appJs.includes("window.ForgeRealEstateOnboarding") && !appJs.includes("assistant_contract=Tu geres l'onboarding"));
expect("trading lifecycle controller is TypeScript source of truth", tradingControllerTs.includes("createForgeTradingController") && tradingBundleJs.includes("ForgeTradingController") && tradingJs.includes("window.ForgeTradingController?.create") && tradingJs.includes("tradingController.open()"));
expect("trading UI state patches are TypeScript source of truth", tradingStateTs.includes("tradingDeactivatePatch") && tradingBundleJs.includes("ForgeTradingState") && tradingJs.includes("ForgeTradingState?.deactivatePatch") && tradingJs.includes("ForgeTradingState?.normalizeChatSubbarMode"));
expect("trading static catalog lives outside trading.js", tradingCatalogTs.includes("ForgeTradingCatalog") && tradingCatalogTs.includes("TRADING_INDICATOR_LIBRARY") && tradingBundleJs.includes("ForgeTradingCatalog") && tradingJs.includes("window.ForgeTradingCatalog") && !tradingJs.includes("const TRADING_INDICATOR_LIBRARY = ["));
expect("banger lifecycle controller is TypeScript source of truth", bangerControllerTs.includes("createForgeBangerController") && bangerBundleJs.includes("ForgeBangerController") && bangerJs.includes("window.ForgeBangerController?.create") && bangerJs.includes("bangerController.toggle()"));
// INGEN COMPUTE §19 Phase 1+4 — purge complète des shaders WebGL2 du
// catalog : makeCube / makeGrid (Phase 1), VS_SDF/FS_SDF (Phase 1),
// VS_MESH/FS_MESH/VS_LINE/FS_LINE (Phase 4). Catalog ne publie plus que
// M4 + axis tables. On valide l'absence pour empêcher toute réapparition.
expect("banger maths catalog purged of WebGL2 shaders", bangerCatalogTs.includes("ForgeBangerCatalog") && bangerCatalogTs.includes("export const M4") && !bangerCatalogTs.includes("export const VS_MESH") && !bangerCatalogTs.includes("export const FS_MESH") && !bangerCatalogTs.includes("export const VS_LINE") && !bangerCatalogTs.includes("export const FS_LINE") && !bangerCatalogTs.includes("export const VS_SDF") && !bangerCatalogTs.includes("export const FS_SDF") && !bangerCatalogTs.includes("export function makeCube") && !bangerCatalogTs.includes("export function makeGrid") && bangerBundleJs.includes("ForgeBangerCatalog") && bangerJs.includes("window.ForgeBangerCatalog") && !bangerJs.includes("function makeCube") && !bangerJs.includes("function makeGrid"));
// INGEN COMPUTE §19 Phase 4 verifier — plus aucun getContext("webgl2")
// ne doit survivre dans le surface du banger.
expect("banger surface no longer initialises a WebGL2 context", !bangerJs.includes("getContext(\"webgl2\"") && !bangerJs.includes("getContext('webgl2'") && bangerJs.includes("new IngenRender("));
expect("banger close action is TypeScript-routed", shellRuntimeJs.includes('"#bangerExitBtn", "close-banger"') && bangerBundleJs.includes('"close-banger"') && !bangerJs.includes('els.exitBtn?.addEventListener("click"'));
expect("trading workspace opens through typed controller action only", tradingControllerTs.includes('registerAction("open-trading"') && !tradingJs.includes("forge:trading-toggle-request") && !tradingJs.includes("handleTradingWorkspaceButtonClick"));
expect(
  "single shell state machine fuses navigation, panels, window and onboarding",
  shellMachineTs.includes("reduceForgeShellState")
    && shellTypesTs.includes("ForgeShellEvent")
    && shellTypesTs.includes("ForgeShellMode")
    && shellTypesTs.includes("SET_SECTION_ACTIVE")
    && shellTypesTs.includes("SET_PANEL")
    && shellTypesTs.includes("SET_WINDOW_COMMAND")
    && shellTypesTs.includes("SET_ONBOARDING")
    && shellTypesTs.includes("activeSections")
    && shellTypesTs.includes("onboarding")
    && shellTypesTs.includes("window")
    && !shellMachineTs.includes("createRealEstateStateMachine")
    && !shellMachineTs.includes("createTradingStateMachine"),
);
expect("shell runtime owns legacy-compatible section bridge", shellRuntimeJs.includes("__forgeActiveSection") && shellRuntimeJs.includes("__forgeSwitchSection") && !appJs.includes("window.__forgeSwitchSection = switchSection"));
expect("shell actions are typed and routed outside app.js", shellActionsTs.includes("ForgeShellActionName") && shellRuntimeJs.includes("shellClickActions") && shellRuntimeJs.includes("shellShortcutActions") && shellRuntimeJs.includes("open-trading") && shellRuntimeJs.includes("toggle-banger") && shellRuntimeJs.includes("toggle-real-estate-tools") && shellRuntimeJs.includes("registerAction") && appJs.includes('registerAction?.("toggle-webexplorer"'));
expect(
  "minimal UI router owns shell clicks and shortcuts",
  shellRuntimeJs.includes("installForgeShellUiRouter")
    && shellRuntimeJs.includes('"#forgeSearchClose", "close-search"')
    && shellRuntimeJs.includes('"#forgeSearchBackdrop", "close-search"')
    && shellRuntimeJs.includes('key: "k"')
    && shellRuntimeJs.includes('action: "toggle-search"')
    && searchPaletteJs.includes('registerAction?.("toggle-search"')
    && !appJs.includes("Ctrl+K / Cmd+K opens the palette globally")
    && !appJs.includes('forgeSearchClose?.addEventListener("click"')
    && !appJs.includes('forgeSearchBackdrop?.addEventListener("click"'),
);
expect(
  "minimal UI router owns static provider panel clicks",
  shellRuntimeJs.includes('"#providerWorkbenchLaunch", "provider-workbench-launch"')
    && shellRuntimeJs.includes('"#providerLauncherGemini", "provider-launch-gemini"')
    && shellRuntimeJs.includes('"#voiceElevenSaveKey", "provider-voice-save-key"')
    && appJs.includes('registerAction?.("provider-workbench-launch"')
    && appJs.includes('registerAction?.("provider-voice-save-key"')
    && !appJs.includes('providerClose) providerClose.addEventListener("click"')
    && !appJs.includes('openAiProviderConnect) openAiProviderConnect.addEventListener("click"')
    && !appJs.includes('voiceElevenSaveKey) voiceElevenSaveKey.addEventListener("click"'),
);
expect(
  "minimal UI router owns static library program atlas and 3d clicks",
  shellRuntimeJs.includes('"#navLibraryBtn", "library-toggle"')
    && shellRuntimeJs.includes('"#navProgramsBtn", "programs-toggle"')
    && shellRuntimeJs.includes('"#myAtlasRefresh", "atlas-refresh"')
    && shellRuntimeJs.includes('"#alpha3dToggle", "alpha-3d-toggle"')
    && appJs.includes('registerAction?.("library-toggle"')
    && appJs.includes('registerAction?.("programs-toggle"')
    && appJs.includes('registerAction?.("alpha-3d-toggle"')
    && !appJs.includes('navLibraryBtn) navLibraryBtn.addEventListener("click"')
    && !appJs.includes('navProgramsBtn) navProgramsBtn.addEventListener("click"')
    && !appJs.includes('alpha3dToggle?.addEventListener("click"'),
);
expect(
  "minimal UI router forwards dataset for static filter clicks",
  shellRuntimeJs.includes("dataset: datasetFor(button)")
    && shellRuntimeJs.includes('".lib-filter-btn"')
    && shellRuntimeJs.includes('"library-filter"')
    && shellRuntimeJs.includes('"atlas-subtab"')
    && appJs.includes('registerAction?.("library-filter"')
    && appJs.includes("payload?.dataset?.filter")
    && appJs.includes("payload?.dataset?.atlasTab")
    && !appJs.includes('document.querySelectorAll(".lib-filter-btn").forEach(btn => btn.addEventListener("click"')
    && !appJs.includes('document.querySelectorAll(".programs-filter-btn").forEach(btn => btn.addEventListener("click"'),
);
expect(
  "minimal UI router owns shared escape overlay handling",
  shellRuntimeJs.includes('key: "escape"')
    && shellRuntimeJs.includes('"escape-overlays"')
    && appJs.includes('registerAction?.("escape-overlays"')
    && appJs.includes("closeProviderOverlay()")
    && appJs.includes("closeProgramsOverlay()")
    && !appJs.includes('if (e.key === "Escape" && providerOverlay')
    && !appJs.includes('if (e.key === "Escape" && programsOverlay'),
);
expect(
  "typed tauri client centralizes command contracts through the bridge",
  shellTauriClientTs.includes("forgeTauriCommandContracts")
    && shellTauriClientTs.includes("forge_kernel")
    && shellTauriClientTs.includes("get_hardware_info")
    && occurrenceOrder(shellTauriClientTs, "optionsFor(command, options)", "window.ForgeTauriBridge.invoke<T>", "throw new Error(`Tauri command unavailable"),
);
expect(
  "Tauri bridge uses explicit Tauri v2 imports without global API exposure",
  shellTauriBridgeTs.includes('from "@tauri-apps/api/core"')
    && shellTauriBridgeTs.includes('from "@tauri-apps/api/event"')
    && !shellTauriBridgeTs.includes("__TAURI__")
    && !shellTauriClientTs.includes("__TAURI__")
    && !windowControlsJs.includes("__TAURI__"),
);
expect(
  "shell surface uses the typed runtime bridge for forge jobs",
  appJs.includes("function forgeInvoke(command")
    && appJs.includes('forgeInvoke("create_forge_pending_job"')
    && appJs.includes('forgeInvoke("list_forge_jobs"')
    && !appJs.includes("function invokeWithTimeout"),
);
expect(
  "trading native commands use the typed runtime bridge first",
  tradingJs.includes("window.ForgeShellRuntime?.tauri?.invoke")
    && occurrenceOrder(tradingJs, "async function invoke(command", "window.ForgeShellRuntime?.tauri?.invoke", "Forge runtime bus unavailable"),
);
expect("boot global is exposed", bootJs.includes("window.ForgeBoot"));
expect("window controls global is exposed", windowControlsJs.includes("window.ForgeWindowControls"));
expect("hardware cell global is exposed", hardwareBundleJs.includes("window.ForgeHardwareCell"));
expect("shell guardian global is exposed", shellGuardianJs.includes("window.ForgeShellGuardian"));
expect("frontend global errors belong to forge boot", bootJs.includes("installGlobalErrorHandlers") && !appJs.includes('window.addEventListener("unhandledrejection"'));
expect("sidebar cell global is exposed", sidebarJs.includes("window.ForgeSidebarCell") && sidebarJs.includes("__forgeSetSidebarCollapsed"));
expect("webexplorer config global is exposed", webExplorerConfigJs.includes("window.ForgeWebExplorerConfig"));
expect("bridge blocks heavy native commands during boot", bridgeJs.includes("bootBlockedCommands"));
expect("section ownership manifest exists", sectionOwnership.includes('"sections"'));
expect(
  "window controls go through boot-safe bridge",
  windowControlsJs.includes("control.request") && windowControlsJs.includes("bootSafe: true") && windowControlsJs.includes("SET_WINDOW_COMMAND"),
);
expect(
  "window controls have a single backend-first owner with native fallback",
  occurrenceOrder(windowControlsJs, "function runMainWindowControl", "invokeWindowCommand(command", "fallbackTauriWindowControl(command)")
    && !shellGuardianJs.includes("event.stopImmediatePropagation?.();\n      invokeWindow(command);"),
);
expect(
  "shell guardian protects section header fallbacks outside app.js",
  shellGuardianJs.includes("installHeaderFallbackGuard")
    && shellGuardianJs.includes("__forgeSetRealEstateModeActive")
    && shellGuardianJs.includes("__forgeOpenWebExplorer")
    && appJs.includes("__forgeSetRealEstateModeActive")
    && sidebarJs.includes("__forgeSetSidebarCollapsed")
    && searchPaletteJs.includes("__forgeOpenSearch"),
);
expect("window controls no longer live in app.js", !appJs.includes("function bindMainWindowControl"));
expect("boot handlers no longer live in app.js", !appJs.includes("function suppressNativeTitleTooltips"));
expect(
  "titlebar and nav shell buttons have a single runtime click owner",
  shellRuntimeJs.includes('"#webexplorer", "toggle-webexplorer"')
    && shellRuntimeJs.includes('"#realEstateModeBtn", "toggle-real-estate"')
    && shellRuntimeJs.includes('"#tradingWorkspaceBtn", "open-trading"')
    && shellRuntimeJs.includes('"#bangerBoomBtn", "toggle-banger"')
    && shellRuntimeJs.includes('"#alphaProofToggle", "toggle-right-panel"')
    && shellRuntimeJs.includes('"#profileBtn", "profile-toggle"')
    && shellRuntimeJs.includes('"[data-profile-action]", "profile-action"')
    && shellRuntimeJs.includes('"#docsClose", "docs-close"')
    && !appJs.includes('webExplorerBtn?.addEventListener("click"')
    && !appJs.includes('realEstateModeBtn?.addEventListener("click"')
    && !appJs.includes('forgeSearchBtn?.addEventListener("click"')
    && !appJs.includes('alphaProofToggle?.addEventListener("click"')
    && !appJs.includes('alphaProofClose?.addEventListener("click"')
    && !appJs.includes('profileBtn?.addEventListener("click"')
    && !appJs.includes('profileMenu?.addEventListener("click"')
    && !appJs.includes('docsClose?.addEventListener("click"')
    && !tradingJs.includes('closest?.("#tradingWorkspaceBtn"')
    && !bangerJs.includes('closest?.("#bangerBoomBtn"')
    && !appJs.includes('closest?.("#forgeSearchBtn")')
    && !appJs.includes('alphaSidebarToggle?.addEventListener("click"')
    && !appJs.includes('realEstateToolsPanelBtn?.addEventListener("click"')
    && !appJs.includes('realEstateCrmPanelBtn?.addEventListener("click"')
    && appJs.includes('registerAction?.("toggle-right-panel"')
    && appJs.includes('registerAction?.("profile-action"')
    && appJs.includes('registerAction?.("docs-close"')
    && realEstatePanelRuntimeTs.includes('registerAction?.("toggle-real-estate-tools"')
    && realEstatePanelRuntimeTs.includes('registerAction?.("toggle-real-estate-contacts"')
    && !appJs.includes("Same defensive capture-phase pattern as search/banger"),
);
expect(
  "section cells expose lifecycle, permissions, commands and compact projections",
  shellTypesTs.includes("ForgeSectionCell")
    && shellTypesTs.includes("ForgeSectionLifecycle")
    && shellTypesTs.includes("ForgeSectionPermission")
    && shellRuntimeJs.includes("applyState(state)")
    && shellRuntimeJs.includes("window.ForgeSectionCells")
    && sectionManifestsTs.includes("permissions:")
    && sectionManifestsTs.includes("commands:")
    && sectionManifestsTs.includes('owns: ["canvas", "chatbar", "rightPanel", "jobs"]'),
);
expect("webexplorer does not trigger heavy real estate sync", appJs.includes("syncRealEstateModeUi({ skipHeavy: true })") && appJs.includes("function syncRealEstateModeUi(options = {})"));
expect("empty canvas log layer cannot flash a scrollbar", appJs.includes('alphaLogLayer.classList.toggle(') && read("ui/styles.css").includes(".log-layer.is-empty"));
expect("shell surface mirrors shell transitions into typed runtime", appJs.includes("const forgeShellRuntime = window.ForgeShellRuntime") && appJs.includes("forgeShellRuntime?.dispatch?.({ type: \"SET_REAL_ESTATE_MODE\"") && appJs.includes("forgeShellRuntime?.dispatch?.({ type: \"BOOT_READY\""));
expect(
  "forge event kernel owns critical shell projection",
  forgeKernelRs.includes("ForgeKernelProjection")
    && forgeKernelRs.includes("forge_kernel_events.jsonl")
    && forgeKernelRs.includes("fn replay")
    && forgeKernelRs.includes("activate_section")
    && forgeKernelRs.includes("set_surface_active")
    && forgeKernelRs.includes("set_real_estate_mode")
    && forgeKernelRs.includes("toggle_left_panel")
    && forgeKernelRs.includes("set_panel")
    && forgeKernelRs.includes("set_overlay")
    && forgeKernelRs.includes("set_onboarding")
    && forgeKernelRs.includes("hardware_observed")
    && mainRs.includes("mod forge_kernel")
    && mainRs.includes("forge_kernel::forge_kernel")
    && shellRuntimeJs.includes("dispatchShellIntentToKernel")
    && shellRuntimeJs.includes("syncShellFromKernel")
    && shellMachineTs.includes("forgeShellStateFromKernelProjection")
    && shellRuntimeJs.includes('"forge_kernel"'),
);
expect(
  "shell bus is kernel-first: intent -> forge_kernel -> projection -> render",
  shellRuntimeJs.includes("dispatchShellIntentToKernel")
    && shellRuntimeJs.includes('source: "ui-intent"')
    && occurrenceOrder(shellRuntimeJs, "dispatchShellIntentToKernel", '"forge_kernel"', "then(acceptProjection)", "catch(fallbackRender)")
    && shellRuntimeJs.includes("renderProjection")
    && shellRuntimeJs.includes("subscribe: (listener"),
);
expect(
  "backend emits one compact UI projection",
  forgeKernelRs.includes("pub shell: JsonValue")
    && forgeKernelRs.includes("pub section: JsonValue")
    && forgeKernelRs.includes("pub left_panel: JsonValue")
    && forgeKernelRs.includes("pub canvas: JsonValue")
    && forgeKernelRs.includes("pub chatbar: JsonValue")
    && forgeKernelRs.includes("pub right_panel: JsonValue")
    && forgeKernelRs.includes("pub jobs: JsonValue")
    && forgeKernelRs.includes("pub hardware: Option<JsonValue>")
    && forgeKernelRs.includes('"set_canvas"')
    && forgeKernelRs.includes('"set_chatbar"')
    && forgeKernelRs.includes('"set_right_panel"')
    && forgeKernelRs.includes('"set_jobs"'),
);
expect(
  "job runtime uses one fused job contract across domains",
  forgeJobRuntimeRs.includes("pub struct ForgeUnifiedJob")
    && forgeJobRuntimeRs.includes("pub id: String")
    && forgeJobRuntimeRs.includes("pub kind: String")
    && forgeJobRuntimeRs.includes("pub payload: JsonValue")
    && forgeJobRuntimeRs.includes("pub status: String")
    && forgeJobRuntimeRs.includes("pub cost: ForgeJobCost")
    && forgeJobRuntimeRs.includes("pub retry: ForgeJobRetry")
    && forgeJobRuntimeRs.includes("pub proof: ForgeJobProof")
    && mainRs.includes("mod forge_job_runtime")
    && mainRs.includes("job: forge_job_runtime::ForgeUnifiedJob")
    && realEstateHarvesterRs.includes("pub unified_jobs: Vec<ForgeUnifiedJob>")
    && realEstateHarvesterRs.includes("fn unified_job_from_real_estate")
    && realEstateDataSyncBinRs.includes("mod forge_job_runtime"),
);
expect(
  "job runtime has an append-only durable ledger and replay snapshot",
  forgeJobRuntimeRs.includes("ForgeJobLedgerEvent")
    && forgeJobRuntimeRs.includes("ForgeJobLedgerSnapshot")
    && forgeJobRuntimeRs.includes("forge_job_ledger.jsonl")
    && forgeJobRuntimeRs.includes("append_job_ledger_event")
    && forgeJobRuntimeRs.includes("recover_job_ledger")
    && mainRs.includes("forge_job_runtime_snapshot")
    && mainRs.includes('append_job_ledger_event(&store_path, "created"')
    && realEstateHarvesterRs.includes("append_job_ledger_event(store_path")
    && shellTauriClientTs.includes("forge_job_runtime_snapshot"),
);
expect(
  "frontend renders from the compact projection shape",
  shellTypesTs.includes("readonly shell?:")
    && shellTypesTs.includes("readonly section?:")
    && shellTypesTs.includes("readonly leftPanel?:")
    && shellTypesTs.includes("readonly canvas?:")
    && shellTypesTs.includes("readonly chatbar?:")
    && shellTypesTs.includes("readonly rightPanel?:")
    && shellTypesTs.includes("readonly jobs?:")
    && shellTypesTs.includes("readonly hardware?:")
    && shellMachineTs.includes("projection.shell")
    && shellMachineTs.includes("projection.section")
    && shellMachineTs.includes("projection.leftPanel || projection.left_panel")
    && shellRuntimeJs.includes("ForgeShellProjection")
    && shellRuntimeJs.includes("forgeRightPanelOpen")
    && shellRuntimeJs.includes("forgeJobsStatus"),
);
expect(
  "shell surface feeds critical canvas, chatbar, jobs, right panel and hardware state into the projection",
  appJs.includes("forgeDispatchProjectionPatch")
    && appJs.includes('type: "SET_CANVAS"')
    && appJs.includes('type: "SET_CHATBAR"')
    && appJs.includes('type: "SET_RIGHT_PANEL"')
    && appJs.includes('type: "SET_JOBS"')
    && appJs.includes('type: "SET_HARDWARE"'),
);
expect(
  "raw Tauri IPC outside the bus is audited as suspect",
  tauriBusAudit.includes("legacy files still bypass the bus")
    && tauriBusAudit.includes("ui/src/shell/tauri-client.ts")
    && tauriBusAudit.includes("--strict"),
);
expect(
  "full app cutover audit locks the eleven migration pillars",
  cutoverAudit.includes("forge_full_app_cutover_audit")
    && cutoverAudit.includes("pillarsChecked: 11")
    && cutoverAudit.includes("shell raw IPC budget exceeded")
    && cutoverAudit.includes("manual JS source debt must be zero")
    && cutoverAudit.includes("forge-boot must not bypass the Tauri bus"),
);
expect("webexplorer real estate suggestion corpus lives outside app.js", !appJs.includes("const REAL_ESTATE_WEB_EXPLORER_SUGGESTIONS = Object.freeze(["));
expect("webexplorer real estate tool copy lives outside app.js", !appJs.includes("const REAL_ESTATE_WEB_EXPLORER_TOOL_COPY = Object.freeze({"));
expect("real estate custom left panel markup is removed", !indexHtml.includes("realEstateAgencyPanel"));
expect("real estate mode hides panel tabs", read("ui/styles.css").includes("body.real-estate-mode .alpha-panel .panel-tabs"));
expect("real estate mode hides default panel actions", read("ui/styles.css").includes(".panel-actions:not(.real-estate-panel-actions)"));
expect(
  "real estate panel actions are ordered",
  occurrenceOrder(
    indexHtml,
    "realEstateNewSessionPanelBtn",
    "realEstateToolsPanelBtn",
    "realEstateAutomationsPanelBtn",
    "realEstatePropertiesPanelBtn",
  ),
);
expect("real estate properties panel label is plural", indexHtml.includes("<span>Biens</span>"));
expect("real estate tools panel exists", indexHtml.includes('id="realEstateToolsPanel"'));
expect("real estate tools panel keeps Google out of the fused tool list", !realEstateToolsPanelHtml.includes("Google Workspace"));
expect(
  "real estate tools panel exposes core agency fallback tools",
  occurrenceOrder(realEstateToolsPanelHtml, "Mandat vendeur", "Diffusion", "Marche &amp; veille", "Pilotage agence", "Back-office", "RH &amp; equipe"),
);
expect("real estate tool clicks open slash program commands", realEstateBundleJs.includes('command: commandForToolId(id)') && realEstateBundleJs.includes('"mandat-vendeur"') && realEstateBundleJs.includes('"recrutement"') && appJs.includes("setComposerCommand(command)"));
expect("real estate tool commands land in the slash rail", appJs.includes("setComposerCommand(command)") && appJs.includes("forgeCanvasChatCommandInput.value = normalized") && appJs.includes("withCanvasCommandPrefix"));
expect("slash rail expands for long real estate commands", appJs.includes("command.length + 2") && read("ui/styles.css").includes("--canvas-command-ch") && read("ui/styles.css").includes("calc(var(--canvas-command-ch"));
expect("right panel shield is available on empty home surfaces", appJs.includes("workspaceHomeOpen") && appJs.includes("canShowRightPanelToggle = hasSource || boomWorkspaceOpen || webExplorerWorkspaceOpen || workspaceHomeOpen"));
expect("real estate mode has isolated session history", appJs.includes("forgeJobsForCurrentShell") && appJs.includes("forgeJobIsRealEstate") && appJs.includes("real_estate_session"));
expect("real estate mode localizes visible shell text", appJs.includes("syncRealEstateFrontendLanguage") && realEstateLanguageRuntimeTs.includes("Nouvelle session immo") && realEstateLanguageRuntimeTs.includes("Dépose n'importe quel fichier"));
expect("real estate mode localizes profile dropdown", realEstateLanguageRuntimeTs.includes("setProfileMenuText") && realEstateLanguageRuntimeTs.includes("Fournisseurs IA") && realEstateLanguageRuntimeTs.includes("Modifier le profil"));
expect("real estate mode treats the local folder as an agency workspace", appJs.includes("workspaceRootLabelForCurrentShell") && realEstateLanguageRuntimeTs.includes("Choisir le dossier agence") && realEstateLanguageRuntimeTs.includes("Afficher le dossier agence"));
expect(
  "real estate LLM init loop never spams provider CLI probes",
  realEstateOnboardingRuntimeTs.includes("statusRefreshInFlight")
    && realEstateOnboardingRuntimeTs.includes("statusLastRefreshAt")
    && realEstateOnboardingRuntimeTs.includes("15000")
    && realEstateOnboardingRuntimeTs.includes("requestLlmStatusRefresh")
    && realEstateOnboardingRuntimeTs.includes("refreshCliProviderStatuses")
    && realEstateOnboardingRuntimeTs.includes("refreshOpenAiProviderStatus")
    && !realEstateModeSyncSource.includes("refreshCliProviderStatuses")
    && !realEstateModeSyncSource.includes("refreshOpenAiProviderStatus"),
);
expect(
  "real estate onboarding is LLM-managed, not canned frontend copy",
  realEstateBundleJs.includes("FORGE_REAL_ESTATE_ONBOARDING")
    && realEstateBundleJs.includes("mode=llm_managed")
    && realEstateOnboardingRuntimeTs.includes("requestLlmTurn")
    && realEstateOnboardingRuntimeTs.includes("recordAnswer")
    && appJs.includes("realEstateOnboardingTurnActive")
    && realEstateOnboardingRuntimeTs.includes("replyLooksUsable")
    && mainRs.includes("forge_canvas_message_forces_runtime_context")
    && mainRs.includes("FORGE_REAL_ESTATE_ONBOARDING:")
    && !appJs.includes("realEstateOnboardingAssistantMessage")
    && !appJs.includes("Bienvenue dans Forge Agence Immo. Je vais te poser")
    && !appJs.includes("C'est noté."),
);
expect("real estate onboarding runtime lives outside shell surface", realEstateOnboardingRuntimeTs.includes("createRealEstateOnboardingRuntime") && realEstateOnboardingRuntimeTs.includes("real_estate_onboarding_state") && realEstateOnboardingRuntimeTs.includes("real_estate_onboarding_answer") && appJs.includes("createRealEstateOnboardingRuntime") && !appJs.includes("async function requestRealEstateOnboardingLlmTurn") && !appJs.includes("async function refreshRealEstateOnboardingState") && !appJs.includes("function syncRealEstateOnboardingMachine"));
expect("real estate slash programs use trailing underscore convention", mainRs.includes("program_to_run") && appJs.includes("real_estate_tool_command_context"));
expect("real estate slash programs inject KASM memory context", mainRs.includes("real_estate_memory_commits.jsonl") && realEstateRuntimeContextTs.includes("FORGE_REAL_ESTATE_MEMORY_CONTEXT") && appJs.includes("loadRealEstateToolCommandContext"));
expect("real estate runtime context lives outside shell surface", realEstateRuntimeContextTs.includes("realEstateChatPlaceholderIdeas") && realEstateRuntimeContextTs.includes("buildRealEstatePrivacyPacket") && realEstateRuntimeContextTs.includes("realEstateCommandPacket") && appJs.includes("buildRealEstateCommandPacket") && !appJs.includes("REAL_ESTATE_CHAT_PLACEHOLDER_IDEAS") && !appJs.includes("REAL_ESTATE_CANVAS_THINKING_PHRASES") && !appJs.includes("function realEstateMemoryCommitLines"));
expect("real estate mode redacts client data before model runtimes", realEstateRuntimeContextTs.includes("redactRealEstateClientData") && appJs.includes("privacyScope") && realEstateRuntimeContextTs.includes("FORGE_REAL_ESTATE_PRIVACY") && mainRs.includes("real_estate_privacy_guard"));
expect("real estate privacy guard writes hash-only audit logs", mainRs.includes("real_estate_privacy_audit.jsonl") && mainRs.includes("originalHash") && mainRs.includes("sanitizedHash"));
expect("real estate tools panel does not expose KASM Programs", !realEstateToolsPanelHtml.includes("KASM Programs"));
expect("real estate tools button opens a dedicated panel", realEstatePanelRuntimeTs.includes("setToolsOpen(true)"));
expect("real estate tools panel is fixed outside left panel flow", read("ui/styles.css").includes("position: fixed") && read("ui/styles.css").includes("overflow: hidden"));
expect("real estate tools panel does not cover the chat bar", read("ui/styles.css").includes("bottom: var(--real-estate-tools-panel-bottom") && realEstatePanelRuntimeTs.includes("syncBounds") && realEstatePanelRuntimeTs.includes("getBoundingClientRect?.()") && read("ui/styles.css").includes("z-index: 26"));
expect("real estate tools panel opens and closes with a soft transition", realEstatePanelRuntimeTs.includes("setFloatingPanelVisibility") && read("ui/styles.css").includes(".real-estate-tools-panel.is-visible") && read("ui/styles.css").includes("transform 180ms cubic-bezier"));
expect("real estate tools use the same custom rail scrollbar as history", indexHtml.includes("realEstateToolsScrollbar") && realEstateBundleJs.includes("bindScrollbar?.(root, scrollbar, thumb)") && appJs.includes("bindForgeCustomScrollbar") && read("ui/styles.css").includes("scrollbar-width: none"));
expect("real estate tools list remains scrollable to the last item", read("ui/styles.css").includes("overscroll-behavior: contain") && read("ui/styles.css").includes("padding: 0 10px 28px 0"));
expect("real estate harvester backend module exists", realEstateHarvesterRs.includes("HarvesterRegistry") && realEstateHarvesterRs.includes("default_registry"));
expect("real estate harvester commands are registered", mainRs.includes("real_estate_harvester_snapshot") && mainRs.includes("real_estate_harvester_run_tool"));
expect("real estate harvester starts in the backend", mainRs.includes("real_estate_harvester::start_background"));
expect("real estate tools read harvester status instead of launching collection on click", appJs.includes("real_estate_harvester_snapshot") && !appJs.includes("runRealEstateHarvesterTool"));
expect("real estate data sync has a headless binary", realEstateDataSyncRs.includes("start_background") && realEstateDataSyncRs.includes("--once"));
expect("real estate data sync writes a status heartbeat", realEstateHarvesterRs.includes("STATUS_FILE") && realEstateHarvesterRs.includes("write_runtime_status"));
expect("real estate public source registry exists", realEstateSourceRegistry.includes('"real-estate-public-mega-harvester"') && realEstateSourceRegistry.includes('"ban_address_api"') && realEstateSourceRegistry.includes('"dvf_donnees_foncieres"'));
expect("real estate source audit validates parsers and live URLs", realEstateSourceAudit.includes("validateRegistry") && realEstateSourceAudit.includes("--live") && realEstateSourceAudit.includes("previewHash"));
expect("real estate source discovery writes source manifests", realEstateSourceDiscovery.includes("source_manifest.jsonl") && realEstateSourceDiscovery.includes("discoverJsonResources") && realEstateSourceDiscovery.includes("discoveredResources"));
expect("real estate raw downloader writes content-addressed files", realEstateRawDownloader.includes("raw_downloads.jsonl") && realEstateRawDownloader.includes("rawPathForHash") && realEstateRawDownloader.includes("cacheStatus"));
expect("real estate raw downloader prioritizes data-bearing resources", realEstateRawDownloader.includes("scoreResource") && realEstateRawDownloader.includes("downloadScore") && realEstateRawDownloader.includes("b.downloadScore - a.downloadScore"));
expect("real estate parser adapter registry captures SOTA tools", realEstateParserAdapters.includes('"duckdb"') && realEstateParserAdapters.includes('"tika"') && realEstateParserAdapters.includes('"docling"') && realEstateParserAdapters.includes('"magika"'));
expect("real estate parser router emits normalized events", realEstateParserRouter.includes("normalized_events.jsonl") && realEstateParserRouter.includes("parseJsonLike") && realEstateParserRouter.includes("eventHash") && realEstateParserRouter.includes("selectAdapter"));
expect("real estate JSONL artifacts are reset per run", realEstateSourceDiscovery.includes("writeFileSync(manifestPath, \"\")") && realEstateRawDownloader.includes("writeFileSync(downloadsPath, \"\")") && realEstateParserRouter.includes("writeFileSync(eventsPath, \"\")") && realEstateEntityResolver.includes("writeFileSync(graphPath, \"\")"));
expect("real estate parser router probes SOTA adapters before native fallback", realEstateParserRouter.includes("detectAvailableAdapters") && realEstateParserRouter.includes("spawnSync(command, [\"--version\"]") && realEstateParserRouter.includes("availableAdapters"));
expect("real estate parser router emits universal ingestion objects", realEstateParserRouter.includes("buildUniversalIngestion") && realEstateParserRouter.includes("entityCandidatesFromEvent") && realEstateParserRouter.includes("metricSeedsFromEvent") && realEstateParserRouter.includes("lineageEvent"));
expect("real estate entity resolver emits a compact graph", realEstateEntityResolver.includes("entity_graph.jsonl") && realEstateEntityResolver.includes("upsertCluster") && realEstateEntityResolver.includes("canonicalEntityId") && realEstateEntityResolver.includes("real_estate_entity_resolver_summary"));
expect("real estate intel pack builder emits LLM-safe packs", realEstateIntelPackBuilder.includes("intel_packs.jsonl") && realEstateIntelPackBuilder.includes("llmContract") && realEstateIntelPackBuilder.includes("usabilityScore") && realEstateIntelPackBuilder.includes("real_estate_intel_pack_builder_summary"));
expect("real estate KASM seed builder emits feature vectors", realEstateKasmSeedBuilder.includes("kasm_metric_seeds.jsonl") && realEstateKasmSeedBuilder.includes("featureVectorForPack") && realEstateKasmSeedBuilder.includes("priorityScoreForVector") && realEstateKasmSeedBuilder.includes("real_estate_kasm_seed_builder_summary"));
expect("real estate brain commit bridge emits semantic memory commit requests", realEstateBrainCommit.includes("real_estate_memory_commits.jsonl") && realEstateBrainCommit.includes("brainCommitRequest") && realEstateBrainCommit.includes("forge-brain-llm-note-v1") && realEstateBrainCommit.includes("ranked_actions.json"));
expect("real estate living dataflow graph materializes incremental graph views", realEstateLivingDataflowGraph.includes("living_dataflow_graph.json") && realEstateLivingDataflowGraph.includes("living_dataflow_graph.jsonl") && realEstateLivingDataflowGraph.includes("inputHash") && realEstateLivingDataflowGraph.includes("status: \"reused\"") && realEstateLivingDataflowGraph.includes("source -> raw -> event -> entity/signal -> intelPack -> score -> action -> memoryFact"));
expect("real estate tool cells manifest every UI tool with schemas and permissions", realEstateToolCells.includes('"kind": "forge_real_estate_tool_cells"') && realEstateToolCells.includes('"inputSchema"') && realEstateToolCells.includes('"outputSchema"') && realEstateToolCells.includes('"permissions"') && realEstateToolCells.includes('"filesystem:raw_client_files"') && realEstateToolCells.includes('"id": "marche-veille"'));
expect("real estate evidence memory builder emits verified fact memory", realEstateEvidenceMemoryBuilder.includes("kind: \"evidence_fact\"") && realEstateEvidenceMemoryBuilder.includes("evidence_memory_facts.jsonl") && realEstateEvidenceMemoryBuilder.includes("evidence_memory_latest.json") && realEstateEvidenceMemoryBuilder.includes("supersedes") && realEstateEvidenceMemoryBuilder.includes("contradicts") && realEstateEvidenceMemoryBuilder.includes("requiredFields") && realEstateEvidenceMemoryBuilder.includes("Do not read raw client files"));
expect("real estate source pipeline orchestrates proofed stages", realEstateSourcePipeline.includes("pipeline_run.json") && realEstateSourcePipeline.includes("runStep(\"audit\"") && realEstateSourcePipeline.includes("runStep(\"resolve\"") && realEstateSourcePipeline.includes("runStep(\"intel\"") && realEstateSourcePipeline.includes("runStep(\"seeds\"") && realEstateSourcePipeline.includes("runStep(\"memory\"") && realEstateSourcePipeline.includes("runStep(\"dataflow\"") && realEstateSourcePipeline.includes("runStep(\"evidence\"") && realEstateSourcePipeline.includes("livingDataflowGraph") && realEstateSourcePipeline.includes("evidenceMemoryFacts") && realEstateSourcePipeline.includes("proofHash") && realEstateSourcePipeline.includes("--adapters"));
expect("WebExplorer surface click binding is boot-safe", appJs.includes('typeof handleWebExplorerSurfaceClick === "function"') && !appJs.includes('addEventListener("click", handleWebExplorerSurfaceClick)'));
expect("real estate mode has a main house section button in the left titlebar", indexHtml.includes("realEstateHomeSectionBtn"));
expect(
  "real estate main house button is before Google in the titlebar",
  indexHtml.indexOf("realEstateHomeSectionBtn") > -1
    && indexHtml.indexOf("webexplorer") > -1
    && indexHtml.indexOf("realEstateHomeSectionBtn") < indexHtml.indexOf("webexplorer"),
);
expect("app marks the section registry ready after boot", appJs.includes("markReady"));

for (const sectionId of ["shell", "alpha", "forge", "webexplorer", "real-estate"]) {
  expect(`section manifest registers section ${sectionId}`, sectionManifestsJs.includes(`id: "${sectionId}"`));
}
expect("trading and banger section registration belongs to manifests", sectionManifestsTs.includes('id: "trading"') && sectionManifestsTs.includes('id: "banger"') && !tradingJs.includes('id: "trading"') && !bangerJs.includes('id: "banger"'));
expect("webexplorer active state is mirrored to registry", appJs.includes('setActive?.("webexplorer"'));
expect("real estate active state is mirrored to registry", realEstateModeRuntimeTs.includes('setActive?.("real-estate"'));
expect("webexplorer active state is mirrored to shell runtime", appJs.includes('SET_SURFACE_ACTIVE", section: "webexplorer"'));
expect("real estate active state is mirrored to shell runtime", appJs.includes('SET_REAL_ESTATE_MODE"') && !appJs.includes('SET_SECTION_ACTIVE", section: "real-estate"'));
expect("trading active state is mirrored through shell runtime only", !tradingJs.includes('activate?.("trading"') && !tradingJs.includes('deactivate?.("trading"') && tradingControllerTs.includes('SET_SURFACE_ACTIVE", section: "trading"') && tradingJs.includes("tradingController?.publishActive"));
expect("banger active state is mirrored through shell runtime only", !bangerControllerTs.includes('activate?.("banger"') && !bangerControllerTs.includes('deactivate?.("banger"') && bangerControllerTs.includes('SET_SURFACE_ACTIVE", section: "banger"') && !bangerJs.includes('SET_SECTION_ACTIVE", section: "banger"') && !bangerJs.includes('ACTIVATE_SECTION", section: "banger"'));
expect("trading charts are gated to the trading section", appJs.includes("alphaShouldRenderTradingChartSurface") && appJs.includes("resetInactiveTradingChartSurface") && tradingJs.includes("forceImmediateRender?.()"));

expect(
  "webexplorer native present must require active section",
  guardedCall(appJs, '"webexplorer_native_present"', "requiresActiveSection: true"),
);
expect(
  "bloomberg native present must require active section",
  guardedCall(appJs, '"bloomberg_live_native_present"', "requiresActiveSection: true"),
);
expect(
  "webexplorer native present must not have raw invoke fallback",
  !/(^|[^.\w])invoke\(\s*["']webexplorer_native_present["']/.test(appJs),
);
expect(
  "bloomberg native present must not have raw invoke fallback",
  !/(^|[^.\w])invoke\(\s*["']bloomberg_live_native_present["']/.test(appJs),
);
expect("bloomberg prewarm must not create windows at boot", !mainRs.includes("prewarm_bloomberg_live_native"));
expect("bloomberg prewarm path is explicitly skipped", mainRs.includes("prewarm.skipped create-on-open"));
expect("bloomberg hide is no-op when no webview exists", mainRs.includes("hide.noop.no-webview"));
expect(
  "hardware scan stays off the UI thread",
  mainRs.includes("spawn_blocking(scan::GpuNodeRuntime::bootstrap_best_effort)"),
);
expect(
  "hardware info is a global shell command, not an active section command",
  guardedCall(hardwareTs, '"get_hardware_info"', "requiresActiveSection: false") && hardwareTs.includes("retryTimer") && mainRs.includes("windows_gpu_name_fallback()"),
);

if (failures.length) {
  console.error("[forge-ui-smoke] failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[forge-ui-smoke] ok");
