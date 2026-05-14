(function () {
  "use strict";

  const tradingRuntimeUrl = new URL(window.location.href);
  const tradingSurfaceMode = tradingRuntimeUrl.searchParams.get("surface") || "";
  if (tradingSurfaceMode === "webexplorer") {
    if (typeof window !== "undefined") {
      const inactiveBridge = {
        isActive: () => false,
        anyRuntimeInvolved: () => false,
        isRuntimeInvolved: () => false,
        routeLocalCommand: async () => ({ label: "trading disabled", message: "" }),
        buildContextEnvelope: () => "",
        buildContextDigest: () => "",
      };
      window.__forgeTradingChatBridge = inactiveBridge;
      window.__forgeCloseTrading = () => {};
      window.__forgeTradingActive = false;
    }
    return;
  }

  const $ = (id) => document.getElementById(id);

  const els = {
    button: $("tradingWorkspaceBtn"),
    content: document.querySelector("#alphaSection .content"),
    canvasWrap: document.querySelector("#alphaSection .canvas-wrap"),
    topbarBreadcrumb: $("topbarBreadcrumb"),
    chartModeTrigger: $("alphaAddFileBtn"),
    programTrigger: $("alphaAddProgramBtn"),
    compareTrigger: $("tradingCompareTrigger"),
    compareTriggerLabel: $("tradingCompareTriggerLabel"),
    addTrigger: $("tradingAddTrigger"),
    addTriggerLabel: $("tradingAddTriggerLabel"),
    indicatorDock: $("tradingIndicatorDock"),
    compareSearchWrap: $("tradingCompareSearch"),
    compareSearchInput: $("tradingCompareSearchInput"),
    compareMenu: $("tradingCompareMenu"),
    timeframeRail: $("tradingTimeframeRail"),
    chatRoot: $("forgeCanvasChat"),
    chatTradingActions: $("forgeCanvasTradingActions"),
    tradingSubbar: $("forgeCanvasTradingSubbar"),
    selectionTrigger: $("forgeCanvasTradingSelect"),
    indicatorsTrigger: $("forgeCanvasTradingIndicators"),
    replayTrigger: $("forgeCanvasTradingReplay"),
    alertTrigger: $("forgeCanvasTradingAlert"),
    llmModeToggle: $("forgeCanvasChatLlmModeToggle"),
    planetTrigger: $("marsLensToggle"),
    panelTabCompute: $("panelTabCompute"),
    statusText: $("alphaStatusText"),
    pinDrop: $("forgePinDrop"),
    pinDropText: $("forgePinDropText"),
    pinMenuBtn: $("forgePinMenuBtn"),
    pinnedList: $("forgePinnedJobList"),
    jobList: $("forgeJobList"),
    historyHeading: document.querySelector(".history-heading span"),
    proofToggle: $("alphaProofToggle"),
    proofClose: $("alphaProofClose"),
    jobMenu: $("forgeJobMenu"),
  };

  if (!els.button) return;
  window.ForgeSectionRegistry?.register?.({
    id: "trading",
    label: "Trading",
    kind: "tool-section",
    parent: "alpha",
    lazy: true,
  });

  const DEFAULT_INSTRUMENT = "NATGAS_USD";
  const DEFAULT_GRANULARITY = "H4";
  const DEFAULT_CHART_DISPLAY_MODE = "candles";
  const MAX_ADDED_CHARTS = 3;
  const HISTORY_SERIES_MISS_CACHE_MAX = 512;
  const TIMEFRAME_OPTIONS = [
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
  const TRADING_INDICATOR_LIBRARY = [
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
  const TRADING_INDICATOR_STORAGE_KEY = "forge.trading.indicators.v1";
  const TRADING_INDICATOR_SECTIONS = ["favorites", "indicators", "strategies", "profile", "patterns", "create"];
  const TRADING_CREATE_LIBRARY = [
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
  const FULL_HISTORY_GRANULARITIES = TIMEFRAME_OPTIONS.map((item) => item.value);
  const CHART_DISPLAY_OPTIONS = [
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
  const CHART_TAIL_ROWS_BY_GRANULARITY = {
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
  const POLL_MS = 1_000;
  const TRADING_LLM_INVOLVEMENT_KEY = "forge.trading.llm.involvement.v1";
  const LLM_RUNTIMES = ["codex", "gemini", "claude"];
  const PREVIEW_ASSETS = [
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
  const ASSET_TOKEN_LABELS = {
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
  const ASSET_CLASS_LABELS = {
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
  const PINNED_ASSETS = ["NATGAS_USD", "EUR_USD", "BTC_USD"];

  function loadLlmInvolvement() {
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
  const TRADING_CHART_MODE_TRIGGER_SVG = `
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
  const TRADING_RIGHT_PANEL_TRIGGER_SVG = `
    <svg viewBox="0 0 24 24" focusable="false" aria-hidden="true">
      <rect x="4.5" y="5.5" width="15" height="13" rx="1.5" />
      <path d="M9 5.5v13" />
      <path d="M12.75 9.25h3.25" />
      <path d="M12.75 12h3.25" />
      <path d="M12.75 14.75h2.5" />
    </svg>
  `;
  const chartModeTriggerDefault = els.chartModeTrigger ? {
    innerHTML: els.chartModeTrigger.innerHTML,
    ariaLabel: els.chartModeTrigger.getAttribute("aria-label") || "Add file",
    title: els.chartModeTrigger.getAttribute("title") || "Add file",
  } : null;
  const programTriggerDefault = els.programTrigger ? {
    innerHTML: els.programTrigger.innerHTML,
    ariaLabel: els.programTrigger.getAttribute("aria-label") || "Add program",
    title: els.programTrigger.getAttribute("title") || "Add program",
  } : null;
  const state = {
    active: false,
    snapshot: null,
    market: null,
    catalog: [],
    assetCatalog: [],
    selectedInstrument: DEFAULT_INSTRUMENT,
    selectedGranularity: DEFAULT_GRANULARITY,
    chartDisplayMode: DEFAULT_CHART_DISPLAY_MODE,
    candles: [],
    chartCache: new Map(),
    historySeriesPromises: new Map(),
    historySeriesMisses: new Set(),
    previewSeedActive: false,
    localCatalogPromise: null,
    selectedHistorySyncPromise: null,
    selectedHistorySyncKey: "",
    renderedSeriesKey: "",
    pollTimer: 0,
    refreshPromise: null,
    pendingRefreshOptions: null,
    uiSnapshot: null,
    consoleDrafts: {
      pine: [
        "indicator('NATGAS Bias', overlay=true)",
        "fast = ta.ema(close, 8)",
        "slow = ta.ema(close, 21)",
        "plot(fast, color=color.new(color.green, 0))",
        "plot(slow, color=color.new(color.orange, 0))",
      ].join("\n"),
      rust: [
        "fn main() {",
        "    println!(\"trading context ready\");",
        "    println!(\"next: regime, volatility, execution rules\");",
        "}",
      ].join("\n"),
      kasm: [
        "scene \"trading\"",
        "use market active",
        "query candles granularity H4",
        "tool order.preview side=\"BUY\" units=100",
      ].join("\n"),
    },
    credentials: {
      accountId: "",
      apiKey: "",
      baseUrl: "https://api-fxpractice.oanda.com",
    },
    orderForm: {
      instrument: DEFAULT_INSTRUMENT,
      side: "BUY",
      units: "100",
      orderType: "MARKET",
      limitPrice: "",
      stopLoss: "",
      takeProfit: "",
    },
    ordersOutput: "No order sent yet.",
    compareInstruments: [],
    addInstruments: [],
    compareSearch: "",
    compareScrollTop: 0,
    chatSubbarMode: "none",
    chatSubbarExpanded: false,
    chatSubbarSection: "indicators",
    activeIndicators: [],
    indicatorSettingsId: "",
    indicatorSettingsTab: "inputs",
    indicatorsModeActive: false,
    replayModeActive: false,
    llmInvolvement: loadLlmInvolvement(),
    headerMenuScrollTop: {},
    headerMenuMode: "none",
    headerMenuTriggerEl: null,
    headerMenuHideTimer: 0,
    universeHistorySyncDone: false,
    universeHistorySyncPromise: null,
    alerts: [],
    alertModalOpen: false,
    alertFormMode: "create",
    alertDraft: null,
    alertToastTimer: 0,
    strategyLab: {
      active: false,
      status: "idle",
      title: "",
      prompt: "",
      source: "",
      instrument: DEFAULT_INSTRUMENT,
      granularity: DEFAULT_GRANULARITY,
      rules: null,
      spec: null,
      missingMetrics: [],
      questions: [],
      runner: null,
      liveJob: null,
      backtest: null,
      live: null,
      createdAtMs: 0,
      updatedAtMs: 0,
      backtestKey: "",
      liveKey: "",
      logs: [],
    },
      uiCache: {
        leftPanelKey: "",
        timeframeKey: "",
        headerMenuKey: "",
        contextSnapshotKey: "",
        contextSnapshotValue: null,
        contextDigestKey: "",
        contextDigestValue: "",
        contextDigestV3Key: "",
        contextDigestV3Value: "",
        availableAssetsKey: "",
        availableAssetsValue: null,
        libraryAssetsKey: "",
        libraryAssetsValue: null,
        brokerInstrumentSetKey: "",
        brokerInstrumentSetValue: null,
        catalogIndexKey: "",
        catalogIndexValue: null,
        granularitiesKey: "",
        granularitiesValue: null,
        assetSearchKey: "",
        assetSearchValue: null,
        compareMenuModelKey: "",
        compareMenuModelValue: null,
        extraChartsKey: "",
        extraChartsValue: null,
        indicatorCatalogKey: "",
        indicatorCatalogValue: null,
        favoriteIndicatorsValue: null,
        createLibraryValue: null,
        strategyLibraryValue: null,
        activeIndicatorSetKey: "",
        activeIndicatorSetValue: null,
        tradingHeaderKey: "",
        indicatorDockKey: "",
        chartModeTriggerKey: "",
        programTriggerKey: "",
        alertTriggerKey: "",
        chatActionsKey: "",
        runtimeButtonsKey: "",
        tradingSubbarKey: "",
        tradingSubbarMarkup: "",
        canvasAlertsKey: "",
        canvasAlertsValue: null,
        instrumentAlertsKey: "",
        instrumentAlertsValue: null,
        alertModalKey: "",
        contextEnvelopeState: {
          codex: { hash: "", sessionJobId: "" },
          gemini: { hash: "", sessionJobId: "" },
          claude: { hash: "", sessionJobId: "" },
        },
      },
  };
  const candleSeriesMeta = new WeakMap();
  const chartSeriesRevisions = new Map();
  let chartSeriesRevision = 0;
  let assetUniverseRevision = 0;
  let catalogUniverseRevision = 0;
  let alertUniverseRevision = 0;
  let alertUniverseSignature = "0";
  let activeIndicatorRevision = 0;
  let strategyRuntimeRevision = 0;

  registerTradingIndicatorsWithAtlas();
  state.activeIndicators = loadPersistedTradingIndicators();

  const alertUi = {
    overlay: null,
    toastRack: null,
  };
  const seenAlertEventKeys = new Set();

  function normalizeChartDisplayMode(mode) {
    const normalized = String(mode || "").trim().toLowerCase();
    return CHART_DISPLAY_OPTIONS.some((item) => item.value === normalized)
      ? normalized
      : DEFAULT_CHART_DISPLAY_MODE;
  }

  function hasTauriInvoke() {
    return typeof window !== "undefined" && !!window.__TAURI__?.core?.invoke;
  }

  function atlasSlashRegistry() {
    return typeof window !== "undefined" ? window.__forgeAtlasSlashRegistry || null : null;
  }

  function registerTradingIndicatorsWithAtlas() {
    const registry = atlasSlashRegistry();
    if (!registry?.registerBatch) return [];
    return registry.registerBatch([
      ...TRADING_INDICATOR_LIBRARY.map((entry) => ({
        id: entry.id,
        token: entry.command,
        label: entry.label,
        summary: entry.summary,
        kind: "metric",
        family: "indicator",
        source: "trading",
        favorites: entry.favorites === true,
        settings: entry.settings || {},
      })),
      ...TRADING_CREATE_LIBRARY.map((entry) => ({
        id: entry.id,
        token: entry.command,
        label: entry.label,
        summary: entry.summary,
        kind: entry.kind,
        family: entry.family,
        source: entry.source || "trading",
      })),
    ]);
  }

  async function invoke(command, args = {}) {
    return window.__TAURI__.core.invoke(command, args);
  }

  function tradingIndicatorCatalog() {
    registerTradingIndicatorsWithAtlas();
    const registry = atlasSlashRegistry();
    const cacheKey = registry?.list ? "atlas-registry" : "static-library";
    if (
      state.uiCache.indicatorCatalogKey === cacheKey
      && Array.isArray(state.uiCache.indicatorCatalogValue)
    ) {
      return state.uiCache.indicatorCatalogValue;
    }
    if (registry?.list) {
      const atlasEntries = registry.list({ kind: "metric", family: "indicator", source: "trading" })
        .map((entry) => ({
          ...entry,
          id: String(entry.id || "").trim().toLowerCase(),
          command: String(entry.command || entry.token || "").trim().toLowerCase(),
          label: String(entry.label || entry.id || entry.token || "").trim(),
          summary: String(entry.summary || "").trim(),
          favorites: entry.favorites === true,
          settings: entry.settings || {},
        }))
        .filter((entry) => entry.id && entry.command);
      if (atlasEntries.length) {
        state.uiCache.indicatorCatalogKey = cacheKey;
        state.uiCache.indicatorCatalogValue = atlasEntries;
        state.uiCache.favoriteIndicatorsValue = null;
        return atlasEntries;
      }
    }
    state.uiCache.indicatorCatalogKey = cacheKey;
    state.uiCache.indicatorCatalogValue = TRADING_INDICATOR_LIBRARY;
    state.uiCache.favoriteIndicatorsValue = null;
    return state.uiCache.indicatorCatalogValue;
  }

  function tradingIndicatorDefinition(id = "") {
    const normalized = String(id || "").trim().toLowerCase();
    return tradingIndicatorCatalog().find((entry) => entry.id === normalized) || null;
  }

  function tradingIndicatorDefinitionByCommand(command = "") {
    const normalized = String(command || "").trim().toLowerCase();
    const atlasMatch = atlasSlashRegistry()?.resolve?.(normalized);
    if (atlasMatch?.kind === "metric" && atlasMatch?.family === "indicator") {
      const byId = tradingIndicatorDefinition(atlasMatch.id || atlasMatch.slug || "");
      if (byId) return byId;
    }
    return tradingIndicatorCatalog().find((entry) => entry.command === normalized) || null;
  }

  function cloneIndicatorSettings(definition) {
    const settings = {};
    for (const [key, meta] of Object.entries(definition?.settings || {})) {
      settings[key] = meta?.value;
    }
    return settings;
  }

  function normalizeIndicatorSettings(definition, raw = {}) {
    const settings = cloneIndicatorSettings(definition);
    for (const [key, meta] of Object.entries(definition?.settings || {})) {
      if (!(key in raw)) continue;
      const nextValue = raw[key];
      if (meta?.type === "checkbox") settings[key] = !!nextValue;
      else if (meta?.type === "number") settings[key] = Number.isFinite(Number(nextValue)) ? Number(nextValue) : meta.value;
      else settings[key] = String(nextValue ?? meta.value ?? "");
    }
    return settings;
  }

  function normalizeTradingIndicatorInstance(raw = {}) {
    const definition = tradingIndicatorDefinition(raw?.id);
    if (!definition) return null;
    return {
      id: definition.id,
      label: definition.label,
      command: definition.command,
      visible: raw?.visible !== false,
      settings: normalizeIndicatorSettings(definition, raw?.settings || {}),
    };
  }

  function loadPersistedTradingIndicators() {
    try {
      const raw = localStorage.getItem(TRADING_INDICATOR_STORAGE_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed.map(normalizeTradingIndicatorInstance).filter(Boolean) : [];
    } catch (_) {
      return [];
    }
  }

  function persistTradingIndicators() {
    try {
      localStorage.setItem(TRADING_INDICATOR_STORAGE_KEY, JSON.stringify(state.activeIndicators));
    } catch (_) {}
  }

  function activeIndicatorInstance(id = "") {
    const normalized = String(id || "").trim().toLowerCase();
    return state.activeIndicators.find((entry) => entry.id === normalized) || null;
  }

  function activeIndicatorIdSignature() {
    return state.activeIndicators.map((entry) => entry.id).join("|");
  }

  function activeIndicatorIdSet() {
    const key = `${activeIndicatorRevision}::${activeIndicatorIdSignature()}`;
    if (state.uiCache.activeIndicatorSetKey === key && state.uiCache.activeIndicatorSetValue instanceof Set) {
      return state.uiCache.activeIndicatorSetValue;
    }
    const set = new Set(state.activeIndicators.map((entry) => entry.id));
    state.uiCache.activeIndicatorSetKey = key;
    state.uiCache.activeIndicatorSetValue = set;
    return set;
  }

  function invalidateTradingSubbarCache() {
    state.uiCache.activeIndicatorSetKey = "";
    state.uiCache.activeIndicatorSetValue = null;
    state.uiCache.indicatorDockKey = "";
    state.uiCache.tradingSubbarKey = "";
    state.uiCache.tradingSubbarMarkup = "";
  }

  function syncTradingIndicatorsToCanvas() {
    const bridge = canvasBridge();
    bridge?.setTradingIndicators?.({
      indicators: state.activeIndicators.map((entry) => ({
        id: entry.id,
        label: entry.label,
        command: entry.command,
        visible: entry.visible !== false,
        settings: { ...(entry.settings || {}) },
      })),
    });
    bridge?.forceImmediateRender?.();
  }

  async function listAlerts() {
    if (!hasTauriInvoke()) {
      return {
        alerts: state.alerts.slice(),
        events: [],
      };
    }
    return invoke("trading_alerts_list", {
      request: {
        instrument: state.selectedInstrument,
      },
    });
  }

  async function reloadAlerts(options = {}) {
    const response = await listAlerts();
    setAlertRecords(response?.alerts || []);
    syncCanvasAlerts();
    if (state.alertModalOpen || options.render === true) renderAlertModal();
    if (state.chatSubbarMode === "alert") renderTradingChatSubbar();
    return state.alerts;
  }

  async function saveAlertDraft() {
    const draft = state.alertDraft ? makeAlertDraft(state.alertDraft) : makeAlertDraft();
    if (!String(draft.message || "").trim()) draft.message = buildAlertMessage(draft);
    const alert = {
      id: draft.id || undefined,
      instrument: state.selectedInstrument,
      granularity: state.selectedGranularity,
      conditionKind: "price",
      operator: draft.operator,
      targetValue: Number(draft.targetValue),
      triggerMode: draft.triggerMode,
      expirationTimeMs: draft.expirationTimeMs || null,
      message: String(draft.message || buildAlertMessage(draft)),
      active: true,
      notifications: {
        ...defaultAlertNotifications(),
        ...(draft.notifications || {}),
      },
    };
    if (!Number.isFinite(alert.targetValue)) return;
    if (hasTauriInvoke()) {
      await invoke("trading_alerts_upsert", { request: { alert } });
    } else {
      const now = tradingNowMs();
      const normalized = normalizeAlertRecord({
        ...alert,
        id: alert.id || `preview-${now}`,
        triggeredCount: 0,
        lastTriggeredAtMs: null,
        updatedAtMs: now,
      });
      const existingIndex = state.alerts.findIndex((item) => item.id === normalized.id);
      if (existingIndex >= 0) state.alerts.splice(existingIndex, 1, normalized);
      else state.alerts.unshift(normalized);
      refreshAlertRecords();
    }
    state.alertFormMode = "create";
    state.alertDraft = makeAlertDraft();
    await reloadAlerts({ render: true });
  }

  async function deleteAlertRecord(alertId) {
    const targetId = String(alertId || "").trim();
    if (!targetId) return;
    if (hasTauriInvoke()) {
      await invoke("trading_alerts_delete", { request: { id: targetId } });
    } else {
      state.alerts = state.alerts.filter((item) => String(item?.id || "") !== targetId);
      refreshAlertRecords();
    }
    if (state.alertDraft?.id === targetId) {
      state.alertFormMode = "create";
      state.alertDraft = makeAlertDraft();
    }
    await reloadAlerts({ render: true });
  }

  function canvasBridge() {
    return window.__forgeAlphaCanvasBridge || null;
  }

  function setDropdownScrim(open) {
    try {
      window.__forgeSetDropdownScrim?.(!!open);
    } catch (_) {}
  }

  function brokerApi() {
    return window.__forgeTradingBrokerApi || null;
  }

  function selectedBrokerKind() {
    return brokerApi()?.getActiveBroker?.() || "oanda";
  }

  function selectedBrokerLabel() {
    return brokerApi()?.getActiveBrokerLabel?.() || "OANDA";
  }

  function selectedBrokerLogoKind() {
    const kind = String(selectedBrokerKind() || "").trim().toLowerCase();
    if (kind === "oanda") return kind;
    return "";
  }

  function availableBrokers() {
    return brokerApi()?.listBrokers?.() || [{ kind: "oanda", label: "OANDA", active: true }];
  }

  function activeBrokerInstrumentSet() {
    const cacheKey = brokerInstrumentUniverseCacheKey();
    if (state.uiCache.brokerInstrumentSetKey === cacheKey && state.uiCache.brokerInstrumentSetValue instanceof Set) {
      return state.uiCache.brokerInstrumentSetValue;
    }
    const set = new Set();
    if (state.assetCatalog.length) {
      for (const item of state.assetCatalog) {
        const name = String(item?.name || item?.instrument || "").trim();
        if (name) set.add(name);
      }
    } else {
      for (const item of Array.isArray(state.snapshot?.instruments) ? state.snapshot.instruments : []) {
        const name = String(item?.name || item?.instrument || "").trim();
        if (name) set.add(name);
      }
      for (const name of tradingCatalogIndex().byInstrument.keys()) {
        if (name) set.add(name);
      }
    }
    state.uiCache.brokerInstrumentSetKey = cacheKey;
    state.uiCache.brokerInstrumentSetValue = set;
    return set;
  }

  function isSelectedInstrumentTradable() {
    return activeBrokerInstrumentSet().has(String(state.selectedInstrument || "").trim());
  }

  function timeframeLabel(granularity = state.selectedGranularity) {
    return TIMEFRAME_OPTIONS.find((item) => item.value === granularity)?.label || String(granularity || "");
  }

  function brokerInstrumentCode(raw) {
    const source = String(raw || "").trim();
    if (!source) return "";
    if (selectedBrokerKind() === "oanda") return source.replace(/[^A-Za-z0-9]/g, "");
    return source;
  }

  function splitInstrumentTokens(raw) {
    return String(raw || "")
      .trim()
      .split(/[_/:\-\s]+/)
      .map((token) => token.trim().toUpperCase())
      .filter(Boolean);
  }

  function assetTokenLabel(token) {
    return ASSET_TOKEN_LABELS[String(token || "").toUpperCase()] || String(token || "");
  }

  function inferAssetClass(raw, provided = "") {
    const explicit = String(provided || "").trim().toLowerCase();
    if (explicit) {
      if (explicit === "metal") return "commodity";
      if (explicit === "stock" || explicit === "equity_cfd") return "equity";
      if (explicit === "bond" || explicit === "bund") return "bond";
      return explicit;
    }
    const tokens = splitInstrumentTokens(raw);
    const [base = "", quote = ""] = tokens;
    if (["BTC", "ETH", "SOL", "XRP", "LTC"].includes(base)) return "crypto";
    if (["XAU", "XAG", "XCU", "XPT", "XPD"].includes(base)) return "commodity";
    if (["NATGAS", "WTI", "BRENT", "SILVER", "CORN", "SOYBN", "WHEAT", "SUGAR", "COTTON", "COCOA"].includes(base)) return "commodity";
    if (["BUND", "UST", "US10Y", "DE10Y", "DE10YB", "USB10Y", "UK10YB", "FR10YB"].includes(base) || /10YB$/.test(base)) return "bond";
    if (["AAPL", "TSLA", "NVDA", "AMZN", "MSFT", "META", "GOOG", "NFLX"].includes(base)) return "equity";
    if (base && quote && ASSET_TOKEN_LABELS[base] && ASSET_TOKEN_LABELS[quote]) return "forex";
    if (
      /[0-9]{2,}/.test(base)
      || /SPX|NAS|DJI|DE40|DE30|UK100|JP225|AU200|EU50|FRA40|FR40|CN50|HK33|CH20|CHINAH|ESPIX|NL25|SG30/i.test(base)
    ) return "index";
    return "instrument";
  }

  function humanAssetDisplayName(raw, provided = "") {
    const source = String(provided || "").trim();
    const code = brokerInstrumentCode(raw) || String(raw || "");
    if (source && source.replace(/\s+/g, "").toUpperCase() !== code.toUpperCase() && !/^[A-Z0-9/_-]+$/.test(source)) {
      return source;
    }
    const tokens = splitInstrumentTokens(raw);
    if (!tokens.length) return code;
    if (tokens.length === 1) return assetTokenLabel(tokens[0]);
    return tokens.map(assetTokenLabel).join(" / ");
  }

  function compareAssetCode(raw) {
    const tokens = splitInstrumentTokens(raw);
    if (tokens.length >= 2) return `${tokens[0]}/${tokens[1]}`;
    return brokerInstrumentCode(raw) || String(raw || "");
  }

  function compareAssetSubtitle(asset) {
    const source = String(asset?.displayName || "").trim();
    const shortCode = splitInstrumentTokens(asset?.name || "").join(" / ");
    if (source && source.toUpperCase() !== shortCode.toUpperCase()) return source;
    const tokens = splitInstrumentTokens(asset?.name || "");
    if (tokens.length >= 2) return tokens.map(assetTokenLabel).join(" / ");
    return humanAssetDisplayName(asset?.name || "", source);
  }

  function normalizeAssetEntry(item = {}) {
    const name = String(item?.name || item?.instrument || "").trim();
    if (!name) return null;
    if (!/[A-Za-z0-9]/.test(name)) return null;
    return {
      name,
      displayName: humanAssetDisplayName(name, item?.displayName || item?.display_name || ""),
      assetClass: inferAssetClass(name, item?.assetClass || item?.asset_class || ""),
      pipLocation: item?.pipLocation ?? item?.pip_location ?? null,
      displayPrecision: item?.displayPrecision ?? item?.display_precision ?? null,
      tradeUnitsPrecision: item?.tradeUnitsPrecision ?? item?.trade_units_precision ?? null,
      minimumTradeSize: item?.minimumTradeSize ?? item?.minimum_trade_size ?? null,
    };
  }

  function formatNumber(value, digits = 3) {
    const n = Number(value);
    return Number.isFinite(n) ? n.toFixed(digits) : "—";
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  function tradingNowMs() {
    return Date.now();
  }

  function toDateTimeLocalValue(timestampMs) {
    const value = Number(timestampMs);
    if (!Number.isFinite(value) || value <= 0) return "";
    const date = new Date(value);
    const pad = (part) => String(part).padStart(2, "0");
    return [
      `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
      `${pad(date.getHours())}:${pad(date.getMinutes())}`,
    ].join("T");
  }

  function fromDateTimeLocalValue(value) {
    const parsed = Date.parse(String(value || "").trim());
    return Number.isFinite(parsed) ? parsed : null;
  }

  function candleTimeMs(item, fallback = NaN) {
    const raw = item?.timeMs ?? item?.time;
    const numeric = Number(raw);
    if (Number.isFinite(numeric)) return Math.abs(numeric) < 1e12 ? numeric * 1000 : numeric;
    const parsed = Date.parse(String(raw || "").trim());
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  function normalizeCachedCandle(item) {
    if (item?.time == null) return null;
    const timeMs = candleTimeMs(item, NaN);
    if (!Number.isFinite(timeMs)) return null;
    return item.timeMs === timeMs ? item : { ...item, timeMs };
  }

  function defaultAlertNotifications() {
    return {
      app: true,
      toast: true,
      email: false,
      sound: true,
      emailTo: "",
      soundProfile: "soft",
      soundVolume: 0.82,
      soundRepeat: 2,
    };
  }

  function activeTradingReferencePrice() {
    const live = Number(state.market?.price?.mid);
    if (Number.isFinite(live)) return live;
    const candles = Array.isArray(state.candles) ? state.candles : [];
    const lastClose = Number(candles[candles.length - 1]?.close);
    return Number.isFinite(lastClose) ? lastClose : 0;
  }

  function buildAlertMessage(draft = {}) {
    const symbol = String(draft.instrument || state.selectedInstrument || DEFAULT_INSTRUMENT);
    const operatorLabel = ({
      crossing: "Croisement",
      crossing_up: "Croisement haussier",
      crossing_down: "Croisement baissier",
      above: "Au-dessus",
      below: "En dessous",
    })[String(draft.operator || "crossing")] || "Alerte";
    return `${symbol} ${operatorLabel} ${formatNumber(draft.targetValue, 3)}`;
  }

  function makeAlertDraft(overrides = {}) {
    const basePrice = activeTradingReferencePrice() || 0;
    const expirationTimeMs = tradingNowMs() + 7 * 24 * 60 * 60 * 1000;
    const draft = {
      id: "",
      instrument: state.selectedInstrument,
      granularity: state.selectedGranularity,
      conditionKind: "price",
      operator: "crossing",
      targetValue: Number.isFinite(Number(overrides.targetValue)) ? Number(overrides.targetValue) : basePrice,
      triggerMode: "once",
      expirationTimeMs,
      message: "",
      active: true,
      notifications: defaultAlertNotifications(),
      ...overrides,
    };
    draft.notifications = {
      ...defaultAlertNotifications(),
      ...(draft.notifications && typeof draft.notifications === "object" ? draft.notifications : {}),
    };
    draft.message = String(draft.message || buildAlertMessage(draft));
    return draft;
  }

  function normalizeAlertRecord(raw = {}) {
    const notifications = {
      ...defaultAlertNotifications(),
      ...(raw.notifications && typeof raw.notifications === "object" ? raw.notifications : {}),
    };
    return {
      id: String(raw.id || ""),
      instrument: String(raw.instrument || state.selectedInstrument || DEFAULT_INSTRUMENT),
      granularity: String(raw.granularity || state.selectedGranularity || DEFAULT_GRANULARITY),
      conditionKind: String(raw.conditionKind || "price"),
      operator: String(raw.operator || "crossing"),
      targetValue: Number(raw.targetValue),
      triggerMode: String(raw.triggerMode || "once"),
      expirationTimeMs: Number.isFinite(Number(raw.expirationTimeMs)) ? Number(raw.expirationTimeMs) : null,
      message: String(raw.message || ""),
      active: raw.active !== false,
      triggeredCount: Math.max(0, Number(raw.triggeredCount) || 0),
      lastTriggeredAtMs: Number.isFinite(Number(raw.lastTriggeredAtMs)) ? Number(raw.lastTriggeredAtMs) : null,
      updatedAtMs: Number.isFinite(Number(raw.updatedAtMs)) ? Number(raw.updatedAtMs) : null,
      notifications,
    };
  }

  function mapAlertForCanvas(alert) {
    const record = normalizeAlertRecord(alert);
    return mapNormalizedAlertForCanvas(record);
  }

  function mapNormalizedAlertForCanvas(record) {
    return {
      id: record.id,
      instrument: record.instrument,
      granularity: record.granularity,
      operator: record.operator,
      targetValue: record.targetValue,
      message: record.message,
      active: record.active,
      triggeredCount: record.triggeredCount,
      lastTriggeredAtMs: record.lastTriggeredAtMs,
    };
  }

  function alertRecordsSignature(alerts = state.alerts) {
    const rows = Array.isArray(alerts) ? alerts : [];
    let out = `${rows.length}`;
    for (const alert of rows) {
      const notifications = alert.notifications || {};
      out += `|${alert.id}:${alert.instrument}:${alert.granularity}:${alert.conditionKind}:${alert.operator}:${alert.targetValue}:${alert.triggerMode}:${alert.expirationTimeMs || 0}:${alert.active ? 1 : 0}:${alert.triggeredCount}:${alert.lastTriggeredAtMs || 0}:${alert.updatedAtMs || 0}:${notifications.app ? 1 : 0}:${notifications.toast ? 1 : 0}:${notifications.email ? 1 : 0}:${notifications.sound ? 1 : 0}:${notifications.emailTo || ""}:${notifications.soundProfile || ""}:${notifications.soundVolume || ""}:${notifications.soundRepeat || ""}:${alert.message}`;
    }
    return out;
  }

  function invalidateContextCaches() {
    state.uiCache.contextSnapshotKey = "";
    state.uiCache.contextSnapshotValue = null;
    state.uiCache.contextDigestKey = "";
    state.uiCache.contextDigestValue = "";
    state.uiCache.contextDigestV3Key = "";
    state.uiCache.contextDigestV3Value = "";
  }

  function invalidateAlertCaches() {
    alertUniverseRevision += 1;
    state.uiCache.canvasAlertsKey = "";
    state.uiCache.canvasAlertsValue = null;
    state.uiCache.instrumentAlertsKey = "";
    state.uiCache.instrumentAlertsValue = null;
    state.uiCache.alertModalKey = "";
    invalidateContextCaches();
  }

  function refreshAlertRecords() {
    const normalized = state.alerts.map(normalizeAlertRecord);
    const signature = alertRecordsSignature(normalized);
    if (signature === alertUniverseSignature) {
      state.alerts = normalized;
      return state.alerts;
    }
    state.alerts = normalized;
    alertUniverseSignature = signature;
    invalidateAlertCaches();
    return state.alerts;
  }

  function setAlertRecords(alerts = []) {
    const normalized = (Array.isArray(alerts) ? alerts : []).map(normalizeAlertRecord);
    const signature = alertRecordsSignature(normalized);
    if (signature === alertUniverseSignature) return state.alerts;
    state.alerts = normalized;
    alertUniverseSignature = signature;
    invalidateAlertCaches();
    return state.alerts;
  }

  function activePriceLine() {
    const price = state.market?.price || state.snapshot?.price || null;
    if (!price || price.instrument !== state.selectedInstrument) {
      return `${state.selectedInstrument} · ${state.selectedGranularity}`;
    }
    return [
      `${price.instrument} · ${state.selectedGranularity}`,
      `bid ${formatNumber(price.bid)}`,
      `ask ${formatNumber(price.ask)}`,
      `mid ${formatNumber(price.mid)}`,
      `spr ${formatNumber(price.spread, 4)}`,
    ].join("  ");
  }

  function findAsset(instrument = state.selectedInstrument) {
    return assetSearchRecord(instrument)?.asset || null;
  }

  function renderIndicatorDock() {
    if (!els.indicatorDock) return;
    if (!state.active || !state.activeIndicators.length) {
      const emptyKey = `inactive::${state.active ? 1 : 0}`;
      if (state.uiCache.indicatorDockKey === emptyKey && els.indicatorDock.hidden) return;
      state.uiCache.indicatorDockKey = emptyKey;
      els.indicatorDock.hidden = true;
      els.indicatorDock.innerHTML = "";
      return;
    }
    const dockKey = [
      activeIndicatorRevision,
      state.activeIndicators.map((indicator) => `${indicator.id}:${indicator.command}:${indicator.visible === false ? 0 : 1}`).join("|"),
    ].join("::");
    if (state.uiCache.indicatorDockKey === dockKey) return;
    const markup = state.activeIndicators.map((indicator) => `
      <div class="trading-indicator-chip${indicator.visible === false ? " is-hidden" : ""}" data-indicator-id="${escapeHtml(indicator.id)}">
        <button type="button" class="trading-indicator-chip-label" data-indicator-action="inject" data-indicator-id="${escapeHtml(indicator.id)}">${escapeHtml(indicator.command)}</button>
        <div class="trading-indicator-chip-tools">
          <button type="button" class="trading-indicator-chip-tool" data-indicator-action="settings" data-indicator-id="${escapeHtml(indicator.id)}" aria-label="Indicator settings">
            ${tradingSubbarIcon("settings")}
          </button>
          <button type="button" class="trading-indicator-chip-tool" data-indicator-action="toggle" data-indicator-id="${escapeHtml(indicator.id)}" aria-label="${indicator.visible === false ? "Show indicator" : "Hide indicator"}">
            ${indicator.visible === false
              ? `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M2.5 10s2.9-4.5 7.5-4.5S17.5 10 17.5 10s-2.9 4.5-7.5 4.5S2.5 10 2.5 10Z"/><path d="M10 7.7a2.3 2.3 0 1 0 0 4.6 2.3 2.3 0 0 0 0-4.6Z"/><path d="m4 4 12 12"/></svg>`
              : `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M2.5 10s2.9-4.5 7.5-4.5S17.5 10 17.5 10s-2.9 4.5-7.5 4.5S2.5 10 2.5 10Z"/><path d="M10 7.7a2.3 2.3 0 1 0 0 4.6 2.3 2.3 0 0 0 0-4.6Z"/></svg>`}
          </button>
          <button type="button" class="trading-indicator-chip-tool" data-indicator-action="remove" data-indicator-id="${escapeHtml(indicator.id)}" aria-label="Remove indicator">
            <svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M5.75 6.25h8.5"/><path d="M7.25 6.25V5.1A1.1 1.1 0 0 1 8.35 4h3.3a1.1 1.1 0 0 1 1.1 1.1v1.15"/><path d="M6.7 6.25 7.2 15a1.2 1.2 0 0 0 1.2 1.13h3.2A1.2 1.2 0 0 0 12.8 15l.5-8.75"/><path d="M8.7 8.5v5.2"/><path d="M11.3 8.5v5.2"/></svg>
          </button>
        </div>
      </div>
    `).join("");
    state.uiCache.indicatorDockKey = dockKey;
    els.indicatorDock.hidden = false;
    els.indicatorDock.innerHTML = markup;
  }

  function addedChartInstruments() {
    const raw = Array.isArray(state.addInstruments)
      ? state.addInstruments
      : (state.addInstrument ? [state.addInstrument] : []);
    const seen = new Set();
    const out = [];
    for (const value of raw) {
      const instrument = String(value || "").trim();
      if (!instrument || instrument === state.selectedInstrument || seen.has(instrument)) continue;
      seen.add(instrument);
      out.push(instrument);
      if (out.length >= MAX_ADDED_CHARTS) break;
    }
    return out;
  }

  function setAddedChartInstruments(instruments = []) {
    const seen = new Set();
    const out = [];
    for (const value of Array.isArray(instruments) ? instruments : []) {
      const instrument = String(value || "").trim();
      if (!instrument || instrument === state.selectedInstrument || seen.has(instrument)) continue;
      seen.add(instrument);
      out.push(instrument);
    }
    state.addInstruments = out.slice(-MAX_ADDED_CHARTS);
  }

  function syncTradingHeader() {
    const asset = findAsset(state.selectedInstrument);
    const addInstruments = addedChartInstruments();
    const instrumentTradable = isSelectedInstrumentTradable();
    const brokerHeaderLabel = instrumentTradable ? selectedBrokerLabel() : "PAPERTRADING";
    const headerPayload = {
      active: state.active,
      brokerLabel: brokerHeaderLabel,
      brokerLogoKind: instrumentTradable ? selectedBrokerLogoKind() : "",
      instrumentLabel: brokerInstrumentCode(state.selectedInstrument) || state.selectedInstrument,
      compareLabel: "compare_",
      compareActive: state.compareInstruments.length > 0,
      addLabel: "add_",
      addActive: addInstruments.length > 0,
      instrumentDisplayName: asset?.displayName || state.selectedInstrument,
    };
    const headerKey = JSON.stringify([
      assetUniverseRevision,
      selectedBrokerKind(),
      state.selectedInstrument,
      state.compareInstruments.join("|"),
      addInstruments.join("|"),
      headerPayload.active ? 1 : 0,
      headerPayload.brokerLabel,
      headerPayload.brokerLogoKind,
      headerPayload.instrumentLabel,
      headerPayload.compareActive ? 1 : 0,
      headerPayload.addActive ? 1 : 0,
      headerPayload.instrumentDisplayName,
    ]);
    if (state.uiCache.tradingHeaderKey !== headerKey) {
      state.uiCache.tradingHeaderKey = headerKey;
      canvasBridge()?.setTradingHeader?.(headerPayload);
    }
    if (els.compareTriggerLabel && els.compareTriggerLabel.textContent !== "compare_") {
      els.compareTriggerLabel.textContent = "compare_";
    }
    if (els.addTriggerLabel && els.addTriggerLabel.textContent !== "add_") {
      els.addTriggerLabel.textContent = "add_";
    }
    if (els.addTrigger) {
      els.addTrigger.dataset.active = addInstruments.length ? "true" : "false";
    }
    renderIndicatorDock();
    syncCompareShellState();
    syncProgramTrigger();
    syncAlertTrigger();
  }

  function headerTokenForBroker() {
    const label = isSelectedInstrumentTradable() ? selectedBrokerLabel() : "PAPERTRADING";
    return `<broker:${label}>`;
  }

  function headerTokenForInstrument() {
    return `<instrument:${brokerInstrumentCode(state.selectedInstrument) || state.selectedInstrument}>`;
  }

  function headerTokenForCompare() {
    const instruments = state.compareInstruments
      .filter((name) => name && name !== state.selectedInstrument)
      .map((name) => brokerInstrumentCode(name) || name);
    return instruments.length ? `<compare:${instruments.join(",")}>` : "<compare:none>";
  }

  function tokenForBrokerKind(kind = "") {
    const hit = availableBrokers().find((broker) => String(broker?.kind || "").trim().toLowerCase() === String(kind || "").trim().toLowerCase());
    return `<broker:${String(hit?.label || kind || selectedBrokerLabel())}>`;
  }

  function tokenForInstrumentName(name = "") {
    const instrument = String(name || state.selectedInstrument || "").trim();
    return `<instrument:${brokerInstrumentCode(instrument) || instrument}>`;
  }

  function tokenForDisplayMode(mode = "") {
    return `<chart_mode:${String(mode || state.chartDisplayMode || "candles")}>`;
  }

  function tokenForOrderDraftField(field = "") {
    const draft = summarizeOrderDraft(state.orderForm);
    const mapping = {
      instrument: draft.instrument || state.selectedInstrument,
      side: draft.side || "BUY",
      units: String(draft.units || 0),
      orderType: draft.orderType || "MARKET",
      limitPrice: formatNumber(draft.limitPrice),
      stopLoss: formatNumber(draft.stopLoss),
      takeProfit: formatNumber(draft.takeProfit),
    };
    return `<order_draft:${field}=${mapping[field] ?? "n/a"}>`;
  }

  function tokenForAlertField(field = "", draft = state.alertDraft || makeAlertDraft()) {
    if (field === "operator") return `<alert:operator=${draft.operator}>`;
    if (field === "targetValue") return `<alert:target=${formatNumber(draft.targetValue, 3)}>`;
    if (field === "triggerMode") return `<alert:trigger=${draft.triggerMode}>`;
    if (field === "expirationTimeMs") return `<alert:expiration=${draft.expirationTimeMs || "none"}>`;
    if (field === "message") return `<alert:message=${draft.message || "n/a"}>`;
    if (field === "emailTo") return `<alert:email_to=${draft.notifications?.emailTo || "n/a"}>`;
    if (field === "soundProfile") return `<alert:sound_profile=${draft.notifications?.soundProfile || "soft"}>`;
    if (field === "soundVolume") return `<alert:sound_volume=${formatNumber((draft.notifications?.soundVolume || 0) * 100, 0)}>`;
    if (field === "soundRepeat") return `<alert:sound_repeat=${draft.notifications?.soundRepeat || 1}>`;
    return `<alert:${field || "panel"}>`;
  }

  function injectIndicatorSlashCommand(indicatorId = "") {
    const definition = tradingIndicatorDefinition(indicatorId);
    if (!definition) return;
    try {
      window.__forgeTradingInjectChatToken?.(definition.command);
    } catch (_) {
      injectTradingChatToken(definition.command);
    }
  }

  function extractTradingSlashTokens(raw = "") {
    return String(raw || "")
      .trim()
      .split(/\s+/)
      .map((token) => String(token || "").trim().toLowerCase())
      .filter((token) => token.startsWith("/"));
  }

  function upsertTradingIndicator(indicatorId = "", overrides = {}) {
    const definition = tradingIndicatorDefinition(indicatorId);
    if (!definition) return null;
    const current = activeIndicatorInstance(definition.id);
    const next = normalizeTradingIndicatorInstance({
      id: definition.id,
      visible: overrides.visible ?? current?.visible ?? true,
      settings: {
        ...(current?.settings || cloneIndicatorSettings(definition)),
        ...(overrides.settings || {}),
      },
    });
    if (!next) return null;
    if (current) {
      state.activeIndicators = state.activeIndicators.map((entry) => entry.id === next.id ? next : entry);
    } else {
      state.activeIndicators = [...state.activeIndicators, next];
    }
    activeIndicatorRevision += 1;
    invalidateTradingSubbarCache();
    persistTradingIndicators();
    renderIndicatorDock();
    renderTradingChatSubbar();
    syncTradingIndicatorsToCanvas();
    return next;
  }

  function removeTradingIndicator(indicatorId = "") {
    const normalized = String(indicatorId || "").trim().toLowerCase();
    const before = state.activeIndicators.length;
    state.activeIndicators = state.activeIndicators.filter((entry) => entry.id !== normalized);
    if (state.indicatorSettingsId === normalized) state.indicatorSettingsId = "";
    if (before === state.activeIndicators.length) return;
    activeIndicatorRevision += 1;
    invalidateTradingSubbarCache();
    persistTradingIndicators();
    renderIndicatorDock();
    renderTradingChatSubbar();
    syncTradingIndicatorsToCanvas();
    if (state.headerMenuMode === "indicator-settings" && !activeIndicatorInstance(normalized)) {
      closeHeaderMenu();
    }
  }

  function toggleTradingIndicatorVisibility(indicatorId = "") {
    const current = activeIndicatorInstance(indicatorId);
    if (!current) return;
    upsertTradingIndicator(indicatorId, { visible: current.visible === false });
  }

  function syncCompareShellState() {
    const compareOpen = !!(state.active && (state.headerMenuMode === "compare" || state.headerMenuMode === "add") && els.compareMenu?.hidden === false);
    const headerMenuOpen = !!(state.active && state.headerMenuMode !== "none" && els.compareMenu?.hidden === false);
    els.canvasWrap?.classList.toggle("is-trading-compare-open", compareOpen);
    els.canvasWrap?.classList.toggle("is-trading-header-menu-open", headerMenuOpen);
    els.topbarBreadcrumb?.classList.toggle("is-trading-compare-open", compareOpen);
    els.topbarBreadcrumb?.classList.toggle("is-trading-header-menu-open", headerMenuOpen);
  }

  function invalidateExtraChartsCache() {
    state.uiCache.extraChartsKey = "";
    state.uiCache.extraChartsValue = null;
  }

  function invalidateAssetUniverseCache(options = {}) {
    assetUniverseRevision += 1;
    state.uiCache.availableAssetsKey = "";
    state.uiCache.availableAssetsValue = null;
    state.uiCache.libraryAssetsKey = "";
    state.uiCache.libraryAssetsValue = null;
    state.uiCache.assetSearchKey = "";
    state.uiCache.assetSearchValue = null;
    state.uiCache.compareMenuModelKey = "";
    state.uiCache.compareMenuModelValue = null;
    invalidateExtraChartsCache();
    invalidateContextCaches();
    if (options.catalog) {
      catalogUniverseRevision += 1;
      state.historySeriesMisses.clear();
      state.uiCache.brokerInstrumentSetKey = "";
      state.uiCache.brokerInstrumentSetValue = null;
      state.uiCache.catalogIndexKey = "";
      state.uiCache.catalogIndexValue = null;
      state.uiCache.granularitiesKey = "";
      state.uiCache.granularitiesValue = null;
    }
  }

  function assetUniverseCacheKey() {
    return [
      assetUniverseRevision,
      hasTauriInvoke() ? 1 : 0,
      String(state.selectedInstrument || ""),
      state.compareInstruments.join("|"),
      addedChartInstruments().join("|"),
      state.assetCatalog.length,
      state.catalog.length,
      Array.isArray(state.snapshot?.instruments) ? state.snapshot.instruments.length : 0,
    ].join("::");
  }

  function brokerInstrumentUniverseCacheKey() {
    return [
      catalogUniverseRevision,
      hasTauriInvoke() ? 1 : 0,
      selectedBrokerKind(),
      state.assetCatalog.length,
      state.catalog.length,
      Array.isArray(state.snapshot?.instruments) ? state.snapshot.instruments.length : 0,
    ].join("::");
  }

  function tradingCatalogIndex() {
    const cacheKey = `${catalogUniverseRevision}::${state.catalog.length}`;
    if (state.uiCache.catalogIndexKey === cacheKey && state.uiCache.catalogIndexValue) {
      return state.uiCache.catalogIndexValue;
    }
    const pairRows = new Map();
    const byInstrument = new Map();
    const granularityCoverage = {};
    for (const entry of Array.isArray(state.catalog) ? state.catalog : []) {
      const instrument = String(entry?.instrument || "").trim();
      const granularity = String(entry?.granularity || "").trim().toUpperCase();
      if (!instrument) continue;
      if (!byInstrument.has(instrument)) byInstrument.set(instrument, []);
      byInstrument.get(instrument).push(entry);
      if (granularity) {
        pairRows.set(chartCacheKey(instrument, granularity), Math.max(0, Number(entry?.rows || 0)));
        granularityCoverage[granularity] = (granularityCoverage[granularity] || 0) + 1;
      }
    }
    const index = { pairRows, byInstrument, granularityCoverage };
    state.uiCache.catalogIndexKey = cacheKey;
    state.uiCache.catalogIndexValue = index;
    return index;
  }

  function syncSnapshotCatalog(files, options = {}) {
    const incoming = Array.isArray(files) ? files.slice() : [];
    const nextCatalog = options.replace
      ? incoming
      : mergeHistoryCatalog(state.catalog, incoming);
    state.catalog = nextCatalog;
    syncAssetCatalog(options.assets, nextCatalog);
    if (state.snapshot) {
      state.snapshot.historyFiles = nextCatalog.slice();
    }
  }

  function buildAssetCatalogFromFiles(files = state.catalog) {
    const byInstrument = new Map();
    for (const entry of Array.isArray(files) ? files : []) {
      const instrument = String(entry?.instrument || "").trim();
      const granularity = String(entry?.granularity || "").trim().toUpperCase();
      if (!instrument) continue;
      const normalized = normalizeAssetEntry({ name: instrument });
      if (!normalized) continue;
      if (!byInstrument.has(instrument)) {
        byInstrument.set(instrument, {
          name: instrument,
          displayName: normalized.displayName,
          assetClass: normalized.assetClass,
          granularities: [],
          rows: 0,
          firstTime: "",
          lastTime: "",
          updatedAtMs: 0,
        });
      }
      const asset = byInstrument.get(instrument);
      if (granularity && !asset.granularities.includes(granularity)) {
        asset.granularities.push(granularity);
      }
      asset.rows += Number(entry?.rows || 0);
      asset.updatedAtMs = Math.max(asset.updatedAtMs, Number(entry?.updatedAtMs || 0));
      const firstTime = String(entry?.firstTime || "");
      const lastTime = String(entry?.lastTime || "");
      if (firstTime && (!asset.firstTime || firstTime < asset.firstTime)) asset.firstTime = firstTime;
      if (lastTime && (!asset.lastTime || lastTime > asset.lastTime)) asset.lastTime = lastTime;
    }
    return Array.from(byInstrument.values()).sort((a, b) => {
      if (a.name === DEFAULT_INSTRUMENT) return -1;
      if (b.name === DEFAULT_INSTRUMENT) return 1;
      return a.name.localeCompare(b.name);
    });
  }

  function syncAssetCatalog(assets = [], files = state.catalog) {
    const normalizedAssets = Array.isArray(assets) && assets.length
      ? assets
        .map((item) => {
          const normalized = normalizeAssetEntry({
            name: item?.instrument || item?.name || "",
            displayName: item?.displayName || item?.display_name || "",
            assetClass: item?.assetClass || item?.asset_class || "",
          });
          if (!normalized) return null;
          return {
            ...normalized,
            granularities: Array.isArray(item?.granularities)
              ? item.granularities.map((value) => String(value || "").trim().toUpperCase()).filter(Boolean)
              : [],
            rows: Number(item?.rows || 0),
            firstTime: String(item?.firstTime || item?.first_time || ""),
            lastTime: String(item?.lastTime || item?.last_time || ""),
            updatedAtMs: Number(item?.updatedAtMs || item?.updated_at_ms || 0),
          };
        })
        .filter(Boolean)
      : buildAssetCatalogFromFiles(files);
    state.assetCatalog = normalizedAssets;
    invalidateAssetUniverseCache({ catalog: true });
    if (state.snapshot) {
      state.snapshot.assetCatalog = normalizedAssets.slice();
    }
  }

  function mergeHistoryCatalog(baseFiles = [], updateFiles = []) {
    const merged = new Map();
    for (const entry of Array.isArray(baseFiles) ? baseFiles : []) {
      const instrument = String(entry?.instrument || "").trim();
      const granularity = String(entry?.granularity || "").trim().toUpperCase();
      if (!instrument || !granularity) continue;
      merged.set(`${instrument}::${granularity}`, entry);
    }
    for (const entry of Array.isArray(updateFiles) ? updateFiles : []) {
      const instrument = String(entry?.instrument || "").trim();
      const granularity = String(entry?.granularity || "").trim().toUpperCase();
      if (!instrument || !granularity) continue;
      merged.set(`${instrument}::${granularity}`, entry);
    }
    return Array.from(merged.values()).sort((a, b) =>
      String(a?.instrument || "").localeCompare(String(b?.instrument || ""))
      || String(a?.granularity || "").localeCompare(String(b?.granularity || ""))
    );
  }

  function compareUniverseCoverageNeedsSync(files = state.catalog) {
    const catalog = Array.isArray(files) ? files : [];
    if (!catalog.length) return true;
    const requiredGranularities = new Set(FULL_HISTORY_GRANULARITIES.map((item) => String(item || "").toUpperCase()));
    const seenPairs = new Set();
    for (const entry of catalog) {
      const instrument = String(entry?.instrument || "").trim();
      const granularity = String(entry?.granularity || "").trim().toUpperCase();
      if (!instrument || !granularity) continue;
      seenPairs.add(`${instrument}::${granularity}`);
    }
    const brokerInstrumentNames = new Set(
      Array.isArray(state.snapshot?.instruments)
        ? state.snapshot.instruments
          .map((item) => String(item?.name || "").trim())
          .filter(Boolean)
        : []
    );
    if (brokerInstrumentNames.size > 0) {
      for (const instrument of brokerInstrumentNames) {
        for (const granularity of requiredGranularities) {
          if (!seenPairs.has(`${instrument}::${granularity}`)) return true;
        }
      }
    } else {
      for (const granularity of requiredGranularities) {
        if (!catalog.some((entry) => String(entry?.granularity || "").trim().toUpperCase() === granularity)) return true;
      }
    }
    return false;
  }

  async function ensureUniverseHistorySync(options = {}) {
    if (!hasTauriInvoke()) return;
    if (state.universeHistorySyncDone && !options.force) return;
    if (state.universeHistorySyncPromise) {
      await state.universeHistorySyncPromise;
      return;
    }
    state.universeHistorySyncPromise = (async () => {
      try {
        let existingCatalog = state.catalog;
        try {
          const catalogResponse = await invoke("trading_oanda_history_catalog");
          if (Array.isArray(catalogResponse?.files)) {
            existingCatalog = catalogResponse.files;
            syncSnapshotCatalog(existingCatalog, {
              replace: true,
              assets: catalogResponse?.assets,
            });
          }
        } catch (_) {}
        const shouldSync = !!options.force || compareUniverseCoverageNeedsSync(existingCatalog);
        if (!shouldSync) {
          state.universeHistorySyncDone = true;
          return;
        }
        state.ordersOutput = "Syncing OANDA history from 2006 with a TradingView-style base-feed plan: fetch minimal native feeds, rebuild higher timeframes locally…";
        canvasBridge()?.refreshRightPanel?.();
        const response = await invoke("trading_oanda_sync_history", {
          request: {
            granularities: FULL_HISTORY_GRANULARITIES.slice(),
          },
        });
        syncSnapshotCatalog(Array.isArray(response?.files) ? response.files : [], {
          replace: true,
          assets: response?.assets,
        });
        state.ordersOutput = [
          "Full OANDA library synced.",
          ...((response?.notes || []).slice(0, 4)),
        ].join("\n");
        state.universeHistorySyncDone = true;
        renderLeftPanel();
        renderTimeframeRail();
        if (state.active) {
          await syncComparisonSeries();
        }
        canvasBridge()?.refreshRightPanel?.();
      } catch (error) {
        state.ordersOutput = `OANDA universe sync pending: ${error?.message || error || "unknown error"}`;
        canvasBridge()?.refreshRightPanel?.();
      }
    })().finally(() => {
      state.universeHistorySyncPromise = null;
    });
    await state.universeHistorySyncPromise;
  }

  async function ensureSelectedHistorySync(options = {}) {
    if (!hasTauriInvoke()) return;
    const instrument = String(options.instrument || state.selectedInstrument || "").trim();
    const granularity = String(options.granularity || state.selectedGranularity || "").trim().toUpperCase();
    if (!instrument || !granularity) return;
    const syncKey = `${instrument}::${granularity}`;
    if (state.selectedHistorySyncPromise && state.selectedHistorySyncKey === syncKey && !options.force) {
      await state.selectedHistorySyncPromise;
      return;
    }
    state.selectedHistorySyncKey = syncKey;
    state.selectedHistorySyncPromise = (async () => {
      try {
        const response = await invoke("trading_oanda_sync_history", {
          request: {
            instruments: [instrument],
            granularities: [granularity],
          },
        });
        if (Array.isArray(response?.files)) {
          syncSnapshotCatalog(response.files, {
            replace: true,
            assets: response?.assets,
          });
        }
      } catch (_) {
        // Live market feed still drives the visible chart; selected history sync is best-effort.
      }
    })().finally(() => {
      if (state.selectedHistorySyncKey === syncKey) {
        state.selectedHistorySyncPromise = null;
      }
    });
    await state.selectedHistorySyncPromise;
  }

  function previewCandles(count = 240, stepMs = 4 * 3_600_000) {
    const out = [];
    let close = 2.74;
    const start = Date.UTC(2025, 0, 1, 0, 0, 0);
    for (let i = 0; i < count; i += 1) {
      const open = close;
      close = Math.max(1.5, open + Math.sin(i / 8) * 0.03 + Math.cos(i / 19) * 0.02 + 0.002);
      const high = Math.max(open, close) + 0.02 + Math.abs(Math.sin(i / 5)) * 0.01;
      const low = Math.min(open, close) - 0.02 - Math.abs(Math.cos(i / 7)) * 0.01;
      out.push({
        time: new Date(start + i * stepMs).toISOString(),
        open,
        high,
        low,
        close,
        volume: 120 + (i % 18) * 11,
      });
    }
    return out;
  }

  function mergeCandles(base, recent) {
    const baseList = sortedUniqueCandleList(base);
    const recentList = normalizeCandleList(recent);
    if (!baseList.length) {
      return markCandleSeries(recentList.sort((a, b) => a.timeMs - b.timeMs), true);
    }
    if (!recentList.length) return baseList;
    const updates = new Map();
    for (const candle of recentList) updates.set(String(candle.time), candle);
    const baseMerged = [];
    const usedUpdates = new Set();
    for (const candle of baseList) {
      const key = String(candle.time);
      if (updates.has(key)) {
        baseMerged.push(updates.get(key));
        usedUpdates.add(key);
      } else {
        baseMerged.push(candle);
      }
    }
    const additions = [];
    for (const [key, candle] of updates) {
      if (!usedUpdates.has(key)) additions.push(candle);
    }
    additions.sort((a, b) => a.timeMs - b.timeMs);
    return mergeSortedCandleLists(baseMerged, additions);
  }

  function mergeCandlesLegacy(base, recent) {
    const map = new Map();
    const absorb = (candles) => {
      if (!Array.isArray(candles)) return;
      for (const candle of candles) {
        if (candle?.time == null) continue;
        const normalized = normalizeCachedCandle(candle);
        if (normalized) map.set(String(normalized.time), normalized);
      }
    };
    absorb(base);
    absorb(recent);
    return markCandleSeries(
      Array.from(map.values()).sort((a, b) => candleTimeMs(a, 0) - candleTimeMs(b, 0)),
      true,
    );
  }

  function normalizeCandleList(candles) {
    if (!Array.isArray(candles) || !candles.length) return [];
    const meta = candleSeriesMeta.get(candles);
    if (meta?.normalized) return candles;
    const out = [];
    for (const candle of candles) {
      const normalized = normalizeCachedCandle(candle);
      if (normalized) out.push(normalized);
    }
    return markCandleSeries(out, false);
  }

  function sortedUniqueCandleList(candles) {
    const normalized = normalizeCandleList(candles);
    if (!normalized.length) return normalized;
    const meta = candleSeriesMeta.get(normalized);
    if (meta?.sortedUnique) return normalized;
    if (isSortedUniqueCandleSeries(normalized)) {
      return markCandleSeries(normalized, true);
    }
    return mergeCandlesLegacy([], normalized);
  }

  function markCandleSeries(candles, sortedUnique = false) {
    if (Array.isArray(candles)) {
      candleSeriesMeta.set(candles, {
        normalized: true,
        sortedUnique: !!sortedUnique,
      });
    }
    return candles;
  }

  function isSortedUniqueCandleSeries(candles) {
    let prevTime = -Infinity;
    let prevKey = "";
    for (const candle of candles) {
      const timeMs = candleTimeMs(candle, NaN);
      if (!Number.isFinite(timeMs) || timeMs < prevTime) return false;
      const key = String(candle.time);
      if (timeMs === prevTime && key === prevKey) return false;
      prevTime = timeMs;
      prevKey = key;
    }
    return true;
  }

  function mergeSortedCandleLists(baseList, additions) {
    if (!additions.length) return markCandleSeries(baseList.slice(), true);
    const out = [];
    let baseIndex = 0;
    let addIndex = 0;
    while (baseIndex < baseList.length || addIndex < additions.length) {
      const baseCandle = baseList[baseIndex];
      const addCandle = additions[addIndex];
      if (!addCandle || (baseCandle && candleTimeMs(baseCandle, 0) <= candleTimeMs(addCandle, 0))) {
        out.push(baseCandle);
        baseIndex += 1;
      } else {
        out.push(addCandle);
        addIndex += 1;
      }
    }
    return markCandleSeries(out, true);
  }

  function chartCacheKey(instrument, granularity) {
    return `${String(instrument || "").trim()}::${String(granularity || "").trim().toUpperCase()}`;
  }

  function setCachedSeries(instrument, granularity, candles) {
    const key = chartCacheKey(instrument, granularity);
    const normalized = sortedUniqueCandleList(candles);
    state.chartCache.set(key, normalized);
    state.historySeriesMisses.delete(key);
    chartSeriesRevision += 1;
    chartSeriesRevisions.set(key, chartSeriesRevision);
    return normalized;
  }

  function getCachedSeries(instrument, granularity) {
    return state.chartCache.get(chartCacheKey(instrument, granularity)) || [];
  }

  function cachedSeriesRevision(instrument, granularity) {
    return chartSeriesRevisions.get(chartCacheKey(instrument, granularity)) || 0;
  }

  function deleteCachedSeries(instrument, granularity) {
    const key = chartCacheKey(instrument, granularity);
    state.chartCache.delete(key);
    state.historySeriesMisses.delete(key);
    chartSeriesRevision += 1;
    chartSeriesRevisions.set(key, chartSeriesRevision);
  }

  function rememberHistorySeriesMiss(key) {
    if (!key) return;
    if (!state.historySeriesMisses.has(key) && state.historySeriesMisses.size >= HISTORY_SERIES_MISS_CACHE_MAX) {
      const oldest = state.historySeriesMisses.values().next().value;
      if (oldest) state.historySeriesMisses.delete(oldest);
    }
    state.historySeriesMisses.add(key);
  }

  function expectedHistoryRows(instrument, granularity) {
    return tradingCatalogIndex().pairRows.get(chartCacheKey(instrument, granularity)) || 0;
  }

  function compactAssetToken(value) {
    return String(value || "").toUpperCase().replace(/[^A-Z0-9]+/g, "");
  }

  function availableAssets() {
    const cacheKey = assetUniverseCacheKey();
    if (state.uiCache.availableAssetsKey === cacheKey && Array.isArray(state.uiCache.availableAssetsValue)) {
      return state.uiCache.availableAssetsValue;
    }
    const catalogByName = new Map();
    for (const asset of state.assetCatalog) {
      if (asset?.name) catalogByName.set(asset.name, asset);
    }
    const snapshotInstruments = Array.isArray(state.snapshot?.instruments) ? state.snapshot.instruments : [];
    const catalogInstruments = Array.isArray(state.catalog)
      ? state.catalog.map((item) => ({ name: item?.instrument || "" }))
      : [];
    const hasLoadedUniverse = snapshotInstruments.length
      || catalogInstruments.some((item) => String(item?.name || "").trim());
    const source = state.assetCatalog.length
      ? [
        ...state.assetCatalog,
        { name: state.selectedInstrument },
        ...state.compareInstruments.map((name) => ({ name })),
        ...addedChartInstruments().map((name) => ({ name })),
      ]
      : [
        ...(hasLoadedUniverse ? [] : PREVIEW_ASSETS),
        ...snapshotInstruments,
        ...catalogInstruments,
        { name: state.selectedInstrument },
        ...state.compareInstruments.map((name) => ({ name })),
        ...addedChartInstruments().map((name) => ({ name })),
      ];
    const seen = new Set();
    const out = [];
    for (const item of source) {
      const normalized = normalizeAssetEntry(item);
      if (!normalized || seen.has(normalized.name)) continue;
      seen.add(normalized.name);
      const catalogHit = catalogByName.get(normalized.name);
      out.push(catalogHit ? { ...catalogHit } : normalized);
    }
    if (!seen.has(state.selectedInstrument)) {
      out.unshift(normalizeAssetEntry({ name: state.selectedInstrument }));
    }
    out.sort((a, b) => {
      if (a.name === DEFAULT_INSTRUMENT) return -1;
      if (b.name === DEFAULT_INSTRUMENT) return 1;
      return a.name.localeCompare(b.name);
    });
    state.uiCache.availableAssetsKey = cacheKey;
    state.uiCache.availableAssetsValue = out;
    return out;
  }

  function libraryAssets() {
    const cacheKey = `${assetUniverseCacheKey()}::library`;
    if (state.uiCache.libraryAssetsKey === cacheKey && Array.isArray(state.uiCache.libraryAssetsValue)) {
      return state.uiCache.libraryAssetsValue;
    }
    const out = availableAssets().slice();
    out.sort((a, b) => {
      if (a.name === state.selectedInstrument) return -1;
      if (b.name === state.selectedInstrument) return 1;
      return a.name.localeCompare(b.name);
    });
    state.uiCache.libraryAssetsKey = cacheKey;
    state.uiCache.libraryAssetsValue = out;
    return out;
  }

  function assetSearchIndex() {
    const cacheKey = `${assetUniverseCacheKey()}::${selectedBrokerKind()}`;
    if (state.uiCache.assetSearchKey === cacheKey && state.uiCache.assetSearchValue) {
      return state.uiCache.assetSearchValue;
    }
    const byName = new Map();
    const records = [];
    for (const asset of libraryAssets()) {
      const name = String(asset?.name || "").trim();
      if (!name) continue;
      const brokerCode = brokerInstrumentCode(name) || name;
      const compareCode = compareAssetCode(name);
      const subtitle = compareAssetSubtitle(asset);
      const classLabel = assetClassLabel(asset.assetClass);
      const mentionAliases = [];
      const seenAliases = new Set();
      for (const alias of [brokerCode, name]) {
        const compact = compactAssetToken(alias);
        if (compact && !seenAliases.has(compact)) {
          seenAliases.add(compact);
          mentionAliases.push(compact);
        }
      }
      const record = {
        asset,
        name,
        brokerCode,
        compareCode,
        subtitle,
        classLabel,
        searchHaystack: [name, brokerCode, compareCode, asset.displayName, classLabel].join(" ").toLowerCase(),
        mentionAliases,
        maxMentionAliasLength: mentionAliases.reduce((max, alias) => Math.max(max, alias.length), 0),
      };
      byName.set(name, record);
      records.push(record);
    }
    const mentionRecords = records
      .slice()
      .sort((a, b) => b.maxMentionAliasLength - a.maxMentionAliasLength || a.name.localeCompare(b.name));
    const index = { key: cacheKey, byName, records, mentionRecords };
    state.uiCache.assetSearchKey = cacheKey;
    state.uiCache.assetSearchValue = index;
    return index;
  }

  function assetSearchRecord(instrument = state.selectedInstrument) {
    return assetSearchIndex().byName.get(String(instrument || "").trim()) || null;
  }

  function catalogForInstrument(instrument = state.selectedInstrument) {
    const target = String(instrument || "").trim();
    return tradingCatalogIndex().byInstrument.get(target) || [];
  }

  function availableGranularities(instrument = state.selectedInstrument) {
    const target = String(instrument || "").trim();
    const cacheKey = `${catalogUniverseRevision}::${target}`;
    if (state.uiCache.granularitiesKey === cacheKey && Array.isArray(state.uiCache.granularitiesValue)) {
      return state.uiCache.granularitiesValue;
    }
    const catalogAsset = state.assetCatalog.find((item) => item.name === target);
    if (catalogAsset?.granularities?.length) {
      const result = catalogAsset.granularities.slice();
      state.uiCache.granularitiesKey = cacheKey;
      state.uiCache.granularitiesValue = result;
      return result;
    }
    const seen = new Set();
    const out = [];
    for (const item of catalogForInstrument(target)) {
      const granularity = String(item?.granularity || "").trim().toUpperCase();
      if (!granularity || seen.has(granularity)) continue;
      seen.add(granularity);
      out.push(granularity);
    }
    const result = out.length ? out : FULL_HISTORY_GRANULARITIES.slice();
    if (out.length) {
      result.sort((a, b) => {
        const ai = FULL_HISTORY_GRANULARITIES.indexOf(a);
        const bi = FULL_HISTORY_GRANULARITIES.indexOf(b);
        if (ai >= 0 && bi >= 0) return ai - bi;
        if (ai >= 0) return -1;
        if (bi >= 0) return 1;
        return a.localeCompare(b);
      });
    }
    state.uiCache.granularitiesKey = cacheKey;
    state.uiCache.granularitiesValue = result;
    return result;
  }

  function chartTailRows(granularity = state.selectedGranularity) {
    return CHART_TAIL_ROWS_BY_GRANULARITY[String(granularity || "").toUpperCase()] || 20000;
  }

  function previewStepMs(granularity = state.selectedGranularity) {
    const key = String(granularity || "").toUpperCase();
    if (key === "S10") return 10 * 1000;
    if (key === "S30") return 30 * 1000;
    if (key === "M1") return 60 * 1000;
    if (key === "M5") return 5 * 60 * 1000;
    if (key === "M15") return 15 * 60 * 1000;
    if (key === "M30") return 30 * 60 * 1000;
    if (key === "H1") return 60 * 60 * 1000;
    if (key === "H4") return 4 * 60 * 60 * 1000;
    if (key === "D") return 24 * 60 * 60 * 1000;
    if (key === "W") return 7 * 24 * 60 * 60 * 1000;
    return 4 * 60 * 60 * 1000;
  }

  function ensureImmediateTradingSnapshot() {
    if (state.snapshot) return;
    state.snapshot = {
      config: {
        available: false,
        source: "loading",
        message: "Connecting broker feed…",
        baseUrl: "https://api-fxpractice.oanda.com",
      },
      account: null,
      instruments: [],
      pendingOrders: [],
      openTrades: [],
      assetCatalog: [],
      historyFiles: [],
    };
    syncSnapshotCatalog(Array.isArray(state.snapshot.historyFiles) ? state.snapshot.historyFiles : [], {
      replace: true,
      assets: state.snapshot.assetCatalog,
    });
  }

  async function primeLocalAssetCatalog() {
    if (!hasTauriInvoke()) return;
    if (state.assetCatalog.length && state.catalog.length) return;
    if (state.localCatalogPromise) return state.localCatalogPromise;
    state.localCatalogPromise = (async () => {
      try {
        const response = await invoke("trading_oanda_history_catalog");
        const files = Array.isArray(response?.files) ? response.files : [];
        const assets = Array.isArray(response?.assets) ? response.assets : [];
        ensureImmediateTradingSnapshot();
        syncSnapshotCatalog(files, { replace: true, assets });
        if (state.snapshot) {
          state.snapshot.historyFiles = files.slice();
          state.snapshot.assetCatalog = state.assetCatalog.slice();
          state.snapshot.instruments = state.assetCatalog.slice();
        }
        state.uiCache.leftPanelKey = "";
        state.uiCache.headerMenuKey = "";
        renderLeftPanel();
        syncTradingHeader();
      } catch (_) {
        window.alphaTrace?.("trading.asset_catalog.error", "history catalog unavailable");
        return null;
      } finally {
        state.localCatalogPromise = null;
      }
      return state.assetCatalog;
    })();
    return state.localCatalogPromise;
  }

  function seedTradingSurfaceImmediate(options = {}) {
    ensureImmediateTradingSnapshot();
    void primeLocalAssetCatalog();
    const allowEmpty = options.allowEmpty !== false;
    const cached = getCachedSeries(state.selectedInstrument, state.selectedGranularity);
    if (cached.length) {
      state.previewSeedActive = false;
      setCanvasSeries(cached, { preserveViewport: false });
    } else if (hasTauriInvoke() && allowEmpty) {
      state.previewSeedActive = false;
      setCanvasSeries([], { preserveViewport: false });
    } else {
      if (hasTauriInvoke()) {
        state.previewSeedActive = false;
      } else {
      const immediateCandles = previewCandles(240, previewStepMs(state.selectedGranularity));
      state.previewSeedActive = true;
      setCachedSeries(state.selectedInstrument, state.selectedGranularity, immediateCandles);
      setCanvasSeries(immediateCandles, { preserveViewport: false });
      }
    }
    renderLeftPanel();
    updateOrderFormInstrument();
    canvasBridge()?.refreshRightPanel?.();
  }

  function captureUiSnapshot() {
    const bridge = canvasBridge();
    return {
      statusText: els.statusText?.textContent || "",
      pinText: els.pinDropText?.textContent || "",
      historyText: els.historyHeading?.textContent || "",
      pinnedHtml: els.pinnedList?.innerHTML || "",
      jobsHtml: els.jobList?.innerHTML || "",
      pinMenuHidden: !!els.pinMenuBtn?.hidden,
      pinPointerEvents: els.pinDrop?.style.pointerEvents || "",
      canvasState: bridge?.snapshotState?.() || null,
    };
  }

  function restoreUiSnapshot() {
    if (!state.uiSnapshot) return;
    const snapshot = state.uiSnapshot;
    if (els.statusText) els.statusText.textContent = snapshot.statusText;
    if (els.pinDropText) els.pinDropText.textContent = snapshot.pinText;
    if (els.historyHeading) els.historyHeading.textContent = snapshot.historyText;
    if (els.pinnedList) els.pinnedList.innerHTML = snapshot.pinnedHtml;
    if (els.jobList) els.jobList.innerHTML = snapshot.jobsHtml;
    if (els.pinMenuBtn) els.pinMenuBtn.hidden = snapshot.pinMenuHidden;
    if (els.pinDrop) els.pinDrop.style.pointerEvents = snapshot.pinPointerEvents;
    els.canvasWrap?.classList.remove("is-trading-mode");
    canvasBridge()?.restoreState?.(snapshot.canvasState);
    state.uiSnapshot = null;
  }

  function syncChartModeTrigger() {
    if (!els.chartModeTrigger || !chartModeTriggerDefault) return;
    const cacheKey = [
      state.active ? 1 : 0,
      state.headerMenuMode === "display" ? 1 : 0,
      state.chartDisplayMode,
    ].join("::");
    if (state.uiCache.chartModeTriggerKey === cacheKey) return;
    state.uiCache.chartModeTriggerKey = cacheKey;
    if (state.active) {
      els.chartModeTrigger.innerHTML = TRADING_CHART_MODE_TRIGGER_SVG;
      els.chartModeTrigger.setAttribute("aria-label", "Mode du graphique");
      els.chartModeTrigger.setAttribute("title", "Mode du graphique");
      els.chartModeTrigger.setAttribute("aria-haspopup", "menu");
      els.chartModeTrigger.setAttribute("aria-expanded", state.headerMenuMode === "display" ? "true" : "false");
      els.chartModeTrigger.dataset.tradingTrigger = "display-mode";
      els.chartModeTrigger.dataset.tradingToken = tokenForDisplayMode(state.chartDisplayMode);
      return;
    }
    els.chartModeTrigger.innerHTML = chartModeTriggerDefault.innerHTML;
    els.chartModeTrigger.setAttribute("aria-label", chartModeTriggerDefault.ariaLabel);
    els.chartModeTrigger.setAttribute("title", chartModeTriggerDefault.title);
    els.chartModeTrigger.removeAttribute("aria-haspopup");
    els.chartModeTrigger.removeAttribute("aria-expanded");
    delete els.chartModeTrigger.dataset.tradingTrigger;
    delete els.chartModeTrigger.dataset.tradingToken;
  }

  function isRightPanelOpen() {
    return !!els.content?.classList.contains("proof-open");
  }

  function syncProgramTrigger() {
    if (!els.programTrigger || !programTriggerDefault) return;
    const rightPanelOpen = isRightPanelOpen();
    const cacheKey = `${state.active ? 1 : 0}::${rightPanelOpen ? 1 : 0}`;
    if (state.uiCache.programTriggerKey === cacheKey) return;
    state.uiCache.programTriggerKey = cacheKey;
    if (state.active) {
      els.programTrigger.innerHTML = TRADING_RIGHT_PANEL_TRIGGER_SVG;
      els.programTrigger.setAttribute("aria-label", rightPanelOpen ? "Fermer le panel de droite" : "Ouvrir le panel de droite");
      els.programTrigger.setAttribute("title", rightPanelOpen ? "Fermer le panel de droite" : "Ouvrir le panel de droite");
      els.programTrigger.setAttribute("aria-pressed", rightPanelOpen ? "true" : "false");
      els.programTrigger.dataset.tradingTrigger = "right-panel";
      els.programTrigger.dataset.tradingToken = `<right_panel:${rightPanelOpen ? "open" : "closed"}>`;
      return;
    }
    els.programTrigger.innerHTML = programTriggerDefault.innerHTML;
    els.programTrigger.setAttribute("aria-label", programTriggerDefault.ariaLabel);
    els.programTrigger.setAttribute("title", programTriggerDefault.title);
    els.programTrigger.removeAttribute("aria-pressed");
    delete els.programTrigger.dataset.tradingTrigger;
    delete els.programTrigger.dataset.tradingToken;
  }

  function persistLlmInvolvement() {
    try {
      localStorage.setItem(TRADING_LLM_INVOLVEMENT_KEY, JSON.stringify(state.llmInvolvement));
    } catch (_) {}
  }

  function isRuntimeInvolved(runtime = "codex") {
    const key = String(runtime || "").trim().toLowerCase();
    return state.llmInvolvement[key] !== false;
  }

  function anyRuntimeInvolved() {
    return LLM_RUNTIMES.some((runtime) => isRuntimeInvolved(runtime));
  }

  function setRuntimeInvolvement(runtime = "codex", involved = true) {
    const key = String(runtime || "").trim().toLowerCase();
    if (!LLM_RUNTIMES.includes(key)) return;
    state.llmInvolvement[key] = involved !== false;
    persistLlmInvolvement();
    syncTradingChatActions();
  }

  function setAllRuntimeInvolvement(involved = true) {
    for (const runtime of LLM_RUNTIMES) {
      state.llmInvolvement[runtime] = involved !== false;
    }
    persistLlmInvolvement();
    syncTradingChatActions();
  }

  function syncRuntimeInvolvementButtons() {
    const involved = anyRuntimeInvolved();
    const cacheKey = `${state.active ? 1 : 0}::${involved ? 1 : 0}`;
    if (state.uiCache.runtimeButtonsKey === cacheKey) return;
    state.uiCache.runtimeButtonsKey = cacheKey;
    if (els.llmModeToggle) {
      els.llmModeToggle.hidden = !state.active;
      els.llmModeToggle.setAttribute("aria-pressed", involved ? "true" : "false");
      els.llmModeToggle.dataset.state = involved ? "on" : "off";
    }
    try {
      window.__forgeSyncTradingChatInvolvementControls?.();
    } catch (_) {}
  }

  function normalizeTradingChatSubbarMode(mode = "none") {
    const normalized = String(mode || "none").trim().toLowerCase();
    return normalized === "indicators" || normalized === "replay" || normalized === "alert"
      ? normalized
      : "none";
  }

  function setTradingChatSubbarMode(mode = "none", options = {}) {
    const nextMode = state.active ? normalizeTradingChatSubbarMode(mode) : "none";
    if (!options.force && state.chatSubbarMode === nextMode) return;
    state.chatSubbarMode = nextMode;
    if (nextMode !== "indicators") state.chatSubbarExpanded = false;
    if (nextMode === "indicators") state.chatSubbarSection = normalizeTradingSubbarSection(state.chatSubbarSection);
    state.indicatorsModeActive = nextMode === "indicators";
    state.replayModeActive = nextMode === "replay";
    if (nextMode !== "alert" && state.alertModalOpen) closeAlertModal({ preserveSubbar: true });
    renderTradingChatSubbar();
    syncTradingChatActions();
  }

  function toggleTradingChatSubbarMode(mode = "none") {
    const normalized = normalizeTradingChatSubbarMode(mode);
    setTradingChatSubbarMode(state.chatSubbarMode === normalized ? "none" : normalized);
  }

  function renderTradingChatSubbar() {
    if (!els.tradingSubbar) return;
    const mode = state.active ? normalizeTradingChatSubbarMode(state.chatSubbarMode) : "none";
    if (mode === "none") {
      if (state.uiCache.tradingSubbarKey === "none" && els.tradingSubbar.hidden) return;
      els.tradingSubbar.hidden = true;
      els.tradingSubbar.removeAttribute("data-mode");
      els.tradingSubbar.classList.remove("is-expanded");
      els.tradingSubbar.innerHTML = "";
      state.uiCache.tradingSubbarKey = "none";
      state.uiCache.tradingSubbarMarkup = "";
      return;
    }
    const renderKey = tradingSubbarRenderKey(mode);
    els.tradingSubbar.hidden = false;
    els.tradingSubbar.dataset.mode = mode;
    els.tradingSubbar.classList.toggle("is-expanded", mode === "indicators" && !!state.chatSubbarExpanded);
    if (state.uiCache.tradingSubbarKey === renderKey) return;
    const markup = tradingSubbarMarkup(mode);
    state.uiCache.tradingSubbarKey = renderKey;
    state.uiCache.tradingSubbarMarkup = markup;
    els.tradingSubbar.innerHTML = markup;
  }

  function normalizeTradingSubbarSection(section = "indicators") {
    const normalized = String(section || "indicators").trim().toLowerCase();
    return TRADING_INDICATOR_SECTIONS.includes(normalized)
      ? normalized
      : "indicators";
  }

  function favoriteIndicators() {
    if (Array.isArray(state.uiCache.favoriteIndicatorsValue)) return state.uiCache.favoriteIndicatorsValue;
    state.uiCache.favoriteIndicatorsValue = tradingIndicatorCatalog().filter((entry) => entry.favorites);
    return state.uiCache.favoriteIndicatorsValue;
  }

  function indicatorLibraryForSection(section = "indicators") {
    if (section === "favorites") return favoriteIndicators();
    if (section === "indicators") return tradingIndicatorCatalog();
    if (section === "create") {
      if (!Array.isArray(state.uiCache.createLibraryValue)) {
        state.uiCache.createLibraryValue = TRADING_CREATE_LIBRARY.slice();
      }
      return state.uiCache.createLibraryValue;
    }
    if (section === "strategies") {
      if (!Array.isArray(state.uiCache.strategyLibraryValue)) {
        state.uiCache.strategyLibraryValue = TRADING_CREATE_LIBRARY.filter((entry) => entry.family === "strategy");
      }
      return state.uiCache.strategyLibraryValue;
    }
    return [];
  }

  function tradingSubbarIcon(kind = "") {
    if (kind === "bookmark") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M6 4.75h8v10.5l-4-2.55-4 2.55z" /></svg>`;
    if (kind === "spark") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="m10 3.6 1.4 4.1 4.1 1.4-4.1 1.4-1.4 4.1-1.4-4.1-4.1-1.4 4.1-1.4z" /></svg>`;
    if (kind === "settings") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M4.5 6.2h6" /><path d="M13.2 6.2h2.3" /><path d="M10.5 6.2a1.4 1.4 0 1 0 0 .01" /><path d="M4.5 10h2.3" /><path d="M9.5 10h6" /><path d="M7.7 10a1.4 1.4 0 1 0 0 .01" /><path d="M4.5 13.8h6" /><path d="M13.2 13.8h2.3" /><path d="M10.5 13.8a1.4 1.4 0 1 0 0 .01" /></svg>`;
    if (kind === "expand") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M8.25 6.5h-2.5V9" /><path d="M11.75 6.5h2.5V9" /><path d="M8.25 13.5h-2.5V11" /><path d="M11.75 13.5h2.5V11" /></svg>`;
    if (kind === "collapse") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="M7.25 4.75H4.75V7.25" /><path d="M12.75 4.75h2.5V7.25" /><path d="M7.25 15.25H4.75v-2.5" /><path d="M12.75 15.25h2.5v-2.5" /></svg>`;
    if (kind === "close") return `<svg viewBox="0 0 20 20" focusable="false" aria-hidden="true"><path d="m6 6 8 8" /><path d="m14 6-8 8" /></svg>`;
    return "";
  }

  function tradingSubbarMarkup(mode = state.chatSubbarMode) {
    if (mode === "none") return "";
    if (mode !== "indicators") return `<div class="canvas-chat-trading-subbar-surface"></div>`;
    const activeSection = normalizeTradingSubbarSection(state.chatSubbarSection);
    const attachedIndicators = activeIndicatorIdSet();
    const sections = [
      { key: "favorites", label: "Favorites", icon: "bookmark" },
      { key: "indicators", label: "Indicators" },
      { key: "strategies", label: "Strategies" },
      { key: "profile", label: "Profile" },
      { key: "patterns", label: "Patterns" },
      { key: "create", label: "create_", icon: "spark", create: true },
    ];
    const nav = sections.map((entry) => `
      <button
        type="button"
        class="canvas-chat-trading-subbar-link${entry.key === activeSection ? " is-active" : ""}${entry.create ? " is-create" : ""}"
        data-subbar-section="${entry.key}"
        aria-pressed="${entry.key === activeSection ? "true" : "false"}"
      >
        ${entry.icon ? `<span class="canvas-chat-trading-subbar-link-icon" aria-hidden="true">${tradingSubbarIcon(entry.icon)}</span>` : ""}
        <span class="canvas-chat-trading-subbar-link-label">${escapeHtml(entry.label)}</span>
      </button>
    `).join("");
    const library = indicatorLibraryForSection(activeSection);
    const body = activeSection === "favorites" || activeSection === "indicators"
      ? `
        <div class="canvas-chat-trading-subbar-grid">
          ${library.map((indicator) => `
            <button
              type="button"
              class="canvas-chat-trading-subbar-grid-item${attachedIndicators.has(indicator.id) ? " is-attached" : ""}"
              data-indicator-pick="${escapeHtml(indicator.id)}"
            >
              <span class="canvas-chat-trading-subbar-grid-command">${escapeHtml(indicator.command)}</span>
            </button>
          `).join("")}
        </div>
      `
      : activeSection === "create" || activeSection === "strategies"
        ? `
        <div class="canvas-chat-trading-subbar-grid">
          ${library.map((entry) => `
            <button
              type="button"
              class="canvas-chat-trading-subbar-grid-item is-program"
              data-slash-pick="${escapeHtml(entry.command)}"
            >
              <span class="canvas-chat-trading-subbar-grid-command">${escapeHtml(entry.command)}</span>
            </button>
          `).join("")}
        </div>
      `
      : `<div class="canvas-chat-trading-subbar-empty"></div>`;
    return `
        <div class="canvas-chat-trading-subbar-surface">
          <div class="canvas-chat-trading-subbar-head">
            <div class="canvas-chat-trading-subbar-nav">${nav}</div>
            <div class="canvas-chat-trading-subbar-tools">
              <button type="button" class="canvas-chat-trading-subbar-tool" data-subbar-action="toggle-expand" aria-label="${state.chatSubbarExpanded ? "Collapse panel" : "Expand panel"}">${tradingSubbarIcon(state.chatSubbarExpanded ? "collapse" : "expand")}</button>
              <button type="button" class="canvas-chat-trading-subbar-tool" data-subbar-action="close" aria-label="Close panel">${tradingSubbarIcon("close")}</button>
            </div>
          </div>
          <div class="canvas-chat-trading-subbar-body">${body}</div>
      </div>
    `;
  }

  function tradingSubbarRenderKey(mode = state.chatSubbarMode) {
    if (mode === "none") return "none";
    if (mode !== "indicators") return `${mode}::surface`;
    return [
      mode,
      normalizeTradingSubbarSection(state.chatSubbarSection),
      state.chatSubbarExpanded ? 1 : 0,
      activeIndicatorRevision,
      activeIndicatorIdSignature(),
    ].join("::");
  }

  function syncTradingChatActions() {
    const active = !!state.active;
    const involved = anyRuntimeInvolved();
    const selectionEnabled = active && !!window.__forgeAlphaCanvasBridge?.isTradingSelectionMode?.();
    const cacheKey = [
      active ? 1 : 0,
      involved ? 1 : 0,
      selectionEnabled ? 1 : 0,
      state.chatSubbarMode,
    ].join("::");
    renderTradingChatSubbar();
    if (state.uiCache.chatActionsKey === cacheKey) return;
    state.uiCache.chatActionsKey = cacheKey;
    if (els.chatRoot) els.chatRoot.classList.toggle("is-trading-mode", !!state.active);
    if (els.chatRoot) els.chatRoot.classList.toggle("is-trading-command-mode", !!state.active && !involved);
    if (els.chatTradingActions) els.chatTradingActions.hidden = !state.active;
    if (els.planetTrigger) els.planetTrigger.hidden = !!state.active;
    if (els.selectionTrigger) {
      els.selectionTrigger.setAttribute("aria-pressed", state.active && selectionEnabled ? "true" : "false");
      els.selectionTrigger.hidden = !state.active;
    }
    if (els.indicatorsTrigger) {
      els.indicatorsTrigger.setAttribute("aria-pressed", state.active && state.chatSubbarMode === "indicators" ? "true" : "false");
      els.indicatorsTrigger.hidden = !state.active;
    }
    if (els.replayTrigger) {
      els.replayTrigger.setAttribute("aria-pressed", state.active && state.chatSubbarMode === "replay" ? "true" : "false");
      els.replayTrigger.hidden = !state.active;
    }
    if (els.alertTrigger) {
      els.alertTrigger.setAttribute("aria-pressed", state.active && state.chatSubbarMode === "alert" ? "true" : "false");
      els.alertTrigger.hidden = !state.active;
    }
    syncRuntimeInvolvementButtons();
  }

  function syncAlertTrigger() {
    if (els.alertTrigger) {
      if (state.uiCache.alertTriggerKey !== "static-alert-trigger") {
        state.uiCache.alertTriggerKey = "static-alert-trigger";
        els.alertTrigger.setAttribute("aria-label", "Open alert tools");
        els.alertTrigger.setAttribute("title", "Alert");
        els.alertTrigger.dataset.tradingTrigger = "alerts";
      }
    }
    syncTradingChatActions();
  }

  function syncSplitTriggerLabel() {
    if (!els.content) return;
    const splitActive = !!els.content.classList.contains("has-split-view");
    if (!state.active) return;
    els.compareTrigger?.setAttribute("data-trading-split", splitActive ? "true" : "false");
  }

  function syncCanvasAlerts() {
    const cacheKey = `${alertUniverseRevision}::${state.selectedInstrument}::canvas-alerts`;
    let visibleAlerts = state.uiCache.canvasAlertsKey === cacheKey
      && Array.isArray(state.uiCache.canvasAlertsValue)
      ? state.uiCache.canvasAlertsValue
      : null;
    if (!visibleAlerts) {
      visibleAlerts = [];
      for (const alert of state.alerts) {
        if (
          alert.instrument === state.selectedInstrument
          && (alert.active || alert.triggeredCount > 0)
          && Number.isFinite(alert.targetValue)
        ) {
          visibleAlerts.push(mapNormalizedAlertForCanvas(alert));
        }
      }
      state.uiCache.canvasAlertsKey = cacheKey;
      state.uiCache.canvasAlertsValue = visibleAlerts;
    }
    canvasBridge()?.setTradingAlerts?.({ alerts: visibleAlerts });
  }

  function normalizedStrategyVisualProbes(probes = []) {
    if (!Array.isArray(probes)) return [];
    return probes
      .map((probe) => ({
        entryTime: String(probe?.entryTime || probe?.entry_time || "").trim(),
        exitTime: String(probe?.exitTime || probe?.exit_time || "").trim(),
        entryIndex: Number(probe?.entryIndex ?? probe?.entry_index),
        entryPrice: Number(probe?.entryPrice ?? probe?.entry_price),
        direction: String(probe?.direction || "").trim().toLowerCase(),
        stopPrice: Number(probe?.stopPrice ?? probe?.stop_price),
        takeProfitPrice: Number(probe?.takeProfitPrice ?? probe?.take_profit_price),
        stopLossDistance: Number(probe?.stopLossDistance ?? probe?.stop_loss_distance),
        takeProfitDistance: Number(probe?.takeProfitDistance ?? probe?.take_profit_distance),
        pnlDistance: Number(probe?.pnlDistance ?? probe?.pnl_distance),
        outcome: String(probe?.outcome || "").trim().toLowerCase(),
        heldBars: Number(probe?.heldBars ?? probe?.held_bars),
      }))
      .filter((probe) => (
        probe.entryTime
        && (probe.direction === "long" || probe.direction === "short")
        && Number.isFinite(probe.entryPrice)
        && Number.isFinite(probe.stopPrice)
        && Number.isFinite(probe.takeProfitPrice)
      ));
  }

  function syncStrategyOverlayToCanvas(strategy = null) {
    const next = strategy || summarizeStrategyLabState();
    const probes = normalizedStrategyVisualProbes(next.visualProbes || next.backtest?.visualProbes || []);
    const active = !!next.active && probes.length > 0;
    const bridge = canvasBridge();
    if (!bridge?.setTradingStrategyOverlay) {
      state.uiCache.strategyOverlayKey = "";
      return;
    }
    const last = probes[probes.length - 1] || {};
    const cacheKey = [
      active ? 1 : 0,
      state.selectedInstrument,
      state.selectedGranularity,
      probes.length,
      last.entryTime || "",
      last.direction || "",
      last.entryPrice || "",
      next.backtest?.direction || "",
      next.backtest?.takeProfitDistance ?? "",
    ].join("|");
    if (state.uiCache.strategyOverlayKey === cacheKey) return;
    state.uiCache.strategyOverlayKey = cacheKey;
    bridge.setTradingStrategyOverlay({
      active,
      probes,
      spec: next.spec || {},
      backtest: next.backtest || {},
      pairedProbe: next.pairedProbe || null,
      computePlan: next.computePlan || null,
    });
  }

  function escapeHtml(value) {
    return String(value || "").replace(/[&<>\"']/g, (char) => ({
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      "\"": "&quot;",
      "'": "&#39;",
    })[char] || char);
  }

  function currentInstrumentAlerts() {
    const cacheKey = `${alertUniverseRevision}::${state.selectedInstrument}::instrument-alerts`;
    if (
      state.uiCache.instrumentAlertsKey === cacheKey
      && Array.isArray(state.uiCache.instrumentAlertsValue)
    ) {
      return state.uiCache.instrumentAlertsValue;
    }
    const alerts = state.alerts
      .filter((alert) => alert.instrument === state.selectedInstrument)
      .sort((a, b) => {
        if ((a.active ? 1 : 0) !== (b.active ? 1 : 0)) return (b.active ? 1 : 0) - (a.active ? 1 : 0);
        return (b.updatedAtMs || b.lastTriggeredAtMs || 0) - (a.updatedAtMs || a.lastTriggeredAtMs || 0);
      });
    state.uiCache.instrumentAlertsKey = cacheKey;
    state.uiCache.instrumentAlertsValue = alerts;
    return alerts;
  }

  function alertDraftRenderKey(draft = state.alertDraft || makeAlertDraft()) {
    const notifications = draft.notifications || {};
    return [
      draft.id || "",
      draft.operator || "",
      draft.targetValue || "",
      draft.triggerMode || "",
      draft.expirationTimeMs || "",
      draft.message || "",
      notifications.app ? 1 : 0,
      notifications.toast ? 1 : 0,
      notifications.email ? 1 : 0,
      notifications.sound ? 1 : 0,
      notifications.emailTo || "",
      notifications.soundProfile || "",
      notifications.soundVolume || "",
      notifications.soundRepeat || "",
    ].join("\u001f");
  }

  function ensureAlertUi() {
    if (alertUi.overlay || !els.canvasWrap) return;
    const overlay = document.createElement("div");
    overlay.className = "trading-alert-overlay";
    overlay.hidden = true;
    overlay.innerHTML = `
      <div class="trading-alert-backdrop" data-alert-action="close"></div>
      <section class="trading-alert-modal" role="dialog" aria-modal="true" aria-label="Alertes trading"></section>
    `;
    els.canvasWrap.appendChild(overlay);
    const toastRack = document.createElement("div");
    toastRack.className = "trading-alert-toast-rack";
    els.canvasWrap.appendChild(toastRack);
    alertUi.overlay = overlay;
    alertUi.modal = overlay.querySelector(".trading-alert-modal");
    alertUi.toastRack = toastRack;

    overlay.addEventListener("click", (event) => {
      const actionNode = event.target?.closest?.("[data-alert-action]");
      const action = actionNode?.dataset?.alertAction || "";
      if (!action) return;
      event.preventDefault();
      if (action === "close" || action === "cancel") {
        closeAlertModal();
      } else if (action === "new") {
        state.alertFormMode = "create";
        state.alertDraft = makeAlertDraft();
        renderAlertModal();
      } else if (action === "edit") {
        const record = state.alerts.find((item) => String(item?.id || "") === String(actionNode.dataset.alertId || ""));
        if (!record) return;
        state.alertFormMode = "edit";
        state.alertDraft = makeAlertDraft(normalizeAlertRecord(record));
        renderAlertModal();
      } else if (action === "delete") {
        void deleteAlertRecord(actionNode.dataset.alertId || "");
      } else if (action === "save") {
        void saveAlertDraft();
      }
    });

    overlay.addEventListener("input", (event) => {
      if (!state.alertDraft) return;
      const target = event.target;
      if (!target) return;
      const field = target.dataset?.alertField || "";
      const flag = target.dataset?.alertFlag || "";
      if (field === "targetValue") {
        state.alertDraft.targetValue = Number(target.value);
      } else if (field === "message") {
        state.alertDraft.message = String(target.value || "");
      } else if (field === "emailTo") {
        state.alertDraft.notifications.emailTo = String(target.value || "");
      } else if (field === "soundVolume") {
        state.alertDraft.notifications.soundVolume = clamp(Number(target.value) / 100, 0, 1);
      } else if (flag) {
        state.alertDraft.notifications[flag] = !!target.checked;
      }
    });

    overlay.addEventListener("change", (event) => {
      if (!state.alertDraft) return;
      const target = event.target;
      if (!target) return;
      const field = target.dataset?.alertField || "";
      if (field === "operator") {
        state.alertDraft.operator = String(target.value || "crossing");
        if (!String(state.alertDraft.message || "").trim()) state.alertDraft.message = buildAlertMessage(state.alertDraft);
      } else if (field === "triggerMode") {
        state.alertDraft.triggerMode = String(target.value || "once");
      } else if (field === "expirationTimeMs") {
        state.alertDraft.expirationTimeMs = fromDateTimeLocalValue(target.value);
      } else if (field === "soundProfile") {
        state.alertDraft.notifications.soundProfile = String(target.value || "soft");
      } else if (field === "soundRepeat") {
        state.alertDraft.notifications.soundRepeat = Math.max(1, Math.min(4, Number(target.value) || 1));
      }
      renderAlertModal();
    });
  }

  function renderAlertModal() {
    ensureAlertUi();
    if (!alertUi.modal) return;
    if (!state.alertModalOpen) {
      alertUi.overlay.hidden = true;
      state.uiCache.alertModalKey = "";
      return;
    }
    const draft = state.alertDraft || makeAlertDraft();
    const alerts = currentInstrumentAlerts();
    const modalKey = [
      alertUniverseRevision,
      state.selectedInstrument,
      state.selectedGranularity,
      state.alertFormMode,
      alertDraftRenderKey(draft),
      alerts.length,
    ].join("\u001e");
    if (state.uiCache.alertModalKey === modalKey) {
      alertUi.overlay.hidden = false;
      syncAlertTrigger();
      return;
    }
    state.uiCache.alertModalKey = modalKey;
    const alertRows = alerts.length
      ? alerts.map((alert) => `
        <article class="trading-alert-row${alert.active ? "" : " is-muted"}">
          <div class="trading-alert-row-main">
            <strong>${escapeHtml(alert.message || buildAlertMessage(alert))}</strong>
            <span>${escapeHtml(alert.operator)} · ${escapeHtml(formatNumber(alert.targetValue, 3))}${alert.expirationTimeMs ? ` · expire ${escapeHtml(new Date(alert.expirationTimeMs).toLocaleString())}` : ""}</span>
          </div>
          <div class="trading-alert-row-actions">
            <button type="button" class="trading-alert-row-btn" data-alert-action="edit" data-alert-id="${escapeHtml(alert.id)}">Modifier</button>
            <button type="button" class="trading-alert-row-btn" data-alert-action="delete" data-alert-id="${escapeHtml(alert.id)}">Supprimer</button>
          </div>
        </article>
      `).join("")
      : `<p class="trading-alert-empty">Aucune alerte active sur ${escapeHtml(state.selectedInstrument)}.</p>`;
    alertUi.modal.innerHTML = `
      <header class="trading-alert-header">
        <div>
          <h3>Alertes sur ${escapeHtml(state.selectedInstrument)}</h3>
          <p>${state.alertFormMode === "edit" ? "Modifier une alerte" : "Créer une alerte sonore propre et discrète"}</p>
        </div>
        <button type="button" class="trading-alert-close" data-alert-action="close" aria-label="Fermer">&times;</button>
      </header>
      <section class="trading-alert-grid">
        <label>
          <span>Condition</span>
          <select data-alert-field="operator">
            <option value="crossing"${draft.operator === "crossing" ? " selected" : ""}>Croisement</option>
            <option value="crossing_up"${draft.operator === "crossing_up" ? " selected" : ""}>Croisement haussier</option>
            <option value="crossing_down"${draft.operator === "crossing_down" ? " selected" : ""}>Croisement baissier</option>
            <option value="above"${draft.operator === "above" ? " selected" : ""}>Au-dessus</option>
            <option value="below"${draft.operator === "below" ? " selected" : ""}>En dessous</option>
          </select>
        </label>
        <label>
          <span>Valeur</span>
          <input data-alert-field="targetValue" type="number" step="0.001" value="${escapeHtml(formatNumber(draft.targetValue, 3))}">
        </label>
        <label>
          <span>Déclenchement</span>
          <select data-alert-field="triggerMode">
            <option value="once"${draft.triggerMode === "once" ? " selected" : ""}>Une fois seulement</option>
            <option value="repeat"${draft.triggerMode === "repeat" ? " selected" : ""}>À chaque nouveau croisement</option>
          </select>
        </label>
        <label>
          <span>Expiration</span>
          <input data-alert-field="expirationTimeMs" type="datetime-local" value="${escapeHtml(toDateTimeLocalValue(draft.expirationTimeMs))}">
        </label>
        <label class="trading-alert-span-2">
          <span>Message</span>
          <input data-alert-field="message" type="text" value="${escapeHtml(draft.message)}" placeholder="Texte affiché dans les notifications">
        </label>
      </section>
      <section class="trading-alert-notifications">
        <div class="trading-alert-subtitle">Notifications</div>
        <label><input data-alert-flag="app" type="checkbox"${draft.notifications.app ? " checked" : ""}> App</label>
        <label><input data-alert-flag="toast" type="checkbox"${draft.notifications.toast ? " checked" : ""}> Toasts</label>
        <label><input data-alert-flag="email" type="checkbox"${draft.notifications.email ? " checked" : ""}> Email</label>
        <label><input data-alert-flag="sound" type="checkbox"${draft.notifications.sound ? " checked" : ""}> Son</label>
      </section>
      <section class="trading-alert-grid trading-alert-grid-compact">
        <label class="${draft.notifications.email ? "" : " is-disabled"}">
          <span>Email</span>
          <input data-alert-field="emailTo" type="email" value="${escapeHtml(draft.notifications.emailTo || "")}" placeholder="destinataire@exemple.com"${draft.notifications.email ? "" : " disabled"}>
        </label>
        <label class="${draft.notifications.sound ? "" : " is-disabled"}">
          <span>Profil sonore</span>
          <select data-alert-field="soundProfile"${draft.notifications.sound ? "" : " disabled"}>
            <option value="soft"${draft.notifications.soundProfile === "soft" ? " selected" : ""}>Soft</option>
            <option value="bell"${draft.notifications.soundProfile === "bell" ? " selected" : ""}>Bell</option>
            <option value="pulse"${draft.notifications.soundProfile === "pulse" ? " selected" : ""}>Pulse</option>
          </select>
        </label>
        <label class="${draft.notifications.sound ? "" : " is-disabled"}">
          <span>Volume</span>
          <input data-alert-field="soundVolume" type="range" min="20" max="100" value="${escapeHtml(String(Math.round(clamp(Number(draft.notifications.soundVolume || 0.82), 0.2, 1) * 100)))}"${draft.notifications.sound ? "" : " disabled"}>
        </label>
        <label class="${draft.notifications.sound ? "" : " is-disabled"}">
          <span>Répétitions</span>
          <select data-alert-field="soundRepeat"${draft.notifications.sound ? "" : " disabled"}>
            <option value="1"${Number(draft.notifications.soundRepeat) === 1 ? " selected" : ""}>1</option>
            <option value="2"${Number(draft.notifications.soundRepeat) === 2 ? " selected" : ""}>2</option>
            <option value="3"${Number(draft.notifications.soundRepeat) === 3 ? " selected" : ""}>3</option>
            <option value="4"${Number(draft.notifications.soundRepeat) === 4 ? " selected" : ""}>4</option>
          </select>
        </label>
      </section>
      <section class="trading-alert-library">
        <div class="trading-alert-library-head">
          <div class="trading-alert-subtitle">Alertes existantes</div>
          <button type="button" class="trading-alert-secondary" data-alert-action="new">Nouvelle alerte</button>
        </div>
        <div class="trading-alert-list">${alertRows}</div>
      </section>
      <footer class="trading-alert-footer">
        <button type="button" class="trading-alert-secondary" data-alert-action="cancel">Annuler</button>
        <button type="button" class="trading-alert-primary" data-alert-action="save">${state.alertFormMode === "edit" ? "Mettre à jour" : "Créer"}</button>
      </footer>
    `;
    alertUi.modal.querySelector('[data-alert-action="close"]')?.setAttribute("data-trading-token", "<alert:close>");
    alertUi.modal.querySelector('[data-alert-action="new"]')?.setAttribute("data-trading-token", "<alert:new>");
    alertUi.modal.querySelector('[data-alert-action="cancel"]')?.setAttribute("data-trading-token", "<alert:cancel>");
    alertUi.modal.querySelector('[data-alert-action="save"]')?.setAttribute("data-trading-token", "<alert:save>");
    alertUi.modal.querySelectorAll("[data-alert-id]").forEach((node) => {
      const action = String(node.getAttribute("data-alert-action") || "");
      const id = String(node.getAttribute("data-alert-id") || "");
      node.setAttribute("data-trading-token", `<alert:id=${id} action=${action || "select"}>`);
    });
    alertUi.modal.querySelectorAll("[data-alert-field]").forEach((node) => {
      const field = String(node.getAttribute("data-alert-field") || "");
      node.setAttribute("data-trading-token", tokenForAlertField(field, draft));
    });
    alertUi.modal.querySelectorAll("[data-alert-flag]").forEach((node) => {
      const flag = String(node.getAttribute("data-alert-flag") || "");
      node.setAttribute("data-trading-token", `<alert:notify=${flag}>`);
    });
    alertUi.overlay.hidden = false;
    syncAlertTrigger();
  }

  function openAlertModal(draft = null) {
    closeHeaderMenu();
    state.chatSubbarMode = "alert";
    state.alertModalOpen = true;
    state.uiCache.alertModalKey = "";
    state.alertDraft = draft
      ? makeAlertDraft(draft)
      : makeAlertDraft({
        ...(state.alertDraft || {}),
        instrument: state.selectedInstrument,
        granularity: state.selectedGranularity,
      });
    renderAlertModal();
    void reloadAlerts({ render: true });
  }

  function closeAlertModal(options = {}) {
    state.alertModalOpen = false;
    state.uiCache.alertModalKey = "";
    if (alertUi.overlay) alertUi.overlay.hidden = true;
    if (!options.preserveSubbar && state.chatSubbarMode === "alert") {
      state.chatSubbarMode = "none";
    }
    syncAlertTrigger();
  }

  function playAlertEventSound(event) {
    const notifications = event?.notifications && typeof event.notifications === "object"
      ? event.notifications
      : defaultAlertNotifications();
    if (!notifications.sound) return;
    const soundMap = {
      soft: "alert-soft",
      bell: "alert-bell",
      pulse: "alert-pulse",
    };
    const soundKind = soundMap[String(notifications.soundProfile || "soft")] || "alert-soft";
    const repeat = Math.max(1, Math.min(4, Number(notifications.soundRepeat) || 1));
    for (let index = 0; index < repeat; index += 1) {
      window.setTimeout(() => {
        try { window.__forgePlayUiSound?.(soundKind); } catch (_) {}
      }, index * 420);
    }
  }

  function showAlertToast(event) {
    ensureAlertUi();
    if (!alertUi.toastRack) return;
    const toast = document.createElement("div");
    toast.className = "trading-alert-toast";
    toast.innerHTML = `
      <strong>${escapeHtml(event?.instrument || state.selectedInstrument)}</strong>
      <span>${escapeHtml(event?.message || "Alerte déclenchée")}</span>
    `;
    alertUi.toastRack.appendChild(toast);
    window.setTimeout(() => {
      toast.classList.add("is-leaving");
      window.setTimeout(() => toast.remove(), 280);
    }, 4200);
  }

  function showSystemAlert(event) {
    const notifications = event?.notifications && typeof event.notifications === "object"
      ? event.notifications
      : defaultAlertNotifications();
    if (!notifications.app || typeof Notification === "undefined") return;
    const body = String(event?.message || "Alerte trading");
    const title = `${event?.instrument || state.selectedInstrument} · ${formatNumber(event?.price, 3)}`;
    if (Notification.permission === "granted") {
      try { new Notification(title, { body }); } catch (_) {}
      return;
    }
    if (Notification.permission === "default") {
      Notification.requestPermission().then((permission) => {
        if (permission === "granted") {
          try { new Notification(title, { body }); } catch (_) {}
        }
      }).catch(() => {});
    }
  }

  function handleAlertEvents(events = []) {
    for (const rawEvent of events) {
      const event = rawEvent && typeof rawEvent === "object" ? rawEvent : null;
      if (!event) continue;
      const key = `${event.id || event.alertId || "alert"}:${event.triggeredAtMs || 0}`;
      if (seenAlertEventKeys.has(key)) continue;
      seenAlertEventKeys.add(key);
      const notifications = event.notifications && typeof event.notifications === "object"
        ? event.notifications
        : defaultAlertNotifications();
      if (notifications.toast) showAlertToast(event);
      showSystemAlert(event);
      playAlertEventSound(event);
    }
  }

  function setCanvasSeries(candles, options = {}) {
    state.candles = Array.isArray(candles) ? candles : [];
    const nextSeriesKey = `${state.selectedInstrument}::${state.selectedGranularity}`;
    const preserveViewport = options.preserveViewport === false
      ? false
      : (
        state.renderedSeriesKey === nextSeriesKey
        && Array.isArray(state.candles)
        && state.candles.length > 0
      );
    canvasBridge()?.setDocument?.({
      candles: state.candles,
      fileName: state.candles.length ? `${state.selectedInstrument}_${state.selectedGranularity}.csv` : "",
      fileSize: 0,
      previewKind: "CSV",
      previewRows: [],
      tradingInstrument: state.selectedInstrument,
      tradingGranularity: state.selectedGranularity,
      chartDisplayMode: state.chartDisplayMode,
      preserveViewport,
    });
    state.renderedSeriesKey = nextSeriesKey;
    syncCanvasAlerts();
    syncStrategyOverlayToCanvas();
    refreshStrategyLiveTest("series");
    void refreshStrategyLiveJob("series");
    syncTradingHeader();
  }

  function closeHeaderMenu() {
    if (!els.compareMenu) return;
    if (state.headerMenuHideTimer) {
      window.clearTimeout(state.headerMenuHideTimer);
      state.headerMenuHideTimer = 0;
    }
    if (els.compareSearchWrap) els.compareSearchWrap.hidden = true;
    state.compareScrollTop = 0;
    state.headerMenuMode = "none";
    state.headerMenuTriggerEl = null;
    state.uiCache.headerMenuKey = "";
    state.uiCache.chartModeTriggerKey = "";
    syncCompareShellState();
    els.compareMenu.classList.remove("is-open");
    state.headerMenuHideTimer = window.setTimeout(() => {
      els.compareMenu.hidden = true;
      els.compareMenu.dataset.mode = "none";
      setDropdownScrim(false);
      syncCompareShellState();
      state.headerMenuHideTimer = 0;
    }, 180);
    els.compareTrigger?.setAttribute("aria-expanded", "false");
    els.addTrigger?.setAttribute("aria-expanded", "false");
    els.chartModeTrigger?.setAttribute("aria-expanded", "false");
    syncChartModeTrigger();
  }

  function renderComparisonMenu() {
    if (!els.compareMenu) return;
    const assets = availableAssets().filter((item) => item.name !== state.selectedInstrument);
    els.compareMenu.innerHTML = "";
    const noneButton = document.createElement("button");
    noneButton.type = "button";
    noneButton.className = `trading-compare-menu-item${!state.compareInstruments.length ? " is-active" : ""}`;
    noneButton.dataset.compareInstrument = "";
    noneButton.innerHTML = `
      <span class="trading-compare-menu-check">${!state.compareInstruments.length ? "✓" : ""}</span>
      <span class="trading-compare-menu-copy">
        <span class="trading-compare-menu-title">No comparison</span>
        <span class="trading-compare-menu-subtitle">Show only the main broker asset</span>
      </span>
    `;
    els.compareMenu.appendChild(noneButton);
    for (const asset of assets) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `trading-compare-menu-item${state.compareInstruments.includes(asset.name) ? " is-active" : ""}`;
      button.dataset.compareInstrument = asset.name;
      button.innerHTML = `
        <span class="trading-compare-menu-check">${state.compareInstruments.includes(asset.name) ? "✓" : ""}</span>
        <span class="trading-compare-menu-copy">
          <span class="trading-compare-menu-title">${brokerInstrumentCode(asset.name) || asset.name}</span>
          <span class="trading-compare-menu-subtitle">${asset.displayName || asset.assetClass || asset.name}</span>
        </span>
      `;
      els.compareMenu.appendChild(button);
    }
  }

  function openComparisonMenu() {
    if (!els.compareTrigger || !els.compareMenu) return;
    renderComparisonMenu();
    const rect = els.compareTrigger.getBoundingClientRect();
    els.compareMenu.hidden = false;
    els.compareMenu.style.left = `${Math.max(12, rect.left)}px`;
    els.compareMenu.style.top = `${Math.min(window.innerHeight - 24, rect.bottom + 8)}px`;
    els.compareTrigger.setAttribute("aria-expanded", "true");
  }

  function assetClassLabel(assetClass) {
    return ASSET_CLASS_LABELS[String(assetClass || "").trim().toLowerCase()] || "Other";
  }

  function advancedCompareMenuModel(mode = "compare") {
    const index = assetSearchIndex();
    const query = String(state.compareSearch || "").trim().toLowerCase();
    const brokerLogoKind = selectedBrokerLogoKind();
    const brokerLabel = selectedBrokerLabel();
    const menuMode = mode === "add" ? "add" : "compare";
    const addInstruments = addedChartInstruments();
    const activeInstrumentSet = menuMode === "add"
      ? new Set(addInstruments)
      : new Set(state.compareInstruments);
    const modelKey = JSON.stringify({
      mode: menuMode,
      assetSearch: index.key,
      query,
      selected: state.selectedInstrument,
      compare: state.compareInstruments.join("|"),
      add: addInstruments.join("|"),
      brokerLogoKind,
      brokerLabel,
    });
    if (state.uiCache.compareMenuModelKey === modelKey && state.uiCache.compareMenuModelValue) {
      return state.uiCache.compareMenuModelValue;
    }
    const preferredGroupOrder = ["commodity", "forex", "crypto", "index", "equity", "bond", "instrument", "other"];
    const brokerInstrumentNames = activeBrokerInstrumentSet();
    const groups = [];
    const groupMap = new Map();
    for (const record of index.records) {
      const asset = record.asset;
      if (!asset || asset.name === state.selectedInstrument) continue;
      if (query && !record.searchHaystack.includes(query)) continue;
      const key = String(asset.assetClass || "other").toLowerCase();
      if (!groupMap.has(key)) groupMap.set(key, []);
      groupMap.get(key).push(record);
    }
    const orderedKeys = [
      ...preferredGroupOrder.filter((key) => groupMap.has(key)),
      ...Array.from(groupMap.keys()).filter((key) => !preferredGroupOrder.includes(key)),
    ];
    for (const assetClass of orderedKeys) {
      const assets = groupMap.get(assetClass) || [];
      groups.push({
        assetClass,
        label: `${assetClassLabel(assetClass).toLowerCase()}_`,
        assets: assets.map((record) => ({
          value: record.name,
          title: record.compareCode,
          subtitle: record.subtitle,
          active: activeInstrumentSet.has(record.name),
          tradable: brokerInstrumentNames.has(record.name),
          brokerLogoKind,
          brokerLabel,
        })),
      });
    }
    const headerMenuKey = JSON.stringify({
      mode: menuMode,
      query: state.compareSearch,
      brokerLogoKind,
      groups: groups.map((group) => [group.assetClass, group.assets.map((item) => [item.value, item.active, item.tradable])]),
    });
    const model = {
      groups,
      query: state.compareSearch,
      brokerLogoKind,
      brokerLabel,
      headerMenuKey,
    };
    state.uiCache.compareMenuModelKey = modelKey;
    state.uiCache.compareMenuModelValue = model;
    return model;
  }

  function renderAdvancedCompareMenu(mode = "compare") {
    const menuMode = mode === "add" ? "add" : "compare";
    const { groups, headerMenuKey } = advancedCompareMenuModel(menuMode);
    if (state.uiCache.headerMenuKey === headerMenuKey) return;
    state.uiCache.headerMenuKey = headerMenuKey;
    els.compareMenu.innerHTML = "";
    const shell = document.createElement("div");
    shell.className = "trading-compare-browser";
    shell.style.setProperty("--compare-cols", String(Math.max(1, groups.length)));
    shell.innerHTML = `
      <div class="trading-compare-browser-panel-shell">
        <div class="trading-compare-browser-panel">
          <div class="trading-compare-head-row"></div>
          <div class="trading-compare-browser-grid"></div>
        </div>
      </div>
    `;
    const panelShell = shell.querySelector(".trading-compare-browser-panel-shell");
    const panel = shell.querySelector(".trading-compare-browser-panel");
    const headRow = shell.querySelector(".trading-compare-head-row");
    const grid = shell.querySelector(".trading-compare-browser-grid");
    if (!groups.length) {
      const empty = document.createElement("div");
      empty.className = "trading-compare-empty";
      empty.textContent = state.compareSearch ? "no matching assets" : "start typing to filter assets";
      grid?.appendChild(empty);
    }
    groups.forEach((group, index) => {
      const head = document.createElement("div");
      head.className = "trading-compare-group-head";
      head.style.transitionDelay = `${Math.min(index, 5) * 28}ms`;
      head.textContent = group.label;
      head.dataset.tradingToken = `<asset_class:${group.assetClass}>`;
      headRow?.appendChild(head);
      const section = document.createElement("section");
      section.className = "trading-compare-group";
      section.style.transitionDelay = `${Math.min(index, 5) * 28}ms`;
      const list = document.createElement("div");
      list.className = "trading-compare-group-list";
      group.assets.forEach((item) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = `trading-compare-menu-item trading-compare-asset-item${item.active ? " is-active" : ""}`;
        button.dataset.menuKind = menuMode;
        button.dataset.menuValue = item.value;
        button.dataset.tradingToken = tokenForInstrumentName(item.value);
        button.innerHTML = `
          <span class="trading-compare-menu-check">${item.active ? "✓" : ""}</span>
          <span class="trading-compare-menu-copy">
            <span class="trading-compare-menu-title-row">
              <span class="trading-compare-menu-title">${item.title}</span>
              ${item.tradable && item.brokerLogoKind ? `<span class="trading-compare-broker-badge" title="Tradable on ${item.brokerLabel}" aria-label="Tradable on ${item.brokerLabel}"><span class="trading-broker-logo-mark trading-broker-logo-${item.brokerLogoKind} trading-compare-broker-logo"></span></span>` : ""}
            </span>
            <span class="trading-compare-menu-subtitle">${item.subtitle}</span>
          </span>
        `;
        list.appendChild(button);
      });
      section.appendChild(list);
      grid?.appendChild(section);
    });
    if (panel) {
      panel.scrollTop = state.compareScrollTop || 0;
      panel.addEventListener("scroll", () => {
        state.compareScrollTop = panel.scrollTop;
      }, { passive: true });
      const createScrollbar = window.__forgeCreateScrollbarElements;
      const bindScrollbar = window.__forgeBindCustomScrollbar;
      if (typeof createScrollbar === "function" && typeof bindScrollbar === "function" && panelShell) {
        const { rail, thumb } = createScrollbar();
        rail.classList.add("trading-compare-scrollbar");
        panelShell.appendChild(rail);
        bindScrollbar(panel, rail, thumb);
      }
    }
    els.compareMenu.appendChild(shell);
  }

  function renderTimeframeRail() {
    if (!els.timeframeRail) return;
    els.timeframeRail.hidden = !state.active;
    const timeframeKey = JSON.stringify({
      active: state.active,
      selectedGranularity: state.selectedGranularity,
      options: TIMEFRAME_OPTIONS.map((item) => item.value),
    });
    if (state.uiCache.timeframeKey === timeframeKey) return;
    state.uiCache.timeframeKey = timeframeKey;
    els.timeframeRail.innerHTML = "";
    for (const option of TIMEFRAME_OPTIONS) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `trading-timeframe-btn${state.selectedGranularity === option.value ? " is-active" : ""}`;
      button.dataset.tradingGranularity = option.value;
      button.textContent = option.label;
      els.timeframeRail.appendChild(button);
    }
  }

  function indicatorSettingsFieldMarkup(indicator, fieldKey, fieldMeta) {
    const settings = indicator?.settings || {};
    const value = settings[fieldKey];
    if (fieldMeta?.type === "checkbox") {
      return `
        <label class="trading-indicator-settings-check">
          <input type="checkbox" data-indicator-field="${escapeHtml(fieldKey)}"${value ? " checked" : ""}>
          <span>${escapeHtml(fieldMeta.label)}</span>
        </label>
      `;
    }
    if (fieldMeta?.type === "select") {
      return `
        <label class="trading-indicator-settings-row">
          <span>${escapeHtml(fieldMeta.label)}</span>
          <select data-indicator-field="${escapeHtml(fieldKey)}">
            ${(fieldMeta.options || []).map((option) => `
              <option value="${escapeHtml(option)}"${String(option) === String(value) ? " selected" : ""}>${escapeHtml(String(option).replace(/^./, (char) => char.toUpperCase()))}</option>
            `).join("")}
          </select>
        </label>
      `;
    }
    return `
      <label class="trading-indicator-settings-row">
        <span>${escapeHtml(fieldMeta.label)}</span>
        <input
          type="number"
          data-indicator-field="${escapeHtml(fieldKey)}"
          value="${escapeHtml(String(value ?? fieldMeta.value ?? 0))}"
          ${fieldMeta.min != null ? `min="${escapeHtml(String(fieldMeta.min))}"` : ""}
          ${fieldMeta.max != null ? `max="${escapeHtml(String(fieldMeta.max))}"` : ""}
          ${fieldMeta.step != null ? `step="${escapeHtml(String(fieldMeta.step))}"` : ""}
        >
      </label>
    `;
  }

  function renderIndicatorSettingsMenu() {
    if (!els.compareMenu) return;
    const indicator = activeIndicatorInstance(state.indicatorSettingsId);
    const definition = tradingIndicatorDefinition(state.indicatorSettingsId);
    if (!indicator || !definition) {
      els.compareMenu.innerHTML = "";
      return;
    }
    const tabs = [
      ["inputs", "Inputs"],
      ["style", "Style"],
      ["visibility", "Visibility"],
    ];
    const activeTab = ["inputs", "style", "visibility"].includes(state.indicatorSettingsTab)
      ? state.indicatorSettingsTab
      : "inputs";
    const body = activeTab === "inputs"
      ? Object.entries(definition.settings || {}).map(([fieldKey, fieldMeta]) => indicatorSettingsFieldMarkup(indicator, fieldKey, fieldMeta)).join("")
      : activeTab === "style"
        ? `<div class="trading-indicator-settings-note">Minimal style surface. The chart keeps the clean Forge palette by default.</div>`
        : `<label class="trading-indicator-settings-check"><input type="checkbox" data-indicator-field-visible="true"${indicator.visible === false ? "" : " checked"}><span>Visible on chart</span></label>`;
    els.compareMenu.innerHTML = `
      <div class="trading-indicator-settings-shell">
        <div class="trading-indicator-settings-head">
          <strong>${escapeHtml(definition.label)}</strong>
          <button type="button" class="trading-indicator-settings-close" data-subbar-action="close-indicator-settings" aria-label="Close">
            ${tradingSubbarIcon("close")}
          </button>
        </div>
        <div class="trading-indicator-settings-tabs">
          ${tabs.map(([key, label]) => `
            <button type="button" class="trading-indicator-settings-tab${activeTab === key ? " is-active" : ""}" data-indicator-settings-tab="${key}">${label}</button>
          `).join("")}
        </div>
        <div class="trading-indicator-settings-body">${body}</div>
      </div>
    `;
  }

  function renderHeaderMenu(mode = "compare") {
    if (!els.compareMenu) return;
    state.headerMenuMode = mode;
    if (mode === "indicator-settings") {
      renderIndicatorSettingsMenu();
      return;
    }
    if (mode === "compare" || mode === "add") {
      renderAdvancedCompareMenu(mode);
      return;
    }
    let items = [];
    if (mode === "broker") {
      items = availableBrokers().map((broker) => ({
        kind: "broker",
        value: broker.kind,
        title: broker.label,
        subtitle: broker.kind === selectedBrokerKind() ? "active" : "available",
        active: broker.kind === selectedBrokerKind(),
      }));
    } else if (mode === "display") {
      items = CHART_DISPLAY_OPTIONS.map((option) => ({
        kind: "display",
        value: option.value,
        title: option.title,
        subtitle: option.subtitle,
        active: option.value === state.chartDisplayMode,
      }));
    } else if (mode === "asset") {
      items = assetSearchIndex().records.map((record) => ({
        kind: "asset",
        value: record.name,
        title: record.brokerCode,
        subtitle: record.asset.displayName || record.asset.assetClass || record.name,
        active: record.name === state.selectedInstrument,
      }));
    } else {
      items = assetSearchIndex().records
        .filter((record) => record.name !== state.selectedInstrument)
        .map((record) => ({
          kind: "compare",
          value: record.name,
          title: `${record.brokerCode}_`,
          subtitle: record.asset.displayName || record.asset.assetClass || record.name,
          active: state.compareInstruments.includes(record.name),
        }));
    }
    const headerMenuKey = JSON.stringify({
      mode,
      items: items.map((item) => [item.kind, item.value, item.title, item.subtitle, item.active]),
    });
    if (state.uiCache.headerMenuKey === headerMenuKey) return;
    state.uiCache.headerMenuKey = headerMenuKey;
    els.compareMenu.innerHTML = "";
    const shell = document.createElement("div");
    shell.className = "trading-flyout-menu-shell";
    const body = document.createElement("div");
    body.className = "trading-flyout-menu-body";
    shell.appendChild(body);
    for (const item of items) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `trading-compare-menu-item${item.active ? " is-active" : ""}`;
      button.dataset.menuKind = item.kind;
      button.dataset.menuValue = item.value;
      if (item.kind === "broker") button.dataset.tradingToken = tokenForBrokerKind(item.value);
      else if (item.kind === "display") button.dataset.tradingToken = tokenForDisplayMode(item.value);
      else button.dataset.tradingToken = tokenForInstrumentName(item.value);
      button.innerHTML = `
        <span class="trading-compare-menu-check">${item.active ? "✓" : ""}</span>
        <span class="trading-compare-menu-copy">
          <span class="trading-compare-menu-title">${item.title}</span>
          <span class="trading-compare-menu-subtitle">${item.subtitle}</span>
        </span>
      `;
      body.appendChild(button);
    }
    body.scrollTop = state.headerMenuScrollTop[mode] || 0;
    body.addEventListener("scroll", () => {
      state.headerMenuScrollTop[mode] = body.scrollTop;
    }, { passive: true });
    const createScrollbar = window.__forgeCreateScrollbarElements;
    const bindScrollbar = window.__forgeBindCustomScrollbar;
    if (typeof createScrollbar === "function" && typeof bindScrollbar === "function") {
      const { rail, thumb } = createScrollbar();
      rail.classList.add("trading-compare-scrollbar");
      shell.appendChild(rail);
      bindScrollbar(body, rail, thumb);
    }
    els.compareMenu.appendChild(shell);
  }

  function openHeaderMenu(trigger, mode = "compare") {
    if (!trigger || !els.compareMenu) return;
    if (state.headerMenuHideTimer) {
      window.clearTimeout(state.headerMenuHideTimer);
      state.headerMenuHideTimer = 0;
    }
    renderHeaderMenu(mode);
    state.headerMenuTriggerEl = trigger;
    els.compareMenu.dataset.mode = mode;
    els.compareMenu.hidden = false;
    setDropdownScrim(true);
    if (mode === "compare" || mode === "add" || mode === "indicator-settings") {
      if (els.compareSearchWrap) els.compareSearchWrap.hidden = !(mode === "compare" || mode === "add");
      if (els.compareSearchInput) els.compareSearchInput.value = (mode === "compare" || mode === "add") ? String(state.compareSearch || "") : "";
      els.compareMenu.style.left = "";
      els.compareMenu.style.top = "";
    } else {
      if (els.compareSearchWrap) els.compareSearchWrap.hidden = true;
      const rect = trigger.getBoundingClientRect();
      const targetWidth = mode === "display" ? 260 : (mode === "asset" ? 320 : (mode === "indicator-settings" ? 420 : 360));
      const preferredLeft = mode === "display" ? (rect.right - targetWidth) : rect.left;
      const left = Math.max(12, Math.min(preferredLeft, window.innerWidth - targetWidth - 12));
      els.compareMenu.style.left = `${left}px`;
      els.compareMenu.style.top = `${Math.min(window.innerHeight - 24, rect.bottom + 4)}px`;
    }
    syncCompareShellState();
    requestAnimationFrame(() => els.compareMenu.classList.add("is-open"));
    if (trigger === els.compareTrigger) els.compareTrigger.setAttribute("aria-expanded", "true");
    if (trigger === els.addTrigger) els.addTrigger.setAttribute("aria-expanded", "true");
    if (trigger === els.chartModeTrigger) {
      state.uiCache.chartModeTriggerKey = "";
      syncChartModeTrigger();
    }
  }

  function clearComparisonMenu() {
    closeHeaderMenu();
  }

  async function syncComparisonSeries() {
    const splitTargets = addedChartInstruments();
    const splitActive = splitTargets.length > 0;
    const targets = splitActive ? splitTargets : state.compareInstruments.filter((instrument) =>
      instrument && instrument !== state.selectedInstrument
    );
    if (!targets.length) {
      if (splitActive) setAddedChartInstruments([]);
      else state.compareInstruments = [];
      if (state.uiCache.extraChartsKey !== "empty") {
        state.uiCache.extraChartsKey = "empty";
        state.uiCache.extraChartsValue = [];
        canvasBridge()?.setExtraCharts?.([]);
      }
      syncTradingHeader();
      return;
    }
    const cachedKey = [
      assetUniverseRevision,
      splitActive ? "split" : "overlay",
      state.selectedGranularity,
      targets.map((instrument) => `${instrument}:${cachedSeriesRevision(instrument, state.selectedGranularity)}`).join("|"),
    ].join("::");
    if (
      state.uiCache.extraChartsKey === cachedKey
      && Array.isArray(state.uiCache.extraChartsValue)
      && (!hasTauriInvoke() || targets.every((instrument) => cachedSeriesRevision(instrument, state.selectedGranularity) > 0))
    ) {
      canvasBridge()?.setExtraCharts?.(state.uiCache.extraChartsValue);
      syncTradingHeader();
      return;
    }
    const extraCharts = [];
    const brokerInstrumentSet = activeBrokerInstrumentSet();
    for (const instrument of targets) {
      let candles = getCachedSeries(instrument, state.selectedGranularity);
      if (!candles.length && hasTauriInvoke()) {
        candles = await loadHistorySeries(instrument, state.selectedGranularity, {
          maxRows: chartTailRows(state.selectedGranularity),
        });
      }
      if (!candles.length) continue;
      const asset = findAsset(instrument);
      extraCharts.push({
        instrument,
        instrumentLabel: brokerInstrumentCode(instrument) || instrument,
        tradingInstrument: instrument,
        tradingGranularity: state.selectedGranularity,
        brokerLabel: brokerInstrumentSet.has(instrument) ? selectedBrokerLabel() : "PAPERTRADING",
        brokerLogoKind: brokerInstrumentSet.has(instrument) ? selectedBrokerLogoKind() : "",
        displayName: asset?.displayName || instrument,
        fileName: `${instrument}_${state.selectedGranularity}.csv`,
        layout: splitActive ? "split" : "overlay",
        candles,
      });
    }
    const finalKey = [
      assetUniverseRevision,
      splitActive ? "split" : "overlay",
      state.selectedGranularity,
      targets.map((instrument) => `${instrument}:${cachedSeriesRevision(instrument, state.selectedGranularity)}`).join("|"),
    ].join("::");
    const cacheable = !hasTauriInvoke() || targets.every((instrument) => cachedSeriesRevision(instrument, state.selectedGranularity) > 0);
    if (cacheable) {
      state.uiCache.extraChartsKey = finalKey;
      state.uiCache.extraChartsValue = extraCharts;
    }
    canvasBridge()?.setExtraCharts?.(extraCharts);
    syncTradingHeader();
  }

  function renderAssetList(target, assets) {
    if (!target) return;
    target.innerHTML = "";
    for (const asset of assets) {
      const item = document.createElement("li");
      item.className = `job-item${asset.name === state.selectedInstrument ? " active" : ""}`;
      item.dataset.tradingInstrument = asset.name;
      item.setAttribute("role", "button");
      item.setAttribute("tabindex", "0");
      item.title = `${asset.displayName} · ${asset.assetClass}`;
      const title = document.createElement("span");
      title.className = "job-title";
      title.textContent = asset.name;
      const spacer = document.createElement("span");
      spacer.setAttribute("aria-hidden", "true");
      item.appendChild(title);
      item.appendChild(spacer);
      target.appendChild(item);
    }
  }

  function renderLeftPanel() {
    const assets = libraryAssets();
    const pinned = assets.filter((item) => PINNED_ASSETS.includes(item.name));
    const main = assets.filter((item) => !PINNED_ASSETS.includes(item.name));
    const leftPanelKey = JSON.stringify({
      selectedInstrument: state.selectedInstrument,
      activePriceLine: activePriceLine(),
      pinned: pinned.map((item) => [item.name, item.displayName, item.assetClass]),
      main: (main.length ? main : assets).map((item) => [item.name, item.displayName, item.assetClass]),
    });
    if (state.uiCache.leftPanelKey === leftPanelKey) return;
    state.uiCache.leftPanelKey = leftPanelKey;
    if (els.statusText) els.statusText.textContent = activePriceLine();
    if (els.pinDropText) els.pinDropText.textContent = "Pinned assets";
    if (els.historyHeading) els.historyHeading.textContent = "Assets";
    if (els.pinMenuBtn) els.pinMenuBtn.hidden = true;
    if (els.pinDrop) els.pinDrop.style.pointerEvents = "none";
    renderAssetList(els.pinnedList, pinned);
    renderAssetList(els.jobList, main.length ? main : assets);
    if (els.jobMenu) els.jobMenu.hidden = true;
  }

  function updateOrderFormInstrument() {
    state.orderForm.instrument = state.selectedInstrument;
  }

  function defaultConsoleContext() {
    return {
      workspaceMode: "trading",
      instrument: state.selectedInstrument,
      granularity: state.selectedGranularity,
      config: state.snapshot?.config || null,
      account: state.snapshot?.account || null,
      price: state.market?.price || state.snapshot?.price || null,
      book: state.market?.book || null,
      pendingOrders: state.market?.pendingOrders || state.snapshot?.pendingOrders || [],
      openTrades: state.market?.openTrades || state.snapshot?.openTrades || [],
      historyFiles: state.catalog || [],
      chartBars: state.candles.length,
    };
  }

  function canvasTradingSnapshot() {
    return canvasBridge()?.snapshotState?.() || null;
  }

  function snapshotAxisSummary(snapshot = canvasTradingSnapshot()) {
    return {
      xMode: String(snapshot?.tradingAxisMetricPrefs?.xMode || "time"),
      yMode: String(snapshot?.tradingAxisMetricPrefs?.yMode || "price"),
      metricSpace: String(snapshot?.tradingMetricSpacePrefs?.mode || "classic"),
      timeZone: String(snapshot?.tradingTimePrefs?.timeZone || "UTC"),
      sessionBreaks: snapshot?.tradingTimePrefs?.showSessionBreaks === true,
      signalMarkers: snapshot?.tradingTimePrefs?.showSignalMarkers === true,
      autoFit: snapshot?.tradingScalePrefs?.autoFit !== false,
      invertPriceScale: snapshot?.tradingScalePrefs?.invertPriceScale === true,
      priceAxisSide: String(snapshot?.tradingScalePrefs?.priceAxisSide || "right"),
      showPriceLabels: snapshot?.tradingScalePrefs?.showPriceLabels !== false,
      showGridLines: snapshot?.tradingScalePrefs?.showGridLines !== false,
      showPlusButton: snapshot?.tradingScalePrefs?.showPlusButton !== false,
    };
  }

  function summarizeCandles(candles = state.candles) {
    const rows = Array.isArray(candles) ? candles : [];
    if (!rows.length) {
      return {
        count: 0,
        firstTime: "",
        lastTime: "",
        firstOpen: null,
        lastClose: null,
        high: null,
        low: null,
        visiblePreview: [],
      };
    }
    let high = -Infinity;
    let low = Infinity;
    for (const candle of rows) {
      high = Math.max(high, Number(candle?.high) || -Infinity);
      low = Math.min(low, Number(candle?.low) || Infinity);
    }
    return {
      count: rows.length,
      firstTime: String(rows[0]?.time || ""),
      lastTime: String(rows[rows.length - 1]?.time || ""),
      firstOpen: Number(rows[0]?.open),
      lastClose: Number(rows[rows.length - 1]?.close),
      high: Number.isFinite(high) ? high : null,
      low: Number.isFinite(low) ? low : null,
      visiblePreview: rows.slice(-5).map((candle) => ({
        time: String(candle?.time || ""),
        open: Number(candle?.open),
        high: Number(candle?.high),
        low: Number(candle?.low),
        close: Number(candle?.close),
        volume: Number(candle?.volume || 0),
      })),
    };
  }

  function summarizeTrades(items = []) {
    return (Array.isArray(items) ? items : []).slice(0, 8).map((item) => ({
      id: String(item?.id || item?.tradeId || item?.orderId || ""),
      instrument: String(item?.instrument || state.selectedInstrument || ""),
      side: String(item?.side || item?.currentUnits || item?.units || ""),
      units: Number(item?.units || item?.currentUnits || 0),
      price: Number(item?.price || item?.entryPrice || item?.averagePrice || 0),
      takeProfit: Number(item?.takeProfit || item?.takeProfitPrice || 0),
      stopLoss: Number(item?.stopLoss || item?.stopLossPrice || 0),
      trailingStop: Number(item?.trailingStop || item?.trailingStopPrice || 0),
      time: String(item?.time || item?.openTime || item?.createTime || ""),
    }));
  }

  function summarizePrice(price = state.market?.price || state.snapshot?.price || null) {
    if (!price || typeof price !== "object") return null;
    return {
      instrument: String(price.instrument || state.selectedInstrument || ""),
      bid: Number(price.bid),
      ask: Number(price.ask),
      mid: Number(price.mid),
      spread: Number(price.spread),
      time: String(price.time || ""),
    };
  }

  function summarizeBook(book = state.market?.book || state.snapshot?.book || null) {
    if (!book || typeof book !== "object") return null;
    return {
      kind: String(book.kind || ""),
      note: String(book.note || ""),
      bids: Array.isArray(book.bids) ? book.bids.slice(0, 5) : [],
      asks: Array.isArray(book.asks) ? book.asks.slice(0, 5) : [],
    };
  }

  function summarizeAccount(account = state.snapshot?.account || null) {
    if (!account || typeof account !== "object") return null;
    return {
      alias: String(account.alias || ""),
      currency: String(account.currency || ""),
      balance: Number(account.balance),
      nav: Number(account.nav),
      unrealizedPl: Number(account.unrealizedPl),
      marginAvailable: Number(account.marginAvailable),
      openTradeCount: Number(account.openTradeCount || 0),
      openPositionCount: Number(account.openPositionCount || 0),
      pendingOrderCount: Number(account.pendingOrderCount || 0),
    };
  }

  function summarizeCatalog(files = state.catalog) {
    const catalog = Array.isArray(files) ? files : [];
    const index = catalog === state.catalog ? tradingCatalogIndex() : null;
    return {
      fileCount: catalog.length,
      instrumentCount: index ? index.byInstrument.size : new Set(catalog.map((entry) => String(entry?.instrument || "").trim()).filter(Boolean)).size,
      granularityCoverage: index ? index.granularityCoverage : catalog.reduce((out, entry) => {
        const granularity = String(entry?.granularity || "").trim().toUpperCase();
        if (granularity) out[granularity] = (out[granularity] || 0) + 1;
        return out;
      }, {}),
      universeHistoryReady: !!state.universeHistorySyncDone,
    };
  }

  function summarizeAlerts(alerts = state.alerts) {
    return (Array.isArray(alerts) ? alerts : []).slice(0, 12).map((alert) => ({
      id: String(alert?.id || ""),
      instrument: String(alert?.instrument || state.selectedInstrument || ""),
      granularity: String(alert?.granularity || state.selectedGranularity || ""),
      operator: String(alert?.operator || ""),
      targetValue: Number(alert?.targetValue || 0),
      active: alert?.active !== false,
      triggerMode: String(alert?.triggerMode || ""),
      triggeredCount: Number(alert?.triggeredCount || 0),
      lastTriggeredAtMs: Number(alert?.lastTriggeredAtMs || 0),
      message: String(alert?.message || ""),
    }));
  }

  function summarizeCompareAssets(compareAssets = []) {
    return compareAssets.map((item) => {
      const cached = getCachedSeries(item.instrument, state.selectedGranularity);
      return {
        ...item,
        candleCount: cached.length,
        lastClose: cached.length ? Number(cached[cached.length - 1]?.close) : null,
      };
    });
  }

  function summarizeUiState(bridgeSnapshot, compareAssets, observedAssets = null) {
    const observed = Array.isArray(observedAssets?.assets) ? observedAssets.assets : [];
    return {
      active: !!state.active,
      headerMenuMode: state.headerMenuMode,
      compareMenuOpen: state.headerMenuMode === "compare" && els.compareMenu?.hidden === false,
      addMenuOpen: state.headerMenuMode === "add" && els.compareMenu?.hidden === false,
      rightPanelOpen: !!bridgeSnapshot?.rightPanelOpen,
      rightPanelMode: String(bridgeSnapshot?.rightPanelMode || ""),
      topbarCompareVisible: !els.compareTrigger?.hidden,
      topbarAddVisible: !els.addTrigger?.hidden,
      timeframeRailVisible: !els.timeframeRail?.hidden,
      compareSelectionCount: compareAssets.length,
      addSelectionCount: Math.max(0, observed.length - 1),
      observedAssetCount: observed.length || 1,
      compareSearch: String(state.compareSearch || ""),
    };
  }

  function summarizeIndicatorState(bridgeSnapshot) {
    const axis = snapshotAxisSummary(bridgeSnapshot);
    return {
      panelOpen: !!state.indicatorsModeActive,
      activeCount: 0,
      active: [],
      xMetricMode: String(axis.xMode || "time"),
      yMetricMode: String(axis.yMode || "price"),
      note: state.indicatorsModeActive
        ? "Indicators panel is open, but no custom study registry is wired yet."
        : "Indicators panel is closed.",
    };
  }

  function summarizeReplayState(bridgeSnapshot) {
    const viewport = bridgeSnapshot?.tradingViewport || null;
    return {
      active: !!state.replayModeActive,
      visibleBars: Number(viewport?.visibleBars || 0),
      timeStartMs: Number(viewport?.timeStartMs || 0),
      timeEndMs: Number(viewport?.timeEndMs || 0),
      note: state.replayModeActive
        ? "Replay mode is armed from the current viewport."
        : "Replay mode is inactive.",
    };
  }

  function summarizeOrderDraft(orderForm = state.orderForm) {
    const units = Number(orderForm?.units || 0);
    const limitPrice = orderForm?.limitPrice ? Number(orderForm.limitPrice) : null;
    const stopLoss = orderForm?.stopLoss ? Number(orderForm.stopLoss) : null;
    const takeProfit = orderForm?.takeProfit ? Number(orderForm.takeProfit) : null;
    return {
      instrument: String(orderForm?.instrument || state.selectedInstrument || ""),
      side: String(orderForm?.side || ""),
      units,
      orderType: String(orderForm?.orderType || ""),
      limitPrice,
      stopLoss,
      takeProfit,
      ready: units > 0 && !!String(orderForm?.side || "").trim() && !!String(orderForm?.orderType || "").trim(),
    };
  }

  function priceDeltaPct(nextValue, baseValue) {
    const next = Number(nextValue);
    const base = Number(baseValue);
    if (!Number.isFinite(next) || !Number.isFinite(base) || Math.abs(base) < 0.000001) return null;
    return ((next - base) / base) * 100;
  }

  function summarizeSignalState(candles = state.candles, compareAssets = [], price = state.market?.price || state.snapshot?.price || null, alerts = state.alerts) {
    const rows = Array.isArray(candles) ? candles : [];
    if (!rows.length) {
      return {
        bias: "unknown",
        structure: "unknown",
        momentumPct: null,
        intrabarRangePct: null,
        lastCandleDirection: "unknown",
        nearestAlertDistancePct: null,
        compareStrength: [],
        note: "No candle history loaded for signal synthesis.",
      };
    }
    const last = rows[rows.length - 1];
    const prev = rows.length > 1 ? rows[rows.length - 2] : null;
    const anchor = rows[Math.max(0, rows.length - 21)];
    const lastOpen = Number(last?.open);
    const lastClose = Number(last?.close);
    const lastHigh = Number(last?.high);
    const lastLow = Number(last?.low);
    const lastDirection = lastClose > lastOpen ? "bullish" : lastClose < lastOpen ? "bearish" : "flat";
    const momentumPct = priceDeltaPct(lastClose, anchor?.close);
    const intrabarRangePct = priceDeltaPct(lastHigh, Math.max(0.000001, lastLow)) ;
    let structure = "undetermined";
    if (prev) {
      const prevHigh = Number(prev?.high);
      const prevLow = Number(prev?.low);
      if (Number.isFinite(prevHigh) && lastClose > prevHigh) structure = "breakout_up";
      else if (Number.isFinite(prevLow) && lastClose < prevLow) structure = "breakout_down";
      else if (Number.isFinite(prevHigh) && Number.isFinite(prevLow) && lastHigh <= prevHigh && lastLow >= prevLow) structure = "inside_bar";
      else structure = "range_overlap";
    }
    let bias = "neutral";
    if (typeof momentumPct === "number") {
      if (momentumPct >= 1.5) bias = "strong_bullish";
      else if (momentumPct >= 0.35) bias = "bullish";
      else if (momentumPct <= -1.5) bias = "strong_bearish";
      else if (momentumPct <= -0.35) bias = "bearish";
    } else if (lastDirection !== "flat") {
      bias = lastDirection;
    }
    const currentMid = Number(price?.mid || lastClose);
    const relevantAlerts = (Array.isArray(alerts) ? alerts : []).filter((alert) => alert?.active !== false && String(alert?.instrument || state.selectedInstrument) === state.selectedInstrument);
    let nearestAlertDistancePct = null;
    for (const alert of relevantAlerts) {
      const target = Number(alert?.targetValue || 0);
      const distance = Math.abs(priceDeltaPct(target, currentMid) || 0);
      if (nearestAlertDistancePct == null || distance < nearestAlertDistancePct) nearestAlertDistancePct = distance;
    }
    const compareStrength = (Array.isArray(compareAssets) ? compareAssets : []).slice(0, 6).map((item) => {
      const series = getCachedSeries(item.instrument, state.selectedGranularity);
      if (!series.length) {
        return {
          instrument: item.instrument,
          instrumentLabel: item.instrumentLabel || item.instrument,
          momentumPct: null,
          relativeStrengthPct: null,
        };
      }
      const compareLast = series[series.length - 1];
      const compareAnchor = series[Math.max(0, series.length - 21)];
      const compareMomentumPct = priceDeltaPct(compareLast?.close, compareAnchor?.close);
      return {
        instrument: item.instrument,
        instrumentLabel: item.instrumentLabel || item.instrument,
        momentumPct: compareMomentumPct,
        relativeStrengthPct: typeof compareMomentumPct === "number" && typeof momentumPct === "number"
          ? compareMomentumPct - momentumPct
          : null,
      };
    });
    return {
      bias,
      structure,
      momentumPct,
      intrabarRangePct,
      lastCandleDirection: lastDirection,
      nearestAlertDistancePct,
      compareStrength,
      note: "Synthesized locally from currently loaded candles and compare series.",
    };
  }

  function strategyCandleRows(candles = state.candles, limit = 2500) {
    const source = Array.isArray(candles) ? candles : [];
    const start = Math.max(0, source.length - Math.max(80, Number(limit) || 2500));
    const rows = [];
    for (let i = start; i < source.length; i += 1) {
      const candle = source[i] || {};
      const open = Number(candle.open);
      const high = Number(candle.high);
      const low = Number(candle.low);
      const close = Number(candle.close);
      if (![open, high, low, close].every(Number.isFinite)) continue;
      rows.push({
        open,
        high,
        low,
        close,
        time: String(candle.time || ""),
        timeMs: candleTimeMs(candle, NaN),
      });
    }
    return rows;
  }

  function seriesEma(values = [], period = 14) {
    const p = Math.max(2, Math.floor(Number(period) || 14));
    const alpha = 2 / (p + 1);
    const out = new Array(values.length).fill(null);
    let ema = null;
    for (let i = 0; i < values.length; i += 1) {
      const value = Number(values[i]);
      if (!Number.isFinite(value)) continue;
      ema = ema == null ? value : (value * alpha) + (ema * (1 - alpha));
      out[i] = ema;
    }
    return out;
  }

  function seriesAtr(rows = [], period = 14) {
    const p = Math.max(2, Math.floor(Number(period) || 14));
    const tr = new Array(rows.length).fill(null);
    for (let i = 0; i < rows.length; i += 1) {
      const row = rows[i];
      const prevClose = i > 0 ? rows[i - 1]?.close : row?.close;
      if (!row || !Number.isFinite(prevClose)) continue;
      tr[i] = Math.max(
        row.high - row.low,
        Math.abs(row.high - prevClose),
        Math.abs(row.low - prevClose),
      );
    }
    return seriesEma(tr, p);
  }

  function seriesRsi(values = [], period = 14) {
    const p = Math.max(2, Math.floor(Number(period) || 14));
    const out = new Array(values.length).fill(null);
    let avgGain = 0;
    let avgLoss = 0;
    for (let i = 1; i < values.length; i += 1) {
      const current = Number(values[i]);
      const prev = Number(values[i - 1]);
      if (!Number.isFinite(current) || !Number.isFinite(prev)) continue;
      const delta = current - prev;
      const gain = Math.max(0, delta);
      const loss = Math.max(0, -delta);
      if (i <= p) {
        avgGain += gain;
        avgLoss += loss;
        if (i === p) {
          avgGain /= p;
          avgLoss /= p;
        } else {
          continue;
        }
      } else {
        avgGain = ((avgGain * (p - 1)) + gain) / p;
        avgLoss = ((avgLoss * (p - 1)) + loss) / p;
      }
      const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
      out[i] = 100 - (100 / (1 + rs));
    }
    return out;
  }

  function makeStrategyIndicators(rows = [], rules = {}) {
    const closes = rows.map((row) => row.close);
    return {
      fast: seriesEma(closes, rules.fastPeriod || 9),
      slow: seriesEma(closes, rules.slowPeriod || 21),
      atr: seriesAtr(rows, rules.atrPeriod || 14),
      rsi: seriesRsi(closes, rules.rsiPeriod || 14),
    };
  }

  function rollingHigh(rows = [], endIndex = 0, lookback = 20) {
    let value = -Infinity;
    const start = Math.max(0, endIndex - Math.max(2, lookback));
    for (let i = start; i < endIndex; i += 1) value = Math.max(value, rows[i]?.high ?? -Infinity);
    return Number.isFinite(value) ? value : null;
  }

  function rollingLow(rows = [], endIndex = 0, lookback = 20) {
    let value = Infinity;
    const start = Math.max(0, endIndex - Math.max(2, lookback));
    for (let i = start; i < endIndex; i += 1) value = Math.min(value, rows[i]?.low ?? Infinity);
    return Number.isFinite(value) ? value : null;
  }

  function synthesizeStrategyRules(prompt = "", snapshot = null) {
    const text = String(prompt || "").toLowerCase();
    const instrument = snapshot?.instrument?.label || state.selectedInstrument;
    const granularity = snapshot?.timeframe?.granularity || state.selectedGranularity;
    const base = {
      id: "ema_momentum_pullback",
      title: "EMA Momentum Pullback",
      kind: "ema_momentum",
      instrument,
      granularity,
      fastPeriod: 9,
      slowPeriod: 21,
      atrPeriod: 14,
      rsiPeriod: 14,
      breakoutLookback: 24,
      riskAtr: 1.4,
      rewardRisk: 1.8,
      maxHoldBars: 48,
      side: "both",
    };
    if (/\brsi\b|\bmean\b|\breversion\b|\brange\b|\bsurvend|surachet|rebond|retour/.test(text)) {
      return {
        ...base,
        id: "rsi_atr_reversion",
        title: "RSI ATR Reversion",
        kind: "rsi_reversion",
        fastPeriod: 8,
        slowPeriod: 34,
        riskAtr: 1.1,
        rewardRisk: 1.35,
        maxHoldBars: 30,
      };
    }
    if (/\bbreak\b|\bbreakout\b|\bdonchian\b|\bvolatil|\bexplosion|\bmomentum fort|\brange break/.test(text)) {
      return {
        ...base,
        id: "volatility_breakout",
        title: "Volatility Breakout",
        kind: "breakout",
        fastPeriod: 12,
        slowPeriod: 40,
        breakoutLookback: 30,
        riskAtr: 1.6,
        rewardRisk: 2.1,
        maxHoldBars: 64,
      };
    }
    return base;
  }

  function parseStrategyDecimal(value) {
    const raw = String(value ?? "").trim().replace(",", ".");
    const match = raw.match(/-?\d+(?:\.\d+)?/);
    if (!match) return null;
    const number = Number(match[0]);
    return Number.isFinite(number) ? number : null;
  }

  function inferStrategyInstrumentFromPrompt(text = "", snapshot = null) {
    const lower = String(text || "").toLowerCase();
    if (/\b(gaz naturel|natural gas|natgas|ngas|gaz)\b/.test(lower)) return "NATGAS_USD";
    if (/\b(eur[\/_\s-]?usd|euro dollar)\b/.test(lower)) return "EUR_USD";
    if (/\b(gbp[\/_\s-]?usd|cable)\b/.test(lower)) return "GBP_USD";
    if (/\b(xau[\/_\s-]?usd|gold|or)\b/.test(lower)) return "XAU_USD";
    if (/\b(wti|oil|petrole|pétrole)\b/.test(lower)) return "WTICO_USD";
    return snapshot?.instrument?.name || state.selectedInstrument || "";
  }

  function inferStrategyPointSize(prompt = "", spec = {}) {
    const text = String(prompt || "");
    const example = text.match(/\blong\s+(\d+(?:[,.]\d+)?)\s+sl\s+(\d+(?:[,.]\d+)?)/i)
      || text.match(/\bshort\s+(\d+(?:[,.]\d+)?)\s+sl\s+(\d+(?:[,.]\d+)?)/i);
    const slPoints = text.match(/\bsl\b[^\d-]*(\d+(?:[,.]\d+)?)\s*p\b/i);
    if (example && slPoints) {
      const entry = parseStrategyDecimal(example[1]);
      const stop = parseStrategyDecimal(example[2]);
      const points = parseStrategyDecimal(slPoints[1]);
      if (Number.isFinite(entry) && Number.isFinite(stop) && Number.isFinite(points) && points > 0) {
        const inferred = Math.abs(entry - stop) / points;
        if (Number.isFinite(inferred) && inferred > 0) return inferred;
      }
    }
    const instrument = String(spec.instrument || state.selectedInstrument || "").toUpperCase();
    if (instrument.includes("NATGAS")) return 0.01;
    if (instrument.includes("JPY")) return 0.01;
    if (instrument.includes("_USD") || instrument.includes("USD_")) return 0.0001;
    return null;
  }

  function brokerInstrumentSummary(instrument = "") {
    const target = String(instrument || "").trim();
    if (!target) return null;
    const sources = [
      ...(Array.isArray(state.snapshot?.instruments) ? state.snapshot.instruments : []),
      ...(Array.isArray(state.assetCatalog) ? state.assetCatalog : []),
    ];
    for (const item of sources) {
      const name = String(item?.name || item?.instrument || "").trim();
      if (name && name.toUpperCase() === target.toUpperCase()) return item;
    }
    return null;
  }

  function resolveStrategyUnitSpec(instrument = "", prompt = "") {
    const target = String(instrument || "").trim();
    const summary = brokerInstrumentSummary(target);
    const pipLocation = Number(summary?.pipLocation ?? summary?.pip_location);
    if (Number.isFinite(pipLocation)) {
      const pointSize = 10 ** pipLocation;
      if (Number.isFinite(pointSize) && pointSize > 0) {
        return {
          pointSize,
          pointSizeSource: `oanda-pipLocation:${pipLocation}`,
          pointSizeWarning: "",
        };
      }
    }
    const inferred = inferStrategyPointSize(prompt, { instrument: target });
    if (Number.isFinite(inferred) && inferred > 0) {
      return {
        pointSize: inferred,
        pointSizeSource: target === DEFAULT_INSTRUMENT ? "default-natgas-oanda-pipLocation:-2" : "prompt-or-class-inferred",
        pointSizeWarning: target === DEFAULT_INSTRUMENT ? "" : "Point size inferred because broker metadata is not loaded.",
      };
    }
    return {
      pointSize: null,
      pointSizeSource: "",
      pointSizeWarning: "Point size is unresolved.",
    };
  }

  function normalizeStrategyMetricToken(value = "") {
    const raw = String(value || "").trim();
    if (!raw) return "";
    return raw
      .replace(/^\/+/, "")
      .replace(/-/g, "_")
      .replace(/lowerband/gi, "lower_band")
      .replace(/upperband/gi, "upper_band")
      .replace(/middleband|midband/gi, "basis")
      .replace(/[^A-Za-z0-9_]/g, "_")
      .replace(/_+/g, "_")
      .replace(/^_|_$/g, "")
      .toLowerCase();
  }

  function extractStrategyMappedRefs(prompt = "", snapshot = null) {
    const text = String(prompt || "");
    const candleRefs = [];
    const indicatorRefs = [];
    const seenCandles = new Set();
    const seenIndicators = new Set();
    const addCandle = (token) => {
      const normalized = normalizeStrategyMetricToken(token);
      if (normalized && !seenCandles.has(normalized)) {
        seenCandles.add(normalized);
        candleRefs.push(normalized);
      }
    };
    const addIndicator = (token) => {
      const normalized = normalizeStrategyMetricToken(token);
      if (normalized && !seenIndicators.has(normalized)) {
        seenIndicators.add(normalized);
        indicatorRefs.push(normalized);
      }
    };
    for (const match of text.matchAll(/\/?candle(?:_)?([smhdw]\d+)?[_-]?(\d{1,2})(am|pm)?\b/gi)) {
      addCandle(match[0]);
    }
    for (const match of text.matchAll(/\/[A-Za-z][A-Za-z0-9_]*(?:_[A-Za-z0-9]+)*/g)) {
      const token = match[0];
      const compact = normalizeStrategyMetricToken(token);
      if (compact.startsWith("candle")) addCandle(compact);
      if (TRADING_INDICATOR_LIBRARY.some((item) => compact.includes(item.id)) || /bollinger|lower_band|upper_band|basis|vwap|ema|sma|rsi|atr/.test(compact)) {
        addIndicator(compact);
      }
    }
    for (const match of text.matchAll(/\b(?:bollinger|vwap|ema|sma|rsi|atr|keltner|donchian)[A-Za-z0-9_]*(?:lowerband|lower_band|upperband|upper_band|basis|cloud)?\b/gi)) {
      addIndicator(match[0]);
    }
    if (/\/indicators\b/i.test(text)) {
      for (const indicator of Array.isArray(state.activeIndicators) ? state.activeIndicators : []) {
        if (indicator?.visible === false) continue;
        addIndicator(indicator.command || indicator.id || "");
      }
      const active = snapshot?.indicators?.active || [];
      for (const indicator of Array.isArray(active) ? active : []) addIndicator(indicator.command || indicator.id || "");
    }
    if (/\/backtest_\b/i.test(text)) {
      addIndicator("strategy_backtest");
    }
    return { candleRefs, indicatorRefs };
  }

  function applyMappedCandleRefToSpec(spec = {}, candleRefs = []) {
    for (const ref of Array.isArray(candleRefs) ? candleRefs : []) {
      const match = String(ref || "").match(/^candle_?([smhdw]\d+)?_?(\d{1,2})(am|pm)?$/i);
      if (!match) continue;
      const timeframe = String(match[1] || "").toUpperCase();
      let hour = Number(match[2]);
      const suffix = String(match[3] || "").toLowerCase();
      if (suffix === "pm" && hour < 12) hour += 12;
      if (suffix === "am" && hour === 12) hour = 0;
      if (timeframe && !spec.granularity) spec.granularity = timeframe;
      if (Number.isInteger(hour) && hour >= 0 && hour <= 23 && spec.entryHour == null) spec.entryHour = hour;
    }
  }

  function buildStrategyMetricCommandsFromSpec(spec = {}) {
    const seen = new Set();
    const out = [];
    const push = (token) => {
      const normalized = normalizeStrategyMetricToken(token);
      if (!normalized || seen.has(normalized)) return;
      seen.add(normalized);
      out.push(`/${normalized}`);
    };
    push("/asset");
    if (spec.instrument) push(`/asset_${String(spec.instrument).toLowerCase()}`);
    if (spec.granularity) push(`/candle_${String(spec.granularity).toLowerCase()}`);
    for (const ref of Array.isArray(spec.candleRefs) ? spec.candleRefs : []) push(ref);
    for (const ref of Array.isArray(spec.indicatorRefs) ? spec.indicatorRefs : []) push(ref);
    if (spec.lowVolatilityMetric) push(spec.lowVolatilityMetric);
    if (spec.forceDailyEntry) push("/strategy_daily_21h_entry");
    push("/strategy_indicator_feature_bank");
    push("/strategy_paired_long_short");
    push("/strategy_tp_grid");
    return out;
  }

  function extractTradingStrategySpecFromPrompt(prompt = "", snapshot = null) {
    const text = String(prompt || "");
    const lower = text.toLowerCase();
    const granularity = commandGranularityFromText(text) || snapshot?.timeframe?.granularity || state.selectedGranularity || "";
    const instrument = inferStrategyInstrumentFromPrompt(text, snapshot);
    const mappedRefs = extractStrategyMappedRefs(text, snapshot);
    const unit = resolveStrategyUnitSpec(instrument, text);
    const spec = {
      instrument,
      granularity,
      broker: /\boanda\b/i.test(text) ? "oanda" : (snapshot?.broker?.selectedKind || selectedBrokerKind() || ""),
      pointSize: unit.pointSize,
      pointSizeSource: unit.pointSizeSource,
      pointSizeWarning: unit.pointSizeWarning,
      entryHour: null,
      entryTimezone: null,
      direction: /\b(short|sell|vente|vendeur)\b/i.test(text)
        ? "short"
        : (/\b(long|buy|achat|acheteur)\b/i.test(text) ? "long" : "both"),
      stopLossDistance: null,
      takeProfitMinDistance: null,
      takeProfitMaxDistance: null,
      targetWinRate: null,
      lowVolatilityMetric: null,
      lowVolatilityLookback: null,
      lowVolatilityPercentile: null,
      forceDailyEntry: /(?:1|un)\s+trade\s+(?:tous\s+les\s+jours|chaque\s+jour)|tous\s+les\s+jours\s+de\s+trading|every\s+(?:open\s+trading\s+)?day|daily\s+trade/i.test(text),
      spreadCostDistance: null,
      slippageDistance: null,
      maxHoldBars: null,
      trainTestSplit: 0.7,
      candleRefs: mappedRefs.candleRefs,
      indicatorRefs: mappedRefs.indicatorRefs,
      sourceText: text.trim(),
    };
    applyMappedCandleRefToSpec(spec, mappedRefs.candleRefs);
    const hour = text.match(/\b([01]?\d|2[0-3])\s*h(?:eure)?\b/i)
      || text.match(/\b([01]?\d|2[0-3]):00\b/);
    if (hour) spec.entryHour = Number(hour[1]);
    if (/\butc\b|\bzulu\b/i.test(text)) spec.entryTimezone = "UTC";
    else if (/\boanda\s+(?:time|hour|heure)|serveur|server/i.test(text)) spec.entryTimezone = "OANDA";
    else if (/paris|europe\/paris|heure francaise|heure française/i.test(text)) spec.entryTimezone = "Europe/Paris";

    const pointSize = Number.isFinite(unit.pointSize) && unit.pointSize > 0 ? unit.pointSize : inferStrategyPointSize(text, spec);
    const example = text.match(/\blong\s+(\d+(?:[,.]\d+)?)\s+sl\s+(\d+(?:[,.]\d+)?)/i)
      || text.match(/\bshort\s+(\d+(?:[,.]\d+)?)\s+sl\s+(\d+(?:[,.]\d+)?)/i);
    if (example) {
      const entry = parseStrategyDecimal(example[1]);
      const stop = parseStrategyDecimal(example[2]);
      if (Number.isFinite(entry) && Number.isFinite(stop)) spec.stopLossDistance = Math.abs(entry - stop);
    }
    const slPoints = text.match(/\bsl\b[^\d-]*(\d+(?:[,.]\d+)?)\s*p\b/i);
    if (!Number.isFinite(spec.stopLossDistance) && slPoints && pointSize) {
      spec.stopLossDistance = parseStrategyDecimal(slPoints[1]) * pointSize;
    }
    const tpMin = text.match(/\btp\b[^\d-]*(\d+(?:[,.]\d+)?)\s*p\b/i);
    if (tpMin && pointSize) spec.takeProfitMinDistance = parseStrategyDecimal(tpMin[1]) * pointSize;
    const tpMax = text.match(/(\d+(?:[,.]\d+)?)\s*(?:points?|p)\s*(?:max|maximum)\b/i)
      || text.match(/\bmax(?:imum)?[^\d]*(\d+(?:[,.]\d+)?)\s*(?:points?|p)\b/i);
    if (tpMax && pointSize) spec.takeProfitMaxDistance = parseStrategyDecimal(tpMax[1]) * pointSize;
    if (Number.isFinite(spec.takeProfitMinDistance) && !Number.isFinite(spec.takeProfitMaxDistance)) {
      spec.takeProfitMaxDistance = spec.takeProfitMinDistance;
    }
    const target = text.match(/(\d+(?:[,.]\d+)?)\s*%\s*(?:minimum|min|de\s+succ|success|win|réussite|reussite)?/i);
    if (target) {
      const value = parseStrategyDecimal(target[1]);
      if (Number.isFinite(value) && value > 0) spec.targetWinRate = value > 1 ? value / 100 : value;
    }
    if (/faible\s+vol|low\s+vol|volatil/i.test(lower)) {
      if (/bollinger/i.test(text) || mappedRefs.indicatorRefs.some((item) => item.includes("bollinger"))) spec.lowVolatilityMetric = "bollinger_width_percentile";
      else if (/\batr\b/i.test(text)) spec.lowVolatilityMetric = "atr_percentile";
      else if (/\brange\b|bougie|candle/i.test(text)) spec.lowVolatilityMetric = "range_sma_percentile";
    }
    if (spec.lowVolatilityMetric) {
      const lookback = text.match(/\b(?:lookback|sma|atr|moyenne)\s*(\d{1,3})\b/i);
      spec.lowVolatilityLookback = lookback ? Number(lookback[1]) : 24;
      const percentileMatch = text.match(/percentile\s*(\d+(?:[,.]\d+)?)/i);
      spec.lowVolatilityPercentile = percentileMatch ? (parseStrategyDecimal(percentileMatch[1]) / 100) : 0.25;
    }
    const spread = text.match(/\bspread[^\d]*(\d+(?:[,.]\d+)?)/i);
    if (spread) spec.spreadCostDistance = parseStrategyDecimal(spread[1]);
    const slippage = text.match(/\bslippage[^\d]*(\d+(?:[,.]\d+)?)/i);
    if (slippage) spec.slippageDistance = parseStrategyDecimal(slippage[1]);
    if (/couts?\s+a?\s*zero|coûts?\s+à\s+z[eé]ro|costs?\s+zero/i.test(text)) {
      spec.spreadCostDistance = 0;
      spec.slippageDistance = 0;
    }
    const hold = text.match(/\b(?:max\s*hold|tenir|max\s+(\d+)\s+bougies?)\s*(\d{1,4})?\b/i);
    const holdValue = hold ? Number(hold[2] || hold[1] || 0) : 0;
    if (holdValue > 0) spec.maxHoldBars = holdValue;
    spec.metricCommands = buildStrategyMetricCommandsFromSpec(spec);
    return spec;
  }

  function localValidateTradingStrategySpec(spec = {}) {
    const missing = [];
    const push = (id, label, question, reason, examples = []) => {
      missing.push({ id, label, question, reason, examples });
    };
    if (!String(spec.instrument || "").trim()) push("instrument", "Asset", "Quel actif exact faut-il tester ?", "Le runner doit charger un historique precis.", ["NATGAS_USD"]);
    if (!String(spec.granularity || "").trim()) push("granularity", "Timeframe", "Quel timeframe faut-il utiliser ?", "Le pas de bougie change les entrees.", ["H1"]);
    if (!Number.isInteger(spec.entryHour) || spec.entryHour < 0 || spec.entryHour > 23) push("entry_hour", "Entry hour", "A quelle heure exacte faut-il ouvrir le trade ?", "La strategie est horaire.", ["21h UTC"]);
    if (!/^(utc|z|oanda|server|serveur|exchange)$/i.test(String(spec.entryTimezone || ""))) push("entry_timezone", "Timezone", "Le 21h est-il en UTC/OANDA, heure de Paris, ou autre ?", "Les historiques OANDA sont en UTC.", ["21h UTC"]);
    if (!/^(long|short|buy|sell|both|auto)$/i.test(String(spec.direction || ""))) push("direction", "Direction", "Faut-il tester long, short, ou les deux ?", "Le runner doit savoir quelles directions sont autorisees.", ["both"]);
    if (!(Number.isFinite(spec.stopLossDistance) && spec.stopLossDistance > 0)) push("stop_loss_distance", "Stop loss", "Quelle distance de stop normalisee faut-il utiliser ?", "Les pips/points sont ambigus selon l'actif.", ["0.045"]);
    if (!(Number.isFinite(spec.takeProfitMinDistance) && spec.takeProfitMinDistance > 0 && Number.isFinite(spec.takeProfitMaxDistance) && spec.takeProfitMaxDistance > 0)) push("take_profit_distance", "Take profit", "Quelle plage TP min/max faut-il tester ?", "Le runner teste une grille bornee.", ["0.035 a 0.300"]);
    if (!(Number.isFinite(spec.targetWinRate) && spec.targetWinRate > 0 && spec.targetWinRate < 1)) push("target_win_rate", "Win rate", "Quel taux de reussite cible faut-il atteindre ?", "Le seuil doit etre numerique.", ["85%"]);
    if (!/^(range_sma_percentile|atr_percentile|bollinger_width_percentile)$/i.test(String(spec.lowVolatilityMetric || ""))) push("low_volatility_metric", "Low volatility", "Comment mesurer la faible volatilite avant l'entree ?", "Forge ne doit pas inventer la metrique centrale.", ["range SMA 24 sous percentile 25", "Bollinger width 20 sous percentile 25"]);
    if (!(Number.isFinite(spec.pointSize) && spec.pointSize > 0)) push("point_size", "Point size", "Quelle taille de point/pip faut-il appliquer a cet actif ?", "Les distances en pips/points doivent etre resolues par broker metadata.", ["NATGAS_USD pipLocation -2"]);
    if (spec.spreadCostDistance == null || spec.slippageDistance == null) push("execution_costs", "Execution costs", "Quel spread/slippage faut-il appliquer ?", "Un stop court sans couts donne un faux resultat.", ["spread 0.002 slippage 0.001", "couts a zero"]);
    return missing;
  }

  function strategySignalAt(rows = [], indicators = {}, index = 0, rules = {}) {
    const row = rows[index];
    const prev = rows[index - 1];
    if (!row || !prev) return { signal: "HOLD", confidence: 0, reason: "not enough candles" };
    const fast = indicators.fast?.[index];
    const slow = indicators.slow?.[index];
    const prevFast = indicators.fast?.[index - 1];
    const prevSlow = indicators.slow?.[index - 1];
    const atr = indicators.atr?.[index];
    const rsi = indicators.rsi?.[index];
    const confidenceFromAtr = Number.isFinite(atr) && row.close
      ? clamp((atr / row.close) * 180, 0.1, 0.85)
      : 0.35;
    if (rules.kind === "rsi_reversion") {
      if (Number.isFinite(rsi) && rsi <= 31 && row.close >= prev.close) {
        return { signal: "BUY", confidence: 0.62 + confidenceFromAtr * 0.18, reason: `RSI ${formatNumber(rsi, 1)} rebound near oversold zone` };
      }
      if (Number.isFinite(rsi) && rsi >= 69 && row.close <= prev.close) {
        return { signal: "SELL", confidence: 0.62 + confidenceFromAtr * 0.18, reason: `RSI ${formatNumber(rsi, 1)} rejection near overbought zone` };
      }
      return { signal: "HOLD", confidence: 0.34, reason: Number.isFinite(rsi) ? `RSI neutral at ${formatNumber(rsi, 1)}` : "RSI warming up" };
    }
    if (rules.kind === "breakout") {
      const high = rollingHigh(rows, index, rules.breakoutLookback || 24);
      const low = rollingLow(rows, index, rules.breakoutLookback || 24);
      if (high != null && row.close > high) {
        return { signal: "BUY", confidence: 0.68 + confidenceFromAtr * 0.2, reason: `close broke ${rules.breakoutLookback || 24}-bar high` };
      }
      if (low != null && row.close < low) {
        return { signal: "SELL", confidence: 0.68 + confidenceFromAtr * 0.2, reason: `close broke ${rules.breakoutLookback || 24}-bar low` };
      }
      return { signal: "HOLD", confidence: 0.35, reason: "breakout trigger not active" };
    }
    if ([fast, slow, prevFast, prevSlow].every(Number.isFinite)) {
      if (prevFast <= prevSlow && fast > slow) {
        return { signal: "BUY", confidence: 0.72 + confidenceFromAtr * 0.12, reason: `EMA ${rules.fastPeriod || 9}/${rules.slowPeriod || 21} bullish cross` };
      }
      if (prevFast >= prevSlow && fast < slow) {
        return { signal: "SELL", confidence: 0.72 + confidenceFromAtr * 0.12, reason: `EMA ${rules.fastPeriod || 9}/${rules.slowPeriod || 21} bearish cross` };
      }
      if (fast > slow && row.close > fast && row.close > prev.close) {
        return { signal: "BUY", confidence: 0.55 + confidenceFromAtr * 0.14, reason: "trend up, price holding above fast EMA" };
      }
      if (fast < slow && row.close < fast && row.close < prev.close) {
        return { signal: "SELL", confidence: 0.55 + confidenceFromAtr * 0.14, reason: "trend down, price holding below fast EMA" };
      }
    }
    return { signal: "HOLD", confidence: 0.3, reason: "EMA trend not decisive yet" };
  }

  function strategyBacktestKey(candles = state.candles, rules = {}) {
    const rows = Array.isArray(candles) ? candles : [];
    const first = rows[0] || {};
    const last = rows[rows.length - 1] || {};
    return [
      state.selectedInstrument,
      state.selectedGranularity,
      rows.length,
      first.time || "",
      last.time || "",
      simpleStableHash(JSON.stringify(rules || {})),
    ].join("|");
  }

  function backtestTradingStrategy(candles = state.candles, rules = {}) {
    const rows = strategyCandleRows(candles, 2500);
    const minBars = Math.max(60, (rules.slowPeriod || 21) + 10, (rules.breakoutLookback || 24) + 4);
    if (rows.length < minBars) {
      return {
        status: "waiting_for_history",
        bars: rows.length,
        trades: 0,
        wins: 0,
        losses: 0,
        winRate: null,
        pnlPct: 0,
        maxDrawdownPct: 0,
        expectancyPct: 0,
        lastSignal: "HOLD",
        note: `Need at least ${minBars} candles for a meaningful test.`,
      };
    }
    const indicators = makeStrategyIndicators(rows, rules);
    const trades = [];
    let position = null;
    let equity = 0;
    let highWater = 0;
    let maxDrawdownPct = 0;
    const closePosition = (index, price, reason) => {
      if (!position || !Number.isFinite(price) || price <= 0) return;
      const pnlPct = position.side === "BUY"
        ? ((price - position.entry) / position.entry) * 100
        : ((position.entry - price) / position.entry) * 100;
      equity += pnlPct;
      highWater = Math.max(highWater, equity);
      maxDrawdownPct = Math.max(maxDrawdownPct, highWater - equity);
      trades.push({
        side: position.side,
        entry: position.entry,
        exit: price,
        entryTime: position.entryTime,
        exitTime: rows[index]?.time || "",
        pnlPct,
        barsHeld: index - position.entryIndex,
        reason,
      });
      position = null;
    };
    for (let i = minBars + 1; i < rows.length; i += 1) {
      const row = rows[i];
      const signalIndex = i - 1;
      const signal = strategySignalAt(rows, indicators, signalIndex, rules);
      const atr = indicators.atr?.[signalIndex] || Math.max(row.high - row.low, row.close * 0.002);
      if (position) {
        const held = i - position.entryIndex;
        if (position.side === "BUY") {
          if (row.low <= position.stop) closePosition(i, position.stop, "atr_stop");
          else if (row.high >= position.target) closePosition(i, position.target, "atr_target");
          else if (signal.signal === "SELL") closePosition(i, row.close, "opposite_signal");
          else if (held >= (rules.maxHoldBars || 48)) closePosition(i, row.close, "max_hold");
        } else {
          if (row.high >= position.stop) closePosition(i, position.stop, "atr_stop");
          else if (row.low <= position.target) closePosition(i, position.target, "atr_target");
          else if (signal.signal === "BUY") closePosition(i, row.close, "opposite_signal");
          else if (held >= (rules.maxHoldBars || 48)) closePosition(i, row.close, "max_hold");
        }
      }
      if (!position && (signal.signal === "BUY" || signal.signal === "SELL")) {
        const entryPrice = Number.isFinite(row.open) ? row.open : row.close;
        const stopDistance = Math.max(atr * (rules.riskAtr || 1.4), entryPrice * 0.001);
        const targetDistance = stopDistance * (rules.rewardRisk || 1.8);
        position = {
          side: signal.signal,
          entry: entryPrice,
          entryTime: row.time,
          entryIndex: i,
          stop: signal.signal === "BUY" ? entryPrice - stopDistance : entryPrice + stopDistance,
          target: signal.signal === "BUY" ? entryPrice + targetDistance : entryPrice - targetDistance,
        };
      }
    }
    if (position) closePosition(rows.length - 1, rows[rows.length - 1].close, "mark_to_market");
    const wins = trades.filter((trade) => trade.pnlPct > 0).length;
    const losses = trades.filter((trade) => trade.pnlPct <= 0).length;
    const pnlPct = trades.reduce((sum, trade) => sum + trade.pnlPct, 0);
    const lastSignal = strategySignalAt(rows, indicators, rows.length - 1, rules);
    return {
      status: "tested",
      bars: rows.length,
      trades: trades.length,
      wins,
      losses,
      winRate: trades.length ? wins / trades.length : null,
      pnlPct,
      maxDrawdownPct,
      expectancyPct: trades.length ? pnlPct / trades.length : 0,
      lastSignal: lastSignal.signal,
      lastReason: lastSignal.reason,
      sampleTrades: trades.slice(-5),
      note: "Paper backtest on currently loaded candles; no broker order was placed.",
    };
  }

  function evaluateTradingStrategyLive(candles = state.candles, rules = {}) {
    const rows = strategyCandleRows(candles, 320);
    const minBars = Math.max(40, (rules.slowPeriod || 21) + 8);
    if (rows.length < minBars) {
      return {
        status: "warming_up",
        signal: "HOLD",
        confidence: 0,
        bars: rows.length,
        lastCandleTime: "",
        reason: `Waiting for ${minBars} candles.`,
      };
    }
    const indicators = makeStrategyIndicators(rows, rules);
    const index = rows.length - 1;
    const row = rows[index];
    const signal = strategySignalAt(rows, indicators, index, rules);
    const atr = indicators.atr?.[index] || Math.max(row.high - row.low, row.close * 0.002);
    const stopDistance = Math.max(atr * (rules.riskAtr || 1.4), row.close * 0.001);
    const targetDistance = stopDistance * (rules.rewardRisk || 1.8);
    return {
      status: "watching",
      signal: signal.signal,
      confidence: clamp(signal.confidence || 0, 0, 0.98),
      bars: rows.length,
      lastCandleTime: row.time,
      lastClose: row.close,
      reason: signal.reason,
      paperStop: signal.signal === "BUY" ? row.close - stopDistance : signal.signal === "SELL" ? row.close + stopDistance : null,
      paperTarget: signal.signal === "BUY" ? row.close + targetDistance : signal.signal === "SELL" ? row.close - targetDistance : null,
    };
  }

  function summarizeStrategyLabState() {
    const lab = state.strategyLab || {};
    const backtest = lab.backtest || {};
    const live = lab.live || {};
    const runnerBest = lab.runner?.result?.best || lab.runner?.best || null;
    const visualProbes = Array.isArray(lab.visualProbes)
      ? lab.visualProbes
      : Array.isArray(backtest.visualProbes)
        ? backtest.visualProbes
        : Array.isArray(lab.runner?.result?.visualProbes)
          ? lab.runner.result.visualProbes
          : [];
    return {
      active: !!lab.active,
      status: lab.status || "idle",
      title: lab.title || "",
      source: lab.source || "",
      instrument: lab.instrument || state.selectedInstrument,
      granularity: lab.granularity || state.selectedGranularity,
      prompt: lab.prompt || lab.spec?.sourceText || "",
      spec: lab.spec || null,
      missingMetrics: Array.isArray(lab.missingMetrics) ? lab.missingMetrics : [],
      questions: Array.isArray(lab.questions) ? lab.questions : [],
      runner: lab.runner || null,
      liveJob: lab.liveJob || null,
      plan: Array.isArray(lab.plan) ? lab.plan : [],
      computePlan: lab.computePlan || backtest.computePlan || lab.runner?.result?.computePlan || null,
      pairedProbe: lab.pairedProbe || backtest.pairedProbe || lab.runner?.result?.pairedProbe || null,
      visualProbes,
      rules: lab.rules ? {
        id: lab.rules.id,
        title: lab.rules.title,
        kind: lab.rules.kind,
        fastPeriod: lab.rules.fastPeriod,
        slowPeriod: lab.rules.slowPeriod,
        atrPeriod: lab.rules.atrPeriod,
        rsiPeriod: lab.rules.rsiPeriod,
        breakoutLookback: lab.rules.breakoutLookback,
        riskAtr: lab.rules.riskAtr,
        rewardRisk: lab.rules.rewardRisk,
        maxHoldBars: lab.rules.maxHoldBars,
      } : null,
      backtest: {
        status: backtest.status || "idle",
        bars: Number(backtest.bars || 0),
        trades: Number(backtest.trades || 0),
        winRate: backtest.winRate == null ? null : Number(backtest.winRate),
        pnlPct: Number(backtest.pnlPct || 0),
        maxDrawdownPct: Number(backtest.maxDrawdownPct || 0),
        expectancyPct: Number(backtest.expectancyPct || 0),
        pnlDistance: backtest.pnlDistance == null ? null : Number(backtest.pnlDistance),
        expectancyDistance: backtest.expectancyDistance == null ? null : Number(backtest.expectancyDistance),
        direction: backtest.direction || runnerBest?.direction || "",
        takeProfitDistance: backtest.takeProfitDistance == null && runnerBest?.takeProfitDistance == null
          ? null
          : Number(backtest.takeProfitDistance ?? runnerBest?.takeProfitDistance),
        robustness: backtest.robustness || runnerBest?.robustness || null,
        robustnessScore: backtest.robustnessScore == null && runnerBest?.robustness?.score == null
          ? null
          : Number(backtest.robustnessScore ?? runnerBest?.robustness?.score),
        robustnessGrade: backtest.robustnessGrade || runnerBest?.robustness?.grade || "",
        computePlan: backtest.computePlan || lab.runner?.result?.computePlan || null,
        pairedProbe: backtest.pairedProbe || lab.runner?.result?.pairedProbe || null,
        visualProbes,
        note: String(backtest.note || ""),
      },
      live: {
        status: live.status || "idle",
        signal: live.signal || "HOLD",
        confidence: Number(live.confidence || 0),
        lastCandleTime: live.lastCandleTime || "",
        lastClose: Number(live.lastClose || 0),
        reason: live.reason || "",
        paperStop: live.paperStop == null ? null : Number(live.paperStop),
        paperTarget: live.paperTarget == null ? null : Number(live.paperTarget),
      },
      createdAt: lab.createdAtMs ? new Date(lab.createdAtMs).toISOString() : "",
      updatedAt: lab.updatedAtMs ? new Date(lab.updatedAtMs).toISOString() : "",
    };
  }

  function refreshStrategyLiveTest(reason = "refresh") {
    const lab = state.strategyLab || {};
    if (!lab.active || !lab.rules) return null;
    const rows = Array.isArray(state.candles) ? state.candles : [];
    const last = rows[rows.length - 1] || {};
    const liveKey = [
      state.selectedInstrument,
      state.selectedGranularity,
      rows.length,
      last.time || "",
      last.close || "",
      simpleStableHash(JSON.stringify(lab.rules || {})),
    ].join("|");
    if (lab.liveKey === liveKey) return lab.live;
    const previousSignal = lab.live?.signal || "";
    const previousTime = lab.live?.lastCandleTime || "";
    const backtestKey = strategyBacktestKey(rows, lab.rules);
    const backtest = lab.backtestKey === backtestKey
      ? lab.backtest
      : backtestTradingStrategy(rows, lab.rules);
    const live = evaluateTradingStrategyLive(rows, lab.rules);
    state.strategyLab = {
      ...lab,
      status: live.status === "watching" ? "live_testing" : "warming_up",
      instrument: state.selectedInstrument,
      granularity: state.selectedGranularity,
      backtest,
      backtestKey,
      live,
      liveKey,
      updatedAtMs: tradingNowMs(),
    };
    strategyRuntimeRevision += 1;
    invalidateContextCaches();
    const changed = previousSignal !== live.signal || previousTime !== live.lastCandleTime;
    if (changed && state.active) {
      canvasBridge()?.refreshRightPanel?.();
    }
    return live;
  }

  function strategyLiveJobTickKey(lab = {}) {
    const rows = Array.isArray(state.candles) ? state.candles : [];
    const last = rows[rows.length - 1] || {};
    return [
      lab.liveJob?.jobId || lab.backtestKey || simpleStableHash(JSON.stringify(lab.spec || {})),
      state.selectedInstrument,
      state.selectedGranularity,
      rows.length,
      last.time || "",
      lab.runner?.result?.lowVolatilityThreshold ?? "",
      lab.backtest?.direction || lab.runner?.result?.best?.direction || "",
      lab.backtest?.takeProfitDistance ?? lab.runner?.result?.best?.takeProfitDistance ?? "",
    ].join("|");
  }

  async function refreshStrategyLiveJob(reason = "refresh") {
    const lab = state.strategyLab || {};
    if (!lab.active || !lab.spec || !lab.runner?.result || !hasTauriInvoke()) return null;
    const result = lab.runner.result;
    const best = result.best || null;
    const lowVolatilityThreshold = Number(result.lowVolatilityThreshold);
    const direction = String(lab.backtest?.direction || best?.direction || "").trim().toLowerCase();
    const takeProfitDistance = Number(lab.backtest?.takeProfitDistance ?? best?.takeProfitDistance);
    if (!Number.isFinite(lowVolatilityThreshold) || !direction || !Number.isFinite(takeProfitDistance)) return null;
    const tickKey = strategyLiveJobTickKey(lab);
    if (lab.liveJob?.tickKey === tickKey || lab.liveJob?.pendingKey === tickKey) return lab.liveJob;
    state.strategyLab = {
      ...lab,
      liveJob: {
        ...(lab.liveJob || {}),
        pendingKey: tickKey,
        status: "pending",
      },
    };
    try {
      const response = await invoke("trading_strategy_live_tick", {
        request: {
          jobId: lab.liveJob?.jobId || "",
          spec: lab.spec,
          lowVolatilityThreshold,
          direction,
          takeProfitDistance,
          maxRows: Math.max(180, Number(lab.spec.lowVolatilityLookback || 24) + Number(lab.spec.maxHoldBars || 24) + 96),
        },
      });
      const current = state.strategyLab || {};
      if (!current.active || !current.spec) return response;
      const newSignal = response?.newSignal || null;
      const openSignals = Array.isArray(response?.openSignals) ? response.openSignals : [];
      const closedSignals = Array.isArray(response?.closedSignals) ? response.closedSignals : [];
      const latestOpen = openSignals[openSignals.length - 1] || null;
      const focusSignal = newSignal || latestOpen || closedSignals[closedSignals.length - 1] || null;
      const live = {
        ...(current.live || {}),
        status: response?.status === "updated" ? "journal_updated" : "watching_incremental",
        signal: focusSignal?.direction ? String(focusSignal.direction).toUpperCase() : (current.live?.signal || "HOLD"),
        confidence: current.backtest?.winRate == null ? Number(current.live?.confidence || 0) : clamp(Number(current.backtest.winRate), 0, 0.99),
        bars: Array.isArray(state.candles) ? state.candles.length : 0,
        lastCandleTime: response?.evaluatedTime || current.live?.lastCandleTime || "",
        lastClose: Number(state.candles?.[state.candles.length - 1]?.close || current.live?.lastClose || 0),
        reason: newSignal
          ? "new incremental strategy signal recorded"
          : closedSignals.length
            ? `${closedSignals.length} strategy signal(s) resolved`
            : "incremental live journal watching new candles",
        paperStop: focusSignal?.stopPrice ?? current.live?.paperStop ?? null,
        paperTarget: focusSignal?.takeProfitPrice ?? current.live?.paperTarget ?? null,
      };
      const note = [
        `${new Date().toISOString()} live_tick status=${response?.status || "n/a"} evaluated=${response?.evaluatedTime || "n/a"}`,
        newSignal ? `new=${newSignal.direction}@${newSignal.entryTime}` : "",
        closedSignals.length ? `closed=${closedSignals.map((item) => `${item.outcome || "closed"}@${item.exitTime || "n/a"}`).join(",")}` : "",
      ].filter(Boolean).join(" ");
      state.strategyLab = {
        ...current,
        status: current.status === "target_met" ? "target_met" : "live_testing",
        live,
        liveJob: {
          ...response,
          tickKey,
          pendingKey: "",
          updatedAtMs: tradingNowMs(),
        },
        logs: [note, ...(Array.isArray(current.logs) ? current.logs : [])].slice(0, 24),
        updatedAtMs: tradingNowMs(),
      };
      strategyRuntimeRevision += 1;
      invalidateContextCaches();
      if (response?.status === "updated" && state.active) canvasBridge()?.refreshRightPanel?.();
      return response;
    } catch (error) {
      const current = state.strategyLab || {};
      state.strategyLab = {
        ...current,
        liveJob: {
          ...(current.liveJob || {}),
          tickKey: "",
          pendingKey: "",
          status: "error",
          error: String(error?.message || error || "strategy live tick failed"),
          updatedAtMs: tradingNowMs(),
        },
      };
      return null;
    }
  }

  function strategyLabContextPacket() {
    const strategy = summarizeStrategyLabState();
    if (!strategy.active && strategy.status !== "needs_clarification" && strategy.status !== "runner_unavailable") return "";
    if (strategy.status === "needs_clarification") {
      return [
        "TRADING_STRATEGY_LAB:v2",
        "status=needs_clarification",
        "The user requested /create_ /strategy_. Forge parsed a StrategySpec but refused to backtest because key metrics are ambiguous.",
      "Do not invent missing metrics. Ask for the missing items first, then explain that Forge will run the Rust strategy runner after clarification.",
      `instrument=${strategy.spec?.instrument || "n/a"} timeframe=${strategy.spec?.granularity || "n/a"} broker=${strategy.spec?.broker || "n/a"}`,
      `unit=point_size:${strategy.spec?.pointSize ?? "n/a"} source:${strategy.spec?.pointSizeSource || "n/a"}`,
      `chart_refs=candles:${(strategy.spec?.candleRefs || []).join(",") || "none"} indicators:${(strategy.spec?.indicatorRefs || []).join(",") || "none"}`,
      `slash_metrics=${(strategy.spec?.metricCommands || []).join(" ") || "none"}`,
      `missing=${strategy.missingMetrics.map((item) => item.id || item.label).join(",") || "n/a"}`,
      `questions=${strategy.questions.join(" | ") || "n/a"}`,
      ].join("\n");
    }
    return [
      "TRADING_STRATEGY_LAB:v2",
      `status=${strategy.status || "tested"}`,
      "The user requested /create_ /strategy_. Forge parsed a StrategySpec and delegated the candle scan/backtest to the native Rust runner.",
      "Do not claim that broker orders were sent. Treat this as a live paper test unless the user explicitly asks for execution.",
      `strategy=${strategy.title || "n/a"} instrument=${strategy.instrument} timeframe=${strategy.granularity}`,
      `spec=entry_hour:${strategy.spec?.entryHour ?? "n/a"} timezone:${strategy.spec?.entryTimezone || "n/a"} force_daily:${strategy.spec?.forceDailyEntry ? "yes" : "no"} direction:${strategy.spec?.direction || "n/a"} sl:${formatNumber(strategy.spec?.stopLossDistance)} tp_min:${formatNumber(strategy.spec?.takeProfitMinDistance)} tp_max:${formatNumber(strategy.spec?.takeProfitMaxDistance)} target_win_rate:${strategy.spec?.targetWinRate == null ? "n/a" : formatNumber(strategy.spec.targetWinRate * 100, 1) + "%"} point_size:${strategy.spec?.pointSize ?? "n/a"} source:${strategy.spec?.pointSizeSource || "n/a"}`,
      `chart_refs=candles:${(strategy.spec?.candleRefs || []).join(",") || "none"} indicators:${(strategy.spec?.indicatorRefs || []).join(",") || "none"}`,
      `slash_metrics=${(strategy.spec?.metricCommands || []).join(" ") || "none"}`,
      `runner=${strategy.runner?.engine || "n/a"} plan=${strategy.plan.join(" ; ") || "n/a"}`,
      `kasm_plan=engine:${strategy.computePlan?.engine || "n/a"} id:${strategy.computePlan?.planId || "n/a"} mode:${strategy.computePlan?.executionMode || "n/a"} commands:${Array.isArray(strategy.computePlan?.metricCommands) ? strategy.computePlan.metricCommands.map((item) => item.token).join(" ") : "none"} cache:${Array.isArray(strategy.computePlan?.sharedCacheKeys) ? strategy.computePlan.sharedCacheKeys.join(",") : "none"} avoided_recalculations:${strategy.computePlan?.avoidedRecalculations ?? "n/a"}`,
      `strategy_template=id:${strategy.computePlan?.template?.templateId || "n/a"} hash:${strategy.computePlan?.template?.parameterHash || "n/a"} data:${strategy.computePlan?.template?.dataHash || "n/a"}`,
      `strategy_dag=nodes:${Array.isArray(strategy.computePlan?.dagNodes) ? strategy.computePlan.dagNodes.length : 0} hits:${strategy.computePlan?.cacheReport?.hits ?? "n/a"} misses:${strategy.computePlan?.cacheReport?.misses ?? "n/a"} injected:${strategy.computePlan?.cacheReport?.injectedResults ?? "n/a"} reused:${Array.isArray(strategy.computePlan?.cacheReport?.reusedNodes) ? strategy.computePlan.cacheReport.reusedNodes.join(",") : "none"}`,
      `strategy_gpu=kernel:${strategy.computePlan?.gpuPlan?.kernel || "n/a"} work_items:${strategy.computePlan?.gpuPlan?.workItems ?? strategy.computePlan?.simulationCount ?? "n/a"} outcome_cube:${strategy.computePlan?.outcomeCubeKey || strategy.computePlan?.gpuPlan?.outcomeCubeKey || "n/a"} required:${strategy.computePlan?.gpuPlan?.gpuRequired ?? "n/a"}`,
      `live_job=engine:${strategy.liveJob?.engine || "n/a"} status:${strategy.liveJob?.status || "n/a"} job:${strategy.liveJob?.jobId || "n/a"} evaluated:${strategy.liveJob?.evaluatedTime || "n/a"} open:${Array.isArray(strategy.liveJob?.openSignals) ? strategy.liveJob.openSignals.length : 0} closed_last_tick:${Array.isArray(strategy.liveJob?.closedSignals) ? strategy.liveJob.closedSignals.length : 0}`,
      `backtest=status:${strategy.backtest.status} bars:${strategy.backtest.bars} trades:${strategy.backtest.trades} direction:${strategy.backtest.direction || "n/a"} tp:${formatNumber(strategy.backtest.takeProfitDistance)} win_rate:${strategy.backtest.winRate == null ? "n/a" : formatNumber(strategy.backtest.winRate * 100, 1) + "%"} pnl_distance:${strategy.backtest.pnlDistance == null ? "n/a" : formatNumber(strategy.backtest.pnlDistance, 5)} expectancy_distance:${strategy.backtest.expectancyDistance == null ? "n/a" : formatNumber(strategy.backtest.expectancyDistance, 5)}`,
      `paired_probe=edge:${strategy.pairedProbe?.edge || "n/a"} score:${strategy.pairedProbe?.edgeScore == null ? "n/a" : formatNumber(strategy.pairedProbe.edgeScore, 2)} entries:${strategy.pairedProbe?.entries ?? "n/a"} long_wr:${strategy.pairedProbe?.longWinRate == null ? "n/a" : formatNumber(strategy.pairedProbe.longWinRate * 100, 1) + "%"} short_wr:${strategy.pairedProbe?.shortWinRate == null ? "n/a" : formatNumber(strategy.pairedProbe.shortWinRate * 100, 1) + "%"} cache:${strategy.pairedProbe?.sharedEntryScanCacheKey || "n/a"}`,
      `visual_probes=${Array.isArray(strategy.visualProbes) ? strategy.visualProbes.length : 0} latest:${Array.isArray(strategy.visualProbes) && strategy.visualProbes.length ? `${strategy.visualProbes[strategy.visualProbes.length - 1]?.direction || "n/a"}@${strategy.visualProbes[strategy.visualProbes.length - 1]?.entryTime || "n/a"}` : "n/a"}`,
      `robustness=score:${strategy.backtest.robustnessScore == null ? "n/a" : formatNumber(strategy.backtest.robustnessScore, 1)} grade:${strategy.backtest.robustnessGrade || "n/a"} walk_forward:${strategy.backtest.robustness?.walkForwardPassRate == null ? "n/a" : formatNumber(strategy.backtest.robustness.walkForwardPassRate * 100, 0) + "%"} stress:${strategy.backtest.robustness?.stressPassRate == null ? "n/a" : formatNumber(strategy.backtest.robustness.stressPassRate * 100, 0) + "%"} warnings:${Array.isArray(strategy.backtest.robustness?.warnings) ? strategy.backtest.robustness.warnings.join(",") : "none"}`,
      "Respond by explaining the StrategySpec, whether the target was reached, and the exact missing refinement if it was not. Keep it clear that the native runner did the candle calculations.",
    ].join("\n");
  }

  function setStrategyLabClarification(prompt = "", options = {}, spec = {}, response = null) {
    const missingMetrics = Array.isArray(response?.missingMetrics)
      ? response.missingMetrics
      : localValidateTradingStrategySpec(spec);
    const questions = Array.isArray(response?.questions) && response.questions.length
      ? response.questions
      : missingMetrics.map((item) => item.question).filter(Boolean);
    const now = tradingNowMs();
    state.strategyLab = {
      active: false,
      status: "needs_clarification",
      title: "StrategySpec needs clarification",
      prompt: String(prompt || "").trim(),
      source: String(options.source || "create_strategy"),
      instrument: spec.instrument || state.selectedInstrument,
      granularity: spec.granularity || state.selectedGranularity,
      rules: null,
      spec,
      missingMetrics,
      questions,
      runner: response || null,
      liveJob: null,
      plan: Array.isArray(response?.plan) ? response.plan : [],
      visualProbes: [],
      backtest: null,
      live: null,
      createdAtMs: now,
      updatedAtMs: now,
      backtestKey: "",
      liveKey: "",
      logs: [
        `${new Date(now).toISOString()} /create_ /strategy_ -> needs clarification`,
        `missing=${missingMetrics.map((item) => item.id || item.label).join(",")}`,
      ],
    };
    strategyRuntimeRevision += 1;
    invalidateContextCaches();
    syncStrategyOverlayToCanvas();
    state.ordersOutput = [
      "StrategySpec blocked before backtest.",
      "Forge will not invent missing metrics.",
      ...questions.map((question, index) => `${index + 1}. ${question}`),
    ].join("\n");
    canvasBridge()?.refreshRightPanel?.();
    return {
      ok: false,
      label: "strategy spec needs clarification",
      message: `Forge a parse /create_ /strategy_, mais il manque: ${missingMetrics.map((item) => item.label || item.id).join(", ")}. ${questions.join(" ")}`,
    };
  }

  function strategyBacktestStateFromRunner(response = {}) {
    const result = response?.result || {};
    const best = result.best || null;
    if (!best) {
      return {
        status: "tested",
        bars: Number(result.rows || 0),
        trades: 0,
        wins: 0,
        losses: 0,
        winRate: null,
        pnlPct: 0,
        pnlDistance: 0,
        expectancyPct: 0,
        expectancyDistance: 0,
        maxDrawdownPct: 0,
        direction: "",
        takeProfitDistance: null,
        robustness: null,
        robustnessScore: null,
        robustnessGrade: "",
        pairedProbe: result.pairedProbe || null,
        computePlan: result.computePlan || null,
        visualProbes: Array.isArray(result.visualProbes) ? result.visualProbes : [],
        note: "Rust runner completed but found no trades for this StrategySpec.",
      };
    }
    const robustness = best.robustness || null;
    return {
      status: best.meetsTarget ? "target_met" : "tested",
      bars: Number(result.rows || 0),
      trades: Number(best.trades || 0),
      wins: Number(best.wins || 0),
      losses: Number(best.losses || 0),
      winRate: best.winRate == null ? null : Number(best.winRate),
      pnlPct: 0,
      pnlDistance: Number(best.netPnlDistance || 0),
      expectancyPct: 0,
      expectancyDistance: Number(best.expectancyDistance || 0),
      maxDrawdownPct: 0,
      direction: String(best.direction || ""),
      takeProfitDistance: Number(best.takeProfitDistance || 0),
      robustness,
      robustnessScore: robustness?.score == null ? null : Number(robustness.score),
      robustnessGrade: robustness?.grade || "",
      pairedProbe: result.pairedProbe || null,
      computePlan: result.computePlan || null,
      visualProbes: Array.isArray(result.visualProbes) ? result.visualProbes : [],
      note: best.meetsTarget
        ? "Target win rate reached by the Rust runner."
        : "Target win rate not reached yet; refine filters or costs.",
    };
  }

  async function createAndTestStrategyFromPrompt(prompt = "", options = {}) {
    const snapshot = tradingContextSnapshot();
    const spec = extractTradingStrategySpecFromPrompt(prompt, snapshot);
    const localMissing = localValidateTradingStrategySpec(spec);
    if (!hasTauriInvoke()) {
      if (localMissing.length) return setStrategyLabClarification(prompt, options, spec, null);
      const now = tradingNowMs();
      state.strategyLab = {
        active: false,
        status: "runner_unavailable",
        title: "Strategy runner unavailable",
        prompt: String(prompt || "").trim(),
        source: String(options.source || "create_strategy"),
        instrument: spec.instrument || state.selectedInstrument,
        granularity: spec.granularity || state.selectedGranularity,
        rules: null,
        spec,
        missingMetrics: [],
        questions: [],
        runner: null,
        liveJob: null,
        plan: [],
        visualProbes: [],
        backtest: null,
        live: null,
        createdAtMs: now,
        updatedAtMs: now,
        backtestKey: "",
        liveKey: "",
        logs: [`${new Date(now).toISOString()} native strategy runner unavailable`],
      };
      strategyRuntimeRevision += 1;
      invalidateContextCaches();
      syncStrategyOverlayToCanvas();
      canvasBridge()?.refreshRightPanel?.();
      return {
        ok: false,
        label: "strategy runner unavailable",
        message: "Le runner Rust/Tauri n'est pas disponible dans cette session, donc Forge refuse de calculer la strategie dans le front.",
      };
    }

    const planResponse = await invoke("trading_strategy_backtest", {
      request: {
        spec,
        planOnly: true,
        maxRows: 0,
      },
    });
    if (planResponse?.status === "needs_clarification" || Array.isArray(planResponse?.missingMetrics) && planResponse.missingMetrics.length) {
      return setStrategyLabClarification(prompt, options, spec, planResponse);
    }
    const plannedComputePlan = planResponse?.computePlan || null;
    const plannedNow = tradingNowMs();
    state.strategyLab = {
      active: true,
      status: "planned",
      title: "Timed Low Volatility Strategy",
      prompt: String(prompt || "").trim(),
      source: String(options.source || "create_strategy"),
      instrument: spec.instrument || state.selectedInstrument,
      granularity: spec.granularity || state.selectedGranularity,
      rules: null,
      spec,
      missingMetrics: [],
      questions: [],
      runner: planResponse,
      liveJob: null,
      plan: Array.isArray(planResponse?.plan) ? planResponse.plan : [],
      computePlan: plannedComputePlan,
      pairedProbe: null,
      visualProbes: [],
      backtest: {
        status: "planned",
        bars: 0,
        trades: 0,
        wins: 0,
        losses: 0,
        winRate: null,
        pnlPct: 0,
        pnlDistance: null,
        expectancyPct: 0,
        expectancyDistance: null,
        maxDrawdownPct: 0,
        direction: "",
        takeProfitDistance: null,
        robustness: null,
        robustnessScore: null,
        robustnessGrade: "",
        pairedProbe: null,
        computePlan: plannedComputePlan,
        visualProbes: [],
        note: "Forge emitted a plan-only KASM DAG contract; the native backtest is starting now.",
      },
      live: null,
      createdAtMs: plannedNow,
      updatedAtMs: plannedNow,
      backtestKey: "",
      liveKey: "",
      logs: [
        `${new Date(plannedNow).toISOString()} /create_ /strategy_ -> plan_only DAG`,
        `kasm=${plannedComputePlan?.planId || "n/a"} nodes=${Array.isArray(plannedComputePlan?.dagNodes) ? plannedComputePlan.dagNodes.length : 0}`,
      ],
    };
    strategyRuntimeRevision += 1;
    invalidateContextCaches();
    canvasBridge()?.refreshRightPanel?.();
    try {
      options.onProgress?.({
        stage: "strategy_spec_created",
        spec,
        plan: Array.isArray(planResponse?.plan) ? planResponse.plan : [],
        computePlan: plannedComputePlan,
        status: planResponse?.status || "planned",
      });
    } catch (_) {}

    const response = await invoke("trading_strategy_backtest", {
      request: {
        spec,
        planOnly: false,
        maxRows: 0,
      },
    });
    if (response?.status === "needs_clarification" || Array.isArray(response?.missingMetrics) && response.missingMetrics.length) {
      return setStrategyLabClarification(prompt, options, spec, response);
    }
    const backtest = strategyBacktestStateFromRunner(response);
    const best = response?.result?.best || null;
    const live = {
      status: "paper_watch_ready",
      signal: best?.direction ? String(best.direction).toUpperCase() : "HOLD",
      confidence: backtest.winRate == null ? 0 : clamp(backtest.winRate, 0, 0.99),
      bars: Number(response?.result?.rows || 0),
      lastCandleTime: response?.result?.lastTime || "",
      lastClose: Number(state.candles?.[state.candles.length - 1]?.close || 0),
      reason: backtest.note,
      paperStop: null,
      paperTarget: best?.takeProfitDistance ?? null,
    };
    const now = tradingNowMs();
    state.strategyLab = {
      active: true,
      status: best?.meetsTarget ? "target_met" : "tested",
      title: "Timed Low Volatility Strategy",
      prompt: String(prompt || "").trim(),
      source: String(options.source || "create_strategy"),
      instrument: spec.instrument || state.selectedInstrument,
      granularity: spec.granularity || state.selectedGranularity,
      rules: null,
      spec,
      missingMetrics: [],
      questions: [],
      runner: response,
      liveJob: null,
      plan: Array.isArray(response?.plan) ? response.plan : [],
      computePlan: response?.result?.computePlan || null,
      pairedProbe: response?.result?.pairedProbe || null,
      visualProbes: Array.isArray(response?.result?.visualProbes) ? response.result.visualProbes : [],
      backtest,
      live,
      createdAtMs: now,
      updatedAtMs: now,
      backtestKey: simpleStableHash(JSON.stringify({ spec, best })),
      liveKey: "",
      logs: [
        `${new Date(now).toISOString()} /create_ /strategy_ -> Rust runner`,
        `trades=${backtest.trades || 0} win_rate=${backtest.winRate == null ? "n/a" : formatNumber(backtest.winRate * 100, 1) + "%"}`,
        `direction=${backtest.direction || "n/a"} tp=${formatNumber(backtest.takeProfitDistance)}`,
        `kasm=${response?.result?.computePlan?.planId || "n/a"} avoided_recalculations=${response?.result?.computePlan?.avoidedRecalculations ?? "n/a"}`,
      ],
    };
    strategyRuntimeRevision += 1;
    invalidateContextCaches();
    syncStrategyOverlayToCanvas();
    state.ordersOutput = [
      "Strategy Lab tested by Rust runner.",
      `instrument=${spec.instrument || state.selectedInstrument}`,
      `timeframe=${spec.granularity || state.selectedGranularity}`,
      `entry=${spec.entryHour}:00 ${spec.entryTimezone}`,
      `backtest=${backtest.trades || 0} trades, win_rate=${backtest.winRate == null ? "n/a" : formatNumber(backtest.winRate * 100, 1) + "%"}, pnl_distance=${formatNumber(backtest.pnlDistance, 5)}`,
      `robustness=${backtest.robustnessScore == null ? "n/a" : `${formatNumber(backtest.robustnessScore, 1)} ${backtest.robustnessGrade || ""}`}`,
      `paired_probe=${backtest.pairedProbe?.edge || "n/a"} long=${backtest.pairedProbe?.longWinRate == null ? "n/a" : formatNumber(backtest.pairedProbe.longWinRate * 100, 1) + "%"} short=${backtest.pairedProbe?.shortWinRate == null ? "n/a" : formatNumber(backtest.pairedProbe.shortWinRate * 100, 1) + "%"}`,
      `kasm_plan=${backtest.computePlan?.planId || "n/a"} commands=${Array.isArray(backtest.computePlan?.metricCommands) ? backtest.computePlan.metricCommands.length : 0} avoided_recalculations=${backtest.computePlan?.avoidedRecalculations ?? "n/a"}`,
      `best=${backtest.direction || "n/a"} tp=${formatNumber(backtest.takeProfitDistance)}`,
      "mode=paper live test; no broker order placed.",
    ].join("\n");
    await refreshStrategyLiveJob("create");
    syncStrategyOverlayToCanvas();
    canvasBridge()?.refreshRightPanel?.();
    return {
      ok: true,
      label: "strategy backtest completed",
      message: `/create_ /strategy_ a lance le runner Rust. Resultat: ${backtest.trades || 0} trades, win rate ${backtest.winRate == null ? "n/a" : formatNumber(backtest.winRate * 100, 1) + "%"}, robustesse ${backtest.robustnessScore == null ? "n/a" : formatNumber(backtest.robustnessScore, 1)} ${backtest.robustnessGrade || ""}, best ${backtest.direction || "n/a"} TP ${formatNumber(backtest.takeProfitDistance)}. Aucun ordre broker n'a ete place.`,
    };
  }

  function stopStrategyLab(reason = "manual") {
    if (!state.strategyLab?.active) {
      return {
        ok: true,
        label: "strategy lab idle",
        message: "Strategy Lab is already idle.",
      };
    }
    state.strategyLab = {
      ...state.strategyLab,
      active: false,
      status: "stopped",
      source: reason,
      updatedAtMs: tradingNowMs(),
    };
    strategyRuntimeRevision += 1;
    invalidateContextCaches();
    syncStrategyOverlayToCanvas();
    canvasBridge()?.refreshRightPanel?.();
    return {
      ok: true,
      label: "strategy paper live test stopped",
      message: "Strategy Lab stopped. No broker order was placed.",
    };
  }

  function simpleStableHash(text = "") {
    const input = String(text || "");
    let hash = 2166136261;
    for (let i = 0; i < input.length; i += 1) {
      hash ^= input.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    return `fnv1a-${(hash >>> 0).toString(16)}`;
  }

  function tradingContextFocusTags(userMessage = "") {
    const normalized = String(userMessage || "").toLowerCase();
    const tags = [];
    if (/\bcompare\b|\bcorrelat|\bversus\b|\bvs\b/.test(normalized)) tags.push("compare");
    if (/\balert\b/.test(normalized)) tags.push("alerts");
    if (/\border\b|\bposition\b|\btrade\b|\bsl\b|\btp\b|\bstop\b|\blimit\b/.test(normalized)) tags.push("orders");
    if (/\bindicator\b|\bema\b|\brsi\b|\bmacd\b|\bvwap\b/.test(normalized)) tags.push("indicators");
    if (/\breplay\b/.test(normalized)) tags.push("replay");
    if (/\btime zone\b|\butc\b|\btimezone\b/.test(normalized)) tags.push("timezone");
    if (/\bx axis\b|\by axis\b|\baxe x\b|\baxe y\b|\bvolatility\b|\bconviction\b|\banomaly\b/.test(normalized)) tags.push("axes");
    if (/\b3d\b|\b2d\b|\bcube\b/.test(normalized)) tags.push("view");
    if (!tags.length) tags.push("general");
    return tags;
  }

  function tradingContextFocusSummary(snapshot, userMessage = "") {
    const price = snapshot?.market?.price || {};
    const signals = snapshot?.signals || {};
    const observedAssets = Array.isArray(snapshot?.observedAssets?.assets)
      ? snapshot.observedAssets.assets
      : [];
    const observedLine = observedAssets.length
      ? `${observedAssets.length} asset${observedAssets.length > 1 ? "s" : ""}: ${observedAssets.map((item) => `${item.role}:${item.instrumentLabel || item.instrument}`).join(", ")}`
      : "1 asset";
    const compare = Array.isArray(snapshot?.compare) && snapshot.compare.length
      ? snapshot.compare.map((item) => item.instrumentLabel || item.instrument).join(", ")
      : "none";
    return [
      `focus=${tradingContextFocusTags(userMessage).join(",")}`,
      `observing=${observedLine}`,
      `instrument=${snapshot?.instrument?.label || "n/a"}`,
      `timeframe=${snapshot?.timeframe?.label || "n/a"}`,
      `compare=${compare}`,
      `mid=${formatNumber(price?.mid)}`,
      `signal_bias=${signals.bias || "unknown"}`,
      `signal_structure=${signals.structure || "unknown"}`,
    ].join(" | ");
  }

  function contextItemVersion(items, projector) {
    const rows = Array.isArray(items) ? items : [];
    let out = `${rows.length}`;
    for (const item of rows) out += `|${projector(item)}`;
    return out;
  }

  function compareSeriesContextVersion() {
    let out = "";
    for (const name of state.compareInstruments) {
      if (!name || name === state.selectedInstrument) continue;
      const series = getCachedSeries(name, state.selectedGranularity);
      const last = series[series.length - 1] || null;
      out += `|${name}:${series.length}:${last?.time || ""}:${last?.close || ""}`;
    }
    return out;
  }

  function addedChartsContextVersion() {
    let out = "";
    for (const name of addedChartInstruments()) {
      const series = getCachedSeries(name, state.selectedGranularity);
      const last = series[series.length - 1] || null;
      out += `|${name}:${series.length}:${last?.time || ""}:${last?.close || ""}`;
    }
    return out;
  }

  function bridgeContextVersion(bridgeSnapshot) {
    const doc = bridgeSnapshot?.doc || {};
    const viewport = bridgeSnapshot?.tradingViewport || {};
    const axes = bridgeSnapshot?.tradingAxisMetricPrefs || {};
    const timePrefs = bridgeSnapshot?.tradingTimePrefs || {};
    const scalePrefs = bridgeSnapshot?.tradingScalePrefs || {};
    return [
      bridgeSnapshot?.is3dOpen ? 1 : 0,
      bridgeSnapshot?.rightPanelOpen ? 1 : 0,
      bridgeSnapshot?.rightPanelMode || "",
      doc.candleViewCount || 0,
      doc.candleViewEnd || 0,
      doc.priceZoom || 1,
      doc.pricePan || 0,
      viewport.visibleBars || 0,
      viewport.timeEndMs || 0,
      viewport.priceMin || 0,
      viewport.priceMax || 0,
      axes.xMode || "",
      axes.yMode || "",
      timePrefs.timeZone || "",
      timePrefs.showSessionBreaks ? 1 : 0,
      timePrefs.showSignalMarkers ? 1 : 0,
      scalePrefs.autoFit === false ? 0 : 1,
      scalePrefs.invertPriceScale ? 1 : 0,
      scalePrefs.priceAxisSide || "",
      scalePrefs.showPriceLabels === false ? 0 : 1,
      scalePrefs.showGridLines === false ? 0 : 1,
      scalePrefs.showPlusButton === false ? 0 : 1,
    ].join("|");
  }

  function tradingContextSnapshotCacheKey(bridgeSnapshot) {
    const price = state.market?.price || state.snapshot?.price || {};
    const book = state.market?.book || state.snapshot?.book || {};
    const lastCandle = state.candles[state.candles.length - 1] || {};
    const firstCandle = state.candles[0] || {};
    return [
      "ctx-v5",
      selectedBrokerKind(),
      state.selectedInstrument,
      state.selectedGranularity,
      state.chartDisplayMode,
      state.indicatorsModeActive ? 1 : 0,
      state.replayModeActive ? 1 : 0,
      state.alertModalOpen ? 1 : 0,
      state.headerMenuMode,
      assetUniverseRevision,
      catalogUniverseRevision,
      state.catalog.length,
      state.universeHistorySyncDone ? 1 : 0,
      state.compareInstruments.join(","),
      compareSeriesContextVersion(),
      addedChartInstruments().join(","),
      addedChartsContextVersion(),
      alertUniverseRevision,
      strategyRuntimeRevision,
      contextItemVersion(state.market?.openTrades || state.snapshot?.openTrades || [], (item) => [
        item?.id || item?.tradeId || "",
        item?.instrument || "",
        item?.price || item?.entryPrice || "",
        item?.currentUnits || item?.units || "",
      ].join(":")),
      contextItemVersion(state.market?.pendingOrders || state.snapshot?.pendingOrders || [], (item) => [
        item?.id || item?.orderId || "",
        item?.instrument || "",
        item?.price || item?.entryPrice || "",
        item?.units || "",
      ].join(":")),
      [
        state.snapshot?.account?.balance || 0,
        state.snapshot?.account?.nav || 0,
        state.snapshot?.account?.unrealizedPl || 0,
        state.snapshot?.account?.marginAvailable || 0,
      ].join(":"),
      [
        book.kind || "",
        Array.isArray(book.bids) ? book.bids.length : 0,
        Array.isArray(book.asks) ? book.asks.length : 0,
        JSON.stringify(Array.isArray(book.bids) ? book.bids[0] || null : null),
        JSON.stringify(Array.isArray(book.asks) ? book.asks[0] || null : null),
      ].join(":"),
      [price.time || "", price.bid || 0, price.ask || 0, price.mid || 0, price.spread || 0].join(":"),
      [
        state.candles.length,
        firstCandle.time || "",
        firstCandle.open || "",
        lastCandle.time || "",
        lastCandle.close || "",
      ].join(":"),
      [
        state.orderForm.instrument,
        state.orderForm.side,
        state.orderForm.units,
        state.orderForm.orderType,
        state.orderForm.limitPrice,
        state.orderForm.stopLoss,
        state.orderForm.takeProfit,
      ].join(":"),
      bridgeContextVersion(bridgeSnapshot),
      JSON.stringify(state.llmInvolvement),
    ].join("\u001f");
  }

  function summarizeContextCompareAssets(brokerInstrumentSet) {
    const out = [];
    for (const name of state.compareInstruments) {
      if (!name || name === state.selectedInstrument) continue;
      const hit = findAsset(name) || normalizeAssetEntry({ name });
      const cached = getCachedSeries(name, state.selectedGranularity);
      out.push({
        instrument: name,
        instrumentLabel: brokerInstrumentCode(name) || name,
        displayName: hit?.displayName || name,
        assetClass: hit?.assetClass || inferAssetClass(name),
        tradableOnSelectedBroker: brokerInstrumentSet.has(name),
        candleCount: cached.length,
        lastClose: cached.length ? Number(cached[cached.length - 1]?.close) : null,
      });
    }
    return out;
  }

  function summarizeContextObservedAsset(name, role, slot, brokerInstrumentSet) {
    const instrument = String(name || "").trim();
    if (!instrument) return null;
    const hit = findAsset(instrument) || normalizeAssetEntry({ name: instrument });
    const cached = getCachedSeries(instrument, state.selectedGranularity);
    return {
      role,
      slot,
      instrument,
      instrumentLabel: brokerInstrumentCode(instrument) || instrument,
      displayName: hit?.displayName || instrument,
      assetClass: hit?.assetClass || inferAssetClass(instrument),
      tradableOnSelectedBroker: brokerInstrumentSet.has(instrument),
      candleCount: cached.length,
      lastClose: cached.length ? Number(cached[cached.length - 1]?.close) : null,
      timeframe: state.selectedGranularity,
    };
  }

  function summarizeObservedAssets(brokerInstrumentSet) {
    const assets = [];
    const primary = summarizeContextObservedAsset(state.selectedInstrument, "primary", 1, brokerInstrumentSet);
    if (primary) assets.push(primary);
    addedChartInstruments().forEach((instrument, index) => {
      const asset = summarizeContextObservedAsset(instrument, `add_${index + 1}`, index + 2, brokerInstrumentSet);
      if (asset) assets.push(asset);
    });
    return {
      count: assets.length,
      maxCount: 4,
      mode: assets.length > 1 ? "split_chart" : "single_chart",
      layout: assets.length <= 1 ? "single" : (assets.length === 2 ? "two_up" : "grid_2x2"),
      assets,
      summary: assets.map((item) => `${item.role}:${item.instrumentLabel}`).join(", "),
    };
  }

  function uniqueContextAssetList(...groups) {
    const seen = new Set();
    const out = [];
    for (const group of groups) {
      for (const item of Array.isArray(group) ? group : []) {
        const key = String(item?.instrument || "").trim();
        if (!key || seen.has(key) || key === state.selectedInstrument) continue;
        seen.add(key);
        out.push(item);
      }
    }
    return out;
  }

  function tradingContextSnapshot() {
    const bridgeSnapshot = canvasTradingSnapshot();
    const snapshotKey = tradingContextSnapshotCacheKey(bridgeSnapshot);
    if (state.uiCache.contextSnapshotKey === snapshotKey && state.uiCache.contextSnapshotValue) {
      return state.uiCache.contextSnapshotValue;
    }
    const asset = findAsset(state.selectedInstrument);
    const instrumentTradable = isSelectedInstrumentTradable();
    const brokerInstrumentSet = activeBrokerInstrumentSet();
    const compareAssets = summarizeContextCompareAssets(brokerInstrumentSet);
    const observedAssets = summarizeObservedAssets(brokerInstrumentSet);
    const signalPeerAssets = uniqueContextAssetList(observedAssets.assets.slice(1), compareAssets);
    const snapshot = {
      generatedAt: new Date().toISOString(),
      snapshotVersion: "trading-context-v5",
      workspaceMode: "trading",
      broker: {
        selectedKind: selectedBrokerKind(),
        selectedLabel: selectedBrokerLabel(),
        effectiveLabel: instrumentTradable ? selectedBrokerLabel() : "PAPERTRADING",
        tradableUniverseCount: brokerInstrumentSet.size,
        availableBrokers: availableBrokers().map((item) => ({
          kind: String(item?.kind || ""),
          label: String(item?.label || ""),
          active: item?.active === true,
        })),
        instrumentTradable,
        config: state.snapshot?.config || null,
      },
      instrument: {
        name: state.selectedInstrument,
        label: brokerInstrumentCode(state.selectedInstrument) || state.selectedInstrument,
        displayName: asset?.displayName || state.selectedInstrument,
        assetClass: asset?.assetClass || inferAssetClass(state.selectedInstrument),
      },
      timeframe: {
        granularity: state.selectedGranularity,
        label: timeframeLabel(state.selectedGranularity),
        available: availableGranularities(),
      },
      observedAssets,
      compare: compareAssets,
      chart: {
        displayMode: state.chartDisplayMode,
        observedAssetCount: observedAssets.count,
        splitMode: observedAssets.mode,
        splitLayout: observedAssets.layout,
        addedCharts: observedAssets.assets.slice(1),
        indicatorsPanelActive: !!state.indicatorsModeActive,
        replayModeActive: !!state.replayModeActive,
        alertPanelOpen: !!state.alertModalOpen,
        is3dOpen: !!bridgeSnapshot?.is3dOpen,
        viewport: bridgeSnapshot?.tradingViewport || null,
        doc: {
          candleViewCount: Number(bridgeSnapshot?.doc?.candleViewCount || 0),
          candleViewEnd: Number(bridgeSnapshot?.doc?.candleViewEnd || 0),
          priceZoom: Number(bridgeSnapshot?.doc?.priceZoom || 1),
          pricePan: Number(bridgeSnapshot?.doc?.pricePan || 0),
          chartDisplayMode: String(bridgeSnapshot?.doc?.chartDisplayMode || state.chartDisplayMode || ""),
        },
        axes: snapshotAxisSummary(bridgeSnapshot),
      },
      ui: summarizeUiState(bridgeSnapshot, compareAssets, observedAssets),
      history: summarizeCatalog(state.catalog),
      market: {
        price: summarizePrice(),
        book: summarizeBook(),
        candles: summarizeCandles(state.candles),
      },
      indicators: summarizeIndicatorState(bridgeSnapshot),
      replay: summarizeReplayState(bridgeSnapshot),
      account: summarizeAccount(),
      orderDraft: summarizeOrderDraft(state.orderForm),
      orders: {
        pending: summarizeTrades(state.market?.pendingOrders || state.snapshot?.pendingOrders || []),
        openTrades: summarizeTrades(state.market?.openTrades || state.snapshot?.openTrades || []),
      },
      alerts: summarizeAlerts(state.alerts),
      signals: summarizeSignalState(state.candles, signalPeerAssets, state.market?.price || state.snapshot?.price || null, state.alerts),
      strategyLab: summarizeStrategyLabState(),
      llmInvolvement: {
        codex: isRuntimeInvolved("codex"),
        gemini: isRuntimeInvolved("gemini"),
        claude: isRuntimeInvolved("claude"),
        any: anyRuntimeInvolved(),
      },
    };
    state.uiCache.contextSnapshotKey = snapshotKey;
    state.uiCache.contextSnapshotValue = snapshot;
    state.uiCache.contextDigestKey = "";
    state.uiCache.contextDigestValue = "";
    state.uiCache.contextDigestV3Key = "";
    state.uiCache.contextDigestV3Value = "";
    return snapshot;
  }

  function tradingObservedAssetsDigestLine(snapshot, separator = " | ") {
    const observed = snapshot?.observedAssets || { count: 1, mode: "single_chart", layout: "single", assets: [] };
    const assets = Array.isArray(observed.assets) ? observed.assets : [];
    const observedLine = assets.length
      ? assets.map((item) => {
        const last = Number.isFinite(Number(item.lastClose)) ? `${separator}last:${formatNumber(item.lastClose)}` : "";
        const bars = item.candleCount ? `${separator}bars:${item.candleCount}` : "";
        return `${item.role}:${item.instrumentLabel || item.instrument}${bars}${last}`;
      }).join(" ; ")
      : `primary:${snapshot?.instrument?.label || "n/a"}`;
    return `observed_assets=count:${observed.count || 1}/4${separator}mode:${observed.mode || "single_chart"}${separator}layout:${observed.layout || "single"}${separator}${observedLine}`;
  }

  function tradingContextDigest() {
    const snapshot = tradingContextSnapshot();
    const digestKey = `${state.uiCache.contextSnapshotKey || ""}::digest-v2`;
    if (state.uiCache.contextDigestKey === digestKey && state.uiCache.contextDigestValue) {
      return state.uiCache.contextDigestValue;
    }
    const price = snapshot.market?.price || {};
    const candles = snapshot.market?.candles || {};
    const compareLine = snapshot.compare.length
      ? snapshot.compare.map((item) => `${item.label || item.instrumentLabel || item.instrument}${item.tradableOnSelectedBroker ? " [tradable]" : " [library]"}`).join(", ")
      : "none";
    const pendingCount = snapshot.orders.pending.length;
    const openTradeCount = snapshot.orders.openTrades.length;
    const alertCount = snapshot.alerts.length;
    const axis = snapshot.chart.axes || {};
    const lines = [
      `broker=${snapshot.broker.effectiveLabel} (${snapshot.broker.selectedKind})`,
      tradingObservedAssetsDigestLine(snapshot, " Â· "),
      `instrument=${snapshot.instrument.label} · display=${snapshot.instrument.displayName} · class=${snapshot.instrument.assetClass}`,
      `timeframe=${snapshot.timeframe.label} (${snapshot.timeframe.granularity})`,
      `compare=${compareLine}`,
      `chart_mode=${snapshot.chart.displayMode} · 3d=${snapshot.chart.is3dOpen ? "on" : "off"} · replay=${snapshot.chart.replayModeActive ? "on" : "off"} · indicators_panel=${snapshot.chart.indicatorsPanelActive ? "on" : "off"} · alerts_panel=${snapshot.chart.alertPanelOpen ? "open" : "closed"}`,
      `axes=x:${axis.xMode} · y:${axis.yMode} · space:${axis.metricSpace} · timezone:${axis.timeZone} · labels=${axis.showPriceLabels ? "on" : "off"} · grid=${axis.showGridLines ? "on" : "off"} · invert_price=${axis.invertPriceScale ? "on" : "off"}`,
      `price=bid:${formatNumber(price?.bid)} ask:${formatNumber(price?.ask)} mid:${formatNumber(price?.mid)} spread:${formatNumber(price?.spread, 4)}`,
      `candles=count:${candles.count || 0} · first:${candles.firstTime || "n/a"} · last:${candles.lastTime || "n/a"} · high:${formatNumber(candles.high)} · low:${formatNumber(candles.low)} · last_close:${formatNumber(candles.lastClose)}`,
      `orders=open_trades:${openTradeCount} · pending_orders:${pendingCount} · alerts:${alertCount}`,
      `llm_involvement=codex:${snapshot.llmInvolvement.codex ? "on" : "off"} gemini:${snapshot.llmInvolvement.gemini ? "on" : "off"} claude:${snapshot.llmInvolvement.claude ? "on" : "off"}`,
    ];
    if (candles.visiblePreview.length) {
      lines.push(`recent_bars=${candles.visiblePreview.map((bar) => `${bar.time} O:${formatNumber(bar.open)} H:${formatNumber(bar.high)} L:${formatNumber(bar.low)} C:${formatNumber(bar.close)}`).join(" | ")}`);
    }
    if (openTradeCount) {
      lines.push(`open_trade_preview=${snapshot.orders.openTrades.map((trade) => `${trade.instrument}:${trade.side}@${formatNumber(trade.price)}`).join(" ; ")}`);
    }
    if (pendingCount) {
      lines.push(`pending_order_preview=${snapshot.orders.pending.map((order) => `${order.instrument}:${order.side}@${formatNumber(order.price)}`).join(" ; ")}`);
    }
    if (alertCount) {
      lines.push(`alert_preview=${snapshot.alerts.map((alert) => `${alert.operator}:${formatNumber(alert.targetValue)}`).join(" ; ")}`);
    }
    const digest = lines.join("\n");
    state.uiCache.contextDigestKey = digestKey;
    state.uiCache.contextDigestValue = digest;
    return digest;
  }

  function tradingContextDigestV3() {
    const snapshot = tradingContextSnapshot();
    const digestKey = `${state.uiCache.contextSnapshotKey || ""}::digest-v3`;
    if (state.uiCache.contextDigestV3Key === digestKey && state.uiCache.contextDigestV3Value) {
      return state.uiCache.contextDigestV3Value;
    }
    const price = snapshot.market?.price || {};
    const book = snapshot.market?.book || {};
    const candles = snapshot.market?.candles || {};
    const compareLine = snapshot.compare.length
      ? snapshot.compare.map((item) => `${item.instrumentLabel || item.instrument}${item.tradableOnSelectedBroker ? " [tradable]" : " [library]"}`).join(", ")
      : "none";
    const pendingCount = snapshot.orders.pending.length;
    const openTradeCount = snapshot.orders.openTrades.length;
    const alertCount = snapshot.alerts.length;
    const axis = snapshot.chart.axes || {};
    const ui = snapshot.ui || {};
    const history = snapshot.history || {};
    const account = snapshot.account || {};
    const indicators = snapshot.indicators || {};
    const replay = snapshot.replay || {};
    const signals = snapshot.signals || {};
    const strategy = snapshot.strategyLab || {};
    const orderDraft = snapshot.orderDraft || {};
    const lines = [
      `snapshot_version=${snapshot.snapshotVersion} generated_at=${snapshot.generatedAt}`,
      `broker=${snapshot.broker.effectiveLabel} (${snapshot.broker.selectedKind})`,
      tradingObservedAssetsDigestLine(snapshot),
      `instrument=${snapshot.instrument.label} | display=${snapshot.instrument.displayName} | class=${snapshot.instrument.assetClass} | tradable=${snapshot.broker.instrumentTradable ? "yes" : "no"}`,
      `timeframe=${snapshot.timeframe.label} (${snapshot.timeframe.granularity})`,
      `compare=${compareLine}`,
      `chart_mode=${snapshot.chart.displayMode} | observed_count:${snapshot.chart.observedAssetCount || snapshot.observedAssets?.count || 1} | split:${snapshot.chart.splitMode || snapshot.observedAssets?.mode || "single_chart"} | 3d=${snapshot.chart.is3dOpen ? "on" : "off"} | replay=${snapshot.chart.replayModeActive ? "on" : "off"} | indicators_panel=${snapshot.chart.indicatorsPanelActive ? "on" : "off"} | alerts_panel=${snapshot.chart.alertPanelOpen ? "open" : "closed"}`,
      `axes=x:${axis.xMode} | y:${axis.yMode} | space:${axis.metricSpace} | timezone:${axis.timeZone} | labels=${axis.showPriceLabels ? "on" : "off"} | grid=${axis.showGridLines ? "on" : "off"} | invert_price=${axis.invertPriceScale ? "on" : "off"} | auto_fit=${axis.autoFit ? "on" : "off"}`,
      `ui=header_menu:${ui.headerMenuMode || "none"} | right_panel:${ui.rightPanelOpen ? "open" : "closed"}(${ui.rightPanelMode || "none"}) | compare_menu:${ui.compareMenuOpen ? "open" : "closed"} | timeframe_rail:${ui.timeframeRailVisible ? "visible" : "hidden"}`,
      `history=files:${history.fileCount || 0} | instruments:${history.instrumentCount || 0} | universe_sync=${history.universeHistoryReady ? "ready" : "pending"}`,
      `price=bid:${formatNumber(price?.bid)} ask:${formatNumber(price?.ask)} mid:${formatNumber(price?.mid)} spread:${formatNumber(price?.spread, 4)}`,
      `book=kind:${book.kind || "n/a"} | bids:${Array.isArray(book.bids) ? book.bids.length : 0} | asks:${Array.isArray(book.asks) ? book.asks.length : 0}`,
      `candles=count:${candles.count || 0} | first:${candles.firstTime || "n/a"} | last:${candles.lastTime || "n/a"} | high:${formatNumber(candles.high)} | low:${formatNumber(candles.low)} | last_close:${formatNumber(candles.lastClose)}`,
      `signals=bias:${signals.bias || "unknown"} | structure:${signals.structure || "unknown"} | last_candle:${signals.lastCandleDirection || "unknown"} | momentum_20:${formatNumber(signals.momentumPct, 2)} | range_pct:${formatNumber(signals.intrabarRangePct, 2)} | alert_proximity_pct:${formatNumber(signals.nearestAlertDistancePct, 2)}`,
      `strategy_lab=active:${strategy.active ? "yes" : "no"} | status:${strategy.status || "idle"} | name:${strategy.title || "none"} | kind:${strategy.rules?.kind || "none"} | backtest_trades:${strategy.backtest?.trades || 0} | win_rate:${strategy.backtest?.winRate == null ? "n/a" : formatNumber(strategy.backtest.winRate * 100, 1) + "%"} | pnl_pct:${formatNumber(strategy.backtest?.pnlPct, 2)} | live_signal:${strategy.live?.signal || "HOLD"} | live_confidence:${formatNumber((strategy.live?.confidence || 0) * 100, 1)}% | live_reason:${strategy.live?.reason || "n/a"}`,
      `indicators=panel:${indicators.panelOpen ? "open" : "closed"} | active_count:${indicators.activeCount || 0} | x_metric_hint:${indicators.xMetricMode || axis.xMode} | y_metric_hint:${indicators.yMetricMode || axis.yMode}`,
      `replay=active:${replay.active ? "on" : "off"} | visible_bars:${replay.visibleBars || 0} | window_start:${replay.timeStartMs || 0} | window_end:${replay.timeEndMs || 0}`,
      `account=balance:${formatNumber(account.balance)} nav:${formatNumber(account.nav)} upl:${formatNumber(account.unrealizedPl)} margin_available:${formatNumber(account.marginAvailable)}`,
      `orders=open_trades:${openTradeCount} | pending_orders:${pendingCount} | alerts:${alertCount}`,
      `order_draft=instrument:${orderDraft.instrument || snapshot.instrument.label} | side:${orderDraft.side || "n/a"} | units:${formatNumber(orderDraft.units, 0)} | type:${orderDraft.orderType || "n/a"} | limit:${formatNumber(orderDraft.limitPrice)} | sl:${formatNumber(orderDraft.stopLoss)} | tp:${formatNumber(orderDraft.takeProfit)} | ready:${orderDraft.ready ? "yes" : "no"}`,
      `llm_involvement=codex:${snapshot.llmInvolvement.codex ? "on" : "off"} gemini:${snapshot.llmInvolvement.gemini ? "on" : "off"} claude:${snapshot.llmInvolvement.claude ? "on" : "off"}`,
    ];
    if (candles.visiblePreview.length) {
      lines.push(`recent_bars=${candles.visiblePreview.map((bar) => `${bar.time} O:${formatNumber(bar.open)} H:${formatNumber(bar.high)} L:${formatNumber(bar.low)} C:${formatNumber(bar.close)}`).join(" | ")}`);
    }
    if (openTradeCount) {
      lines.push(`open_trade_preview=${snapshot.orders.openTrades.map((trade) => `${trade.instrument}:${trade.side}@${formatNumber(trade.price)}`).join(" ; ")}`);
    }
    if (pendingCount) {
      lines.push(`pending_order_preview=${snapshot.orders.pending.map((order) => `${order.instrument}:${order.side}@${formatNumber(order.price)}`).join(" ; ")}`);
    }
    if (alertCount) {
      lines.push(`alert_preview=${snapshot.alerts.map((alert) => `${alert.operator}:${formatNumber(alert.targetValue)}`).join(" ; ")}`);
    }
    if (signals.compareStrength?.length) {
      lines.push(`compare_strength=${signals.compareStrength.map((item) => `${item.instrumentLabel}:${formatNumber(item.momentumPct, 2)}% rs:${formatNumber(item.relativeStrengthPct, 2)}`).join(" ; ")}`);
    }
    if (indicators.note) {
      lines.push(`indicator_note=${indicators.note}`);
    }
    if (replay.note) {
      lines.push(`replay_note=${replay.note}`);
    }
    if (signals.note) {
      lines.push(`signal_note=${signals.note}`);
    }
    const digest = lines.join("\n");
    state.uiCache.contextDigestV3Key = digestKey;
    state.uiCache.contextDigestV3Value = digest;
    return digest;
  }

  function tradingContextEnvelope({ runtime = "codex", sessionJobId = "", userMessage = "" } = {}) {
    const snapshot = tradingContextSnapshot();
    const digest = tradingContextDigestV3();
    const digestHash = simpleStableHash(digest);
    const runtimeKey = ["codex", "gemini", "claude"].includes(runtime) ? runtime : "codex";
    const relay = state.uiCache.contextEnvelopeState?.[runtimeKey] || { hash: "", sessionJobId: "" };
    const sameDigest = relay.hash === digestHash && relay.sessionJobId === String(sessionJobId || "");
    const focus = tradingContextFocusSummary(snapshot, userMessage);
    const directives = [
      "Use the Forge Trading context as the source of truth for the current market/chart/UI state.",
      "Immediately account for observed_assets: the user may be watching 1, 2, 3, or 4 charts simultaneously via add_. Treat every observed asset as active visual context, not as an optional mention.",
      "Do not ask the user to restate asset, timeframe, compare set, replay state, indicators, axes, or orders if they are already present here.",
      "Do not reconstruct raw chart state from scratch when the digest already summarizes it.",
      "If you need deeper detail, ask for a targeted Forge-local expansion instead of requesting the full raw history.",
    ];
    if (/\/create_\b/.test(String(userMessage || "")) && /\/strategy_\b/.test(String(userMessage || ""))) {
      directives.push(
        "For /create_ /strategy_, Forge starts a local Strategy Lab paper backtest/live watch before your response. Treat strategy_lab as the measured baseline and refine from it.",
        "All requested strategy metrics must stay as slash/metric commands in the / channel; KASM/native runners execute them and reuse cache keys instead of the LLM recalculating candles.",
        "Use paired_probe when available: it tests long and short from the same eligible open price with one shared entry scan.",
        "If the chat already displayed the /create_ and /backtest_ execution trace, continue with the actual results, pass/fail explanation, risks and next refinement.",
        "Never imply live broker execution unless the user explicitly asks to place orders; this mode is paper live testing by default.",
      );
    }
    const packet = [
      "TRADING_CONTEXT_PROTOCOL:v1",
      `TRADING_CONTEXT_RUNTIME:${runtimeKey}`,
      `TRADING_CONTEXT_SESSION:${String(sessionJobId || "none")}`,
      `TRADING_CONTEXT_HASH:${digestHash}`,
      `TRADING_CONTEXT_STATUS:${sameDigest ? "unchanged_since_last_turn_for_runtime" : "full_snapshot"}`,
      `TRADING_CONTEXT_FOCUS:${focus}`,
      `TRADING_CONTEXT_DIRECTIVES:\n- ${directives.join("\n- ")}`,
    ];
    if (sameDigest) {
      packet.push(`TRADING_CONTEXT_COMPACT:\n${focus}`);
    } else {
      packet.push(`TRADING_CONTEXT_DIGEST:\n${digest}`);
    }
    state.uiCache.contextEnvelopeState[runtimeKey] = {
      hash: digestHash,
      sessionJobId: String(sessionJobId || ""),
    };
    return packet.join("\n\n");
  }

  function commandGranularityFromText(text = "") {
    const normalized = String(text || "").toUpperCase();
    const aliases = [
      ["W1", "W"], [" D1", "D"], ["H4", "H4"], ["H1", "H1"],
      ["M30", "M30"], ["M15", "M15"], ["M5", "M5"], ["M1", "M1"],
      ["S30", "S30"], ["S10", "S10"],
    ];
    for (const [token, granularity] of aliases) {
      if (normalized.includes(token.trim())) return granularity;
    }
    return "";
  }

  function findMentionedAssets(text = "") {
    const compact = compactAssetToken(text);
    const hits = [];
    const seen = new Set();
    for (const record of assetSearchIndex().mentionRecords) {
      for (const alias of record.mentionAliases) {
        if (alias && compact.includes(alias)) {
          if (!seen.has(record.name)) {
            seen.add(record.name);
            hits.push(record.asset);
          }
          break;
        }
      }
    }
    return hits;
  }

  function commandIntentState(normalized = "", positivePattern, negativePattern) {
    if (negativePattern && negativePattern.test(normalized)) return false;
    if (positivePattern && positivePattern.test(normalized)) return true;
    return null;
  }

  function commandBrokerKind(normalized = "") {
    for (const broker of availableBrokers()) {
      const kind = String(broker?.kind || "").trim().toLowerCase();
      const label = String(broker?.label || "").trim().toLowerCase();
      if (!kind) continue;
      if (normalized.includes(kind) || (label && normalized.includes(label))) return kind;
    }
    return "";
  }

  function commandChartDisplayMode(normalized = "") {
    if (/\bohlc\b|\bbars?\b/.test(normalized)) return "ohlc";
    if (/\barea\b|\bzone\b/.test(normalized)) return "area";
    if (/\bline\b|\bligne\b/.test(normalized)) return "line";
    if (/\bcandles?\b|\bcandlesticks?\b|\bbougies?\b/.test(normalized)) return "candles";
    return "";
  }

  function commandXAxisMode(normalized = "") {
    if (/\bx axis\b|\bx-axis\b|\baxe x\b|\bhorizontal axis\b/.test(normalized)) {
      if (/\bvolatility\b|\bcumulative volatility\b|\bvol cumul/.test(normalized)) return "volatility";
      if (/\bsignal density\b|\bforge signal density\b|\bdensit/.test(normalized)) return "signal-density";
      if (/\bregime\b|\bstate progression\b/.test(normalized)) return "regime";
      if (/\btime\b|\bclassic\b|\btemps\b/.test(normalized)) return "time";
    }
    return "";
  }

  function commandYAxisMode(normalized = "") {
    if (/\by axis\b|\by-axis\b|\baxe y\b|\bvertical axis\b|\bprice scale\b|\bscale mode\b/.test(normalized)) {
      if (/\bfair value gap\b|\bfair gap\b/.test(normalized)) return "fair-gap";
      if (/\bconviction\b/.test(normalized)) return "conviction";
      if (/\banomaly\b/.test(normalized)) return "anomaly";
      if (/\bprice\b|\bclassic\b|\bprix\b/.test(normalized)) return "price";
    }
    return "";
  }

  function commandTimeZoneId(normalized = "") {
    const table = [
      ["utc", "UTC"],
      ["market", "local"],
      ["local", "local"],
      ["new york", "America/New_York"],
      ["toronto", "America/Toronto"],
      ["chicago", "America/Chicago"],
      ["mexico", "America/Mexico_City"],
      ["phoenix", "America/Phoenix"],
      ["vancouver", "America/Vancouver"],
      ["honolulu", "Pacific/Honolulu"],
      ["anchorage", "America/Anchorage"],
      ["bogota", "America/Bogota"],
      ["lima", "America/Lima"],
      ["santiago", "America/Santiago"],
      ["caracas", "America/Caracas"],
      ["reykjavik", "Atlantic/Reykjavik"],
      ["casablanca", "Africa/Casablanca"],
      ["london", "Europe/London"],
      ["paris", "Europe/Paris"],
      ["tokyo", "Asia/Tokyo"],
      ["singapore", "Asia/Singapore"],
      ["hong kong", "Asia/Hong_Kong"],
      ["sydney", "Australia/Sydney"],
    ];
    for (const [needle, value] of table) {
      if (normalized.includes(needle)) return value;
    }
    return "";
  }

  function commandOrderType(normalized = "") {
    if (/\blimit\b/.test(normalized)) return "LIMIT";
    if (/\bmarket\b/.test(normalized)) return "MARKET";
    return "";
  }

  function parseFirstCommandNumber(raw = "", pattern) {
    const hit = String(raw || "").match(pattern);
    return hit ? Number(hit[1]) : null;
  }

  function stageOrderDraftFromCommand(raw = "", normalized = "") {
    if (!/\b(buy|sell|order|limit|market|stop loss|take profit|tp|sl)\b/.test(normalized)) return null;
    const next = { ...state.orderForm };
    if (/\bbuy\b/.test(normalized)) next.side = "BUY";
    if (/\bsell\b/.test(normalized)) next.side = "SELL";
    const orderType = commandOrderType(normalized);
    if (orderType) next.orderType = orderType;
    const units = parseFirstCommandNumber(raw, /\b(\d+(?:\.\d+)?)\s*(?:units?|contracts?|lots?)\b/i);
    if (units != null && Number.isFinite(units)) next.units = String(units);
    const atPrice = parseFirstCommandNumber(raw, /\b(?:at|@)\s*(\d+(?:[.,]\d+)?)\b/i);
    if (atPrice != null && Number.isFinite(atPrice)) next.limitPrice = String(atPrice).replace(",", ".");
    const sl = parseFirstCommandNumber(raw, /\b(?:sl|stop loss)\s*(?:=|at|@)?\s*(\d+(?:[.,]\d+)?)\b/i);
    if (sl != null && Number.isFinite(sl)) next.stopLoss = String(sl).replace(",", ".");
    const tp = parseFirstCommandNumber(raw, /\b(?:tp|take profit)\s*(?:=|at|@)?\s*(\d+(?:[.,]\d+)?)\b/i);
    if (tp != null && Number.isFinite(tp)) next.takeProfit = String(tp).replace(",", ".");
    state.orderForm = next;
    return summarizeOrderDraft(next);
  }

  function applyTradingIndicatorCommand(raw = "") {
    const tokens = extractTradingSlashTokens(raw);
    if (!tokens.length) return null;
    const [command, createTarget = ""] = tokens;
    if (command === "/create_") {
      state.chatSubbarSection = "create";
      setTradingChatSubbarMode("indicators", { force: true });
      const targetEntity = createTarget ? atlasSlashRegistry()?.resolve?.(createTarget) : null;
      if (targetEntity) {
        return {
          ok: true,
          label: "create blueprint staged",
          message: `${command} ${targetEntity.token} is staged in Atlas as a creation blueprint.`,
        };
      }
      return {
        ok: true,
        label: "create lane",
        message: "Create lane opened. Pick a target like `/indicator`, `/alert_`, `/order`, `/program_`, `/strategy_`, `/geo`, `/minigeo`, `/lens_`, `/geonode_view_`, `/dataset_`, or `/map_`.",
      };
    }
    const injected = [];
    const seen = new Set();
    for (const token of tokens) {
      const definition = tradingIndicatorDefinitionByCommand(token);
      if (!definition || seen.has(definition.id)) continue;
      seen.add(definition.id);
      upsertTradingIndicator(definition.id);
      injected.push(definition.command);
    }
    if (!injected.length) return null;
    return {
      ok: true,
      label: injected.length === 1 ? "indicator injected" : "indicators injected",
      message: `${injected.join(" ")} injected on the chart.`,
    };
  }

  function applyTradingAtlasEntityCommand(raw = "") {
    const tokens = extractTradingSlashTokens(raw);
    if (!tokens.length) return null;
    const summary = {
      candles: [],
      indicatorMetrics: [],
      strategies: [],
      programs: [],
      creates: [],
      geos: [],
    };
    for (const token of tokens) {
      const entity = atlasSlashRegistry()?.resolve?.(token);
      if (!entity) continue;
      if (entity.kind === "metric" && entity.family === "indicator" && tradingIndicatorDefinitionByCommand(token)) {
        continue;
      }
      if (entity.kind === "metric" && entity.family === "candle") summary.candles.push(token);
      else if (entity.kind === "metric" && entity.family === "indicator") summary.indicatorMetrics.push(token);
      else if (entity.kind === "program" && entity.family === "strategy") summary.strategies.push(token);
      else if (entity.kind === "program" && entity.family === "program") summary.programs.push(token);
      else if (entity.kind === "program" && entity.family === "create") summary.creates.push(token);
      else if (entity.kind === "metric" && (entity.family === "geo" || entity.family === "minigeo")) summary.geos.push(token);
    }
    const messages = [];
    if (summary.candles.length) messages.push(`${summary.candles.join(" ")} added as candle metric reference${summary.candles.length > 1 ? "s" : ""}.`);
    if (summary.indicatorMetrics.length) messages.push(`${summary.indicatorMetrics.join(" ")} added as indicator metric reference${summary.indicatorMetrics.length > 1 ? "s" : ""}.`);
    if (summary.strategies.length) messages.push(`${summary.strategies.join(" ")} staged as strategy token${summary.strategies.length > 1 ? "s" : ""}.`);
    if (summary.programs.length) messages.push(`${summary.programs.join(" ")} staged as Atlas program token${summary.programs.length > 1 ? "s" : ""}.`);
    if (summary.creates.length) messages.push(`${summary.creates.join(" ")} available as create target${summary.creates.length > 1 ? "s" : ""}.`);
    if (summary.geos.length) messages.push(`${summary.geos.join(" ")} added as Atlas geo metric reference${summary.geos.length > 1 ? "s" : ""}.`);
    if (!messages.length) return null;
    return {
      ok: true,
      label: "atlas tokens staged",
      message: messages.join(" "),
    };
  }

  async function routeTradingCommand(text = "", meta = {}) {
    const raw = String(text || "").trim();
    const normalized = raw.toLowerCase();
    const actions = [];
    const routeTokens = extractTradingSlashTokens(raw);
    if (routeTokens.includes("/strategy_") && /\b(stop|off|disable|halt|pause)\b/.test(normalized)) {
      return stopStrategyLab("local_route");
    }
    if (routeTokens.includes("/backtest_")) {
      const reusablePrompt = raw
        .replace(/\/backtest_\b/ig, "")
        .replace(/\/[a-z0-9][a-z0-9_-]{0,63}\b/ig, "")
        .trim()
        || state.strategyLab?.prompt
        || state.strategyLab?.spec?.sourceText
        || raw;
      return await createAndTestStrategyFromPrompt(reusablePrompt, {
        source: meta?.source || "backtest_route",
        turnId: meta?.turnId || "",
        sessionJobId: meta?.sessionJobId || "",
      });
    }
    if (routeTokens.includes("/create_") && routeTokens.includes("/strategy_")) {
      return await createAndTestStrategyFromPrompt(raw, {
        source: meta?.source || "local_route",
        turnId: meta?.turnId || "",
        sessionJobId: meta?.sessionJobId || "",
      });
    }
    const indicatorCommand = applyTradingIndicatorCommand(raw);
    const atlasEntityCommand = applyTradingAtlasEntityCommand(raw);
    if (indicatorCommand && atlasEntityCommand) {
      return {
        ok: true,
        label: `${indicatorCommand.label} + atlas tokens staged`,
        message: `${indicatorCommand.message} ${atlasEntityCommand.message}`.trim(),
      };
    }
    if (indicatorCommand) return indicatorCommand;
    if (atlasEntityCommand) return atlasEntityCommand;
    if (!raw) {
      return {
        ok: true,
        label: "empty local trading command",
        message: "Trading command mode is active. No LLM is involved right now, so this bar expects a local Forge trading command like `alert`, `replay`, `indicator`, `compare BTCUSD`, or `load H1 EURUSD`.",
      };
    }
    const granularity = commandGranularityFromText(raw);
    if (granularity && granularity !== state.selectedGranularity) {
      await selectGranularity(granularity);
      actions.push(`timeframe -> ${timeframeLabel(granularity)}`);
    }
    const brokerKind = commandBrokerKind(normalized);
    if (brokerKind && brokerKind !== selectedBrokerKind()) {
      await selectTradingBroker(brokerKind);
      actions.push(`broker -> ${selectedBrokerLabel()}`);
    }
    const displayMode = commandChartDisplayMode(normalized);
    if (displayMode && displayMode !== state.chartDisplayMode) {
      selectChartDisplayMode(displayMode);
      actions.push(`chart mode -> ${displayMode}`);
    }
    const mentionedAssets = findMentionedAssets(raw);
    const compareIntent = /\bcompare\b|\bcorrelat|\bversus\b|\bvs\b/.test(normalized);
    if (compareIntent) {
      if (/clear compare|remove compare|reset compare|stop compare/.test(normalized)) {
        if (mentionedAssets.length && /\bremove\b|\bdelete\b|\bstop\b/.test(normalized)) {
          const removeSet = new Set(mentionedAssets.map((asset) => asset.name));
          state.compareInstruments = state.compareInstruments.filter((name) => !removeSet.has(name));
          invalidateAssetUniverseCache();
          await syncComparisonSeries();
          actions.push(`compare -> removed ${mentionedAssets.map((asset) => brokerInstrumentCode(asset.name) || asset.name).join(", ")}`);
        } else {
          state.compareInstruments = [];
          invalidateAssetUniverseCache();
          await syncComparisonSeries();
          actions.push("compare -> cleared");
        }
      } else {
        let compareAdded = 0;
        for (const asset of mentionedAssets) {
          if (!asset?.name || asset.name === state.selectedInstrument || state.compareInstruments.includes(asset.name)) continue;
          state.compareInstruments = [...state.compareInstruments, asset.name];
          invalidateAssetUniverseCache();
          compareAdded += 1;
        }
        if (compareAdded > 0) {
          await syncComparisonSeries();
          actions.push(`compare -> ${state.compareInstruments.map((name) => brokerInstrumentCode(name) || name).join(", ")}`);
        }
      }
    } else if (mentionedAssets.length) {
      const nextInstrument = mentionedAssets[0]?.name || "";
      if (nextInstrument && nextInstrument !== state.selectedInstrument) {
        await selectInstrument(nextInstrument);
        actions.push(`instrument -> ${brokerInstrumentCode(nextInstrument) || nextInstrument}`);
      }
    }
    const xAxisMode = commandXAxisMode(normalized);
    if (xAxisMode) {
      canvasBridge()?.applyTradingXAxisMetric?.(xAxisMode);
      actions.push(`x axis -> ${xAxisMode}`);
    }
    const yAxisMode = commandYAxisMode(normalized);
    if (yAxisMode) {
      canvasBridge()?.applyTradingYAxisMetric?.(yAxisMode);
      actions.push(`y axis -> ${yAxisMode}`);
    }
    const timeZoneId = commandTimeZoneId(normalized);
    if (timeZoneId) {
      canvasBridge()?.applyTradingTimeZone?.(timeZoneId);
      actions.push(`time zone -> ${timeZoneId}`);
    }
    const snapshot = tradingContextSnapshot();
    const wantSessionBreaks = /\bsession breaks?\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with)\b/, /\b(hide|disable|turn off|without)\b/)
      : null;
    if (wantSessionBreaks !== null && wantSessionBreaks !== !!snapshot.chart?.axes?.sessionBreaks) {
      canvasBridge()?.applyTradingTimeAction?.("toggle-session-breaks");
      actions.push(`session breaks -> ${wantSessionBreaks ? "on" : "off"}`);
    }
    const wantSignalMarkers = /\bsignal markers?\b|\bforge signal markers?\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with)\b/, /\b(hide|disable|turn off|without)\b/)
      : null;
    if (wantSignalMarkers !== null && wantSignalMarkers !== !!snapshot.chart?.axes?.signalMarkers) {
      canvasBridge()?.applyTradingTimeAction?.("toggle-signal-markers");
      actions.push(`signal markers -> ${wantSignalMarkers ? "on" : "off"}`);
    }
    const wantAutoFit = /\bauto fit\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with|auto fit)\b/, /\b(hide|disable|turn off|without|manual)\b/)
      : null;
    if (wantAutoFit !== null && wantAutoFit !== !!snapshot.chart?.axes?.autoFit) {
      canvasBridge()?.applyTradingScaleAction?.("auto-fit");
      actions.push(`auto fit -> ${wantAutoFit ? "on" : "off"}`);
    }
    const wantInvert = /\binvert scale\b|\binvert price scale\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with|invert)\b/, /\b(hide|disable|turn off|without|normal)\b/)
      : null;
    if (wantInvert !== null && wantInvert !== !!snapshot.chart?.axes?.invertPriceScale) {
      canvasBridge()?.applyTradingScaleAction?.("invert-price-scale");
      actions.push(`invert scale -> ${wantInvert ? "on" : "off"}`);
    }
    const wantLabels = /\blabels?\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with)\b/, /\b(hide|disable|turn off|without)\b/)
      : null;
    if (wantLabels !== null && wantLabels !== !!snapshot.chart?.axes?.showPriceLabels) {
      canvasBridge()?.applyTradingScaleAction?.("toggle-labels");
      actions.push(`labels -> ${wantLabels ? "on" : "off"}`);
    }
    const wantGrid = /\bgrid\b|\blines\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with)\b/, /\b(hide|disable|turn off|without)\b/)
      : null;
    if (wantGrid !== null && wantGrid !== !!snapshot.chart?.axes?.showGridLines) {
      canvasBridge()?.applyTradingScaleAction?.("toggle-lines");
      actions.push(`grid -> ${wantGrid ? "on" : "off"}`);
    }
    const wantPlusButton = /\bplus button\b|\bprice plus\b/.test(normalized)
      ? commandIntentState(normalized, /\b(show|enable|turn on|with)\b/, /\b(hide|disable|turn off|without)\b/)
      : null;
    if (wantPlusButton !== null && wantPlusButton !== !!snapshot.chart?.axes?.showPlusButton) {
      canvasBridge()?.applyTradingScaleAction?.("toggle-plus-button");
      actions.push(`plus button -> ${wantPlusButton ? "on" : "off"}`);
    }
    if (/\b(open|show)\b.*\borders?\b|\border entry\b/.test(normalized)) {
      canvasBridge()?.setRightPanelMode?.("orders", true);
      actions.push("right panel -> orders");
    } else if (/\bbloomberg\b|\blive news\b|\bmarket news\b/.test(normalized)) {
      canvasBridge()?.setRightPanelMode?.("bloomberg", true);
      actions.push("right panel -> bloomberg");
    } else if (/\b(open|show)\b.*\b(console|kasm|pine|rust)\b/.test(normalized)) {
      canvasBridge()?.setRightPanelMode?.("console", true);
      actions.push("right panel -> console");
    } else if (/\b(close|hide)\b.*\b(right panel|panel)\b/.test(normalized)) {
      canvasBridge()?.setRightPanelOpen?.(false);
      actions.push("right panel -> closed");
    } else if (/\b(open|show)\b.*\b(right panel|panel)\b/.test(normalized)) {
      canvasBridge()?.setRightPanelOpen?.(true);
      actions.push("right panel -> open");
    }
    if (/\bindicator/.test(normalized)) {
      const open = !/\b(off|close|hide|disable)\b/.test(normalized);
      setTradingChatSubbarMode(open ? "indicators" : "none");
      actions.push(`indicators panel -> ${open ? "open" : "closed"}`);
    }
    if (/\breplay\b/.test(normalized)) {
      const open = !/\b(off|close|hide|stop|disable)\b/.test(normalized);
      setTradingChatSubbarMode(open ? "replay" : "none");
      actions.push(`replay mode -> ${open ? "on" : "off"}`);
    }
    if (/\balert\b/.test(normalized)) {
      if (/\b(close|hide)\b/.test(normalized)) {
        if (state.alertModalOpen) closeAlertModal();
        if (state.chatSubbarMode === "alert") setTradingChatSubbarMode("none");
        actions.push("alert panel -> closed");
      } else {
        setTradingChatSubbarMode("alert");
        actions.push("alert tools -> open");
      }
    }
    const stagedDraft = stageOrderDraftFromCommand(raw, normalized);
    if (stagedDraft) {
      state.ordersOutput = [
        "Forge local trading command captured.",
        `draft=${raw}`,
        `instrument=${stagedDraft.instrument}`,
        `side=${stagedDraft.side}`,
        `units=${stagedDraft.units}`,
        `type=${stagedDraft.orderType}`,
        "Safety rail: no broker order was fired automatically in command-only mode.",
      ].join("\n");
      canvasBridge()?.refreshRightPanel?.();
      actions.push("order draft -> staged locally");
    }
    syncTradingChatActions();
    syncTradingHeader();
    renderTimeframeRail();
    renderLeftPanel();
    const fallback = actions.length
      ? `Forge handled this locally:\n- ${actions.join("\n- ")}`
      : "Trading command mode is active, but I did not map this sentence to a safe local action yet. Try commands like `load H1 BTCUSD`, `compare XAUUSD`, `set x axis volatility`, `set y axis conviction`, `timezone new york`, `show orders panel`, `bloomberg`, `alert`, `replay`, or `indicators`.";
    return {
      ok: true,
      label: actions.length ? "local trading command executed" : "local trading command not mapped",
      message: fallback,
      actions,
      meta,
    };
  }

  function consoleTemplate(language) {
    return state.consoleDrafts[language] || "";
  }

  function consoleContextInsertion(language, context) {
    if (language === "kasm") {
      return [
        "# trading context",
        `use instrument ${JSON.stringify(context.instrument)}`,
        `use granularity ${JSON.stringify(context.granularity)}`,
        `use price ${JSON.stringify(formatNumber(context.price?.mid))}`,
      ].join("\n");
    }
    if (language === "pine") {
      return [
        "// trading context",
        `// instrument: ${context.instrument}`,
        `// granularity: ${context.granularity}`,
        `// mid: ${formatNumber(context.price?.mid)}`,
      ].join("\n");
    }
    return [
      "// trading context",
      `let instrument = "${context.instrument}";`,
      `let granularity = "${context.granularity}";`,
      `let mid_price = "${formatNumber(context.price?.mid)}";`,
    ].join("\n");
  }

  function routeLabel(language) {
    if (language === "pine") return "trading / pinescript draft";
    if (language === "kasm") return "trading / kasm plan";
    return "trading / rust runtime";
  }

  function injectTradingChatToken(token = "") {
    const clean = String(token || "").trim();
    if (!clean) return;
    try {
      window.__forgeTradingInjectChatToken?.(clean);
    } catch (_) {}
  }

  function ctrlClickRequested(event) {
    return !!(event?.ctrlKey || event?.metaKey);
  }

  function eventTargetElement(event) {
    const target = event?.target;
    if (target instanceof Element) return target;
    if (target && target.nodeType === Node.TEXT_NODE) return target.parentElement;
    return null;
  }

  function handleTradingTokenClick(event, token = "") {
    if (!state.active || !ctrlClickRequested(event)) return false;
    event.preventDefault();
    event.stopPropagation();
    injectTradingChatToken(token);
    return true;
  }

  async function runTradingConsole(language, script, meta) {
    const context = defaultConsoleContext();
    if (language === "pine") {
      return {
        language: "pine",
        status: "draft",
        scriptHash: meta.scriptHash,
        stdout: [
          "PineScript draft prepared.",
          `instrument=${context.instrument}`,
          `granularity=${context.granularity}`,
          "Execution is external to Forge; use this draft for TradingView or translation.",
        ].join("\n"),
      };
    }
    if (language === "kasm") {
      const steps = String(script || "")
        .split(/\r?\n/)
        .map((line, index) => ({ line: index + 1, command: line.trim() }))
        .filter((step) => step.command);
      return {
        language: "kasm",
        status: "ok",
        scriptHash: meta.scriptHash,
        steps,
        context,
      };
    }
    if (!hasTauriInvoke()) {
      return {
        language: "rust",
        status: "preview",
        scriptHash: meta.scriptHash,
        stdout: "Rust execution needs the Forge desktop runtime.",
      };
    }
    const response = await invoke("banger_run_rust_console", {
      request: {
        script,
        contextJson: JSON.stringify(context),
        scriptHash: meta.scriptHash,
      },
    });
    return {
      language: "rust",
      tool: "banger_run_rust_console",
      ...response,
    };
  }

  function panelBridge() {
    return {
      isActive: () => state.active,
      isRuntimeInvolved,
      anyRuntimeInvolved,
      setRuntimeInvolvement,
      setAllRuntimeInvolvement,
      buildContextSnapshot: tradingContextSnapshot,
      buildContextDigest: tradingContextDigestV3,
      buildContextEnvelope: tradingContextEnvelope,
      startStrategyCreation: async ({ prompt = "", source = "chat_command" } = {}) => createAndTestStrategyFromPrompt(prompt, { source }),
      stopStrategyLab,
      strategyContextPacket: strategyLabContextPacket,
      strategyState: summarizeStrategyLabState,
      routeLocalCommand: routeTradingCommand,
      consoleTemplate,
      consoleContext: defaultConsoleContext,
      consoleContextInsertion,
      routeLabel,
      runConsole: runTradingConsole,
      renderOrders: renderOrdersPanel,
    };
  }

  function installPanelBridge() {
    const bridge = panelBridge();
    window.__forgeTradingPanelBridge = bridge;
    window.__forgeTradingChatBridge = bridge;
    window.__forgeTradingActive = state.active;
  }

  async function loadSnapshot(options = {}) {
    if (!options.force && state.snapshot) return state.snapshot;
    if (!hasTauriInvoke()) {
      state.snapshot = {
        config: {
          available: false,
          source: "preview",
          message: "Browser preview mode",
          baseUrl: "https://api-fxpractice.oanda.com",
        },
        account: {
          alias: "Preview",
          currency: "USD",
          balance: 25000,
          nav: 25142.4,
          unrealizedPl: 142.4,
          marginAvailable: 18900,
        openTradeCount: 2,
        openPositionCount: 1,
        pendingOrderCount: 3,
      },
      instruments: PREVIEW_ASSETS,
      assetCatalog: PREVIEW_ASSETS.map((item) => ({
        ...item,
        granularities: FULL_HISTORY_GRANULARITIES.slice(),
        rows: 0,
        firstTime: "",
        lastTime: "",
        updatedAtMs: 0,
      })),
      pendingOrders: [],
      openTrades: [],
      historyFiles: FULL_HISTORY_GRANULARITIES.map((granularity) => ({
        instrument: DEFAULT_INSTRUMENT,
        granularity,
      })),
    };
      syncSnapshotCatalog(state.snapshot.historyFiles, {
        replace: true,
        assets: state.snapshot.assetCatalog,
      });
      return state.snapshot;
    }
    state.snapshot = await invoke("trading_oanda_snapshot");
    syncSnapshotCatalog(Array.isArray(state.snapshot?.historyFiles) ? state.snapshot.historyFiles : [], {
      replace: true,
      assets: state.snapshot?.assetCatalog,
    });
    if (state.snapshot?.config?.baseUrl) {
      state.credentials.baseUrl = state.snapshot.config.baseUrl;
    }
    return state.snapshot;
  }

  async function loadHistorySeries(instrument = state.selectedInstrument, granularity = state.selectedGranularity, options = {}) {
    const targetInstrument = String(instrument || state.selectedInstrument || "").trim();
    const targetGranularity = String(granularity || state.selectedGranularity || "").trim().toUpperCase();
    const maxRows = Math.max(0, Number(options?.maxRows || 0));
    const cacheKey = chartCacheKey(targetInstrument, targetGranularity);
    const cached = getCachedSeries(targetInstrument, targetGranularity);
    const expectedRows = expectedHistoryRows(targetInstrument, targetGranularity);
    const requiredRows = maxRows > 0 ? maxRows : expectedRows;
    if (cached.length && (!requiredRows || cached.length >= requiredRows)) return cached;
    if (state.historySeriesMisses.has(cacheKey)) return [];
    if (!hasTauriInvoke()) return [];
    const exists = tradingCatalogIndex().pairRows.has(cacheKey);
    if (!exists) {
      rememberHistorySeriesMiss(cacheKey);
      return [];
    }
    const promiseKey = `${cacheKey}::${maxRows > 0 ? maxRows : "full"}`;
    if (state.historySeriesPromises.has(promiseKey)) {
      return state.historySeriesPromises.get(promiseKey);
    }
    const promise = (async () => {
      const freshCached = getCachedSeries(targetInstrument, targetGranularity);
      const freshExpectedRows = expectedHistoryRows(targetInstrument, targetGranularity);
      const freshRequiredRows = maxRows > 0 ? maxRows : freshExpectedRows;
      if (freshCached.length && (!freshRequiredRows || freshCached.length >= freshRequiredRows)) return freshCached;
      const response = await invoke("trading_chart_series", {
        request: {
          instrument: targetInstrument,
          granularity: targetGranularity,
          maxRows,
        },
      });
      const candles = Array.isArray(response?.candles) ? response.candles : [];
      if (candles.length) {
        return setCachedSeries(targetInstrument, targetGranularity, candles);
      }
      rememberHistorySeriesMiss(cacheKey);
      return [];
    })();
    state.historySeriesPromises.set(promiseKey, promise);
    try {
      return await promise;
    } catch (_) {
      return [];
    } finally {
      if (state.historySeriesPromises.get(promiseKey) === promise) {
        state.historySeriesPromises.delete(promiseKey);
      }
    }
  }

  async function loadMarketFeed(instrument = state.selectedInstrument, granularity = state.selectedGranularity, options = {}) {
    const targetInstrument = String(instrument || state.selectedInstrument || "").trim();
    const targetGranularity = String(granularity || state.selectedGranularity || "").trim().toUpperCase();
    const cacheCandles = options.cacheCandles !== false;
    if (!hasTauriInvoke()) {
      const market = {
        instrument: targetInstrument,
        granularity: targetGranularity,
        price: {
          instrument: targetInstrument,
          bid: 2.846,
          ask: 2.851,
          mid: 2.8485,
          spread: 0.005,
          time: new Date().toISOString(),
        },
        book: {
          kind: "synthetic_ladder",
          note: "Preview order book",
        },
        pendingOrders: [],
        openTrades: [],
        candles: previewCandles(240, targetGranularity === "D" ? 24 * 3_600_000 : 4 * 3_600_000),
        alerts: state.alerts.slice(),
        alertEvents: [],
      };
      if (cacheCandles) {
        setCachedSeries(targetInstrument, targetGranularity, market.candles);
      }
      return market;
    }
    const market = await invoke("trading_oanda_market_feed", {
      request: {
        instrument: targetInstrument,
        granularity: targetGranularity,
        count: 240,
      },
    });
    if (cacheCandles && Array.isArray(market?.candles) && market.candles.length) {
      const existing = getCachedSeries(targetInstrument, targetGranularity);
      const merged = existing.length
        ? mergeCandles(existing, market.candles)
        : market.candles;
      setCachedSeries(targetInstrument, targetGranularity, merged);
    }
    return market;
  }

  async function refreshTradingData(options = {}) {
    if (!state.active) return;
    if (state.refreshPromise) {
      state.pendingRefreshOptions = { ...(state.pendingRefreshOptions || {}), ...options };
      return state.refreshPromise;
    }
    state.refreshPromise = (async () => {
      const requestedInstrument = String(options.instrument || state.selectedInstrument || "").trim();
      const requestedGranularity = String(options.granularity || state.selectedGranularity || "").trim().toUpperCase();
      const requestKey = chartCacheKey(requestedInstrument, requestedGranularity);
      const immediateCached = getCachedSeries(requestedInstrument, requestedGranularity);
      if (
        requestedInstrument === state.selectedInstrument
        && requestedGranularity === state.selectedGranularity
        && immediateCached.length
        && state.renderedSeriesKey !== requestKey
      ) {
        setCanvasSeries(immediateCached, { preserveViewport: false });
      }
      if (options.reloadSnapshot || !state.snapshot) {
        await loadSnapshot({ force: !!options.reloadSnapshot });
      }
      const selectedHistoryPromise = options.syncSelectedHistory
        ? ensureSelectedHistorySync({
          instrument: requestedInstrument,
          granularity: requestedGranularity,
          force: !!options.forceSelectedHistorySync,
        })
        : null;
      const market = await loadMarketFeed(requestedInstrument, requestedGranularity, {
        cacheCandles: false,
      });
      const baseHistory = options.liveOnly
        ? getCachedSeries(requestedInstrument, requestedGranularity)
        : await loadHistorySeries(requestedInstrument, requestedGranularity);
      const recent = Array.isArray(market?.candles) ? market.candles : [];
      const fallback = state.candles.length
        && requestedInstrument === state.selectedInstrument
        && requestedGranularity === state.selectedGranularity
        ? state.candles
        : getCachedSeries(requestedInstrument, requestedGranularity);
      const candles = baseHistory.length
        ? mergeCandles(baseHistory, recent)
        : recent.length
          ? recent
          : fallback;
      const hasRealSeries = baseHistory.length > 0 || recent.length > 0;
      const replacingPreviewSeed = state.previewSeedActive && hasRealSeries;
      if (
        requestedInstrument !== state.selectedInstrument
        || requestedGranularity !== state.selectedGranularity
      ) {
        return;
      }
      state.market = market;
      if (Array.isArray(state.market?.alerts)) setAlertRecords(state.market.alerts);
      syncCanvasAlerts();
      handleAlertEvents(Array.isArray(state.market?.alertEvents) ? state.market.alertEvents : []);
      if (state.alertModalOpen) renderAlertModal();
      if (candles.length && recent.length) {
        setCachedSeries(requestedInstrument, requestedGranularity, candles);
      }
      setCanvasSeries(candles, { preserveViewport: !replacingPreviewSeed });
      if (hasRealSeries) {
        state.previewSeedActive = false;
      }
      if (!options.liveOnly) {
        await syncComparisonSeries();
        renderLeftPanel();
        renderTimeframeRail();
        updateOrderFormInstrument();
        installPanelBridge();
        syncTradingIndicatorsToCanvas();
      }
      if (!options.liveOnly && options.refreshRightPanel !== false) {
        canvasBridge()?.refreshRightPanel?.();
      }
      if (selectedHistoryPromise) {
        await selectedHistoryPromise;
        if (
          requestedInstrument === state.selectedInstrument
          && requestedGranularity === state.selectedGranularity
        ) {
          deleteCachedSeries(requestedInstrument, requestedGranularity);
          const syncedHistory = await loadHistorySeries(requestedInstrument, requestedGranularity);
          const liveRecent = Array.isArray(state.market?.candles) ? state.market.candles : [];
          const syncedCandles = syncedHistory.length
            ? mergeCandles(syncedHistory, liveRecent)
            : liveRecent.length
              ? liveRecent
              : state.candles;
          setCanvasSeries(syncedCandles, { preserveViewport: true });
          renderLeftPanel();
          renderTimeframeRail();
        }
      }
    })().finally(() => {
      state.refreshPromise = null;
      const pending = state.pendingRefreshOptions;
      state.pendingRefreshOptions = null;
      if (pending && state.active) {
        void refreshTradingData(pending);
      }
    });
    await state.refreshPromise;
  }

  function startPolling() {
    stopPolling();
    state.pollTimer = window.setInterval(() => {
      void refreshTradingData({
        liveOnly: true,
        refreshRightPanel: false,
        syncSelectedHistory: false,
      });
    }, POLL_MS);
  }

  function stopPolling() {
    if (state.pollTimer) {
      window.clearInterval(state.pollTimer);
      state.pollTimer = 0;
    }
  }

  async function selectInstrument(instrument) {
    if (!instrument || instrument === state.selectedInstrument) return;
    state.selectedInstrument = instrument;
    state.compareInstruments = state.compareInstruments.filter((item) => item !== instrument);
    setAddedChartInstruments(addedChartInstruments().filter((item) => item !== instrument));
    invalidateAssetUniverseCache();
    closeHeaderMenu();
    const granularities = availableGranularities(instrument);
    if (!granularities.includes(state.selectedGranularity)) {
      state.selectedGranularity = granularities[0] || DEFAULT_GRANULARITY;
    }
    updateOrderFormInstrument();
    renderLeftPanel();
    renderTimeframeRail();
    syncTradingHeader();
    seedTradingSurfaceImmediate({ allowEmpty: false });
    void refreshTradingData({
      syncSelectedHistory: true,
      forceSelectedHistorySync: true,
      instrument,
      granularity: state.selectedGranularity,
    });
  }

  async function selectGranularity(granularity) {
    if (!granularity || granularity === state.selectedGranularity) return;
    state.selectedGranularity = granularity;
    closeHeaderMenu();
    renderTimeframeRail();
    syncTradingHeader();
    seedTradingSurfaceImmediate({ allowEmpty: false });
    void refreshTradingData({
      syncSelectedHistory: true,
      forceSelectedHistorySync: true,
      instrument: state.selectedInstrument,
      granularity,
    });
  }

  async function selectComparisonInstrument(instrument) {
    const next = String(instrument || "").trim();
    if (!next || next === state.selectedInstrument) return;
    state.compareInstruments = state.compareInstruments.includes(next)
      ? state.compareInstruments.filter((item) => item !== next)
      : [...state.compareInstruments, next];
    invalidateAssetUniverseCache();
    state.uiCache.headerMenuKey = "";
    renderHeaderMenu("compare");
    await syncComparisonSeries();
  }

  async function selectAddInstrument(instrument) {
    const next = String(instrument || "").trim();
    if (!next || next === state.selectedInstrument) return;
    const current = addedChartInstruments();
    setAddedChartInstruments(
      current.includes(next)
        ? current.filter((item) => item !== next)
        : [...current, next],
    );
    invalidateAssetUniverseCache();
    state.uiCache.headerMenuKey = "";
    renderHeaderMenu("add");
    await syncComparisonSeries();
  }

  function selectChartDisplayMode(mode) {
    const next = normalizeChartDisplayMode(mode);
    if (next === state.chartDisplayMode) {
      closeHeaderMenu();
      return;
    }
    state.chartDisplayMode = next;
    setCanvasSeries(state.candles);
    closeHeaderMenu();
  }

  async function selectTradingBroker(kind) {
    const next = String(kind || "").trim().toLowerCase();
    if (!next) return;
    brokerApi()?.activateBroker?.(next);
    state.universeHistorySyncDone = false;
    closeHeaderMenu();
    syncTradingHeader();
    await refreshTradingData({ reloadSnapshot: true, syncSelectedHistory: true, forceSelectedHistorySync: true });
    void ensureUniverseHistorySync();
  }

  async function saveCredentials() {
    await invoke("trading_oanda_save_credentials", {
      request: {
        accountId: state.credentials.accountId,
        apiKey: state.credentials.apiKey,
        baseUrl: state.credentials.baseUrl,
      },
    });
    state.universeHistorySyncDone = false;
    state.ordersOutput = "OANDA credentials saved locally in Forge.";
    await refreshTradingData({ reloadSnapshot: true, syncSelectedHistory: true, forceSelectedHistorySync: true });
    void ensureUniverseHistorySync();
  }

  async function syncUniverseHistory() {
    state.ordersOutput = "Syncing OANDA universe history from 2006 with minimal native feeds and local timeframe rebuilds…";
    canvasBridge()?.refreshRightPanel?.();
    const response = await invoke("trading_oanda_sync_history", {
      request: {
        granularities: FULL_HISTORY_GRANULARITIES.slice(),
      },
    });
    syncSnapshotCatalog(Array.isArray(response?.files) ? response.files : [], {
      replace: true,
      assets: response?.assets,
    });
    state.universeHistorySyncDone = true;
    state.ordersOutput = [
      "Full OANDA universe history synced.",
      ...((response?.notes || []).slice(0, 4)),
    ].join("\n");
    await refreshTradingData();
  }

  async function placeOrder() {
    const response = await invoke("trading_oanda_place_order", {
      request: {
        instrument: state.orderForm.instrument || state.selectedInstrument,
        side: state.orderForm.side,
        units: Number(state.orderForm.units || 0),
        orderType: state.orderForm.orderType,
        limitPrice: state.orderForm.limitPrice ? Number(state.orderForm.limitPrice) : null,
        stopLoss: state.orderForm.stopLoss ? Number(state.orderForm.stopLoss) : null,
        takeProfit: state.orderForm.takeProfit ? Number(state.orderForm.takeProfit) : null,
      },
    });
    state.ordersOutput = [
      response?.message || "Order sent.",
      `instrument=${response?.instrument || state.orderForm.instrument}`,
      `side=${response?.side || state.orderForm.side}`,
      `units=${response?.units ?? state.orderForm.units}`,
      `type=${response?.orderType || state.orderForm.orderType}`,
    ].join("\n");
    await refreshTradingData();
  }

  function appendSectionTitle(section, title, note) {
    const heading = document.createElement("h3");
    heading.className = "proof-section-title";
    heading.textContent = title;
    section.appendChild(heading);
    if (note) {
      const paragraph = document.createElement("p");
      paragraph.className = "proof-note";
      paragraph.textContent = note;
      section.appendChild(paragraph);
    }
  }

  function buildField(labelText, value, onInput, type = "text", extra = {}) {
    const label = document.createElement("label");
    label.className = "provider-model-field";
    if (extra.token) label.dataset.tradingToken = String(extra.token);
    const span = document.createElement("span");
    span.textContent = labelText;
    label.appendChild(span);
    let field;
    if (Array.isArray(extra.options)) {
      field = document.createElement("select");
      for (const option of extra.options) {
        const el = document.createElement("option");
        el.value = option.value;
        el.textContent = option.label;
        if (String(option.value) === String(value)) el.selected = true;
        field.appendChild(el);
      }
    } else {
      field = document.createElement("input");
      field.type = type;
      field.value = value;
      if (extra.placeholder) field.placeholder = extra.placeholder;
      if (extra.step) field.step = extra.step;
    }
    if (extra.token) field.dataset.tradingToken = String(extra.token);
    field.addEventListener("input", (event) => onInput(event.target.value));
    label.appendChild(field);
    return label;
  }

  function renderOrdersPanel(container) {
    if (!container) return;
    container.innerHTML = "";

    const shell = document.createElement("div");
    shell.className = "trading-order-shell";

    const feedSection = document.createElement("section");
    feedSection.className = "proof-section";
    appendSectionTitle(feedSection, "Market", activePriceLine());
    const feedGrid = document.createElement("div");
    feedGrid.className = "trading-order-grid";
      feedGrid.appendChild(buildField(
        "Chart timeframe",
        state.selectedGranularity,
        (value) => { void selectGranularity(String(value || DEFAULT_GRANULARITY).toUpperCase()); },
        "text",
        {
          token: `<timeframe:${state.selectedGranularity}>`,
          options: availableGranularities()
            .map((granularity) => ({ value: granularity, label: granularity })),
        }
      ));
    feedGrid.appendChild(buildField(
      "Instrument",
        state.selectedInstrument,
        (value) => { void selectInstrument(String(value || DEFAULT_INSTRUMENT)); },
        "text",
        {
          token: tokenForInstrumentName(state.selectedInstrument),
          options: availableAssets().map((item) => ({ value: item.name, label: item.name })),
        }
      ));
    feedSection.appendChild(feedGrid);
    shell.appendChild(feedSection);

    const credsSection = document.createElement("section");
    credsSection.className = "proof-section";
    appendSectionTitle(
      credsSection,
      "OANDA",
      state.snapshot?.config?.available
        ? `Source: ${state.snapshot.config.source}`
        : "Configure credentials here or via the Trading bot environment."
    );
    const credsGrid = document.createElement("div");
    credsGrid.className = "trading-order-grid";
      credsGrid.appendChild(buildField("Account ID", state.credentials.accountId, (value) => { state.credentials.accountId = value; }, "text", { token: `<broker_account:${selectedBrokerLabel()}>` }));
      credsGrid.appendChild(buildField("API key", state.credentials.apiKey, (value) => { state.credentials.apiKey = value; }, "password", { token: `<broker_api_key:${selectedBrokerLabel()}>` }));
      credsGrid.appendChild(buildField("Base URL", state.credentials.baseUrl, (value) => { state.credentials.baseUrl = value; }, "text", { token: `<broker_base_url:${state.credentials.baseUrl || "n/a"}>` }));
    credsSection.appendChild(credsGrid);
    const credsActions = document.createElement("div");
    credsActions.className = "trading-order-actions";
      const saveBtn = document.createElement("button");
      saveBtn.className = "provider-flat-btn";
      saveBtn.type = "button";
      saveBtn.textContent = "Save credentials";
      saveBtn.dataset.tradingToken = "<broker_credentials:save>";
      saveBtn.addEventListener("click", () => void saveCredentials());
      const syncBtn = document.createElement("button");
      syncBtn.className = "provider-flat-btn provider-flat-btn-strong";
      syncBtn.type = "button";
      syncBtn.textContent = "Sync full OANDA history";
      syncBtn.dataset.tradingToken = "<history_sync:full_oanda>";
      syncBtn.addEventListener("click", () => void syncUniverseHistory());
    credsActions.appendChild(saveBtn);
    credsActions.appendChild(syncBtn);
    credsSection.appendChild(credsActions);
    shell.appendChild(credsSection);

    const orderSection = document.createElement("section");
    orderSection.className = "proof-section";
    appendSectionTitle(orderSection, "Order entry", "Orders are routed through OANDA v20.");
    const orderGrid = document.createElement("div");
    orderGrid.className = "trading-order-grid";
      orderGrid.appendChild(buildField("Instrument", state.orderForm.instrument, (value) => { state.orderForm.instrument = value; }, "text", {
        token: tokenForOrderDraftField("instrument"),
        options: availableAssets().map((item) => ({ value: item.name, label: item.name })),
      }));
      orderGrid.appendChild(buildField("Side", state.orderForm.side, (value) => { state.orderForm.side = value; }, "text", {
        token: tokenForOrderDraftField("side"),
        options: [
          { value: "BUY", label: "BUY" },
          { value: "SELL", label: "SELL" },
        ],
      }));
      orderGrid.appendChild(buildField("Units", state.orderForm.units, (value) => { state.orderForm.units = value; }, "number", { step: "1", token: tokenForOrderDraftField("units") }));
      orderGrid.appendChild(buildField("Type", state.orderForm.orderType, (value) => { state.orderForm.orderType = value; }, "text", {
        token: tokenForOrderDraftField("orderType"),
        options: [
          { value: "MARKET", label: "MARKET" },
          { value: "LIMIT", label: "LIMIT" },
        ],
      }));
      orderGrid.appendChild(buildField("Limit price", state.orderForm.limitPrice, (value) => { state.orderForm.limitPrice = value; }, "number", { step: "0.001", token: tokenForOrderDraftField("limitPrice") }));
      orderGrid.appendChild(buildField("Stop loss", state.orderForm.stopLoss, (value) => { state.orderForm.stopLoss = value; }, "number", { step: "0.001", token: tokenForOrderDraftField("stopLoss") }));
      orderGrid.appendChild(buildField("Take profit", state.orderForm.takeProfit, (value) => { state.orderForm.takeProfit = value; }, "number", { step: "0.001", token: tokenForOrderDraftField("takeProfit") }));
    orderSection.appendChild(orderGrid);
    const orderActions = document.createElement("div");
    orderActions.className = "trading-order-actions";
      const sendBtn = document.createElement("button");
      sendBtn.className = "provider-flat-btn provider-flat-btn-strong";
      sendBtn.type = "button";
      sendBtn.textContent = "Send order";
      sendBtn.dataset.tradingToken = "<order:submit>";
      sendBtn.addEventListener("click", () => void placeOrder());
    orderActions.appendChild(sendBtn);
    orderSection.appendChild(orderActions);
    shell.appendChild(orderSection);

    const strategy = summarizeStrategyLabState();
    const strategySection = document.createElement("section");
    strategySection.className = "proof-section trading-strategy-lab";
    appendSectionTitle(
      strategySection,
      "Strategy Lab",
      strategy.active
        ? "Local paper backtest and live candle watch. No broker order is placed."
        : "Use /create_ /strategy_ to generate and paper-test a strategy from the current chart."
    );
    const strategyCard = document.createElement("div");
    strategyCard.className = "trading-strategy-card";
    if (strategy.status === "needs_clarification") {
      const questions = strategy.questions.length ? strategy.questions : strategy.missingMetrics.map((item) => item.question).filter(Boolean);
      strategyCard.innerHTML = `
        <div class="trading-strategy-head">
          <span class="trading-strategy-token">/create_ /strategy_</span>
          <span class="trading-strategy-state">needs metrics</span>
        </div>
        <div class="trading-strategy-title">StrategySpec blocked</div>
        <div class="trading-strategy-reason">Forge refused to backtest ambiguous inputs.</div>
        <div class="trading-strategy-questions">
          ${questions.slice(0, 4).map((question) => `<span>${escapeHtml(question)}</span>`).join("")}
        </div>
      `;
    } else if (strategy.active) {
      const winRate = strategy.backtest.winRate == null ? "n/a" : `${formatNumber(strategy.backtest.winRate * 100, 1)}%`;
      const pnlValue = strategy.backtest.pnlDistance == null
        ? `${formatNumber(strategy.backtest.pnlPct, 2)}%`
        : formatNumber(strategy.backtest.pnlDistance, 5);
      const journalTail = Array.isArray(strategy.liveJob?.journalTail) ? strategy.liveJob.journalTail.slice(-3) : [];
      const robustness = strategy.backtest.robustness || {};
      const robustnessText = strategy.backtest.robustnessScore == null
        ? "n/a"
        : `${formatNumber(strategy.backtest.robustnessScore, 1)} ${strategy.backtest.robustnessGrade || ""}`;
      const paired = strategy.pairedProbe || strategy.backtest.pairedProbe || null;
      const computePlan = strategy.computePlan || strategy.backtest.computePlan || null;
      const cacheReport = computePlan?.cacheReport || null;
      const gpuPlan = computePlan?.gpuPlan || null;
      const pairedText = paired
        ? `${paired.edge || "flat"} L ${paired.longWinRate == null ? "n/a" : `${formatNumber(Number(paired.longWinRate) * 100, 1)}%`} / S ${paired.shortWinRate == null ? "n/a" : `${formatNumber(Number(paired.shortWinRate) * 100, 1)}%`}`
        : "n/a";
      const kasmText = computePlan
        ? `${Array.isArray(computePlan.dagNodes) ? computePlan.dagNodes.length : 0} nodes - saved ${computePlan.avoidedRecalculations ?? 0}`
        : "n/a";
      const gpuText = gpuPlan
        ? `${gpuPlan.kernel || "cube"} - ${formatNumber(gpuPlan.workItems || computePlan?.simulationCount || 0, 0)} work`
        : "n/a";
      const cacheText = cacheReport
        ? `hit ${cacheReport.hits ?? 0} / miss ${cacheReport.misses ?? 0}`
        : "n/a";
      strategyCard.innerHTML = `
        <div class="trading-strategy-head">
          <span class="trading-strategy-token">/create_ /strategy_</span>
          <span class="trading-strategy-state">${strategy.status === "target_met" ? "target met" : (strategy.live.signal || "HOLD")}</span>
        </div>
        <div class="trading-strategy-title">${strategy.title || "Strategy draft"}</div>
        <div class="trading-strategy-grid">
          <span>runner</span><strong>${strategy.runner?.engine || strategy.rules?.kind || "n/a"}</strong>
          <span>backtest</span><strong>${strategy.backtest.trades || 0} trades - ${winRate}</strong>
          <span>pnl</span><strong>${pnlValue}</strong>
          <span>best</span><strong>${strategy.backtest.direction || strategy.live.signal || "n/a"} - TP ${formatNumber(strategy.backtest.takeProfitDistance)}</strong>
          <span>robust</span><strong>${robustnessText}</strong>
          <span>pair</span><strong>${pairedText}</strong>
          <span>KASM</span><strong>${kasmText}</strong>
          <span>cache</span><strong>${cacheText}</strong>
          <span>GPU</span><strong>${gpuText}</strong>
        </div>
        ${computePlan ? `<div class="trading-strategy-robustness">
          ${(Array.isArray(computePlan.dagNodes) ? computePlan.dagNodes.slice(0, 5) : []).map((item) => `<span>${escapeHtml(`${item.id || "node"}:${item.cacheHit ? "hit" : "miss"}`)}</span>`).join("")}
        </div>` : ""}
        ${strategy.backtest.robustness ? `<div class="trading-strategy-robustness">
          <span>wf ${formatNumber(Number(robustness.walkForwardPassRate || 0) * 100, 0)}%</span>
          <span>stress ${formatNumber(Number(robustness.stressPassRate || 0) * 100, 0)}%</span>
          <span>months ${robustness.monthlyPositiveRate == null ? "n/a" : `${formatNumber(Number(robustness.monthlyPositiveRate) * 100, 0)}%`}</span>
        </div>` : ""}
        <div class="trading-strategy-reason">${strategy.live.reason || strategy.backtest.note || "Watching current candles."}</div>
        ${journalTail.length ? `<div class="trading-strategy-journal">
          ${journalTail.map((signal) => `<span>${escapeHtml(signal.status || "open")} ${escapeHtml(signal.direction || "")} ${escapeHtml(signal.entryTime || "")} ${escapeHtml(signal.outcome || "")}</span>`).join("")}
        </div>` : ""}
      `;
    } else {
      strategyCard.innerHTML = `
        <div class="trading-strategy-head">
          <span class="trading-strategy-token">/create_ /strategy_</span>
          <span class="trading-strategy-state">idle</span>
        </div>
        <div class="trading-strategy-reason">No strategy is running in Strategy Lab.</div>
      `;
    }
    strategySection.appendChild(strategyCard);
    shell.appendChild(strategySection);

    const logSection = document.createElement("section");
    logSection.className = "proof-section";
    appendSectionTitle(logSection, "Trading log", state.market?.book?.note || "Execution feedback.");
    const log = document.createElement("pre");
    log.className = "trading-order-log";
    log.textContent = state.ordersOutput;
    logSection.appendChild(log);
    shell.appendChild(logSection);

    container.appendChild(shell);
  }

  function activateTrading() {
    if (state.active) return;
    try {
      window.__forgeSwitchSection?.("alpha");
    } catch (_) {}
    try {
      if (window.__forgeWebExplorerIsActive?.()) window.__forgeCloseWebExplorer?.();
    } catch (_) {}
    state.active = true;
    window.ForgeSectionRegistry?.activate?.("trading");
    state.alertFormMode = "create";
    state.alertDraft = makeAlertDraft();
    state.snapshot = null;
    state.market = null;
    state.catalog = [];
    state.assetCatalog = [];
    invalidateAssetUniverseCache({ catalog: true });
    state.localCatalogPromise = null;
    state.uiCache.leftPanelKey = "";
    state.uiCache.headerMenuKey = "";
    state.uiCache.tradingHeaderKey = "";
    state.uiCache.indicatorDockKey = "";
    state.uiSnapshot = captureUiSnapshot();
    installPanelBridge();
    syncTradingIndicatorsToCanvas();
    ensureAlertUi();
    syncChartModeTrigger();
    syncProgramTrigger();
    syncAlertTrigger();
    els.content?.classList.add("trading-mode");
    els.canvasWrap?.classList.add("is-trading-mode");
    els.panelTabCompute?.click?.();
    updateOrderFormInstrument();
    syncTradingHeader();
    renderTimeframeRail();
    seedTradingSurfaceImmediate();
    canvasBridge()?.forceImmediateRender?.();
    try {
      if (window.__forgeBoomIsActive) window.__forgeCloseBoom?.();
    } catch (_) {}
    syncTradingWorkspaceButton();
    startPolling();
    try {
      window.__forgeScheduleBloombergIdlePrewarm?.();
    } catch (_) {}
    void (async () => {
      await refreshTradingData({
        reloadSnapshot: true,
        syncSelectedHistory: true,
        forceSelectedHistorySync: true,
      });
      await ensureUniverseHistorySync();
    })();
    void reloadAlerts();
  }

  function deactivateTrading() {
    if (!state.active) return;
    state.active = false;
    window.ForgeSectionRegistry?.deactivate?.("trading");
    state.chatSubbarMode = "none";
    state.chatSubbarExpanded = false;
    state.chatSubbarSection = "indicators";
    state.indicatorsModeActive = false;
    state.replayModeActive = false;
    stopPolling();
    window.__forgeTradingActive = false;
    installPanelBridge();
    closeAlertModal();
    setAlertRecords([]);
    canvasBridge()?.setTradingAlerts?.({ alerts: [] });
    syncChartModeTrigger();
    syncProgramTrigger();
    syncAlertTrigger();
    els.content?.classList.remove("trading-mode");
    closeHeaderMenu();
    state.uiCache.leftPanelKey = "";
    state.uiCache.timeframeKey = "";
    state.uiCache.headerMenuKey = "";
    state.uiCache.tradingHeaderKey = "";
    state.uiCache.indicatorDockKey = "";
    state.compareScrollTop = 0;
    if (els.timeframeRail) els.timeframeRail.hidden = true;
    canvasBridge()?.setExtraCharts?.([]);
    canvasBridge()?.setTradingIndicators?.({ indicators: [] });
    canvasBridge()?.setTradingHeader?.({ active: false });
    renderIndicatorDock();
    syncTradingWorkspaceButton();
    restoreUiSnapshot();
  }

  function focusTradingSurface() {
    try {
      window.__forgeSwitchSection?.("alpha");
    } catch (_) {}
    try {
      if (window.__forgeWebExplorerIsActive?.()) window.__forgeCloseWebExplorer?.();
    } catch (_) {}
    if (state.active) {
      installPanelBridge();
      els.content?.classList.add("trading-mode");
      els.canvasWrap?.classList.add("is-trading-mode");
      syncTradingIndicatorsToCanvas();
      syncChartModeTrigger();
      syncProgramTrigger();
      syncAlertTrigger();
      syncTradingHeader();
      renderTimeframeRail();
      renderIndicatorDock();
    }
    canvasBridge()?.forceImmediateRender?.();
    syncTradingWorkspaceButton();
  }

  function toggleTrading() {
    if (state.active) {
      const activeSection = String(window.__forgeActiveSection?.() || "");
      if (activeSection !== "alpha" || window.__forgeWebExplorerIsActive?.()) {
        focusTradingSurface();
        return;
      }
      deactivateTrading();
      return;
    }
    activateTrading();
  }

  function openTradingWorkspace() {
    if (window.__forgeRealEstateModeActive) return;
    try {
      window.alphaTrace?.("trading.workspace.open", { active: !!state.active });
    } catch (_) {}
    if (state.active) {
      focusTradingSurface();
      return;
    }
    activateTrading();
  }

  function handleTradingWorkspaceButtonClick(event) {
    event?.preventDefault?.();
    event?.stopPropagation?.();
    event?.stopImmediatePropagation?.();
    openTradingWorkspace();
  }

  function syncTradingWorkspaceButton() {
    if (!els.button) return;
    els.button.classList.toggle("is-active", !!state.active);
    els.button.setAttribute("aria-pressed", state.active ? "true" : "false");
    els.button.setAttribute("title", state.active ? "Close Trading workspace" : "Trading workspace");
    if (!state.active) requestAnimationFrame(() => els.button?.blur?.());
  }

  function onAssetClick(event) {
    if (!state.active) return;
    const row = eventTargetElement(event)?.closest?.("[data-trading-instrument]");
    if (!row) return;
    if (handleTradingTokenClick(event, `<instrument:${brokerInstrumentCode(row.dataset.tradingInstrument || DEFAULT_INSTRUMENT) || row.dataset.tradingInstrument || DEFAULT_INSTRUMENT}>`)) return;
    event.preventDefault();
    void selectInstrument(row.dataset.tradingInstrument || DEFAULT_INSTRUMENT);
  }

  els.pinnedList?.addEventListener("click", onAssetClick);
  els.jobList?.addEventListener("click", onAssetClick);
  els.pinnedList?.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") onAssetClick(event);
  });
  els.jobList?.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") onAssetClick(event);
  });

  els.button.addEventListener("pointerdown", handleTradingWorkspaceButtonClick);
  els.button.addEventListener("click", handleTradingWorkspaceButtonClick);
  document.addEventListener("pointerdown", (event) => {
    const hit = event.target?.closest?.("#tradingWorkspaceBtn");
    if (!hit) return;
    handleTradingWorkspaceButtonClick(event);
  }, true);
  document.addEventListener("click", (event) => {
    const hit = event.target?.closest?.("#tradingWorkspaceBtn");
    if (!hit) return;
    handleTradingWorkspaceButtonClick(event);
  }, true);
  window.addEventListener("forge:trading-toggle-request", handleTradingWorkspaceButtonClick);
  document.addEventListener("click", (event) => {
    if (!state.active) return;
    const tokenNode = event.target?.closest?.("[data-trading-token]");
    if (!tokenNode) return;
    handleTradingTokenClick(event, tokenNode.dataset.tradingToken || "");
  }, true);
  els.compareTrigger?.addEventListener("click", (event) => {
    if (handleTradingTokenClick(event, headerTokenForCompare())) return;
    event.preventDefault();
    event.stopPropagation();
    if (els.compareMenu?.hidden === false && state.headerMenuMode === "compare") closeHeaderMenu();
    else openHeaderMenu(els.compareTrigger, "compare");
  });
  els.addTrigger?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    if (els.compareMenu?.hidden === false && state.headerMenuMode === "add") closeHeaderMenu();
    else openHeaderMenu(els.addTrigger, "add");
  });
  els.compareMenu?.addEventListener("click", (event) => {
    const settingsClose = event.target?.closest?.("[data-subbar-action='close-indicator-settings']");
    if (settingsClose) {
      event.preventDefault();
      closeHeaderMenu();
      return;
    }
    const settingsTab = event.target?.closest?.("[data-indicator-settings-tab]");
    if (settingsTab && state.headerMenuMode === "indicator-settings") {
      event.preventDefault();
      state.indicatorSettingsTab = String(settingsTab.dataset.indicatorSettingsTab || "inputs").toLowerCase();
      renderHeaderMenu("indicator-settings");
      return;
    }
    const button = event.target?.closest?.("[data-menu-kind][data-menu-value]");
    if (!button) return;
    event.preventDefault();
    const kind = button.dataset.menuKind || "";
    const value = button.dataset.menuValue || "";
    if (kind === "broker") {
      void selectTradingBroker(value);
    } else if (kind === "display") {
      selectChartDisplayMode(value);
    } else if (kind === "asset") {
      void selectInstrument(value || DEFAULT_INSTRUMENT);
    } else if (kind === "add") {
      void selectAddInstrument(value || "");
    } else {
      void selectComparisonInstrument(value || "");
    }
  });
  els.compareMenu?.addEventListener("change", (event) => {
    if (state.headerMenuMode !== "indicator-settings") return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const field = String(target.dataset?.indicatorField || "");
    const visibilityField = String(target.dataset?.indicatorFieldVisible || "");
    const indicator = activeIndicatorInstance(state.indicatorSettingsId);
    if (!indicator) return;
    if (visibilityField) {
      upsertTradingIndicator(indicator.id, { visible: !!target.checked });
      renderHeaderMenu("indicator-settings");
      return;
    }
    if (!field) return;
    const definition = tradingIndicatorDefinition(indicator.id);
    const meta = definition?.settings?.[field];
    if (!meta) return;
    const nextValue = meta.type === "checkbox"
      ? !!target.checked
      : meta.type === "number"
        ? Number(target.value)
        : String(target.value || "");
    upsertTradingIndicator(indicator.id, { settings: { [field]: nextValue } });
    renderHeaderMenu("indicator-settings");
  });
  els.compareSearchInput?.addEventListener("input", (event) => {
    if (state.headerMenuMode !== "compare" && state.headerMenuMode !== "add") return;
    state.compareSearch = String(event.target.value || "");
    state.uiCache.headerMenuKey = "";
    renderHeaderMenu(state.headerMenuMode);
  });
  if (els.compareSearchInput) {
    els.compareSearchInput.dataset.tradingToken = "<compare:search>";
  }
  els.programTrigger?.addEventListener("click", (event) => {
    if (!state.active) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    event.stopPropagation();
    els.proofToggle?.click?.();
    window.requestAnimationFrame(syncProgramTrigger);
  }, true);
  els.tradingSubbar?.addEventListener("click", (event) => {
    if (!state.active) return;
    const closeButton = event.target?.closest?.("[data-subbar-action='close']");
    if (closeButton) {
      event.preventDefault();
      event.stopPropagation();
      setTradingChatSubbarMode("none");
      return;
    }
    const expandButton = event.target?.closest?.("[data-subbar-action='toggle-expand']");
    if (expandButton && state.chatSubbarMode === "indicators") {
      event.preventDefault();
      event.stopPropagation();
      state.chatSubbarExpanded = !state.chatSubbarExpanded;
      renderTradingChatSubbar();
      return;
    }
    const sectionButton = event.target?.closest?.("[data-subbar-section]");
    if (sectionButton && state.chatSubbarMode === "indicators") {
      event.preventDefault();
      event.stopPropagation();
      state.chatSubbarSection = normalizeTradingSubbarSection(sectionButton.dataset.subbarSection || "indicators");
      renderTradingChatSubbar();
      return;
    }
    const indicatorPick = event.target?.closest?.("[data-indicator-pick]");
    if (indicatorPick && state.chatSubbarMode === "indicators") {
      event.preventDefault();
      event.stopPropagation();
      const indicatorId = String(indicatorPick.dataset.indicatorPick || "").trim().toLowerCase();
      if (!indicatorId) return;
      injectIndicatorSlashCommand(indicatorId);
      return;
    }
    const slashPick = event.target?.closest?.("[data-slash-pick]");
    if (slashPick && state.chatSubbarMode === "indicators") {
      event.preventDefault();
      event.stopPropagation();
      const token = String(slashPick.dataset.slashPick || "").trim();
      if (!token) return;
      injectTradingChatToken(token);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  });
  els.indicatorDock?.addEventListener("click", (event) => {
    if (!state.active) return;
    const action = event.target?.closest?.("[data-indicator-action]");
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    const indicatorId = String(action.dataset.indicatorId || "").trim().toLowerCase();
    const actionName = String(action.dataset.indicatorAction || "");
    if (!indicatorId) return;
    if (actionName === "inject") {
      injectIndicatorSlashCommand(indicatorId);
      return;
    }
    if (actionName === "toggle") {
      toggleTradingIndicatorVisibility(indicatorId);
      return;
    }
    if (actionName === "remove") {
      removeTradingIndicator(indicatorId);
      return;
    }
    if (actionName === "settings") {
      state.indicatorSettingsId = indicatorId;
      state.indicatorSettingsTab = "inputs";
      openHeaderMenu(action, "indicator-settings");
    }
  });
  els.replayTrigger?.addEventListener("click", (event) => {
    if (!state.active) return;
    if (handleTradingTokenClick(event, `<replay:${state.chatSubbarMode === "replay" ? "off" : "on"}>`)) return;
    event.preventDefault();
    event.stopPropagation();
    toggleTradingChatSubbarMode("replay");
  });
  els.indicatorsTrigger?.addEventListener("click", (event) => {
    if (!state.active) return;
    if (handleTradingTokenClick(event, `<indicators:${state.chatSubbarMode === "indicators" ? "off" : "on"}>`)) return;
    event.preventDefault();
    event.stopPropagation();
    toggleTradingChatSubbarMode("indicators");
  });
  els.selectionTrigger?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    const bridge = window.__forgeAlphaCanvasBridge;
    const next = !(bridge?.isTradingSelectionMode?.());
    bridge?.setTradingSelectionMode?.(next);
    syncTradingChatActions();
  });
  els.alertTrigger?.addEventListener("click", (event) => {
    if (!state.active) return;
    if (handleTradingTokenClick(event, "<alert:panel>")) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    event.stopPropagation();
    toggleTradingChatSubbarMode("alert");
  }, true);
  els.proofToggle?.addEventListener("click", () => {
    if (!state.active) return;
    window.requestAnimationFrame(syncProgramTrigger);
  });
  window.addEventListener("forge:trading-selection-mode", () => {
    syncTradingChatActions();
  });
  window.addEventListener("click", (event) => {
    if (els.compareMenu?.hidden !== false) return;
    if (els.compareMenu.contains(event.target)) return;
    if (els.compareTrigger?.contains(event.target)) return;
    if (els.addTrigger?.contains(event.target)) return;
    if (els.indicatorDock?.contains(event.target)) return;
    if (els.compareSearchWrap?.contains(event.target)) return;
    if (els.chartModeTrigger?.contains(event.target)) return;
    if (state.headerMenuTriggerEl?.contains?.(event.target)) return;
    closeHeaderMenu();
  });
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeHeaderMenu();
    if (event.key === "Escape" && state.alertModalOpen) closeAlertModal();
    else if (event.key === "Escape" && state.chatSubbarMode !== "none") setTradingChatSubbarMode("none");
  });
  window.addEventListener("forge:trading-broker-change", () => {
    if (!state.active) return;
    renderHeaderMenu(state.headerMenuMode === "none" ? "compare" : state.headerMenuMode);
    renderLeftPanel();
    syncTradingHeader();
    renderTimeframeRail();
  });
  window.addEventListener("forge:trading-broker-trigger", () => {
    if (!state.active) return;
    const trigger = document.getElementById("workspaceCrumb");
    if (!trigger) return;
    if (els.compareMenu?.hidden === false && state.headerMenuMode === "broker") closeHeaderMenu();
    else openHeaderMenu(trigger, "broker");
  });
  window.addEventListener("forge:trading-asset-trigger", () => {
    if (!state.active) return;
    const trigger = document.getElementById("projectCrumb");
    if (!trigger) return;
    if (els.compareMenu?.hidden === false && state.headerMenuMode === "asset") closeHeaderMenu();
    else openHeaderMenu(trigger, "asset");
  });
  window.addEventListener("forge:trading-slash-preview", (event) => {
    if (!state.active) return;
    const token = String(event?.detail?.token || "").trim().toLowerCase();
    if (token === "/" || token === "/create_") {
      state.chatSubbarSection = "create";
      setTradingChatSubbarMode("indicators", { force: true });
    } else if (token === "/strategy_" || token === "/backtest_") {
      state.chatSubbarSection = "strategies";
      setTradingChatSubbarMode("indicators", { force: true });
    }
  });
  window.addEventListener("forge:trading-chart-mode-trigger", () => {
    if (!state.active || !els.chartModeTrigger) return;
    if (els.compareMenu?.hidden === false && state.headerMenuMode === "display") closeHeaderMenu();
    else openHeaderMenu(els.chartModeTrigger, "display");
  });
  els.timeframeRail?.addEventListener("click", (event) => {
    const button = event.target?.closest?.("[data-trading-granularity]");
    if (!button) return;
    if (handleTradingTokenClick(event, `<timeframe:${button.dataset.tradingGranularity || DEFAULT_GRANULARITY}>`)) return;
    event.preventDefault();
    void selectGranularity(button.dataset.tradingGranularity || DEFAULT_GRANULARITY);
  });

  document.getElementById("workspaceCrumb")?.addEventListener("click", (event) => {
    handleTradingTokenClick(event, headerTokenForBroker());
  }, true);
  document.getElementById("projectCrumb")?.addEventListener("click", (event) => {
    handleTradingTokenClick(event, headerTokenForInstrument());
  }, true);

  window.__forgeOpenTrading = openTradingWorkspace;
  window.__forgeFocusTrading = focusTradingSurface;
  window.__forgeToggleTrading = toggleTrading;
  window.__forgeCloseTrading = deactivateTrading;
  const pendingActions = window.ForgeSectionRegistry?.consumeQueuedActions?.("trading") || [];
  if (pendingActions.some((entry) => entry.action === "open" || entry.action === "toggle")) {
    window.requestAnimationFrame(() => openTradingWorkspace());
  }
  installPanelBridge();
  syncTradingWorkspaceButton();
  syncChartModeTrigger();
  syncProgramTrigger();
  syncAlertTrigger();
})();
