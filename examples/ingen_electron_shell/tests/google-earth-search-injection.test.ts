import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const appSource = readFileSync(join(process.cwd(), "src", "renderer", "App.tsx"), "utf8");
const canvasSource = readFileSync(join(process.cwd(), "src", "renderer", "CanvasSurfacesSlice.tsx"), "utf8");
const geoEntitiesSource = readFileSync(join(process.cwd(), "src", "renderer", "assistant-geo-entities.ts"), "utf8");
const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(process.cwd(), "src", "preload", "preload.ts"), "utf8");
const contractSource = readFileSync(join(process.cwd(), "src", "shared", "ipc-contract.ts"), "utf8");

describe("Google Earth search injection", () => {
  it("passes the assistant's marked geo entity to Google Earth without changing the page URL", () => {
    expect(appSource).toContain("latestAssistantGeoEntityLabel");
    expect(appSource).toContain("primaryAssistantGeoEntityLabel(latestAssistantText)");
    expect(appSource).toContain("mapsUrl={mapsWebviewUrl}");
    expect(appSource).toContain("mapsSearchQuery={latestAssistantGeoEntityLabel}");
    expect(canvasSource).toContain("googleEarthSearchInjectionScript");
    expect(canvasSource).toContain("executeJavaScript(googleEarthSearchInjectionScript(query), true)");
    expect(canvasSource).toContain("input, textarea, [contenteditable='true']");
    expect(canvasSource).toContain("mapsSearchQuery");
    expect(canvasSource).not.toContain("earth.google.com/web/search");
    expect(geoEntitiesSource).toContain("extractAssistantGeoEntities");
    expect(geoEntitiesSource).toContain("primaryAssistantGeoEntityLabel");
    expect(geoEntitiesSource).not.toContain("googleEarthSearchUrlForGeoEntity");
  });

  it("captures Google Earth as a Monster native tandem DOM/RAM cartography target", () => {
    expect(contractSource).toContain('schema: "forge.webexplorer.dom_ram_cartography.v1"');
    expect(contractSource).toContain("NativeDomRamUiTreeNode");
    expect(contractSource).toContain("NativeDomRamUiTreeLandmark");
    expect(contractSource).toContain("googleEarthSearchBar");
    expect(contractSource).toContain("searchCandidates");
    expect(contractSource).toContain('schema: "forge.webexplorer.dom_ram_ui_tree.v1"');
    expect(contractSource).toContain('lane: "native_tandem_dom_ram"');
    expect(contractSource).toContain('nativeDomain: "dom_ram"');
    expect(preloadSource).toContain("captureMapsDomRamCartography()");
    expect(preloadSource).toContain('ipcRenderer.invoke("forge:maps-dom-ram-cartography-capture")');
    expect(mainSource).toContain('on("did-attach-webview"');
    expect(mainSource).toContain("params?: { src?: unknown; partition?: unknown }");
    expect(mainSource).toContain("const attachmentParams = params ?? {}");
    expect(mainSource).toContain("typeof attachmentParams.src");
    expect(mainSource).toContain('partition === "persist:ingen-maps"');
    expect(mainSource).toContain("rememberMapsDomWebviewGuest(webContents, src)");
    expect(mainSource).toContain("DOMSnapshot.captureSnapshot");
    expect(mainSource).toContain("domSnapshotUiTree(cdpSnapshot)");
    expect(mainSource).toContain("googleEarthSearchBarLandmarks(nodes)");
    expect(mainSource).toContain('role: "google_earth_search_bar"');
    expect(mainSource).toContain("primaryAssistantGeoEntityLabelFromText(text)");
    expect(mainSource).toContain("assistantMapsSearchLabelFromText(message.text)");
    expect(mainSource).toContain("mapsResultTargetFromText(text)");
    expect(mainSource).toContain("injectNativeMapsSearchViaLockedLandmark(label)");
    expect(mainSource).toContain("cachedGoogleEarthSearchLockFor(target)");
    expect(mainSource).toContain("rememberGoogleEarthSearchLock(target, landmark)");
    expect(mainSource).toContain("clearGoogleEarthSearchLock(target)");
    expect(mainSource).toContain("dispatchGoogleEarthSearchFromLock(debug, query, cachedLock)");
    expect(mainSource).toContain("googleEarthLockedSearchPrepareFunction()");
    expect(mainSource).toContain("clipboard.writeText(query)");
    expect(mainSource).toContain('key: "v", code: "KeyV"');
    expect(mainSource).not.toContain('key: "ArrowDown"');
    expect(mainSource).toContain('debug.sendCommand("Input.dispatchKeyEvent", { type: "rawKeyDown"');
    expect(mainSource).toContain('debug.sendCommand("DOM.resolveNode", { backendNodeId: lock.backendNodeId })');
    expect(mainSource).toContain('debug.sendCommand("Runtime.callFunctionOn"');
    expect(mainSource.indexOf("cachedGoogleEarthSearchLockFor(target)")).toBeLessThan(
      mainSource.indexOf("captureMapsDomRamUiTreeForTarget(target)")
    );
    expect(mainSource).toContain("await injectAssistantGeoEntityIntoNativeMapsBeforeDisplay(assistantMessage)");
    expect(mainSource).toContain("commitAssistantMessageWithProgressiveSeed(nextTranscript, assistantMessage");
    expect(mainSource).toContain("layoutByNodeIndex");
    expect(mainSource).toContain("backendNodeId");
    expect(mainSource.indexOf("if (nativeMapsView && !nativeMapsView.webContents.isDestroyed())")).toBeLessThan(
      mainSource.indexOf("if (mapsDomWebviewGuest && !mapsDomWebviewGuest.isDestroyed())")
    );
    expect(mainSource).toContain("app.getAppMetrics().find((metric) => metric.pid === processId)?.memory");
    expect(mainSource).toContain("DOM_RAM_ARTIFACT_CONTRACTS");
    expect(mainSource).toContain('"live_cdp_domsnapshot_csr_graph_incremental_node_edge_u64_records"');
    expect(mainSource).toContain('"live_columnar_ram_region_table_resumable_hash_offset_len_flags"');
    expect(mainSource).toContain('"live_nonblocking_browser_event_loop_slice_backpressure_manifest"');
    expect(mainSource).toContain('ipcMain.handle("forge:maps-dom-ram-cartography-capture"');
  });
});
