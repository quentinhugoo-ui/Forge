import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { TradingAccountStateResult, TradingCandle, TradingChartEditAction, TradingChartEditElement, TradingChartEditKind, TradingChartEditRequestEvent, TradingChartEditResult, TradingChartRequestEvent, TradingChartResult, TradingChartWindowSnapshot, TradingTickResult, TradingTimeframe } from "../shared/ipc-contract";

interface MarketDataSource {
  id: string;
  label: string;
  cacheNamespace: string;
}

interface MarketAssetDefinition {
  instrument: string;
  displayName: string;
  pricePrecision: number;
  source: MarketDataSource;
}

const MARKET_SOURCE: MarketDataSource = {
  id: "oanda",
  label: "OANDA",
  cacheNamespace: "oanda_rest_v20"
};

const MARKET_ASSET: MarketAssetDefinition = {
  instrument: "NATGAS_USD",
  displayName: "Natural Gas",
  pricePrecision: 3,
  source: MARKET_SOURCE
};

const INSTRUMENT = MARKET_ASSET.instrument;
const TIMEFRAMES: TradingTimeframe[] = ["H1"];
const BACKFILL_FROM = "2006-01-01T00:00:00.000000000Z";
const BACKFILL_CHUNK_SIZE = 5000;
const LATEST_BOOTSTRAP_COUNT = 240;
const DB_NAME = "forge-trading-market-cache";
const DB_VERSION = 6;
const STORE_NAME = "candles";
const PATCH_STORE_NAME = "candlePatches";
const CANDLE_LIMIT_FOR_PAINT = 150;
const MIN_DURABLE_CHART_CANDLES = 120;
const MAX_BACKFILL_PAGES = 2000;
const OANDA_TICK_POLL_MS = 260;

const TIMEFRAME_MS: Record<TradingTimeframe, number> = {
  H1: 60 * 60 * 1000,
  H4: 4 * 60 * 60 * 1000,
  D1: 24 * 60 * 60 * 1000
};

interface CandleCacheRecord {
  key: string;
  schema: "forge.trading.market_cache.v1";
  instrument: string;
  timeframe: TradingTimeframe;
  candles: TradingCandle[];
  importedAt: string;
  updatedAt: string;
  source: string;
}

interface CandlePatchRecord {
  id: string;
  baseKey: string;
  schema: "forge.trading.market_patch.v1";
  instrument: string;
  timeframe: TradingTimeframe;
  candle: TradingCandle;
  importedAt: string;
  updatedAt: string;
  source: string;
}

type CandleMap = Partial<Record<TradingTimeframe, TradingCandle[]>>;
type TradingOpenTrade = TradingAccountStateResult["openTrades"][number];
type ImportState = Partial<Record<TradingTimeframe, string>>;
type ChartCursor = { x: number; y: number };
type ChartContextMenu = { x: number; y: number } | null;

function marketCacheKey(asset: MarketAssetDefinition, timeframe: TradingTimeframe): string {
  return `${asset.source.id}:${asset.instrument}:${timeframe}:oanda_h1_2006_tick_v2`;
}

function cacheKey(timeframe: TradingTimeframe): string {
  return marketCacheKey(MARKET_ASSET, timeframe);
}

function openTradingDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "key" });
      }
      if (!db.objectStoreNames.contains(PATCH_STORE_NAME)) {
        db.createObjectStore(PATCH_STORE_NAME, { keyPath: "id" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Trading cache open failed"));
  });
}

async function readCachedCandles(timeframe: TradingTimeframe): Promise<CandleCacheRecord | null> {
  const db = await openTradingDb();
  try {
    const baseKey = cacheKey(timeframe);
    const cached = await new Promise<CandleCacheRecord | null>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const request = tx.objectStore(STORE_NAME).get(baseKey);
      request.onsuccess = () => resolve((request.result as CandleCacheRecord | undefined) ?? null);
      request.onerror = () => reject(request.error ?? new Error("Trading cache read failed"));
    });
    const patches = await new Promise<CandlePatchRecord[]>((resolve, reject) => {
      if (!db.objectStoreNames.contains(PATCH_STORE_NAME)) {
        resolve([]);
        return;
      }
      const tx = db.transaction(PATCH_STORE_NAME, "readonly");
      const range = IDBKeyRange.bound(`${baseKey}:`, `${baseKey}:\uffff`);
      const request = tx.objectStore(PATCH_STORE_NAME).getAll(range);
      request.onsuccess = () => resolve((request.result as CandlePatchRecord[] | undefined) ?? []);
      request.onerror = () => reject(request.error ?? new Error("Trading patch cache read failed"));
    });
    if (!cached) return null;
    if (patches.length === 0) {
      return { ...cached, candles: trimDetachedLiveCandle(cached.candles, timeframe) };
    }
    const patchCandles = filterCoherentPatchCandles(cached?.candles ?? [], patches.map((record) => record.candle), timeframe);
    const patchedCandles = trimDetachedLiveCandle(mergeCandles(cached?.candles ?? [], patchCandles), timeframe);
    return {
      key: baseKey,
      schema: "forge.trading.market_cache.v1",
      instrument: MARKET_ASSET.instrument,
      timeframe,
      candles: patchedCandles,
      importedAt: cached?.importedAt ?? patches[0]?.importedAt ?? new Date().toISOString(),
      updatedAt: patches[patches.length - 1]?.updatedAt ?? cached?.updatedAt ?? new Date().toISOString(),
      source: MARKET_ASSET.source.cacheNamespace
    };
  } finally {
    db.close();
  }
}

async function writeCachedCandles(timeframe: TradingTimeframe, candles: TradingCandle[], importedAt?: string): Promise<void> {
  const db = await openTradingDb();
  const now = new Date().toISOString();
  const record: CandleCacheRecord = {
    key: cacheKey(timeframe),
    schema: "forge.trading.market_cache.v1",
    instrument: MARKET_ASSET.instrument,
    timeframe,
    candles,
    importedAt: importedAt ?? now,
    updatedAt: now,
    source: MARKET_ASSET.source.cacheNamespace
  };
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      tx.objectStore(STORE_NAME).put(record);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error("Trading cache write failed"));
    });
  } finally {
    db.close();
  }
}

async function writeCachedCandlePatches(timeframe: TradingTimeframe, candles: TradingCandle[], importedAt?: string): Promise<void> {
  if (candles.length === 0) return;
  const db = await openTradingDb();
  const now = new Date().toISOString();
  const baseKey = cacheKey(timeframe);
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(PATCH_STORE_NAME, "readwrite");
      const store = tx.objectStore(PATCH_STORE_NAME);
      for (const candle of candles) {
        const record: CandlePatchRecord = {
          id: `${baseKey}:${candle.time}`,
          baseKey,
          schema: "forge.trading.market_patch.v1",
          instrument: MARKET_ASSET.instrument,
          timeframe,
          candle,
          importedAt: importedAt ?? now,
          updatedAt: now,
          source: MARKET_ASSET.source.cacheNamespace
        };
        store.put(record);
      }
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error("Trading patch cache write failed"));
    });
  } finally {
    db.close();
  }
}

function mergeCandles(existing: TradingCandle[], incoming: TradingCandle[]): TradingCandle[] {
  const byTime = new Map<string, TradingCandle>();
  for (const candle of existing) byTime.set(candle.time, candle);
  for (const candle of incoming) byTime.set(candle.time, candle);
  return Array.from(byTime.values()).sort((a, b) => Date.parse(a.time) - Date.parse(b.time));
}

function trimDetachedLiveCandle(candles: TradingCandle[], timeframe: TradingTimeframe): TradingCandle[] {
  if (candles.length < 2) return candles;
  const duration = TIMEFRAME_MS[timeframe];
  const sorted = [...candles].sort((a, b) => Date.parse(a.time) - Date.parse(b.time));
  const last = sorted[sorted.length - 1];
  const previous = sorted[sorted.length - 2];
  if (last.complete !== false) return sorted;
  const lastTime = Date.parse(last.time);
  const previousTime = Date.parse(previous.time);
  if (!Number.isFinite(lastTime) || !Number.isFinite(previousTime)) return sorted;
  const lastBucket = Math.floor(lastTime / duration) * duration;
  const previousBucket = Math.floor(previousTime / duration) * duration;
  if (lastBucket - previousBucket > duration) return sorted.slice(0, -1);
  const previousRange = Math.max(candleRange(previous), 0.001);
  const lastRange = candleRange(last);
  return lastRange > Math.max(previousRange * 5, 0.12) ? sorted.slice(0, -1) : sorted;
}

function candleRange(candle: TradingCandle): number {
  return Math.max(0, candle.high - candle.low);
}

function filterCoherentPatchCandles(baseCandles: TradingCandle[], patchCandles: TradingCandle[], timeframe: TradingTimeframe): TradingCandle[] {
  if (patchCandles.length === 0 || baseCandles.length === 0) return patchCandles;
  const baseByTime = new Map(baseCandles.map((candle) => [candle.time, candle]));
  const duration = TIMEFRAME_MS[timeframe];
  return patchCandles.filter((patch) => {
    const base = baseByTime.get(patch.time);
    if (!base) return true;
    if (patch.complete !== false) return true;
    const patchTime = Date.parse(patch.time);
    if (!Number.isFinite(patchTime)) return false;
    const currentBucket = Math.floor(Date.now() / duration) * duration;
    const patchBucket = Math.floor(patchTime / duration) * duration;
    if (patchBucket !== currentBucket) return false;
    const baseRange = Math.max(candleRange(base), 0.001);
    const patchRange = candleRange(patch);
    return patchRange <= Math.max(baseRange * 4, 0.08);
  });
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`).join(",")}}`;
}

async function sha256Hex(value: string): Promise<string> {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest)).map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function normalizeViewportOffset(value: number, candleCount: number): number {
  if (!Number.isFinite(value) || candleCount <= 1) return 0;
  return clamp(value, 0, Math.max(0, candleCount - 1));
}

function latestChartCandles(candles: TradingCandle[], viewportOffset: number, candleCount: number): TradingCandle[] {
  const end = clamp(candles.length - Math.max(0, Math.floor(normalizeViewportOffset(viewportOffset, candles.length))), 0, candles.length);
  const count = clamp(Math.floor(candleCount), 1, 500);
  return candles.slice(Math.max(0, end - count), end);
}

async function chartSnapshotProof(snapshot: Omit<TradingChartWindowSnapshot, "proofHash">): Promise<string> {
  return sha256Hex(stableJson(snapshot));
}

async function chartResultProof(result: Omit<TradingChartResult, "proofHash">): Promise<string> {
  return sha256Hex(stableJson(result));
}

async function chartEditResultProof(result: Omit<TradingChartEditResult, "proofHash">): Promise<string> {
  return sha256Hex(stableJson(result));
}

async function chartEditElementProof(element: Omit<TradingChartEditElement, "proofHash">): Promise<string> {
  return sha256Hex(stableJson(element));
}

function normalizeChartEditLabel(action: TradingChartEditAction): string {
  const fallback = action.kind.replace(/_/g, " ");
  return (action.label || action.id || fallback).replace(/\s+/g, " ").trim().slice(0, 64) || fallback;
}

function normalizeChartEditTag(label: string, preferred?: string): string {
  const cleanPreferred = (preferred ?? "").trim();
  if ((cleanPreferred.startsWith("*") && cleanPreferred.endsWith("*")) || (cleanPreferred.startsWith("^") && cleanPreferred.endsWith("^"))) {
    return cleanPreferred;
  }
  return `*${label.replace(/[\n\r*^]/g, " ").replace(/\s+/g, " ").trim()}*`;
}

function normalizeTradingInstrument(value?: string): string {
  const normalized = (value ?? MARKET_ASSET.instrument).replace(/[^A-Za-z0-9]/g, "").toUpperCase();
  const active = MARKET_ASSET.instrument.replace(/[^A-Za-z0-9]/g, "").toUpperCase();
  return normalized === active ? MARKET_ASSET.instrument : (value ?? MARKET_ASSET.instrument);
}

function normalizeChartEditKind(kind: TradingChartEditAction["kind"] | string): TradingChartEditKind {
  const compact = String(kind).replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
  if (compact === "horizontal_line" || compact === "horizontalline") return "horizontal_line";
  if (compact === "vertical_line" || compact === "verticalline") return "vertical_line";
  if (compact === "select_candles" || compact === "selectcandles") return "select_candles";
  if (compact === "moving_average" || compact === "movingaverage") return "moving_average";
  if (compact === "donchian_channel" || compact === "donchianchannel") return "donchian_channel";
  if (compact === "vwap") return "vwap";
  if (compact === "ray") return "ray";
  if (compact === "clear") return "clear";
  return "select_candles";
}

function normalizeChartEditActionKind(action: TradingChartEditAction): TradingChartEditAction {
  const kind = normalizeChartEditKind(action.kind);
  return kind === action.kind ? action : { ...action, kind };
}

function normalizeChartEditElementId(action: TradingChartEditAction, label: string): string {
  const raw = (action.id || label).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return raw || `edit-${action.kind}`;
}

async function chartEditElementFromAction(action: TradingChartEditAction, event: TradingChartEditRequestEvent, index: number): Promise<TradingChartEditElement | null> {
  if (!action || action.kind === "clear") return null;
  const label = normalizeChartEditLabel(action);
  const baseId = normalizeChartEditElementId(action, label);
  const createdAt = new Date().toISOString();
  const base: Omit<TradingChartEditElement, "proofHash"> = {
    ...action,
    id: `${baseId}-${index + 1}`,
    label,
    tag: normalizeChartEditTag(label, action.tag),
    instrument: normalizeTradingInstrument(event.instrument),
    timeframe: event.timeframe,
    createdAt
  };
  return { ...base, proofHash: await chartEditElementProof(base) };
}

function chartEditMatchesTag(element: TradingChartEditElement, raw: string): boolean {
  const tag = raw.trim().toLowerCase();
  const plain = tag.replace(/^[*^]+|[*^]+$/g, "");
  return element.id.toLowerCase() === plain || element.label.toLowerCase() === plain || element.tag.toLowerCase() === tag;
}

function candleTimesForEditRange(action: TradingChartEditAction, candles: TradingCandle[], timeframe: TradingTimeframe): string[] {
  if (action.kind !== "select_candles") return action.candleTimes ?? [];
  const explicit = (action.candleTimes ?? []).filter(Boolean);
  if (explicit.length > 0) return explicit;
  const start = Date.parse(action.time ?? "");
  if (!Number.isFinite(start)) return [];
  const rawEnd = Date.parse(action.timeEnd ?? "");
  const duration = TIMEFRAME_MS[timeframe];
  const end = Number.isFinite(rawEnd) ? rawEnd : start + duration - 1;
  const min = Math.min(start, end);
  const max = Math.max(start, end);
  return candles
    .filter((candle) => {
      const time = Date.parse(candle.time);
      return Number.isFinite(time) && time >= min && time <= max;
    })
    .map((candle) => candle.time);
}

function normalizeChartEditActionForCandles(action: TradingChartEditAction, candles: TradingCandle[], timeframe: TradingTimeframe): TradingChartEditAction {
  const normalizedAction = normalizeChartEditActionKind(action);
  if (normalizedAction.kind !== "select_candles") return normalizedAction;
  return {
    ...normalizedAction,
    candleTimes: candleTimesForEditRange(normalizedAction, candles, timeframe)
  };
}

function captureTradingChartScreenshot(): { dataUrl: string; width: number; height: number } | null {
  const shell = document.querySelector<HTMLElement>(".tradingCanvas__pane:first-child .tradingCanvas__chartShell");
  if (!shell) return null;
  const canvases = Array.from(shell.querySelectorAll("canvas"));
  const first = canvases[0];
  if (!first) return null;
  const width = first.width;
  const height = first.height;
  if (width <= 0 || height <= 0) return null;
  const output = document.createElement("canvas");
  output.width = width;
  output.height = height;
  const ctx = output.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#090a0f";
  ctx.fillRect(0, 0, width, height);
  for (const canvas of canvases) {
    if (canvas.width > 0 && canvas.height > 0) {
      ctx.drawImage(canvas, 0, 0, width, height);
    }
  }
  return { dataUrl: output.toDataURL("image/png"), width, height };
}

function tradingIpcError(code: string, message: string): TradingChartResult["error"] {
  return { code: "bad_payload", message: `${code}: ${message}`, proofHash: "" };
}

function formatTimeScaleLabel(time: string, timeframe: TradingTimeframe): string {
  const date = new Date(time);
  if (!Number.isFinite(date.getTime())) return "";
  if (timeframe === "D1") {
    return date.toLocaleDateString(undefined, { month: "short", year: "2-digit" });
  }
  return date.toLocaleDateString(undefined, { day: "2-digit", month: "short" }) + " " + date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

function formatCursorTimeLabel(time: string, timeframe: TradingTimeframe): string {
  const date = new Date(time);
  if (!Number.isFinite(date.getTime())) return "";
  if (timeframe === "D1") {
    return date.toLocaleDateString(undefined, { weekday: "short", day: "2-digit", month: "short", year: "2-digit" });
  }
  return date.toLocaleDateString(undefined, { weekday: "short", day: "2-digit", month: "short", year: "2-digit" }) + "  " + date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

function drawRoundRect(ctx: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number): void {
  const r = Math.min(radius, width / 2, height / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + width - r, y);
  ctx.quadraticCurveTo(x + width, y, x + width, y + r);
  ctx.lineTo(x + width, y + height - r);
  ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);
  ctx.lineTo(x + r, y + height);
  ctx.quadraticCurveTo(x, y + height, x, y + height - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function drawCursorLabel(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, align: "center" | "right"): void {
  const paddingX = 8;
  const width = Math.ceil(ctx.measureText(text).width) + paddingX * 2;
  const height = 22;
  const left = align === "right" ? x - width : x - width / 2;
  const top = y - height / 2;
  ctx.fillStyle = "rgba(31, 32, 34, 0.96)";
  drawRoundRect(ctx, left, top, width, height, 3);
  ctx.fill();
  ctx.fillStyle = "rgba(223, 231, 251, 0.92)";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(text, left + width / 2, top + height / 2 + 0.5);
}

function formatCandleCountdown(candle: TradingCandle, timeframe: TradingTimeframe, referenceIso?: string): string {
  const start = Date.parse(candle.time);
  const reference = Date.parse(referenceIso ?? new Date().toISOString());
  if (!Number.isFinite(start) || !Number.isFinite(reference)) return "--:--";
  const closeAt = start + TIMEFRAME_MS[timeframe];
  const totalSeconds = Math.max(0, Math.ceil((closeAt - reference) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mm = String(minutes).padStart(hours > 0 ? 2 : 1, "0");
  const ss = String(seconds).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
}

function livePriceFromProjection(projection: MarketCanvasProjection, fallback: TradingCandle): number {
  const mid = projection.lastTick?.mid;
  return mid !== null && mid !== undefined && Number.isFinite(mid) ? mid : fallback.close;
}

function drawLivePriceBadge(ctx: CanvasRenderingContext2D, projection: MarketCanvasProjection, metrics: ChartMetrics, candle: TradingCandle): void {
  const price = livePriceFromProjection(projection, candle);
  const priceText = formatMarketPrice(price, projection.asset);
  const countdown = formatCandleCountdown(candle, projection.timeframe, projection.lastTick?.fetchedAt ?? projection.lastTick?.time);
  ctx.save();
  ctx.font = '700 12px "Geist Mono", ui-monospace, monospace';
  const priceWidth = ctx.measureText(priceText).width;
  ctx.font = '650 11px "Geist Mono", ui-monospace, monospace';
  const countdownWidth = ctx.measureText(countdown).width;
  const width = Math.max(44, Math.ceil(Math.max(priceWidth, countdownWidth)) + 14);
  const height = 34;
  const y = clamp(metrics.y(price), metrics.plot.top + height / 2, metrics.plot.bottom - height / 2);
  const left = metrics.width - width;
  const top = clamp(y - height / 2, metrics.plot.top, metrics.plot.bottom - height);

  ctx.strokeStyle = "rgba(0, 211, 126, 0.34)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(metrics.plot.left, y);
  ctx.lineTo(left, y);
  ctx.stroke();

  ctx.fillStyle = "rgba(0, 184, 126, 0.96)";
  drawRoundRect(ctx, left, top, width, height, 2);
  ctx.fill();
  ctx.fillStyle = "#eefcf6";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = '700 12px "Geist Mono", ui-monospace, monospace';
  ctx.fillText(priceText, left + width / 2, top + 11);
  ctx.font = '650 11px "Geist Mono", ui-monospace, monospace';
  ctx.fillText(countdown, left + width / 2, top + 24);
  ctx.restore();
}

function timeScaleIndexes(visibleCount: number, width: number): number[] {
  if (visibleCount <= 1) return [0];
  const targetLabels = width >= 980 ? 6 : width >= 680 ? 5 : width >= 480 ? 4 : width >= 340 ? 3 : 2;
  const labels = Math.min(targetLabels, visibleCount);
  const indexes = new Set<number>();
  for (let i = 0; i < labels; i += 1) {
    indexes.add(Math.round((i / Math.max(labels - 1, 1)) * (visibleCount - 1)));
  }
  return Array.from(indexes).sort((a, b) => a - b);
}

function formatMarketPrice(value: number, asset: MarketAssetDefinition): string {
  return value.toFixed(asset.pricePrecision);
}

function buildPriceAxisTicks(plot: ChartMetrics["plot"], low: number, high: number, asset: MarketAssetDefinition, y: (value: number) => number): MarketAxisTick[] {
  const ticks: MarketAxisTick[] = [];
  for (let i = 0; i <= 4; i += 1) {
    const value = high - ((high - low) / 4) * i;
    ticks.push({ label: formatMarketPrice(value, asset), y: y(value) });
  }
  void plot;
  return ticks;
}

function buildTimeAxisTicks(visible: TradingCandle[], plot: ChartMetrics["plot"], xStep: number, projection: MarketCanvasProjection): MarketAxisTick[] {
  const ticks: MarketAxisTick[] = [];
  for (const index of timeScaleIndexes(visible.length, plot.right - plot.left)) {
    const candle = visible[index];
    if (!candle) continue;
    const label = formatTimeScaleLabel(candle.time, projection.timeframe);
    if (!label) continue;
    ticks.push({ label, x: plot.left + index * xStep + xStep / 2 });
  }
  return ticks;
}

function applyTickToCandles(candles: TradingCandle[], tick: TradingTickResult, timeframe: TradingTimeframe): TradingCandle[] {
  if (!tick.accepted || tick.source !== MARKET_ASSET.source.cacheNamespace || tick.mid === null || !Number.isFinite(tick.mid)) return candles;
  const tickTime = Date.parse(tick.time || tick.fetchedAt);
  if (!Number.isFinite(tickTime)) return candles;
  const price = tick.mid;
  const duration = TIMEFRAME_MS[timeframe];
  const bucket = Math.floor(tickTime / duration) * duration;
  if (candles.length === 0) {
    return [{ time: new Date(bucket).toISOString(), open: price, high: price, low: price, close: price, volume: 1, complete: false }];
  }
  const next = [...candles];
  const last = next[next.length - 1];
  const lastTime = Date.parse(last.time);
  if (!Number.isFinite(lastTime)) return candles;
  const lastBucket = Math.floor(lastTime / duration) * duration;
  if (bucket < lastBucket) return candles;
  if (bucket === lastBucket) {
    const previous = next.length > 1 ? next[next.length - 2] : null;
    const previousRange = previous ? Math.max(candleRange(previous), 0.001) : 0.001;
    const currentRange = candleRange(last);
    if (last.complete === false && currentRange > Math.max(previousRange * 5, 0.12)) {
      next[next.length - 1] = { ...last, open: price, high: price, low: price, close: price, volume: Math.max(1, last.volume + 1), complete: false };
      return next;
    }
    next[next.length - 1] = {
      ...last,
      high: Math.max(last.high, price),
      low: Math.min(last.low, price),
      close: price,
      volume: last.volume + 1,
      complete: false
    };
    return next;
  }
  if (bucket - lastBucket > duration) {
    return [
      ...next,
      { time: new Date(bucket).toISOString(), open: price, high: price, low: price, close: price, volume: 1, complete: false }
    ];
  }
  return [
    ...next.slice(0, -1),
    { ...last, complete: true },
    { time: new Date(bucket).toISOString(), open: last.close, high: Math.max(last.close, price), low: Math.min(last.close, price), close: price, volume: 1, complete: false }
  ];
}

function historySyncStart(candles: TradingCandle[], timeframe: TradingTimeframe): string | null {
  if (candles.length === 0) return BACKFILL_FROM;
  const duration = TIMEFRAME_MS[timeframe];
  const sorted = [...candles].sort((a, b) => Date.parse(a.time) - Date.parse(b.time));
  const last = sorted[sorted.length - 1];
  const lastTime = Date.parse(last.time);
  if (!Number.isFinite(lastTime)) return BACKFILL_FROM;
  const lastBucket = Math.floor(lastTime / duration) * duration;
  const currentBucket = Math.floor(Date.now() / duration) * duration;

  if (currentBucket - lastBucket > duration) {
    return last.time;
  }

  const previous = sorted[sorted.length - 2];
  if (!previous || last.complete !== false) return null;
  const previousTime = Date.parse(previous.time);
  if (!Number.isFinite(previousTime)) return previous.time;
  const previousBucket = Math.floor(previousTime / duration) * duration;
  return lastBucket - previousBucket > duration ? previous.time : null;
}

interface MarketCanvasProjection {
  source: MarketDataSource;
  asset: MarketAssetDefinition;
  timeframe: TradingTimeframe;
  candles: TradingCandle[];
  lastTick: TradingTickResult | null;
  cursor: ChartCursor | null;
  viewportOffset: number;
  openTrades: TradingOpenTrade[];
  chartEdits: TradingChartEditElement[];
  highlightedChartEditId: string | null;
}

interface MarketAxisTick {
  label: string;
  x?: number;
  y?: number;
}

interface ChartMetrics {
  width: number;
  height: number;
  dpr: number;
  plot: { left: number; top: number; right: number; bottom: number };
  axis: { priceX: number; priceLabelX: number; timeY: number };
  visible: TradingCandle[];
  low: number;
  high: number;
  xStep: number;
  bodyWidth: number;
  priceTicks: MarketAxisTick[];
  timeTicks: MarketAxisTick[];
  y: (value: number) => number;
}

interface GpuProgramState {
  program: WebGLProgram;
  positionLocation: number;
  colorLocation: WebGLUniformLocation;
  buffer: WebGLBuffer;
}

const GPU_STATE = new WeakMap<HTMLCanvasElement, GpuProgramState | null>();
const BULL_CANDLE_CSS = "#089981";
const BEAR_CANDLE_CSS = "#f23645";
const BULL_CANDLE_GL: [number, number, number, number] = [0.031, 0.6, 0.506, 1];
const BEAR_CANDLE_GL: [number, number, number, number] = [0.949, 0.212, 0.271, 1];

function candleColor(candle: TradingCandle): string {
  return candle.close >= candle.open ? BULL_CANDLE_CSS : BEAR_CANDLE_CSS;
}


function snapToDevicePixel(value: number, dpr: number): number {
  return Math.round(value * dpr) / dpr;
}

function drawCrispCandlestick(ctx: CanvasRenderingContext2D, metrics: ChartMetrics, candle: TradingCandle, index: number): void {
  const dpr = metrics.dpr;
  const color = candleColor(candle);
  const x = snapToDevicePixel(metrics.plot.left + index * metrics.xStep + metrics.xStep / 2, dpr);
  const wickWidth = Math.max(1 / dpr, Math.min(1.15, metrics.bodyWidth * 0.22));
  const wickLeft = snapToDevicePixel(x - wickWidth / 2, dpr);
  const highY = snapToDevicePixel(metrics.y(candle.high), dpr);
  const lowY = snapToDevicePixel(metrics.y(candle.low), dpr);
  const openY = metrics.y(candle.open);
  const closeY = metrics.y(candle.close);
  const bodyWidth = Math.max(2 / dpr, snapToDevicePixel(metrics.bodyWidth, dpr));
  const bodyLeft = snapToDevicePixel(x - bodyWidth / 2, dpr);
  const minBodyHeight = Math.max(1, 1 / dpr);
  let bodyTop = snapToDevicePixel(Math.min(openY, closeY), dpr);
  let bodyBottom = snapToDevicePixel(Math.max(openY, closeY), dpr);
  if (bodyBottom - bodyTop < minBodyHeight) {
    const center = snapToDevicePixel((openY + closeY) / 2, dpr);
    bodyTop = center - minBodyHeight / 2;
    bodyBottom = bodyTop + minBodyHeight;
  }

  ctx.fillStyle = color;
  ctx.fillRect(wickLeft, Math.min(highY, lowY), wickWidth, Math.max(1 / dpr, Math.abs(lowY - highY)));
  ctx.fillRect(bodyLeft, bodyTop, bodyWidth, Math.max(minBodyHeight, bodyBottom - bodyTop));
}
function setupCanvasPixels(canvas: HTMLCanvasElement): { width: number; height: number; dpr: number } | null {
  const parent = canvas.parentElement;
  if (!parent) return null;
  const rect = parent.getBoundingClientRect();
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  const width = Math.max(320, Math.floor(rect.width));
  const height = Math.max(240, Math.floor(rect.height));
  canvas.width = Math.floor(width * dpr);
  canvas.height = Math.floor(height * dpr);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  return { width, height, dpr };
}

function projectedVisibleCandles(candles: TradingCandle[], viewportOffset: number): TradingCandle[] {
  const end = clamp(candles.length - Math.max(0, Math.floor(normalizeViewportOffset(viewportOffset, candles.length))), 0, candles.length);
  const start = Math.max(0, end - CANDLE_LIMIT_FOR_PAINT);
  return candles.slice(start, end);
}

function measureChart(canvas: HTMLCanvasElement, projection: MarketCanvasProjection): ChartMetrics | null {
  const pixels = setupCanvasPixels(canvas);
  if (!pixels) return null;
  const { width, height, dpr } = pixels;
  const axis = { priceX: width - 1, priceLabelX: width - 9, timeY: 0 };
  const plot = { left: 4, top: 38, right: Math.max(80, width - 58), bottom: height - 6 };
  const visible = projectedVisibleCandles(projection.candles, projection.viewportOffset);
  if (visible.length === 0) {
    return { width, height, dpr, plot, axis, visible, low: 0, high: 1, xStep: 1, bodyWidth: 3, priceTicks: [], timeTicks: [], y: () => plot.bottom };
  }
  const positionPrices = projection.openTrades.flatMap((trade) => [trade.price, trade.stopLossPrice, trade.takeProfitPrice].map((value) => Number(value)).filter(Number.isFinite));
  const lows = [...visible.map((candle) => candle.low), ...positionPrices];
  const highs = [...visible.map((candle) => candle.high), ...positionPrices];
  const min = Math.min(...lows);
  const max = Math.max(...highs);
  const pad = Math.max((max - min) * 0.08, 0.01);
  const low = min - pad;
  const high = max + pad;
  const y = (value: number) => plot.bottom - ((value - low) / (high - low)) * (plot.bottom - plot.top);
  const xStep = (plot.right - plot.left) / Math.max(visible.length, 1);
  const rawBodyWidth = Math.max(2 / dpr, Math.min(9, xStep * 0.54));
  const bodyWidth = Math.max(2 / dpr, Math.round(rawBodyWidth * dpr) / dpr);
  return {
    width,
    height,
    dpr,
    plot,
    axis,
    visible,
    low,
    high,
    xStep,
    bodyWidth,
    priceTicks: buildPriceAxisTicks(plot, low, high, projection.asset, y),
    timeTicks: buildTimeAxisTicks(visible, plot, xStep, projection),
    y
  };
}

function compileGpuShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function gpuProgram(canvas: HTMLCanvasElement): GpuProgramState | null {
  if (GPU_STATE.has(canvas)) return GPU_STATE.get(canvas) ?? null;
  const gl = canvas.getContext("webgl2", { alpha: true, antialias: false, depth: false, stencil: false, powerPreference: "high-performance", desynchronized: true });
  if (!gl) {
    GPU_STATE.set(canvas, null);
    return null;
  }
  const vertexShader = compileGpuShader(gl, gl.VERTEX_SHADER, `#version 300 es
    in vec2 a_position;
    void main() { gl_Position = vec4(a_position, 0.0, 1.0); }
  `);
  const fragmentShader = compileGpuShader(gl, gl.FRAGMENT_SHADER, `#version 300 es
    precision mediump float;
    uniform vec4 u_color;
    out vec4 outColor;
    void main() { outColor = u_color; }
  `);
  const program = gl.createProgram();
  const buffer = gl.createBuffer();
  if (!vertexShader || !fragmentShader || !program || !buffer) {
    GPU_STATE.set(canvas, null);
    return null;
  }
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  gl.deleteShader(vertexShader);
  gl.deleteShader(fragmentShader);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    gl.deleteProgram(program);
    gl.deleteBuffer(buffer);
    GPU_STATE.set(canvas, null);
    return null;
  }
  const positionLocation = gl.getAttribLocation(program, "a_position");
  const colorLocation = gl.getUniformLocation(program, "u_color");
  if (positionLocation < 0 || !colorLocation) {
    gl.deleteProgram(program);
    gl.deleteBuffer(buffer);
    GPU_STATE.set(canvas, null);
    return null;
  }
  const state = { program, positionLocation, colorLocation, buffer };
  GPU_STATE.set(canvas, state);
  return state;
}

function clipPoint(metrics: ChartMetrics, x: number, y: number): [number, number] {
  return [(x / metrics.width) * 2 - 1, 1 - (y / metrics.height) * 2];
}

function pushTri(vertices: number[], metrics: ChartMetrics, left: number, top: number, right: number, bottom: number): void {
  const a = clipPoint(metrics, left, top);
  const b = clipPoint(metrics, right, top);
  const c = clipPoint(metrics, left, bottom);
  const d = clipPoint(metrics, right, bottom);
  vertices.push(...a, ...c, ...b, ...b, ...c, ...d);
}

function drawGpuVertices(canvas: HTMLCanvasElement, vertices: number[], mode: number, color: [number, number, number, number]): boolean {
  const state = gpuProgram(canvas);
  const gl = canvas.getContext("webgl2");
  if (!state || !gl || vertices.length === 0) return Boolean(state && gl);
  gl.useProgram(state.program);
  gl.bindBuffer(gl.ARRAY_BUFFER, state.buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(state.positionLocation);
  gl.vertexAttribPointer(state.positionLocation, 2, gl.FLOAT, false, 0, 0);
  gl.uniform4fv(state.colorLocation, color);
  gl.drawArrays(mode, 0, vertices.length / 2);
  return true;
}

function drawGpuMarketLayer(canvas: HTMLCanvasElement, metrics: ChartMetrics, cursor: ChartCursor | null): boolean {
  const state = gpuProgram(canvas);
  const gl = canvas.getContext("webgl2");
  if (!state || !gl) return false;
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  gl.viewport(0, 0, Math.floor(metrics.width * dpr), Math.floor(metrics.height * dpr));
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  void cursor;
  return true;
}

function tradingNumber(value?: string): number | null {
  if (!value) return null;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function tradingTradeSide(units?: string): "long" | "short" | null {
  const parsed = tradingNumber(units);
  if (parsed === null || parsed === 0) return null;
  return parsed > 0 ? "long" : "short";
}

function xForTradeOpenTime(trade: TradingOpenTrade, metrics: ChartMetrics, timeframe: TradingTimeframe): number | null {
  if (metrics.visible.length === 0) return null;
  const openTime = Date.parse(trade.openTime ?? "");
  if (!Number.isFinite(openTime)) return metrics.plot.left;
  const firstTime = Date.parse(metrics.visible[0]?.time ?? "");
  const lastTime = Date.parse(metrics.visible[metrics.visible.length - 1]?.time ?? "");
  if (!Number.isFinite(firstTime) || !Number.isFinite(lastTime)) return metrics.plot.left;
  if (openTime <= firstTime) return metrics.plot.left;
  if (openTime > lastTime + TIMEFRAME_MS[timeframe]) return null;
  const index = metrics.visible.findIndex((candle) => Date.parse(candle.time) >= openTime);
  const resolvedIndex = index >= 0 ? index : metrics.visible.length - 1;
  return metrics.plot.left + resolvedIndex * metrics.xStep + metrics.xStep / 2;
}

function drawTradingPriceMarker(ctx: CanvasRenderingContext2D, label: string, x: number, y: number, color: string, alpha = 0.92): number {
  ctx.save();
  ctx.font = '650 11px "Geist Mono", ui-monospace, monospace';
  const width = Math.ceil(ctx.measureText(label).width) + 12;
  const height = 20;
  const left = x - width;
  const top = y - height / 2;
  ctx.globalAlpha = alpha;
  ctx.fillStyle = color;
  drawRoundRect(ctx, left, top, width, height, 2);
  ctx.fill();
  ctx.fillStyle = "#f7fbff";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(label, left + width / 2, top + height / 2 + 0.5);
  ctx.restore();
  return left;
}

function drawPositionLine(ctx: CanvasRenderingContext2D, metrics: ChartMetrics, projection: MarketCanvasProjection, trade: TradingOpenTrade, price: number, color: string, label: string, subtle: boolean): void {
  const startX = xForTradeOpenTime(trade, metrics, projection.timeframe);
  if (startX === null) return;
  const y = metrics.y(price);
  if (y < metrics.plot.top - 16 || y > metrics.plot.bottom + 16) return;
  const markerY = clamp(y, metrics.plot.top + 10, metrics.plot.bottom - 10);
  const markerLeft = drawTradingPriceMarker(ctx, label, metrics.width - 1, markerY, color, subtle ? 0.86 : 0.94);
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = subtle ? 1 : 1.25;
  ctx.globalAlpha = subtle ? 0.82 : 0.86;

  ctx.beginPath();
  ctx.moveTo(startX, y);
  ctx.lineTo(markerLeft, y);
  ctx.stroke();
  ctx.restore();
}
function drawOpenTradeLines(ctx: CanvasRenderingContext2D, projection: MarketCanvasProjection, metrics: ChartMetrics): void {
  const visibleTrades = projection.openTrades.filter((trade) => trade.instrument === projection.asset.instrument);
  for (const trade of visibleTrades) {
    const side = tradingTradeSide(trade.currentUnits);
    const entryPrice = tradingNumber(trade.price);
    if (!side || entryPrice === null) continue;
    const entryColor = side === "long" ? "rgba(0, 155, 255, 0.92)" : "rgba(255, 149, 36, 0.92)";
    drawPositionLine(ctx, metrics, projection, trade, entryPrice, entryColor, formatMarketPrice(entryPrice, projection.asset), false);
    const stopLossPrice = tradingNumber(trade.stopLossPrice);
    if (stopLossPrice !== null) {
      drawPositionLine(ctx, metrics, projection, trade, stopLossPrice, "rgba(214, 28, 60, 0.94)", `SL ${formatMarketPrice(stopLossPrice, projection.asset)}`, true);
    }
    const takeProfitPrice = tradingNumber(trade.takeProfitPrice);
    if (takeProfitPrice !== null) {
      drawPositionLine(ctx, metrics, projection, trade, takeProfitPrice, "rgba(34, 218, 197, 0.92)", `TP ${formatMarketPrice(takeProfitPrice, projection.asset)}`, true);
    }
  }
}
function chartEditColor(element: TradingChartEditElement): string {
  const color = (element.color ?? "").toLowerCase();
  if (color.includes("cyan") || color.includes("turquoise") || color.includes("teal")) return "rgba(45, 229, 218, 0.95)";
  if (color.includes("red")) return "rgba(255, 75, 102, 0.95)";
  if (color.includes("orange")) return "rgba(255, 170, 70, 0.95)";
  if (color.includes("green")) return "rgba(0, 211, 126, 0.95)";
  if (color.includes("yellow")) return "rgba(240, 210, 85, 0.95)";
  return "rgba(45, 229, 218, 0.95)";
}

function xForEditTime(time: string | undefined, metrics: ChartMetrics): number | null {
  if (!time || metrics.visible.length === 0) return null;
  const target = Date.parse(time);
  if (!Number.isFinite(target)) return null;
  const first = Date.parse(metrics.visible[0]?.time ?? "");
  const last = Date.parse(metrics.visible[metrics.visible.length - 1]?.time ?? "");
  if (!Number.isFinite(first) || !Number.isFinite(last)) return null;
  if (target <= first) return metrics.plot.left;
  if (target >= last) return metrics.plot.right;
  const index = metrics.visible.findIndex((candle) => Date.parse(candle.time) >= target);
  const resolved = index >= 0 ? index : metrics.visible.length - 1;
  return metrics.plot.left + resolved * metrics.xStep + metrics.xStep / 2;
}

function drawChartEditLabel(ctx: CanvasRenderingContext2D, metrics: ChartMetrics, text: string, x: number, y: number, color: string): void {
  ctx.save();
  ctx.font = '650 11px "Geist Mono", ui-monospace, monospace';
  const width = Math.min(180, Math.ceil(ctx.measureText(text).width) + 12);
  const left = clamp(x, metrics.plot.left + width / 2, metrics.plot.right - width / 2) - width / 2;
  const top = clamp(y - 25, metrics.plot.top + 2, metrics.plot.bottom - 24);
  ctx.fillStyle = "rgba(9, 10, 15, 0.82)";
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  drawRoundRect(ctx, left, top, width, 20, 3);
  ctx.fill();
  ctx.stroke();
  ctx.fillStyle = "rgba(237, 252, 255, 0.96)";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(text, left + width / 2, top + 10.5, width - 8);
  ctx.restore();
}

function movingAveragePoints(candles: TradingCandle[], metrics: ChartMetrics, period: number): Array<{ x: number; y: number }> {
  const byTime = new Map(candles.map((candle, index) => [candle.time, index]));
  const points: Array<{ x: number; y: number }> = [];
  metrics.visible.forEach((candle, visibleIndex) => {
    const sourceIndex = byTime.get(candle.time) ?? -1;
    if (sourceIndex < period - 1) return;
    const slice = candles.slice(sourceIndex - period + 1, sourceIndex + 1);
    const average = slice.reduce((sum, item) => sum + item.close, 0) / slice.length;
    points.push({ x: metrics.plot.left + visibleIndex * metrics.xStep + metrics.xStep / 2, y: metrics.y(average) });
  });
  return points;
}

function vwapPoints(candles: TradingCandle[], metrics: ChartMetrics): Array<{ x: number; y: number }> {
  const visibleTimes = new Set(metrics.visible.map((candle) => candle.time));
  let priceVolume = 0;
  let volume = 0;
  const points: Array<{ x: number; y: number }> = [];
  candles.forEach((candle) => {
    const candleVolume = Math.max(1, Number(candle.volume) || 1);
    const typical = (candle.high + candle.low + candle.close) / 3;
    priceVolume += typical * candleVolume;
    volume += candleVolume;
    if (!visibleTimes.has(candle.time)) return;
    const visibleIndex = metrics.visible.findIndex((item) => item.time === candle.time);
    if (visibleIndex < 0) return;
    points.push({ x: metrics.plot.left + visibleIndex * metrics.xStep + metrics.xStep / 2, y: metrics.y(priceVolume / Math.max(1, volume)) });
  });
  return points;
}

function donchianPoints(candles: TradingCandle[], metrics: ChartMetrics, period: number): { upper: Array<{ x: number; y: number }>; lower: Array<{ x: number; y: number }> } {
  const byTime = new Map(candles.map((candle, index) => [candle.time, index]));
  const upper: Array<{ x: number; y: number }> = [];
  const lower: Array<{ x: number; y: number }> = [];
  metrics.visible.forEach((candle, visibleIndex) => {
    const sourceIndex = byTime.get(candle.time) ?? -1;
    if (sourceIndex < period - 1) return;
    const slice = candles.slice(sourceIndex - period + 1, sourceIndex + 1);
    const high = Math.max(...slice.map((item) => item.high));
    const low = Math.min(...slice.map((item) => item.low));
    const x = metrics.plot.left + visibleIndex * metrics.xStep + metrics.xStep / 2;
    upper.push({ x, y: metrics.y(high) });
    lower.push({ x, y: metrics.y(low) });
  });
  return { upper, lower };
}

function strokeChartEditPath(ctx: CanvasRenderingContext2D, points: Array<{ x: number; y: number }>, color: string, highlighted: boolean): void {
  if (points.length < 2) return;
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = highlighted ? 2.6 : 1.45;
  ctx.shadowColor = highlighted ? color : "transparent";
  ctx.shadowBlur = highlighted ? 12 : 0;
  ctx.beginPath();
  points.forEach((point, index) => {
    if (index === 0) ctx.moveTo(point.x, point.y);
    else ctx.lineTo(point.x, point.y);
  });
  ctx.stroke();
  ctx.restore();
}

function drawChartEdits(ctx: CanvasRenderingContext2D, projection: MarketCanvasProjection, metrics: ChartMetrics): void {
  const visibleTimes = new Set(metrics.visible.map((candle) => candle.time));
  for (const element of projection.chartEdits.filter((edit) => edit.timeframe === projection.timeframe && edit.instrument === projection.asset.instrument)) {
    const color = chartEditColor(element);
    const highlighted = projection.highlightedChartEditId === element.id;
    ctx.save();
    ctx.globalAlpha = highlighted ? 1 : 0.88;
    ctx.lineWidth = highlighted ? 2.5 : 1.4;
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.shadowColor = highlighted ? color : "transparent";
    ctx.shadowBlur = highlighted ? 16 : 0;

    if (element.kind === "select_candles") {
      const selected = new Set(element.candleTimes ?? []);
      metrics.visible.forEach((candle, index) => {
        if (!selected.has(candle.time)) return;
        const x = metrics.plot.left + index * metrics.xStep + metrics.xStep / 2;
        const left = x - Math.max(metrics.bodyWidth, metrics.xStep * 0.42);
        const width = Math.max(metrics.bodyWidth * 2.2, metrics.xStep * 0.84);
        ctx.fillStyle = highlighted ? "rgba(45, 229, 218, 0.42)" : "rgba(45, 229, 218, 0.28)";
        drawRoundRect(ctx, left, metrics.plot.top + 2, width, metrics.plot.bottom - metrics.plot.top - 4, 3);
        ctx.fill();
        ctx.strokeStyle = highlighted ? "rgba(159, 246, 255, 0.98)" : "rgba(45, 229, 218, 0.82)";
        ctx.lineWidth = highlighted ? 2.1 : 1.25;
        drawRoundRect(ctx, left, metrics.plot.top + 2, width, metrics.plot.bottom - metrics.plot.top - 4, 3);
        ctx.stroke();
      });
    } else if (element.kind === "horizontal_line" && typeof element.price === "number") {
      const y = metrics.y(element.price);
      ctx.beginPath();
      ctx.moveTo(metrics.plot.left, y);
      ctx.lineTo(metrics.plot.right, y);
      ctx.stroke();
      drawChartEditLabel(ctx, metrics, element.label, metrics.plot.right - 42, y, color);
    } else if (element.kind === "vertical_line") {
      const x = xForEditTime(element.time, metrics);
      if (x !== null) {
        ctx.beginPath();
        ctx.moveTo(x, metrics.plot.top);
        ctx.lineTo(x, metrics.plot.bottom);
        ctx.stroke();
        drawChartEditLabel(ctx, metrics, element.label, x, metrics.plot.top + 28, color);
      }
    } else if (element.kind === "ray" && typeof element.price === "number") {
      const x1 = xForEditTime(element.time, metrics) ?? metrics.plot.left;
      const y1 = metrics.y(element.price);
      const x2 = xForEditTime(element.timeEnd, metrics) ?? metrics.plot.right;
      const y2 = typeof element.priceEnd === "number" ? metrics.y(element.priceEnd) : y1;
      const slope = (y2 - y1) / Math.max(1, x2 - x1);
      const endX = metrics.plot.right;
      const endY = y1 + slope * (endX - x1);
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(endX, clamp(endY, metrics.plot.top - 100, metrics.plot.bottom + 100));
      ctx.stroke();
      drawChartEditLabel(ctx, metrics, element.label, x1 + 54, y1, color);
    } else if (element.kind === "moving_average") {
      const period = Math.max(1, Math.floor(element.period ?? 20));
      const points = movingAveragePoints(projection.candles, metrics, period);
      strokeChartEditPath(ctx, points, color, highlighted);
      const last = points.at(-1);
      if (last) drawChartEditLabel(ctx, metrics, element.label, last.x, last.y, color);
    } else if (element.kind === "vwap") {
      const points = vwapPoints(projection.candles, metrics);
      strokeChartEditPath(ctx, points, color, highlighted);
      const last = points.at(-1);
      if (last) drawChartEditLabel(ctx, metrics, element.label, last.x, last.y, color);
    } else if (element.kind === "donchian_channel") {
      const period = Math.max(1, Math.floor(element.period ?? 20));
      const channel = donchianPoints(projection.candles, metrics, period);
      strokeChartEditPath(ctx, channel.upper, color, highlighted);
      strokeChartEditPath(ctx, channel.lower, color, highlighted);
      const last = channel.upper.at(-1);
      if (last) drawChartEditLabel(ctx, metrics, element.label, last.x, last.y, color);
    }
    ctx.restore();
    void visibleTimes;
  }
}
function drawOverlayLayer(canvas: HTMLCanvasElement, projection: MarketCanvasProjection, metrics: ChartMetrics, includeCandles: boolean): void {
  setupCanvasPixels(canvas);
  const ctx = canvas.getContext("2d", { alpha: true, desynchronized: true });
  if (!ctx) return;
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, metrics.width, metrics.height);
  if (metrics.visible.length === 0) return;

  ctx.fillStyle = "#8f9cb8";
  ctx.font = '11px "Geist Mono", ui-monospace, monospace';
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  for (const tick of metrics.priceTicks) {
    if (typeof tick.y !== "number") continue;
    ctx.fillText(tick.label, metrics.axis.priceLabelX, tick.y);
  }

  ctx.textBaseline = "top";
  for (const tick of metrics.timeTicks) {
    if (typeof tick.x !== "number") continue;
    ctx.textAlign = tick.x < 52 ? "left" : tick.x > metrics.plot.right - 52 ? "right" : "center";
    ctx.fillStyle = "#8f9cb8";
    ctx.fillText(tick.label, tick.x, metrics.axis.timeY);
  }

  if (includeCandles) {
    metrics.visible.forEach((candle, index) => {
      drawCrispCandlestick(ctx, metrics, candle, index);
    });
  }

  drawOpenTradeLines(ctx, projection, metrics);
  drawChartEdits(ctx, projection, metrics);

  const last = metrics.visible[metrics.visible.length - 1];
  const latestCandle = projection.candles.at(-1) ?? last;
  drawLivePriceBadge(ctx, projection, metrics, latestCandle);

  if (projection.cursor) {
    const cursorX = clamp(projection.cursor.x, metrics.plot.left, metrics.plot.right);
    const cursorY = clamp(projection.cursor.y, metrics.plot.top, metrics.plot.bottom);
    const candleIndex = clamp(Math.round((cursorX - metrics.plot.left - metrics.xStep / 2) / metrics.xStep), 0, metrics.visible.length - 1);
    const candle = metrics.visible[candleIndex];
    const crossX = metrics.plot.left + candleIndex * metrics.xStep + metrics.xStep / 2;
    const price = metrics.high - ((cursorY - metrics.plot.top) / (metrics.plot.bottom - metrics.plot.top)) * (metrics.high - metrics.low);
    const dateLabel = candle ? formatCursorTimeLabel(candle.time, projection.timeframe) : "";
    if (includeCandles) {
      ctx.save();
      ctx.setLineDash([4, 5]);
      ctx.strokeStyle = "rgba(143, 156, 184, 0.44)";
      ctx.beginPath();
      ctx.moveTo(crossX, metrics.plot.top);
      ctx.lineTo(crossX, metrics.plot.bottom);
      ctx.moveTo(metrics.plot.left, cursorY);
      ctx.lineTo(metrics.plot.right, cursorY);
      ctx.stroke();
      ctx.restore();
    }
    ctx.font = '11px "Geist Mono", ui-monospace, monospace';
    drawCursorLabel(ctx, formatMarketPrice(price, projection.asset), metrics.axis.priceX - 2, cursorY, "right");
    if (dateLabel) {
      const dateWidth = Math.ceil(ctx.measureText(dateLabel).width) + 16;
      const dateX = clamp(crossX, metrics.plot.left + dateWidth / 2, metrics.plot.right - dateWidth / 2);
      drawCursorLabel(ctx, dateLabel, dateX, metrics.plot.top - 18, "center");
    }
  }

  void projection.source;
  void projection.lastTick;
}

function drawMarketCanvasChart(gpuCanvas: HTMLCanvasElement, overlayCanvas: HTMLCanvasElement, projection: MarketCanvasProjection): void {
  const metrics = measureChart(gpuCanvas, projection);
  if (!metrics) return;
  const gpuReady = drawGpuMarketLayer(gpuCanvas, metrics, projection.cursor);
  drawOverlayLayer(overlayCanvas, projection, metrics, true);
  void gpuReady;
}

interface MarketCanvasSurfaceProps {
  projection: MarketCanvasProjection;
  onCursorMove: (event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => void;
  onCursorLeave: () => void;
  onPointerDown: (event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => void;
  onPointerUp: (event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => void;
  onWheel: (event: globalThis.WheelEvent, canvas: HTMLCanvasElement) => void;
  onContextMenu: (event: globalThis.MouseEvent) => void;
}

function MarketCanvasSurface({ projection, onCursorMove, onCursorLeave, onPointerDown, onPointerUp, onWheel, onContextMenu }: MarketCanvasSurfaceProps) {
  const gpuCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const overlayCanvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const gpuCanvas = gpuCanvasRef.current;
    const overlayCanvas = overlayCanvasRef.current;
    if (!gpuCanvas || !overlayCanvas) return undefined;
    let frame = 0;
    const paint = () => {
      frame = 0;
      drawMarketCanvasChart(gpuCanvas, overlayCanvas, projection);
    };
    const schedule = () => {
      if (frame === 0) frame = window.requestAnimationFrame(paint);
    };
    const observer = new ResizeObserver(schedule);
    if (gpuCanvas.parentElement) observer.observe(gpuCanvas.parentElement);
    schedule();
    return () => {
      if (frame !== 0) window.cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [projection]);

  useEffect(() => {
    const canvas = gpuCanvasRef.current;
    if (!canvas) return undefined;
    const pointerMove = (event: globalThis.PointerEvent) => onCursorMove(event, canvas);
    const pointerDown = (event: globalThis.PointerEvent) => onPointerDown(event, canvas);
    const pointerUp = (event: globalThis.PointerEvent) => onPointerUp(event, canvas);
    const wheel = (event: globalThis.WheelEvent) => onWheel(event, canvas);
    const contextMenu = (event: globalThis.MouseEvent) => onContextMenu(event);
    const pointerLeave = () => onCursorLeave();

    canvas.addEventListener("pointermove", pointerMove, { passive: false });
    canvas.addEventListener("pointerdown", pointerDown, { passive: false });
    canvas.addEventListener("pointerup", pointerUp, { passive: false });
    canvas.addEventListener("pointercancel", pointerUp, { passive: false });
    canvas.addEventListener("lostpointercapture", pointerUp, { passive: false });
    canvas.addEventListener("pointerleave", pointerLeave, { passive: true });
    canvas.addEventListener("wheel", wheel, { passive: false });
    canvas.addEventListener("contextmenu", contextMenu, { passive: false });
    return () => {
      canvas.removeEventListener("pointermove", pointerMove);
      canvas.removeEventListener("pointerdown", pointerDown);
      canvas.removeEventListener("pointerup", pointerUp);
      canvas.removeEventListener("pointercancel", pointerUp);
      canvas.removeEventListener("lostpointercapture", pointerUp);
      canvas.removeEventListener("pointerleave", pointerLeave);
      canvas.removeEventListener("wheel", wheel);
      canvas.removeEventListener("contextmenu", contextMenu);
    };
  }, [onContextMenu, onCursorLeave, onCursorMove, onPointerDown, onPointerUp, onWheel]);

  return (
    <div className="tradingCanvas__chartShell">
      <canvas ref={gpuCanvasRef} className="tradingCanvas__chart tradingCanvas__chart--gpu" />
      <canvas ref={overlayCanvasRef} className="tradingCanvas__chart tradingCanvas__chart--overlay" aria-hidden="true" />
    </div>
  );
}

export function TradingCanvas({ parallelCount = 1 }: { parallelCount?: number }) {
  const [timeframe, setTimeframe] = useState<TradingTimeframe>("H1");
  const [candlesByTimeframe, setCandlesByTimeframe] = useState<CandleMap>({});
  const [importState, setImportState] = useState<ImportState>({});
  const [lastTick, setLastTick] = useState<TradingTickResult | null>(null);
  const [openTrades, setOpenTrades] = useState<TradingOpenTrade[]>([]);
  const [chartEdits, setChartEdits] = useState<TradingChartEditElement[]>([]);
  const [highlightedChartEditId, setHighlightedChartEditId] = useState<string | null>(null);
  const [cursor, setCursor] = useState<ChartCursor | null>(null);
  const [contextMenu, setContextMenu] = useState<ChartContextMenu>(null);
  const [viewportOffset, setViewportOffset] = useState(0);
  const [status, setStatus] = useState("loading OANDA cache");
  const stoppedRef = useRef(false);
  const tickInFlightRef = useRef(false);
  const accountStateInFlightRef = useRef(false);
  const importInFlightRef = useRef(new Set<TradingTimeframe>());
  const importedAtRef = useRef<Partial<Record<TradingTimeframe, string>>>({});
  const dragRef = useRef<{ pointerId: number; startX: number; startOffset: number } | null>(null);
  const userNavigatedViewportRef = useRef(false);
  const cursorFrameRef = useRef(0);
  const pendingCursorRef = useRef<ChartCursor | null>(null);
  const viewportFrameRef = useRef(0);
  const pendingViewportOffsetRef = useRef(0);

  const activeCandles = candlesByTimeframe[timeframe] ?? [];
  const chartCount = clamp(Math.floor(parallelCount), 1, 4);

  const syncH1History = useCallback(async (seed: TradingCandle[] = [], from: string = BACKFILL_FROM, latestFirst = false) => {
    if (importInFlightRef.current.has("H1")) return;
    const api = globalThis.window?.forgeShell?.getTradingCandles;
    if (!api) {
      setImportState((current) => ({ ...current, H1: "bridge unavailable" }));
      return;
    }
    importInFlightRef.current.add("H1");
    const importedAt = new Date().toISOString();
    let merged = seed;
    try {
      if (latestFirst) {
        setImportState((current) => ({ ...current, H1: "downloading latest H1" }));
        const latest = await api({ instrument: INSTRUMENT, timeframe: "H1", count: LATEST_BOOTSTRAP_COUNT });
        if (!latest.accepted || latest.source !== MARKET_ASSET.source.cacheNamespace) {
          setImportState((current) => ({ ...current, H1: latest.error?.message ?? "OANDA latest H1 failed" }));
          return;
        }
        merged = mergeCandles(merged, latest.candles);
        if (merged.length > 0) {
          importedAtRef.current.H1 = importedAt;
          if (!userNavigatedViewportRef.current) setViewportOffset(0);
          setCandlesByTimeframe((current) => ({ ...current, H1: mergeCandles(current.H1 ?? [], merged) }));
          setImportState((current) => ({ ...current, H1: `${merged.length} H1 latest` }));
          await writeCachedCandles("H1", merged, importedAt);
        }
      }

      setImportState((current) => ({ ...current, H1: "downloading H1 since 2006" }));
      let cursor = from;
      for (let page = 0; page < MAX_BACKFILL_PAGES && !stoppedRef.current; page += 1) {
        const result = await api({ instrument: INSTRUMENT, timeframe: "H1", from: cursor, count: BACKFILL_CHUNK_SIZE });
        if (!result.accepted || result.source !== MARKET_ASSET.source.cacheNamespace) {
          setImportState((current) => ({ ...current, H1: result.error?.message ?? "OANDA H1 import failed" }));
          return;
        }
        if (result.candles.length === 0) break;
        merged = mergeCandles(merged, result.candles);
        importedAtRef.current.H1 = importedAt;
        if (!userNavigatedViewportRef.current) setViewportOffset(0);
        setCandlesByTimeframe((current) => ({ ...current, H1: mergeCandles(current.H1 ?? [], merged) }));
        setImportState((current) => ({ ...current, H1: `${merged.length} H1 saved` }));
        await writeCachedCandles("H1", merged, importedAt);
        const nextCursor = result.nextFrom;
        if (!nextCursor || nextCursor === cursor) break;
        cursor = nextCursor;
      }
      setImportState((current) => ({ ...current, H1: merged.length > 0 ? `${merged.length} H1 durable` : "no H1 candles" }));
    } finally {
      importInFlightRef.current.delete("H1");
    }
  }, []);
  useEffect(() => {
    stoppedRef.current = false;
    void (async () => {
      const cached = await readCachedCandles("H1").catch(() => null);
      if (stoppedRef.current) return;
      if (cached?.source === MARKET_ASSET.source.cacheNamespace && cached.candles?.length) {
        importedAtRef.current.H1 = cached.importedAt;
        if (!userNavigatedViewportRef.current) setViewportOffset(0);
        setCandlesByTimeframe({ H1: cached.candles });
        setImportState({ H1: `${cached.candles.length} H1 durable cache` });
        if (cached.candles.length < MIN_DURABLE_CHART_CANDLES) {
          setStatus("refreshing short H1 candle cache");
          await syncH1History(cached.candles, BACKFILL_FROM, true);
          if (!stoppedRef.current) setStatus("H1 cache resident + OANDA live ticks");
          return;
        }
        setStatus("H1 cache resident + OANDA live ticks");
        const syncFrom = historySyncStart(cached.candles, "H1");
        if (syncFrom) {
          setStatus("catching up missing H1 candles");
          await syncH1History(cached.candles, syncFrom, false);
        }
        if (!stoppedRef.current) setStatus("H1 cache resident + OANDA live ticks");
        return;
      }
      setStatus("downloading H1 OANDA history");
      await syncH1History([], BACKFILL_FROM, true);
      if (!stoppedRef.current) setStatus("H1 history resident + OANDA live ticks");
    })();
    return () => {
      stoppedRef.current = true;
    };
  }, [syncH1History]);
  useEffect(() => {
    let timer = 0;
    let cancelled = false;
    const pollTick = async () => {
      if (tickInFlightRef.current) return;
      const api = globalThis.window?.forgeShell?.getTradingTick;
      if (!api) {
        setStatus("trading bridge unavailable");
        return;
      }
      tickInFlightRef.current = true;
      try {
        const tick = await api({ instrument: INSTRUMENT });
        if (!cancelled) {
          setLastTick(tick);
          if (!tick.accepted) {
            setStatus(tick.error?.message ?? "tick rejected");
            return;
          }
          setCandlesByTimeframe((current) => {
            const candles = current.H1 ?? [];
            const updated = applyTickToCandles(candles, tick, "H1");
            if (updated !== candles) {
              const changedCount = updated.length > candles.length ? 2 : 1;
              const changed = updated.slice(Math.max(0, updated.length - changedCount));
              void writeCachedCandlePatches("H1", changed, importedAtRef.current.H1).catch(() => undefined);
            }
            return { ...current, H1: updated };
          });
          setStatus("OANDA live tick loop");
        }
      } finally {
        tickInFlightRef.current = false;
      }
    };
    const schedule = () => {
      if (!cancelled) {
        timer = window.setTimeout(() => void pollTick().finally(schedule), OANDA_TICK_POLL_MS);
      }
    };
    void pollTick().finally(schedule);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    let timer = 0;
    let cancelled = false;
    const pollAccountState = async () => {
      if (accountStateInFlightRef.current) return;
      const api = globalThis.window?.forgeShell?.getTradingAccountState;
      if (!api) return;
      accountStateInFlightRef.current = true;
      try {
        const state = await api({ instrument: INSTRUMENT, includeHistory: false });
        if (!cancelled && state.accepted) {
          setOpenTrades(state.openTrades.filter((trade) => trade.instrument === INSTRUMENT));
        }
      } finally {
        accountStateInFlightRef.current = false;
      }
    };
    const schedule = () => {
      if (!cancelled) {
        timer = window.setTimeout(() => void pollAccountState().finally(schedule), 1000);
      }
    };
    void pollAccountState().finally(schedule);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  const marketProjection = useMemo<MarketCanvasProjection>(() => ({
    source: MARKET_ASSET.source,
    asset: MARKET_ASSET,
    timeframe,
    candles: activeCandles,
    lastTick,
    cursor,
    viewportOffset,
    openTrades,
    chartEdits,
    highlightedChartEditId
  }), [activeCandles, chartEdits, cursor, highlightedChartEditId, lastTick, openTrades, timeframe, viewportOffset]);

  const selectTimeframe = useCallback((frame: TradingTimeframe) => {
    setTimeframe(frame);
    setViewportOffset(0);
  }, []);

  const scheduleCursor = useCallback((next: ChartCursor | null) => {
    pendingCursorRef.current = next;
    if (cursorFrameRef.current !== 0) return;
    cursorFrameRef.current = window.requestAnimationFrame(() => {
      cursorFrameRef.current = 0;
      setCursor(pendingCursorRef.current);
    });
  }, []);

  const scheduleViewportOffset = useCallback((next: number) => {
    pendingViewportOffsetRef.current = normalizeViewportOffset(next, activeCandles.length);
    if (viewportFrameRef.current !== 0) return;
    viewportFrameRef.current = window.requestAnimationFrame(() => {
      viewportFrameRef.current = 0;
      const pending = pendingViewportOffsetRef.current;
      setViewportOffset((current) => current === pending ? current : pending);
    });
  }, [activeCandles.length]);

  useEffect(() => {
    pendingViewportOffsetRef.current = normalizeViewportOffset(viewportOffset, activeCandles.length);
    if (pendingViewportOffsetRef.current !== viewportOffset) {
      setViewportOffset(pendingViewportOffsetRef.current);
    }
  }, [activeCandles.length, viewportOffset]);

  useEffect(() => () => {
    if (cursorFrameRef.current !== 0) window.cancelAnimationFrame(cursorFrameRef.current);
    if (viewportFrameRef.current !== 0) window.cancelAnimationFrame(viewportFrameRef.current);
  }, []);

  const candleStepForWidth = useCallback((width: number) => {
    const visibleCount = Math.max(1, Math.min(CANDLE_LIMIT_FOR_PAINT, activeCandles.length));
    const plotWidth = Math.max(80, width - 62);
    return plotWidth / visibleCount;
  }, [activeCandles.length]);

  const handleCanvasPointerMove = useCallback((event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => {
    const rect = canvas.getBoundingClientRect();
    scheduleCursor({ x: event.clientX - rect.left, y: event.clientY - rect.top });
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    event.preventDefault();
    const deltaCandles = (drag.startX - event.clientX) / Math.max(1, candleStepForWidth(rect.width));
    userNavigatedViewportRef.current = true;
    scheduleViewportOffset(drag.startOffset + deltaCandles);
  }, [candleStepForWidth, scheduleCursor, scheduleViewportOffset]);

  const handleCanvasWheel = useCallback((event: globalThis.WheelEvent, canvas: HTMLCanvasElement) => {
    if (activeCandles.length <= 1) return;
    const horizontal = Math.abs(event.deltaX) >= Math.abs(event.deltaY) ? event.deltaX : event.shiftKey ? event.deltaY : 0;
    if (Math.abs(horizontal) < 0.25) return;
    event.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const deltaCandles = -horizontal / Math.max(1, candleStepForWidth(rect.width));
    userNavigatedViewportRef.current = true;
    scheduleViewportOffset(pendingViewportOffsetRef.current + deltaCandles);
  }, [activeCandles.length, candleStepForWidth, scheduleViewportOffset]);
  const handleCanvasContextMenu = useCallback((event: globalThis.MouseEvent) => {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY });
  }, []);

  const resetChartViewport = useCallback(() => {
    userNavigatedViewportRef.current = false;
    pendingViewportOffsetRef.current = 0;
    setViewportOffset(0);
    setContextMenu(null);
  }, []);
  const handleCanvasPointerDown = useCallback((event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => {
    if (event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();
    setContextMenu(null);
    if (!canvas.hasPointerCapture(event.pointerId)) {
      canvas.setPointerCapture(event.pointerId);
    }
    dragRef.current = { pointerId: event.pointerId, startX: event.clientX, startOffset: pendingViewportOffsetRef.current };
  }, []);

  const handleCanvasPointerUp = useCallback((event: globalThis.PointerEvent, canvas: HTMLCanvasElement) => {
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null;
    }
  }, []);

  const handleCanvasPointerLeave = useCallback(() => {
    if (!dragRef.current) scheduleCursor(null);
  }, [scheduleCursor]);

  useEffect(() => {
    const api = globalThis.window?.forgeShell?.updateTradingChartWindowSnapshot;
    if (!api) return;
    const visible = projectedVisibleCandles(activeCandles, viewportOffset);
    const publish = async () => {
      const base = {
        schema: "forge.trading.chart_window_snapshot.v1" as const,
        source: "renderer_trading_chart" as const,
        instrument: MARKET_ASSET.instrument,
        displayName: MARKET_ASSET.displayName,
        timeframe,
        availableTimeframes: TIMEFRAMES,
        loadedCandleCount: activeCandles.length,
        visibleCandleCount: visible.length,
        firstLoadedTime: activeCandles[0]?.time,
        lastLoadedTime: activeCandles.at(-1)?.time,
        firstVisibleTime: visible[0]?.time,
        lastVisibleTime: visible.at(-1)?.time,
        pricePrecision: MARKET_ASSET.pricePrecision,
        dataSource: MARKET_ASSET.source.cacheNamespace as "oanda_rest_v20",
        chartUpdatedAt: new Date().toISOString()
      };
      const snapshot: TradingChartWindowSnapshot = { ...base, proofHash: await chartSnapshotProof(base) };
      await api(snapshot).catch(() => undefined);
    };
    void publish();
  }, [activeCandles, timeframe, viewportOffset]);

  useEffect(() => {
    const api = globalThis.window?.forgeShell;
    if (!api?.onTradingChartRequestEvent || !api.completeTradingChartRequest) return undefined;
    const dispose = api.onTradingChartRequestEvent((event: TradingChartRequestEvent) => {
      void (async () => {
        const capturedAt = new Date().toISOString();
        const currentCandles = candlesByTimeframe[event.timeframe] ?? [];
        const visible = latestChartCandles(currentCandles, event.timeframe === timeframe ? viewportOffset : 0, event.candleCount);
        const baseSnapshot = {
          schema: "forge.trading.chart_window_snapshot.v1" as const,
          source: "renderer_trading_chart" as const,
          instrument: MARKET_ASSET.instrument,
          displayName: MARKET_ASSET.displayName,
          timeframe: event.timeframe,
          availableTimeframes: TIMEFRAMES,
          loadedCandleCount: currentCandles.length,
          visibleCandleCount: visible.length,
          firstLoadedTime: currentCandles[0]?.time,
          lastLoadedTime: currentCandles.at(-1)?.time,
          firstVisibleTime: visible[0]?.time,
          lastVisibleTime: visible.at(-1)?.time,
          pricePrecision: MARKET_ASSET.pricePrecision,
          dataSource: MARKET_ASSET.source.cacheNamespace as "oanda_rest_v20",
          chartUpdatedAt: capturedAt
        };
        const chartWindowSnapshot: TradingChartWindowSnapshot = { ...baseSnapshot, proofHash: await chartSnapshotProof(baseSnapshot) };
        const requestInstrument = normalizeTradingInstrument(event.instrument);
        const wrongInstrument = Boolean(event.instrument && requestInstrument !== MARKET_ASSET.instrument);
        const unsupportedTimeframe = !TIMEFRAMES.includes(event.timeframe);
        const missingCandles = event.includeOhlc && visible.length === 0;
        const screenshot = event.includeScreenshot && !wrongInstrument && !unsupportedTimeframe ? captureTradingChartScreenshot() : null;
        const screenshotHash = screenshot ? await sha256Hex(screenshot.dataUrl) : undefined;
        const error = wrongInstrument
          ? tradingIpcError("chart_instrument_not_loaded", `Chart renderer has ${MARKET_ASSET.instrument}, not ${event.instrument}.`)
          : unsupportedTimeframe
            ? tradingIpcError("chart_timeframe_not_loaded", `Chart renderer has no loaded ${event.timeframe} data.`)
            : missingCandles
              ? tradingIpcError("chart_candles_not_loaded", `Chart renderer has no loaded candles for ${event.timeframe}.`)
              : event.includeScreenshot && !screenshot
                ? tradingIpcError("chart_screenshot_failed", "Chart renderer could not compose a screenshot from the visible canvases.")
                : undefined;
        if (error) {
          error.proofHash = await sha256Hex(stableJson({ code: error.code, message: error.message, capturedAt }));
        }
        const accepted = !error;
        const baseResult = {
          accepted,
          schema: "forge.trading.chart.result.v1" as const,
          source: "renderer_trading_chart" as const,
          instrument: MARKET_ASSET.instrument,
          displayName: MARKET_ASSET.displayName,
          timeframe: event.timeframe,
          candleCount: accepted && event.includeOhlc ? visible.length : 0,
          candles: accepted && event.includeOhlc ? visible : [],
          screenshotPngDataUrl: accepted ? screenshot?.dataUrl : undefined,
          screenshotWidth: accepted ? screenshot?.width : undefined,
          screenshotHeight: accepted ? screenshot?.height : undefined,
          screenshotHash,
          firstCandleTime: accepted ? visible[0]?.time : undefined,
          lastCandleTime: accepted ? visible.at(-1)?.time : undefined,
          chartWindowSnapshot,
          capturedAt,
          error
        };
        const result: TradingChartResult & { requestId: string } = { ...baseResult, requestId: event.requestId, proofHash: await chartResultProof(baseResult) };
        await api.completeTradingChartRequest?.(result).catch(() => undefined);
      })();
    });
    return dispose;
  }, [candlesByTimeframe, timeframe, viewportOffset]);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ tag?: string; id?: string }>).detail ?? {};
      const lookup = detail.id || detail.tag || "";
      if (!lookup) return;
      const match = chartEdits.find((element) => element.id === lookup || chartEditMatchesTag(element, lookup));
      if (!match) return;
      setHighlightedChartEditId(match.id);
      window.setTimeout(() => setHighlightedChartEditId((current) => current === match.id ? null : current), 1800);
    };
    window.addEventListener("forge:trading-chart-highlight", handler);
    return () => window.removeEventListener("forge:trading-chart-highlight", handler);
  }, [chartEdits]);

  useEffect(() => {
    const api = globalThis.window?.forgeShell;
    if (!api?.onTradingChartEditRequestEvent || !api.completeTradingChartEditRequest) return undefined;
    const dispose = api.onTradingChartEditRequestEvent((event: TradingChartEditRequestEvent) => {
      void (async () => {
        const capturedAt = new Date().toISOString();
        const currentCandles = candlesByTimeframe[event.timeframe] ?? [];
        const visible = latestChartCandles(currentCandles, event.timeframe === timeframe ? viewportOffset : 0, 80);
        const baseSnapshot = {
          schema: "forge.trading.chart_window_snapshot.v1" as const,
          source: "renderer_trading_chart" as const,
          instrument: MARKET_ASSET.instrument,
          displayName: MARKET_ASSET.displayName,
          timeframe: event.timeframe,
          availableTimeframes: TIMEFRAMES,
          loadedCandleCount: currentCandles.length,
          visibleCandleCount: visible.length,
          firstLoadedTime: currentCandles[0]?.time,
          lastLoadedTime: currentCandles.at(-1)?.time,
          firstVisibleTime: visible[0]?.time,
          lastVisibleTime: visible.at(-1)?.time,
          pricePrecision: MARKET_ASSET.pricePrecision,
          dataSource: MARKET_ASSET.source.cacheNamespace as "oanda_rest_v20",
          chartUpdatedAt: capturedAt
        };
        const chartWindowSnapshot: TradingChartWindowSnapshot = { ...baseSnapshot, proofHash: await chartSnapshotProof(baseSnapshot) };
        const requestInstrument = normalizeTradingInstrument(event.instrument);
        const wrongInstrument = Boolean(event.instrument && requestInstrument !== MARKET_ASSET.instrument);
        const unsupportedTimeframe = !TIMEFRAMES.includes(event.timeframe);
        const normalizedActions = event.actions.map((action) => normalizeChartEditActionForCandles(action, currentCandles, event.timeframe));
        const clearRequested = normalizedActions.some((action) => action.kind === "clear");
        const created = (await Promise.all(normalizedActions.map((action, index) => chartEditElementFromAction(action, event, index)))).filter((element): element is TradingChartEditElement => Boolean(element));
        const error = wrongInstrument
          ? tradingIpcError("edit_chart_instrument_not_loaded", `Chart renderer has ${MARKET_ASSET.instrument}, not ${event.instrument}.`)
          : unsupportedTimeframe
            ? tradingIpcError("edit_chart_timeframe_not_loaded", `Chart renderer has no loaded ${event.timeframe} data.`)
            : event.actions.length === 0
              ? tradingIpcError("edit_chart_empty_actions", "No chart edit actions were provided.")
              : undefined;
        if (error) {
          error.proofHash = await sha256Hex(stableJson({ code: error.code, message: error.message, capturedAt }));
        }
        const accepted = !error;
        if (accepted) {
          setChartEdits((current) => {
            const kept = clearRequested ? [] : current.filter((element) => element.timeframe !== event.timeframe || element.instrument !== MARKET_ASSET.instrument || !created.some((next) => next.id === element.id));
            return [...kept, ...created];
          });
          const last = created.at(-1);
          if (last) {
            setHighlightedChartEditId(last.id);
            window.setTimeout(() => setHighlightedChartEditId((current) => current === last.id ? null : current), 1800);
          }
        }
        const baseResult = {
          accepted,
          schema: "forge.trading.edit_chart.result.v1" as const,
          source: "renderer_trading_chart" as const,
          instrument: MARKET_ASSET.instrument,
          timeframe: event.timeframe,
          appliedCount: accepted ? event.actions.filter((action) => action.kind === "clear").length + created.length : 0,
          refusedCount: accepted ? 0 : event.actions.length,
          elements: accepted ? created : [],
          conversationTags: accepted ? created.map((element) => ({ tag: element.tag, elementId: element.id, label: element.label })) : [],
          chartWindowSnapshot,
          capturedAt,
          error
        };
        const result: TradingChartEditResult & { requestId: string } = { ...baseResult, requestId: event.requestId, proofHash: await chartEditResultProof(baseResult) };
        await api.completeTradingChartEditRequest?.(result).catch(() => undefined);
      })();
    });
    return dispose;
  }, [candlesByTimeframe, chartEdits, timeframe, viewportOffset]);
  const lastCandle = activeCandles.at(-1) ?? null;
  const subtitle = useMemo(() => {
    const state = importState[timeframe] ?? "pending";
    const last = lastCandle?.time ? new Date(lastCandle.time).toLocaleString() : "no candle";
    return `${state} / ${last}`;
  }, [importState, lastCandle?.time, timeframe]);

  return (
    <section className={`tradingCanvas tradingCanvas--count${chartCount}`} aria-label={`${MARKET_ASSET.displayName} candlestick chart`}>
      <div className="tradingCanvas__timeframes" role="tablist" aria-label="Trading timeframe">
        {TIMEFRAMES.map((frame) => (
          <button
            key={frame}
            type="button"
            role="tab"
            aria-selected={timeframe === frame}
            className={timeframe === frame ? "tradingCanvas__timeframe tradingCanvas__timeframe--active" : "tradingCanvas__timeframe"}
            onClick={() => selectTimeframe(frame)}
          >
            {frame}
          </button>
        ))}
      </div>
      {chartCount > 1 ? (
        <div className="tradingCanvas__parallelHeaders" aria-hidden="true">
          {Array.from({ length: chartCount }, (_value, index) => (
            <div className="tradingCanvas__parallelHeaderSlot" key={`trading-header-${index}`}>
              {index === 0 ? null : (
                <div className="tradingCanvas__parallelHeader">
                  <img className="tradingCanvas__parallelLogo" src="/shell-assets/oanda-logo.png" alt="" />
                  <span>{MARKET_ASSET.displayName}</span>
                  <strong>{timeframe}</strong>
                </div>
              )}
            </div>
          ))}
        </div>
      ) : null}
      <div className="tradingCanvas__grid">
        {Array.from({ length: chartCount }, (_value, index) => (
          <div className="tradingCanvas__pane" key={`trading-pane-${index}`}>
            <MarketCanvasSurface projection={marketProjection} onCursorMove={handleCanvasPointerMove} onCursorLeave={handleCanvasPointerLeave} onPointerDown={handleCanvasPointerDown} onPointerUp={handleCanvasPointerUp} onWheel={handleCanvasWheel} onContextMenu={handleCanvasContextMenu} />
          </div>
        ))}
      </div>
      {contextMenu ? (
        <div className="tradingCanvas__contextMenu" style={{ left: contextMenu.x, top: contextMenu.y }} role="menu">
          <button type="button" role="menuitem" onClick={resetChartViewport}>R&eacute;initialiser le graphique</button>
        </div>
      ) : null}
      <span className={activeCandles.length === 0 ? "tradingCanvas__status tradingCanvas__status--visible" : "tradingCanvas__status"} aria-live="polite">{status} {subtitle}</span>
    </section>
  );
}
