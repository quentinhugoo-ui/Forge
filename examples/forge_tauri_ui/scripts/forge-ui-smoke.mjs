import { existsSync, readFileSync } from "node:fs";
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

const indexHtml = read("ui/index.html");
const appJs = read("ui/app.js");
const registryJs = read("ui/forge-section-registry.js");
const bridgeJs = read("ui/forge-tauri-bridge.js");
const bootJs = read("ui/forge-boot.js");
const windowControlsJs = read("ui/forge-window-controls.js");
const webExplorerConfigJs = read("ui/forge-webexplorer-config.js");
const tradingJs = read("ui/trading.js");
const bangerJs = read("ui/banger.js");
const mainRs = read("src-tauri/src/main.rs");
const realEstateHarvesterRs = read("src-tauri/src/real_estate_harvester.rs");
const realEstateDataSyncRs = read("src-tauri/src/bin/real_estate_data_sync.rs");
const realEstateSourceRegistry = read("source-registry/real-estate-public-sources.json");
const realEstateSourceAudit = read("scripts/real-estate-source-audit.mjs");
const realEstateSourceDiscovery = read("scripts/real-estate-source-discovery.mjs");
const realEstateRawDownloader = read("scripts/real-estate-raw-downloader.mjs");
const realEstateParserRouter = read("scripts/real-estate-parser-router.mjs");
const realEstateSourcePipeline = read("scripts/real-estate-source-pipeline.mjs");
const realEstateParserAdapters = read("source-registry/real-estate-parser-adapters.json");
const sectionOwnership = read("ui/SECTION_OWNERSHIP.json");
const realEstateToolsPanelHtml = indexHtml.slice(
  indexHtml.indexOf('id="realEstateToolsPanel"'),
  indexHtml.indexOf('id="webExplorerHistoryPanel"'),
);

expect(
  "section registry and tauri bridge must load before app.js",
  occurrenceOrder(
    indexHtml,
    'src="./forge-section-registry.js"',
    'src="./forge-tauri-bridge.js"',
    'src="./forge-boot.js"',
    'src="./forge-window-controls.js"',
    'src="./forge-webexplorer-config.js"',
    'src="./app.js"',
  ),
);
expect("titlebar must not use data-tauri-drag-region", !indexHtml.includes("data-tauri-drag-region"));
expect("section registry global is exposed", registryJs.includes("window.ForgeSectionRegistry"));
expect("tauri bridge global is exposed", bridgeJs.includes("window.ForgeTauriBridge"));
expect("boot global is exposed", bootJs.includes("window.ForgeBoot"));
expect("window controls global is exposed", windowControlsJs.includes("window.ForgeWindowControls"));
expect("webexplorer config global is exposed", webExplorerConfigJs.includes("window.ForgeWebExplorerConfig"));
expect("bridge blocks heavy native commands during boot", bridgeJs.includes("bootBlockedCommands"));
expect("section ownership manifest exists", sectionOwnership.includes('"sections"'));
expect(
  "window controls go through boot-safe bridge",
  windowControlsJs.includes("control.request") && windowControlsJs.includes("bootSafe: true"),
);
expect("window controls no longer live in app.js", !appJs.includes("function bindMainWindowControl"));
expect("boot handlers no longer live in app.js", !appJs.includes("function suppressNativeTitleTooltips"));
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
expect("real estate tool clicks open slash program commands", appJs.includes('command: `/${id.replace(/-/g, "_")}_`') && appJs.includes('"mandat-vendeur"') && appJs.includes('"recrutement"'));
expect("real estate tool commands land in the slash rail", appJs.includes("setComposerCommand(command)") && appJs.includes("forgeCanvasChatCommandInput.value = normalized") && appJs.includes("withCanvasCommandPrefix"));
expect("slash rail expands for long real estate commands", appJs.includes("command.length + 2") && read("ui/styles.css").includes("--canvas-command-ch") && read("ui/styles.css").includes("calc(var(--canvas-command-ch"));
expect("right panel shield is available on empty home surfaces", appJs.includes("workspaceHomeOpen") && appJs.includes("canShowRightPanelToggle = hasSource || boomWorkspaceOpen || webExplorerWorkspaceOpen || workspaceHomeOpen"));
expect("real estate mode has isolated session history", appJs.includes("forgeJobsForCurrentShell") && appJs.includes("forgeJobIsRealEstate") && appJs.includes("real_estate_session"));
expect("real estate mode localizes visible shell text", appJs.includes("syncRealEstateFrontendLanguage") && appJs.includes("Nouvelle session immo") && appJs.includes("Dépose n'importe quel fichier"));
expect("real estate mode localizes profile dropdown", appJs.includes("setProfileMenuText") && appJs.includes("Fournisseurs IA") && appJs.includes("Modifier le profil"));
expect("real estate mode treats the local folder as an agency workspace", appJs.includes("workspaceRootLabelForCurrentShell") && appJs.includes("Choisir le dossier agence") && appJs.includes("Afficher le dossier agence"));
expect("real estate slash programs use trailing underscore convention", mainRs.includes("program_to_run") && appJs.includes("real_estate_tool_command_context"));
expect("real estate mode redacts client data before model runtimes", appJs.includes("redactRealEstateClientData") && appJs.includes("privacyScope") && appJs.includes("FORGE_REAL_ESTATE_PRIVACY") && mainRs.includes("real_estate_privacy_guard"));
expect("real estate privacy guard writes hash-only audit logs", mainRs.includes("real_estate_privacy_audit.jsonl") && mainRs.includes("originalHash") && mainRs.includes("sanitizedHash"));
expect("real estate tools panel does not expose KASM Programs", !realEstateToolsPanelHtml.includes("KASM Programs"));
expect("real estate tools button opens a dedicated panel", appJs.includes("setRealEstateToolsPanelOpen(true)"));
expect("real estate tools panel is fixed outside left panel flow", read("ui/styles.css").includes("position: fixed") && read("ui/styles.css").includes("overflow: hidden"));
expect("real estate tools panel does not cover the chat bar", read("ui/styles.css").includes("bottom: var(--real-estate-tools-panel-bottom") && appJs.includes("syncRealEstatePanelBounds") && appJs.includes("getBoundingClientRect?.()") && read("ui/styles.css").includes("z-index: 26"));
expect("real estate tools panel opens and closes with a soft transition", appJs.includes("setRealEstateFloatingPanelVisibility") && read("ui/styles.css").includes(".real-estate-tools-panel.is-visible") && read("ui/styles.css").includes("transform 180ms cubic-bezier"));
expect("real estate tools use the same custom rail scrollbar as history", indexHtml.includes("realEstateToolsScrollbar") && appJs.includes("bindForgeCustomScrollbar?.(root, realEstateToolsScrollbar") && read("ui/styles.css").includes("scrollbar-width: none"));
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
expect("real estate parser adapter registry captures SOTA tools", realEstateParserAdapters.includes('"duckdb"') && realEstateParserAdapters.includes('"tika"') && realEstateParserAdapters.includes('"docling"') && realEstateParserAdapters.includes('"magika"'));
expect("real estate parser router emits normalized events", realEstateParserRouter.includes("normalized_events.jsonl") && realEstateParserRouter.includes("parseJsonLike") && realEstateParserRouter.includes("eventHash") && realEstateParserRouter.includes("selectAdapter"));
expect("real estate parser router probes SOTA adapters before native fallback", realEstateParserRouter.includes("detectAvailableAdapters") && realEstateParserRouter.includes("spawnSync(command, [\"--version\"]") && realEstateParserRouter.includes("availableAdapters"));
expect("real estate source pipeline orchestrates proofed stages", realEstateSourcePipeline.includes("pipeline_run.json") && realEstateSourcePipeline.includes("runStep(\"audit\"") && realEstateSourcePipeline.includes("proofHash") && realEstateSourcePipeline.includes("--adapters"));
expect("real estate mode has a main house section button in the left titlebar", indexHtml.includes("realEstateHomeSectionBtn"));
expect(
  "real estate main house button is before Google in the titlebar",
  indexHtml.indexOf("realEstateHomeSectionBtn") > -1
    && indexHtml.indexOf("webexplorer") > -1
    && indexHtml.indexOf("realEstateHomeSectionBtn") < indexHtml.indexOf("webexplorer"),
);
expect("app marks the section registry ready after boot", appJs.includes("markReady"));

for (const sectionId of ["shell", "alpha", "forge", "webexplorer", "real-estate"]) {
  expect(`app registers section ${sectionId}`, appJs.includes(`id: "${sectionId}"`));
}
expect("trading registers with the section registry", tradingJs.includes('id: "trading"'));
expect("banger registers with the section registry", bangerJs.includes('id: "banger"'));
expect("webexplorer active state is mirrored to registry", appJs.includes('setActive?.("webexplorer"'));
expect("real estate active state is mirrored to registry", appJs.includes('setActive?.("real-estate"'));
expect("trading active state is mirrored to registry", tradingJs.includes('activate?.("trading"') && tradingJs.includes('deactivate?.("trading"'));
expect("banger active state is mirrored to registry", bangerJs.includes('activate?.("banger"') && bangerJs.includes('deactivate?.("banger"'));
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

if (failures.length) {
  console.error("[forge-ui-smoke] failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[forge-ui-smoke] ok");
