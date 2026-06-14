import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const routerSource = readFileSync("src/renderer/HeaderSurfaceRouter.tsx", "utf8");
const stylesSource = readFileSync("src/renderer/styles.css", "utf8");

describe("Banger Cesium viewport contract", () => {
  it("keeps the Banger page as a full-screen render surface with no inner frame prose", () => {
    expect(routerSource).toContain('"--surface-left": "0px"');
    expect(routerSource).toContain('"--surface-top": "0px"');
    expect(routerSource).toContain('"--surface-width": "100vw"');
    expect(routerSource).toContain('"--surface-height": "100vh"');
    expect(routerSource).toContain('className={cesiumMounted ? "bangerCesiumViewport bangerCesiumViewport--mounted" : "bangerCesiumViewport"}');
    expect(routerSource).not.toContain("Banger native viewport");
    expect(routerSource).not.toContain("native raster frame loading");
  });

  it("keeps Cesium fail-visible instead of returning a black silent canvas", () => {
    expect(routerSource).toContain("requestRenderMode: false");
    expect(routerSource).toContain("skyAtmosphere: false");
    expect(routerSource).toContain("skyBox: false");
    expect(routerSource).toContain("Cesium.Rectangle.fromDegrees(-180, -75, 180, 75)");
    expect(routerSource).toContain("scene.globe.baseColor");
    expect(routerSource).toContain("scene.globe.show = true");
    expect(routerSource).toContain('console.error("Banger Cesium viewport failed to mount.", error)');
    expect(stylesSource).toContain(".bangerCesiumViewport::before");
    expect(stylesSource).toContain(".bangerCesiumViewport--mounted::before");
    expect(stylesSource).toContain("radial-gradient(circle at 49% 45%");
  });
});
