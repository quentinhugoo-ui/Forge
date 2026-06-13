export interface AssistantGeoEntity {
  token: string;
  label: string;
  kind: "city_or_country" | "geographic_entity";
}

const ASSISTANT_GEO_ENTITY_PATTERN = /([@#])\{([^{}\n]{1,120})\}/g;

export function assistantGeoEntityLabel(token: string): string {
  return token.slice(2, -1).replace(/\s+/g, " ").trim();
}

export function extractAssistantGeoEntities(text: string): AssistantGeoEntity[] {
  const entities: AssistantGeoEntity[] = [];
  ASSISTANT_GEO_ENTITY_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = ASSISTANT_GEO_ENTITY_PATTERN.exec(text)) !== null) {
    const marker = match[1];
    const label = match[2]?.replace(/\s+/g, " ").trim() ?? "";
    if (label.length < 2) {
      continue;
    }
    entities.push({
      token: match[0],
      label,
      kind: marker === "#" ? "city_or_country" : "geographic_entity"
    });
  }
  return entities;
}

export function primaryAssistantGeoEntityLabel(text: string): string {
  return extractAssistantGeoEntities(text)[0]?.label ?? "";
}
