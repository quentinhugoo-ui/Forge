import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const routerSource = readFileSync("src/renderer/HeaderSurfaceRouter.tsx", "utf8");
const stylesSource = readFileSync("src/renderer/styles.css", "utf8");
const rustBackendSource = readFileSync("src/main/rust-backend.ts", "utf8");

describe("Banger native viewport contract", () => {
  it("keeps the Banger page as a full-screen native render surface with no inner frame prose", () => {
    expect(routerSource).toContain('"--surface-left": "0px"');
    expect(routerSource).toContain('"--surface-top": "0px"');
    expect(routerSource).toContain('"--surface-width": "100vw"');
    expect(routerSource).toContain('"--surface-height": "100vh"');
    expect(routerSource).toContain("getBangerPreviewFrame");
    expect(routerSource).toContain("getBangerPresentLoopBootstrap");
    expect(routerSource).toContain('"nativeViewportSlot nativeViewportSlot--live"');
    expect(routerSource).toContain('data-native-contract={surface.nativeContract}');
    expect(routerSource).toContain('data-present-loop={presentLoop?.routeStatus ?? "pending"}');
    expect(routerSource).not.toContain("native raster frame loading");
  });

  it("does not mount a browser WebGL globe inside the Banger surface", () => {
    expect(routerSource).not.toContain('from "cesium"');
    expect(routerSource).not.toContain('import("cesium")');
    expect(routerSource).not.toContain("new Cesium.Viewer");
    expect(routerSource).not.toContain("getBangerGoogleTilesConfig");
    expect(routerSource).not.toContain("bangerCesiumViewport");
    expect(stylesSource).not.toContain("bangerCesiumViewport");
    expect(stylesSource).not.toContain("cesium-widget");
    expect(stylesSource).toContain(".surface--banger .nativeViewportSlot");
    expect(stylesSource).toContain("object-fit: cover");
  });

  it("launches the Rust native child-surface host instead of only probing offscreen", () => {
    expect(rustBackendSource).toContain("launchRustBangerNativeHost");
    expect(rustBackendSource).toContain("--banger-native-host");
    expect(rustBackendSource).toContain("FORGE_BANGER_PARENT_HWND");
    expect(rustBackendSource).toContain("shaderSourceHash");
    expect(rustBackendSource).toContain("renderPipelineHash");
    expect(rustBackendSource).toContain("renderLoopPolicy");
  });
});
