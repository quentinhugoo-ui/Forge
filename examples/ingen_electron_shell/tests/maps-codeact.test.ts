import { describe, expect, it } from "vitest";
import {
  createMapsCodeActRequest,
  extractMapsCodeAct,
  GOOGLE_EARTH_DEFAULT_URL,
  MAPS_COMMAND,
  parseMapsCodeAct,
  renderMapsCodeActResult
} from "../src/main/maps-codeact";

describe("Maps CodeAct", () => {
  it("opens the requested Google Earth view for a bare /maps_ command", () => {
    const request = parseMapsCodeAct("/maps_");

    expect(request).toMatchObject({
      schema: "forge.webexplorer.maps.request.v1",
      command: MAPS_COMMAND,
      target: "default_google_earth_view",
      url: GOOGLE_EARTH_DEFAULT_URL,
      source: "explicit_codeact"
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("builds a Google Earth URL from explicit WGS84 coordinates", () => {
    const request = parseMapsCodeAct('/maps_ target="Paris" latitude="48.8566" longitude="2.3522"');

    expect(request?.target).toBe("Paris");
    expect(request?.latitude).toBe(48.8566);
    expect(request?.longitude).toBe(2.3522);
    expect(request?.url).toContain("https://earth.google.com/web/@48.85660000,2.35220000,0a");
  });

  it("accepts assistant Maps command casing", () => {
    const request = parseMapsCodeAct('/Maps_ target="Yakushima"');

    expect(request?.command).toBe(MAPS_COMMAND);
    expect(request?.target).toBe("Yakushima");
  });

  it("can build an explicit coordinate request without implying saved home location", () => {
    const request = createMapsCodeActRequest({
      command: MAPS_COMMAND,
      target: "Yakushima, Japan",
      query: "Yakushima, Japan",
      keywords: ["explicit_maps_target", "google_geocoding"],
      latitude: 30.3586,
      longitude: 130.5286,
      source: "explicit_codeact"
    });

    expect(request.target).toBe("Yakushima, Japan");
    expect(request.keywords).not.toContain("brain_home_location");
    expect(request.url).toContain("https://earth.google.com/web/@30.35860000,130.52860000,0a");
    expect(request.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts /maps_ only when explicitly emitted by the assistant", () => {
    expect(extractMapsCodeAct("Ouvre une carte de la Terre")).toBeUndefined();

    const request = extractMapsCodeAct([
      "J'ouvre Google Earth.",
      '/maps_ target="default_google_earth_view"'
    ].join("\n"));

    expect(request?.command).toBe(MAPS_COMMAND);
    expect(request?.url).toBe(GOOGLE_EARTH_DEFAULT_URL);
  });

  it("renders machine metadata without inventing device-location coordinates", () => {
    const request = parseMapsCodeAct("/maps_");
    expect(request).toBeDefined();

    const rendered = renderMapsCodeActResult(request!);
    expect(rendered).toContain("MAPS_RESULT");
    expect(rendered).toContain("forge.webexplorer.maps.result.v1");
    expect(rendered).toContain("latitude=");
    expect(rendered).toContain("longitude=");
    expect(rendered).toContain('route="banger://maps-sphere"');
    expect(rendered).toContain("visual_target=banger_native_maps_sphere");
    expect(rendered).toContain("tileset_provider=google_photorealistic_3d_tiles");
    expect(rendered).toContain("renderer_contract=forge.banger.google_photorealistic_tiles_config.v1");
    expect(rendered).not.toContain("earth.google.com");
    expect(rendered).not.toContain("current_location");
  });
});
