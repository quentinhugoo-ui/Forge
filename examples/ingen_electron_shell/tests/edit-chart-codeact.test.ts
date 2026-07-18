import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  editChartTemplateProofHash,
  parseEditChartCodeAct,
  readEditChartCodeAct,
  renderEditChartCodeActResult,
  renderEditChartTemplateResult
} from "../src/main/edit-chart-codeact";
import type { TradingChartEditResult, TradingChartWindowSnapshot } from "../src/shared/ipc-contract";

const root = process.cwd();
const preloadSource = readFileSync(join(root, "src", "preload", "preload.ts"), "utf8");
const preloadCjsSource = readFileSync(join(root, "preload.cjs"), "utf8");

const snapshot: TradingChartWindowSnapshot = {
  schema: "forge.trading.chart_window_snapshot.v1",
  source: "renderer_trading_chart",
  instrument: "NATGAS_USD",
  displayName: "Natural Gas",
  timeframe: "H1",
  availableTimeframes: ["H1"],
  loadedCandleCount: 120,
  visibleCandleCount: 80,
  firstLoadedTime: "2026-07-01T00:00:00.000Z",
  lastLoadedTime: "2026-07-03T16:00:00.000Z",
  firstVisibleTime: "2026-07-02T08:00:00.000Z",
  lastVisibleTime: "2026-07-03T16:00:00.000Z",
  pricePrecision: 3,
  dataSource: "oanda_rest_v20",
  chartUpdatedAt: "2026-07-04T12:00:00.000Z",
  proofHash: "chart-snapshot-proof"
};

describe("edit chart CodeAct", () => {
  it("returns a template for bare /edit_chart_", () => {
    const codeAct = readEditChartCodeAct("/edit_chart_", snapshot);
    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    const rendered = renderEditChartTemplateResult(codeAct.result);
    expect(rendered).toContain("EDIT_CHART_TEMPLATE_RESULT");
    expect(rendered).toContain("vwap");
    expect(rendered).toContain("Use this incrementally");
    expect(rendered).toContain("do not batch all final annotations");
    expect(rendered).toContain("chart_window_snapshot_hash");
  });

  it("parses filled typed chart edits", () => {
    const hash = editChartTemplateProofHash();
    const request = parseEditChartCodeAct(`/edit_chart_ template_proof_hash="sha256:${hash}" instrument="NATGAS_USD" timeframe="H1" actions='[{"kind":"horizontal_line","label":"resistance 3.236","price":3.236},{"kind":"donchian_channel","label":"Donchian 20","period":20}]' chart_window_snapshot_hash="chart-snapshot-proof"`, snapshot);
    expect(request?.actions).toHaveLength(2);
    expect(request?.actions[0]?.kind).toBe("horizontal_line");
    expect(request?.actions[1]?.period).toBe(20);
    expect(request?.proofHash).toHaveLength(64);
  });

  it("normalizes LLM candle highlight aliases to select_candles", () => {
    const hash = editChartTemplateProofHash();
    const request = parseEditChartCodeAct(`/edit_chart_ template_proof_hash="sha256:${hash}" instrument="NATGAS_USD" timeframe="H1" actions='[{"kind":"highlight_candles","label":"3 bullish candles","candle_times":["2026-07-16T14:00:00.000000000Z","16/07 15h"],"time_start":"2026-07-16T14:00:00.000000000Z","time_end":"2026-07-16T16:00:00.000000000Z"}]' chart_window_snapshot_hash="chart-snapshot-proof"`, snapshot);
    expect(request?.actions).toHaveLength(1);
    expect(request?.actions[0]?.kind).toBe("select_candles");
    expect(request?.actions[0]?.candleTimes).toEqual(["2026-07-16T14:00:00.000000000Z", "16/07 15h"]);
    expect(request?.actions[0]?.time).toBe("2026-07-16T14:00:00.000000000Z");
    expect(request?.actions[0]?.timeEnd).toBe("2026-07-16T16:00:00.000000000Z");
  });

  it("exposes the renderer IPC bridge used to apply chart edits", () => {
    expect(preloadSource).toContain("onTradingChartEditRequestEvent(listener)");
    expect(preloadSource).toContain("forge:trading-chart-edit-request");
    expect(preloadSource).toContain("completeTradingChartEditRequest(result)");
    expect(preloadSource).toContain("forge:complete-trading-chart-edit-request");
    expect(preloadCjsSource).toContain("onTradingChartEditRequestEvent(listener)");
    expect(preloadCjsSource).toContain("forge:trading-chart-edit-request");
    expect(preloadCjsSource).toContain("completeTradingChartEditRequest(result)");
    expect(preloadCjsSource).toContain("forge:complete-trading-chart-edit-request");
  });

  it("renders returned elements and clickable tags", () => {
    const result: TradingChartEditResult = {
      accepted: true,
      schema: "forge.trading.edit_chart.result.v1",
      source: "renderer_trading_chart",
      instrument: "NATGAS_USD",
      timeframe: "H1",
      appliedCount: 1,
      refusedCount: 0,
      elements: [{ kind: "horizontal_line", id: "resistance-1", label: "resistance 3.236", tag: "*resistance 3.236*", price: 3.236, instrument: "NATGAS_USD", timeframe: "H1", createdAt: "2026-07-04T12:00:00.000Z", proofHash: "element-proof" }],
      conversationTags: [{ tag: "*resistance 3.236*", elementId: "resistance-1", label: "resistance 3.236" }],
      chartWindowSnapshot: snapshot,
      capturedAt: "2026-07-04T12:00:00.000Z",
      proofHash: "result-proof"
    };
    const rendered = renderEditChartCodeActResult(result);
    expect(rendered).toContain("EDIT_CHART_RESULT");
    expect(rendered).toContain("status=applied");
    expect(rendered).toContain("conversation_tags");
    expect(rendered).toContain("*resistance 3.236*");
  });
});
