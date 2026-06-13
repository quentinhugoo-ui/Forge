import { describe, expect, it } from "vitest";
import {
  AIRBNB_COMMAND,
  AIRBNB_HOME_URL,
  extractAirbnbCodeAct,
  parseAirbnbCodeAct,
  renderAirbnbCodeActResult
} from "../src/main/airbnb-codeact";

describe("Airbnb CodeAct", () => {
  it("parses an explicit /airbnb_ open command", () => {
    const request = parseAirbnbCodeAct('/airbnb_ intent="open" query="" keywords=""');

    expect(request).toBeDefined();
    expect(request?.schema).toBe("forge.webexplorer.airbnb.request.v1");
    expect(request?.command).toBe(AIRBNB_COMMAND);
    expect(request?.intent).toBe("open");
    expect(request?.url).toBe(AIRBNB_HOME_URL);
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("preserves Airbnb search context in the navigation URL", () => {
    const request = parseAirbnbCodeAct('/airbnb_ intent="search" query="Paris" keywords="2 voyageurs, appartement"');

    expect(request).toBeDefined();
    expect(request?.intent).toBe("search");
    expect(request?.keywords).toEqual(["2 voyageurs", "appartement"]);
    expect(request?.url).toContain("https://www.airbnb.com/");
    expect(request?.url).toContain("query=Paris+2+voyageurs+appartement");
  });

  it("extracts /airbnb_ only when explicitly emitted by the assistant", () => {
    expect(extractAirbnbCodeAct("Ouvre Airbnb s'il te plait")).toBeUndefined();

    const request = extractAirbnbCodeAct([
      "Oui, je vais ouvrir Airbnb.",
      '/airbnb_ intent="open" query="" keywords=""'
    ].join("\n"));

    expect(request?.command).toBe(AIRBNB_COMMAND);
  });

  it("renders a machine result without app-authored user-facing prose", () => {
    const request = parseAirbnbCodeAct("/airbnb_");
    expect(request).toBeDefined();

    const rendered = renderAirbnbCodeActResult(request!);
    expect(rendered).toContain("AIRBNB_RESULT");
    expect(rendered).toContain("forge.webexplorer.airbnb.result.v1");
    expect(rendered).not.toContain("J'ouvre Airbnb");
    expect(rendered).not.toContain("pour toi");
  });
});
