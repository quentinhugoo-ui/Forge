import { describe, expect, it } from "vitest";
import {
  marketOrderCodeActExecutionResult,
  marketOrderTemplateProofHash,
  parseMarketOrderCodeAct,
  readMarketOrderCodeAct,
  renderMarketOrderCodeActResult,
  renderMarketOrderTemplateResult
} from "../src/main/market-order-codeact";
import type { TradingOrderWindowSnapshot } from "../src/shared/ipc-contract";

const snapshot: TradingOrderWindowSnapshot = {
  schema: "forge.trading.order_window_snapshot.v1",
  source: "renderer_order_window_oanda",
  instrument: "NATGAS_USD",
  side: "sell",
  type: "MARKET",
  units: "12",
  stopLossPrice: "3.180",
  takeProfitPrice: "2.940",
  confirmLiveOrder: false,
  livePrice: {
    bid: 3.04,
    ask: 3.05,
    mid: 3.045,
    tradeable: true,
    time: "2026-07-04T12:00:00.000000000Z",
    fetchedAt: "2026-07-04T12:00:00.100Z",
    unitsAvailable: { default: { long: "100", short: "90" } }
  },
  account: {
    accountIdMasked: "001-***-789",
    currency: "USD",
    balance: "10000.00",
    nav: "10010.00",
    marginAvailable: "9000.00",
    marginUsed: "1000.00",
    pendingOrderCount: 0,
    openTradeCount: 1,
    openPositionCount: 1
  },
  windowUpdatedAt: "2026-07-04T12:00:00.120Z",
  proofHash: "snapshot-proof"
};

describe("Market order CodeAct", () => {
  it("returns a precise template with the live order-window snapshot for bare /market_order_", () => {
    const codeAct = readMarketOrderCodeAct("/market_order_", snapshot);

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.reason).toBe("empty_command");
    expect(codeAct.result.template).toContain('instrument="NATGAS_USD"');
    expect(codeAct.result.template).toContain('side="sell"');
    expect(codeAct.result.template).toContain('units="89"');
    expect(codeAct.result.template).toContain('units_available="90"');
    expect(codeAct.result.template).toContain('max_units_after_1pct_buffer="89"');
    expect(codeAct.result.template).toContain('units_safety_buffer_percent="1"');
    expect(codeAct.result.riskRules.join("\n")).toContain("floor(side-specific unitsAvailable * 0.99)");
    expect(codeAct.result.template).toContain('time_in_force="FOK|IOC"');
    expect(codeAct.result.orderWindowSnapshot?.proofHash).toBe("snapshot-proof");
    expect(renderMarketOrderTemplateResult(codeAct.result)).toContain("MARKET_ORDER_TEMPLATE_RESULT");
  });

  it("requires the template proof before accepting a filled market order", () => {
    const codeAct = readMarketOrderCodeAct('/market_order_ instrument="NATGAS_USD" side="sell" units="12"', snapshot);

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.reason).toBe("template_required");
  });

  it("parses a filled staged request with OANDA market-order constraints", () => {
    const hash = marketOrderTemplateProofHash();
    const request = parseMarketOrderCodeAct([
      `/market_order_ template_proof_hash="sha256:${hash}"`,
      'instrument="NATGAS_USD"',
      'side="sell"',
      'units="12"',
      'units_available="90"',
      'max_units_after_1pct_buffer="89"',
      'units_safety_buffer_percent="1"',
      'time_in_force="IOC"',
      'position_fill="REDUCE_FIRST"',
      'stop_loss_price="3.180"',
      'take_profit_price="2.940"',
      'order_window_snapshot_hash="snapshot-proof"',
      'live_price_time="2026-07-04T12:00:00.000000000Z"',
      'execution_mode="stage_only"',
      'user_confirmation="none"'
    ].join("\n"));

    expect(request).toMatchObject({
      instrument: "NATGAS_USD",
      side: "sell",
      units: "12",
      unitsAvailable: "90",
      maxUnitsAfterSafetyBuffer: "89",
      unitsSafetyBufferPercent: "1",
      timeInForce: "IOC",
      positionFill: "REDUCE_FIRST",
      executionMode: "stage_only",
      userConfirmation: "none"
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("warns when filled units exceed the 1 percent unitsAvailable buffer", () => {
    const hash = marketOrderTemplateProofHash();
    const request = parseMarketOrderCodeAct(`/market_order_ template_proof_hash="sha256:${hash}" instrument="NATGAS_USD" side="sell" units="90" units_available="90" max_units_after_1pct_buffer="89" order_window_snapshot_hash="snapshot-proof" live_price_time="2026-07-04T12:00:00.000000000Z" user_confirmation="none"`)!;
    const result = marketOrderCodeActExecutionResult({ request, orderWindowSnapshot: snapshot });

    expect(result.warnings).toContain("units_exceed_1pct_safety_buffer");
  });
  it("renders a staged result without pretending broker execution happened", () => {
    const hash = marketOrderTemplateProofHash();
    const request = parseMarketOrderCodeAct(`/market_order_ template_proof_hash="sha256:${hash}" instrument="NATGAS_USD" side="buy" units="5" order_window_snapshot_hash="snapshot-proof" live_price_time="2026-07-04T12:00:00.000000000Z" user_confirmation="none"`)!;
    const result = marketOrderCodeActExecutionResult({ request, orderWindowSnapshot: snapshot });
    const rendered = renderMarketOrderCodeActResult(result);

    expect(result.status).toBe("staged");
    expect(rendered).toContain("MARKET_ORDER_RESULT");
    expect(rendered).toContain("broker_result=null");
    expect(rendered).not.toContain("access_token");
  });
});
