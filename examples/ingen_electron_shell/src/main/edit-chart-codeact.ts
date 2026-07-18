import { createHash } from "node:crypto";
import {
  BRAIN_EDIT_CHART_COMMAND,
  BRAIN_EDIT_CHART_RESULT_SCHEMA,
  type IpcError,
  type TradingChartEditAction,
  type TradingChartEditResult,
  type TradingChartWindowSnapshot,
  type TradingTimeframe
} from "../shared/ipc-contract.js";

export const EDIT_CHART_COMMAND = BRAIN_EDIT_CHART_COMMAND;
export const EDIT_CHART_RESULT_SCHEMA = BRAIN_EDIT_CHART_RESULT_SCHEMA;
export const EDIT_CHART_TEMPLATE_RESULT_SCHEMA = "forge.trading.edit_chart.template_result.v1";

const EDIT_CHART_TIMEFRAMES: TradingTimeframe[] = ["H1", "H4", "D1"];
const EDIT_CHART_ACTION_KINDS = ["select_candles", "ray", "horizontal_line", "vertical_line", "moving_average", "vwap", "donchian_channel", "clear"] as const;
const MAX_ACTIONS = 24;
const MAX_LABEL_CHARS = 64;

function normalizeEditChartActionKind(value: unknown): TradingChartEditAction["kind"] | null {
  const compact = String(value ?? "").replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
  if (compact === "selectcandles" || compact === "selectcandle" || compact === "highlightcandles" || compact === "highlightcandle" || compact === "markcandles" || compact === "markcandle" || compact === "bluecandles" || compact === "bluecandle" || compact === "candles" || compact === "candle") return "select_candles";
  if (compact === "horizontalline" || compact === "level" || compact === "pricelevel") return "horizontal_line";
  if (compact === "verticalline" || compact === "timemarker") return "vertical_line";
  if (compact === "movingaverage" || compact === "ma" || compact === "sma") return "moving_average";
  if (compact === "donchianchannel" || compact === "donchian") return "donchian_channel";
  if (compact === "vwap") return "vwap";
  if (compact === "ray" || compact === "trendline") return "ray";
  if (compact === "clear" || compact === "remove" || compact === "reset") return "clear";
  return null;
}

export interface EditChartCodeActRequest {
  schema: "forge.trading.edit_chart.request.v1";
  command: typeof EDIT_CHART_COMMAND;
  templateProofHash: string;
  instrument: string;
  timeframe: TradingTimeframe;
  actions: TradingChartEditAction[];
  chartWindowSnapshotHash: string;
  source: "explicit_codeact";
  proofHash: string;
}

export interface EditChartTemplateResult {
  schema: typeof EDIT_CHART_TEMPLATE_RESULT_SCHEMA;
  command: typeof EDIT_CHART_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    timeframe: TradingTimeframe[];
    actionKind: string[];
  };
  chartWindowSnapshot?: TradingChartWindowSnapshot;
  rules: string[];
  proofHash: string;
}

export type EditChartCodeAct =
  | { kind: "template"; result: EditChartTemplateResult }
  | { kind: "request"; request: EditChartCodeActRequest };

export function editChartTemplateProofHash(): string {
  return stableHash({
    command: EDIT_CHART_COMMAND,
    schema: EDIT_CHART_TEMPLATE_RESULT_SCHEMA,
    fields: ["template_proof_hash", "instrument", "timeframe", "actions", "chart_window_snapshot_hash"],
    actionKinds: EDIT_CHART_ACTION_KINDS,
    terminalResult: EDIT_CHART_RESULT_SCHEMA
  });
}

export function editChartTemplateResult(
  reason: EditChartTemplateResult["reason"] = "empty_command",
  chartWindowSnapshot?: TradingChartWindowSnapshot
): EditChartTemplateResult {
  const snapshot = chartWindowSnapshot;
  const timeframe = snapshot?.timeframe ?? "H1";
  const available = snapshot?.availableTimeframes?.length ? snapshot.availableTimeframes : EDIT_CHART_TIMEFRAMES;
  const exampleActions: TradingChartEditAction[] = [
    { kind: "vwap", label: "VWAP active session", color: "cyan" }
  ];
  const result: EditChartTemplateResult = {
    schema: EDIT_CHART_TEMPLATE_RESULT_SCHEMA,
    command: EDIT_CHART_COMMAND,
    status: "template",
    reason,
    template: [
      `${EDIT_CHART_COMMAND}`,
      `template_proof_hash="sha256:${editChartTemplateProofHash()}"`,
      `instrument="${escapeTemplateValue(snapshot?.instrument ?? "NATGAS_USD")}"`,
      `timeframe="${timeframe}"`,
      `actions='${JSON.stringify(exampleActions)}'`,
      `chart_window_snapshot_hash="${escapeTemplateValue(snapshot?.proofHash ?? "")}"`
    ].join("\n"),
    allowedValues: {
      timeframe: available,
      actionKind: [...EDIT_CHART_ACTION_KINDS]
    },
    chartWindowSnapshot: snapshot,
    rules: [
      "The renderer Trading chart is the source of truth; this CodeAct edits the active chart view, not OANDA.",
      "Use this incrementally: emit /edit_chart_ immediately after the visible sentence that explains why a visual element is needed, before continuing the analysis.",
      "Prefer one conceptual edit per call; do not batch all final annotations unless the elements are inseparable in the same reasoning step.",
      "If checking a level break, select the candle and mark the level in the same immediate call only when both are needed for that single step.",
      "Use typed actions only: select_candles, ray, horizontal_line, vertical_line, moving_average, vwap, donchian_channel, clear.",
      "Every visible element should have a short label; EDIT_CHART_RESULT returns clickable conversation tags for those labels.",
      "Use *label* or ^label^ in later prose only for labels returned by EDIT_CHART_RESULT.",
      `Keep actions between 1 and ${MAX_ACTIONS}; return a refusal instead of blocking if the renderer cannot apply them.`
    ],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function readEditChartCodeAct(input: string, chartWindowSnapshot?: TradingChartWindowSnapshot): EditChartCodeAct | undefined {
  const trimmed = editChartCodeActText(input).trim();
  if (!trimmed || !readEditChartCommand(trimmed)) return undefined;
  const body = trimmed.slice(EDIT_CHART_COMMAND.length).trim();
  if (!body) return { kind: "template", result: editChartTemplateResult("empty_command", chartWindowSnapshot) };
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: editChartTemplateResult("template_required", chartWindowSnapshot) };
  }
  const request = parseEditChartCodeAct(trimmed, chartWindowSnapshot);
  return request ? { kind: "request", request } : { kind: "template", result: editChartTemplateResult("template_required", chartWindowSnapshot) };
}

export function parseEditChartCodeAct(input: string, chartWindowSnapshot?: TradingChartWindowSnapshot): EditChartCodeActRequest | undefined {
  const trimmed = editChartCodeActText(input).trim();
  if (!trimmed || !readEditChartCommand(trimmed)) return undefined;
  const fields = parseTemplateFields(trimmed.slice(EDIT_CHART_COMMAND.length).trim());
  const allowedTimeframes = chartWindowSnapshot?.availableTimeframes?.length ? chartWindowSnapshot.availableTimeframes : EDIT_CHART_TIMEFRAMES;
  const timeframe = readUpperChoice(fields.get("timeframe"), allowedTimeframes, chartWindowSnapshot?.timeframe ?? "H1");
  const actions = parseActions(fields.get("actions") ?? fields.get("edits") ?? "[]");
  const request: EditChartCodeActRequest = {
    schema: "forge.trading.edit_chart.request.v1",
    command: EDIT_CHART_COMMAND,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    instrument: clampText(fields.get("instrument") ?? chartWindowSnapshot?.instrument ?? "NATGAS_USD", 180),
    timeframe,
    actions,
    chartWindowSnapshotHash: normalizeProofHash(fields.get("chart_window_snapshot_hash") ?? fields.get("chartWindowSnapshotHash") ?? chartWindowSnapshot?.proofHash ?? ""),
    source: "explicit_codeact",
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function renderEditChartTemplateResult(result: EditChartTemplateResult): string {
  return [
    "EDIT_CHART_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${editChartTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    `rules=${JSON.stringify(result.rules)}`,
    result.chartWindowSnapshot ? `chart_window_snapshot=${JSON.stringify(compactChartSnapshotForRender(result.chartWindowSnapshot))}` : "chart_window_snapshot=null",
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function renderEditChartCodeActResult(result: TradingChartEditResult): string {
  return [
    "EDIT_CHART_RESULT",
    `schema=${result.schema}`,
    `source=${result.source}`,
    `status=${result.accepted ? "applied" : "refused"}`,
    `instrument=${JSON.stringify(result.instrument)}`,
    `timeframe=${result.timeframe}`,
    `applied_count=${result.appliedCount}`,
    `refused_count=${result.refusedCount}`,
    `elements=${JSON.stringify(result.elements)}`,
    `conversation_tags=${JSON.stringify(result.conversationTags)}`,
    `chart_window_snapshot=${result.chartWindowSnapshot ? JSON.stringify(compactChartSnapshotForRender(result.chartWindowSnapshot)) : "null"}`,
    result.error ? `error=${JSON.stringify(result.error)}` : "error=null",
    `captured_at=${JSON.stringify(result.capturedAt)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function editChartRefusalResult(params: {
  request?: EditChartCodeActRequest;
  snapshot?: TradingChartWindowSnapshot;
  message: string;
  code?: string;
}): TradingChartEditResult {
  const capturedAt = new Date().toISOString();
  const snapshot = params.snapshot;
  const result: TradingChartEditResult = {
    accepted: false,
    schema: EDIT_CHART_RESULT_SCHEMA,
    source: "renderer_trading_chart",
    instrument: params.request?.instrument ?? snapshot?.instrument ?? "NATGAS_USD",
    timeframe: params.request?.timeframe ?? snapshot?.timeframe ?? "H1",
    appliedCount: 0,
    refusedCount: params.request?.actions.length ?? 0,
    elements: [],
    conversationTags: [],
    chartWindowSnapshot: snapshot,
    capturedAt,
    proofHash: "",
    error: {
      code: params.code ?? "edit_chart_unavailable",
      message: params.message,
      proofHash: stableHash({ code: params.code ?? "edit_chart_unavailable", message: params.message, capturedAt })
    } as IpcError
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function parseActions(value: string): TradingChartEditAction[] {
  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.slice(0, MAX_ACTIONS).map(normalizeAction).filter((action): action is TradingChartEditAction => Boolean(action));
  } catch {
    return [];
  }
}

function normalizeAction(value: unknown): TradingChartEditAction | null {
  const record = value && typeof value === "object" ? value as Partial<TradingChartEditAction> & Record<string, unknown> : undefined;
  const kind = normalizeEditChartActionKind(record?.kind);
  if (!record || !kind) return null;
  const rawCandleTimes = Array.isArray(record.candleTimes)
    ? record.candleTimes
    : Array.isArray(record.candle_times)
      ? record.candle_times
      : Array.isArray(record.times)
        ? record.times
        : undefined;
  const action: TradingChartEditAction = {
    kind,
    id: clampText(record.id, MAX_LABEL_CHARS),
    label: clampText(record.label, MAX_LABEL_CHARS),
    tag: clampText(record.tag, MAX_LABEL_CHARS + 2),
    color: clampText(record.color, 32),
    candleTimes: Array.isArray(rawCandleTimes) ? rawCandleTimes.filter((time): time is string => typeof time === "string").slice(0, 80) : undefined,
    time: clampText(stringField(record.time ?? record.timeStart ?? record.time_start ?? record.start_time ?? record.start), 80),
    timeEnd: clampText(stringField(record.timeEnd ?? record.time_end ?? record.timeStop ?? record.time_stop ?? record.end_time ?? record.end), 80),
    price: finiteNumber(record.price),
    priceEnd: finiteNumber(record.priceEnd),
    period: Number.isFinite(Number(record.period)) ? Math.max(1, Math.min(500, Math.floor(Number(record.period)))) : undefined,
    source: record.source === "user" ? "user" : "llm"
  };
  return action;
}

function finiteNumber(value: unknown): number | undefined {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function stringField(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
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
    firstVisibleTime: snapshot.firstVisibleTime,
    lastVisibleTime: snapshot.lastVisibleTime,
    pricePrecision: snapshot.pricePrecision,
    chartUpdatedAt: snapshot.chartUpdatedAt,
    proofHash: snapshot.proofHash
  };
}

function readEditChartCommand(value: string): typeof EDIT_CHART_COMMAND | undefined {
  const trimmed = value.trim();
  return trimmed === EDIT_CHART_COMMAND || trimmed.startsWith(`${EDIT_CHART_COMMAND} `) || trimmed.startsWith(`${EDIT_CHART_COMMAND}\n`) ? EDIT_CHART_COMMAND : undefined;
}

function editChartCodeActText(input: string): string {
  const commandIndex = input.indexOf(EDIT_CHART_COMMAND);
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

function clampText(text: string | undefined, maxChars: number): string {
  const clean = (text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === editChartTemplateProofHash();
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
