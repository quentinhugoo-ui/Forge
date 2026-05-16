import type { RealEstateToolDefinition, RealEstateToolGroup } from "./tools.js";

export interface RealEstateToolRendererDeps {
  readonly tools: readonly RealEstateToolDefinition[];
  readonly groups: readonly RealEstateToolGroup[];
}

export function commandForRealEstateToolId(id: string): `/${string}_` {
  return `/${id.replace(/-/g, "_")}_`;
}

export function realEstateToolByCommand(
  tools: readonly RealEstateToolDefinition[],
  command: string,
): RealEstateToolDefinition | null {
  return tools.find((tool) => tool.command === command) ?? null;
}
