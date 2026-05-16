// Generated from the legacy trading bootstrap during the JS cutover.
// Source of truth now lives in TypeScript; trading.js only consumes this global.

export const DEFAULT_INSTRUMENT = "NATGAS_USD";
export const DEFAULT_GRANULARITY = "H4";
export const DEFAULT_CHART_DISPLAY_MODE = "candles";
export const MAX_ADDED_CHARTS = 3;
export const HISTORY_SERIES_MISS_CACHE_MAX = 512;
export const TIMEFRAME_OPTIONS = [
  { value: "W", label: "W1" },
  { value: "D", label: "D1" },
  { value: "H4", label: "H4" },
  { value: "H1", label: "H1" },
  { value: "M30", label: "M30" },
  { value: "M15", label: "M15" },
  { value: "M5", label: "M5" },
  { value: "M1", label: "M1" },
  { value: "S30", label: "S30" },
  { value: "S10", label: "S10" },
];
export const TRADING_INDICATOR_LIBRARY = [
  {
    id: "vwap",
    label: "VWAP",
    command: "/vwap",
    summary: "Session-anchored volume weighted average price.",
    favorites: true,
    settings: {
      anchor: { type: "select", label: "Anchor", options: ["session", "week", "month", "quarter", "year"], value: "session" },
      source: { type: "select", label: "Source", options: ["hlc3", "close", "ohlc4", "hl2", "open", "high", "low"], value: "hlc3" },
      offset: { type: "number", label: "Offset", value: 0, step: 1 },
      hideOnHigherTf: { type: "checkbox", label: "Hide on 1D or above", value: false },
    },
  },
  {
    id: "ema",
    label: "EMA",
    command: "/ema",
    summary: "Exponential moving average for trend following.",
    favorites: true,
    settings: {
      length: { type: "number", label: "Length", value: 20, min: 1, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2", "open", "high", "low"], value: "close" },
      offset: { type: "number", label: "Offset", value: 0, step: 1 },
    },
  },
  {
    id: "sma",
    label: "SMA",
    command: "/sma",
    summary: "Simple moving average for structure and bias.",
    favorites: true,
    settings: {
      length: { type: "number", label: "Length", value: 50, min: 1, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2", "open", "high", "low"], value: "close" },
      offset: { type: "number", label: "Offset", value: 0, step: 1 },
    },
  },
  {
    id: "wma",
    label: "WMA",
    command: "/wma",
    summary: "Weighted moving average with stronger recency bias.",
    settings: {
      length: { type: "number", label: "Length", value: 21, min: 1, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2", "open", "high", "low"], value: "close" },
    },
  },
  {
    id: "hma",
    label: "HMA",
    command: "/hma",
    summary: "Hull moving average for fast trend smoothing.",
    settings: {
      length: { type: "number", label: "Length", value: 55, min: 2, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2", "open", "high", "low"], value: "close" },
    },
  },
  {
    id: "vwma",
    label: "VWMA",
    command: "/vwma",
    summary: "Volume-weighted moving average.",
    settings: {
      length: { type: "number", label: "Length", value: 20, min: 1, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2"], value: "close" },
    },
  },
  {
    id: "bollinger",
    label: "Bollinger Bands",
    command: "/bollinger",
    summary: "Volatility envelope built from a basis and deviation bands.",
    favorites: true,
    settings: {
      length: { type: "number", label: "Length", value: 20, min: 1, step: 1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2"], value: "close" },
      deviation: { type: "number", label: "Deviation", value: 2, min: 0.1, step: 0.1 },
    },
  },
  {
    id: "donchian",
    label: "Donchian Channel",
    command: "/donchian",
    summary: "Highest-high / lowest-low breakout channel.",
    settings: {
      length: { type: "number", label: "Length", value: 20, min: 1, step: 1 },
    },
  },
  {
    id: "keltner",
    label: "Keltner Channel",
    command: "/keltner",
    summary: "ATR-based envelope around an EMA basis.",
    settings: {
      length: { type: "number", label: "Length", value: 20, min: 1, step: 1 },
      multiplier: { type: "number", label: "ATR multiplier", value: 1.5, min: 0.1, step: 0.1 },
      source: { type: "select", label: "Source", options: ["close", "hlc3", "ohlc4", "hl2"], value: "hlc3" },
    },
  },
  {
    id: "supertrend",
    label: "Supertrend",
    command: "/supertrend",
    summary: "ATR trend filter with dynamic support / resistance.",
    settings: {
      atrLength: { type: "number", label: "ATR length", value: 10, min: 1, step: 1 },
      multiplier: { type: "number", label: "Multiplier", value: 3, min: 0.1, step: 0.1 },
    },
  },
  {
    id: "ichimoku",
    label: "Ichimoku Cloud",
    command: "/ichimoku",
    summary: "Multi-line trend structure and cloud projection.",
    settings: {
      conversion: { type: "number", label: "Conversion", value: 9, min: 1, step: 1 },
      base: { type: "number", label: "Base", value: 26, min: 1, step: 1 },
      spanB: { type: "number", label: "Span B", value: 52, min: 2, step: 1 },
      displacement: { type: "number", label: "Displacement", value: 26, min: 0, step: 1 },
    },
  },
  {
    id: "psar",
    label: "Parabolic SAR",
    command: "/psar",
    summary: "Acceleration-based stop and reverse guide.",
    settings: {
      step: { type: "number", label: "Step", value: 0.02, min: 0.001, step: 0.01 },
      max: { type: "number", label: "Maximum", value: 0.2, min: 0.01, step: 0.01 },
    },
  },
];
export const TRADING_INDICATOR_STORAGE_KEY = "forge.trading.indicators.v1";
export const TRADING_CREATE_LIBRARY = [
  {
    id: "create",
    label: "create_",
    command: "/create_",
    summary: "Open the trading creation lane for program tokens.",
    kind: "program",
    family: "create",
  },
  {
    id: "indicator_create",
    label: "indicator",
    command: "/indicator",
    summary: "Create a new indicator metric or overlay.",
    kind: "program",
    family: "create",
    source: "trading",
  },
  {
    id: "alert_create",
    label: "alert_",
    command: "/alert_",
    summary: "Create a new alert definition.",
    kind: "program",
    family: "create",
    source: "trading",
  },
  {
    id: "order_create",
    label: "order",
    command: "/order",
    summary: "Create a new order blueprint.",
    kind: "program",
    family: "create",
    source: "trading",
  },
  {
    id: "program_create",
    label: "program_",
    command: "/program_",
    summary: "Create a reusable program entity.",
    kind: "program",
    family: "program",
    source: "atlas",
  },
  {
    id: "strategy",
    label: "strategy_",
    command: "/strategy_",
    summary: "Create a trading strategy program from metrics and chart context.",
    kind: "program",
    family: "strategy",
    source: "trading",
  },
  {
    id: "strategy_backtest",
    label: "backtest_",
    command: "/backtest_",
    summary: "Run the current or described strategy through the native Strategy Lab backtest.",
    kind: "program",
    family: "strategy",
    source: "trading",
  },
  {
    id: "geo_create",
    label: "geo",
    command: "/geo",
    summary: "Create a geonode metric anchor.",
    kind: "metric",
    family: "geo",
    source: "atlas",
  },
  {
    id: "minigeo_create",
    label: "minigeo",
    command: "/minigeo",
    summary: "Create a mini-geonode metric anchor.",
    kind: "metric",
    family: "minigeo",
    source: "atlas",
  },
  {
    id: "lens_create",
    label: "lens_",
    command: "/lens_",
    summary: "Create a new Atlas lens.",
    kind: "program",
    family: "create",
    source: "atlas",
  },
  {
    id: "geonode_view_create",
    label: "geonode_view_",
    command: "/geonode_view_",
    summary: "Create a geonode view program.",
    kind: "program",
    family: "create",
    source: "atlas",
  },
  {
    id: "dataset_create",
    label: "dataset_",
    command: "/dataset_",
    summary: "Create a dataset entity.",
    kind: "program",
    family: "create",
    source: "atlas",
  },
  {
    id: "map_create",
    label: "map_",
    command: "/map_",
    summary: "Create a map entity.",
    kind: "program",
    family: "create",
    source: "atlas",
  },
];
export const FULL_HISTORY_GRANULARITIES = TIMEFRAME_OPTIONS.map((item) => item.value);
export const CHART_DISPLAY_OPTIONS = [
  {
    value: "candles",
    title: "Bougies",
    subtitle: "Vue OHLC la plus lisible",
  },
  {
    value: "ohlc",
    title: "Barres",
    subtitle: "Version minimale OHLC",
  },
  {
    value: "line",
    title: "Ligne",
    subtitle: "Cloture uniquement",
  },
  {
    value: "area",
    title: "Zone",
    subtitle: "Tendance avec remplissage",
  },
];
export const CHART_TAIL_ROWS_BY_GRANULARITY = {
  S10: 2500,
  S30: 3500,
  M1: 5000,
  M5: 6000,
  M15: 7000,
  M30: 8000,
  H1: 10000,
  H4: 12000,
  D: 8000,
  W: 3000,
};
export const POLL_MS = 1_000;
export const TRADING_LLM_INVOLVEMENT_KEY = "forge.trading.llm.involvement.v1";
export const LLM_RUNTIMES = ["codex", "gemini", "claude"];
export const PREVIEW_ASSETS = [
  { name: "NATGAS_USD", displayName: "Natural Gas", assetClass: "commodity" },
  { name: "WTICO_USD", displayName: "WTI Crude Oil", assetClass: "commodity" },
  { name: "XAU_USD", displayName: "Gold", assetClass: "commodity" },
  { name: "EUR_USD", displayName: "EUR / USD", assetClass: "forex" },
  { name: "GBP_USD", displayName: "GBP / USD", assetClass: "forex" },
  { name: "USD_JPY", displayName: "USD / JPY", assetClass: "forex" },
  { name: "BTC_USD", displayName: "BTC / USD", assetClass: "crypto" },
  { name: "SPX500_USD", displayName: "US 500", assetClass: "index" },
  { name: "NAS100_USD", displayName: "US Tech 100", assetClass: "index" },
  { name: "DE40_EUR", displayName: "Germany 40", assetClass: "index" },
  { name: "AAPL_USD", displayName: "Apple", assetClass: "equity" },
  { name: "TSLA_USD", displayName: "Tesla", assetClass: "equity" },
  { name: "NVDA_USD", displayName: "NVIDIA", assetClass: "equity" },
  { name: "DE10YB_EUR", displayName: "German 10Y Bund", assetClass: "bond" },
  { name: "USB10Y_USD", displayName: "US 10Y Treasury", assetClass: "bond" },
];
export const ASSET_TOKEN_LABELS = {
  AUD: "Australian Dollar",
  AAPL: "Apple",
  AMZN: "Amazon",
  BTC: "Bitcoin",
  BRENT: "Brent Oil",
  CAD: "Canadian Dollar",
  CHF: "Swiss Franc",
  DE10YB: "German 10Y Bund",
  DE40: "Germany 40",
  ETH: "Ethereum",
  EUR: "Euro",
  EU50: "Europe 50",
  FRA40: "France 40",
  GBP: "British Pound",
  GOOG: "Alphabet",
  HKD: "Hong Kong Dollar",
  JP225: "Japan 225",
  JPY: "Japanese Yen",
  META: "Meta",
  MSFT: "Microsoft",
  NATGAS: "Natural Gas",
  NAS100: "US Tech 100",
  NFLX: "Netflix",
  NVDA: "NVIDIA",
  NZD: "New Zealand Dollar",
  SGD: "Singapore Dollar",
  SILVER: "Silver",
  SPX500: "US 500",
  TSLA: "Tesla",
  UK100: "UK 100",
  UK10YB: "UK 10Y Gilt",
  USD: "US Dollar",
  USB10Y: "US 10Y Treasury",
  WTI: "Crude Oil",
  XAG: "Silver",
  XAU: "Gold",
};
export const ASSET_CLASS_LABELS = {
  bond: "Bonds",
  commodity: "Commodities",
  crypto: "Crypto",
  equity: "Stocks",
  forex: "Forex",
  index: "Indices",
  instrument: "Library",
  other: "Other",
  rate: "Rates",
};
export const PINNED_ASSETS = ["NATGAS_USD", "EUR_USD", "BTC_USD"];

export function loadLlmInvolvement() {
  const defaults = { codex: true, gemini: true, claude: true };
  try {
    const raw = localStorage.getItem(TRADING_LLM_INVOLVEMENT_KEY);
    if (!raw) return defaults;
    const parsed = JSON.parse(raw);
    return {
      codex: parsed?.codex !== false,
      gemini: parsed?.gemini !== false,
      claude: parsed?.claude !== false,
    };
  } catch (_) {
    return defaults;
  }
}
export const TRADING_CHART_MODE_TRIGGER_SVG = `
  <svg viewBox="0 0 24 24" focusable="false" aria-hidden="true">
    <path d="M6 6.5v11" />
    <path d="M9 9.5v5" />
    <path d="M15 7.5v9" />
    <path d="M18 10.5v3" />
    <rect x="4.8" y="10" width="2.4" height="3" rx="0.7" />
    <rect x="7.8" y="7.5" width="2.4" height="9" rx="0.7" />
    <rect x="13.8" y="9" width="2.4" height="6" rx="0.7" />
    <rect x="16.8" y="8" width="2.4" height="8" rx="0.7" />
  </svg>
`;
export const TRADING_RIGHT_PANEL_TRIGGER_SVG = `
  <svg viewBox="0 0 24 24" focusable="false" aria-hidden="true">
    <rect x="4.5" y="5.5" width="15" height="13" rx="1.5" />
    <path d="M9 5.5v13" />
    <path d="M12.75 9.25h3.25" />
    <path d="M12.75 12h3.25" />
    <path d="M12.75 14.75h2.5" />
  </svg>
`;

export const ForgeTradingCatalog = Object.freeze({
  DEFAULT_INSTRUMENT,
  DEFAULT_GRANULARITY,
  DEFAULT_CHART_DISPLAY_MODE,
  MAX_ADDED_CHARTS,
  HISTORY_SERIES_MISS_CACHE_MAX,
  TIMEFRAME_OPTIONS,
  TRADING_INDICATOR_LIBRARY,
  TRADING_INDICATOR_STORAGE_KEY,
  TRADING_CREATE_LIBRARY,
  FULL_HISTORY_GRANULARITIES,
  CHART_DISPLAY_OPTIONS,
  CHART_TAIL_ROWS_BY_GRANULARITY,
  POLL_MS,
  TRADING_LLM_INVOLVEMENT_KEY,
  LLM_RUNTIMES,
  PREVIEW_ASSETS,
  ASSET_TOKEN_LABELS,
  ASSET_CLASS_LABELS,
  PINNED_ASSETS,
  TRADING_CHART_MODE_TRIGGER_SVG,
  TRADING_RIGHT_PANEL_TRIGGER_SVG,
  loadLlmInvolvement,
});

declare global {
  interface Window {
    ForgeTradingCatalog?: typeof ForgeTradingCatalog;
  }
}

window.ForgeTradingCatalog = ForgeTradingCatalog;
