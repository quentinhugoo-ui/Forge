import { describe, expect, it } from "vitest";
import {
  chartRefusalResult,
  chartTemplateProofHash,
  parseChartCodeAct,
  readChartCodeAct,
  renderChartCodeActResult,
  renderChartTemplateResult
} from "../src/main/chart-codeact";
import type { TradingChartWindowSnapshot } from "../src/shared/ipc-contract";

const snapshot: TradingChartWindowSnapshot = {
  schema: "forge.trading.chart_window_snapshot.v1",
  source: "renderer_trading_chart",
  instrument: "NATGAS_USD",
  displayName: "Natural Gas",
  timeframe: "H1",
  availableTimeframes: ["H1"],
  loadedCandleCount: 240,
  visibleCandleCount: 150,
  firstLoadedTime: "2026-07-01T00:00:00.000Z",
  lastLoadedTime: "2026-07-04T12:00:00.000Z",
  firstVisibleTime: "2026-07-02T06:00:00.000Z",
  lastVisibleTime: "2026-07-04T12:00:00.000Z",
  pricePrecision: 3,
  dataSource: "oanda_rest_v20",
  chartUpdatedAt: "2026-07-04T12:00:00.500Z",
  proofHash: "chart-snapshot-proof"
};

describe("Chart CodeAct", () => {
  it("returns a renderer-chart template for bare /chart_", () => {
    const codeAct = readChartCodeAct("/chart_", snapshot);

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.template).toContain('instrument="NATGAS_USD"');
    expect(codeAct.result.template).toContain('timeframe="H1"');
    expect(codeAct.result.template).toContain('candle_count="50"');
    expect(codeAct.result.template).toContain('chart_window_snapshot_hash="chart-snapshot-proof"');
    expect(codeAct.result.rules.join("\n")).toContain("must not fetch OANDA directly");
    expect(renderChartTemplateResult(codeAct.result)).toContain("CHART_TEMPLATE_RESULT");
  });

  it("requires the template proof before accepting a filled chart request", () => {
    const codeAct = readChartCodeAct('/chart_ instrument="NATGAS_USD" timeframe="H1" candle_count="50"', snapshot);

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.reason).toBe("template_required");
  });

  it("parses a filled renderer chart request", () => {
    const hash = chartTemplateProofHash();
    const request = parseChartCodeAct(`/chart_ template_proof_hash="sha256:${hash}" instrument="NATGAS_USD" timeframe="H1" candle_count="50" include_screenshot="true" include_ohlc="true" chart_window_snapshot_hash="chart-snapshot-proof"`, snapshot);

    expect(request).toMatchObject({
      instrument: "NATGAS_USD",
      timeframe: "H1",
      candleCount: 50,
      includeScreenshot: true,
      includeOhlc: true,
      chartWindowSnapshotHash: "chart-snapshot-proof"
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("renders terminal refused results without blocking the loop", () => {
    const result = chartRefusalResult({ snapshot, message: "Trading chart renderer did not answer before timeout.", code: "chart_renderer_timeout" });
    const rendered = renderChartCodeActResult(result);

    expect(result.accepted).toBe(false);
    expect(rendered).toContain("CHART_RESULT");
    expect(rendered).toContain("status=refused");
    expect(rendered).toContain("chart_renderer_timeout");
  });
});
