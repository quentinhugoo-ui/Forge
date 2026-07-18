import { describe, expect, it } from "vitest";
import {
  createMapsCodeActRequest,
  extractMapsCodeAct,
  GOOGLE_EARTH_DEFAULT_URL,
  MAPS_COMMAND,
  mapsTemplateProofHash,
  parseMapsCodeAct,
  readMapsCodeAct,
  renderMapsCodeActResult
} from "../src/main/maps-codeact";

describe("Maps CodeAct", () => {
  const proofHash = mapsTemplateProofHash();

  it("returns the template for a bare /maps_ command", () => {
    const codeAct = readMapsCodeAct("/maps_");

    expect(codeAct?.kind).toBe("template");
    expect(codeAct?.kind === "template" ? codeAct.result : undefined).toMatchObject({
      schema: "forge.webexplorer.maps.template_result.v1",
      command: MAPS_COMMAND,
      status: "template",
      reason: "empty_command"
    });
    expect(codeAct?.kind === "template" ? codeAct.result.template : "").toContain('tileset="google_photorealistic_3d_tiles"');
    expect(codeAct?.kind === "template" ? codeAct.result.template : "").not.toContain("camera");
  });

  it("builds a Google Earth URL from explicit WGS84 coordinates", () => {
    const request = parseMapsCodeAct(`/maps_ template_proof_hash="sha256:${proofHash}" search_kind="gps" target="Paris" latitude="48.8566" longitude="2.3522"`);

    expect(request?.target).toBe("Paris");
    expect(request?.searchKind).toBe("gps");
    expect(request?.latitude).toBe(48.8566);
    expect(request?.longitude).toBe(2.3522);
    expect(request?.url).toContain("https://earth.google.com/web/@48.85660000,2.35220000,0a");
  });

  it("accepts assistant Maps command casing", () => {
    const request = parseMapsCodeAct(`/Maps_ template_proof_hash="sha256:${proofHash}" query="Yakushima" target="Yakushima"`);

    expect(request?.command).toBe(MAPS_COMMAND);
    expect(request?.target).toBe("Yakushima");
  });

  it("can build an explicit coordinate request without implying saved home location", () => {
    const request = createMapsCodeActRequest({
      command: MAPS_COMMAND,
      templateProofHash: proofHash,
      target: "Yakushima, Japan",
      query: "Yakushima, Japan",
      placeId: "ChIJexample",
      keywords: ["explicit_maps_target", "google_geocoding"],
      latitude: 30.3586,
      longitude: 130.5286,
      source: "explicit_codeact"
    });

    expect(request.target).toBe("Yakushima, Japan");
    expect(request.placeId).toBe("ChIJexample");
    expect(request.keywords).not.toContain("brain_home_location");
    expect(request.url).toContain("https://earth.google.com/web/@30.35860000,130.52860000,0a");
    expect(request.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts /maps_ only when explicitly emitted by the assistant", () => {
    expect(extractMapsCodeAct("Ouvre une carte de la Terre")).toBeUndefined();

    const request = extractMapsCodeAct([
      "J'ouvre Google Earth.",
      `/maps_ template_proof_hash="sha256:${proofHash}" query="default_google_earth_view" target="default_google_earth_view"`
    ].join("\n"));

    expect(request?.command).toBe(MAPS_COMMAND);
    expect(request?.url).toBe(GOOGLE_EARTH_DEFAULT_URL);
  });

  it("renders machine metadata without inventing device-location coordinates", () => {
    const request = parseMapsCodeAct(`/maps_ template_proof_hash="sha256:${proofHash}" search_kind="administration" query="rectorat de lille" target="Rectorat de Lille" place_id="ChIJrectorat" latitude="50.6292" longitude="3.0573"`);
    expect(request).toBeDefined();

    const rendered = renderMapsCodeActResult(request!);
    expect(rendered).toContain("MAPS_RESULT");
    expect(rendered).toContain("forge.webexplorer.maps.result.v1");
    expect(rendered).toContain("engine=cesiumjs");
    expect(rendered).toContain("search_kind=administration");
    expect(rendered).toContain('place_id="ChIJrectorat"');
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
