import { createHash } from "node:crypto";
import {
  BRAIN_MARKET_ORDER_COMMAND,
  BRAIN_MARKET_ORDER_RESULT_SCHEMA,
  type TradingOrderResult,
  type TradingOrderSide,
  type TradingOrderWindowSnapshot
} from "../shared/ipc-contract.js";

export const MARKET_ORDER_COMMAND = BRAIN_MARKET_ORDER_COMMAND;
export const MARKET_ORDER_RESULT_SCHEMA = BRAIN_MARKET_ORDER_RESULT_SCHEMA;
export const MARKET_ORDER_TEMPLATE_RESULT_SCHEMA = "forge.trading.market_order.template_result.v1";

const MAX_FIELD_CHARS = 180;
const MAX_UNITS_CHARS = 40;
const MARKET_ORDER_SIDES: TradingOrderSide[] = ["buy", "sell"];
const MARKET_ORDER_TIME_IN_FORCE = ["FOK", "IOC"] as const;
const MARKET_ORDER_POSITION_FILL = ["DEFAULT", "OPEN_ONLY", "REDUCE_FIRST", "REDUCE_ONLY"] as const;
const MARKET_ORDER_EXECUTION_MODE = ["stage_only", "live_oanda_order"] as const;
const MARKET_ORDER_CONFIRMATION = ["none", "user_confirmed_live_order"] as const;

export type MarketOrderTimeInForce = (typeof MARKET_ORDER_TIME_IN_FORCE)[number];
export type MarketOrderPositionFill = (typeof MARKET_ORDER_POSITION_FILL)[number];
export type MarketOrderExecutionMode = (typeof MARKET_ORDER_EXECUTION_MODE)[number];
export type MarketOrderConfirmation = (typeof MARKET_ORDER_CONFIRMATION)[number];

export interface MarketOrderCodeActRequest {
  schema: "forge.trading.market_order.request.v1";
  command: typeof MARKET_ORDER_COMMAND;
  templateProofHash: string;
  instrument: string;
  side: TradingOrderSide;
  units: string;
  unitsAvailable: string;
  maxUnitsAfterSafetyBuffer: string;
  unitsSafetyBufferPercent: "1";
  timeInForce: MarketOrderTimeInForce;
  positionFill: MarketOrderPositionFill;
  priceBound: string;
  stopLossPrice: string;
  takeProfitPrice: string;
  trailingStopDistance: string;
  guaranteedStopLossPrice: string;
  clientExtensionsTag: string;
  orderWindowSnapshotHash: string;
  livePriceTime: string;
  accountIdMasked: string;
  executionMode: MarketOrderExecutionMode;
  userConfirmation: MarketOrderConfirmation;
  source: "explicit_codeact";
  proofHash: string;
}

export interface MarketOrderTemplateResult {
  schema: typeof MARKET_ORDER_TEMPLATE_RESULT_SCHEMA;
  command: typeof MARKET_ORDER_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    side: TradingOrderSide[];
    timeInForce: MarketOrderTimeInForce[];
    positionFill: MarketOrderPositionFill[];
    executionMode: MarketOrderExecutionMode[];
    userConfirmation: MarketOrderConfirmation[];
  };
  orderWindowSnapshot?: TradingOrderWindowSnapshot;
  riskRules: string[];
  proofHash: string;
}

export type MarketOrderCodeAct =
  | { kind: "template"; result: MarketOrderTemplateResult }
  | { kind: "request"; request: MarketOrderCodeActRequest };

export interface MarketOrderVisibilityResult {
  status: "staged" | "visible_open_trade" | "not_visible_yet" | "rejected";
  openTradeId?: string;
  checkedAt: string;
  proofHash: string;
}

export interface MarketOrderCodeActExecutionResult {
  schema: typeof MARKET_ORDER_RESULT_SCHEMA;
  command: typeof MARKET_ORDER_COMMAND;
  status: "staged" | "executed" | "rejected";
  request: MarketOrderCodeActRequest;
  orderWindowSnapshot?: TradingOrderWindowSnapshot;
  brokerResult?: TradingOrderResult;
  orderVisibility?: MarketOrderVisibilityResult;
  warnings: string[];
  proofHash: string;
}

export function marketOrderTemplateProofHash(): string {
  return stableHash({
    command: MARKET_ORDER_COMMAND,
    schema: MARKET_ORDER_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "instrument",
      "side",
      "units",
      "units_available",
      "max_units_after_1pct_buffer",
      "units_safety_buffer_percent",
      "time_in_force",
      "position_fill",
      "price_bound",
      "stop_loss_price",
      "take_profit_price",
      "trailing_stop_distance",
      "guaranteed_stop_loss_price",
      "client_extensions_tag",
      "order_window_snapshot_hash",
      "live_price_time",
      "account_id_masked",
      "execution_mode",
      "user_confirmation"
    ],
    allowedValues: marketOrderAllowedValues(),
    apiContract: {
      broker: "oanda_rest_v20",
      endpoint: "POST /v3/accounts/{accountID}/orders",
      orderType: "MARKET",
      marketTimeInForce: MARKET_ORDER_TIME_IN_FORCE
    },
    safety: [
      "bare_command_returns_template_and_snapshot",
      "filled_command_requires_template_proof_hash",
      "units_must_stay_below_99_percent_of_side_units_available_when_present",
      "live_execution_requires_user_confirmed_live_order",
      "tokens_never_rendered"
    ]
  });
}

export function marketOrderTemplateResult(
  reason: MarketOrderTemplateResult["reason"] = "empty_command",
  orderWindowSnapshot?: TradingOrderWindowSnapshot
): MarketOrderTemplateResult {
  const snapshot = orderWindowSnapshot;
  const side = snapshot?.side ?? "buy";
  const unitsAvailable = marketOrderUnitsAvailableForSide(snapshot, side, "DEFAULT");
  const safeUnits = marketOrderUnitsWithOnePercentBuffer(unitsAvailable);
  const templateUnits = safeUnits || snapshot?.units || "";
  const result: MarketOrderTemplateResult = {
    schema: MARKET_ORDER_TEMPLATE_RESULT_SCHEMA,
    command: MARKET_ORDER_COMMAND,
    status: "template",
    reason,
    template: [
      `${MARKET_ORDER_COMMAND}`,
      `template_proof_hash="sha256:${marketOrderTemplateProofHash()}"`,
      `instrument="${escapeTemplateValue(snapshot?.instrument ?? "NATGAS_USD")}"`,
      `side="${side}"`,
      `units="${escapeTemplateValue(templateUnits)}"`,
      `units_available="${escapeTemplateValue(unitsAvailable)}"`,
      `max_units_after_1pct_buffer="${escapeTemplateValue(safeUnits)}"`,
      'units_safety_buffer_percent="1"',
      'time_in_force="FOK|IOC"',
      'position_fill="DEFAULT|OPEN_ONLY|REDUCE_FIRST|REDUCE_ONLY"',
      'price_bound=""',
      `stop_loss_price="${escapeTemplateValue(snapshot?.stopLossPrice ?? "")}"`,
      `take_profit_price="${escapeTemplateValue(snapshot?.takeProfitPrice ?? "")}"`,
      `trailing_stop_distance="${escapeTemplateValue(snapshot?.trailingStopDistance ?? "")}"`,
      'guaranteed_stop_loss_price=""',
      'client_extensions_tag="forge-trading"',
      `order_window_snapshot_hash="${escapeTemplateValue(snapshot?.proofHash ?? "")}"`,
      `live_price_time="${escapeTemplateValue(snapshot?.livePrice?.time ?? "")}"`,
      `account_id_masked="${escapeTemplateValue(snapshot?.account?.accountIdMasked ?? "")}"`,
      'execution_mode="stage_only|live_oanda_order"',
      'user_confirmation="none|user_confirmed_live_order"'
    ].join("\n"),
    allowedValues: marketOrderAllowedValues(),
    orderWindowSnapshot: snapshot,
    riskRules: [
      "Do not infer units: use the user request, an explicit strategy contract, or the order window value.",
      "When OANDA unitsAvailable is present, fill units at or below max_units_after_1pct_buffer, which is floor(side-specific unitsAvailable * 0.99), never the full unitsAvailable value.",
      "For OANDA market orders, use FOK or IOC only.",
      "Live execution is blocked unless the user has explicitly confirmed the exact order.",
      "Compare order_window_snapshot_hash, live_price_time, bid/ask and margin before live execution."
    ],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function readMarketOrderCodeAct(input: string, orderWindowSnapshot?: TradingOrderWindowSnapshot): MarketOrderCodeAct | undefined {
  const trimmed = marketOrderCodeActText(input).trim();
  if (!trimmed || !readMarketOrderCommand(trimmed)) {
    return undefined;
  }
  const body = trimmed.slice(MARKET_ORDER_COMMAND.length).trim();
  if (!body) {
    return { kind: "template", result: marketOrderTemplateResult("empty_command", orderWindowSnapshot) };
  }
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: marketOrderTemplateResult("template_required", orderWindowSnapshot) };
  }
  const request = parseMarketOrderCodeAct(trimmed);
  return request ? { kind: "request", request } : { kind: "template", result: marketOrderTemplateResult("template_required", orderWindowSnapshot) };
}

export function parseMarketOrderCodeAct(input: string): MarketOrderCodeActRequest | undefined {
  const trimmed = marketOrderCodeActText(input).trim();
  if (!trimmed || !readMarketOrderCommand(trimmed)) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(MARKET_ORDER_COMMAND.length).trim());
  const request: MarketOrderCodeActRequest = {
    schema: "forge.trading.market_order.request.v1",
    command: MARKET_ORDER_COMMAND,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    instrument: clampText(fields.get("instrument") ?? "NATGAS_USD", MAX_FIELD_CHARS),
    side: readChoice(fields.get("side"), MARKET_ORDER_SIDES, "buy"),
    units: clampText(fields.get("units") ?? "", MAX_UNITS_CHARS),
    unitsAvailable: clampText(fields.get("units_available") ?? fields.get("unitsAvailable") ?? "", MAX_UNITS_CHARS),
    maxUnitsAfterSafetyBuffer: clampText(fields.get("max_units_after_1pct_buffer") ?? fields.get("maxUnitsAfterSafetyBuffer") ?? "", MAX_UNITS_CHARS),
    unitsSafetyBufferPercent: "1",
    timeInForce: readUpperChoice(fields.get("time_in_force") ?? fields.get("timeInForce"), MARKET_ORDER_TIME_IN_FORCE, "FOK"),
    positionFill: readUpperChoice(fields.get("position_fill") ?? fields.get("positionFill"), MARKET_ORDER_POSITION_FILL, "DEFAULT"),
    priceBound: clampText(fields.get("price_bound") ?? fields.get("priceBound") ?? "", MAX_FIELD_CHARS),
    stopLossPrice: clampText(fields.get("stop_loss_price") ?? fields.get("stopLossPrice") ?? "", MAX_FIELD_CHARS),
    takeProfitPrice: clampText(fields.get("take_profit_price") ?? fields.get("takeProfitPrice") ?? "", MAX_FIELD_CHARS),
    trailingStopDistance: clampText(fields.get("trailing_stop_distance") ?? fields.get("trailingStopDistance") ?? "", MAX_FIELD_CHARS),
    guaranteedStopLossPrice: clampText(fields.get("guaranteed_stop_loss_price") ?? fields.get("guaranteedStopLossPrice") ?? "", MAX_FIELD_CHARS),
    clientExtensionsTag: clampText(fields.get("client_extensions_tag") ?? fields.get("clientExtensionsTag") ?? "forge-trading", 64),
    orderWindowSnapshotHash: normalizeProofHash(fields.get("order_window_snapshot_hash") ?? fields.get("orderWindowSnapshotHash")),
    livePriceTime: clampText(fields.get("live_price_time") ?? fields.get("livePriceTime") ?? "", MAX_FIELD_CHARS),
    accountIdMasked: clampText(fields.get("account_id_masked") ?? fields.get("accountIdMasked") ?? "", MAX_FIELD_CHARS),
    executionMode: readChoice(fields.get("execution_mode") ?? fields.get("executionMode"), MARKET_ORDER_EXECUTION_MODE, "stage_only"),
    userConfirmation: readChoice(fields.get("user_confirmation") ?? fields.get("userConfirmation"), MARKET_ORDER_CONFIRMATION, "none"),
    source: "explicit_codeact",
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function renderMarketOrderTemplateResult(result: MarketOrderTemplateResult): string {
  return [
    "MARKET_ORDER_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${marketOrderTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    `risk_rules=${JSON.stringify(result.riskRules)}`,
    result.orderWindowSnapshot ? `order_window_snapshot=${JSON.stringify(compactSnapshotForRender(result.orderWindowSnapshot))}` : "order_window_snapshot=null",
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function renderMarketOrderCodeActResult(result: MarketOrderCodeActExecutionResult): string {
  return [
    "MARKET_ORDER_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `instrument=${JSON.stringify(result.request.instrument)}`,
    `side=${result.request.side}`,
    `units=${JSON.stringify(result.request.units)}`,
    `time_in_force=${result.request.timeInForce}`,
    `position_fill=${result.request.positionFill}`,
    `execution_mode=${result.request.executionMode}`,
    `user_confirmation=${result.request.userConfirmation}`,
    `order_window_snapshot_hash=sha256:${result.request.orderWindowSnapshotHash}`,
    `live_price_time=${JSON.stringify(result.request.livePriceTime)}`,
    result.orderWindowSnapshot ? `order_window_snapshot=${JSON.stringify(compactSnapshotForRender(result.orderWindowSnapshot))}` : "order_window_snapshot=null",
    result.brokerResult ? `broker_result=${JSON.stringify(compactBrokerResult(result.brokerResult))}` : "broker_result=null",
    result.orderVisibility ? `order_visibility=${JSON.stringify(result.orderVisibility)}` : "order_visibility=null",
    `warnings=${JSON.stringify(result.warnings)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function marketOrderCodeActExecutionResult(params: {
  request: MarketOrderCodeActRequest;
  orderWindowSnapshot?: TradingOrderWindowSnapshot;
  brokerResult?: TradingOrderResult;
  orderVisibility?: MarketOrderVisibilityResult;
}): MarketOrderCodeActExecutionResult {
  const warnings = marketOrderWarnings(params.request, params.orderWindowSnapshot, params.brokerResult);
  const status: MarketOrderCodeActExecutionResult["status"] = params.brokerResult
    ? params.brokerResult.accepted ? "executed" : "rejected"
    : "staged";
  const result: MarketOrderCodeActExecutionResult = {
    schema: MARKET_ORDER_RESULT_SCHEMA,
    command: MARKET_ORDER_COMMAND,
    status,
    request: params.request,
    orderWindowSnapshot: params.orderWindowSnapshot,
    brokerResult: params.brokerResult,
    orderVisibility: params.orderVisibility,
    warnings,
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function marketOrderRequiresLiveExecution(request: MarketOrderCodeActRequest): boolean {
  return request.executionMode === "live_oanda_order" && request.userConfirmation === "user_confirmed_live_order";
}

function marketOrderWarnings(
  request: MarketOrderCodeActRequest,
  snapshot?: TradingOrderWindowSnapshot,
  brokerResult?: TradingOrderResult
): string[] {
  const warnings: string[] = [];
  if (!request.units) warnings.push("units_empty");
  if (marketOrderUnitsExceedSafetyBuffer(request.units, request.maxUnitsAfterSafetyBuffer)) warnings.push("units_exceed_1pct_safety_buffer");
  if (snapshot?.proofHash && request.orderWindowSnapshotHash && snapshot.proofHash !== request.orderWindowSnapshotHash) warnings.push("order_window_snapshot_hash_mismatch");
  if (snapshot?.livePrice?.time && request.livePriceTime && snapshot.livePrice.time !== request.livePriceTime) warnings.push("live_price_time_mismatch");
  if (request.executionMode === "live_oanda_order" && request.userConfirmation !== "user_confirmed_live_order") warnings.push("live_order_confirmation_missing");
  if (request.priceBound && brokerResult) warnings.push("price_bound_not_supported_by_current_submit_bridge");
  if (request.guaranteedStopLossPrice && brokerResult) warnings.push("guaranteed_stop_loss_not_supported_by_current_submit_bridge");
  return warnings;
}

function compactBrokerResult(result: TradingOrderResult): Record<string, unknown> {
  return {
    accepted: result.accepted,
    source: result.source,
    instrument: result.instrument,
    side: result.side,
    type: result.type,
    units: result.units,
    accountIdMasked: result.accountIdMasked,
    fetchedAt: result.fetchedAt,
    relatedTransactionIDs: result.relatedTransactionIDs,
    lastTransactionID: result.lastTransactionID,
    orderCreateTransactionID: result.orderCreateTransactionID,
    orderFillTransactionID: result.orderFillTransactionID,
    orderCancelTransactionID: result.orderCancelTransactionID,
    orderRejectTransactionID: result.orderRejectTransactionID,
    rejectReason: result.rejectReason,
    error: result.error?.message,
    proofHash: result.proofHash
  };
}

function compactSnapshotForRender(snapshot: TradingOrderWindowSnapshot): Record<string, unknown> {
  return {
    schema: snapshot.schema,
    source: snapshot.source,
    instrument: snapshot.instrument,
    side: snapshot.side,
    type: snapshot.type,
    units: snapshot.units,
    price: snapshot.price,
    stopLossPrice: snapshot.stopLossPrice,
    takeProfitPrice: snapshot.takeProfitPrice,
    trailingStopDistance: snapshot.trailingStopDistance,
    livePrice: snapshot.livePrice,
    account: snapshot.account,
    windowUpdatedAt: snapshot.windowUpdatedAt,
    proofHash: snapshot.proofHash
  };
}

function marketOrderUnitsAvailableForSide(
  snapshot: TradingOrderWindowSnapshot | undefined,
  side: TradingOrderSide,
  positionFill: MarketOrderPositionFill
): string {
  const unitsAvailable = snapshot?.livePrice?.unitsAvailable;
  if (!unitsAvailable) return "";
  const bucketKey = positionFill === "OPEN_ONLY"
    ? "openOnly"
    : positionFill === "REDUCE_FIRST"
      ? "reduceFirst"
      : positionFill === "REDUCE_ONLY"
        ? "reduceOnly"
        : "default";
  const sideKey = side === "buy" ? "long" : "short";
  return unitsAvailable[bucketKey]?.[sideKey] ?? unitsAvailable.default?.[sideKey] ?? "";
}

function marketOrderUnitsWithOnePercentBuffer(unitsAvailable: string): string {
  const parsed = Number(unitsAvailable);
  if (!Number.isFinite(parsed) || parsed <= 0) return "";
  return String(Math.max(0, Math.floor(parsed * 0.99)));
}

function marketOrderUnitsExceedSafetyBuffer(units: string, maxUnitsAfterSafetyBuffer: string): boolean {
  const parsedUnits = Number(units);
  const parsedMax = Number(maxUnitsAfterSafetyBuffer);
  return Number.isFinite(parsedUnits) && Number.isFinite(parsedMax) && parsedMax > 0 && parsedUnits > parsedMax;
}
function marketOrderAllowedValues(): MarketOrderTemplateResult["allowedValues"] {
  return {
    side: MARKET_ORDER_SIDES,
    timeInForce: [...MARKET_ORDER_TIME_IN_FORCE],
    positionFill: [...MARKET_ORDER_POSITION_FILL],
    executionMode: [...MARKET_ORDER_EXECUTION_MODE],
    userConfirmation: [...MARKET_ORDER_CONFIRMATION]
  };
}

function readMarketOrderCommand(value: string): typeof MARKET_ORDER_COMMAND | undefined {
  const trimmed = value.trim();
  return trimmed === MARKET_ORDER_COMMAND || trimmed.startsWith(`${MARKET_ORDER_COMMAND} `) || trimmed.startsWith(`${MARKET_ORDER_COMMAND}\n`)
    ? MARKET_ORDER_COMMAND
    : undefined;
}

function marketOrderCodeActText(input: string): string {
  const commandIndex = input.indexOf(MARKET_ORDER_COMMAND);
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
  return value
    .replace(/\\"/gu, "\"")
    .replace(/\\'/gu, "'")
    .replace(/\\n/gu, "\n")
    .replace(/\\t/gu, "\t")
    .replace(/\\\\/gu, "\\");
}

function escapeTemplateValue(value: string): string {
  return value.replace(/\\/gu, "\\\\").replace(/"/gu, "\\\"");
}

function readChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  if (typeof value !== "string") return fallback;
  const normalized = value.trim().toLowerCase();
  return choices.find((choice) => choice.toLowerCase() === normalized) ?? fallback;
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
  return normalizeProofHash(value) === marketOrderTemplateProofHash();
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
