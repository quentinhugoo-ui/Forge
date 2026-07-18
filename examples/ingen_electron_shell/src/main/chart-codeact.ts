import { createHash } from "node:crypto";
import {
  BRAIN_CHART_COMMAND,
  BRAIN_CHART_RESULT_SCHEMA,
  type IpcError,
  type TradingCandle,
  type TradingChartResult,
  type TradingChartWindowSnapshot,
  type TradingTimeframe
} from "../shared/ipc-contract.js";

export const CHART_COMMAND = BRAIN_CHART_COMMAND;
export const CHART_RESULT_SCHEMA = BRAIN_CHART_RESULT_SCHEMA;
export const CHART_TEMPLATE_RESULT_SCHEMA = "forge.trading.chart.template_result.v1";

const CHART_TIMEFRAMES: TradingTimeframe[] = ["H1", "H4", "D1"];
const MAX_FIELD_CHARS = 180;
const MAX_CANDLE_COUNT = 500;
const DEFAULT_CANDLE_COUNT = 50;

export interface ChartCodeActRequest {
  schema: "forge.trading.chart.request.v1";
  command: typeof CHART_COMMAND;
  templateProofHash: string;
  instrument: string;
  timeframe: TradingTimeframe;
  candleCount: number;
  includeScreenshot: boolean;
  includeOhlc: boolean;
  chartWindowSnapshotHash: string;
  source: "explicit_codeact";
  proofHash: string;
}

export interface ChartTemplateResult {
  schema: typeof CHART_TEMPLATE_RESULT_SCHEMA;
  command: typeof CHART_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    timeframe: TradingTimeframe[];
    includeScreenshot: Array<"true" | "false">;
    includeOhlc: Array<"true" | "false">;
  };
  chartWindowSnapshot?: TradingChartWindowSnapshot;
  rules: string[];
  proofHash: string;
}

export type ChartCodeAct =
  | { kind: "template"; result: ChartTemplateResult }
  | { kind: "request"; request: ChartCodeActRequest };

export function chartTemplateProofHash(): string {
  return stableHash({
    command: CHART_COMMAND,
    schema: CHART_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "instrument",
      "timeframe",
      "candle_count",
      "include_screenshot",
      "include_ohlc",
      "chart_window_snapshot_hash"
    ],
    source: "renderer_trading_chart",
    maxCandleCount: MAX_CANDLE_COUNT,
    terminalResult: CHART_RESULT_SCHEMA
  });
}

export function chartTemplateResult(
  reason: ChartTemplateResult["reason"] = "empty_command",
  chartWindowSnapshot?: TradingChartWindowSnapshot
): ChartTemplateResult {
  const snapshot = chartWindowSnapshot;
  const timeframe = snapshot?.timeframe ?? "H1";
  const available = snapshot?.availableTimeframes?.length ? snapshot.availableTimeframes : CHART_TIMEFRAMES;
  const loadedCount = Math.max(0, Math.floor(Number(snapshot?.loadedCandleCount ?? 0)));
  const defaultCount = loadedCount > 0 ? Math.min(DEFAULT_CANDLE_COUNT, loadedCount, MAX_CANDLE_COUNT) : DEFAULT_CANDLE_COUNT;
  const result: ChartTemplateResult = {
    schema: CHART_TEMPLATE_RESULT_SCHEMA,
    command: CHART_COMMAND,
    status: "template",
    reason,
    template: [
      `${CHART_COMMAND}`,
      `template_proof_hash="sha256:${chartTemplateProofHash()}"`,
      `instrument="${escapeTemplateValue(snapshot?.instrument ?? "NATGAS_USD")}"`,
      `timeframe="${timeframe}"`,
      `candle_count="${defaultCount}"`,
      'include_screenshot="true"',
      'include_ohlc="true"',
      `chart_window_snapshot_hash="${escapeTemplateValue(snapshot?.proofHash ?? "")}"`
    ].join("\n"),
    allowedValues: {
      timeframe: available,
      includeScreenshot: ["true", "false"],
      includeOhlc: ["true", "false"]
    },
    chartWindowSnapshot: snapshot,
    rules: [
      "The renderer Trading chart is the source of truth; this CodeAct must not fetch OANDA directly.",
      "Use the currently displayed chart instrument unless the user explicitly asks to inspect another loaded chart.",
      "If the requested timeframe is not loaded by the chart, return CHART_RESULT accepted=false instead of fabricating candles.",
      `Keep candle_count between 1 and ${MAX_CANDLE_COUNT}; default to 50 for compact LLM analysis.`,
      "Return CHART_RESULT with either screenshot, OHLC candles, or an explicit refusal reason so the loop stream never blocks."
    ],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function readChartCodeAct(input: string, chartWindowSnapshot?: TradingChartWindowSnapshot): ChartCodeAct | undefined {
  const trimmed = chartCodeActText(input).trim();
  if (!trimmed || !readChartCommand(trimmed)) return undefined;
  const body = trimmed.slice(CHART_COMMAND.length).trim();
  if (!body) return { kind: "template", result: chartTemplateResult("empty_command", chartWindowSnapshot) };
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: chartTemplateResult("template_required", chartWindowSnapshot) };
  }
  const request = parseChartCodeAct(trimmed, chartWindowSnapshot);
  return request ? { kind: "request", request } : { kind: "template", result: chartTemplateResult("template_required", chartWindowSnapshot) };
}

export function parseChartCodeAct(input: string, chartWindowSnapshot?: TradingChartWindowSnapshot): ChartCodeActRequest | undefined {
  const trimmed = chartCodeActText(input).trim();
  if (!trimmed || !readChartCommand(trimmed)) return undefined;
  const fields = parseTemplateFields(trimmed.slice(CHART_COMMAND.length).trim());
  const allowedTimeframes = chartWindowSnapshot?.availableTimeframes?.length ? chartWindowSnapshot.availableTimeframes : CHART_TIMEFRAMES;
  const timeframe = readUpperChoice(fields.get("timeframe"), allowedTimeframes, chartWindowSnapshot?.timeframe ?? "H1");
  const requestedCount = Number(fields.get("candle_count") ?? fields.get("candleCount") ?? DEFAULT_CANDLE_COUNT);
  const candleCount = clampInt(requestedCount, 1, MAX_CANDLE_COUNT);
  const request: ChartCodeActRequest = {
    schema: "forge.trading.chart.request.v1",
    command: CHART_COMMAND,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    instrument: clampText(fields.get("instrument") ?? chartWindowSnapshot?.instrument ?? "NATGAS_USD", MAX_FIELD_CHARS),
    timeframe,
    candleCount,
    includeScreenshot: readBool(fields.get("include_screenshot") ?? fields.get("includeScreenshot"), true),
    includeOhlc: readBool(fields.get("include_ohlc") ?? fields.get("includeOhlc"), true),
    chartWindowSnapshotHash: normalizeProofHash(fields.get("chart_window_snapshot_hash") ?? fields.get("chartWindowSnapshotHash") ?? chartWindowSnapshot?.proofHash ?? ""),
    source: "explicit_codeact",
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function renderChartTemplateResult(result: ChartTemplateResult): string {
  return [
    "CHART_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${chartTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    `rules=${JSON.stringify(result.rules)}`,
    result.chartWindowSnapshot ? `chart_window_snapshot=${JSON.stringify(compactChartSnapshotForRender(result.chartWindowSnapshot))}` : "chart_window_snapshot=null",
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function renderChartCodeActResult(result: TradingChartResult): string {
  const screenshot = result.screenshotPngDataUrl ? {
    mime: "image/png",
    width: result.screenshotWidth,
    height: result.screenshotHeight,
    sha256: result.screenshotHash,
    dataUrl: result.screenshotPngDataUrl
  } : null;
  return [
    "CHART_RESULT",
    `schema=${result.schema}`,
    `source=${result.source}`,
    `status=${result.accepted ? "filled" : "refused"}`,
    `instrument=${JSON.stringify(result.instrument)}`,
    `display_name=${JSON.stringify(result.displayName)}`,
    `timeframe=${result.timeframe}`,
    `candle_count=${result.candleCount}`,
    `first_candle_time=${JSON.stringify(result.firstCandleTime ?? "")}`,
    `last_candle_time=${JSON.stringify(result.lastCandleTime ?? "")}`,
    `chart_window_snapshot=${result.chartWindowSnapshot ? JSON.stringify(compactChartSnapshotForRender(result.chartWindowSnapshot)) : "null"}`,
    `screenshot=${JSON.stringify(screenshot)}`,
    `candles=${JSON.stringify(compactCandlesForRender(result.candles))}`,
    result.error ? `error=${JSON.stringify(result.error)}` : "error=null",
    `captured_at=${JSON.stringify(result.capturedAt)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function chartRefusalResult(params: {
  request?: ChartCodeActRequest;
  snapshot?: TradingChartWindowSnapshot;
  message: string;
  code?: string;
}): TradingChartResult {
  const capturedAt = new Date().toISOString();
  const snapshot = params.snapshot;
  const result: TradingChartResult = {
    accepted: false,
    schema: CHART_RESULT_SCHEMA,
    source: "renderer_trading_chart",
    instrument: params.request?.instrument ?? snapshot?.instrument ?? "NATGAS_USD",
    displayName: snapshot?.displayName ?? "Natural Gas",
    timeframe: params.request?.timeframe ?? snapshot?.timeframe ?? "H1",
    candleCount: 0,
    candles: [],
    chartWindowSnapshot: snapshot,
    capturedAt,
    proofHash: "",
    error: {
      code: params.code ?? "chart_unavailable",
      message: params.message,
      proofHash: stableHash({ code: params.code ?? "chart_unavailable", message: params.message, capturedAt })
    } as IpcError
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function compactChartSnapshotForRender(snapshot: TradingChartWindowSnapshot): Record<string, unknown> {
  return {
    schema: snapshot.schema,
    source: snapshot.source,
    instrument: snapshot.instrument,
    displayName: snapshot.displayName,
    timeframe: snapshot.timeframe,
    availableTimeframes: snapshot.availableTimeframes,
    loadedCandleCount: snapshot.loadedCandleCount,
    visibleCandleCount: snapshot.visibleCandleCount,
    firstLoadedTime: snapshot.firstLoadedTime,
    lastLoadedTime: snapshot.lastLoadedTime,
    firstVisibleTime: snapshot.firstVisibleTime,
    lastVisibleTime: snapshot.lastVisibleTime,
    pricePrecision: snapshot.pricePrecision,
    dataSource: snapshot.dataSource,
    chartUpdatedAt: snapshot.chartUpdatedAt,
    proofHash: snapshot.proofHash
  };
}

function compactCandlesForRender(candles: TradingCandle[]): TradingCandle[] {
  return candles.map((candle) => ({
    time: candle.time,
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
    volume: candle.volume,
    complete: candle.complete
  }));
}

function readChartCommand(value: string): typeof CHART_COMMAND | undefined {
  const trimmed = value.trim();
  return trimmed === CHART_COMMAND || trimmed.startsWith(`${CHART_COMMAND} `) || trimmed.startsWith(`${CHART_COMMAND}\n`) ? CHART_COMMAND : undefined;
}

function chartCodeActText(input: string): string {
  const commandIndex = input.indexOf(CHART_COMMAND);
  if (commandIndex < 0) return "";
  const lines = input.slice(commandIndex).split(/\r?\n/);
  const block: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]?.trim() ?? "";
    if (index > 0 && (!line || line.startsWith("/") || /^[A-Z_]+_RESULT\b/.test(line))) break;
    block.push(line);
  }
  return block.join("\n");
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"((?:\\.|[^"])*)"|'((?:\\.|[^'])*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    fields.set(key, decodeTemplateValue(match[2] ?? match[3] ?? match[4] ?? "").trim());
  }
  return fields;
}

function decodeTemplateValue(value: string): string {
  return value.replace(/\\"/gu, "\"").replace(/\\'/gu, "'").replace(/\\n/gu, "\n").replace(/\\t/gu, "\t").replace(/\\\\/gu, "\\");
}

function escapeTemplateValue(value: string): string {
  return value.replace(/\\/gu, "\\\\").replace(/"/gu, "\\\"");
}

function readUpperChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  if (typeof value !== "string") return fallback;
  const normalized = value.trim().toUpperCase();
  return choices.find((choice) => choice === normalized) ?? fallback;
}

function readBool(value: unknown, fallback: boolean): boolean {
  if (typeof value !== "string") return fallback;
  const normalized = value.trim().toLowerCase();
  if (["true", "1", "yes", "oui"].includes(normalized)) return true;
  if (["false", "0", "no", "non"].includes(normalized)) return false;
  return fallback;
}

function clampInt(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, Math.floor(value)));
}

function clampText(text: string | undefined, maxChars: number): string {
  const clean = (text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === chartTemplateProofHash();
}

function normalizeProofHash(value: unknown): string {
  return String(value ?? "").trim().replace(/^sha256:/i, "");
}

function indentBlock(value: string, prefix: string): string {
  return value.split(/\r?\n/).map((line) => `${prefix}${line}`).join("\n");
}

function stableHash(value: unknown): string {
  return createHash("sha256").update(stableJson(value)).digest("hex");
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`).join(",")}}`;
}
