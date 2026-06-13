import { describe, expect, it } from "vitest";
import {
  assistantGeoEntityLabel,
  extractAssistantGeoEntities,
  primaryAssistantGeoEntityLabel
} from "../src/renderer/assistant-geo-entities";

describe("assistant geo entities", () => {
  it("extracts marked cities, countries, and geographic entities in order", () => {
    expect(extractAssistantGeoEntities("Context for #{San Francisco} near @{Silicon Valley}.")).toEqual([
      {
        token: "#{San Francisco}",
        label: "San Francisco",
        kind: "city_or_country"
      },
      {
        token: "@{Silicon Valley}",
        label: "Silicon Valley",
        kind: "geographic_entity"
      }
    ]);
  });

  it("normalizes labels and picks the first geographic target", () => {
    expect(assistantGeoEntityLabel("#{  Paris   France  }")).toBe("Paris France");
    expect(primaryAssistantGeoEntityLabel("Use @{  Golden   Gate Park } then #{San Francisco}.")).toBe("Golden Gate Park");
  });
});
