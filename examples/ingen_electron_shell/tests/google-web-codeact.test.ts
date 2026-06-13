import { describe, expect, it } from "vitest";
import {
  extractGoogleWebCodeAct,
  GOOGLEWEB_COMMAND,
  GOOGLEWEB_HOME_URL,
  parseGoogleWebCodeAct
} from "../src/main/google-web-codeact";

describe("Google WebExplorer CodeAct", () => {
  it("parses explicit /googleweb_ template slots with special keywords", () => {
    const request = parseGoogleWebCodeAct('/googleweb_ query="ville de Kagoshima" keywords="histoire, tourisme, population, Japon"');

    expect(request).toMatchObject({
      command: GOOGLEWEB_COMMAND,
      query: "ville de Kagoshima",
      keywords: ["histoire", "tourisme", "population", "Japon"],
      source: "explicit_codeact"
    });
    expect(request?.url).toContain("https://www.google.com/search?");
    expect(request?.url).toContain("q=ville+de+Kagoshima+histoire+tourisme+population+Japon");
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts /googleweb_ only when the assistant emits an explicit command", () => {
    const naturalText = extractGoogleWebCodeAct("Fais-moi des recherches sur la ville de Kagoshima");
    const assistantText = extractGoogleWebCodeAct([
      "Je vais ouvrir Web Explorer.",
      '/googleweb_ query="ville de Kagoshima" keywords="histoire, tourisme, population, Japon"'
    ].join("\n"));

    expect(naturalText).toBeUndefined();
    expect(assistantText).toMatchObject({
      command: GOOGLEWEB_COMMAND,
      query: "ville de Kagoshima",
      source: "explicit_codeact"
    });
    expect(assistantText?.keywords).toEqual(["histoire", "tourisme", "population", "Japon"]);
    expect(assistantText?.url).toContain("Kagoshima");
  });

  it("opens the Google homepage for a bare /googleweb_ command", () => {
    const request = extractGoogleWebCodeAct([
      "Bien sur, j'ouvre une page web.",
      "/googleweb_"
    ].join("\n"));

    expect(request).toMatchObject({
      command: GOOGLEWEB_COMMAND,
      query: "Google",
      keywords: [],
      url: GOOGLEWEB_HOME_URL,
      source: "explicit_codeact"
    });
  });
});
