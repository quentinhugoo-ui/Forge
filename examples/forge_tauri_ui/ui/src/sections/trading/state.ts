export const tradingSubbarModes = ["none", "indicators", "replay", "alert"] as const;
export const tradingSubbarSections = ["favorites", "indicators", "strategies", "profile", "patterns", "create"] as const;

export type TradingSubbarMode = (typeof tradingSubbarModes)[number];
export type TradingSubbarSection = (typeof tradingSubbarSections)[number];

export interface TradingActivatePatch {
  readonly alertFormMode: "create";
  readonly snapshot: null;
  readonly market: null;
  readonly catalog: readonly [];
  readonly assetCatalog: readonly [];
  readonly localCatalogPromise: null;
}

export interface TradingDeactivatePatch {
  readonly chatSubbarMode: "none";
  readonly chatSubbarExpanded: false;
  readonly chatSubbarSection: "indicators";
  readonly indicatorsModeActive: false;
  readonly replayModeActive: false;
}

export function normalizeTradingChatSubbarMode(mode: unknown): TradingSubbarMode {
  const normalized = String(mode || "none").trim().toLowerCase();
  return tradingSubbarModes.includes(normalized as TradingSubbarMode)
    ? (normalized as TradingSubbarMode)
    : "none";
}

export function normalizeTradingSubbarSection(section: unknown): TradingSubbarSection {
  const normalized = String(section || "indicators").trim().toLowerCase();
  return tradingSubbarSections.includes(normalized as TradingSubbarSection)
    ? (normalized as TradingSubbarSection)
    : "indicators";
}

export function tradingActivatePatch(): TradingActivatePatch {
  return {
    alertFormMode: "create",
    snapshot: null,
    market: null,
    catalog: [],
    assetCatalog: [],
    localCatalogPromise: null,
  };
}

export function tradingDeactivatePatch(): TradingDeactivatePatch {
  return {
    chatSubbarMode: "none",
    chatSubbarExpanded: false,
    chatSubbarSection: "indicators",
    indicatorsModeActive: false,
    replayModeActive: false,
  };
}
