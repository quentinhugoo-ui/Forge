import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

type TauriEvent = { readonly payload?: unknown };

type BridgeOptions = {
  readonly bootSafe?: boolean;
  readonly dedupeKey?: string;
  readonly requiresActiveSection?: boolean;
  readonly section?: string;
  readonly timeoutMs?: number;
  readonly trace?: boolean;
};

declare global {
  interface Window {
    readonly __FORGE_VERBOSE_LOGS?: boolean;
  }
}

const DEFAULT_TIMEOUT_MS = 15000;
const BOOT_TIMEOUT_MS = 5000;
const inflight = new Map<string, Promise<unknown>>();
const stats = {
  calls: 0,
  errors: 0,
  blocked: 0,
  timeouts: 0,
  lastError: "",
  lastCommand: "",
};

const bootBlockedCommands = new Set([
  "bloomberg_live_native_present",
  "bloomberg_live_native_prewarm",
  "webexplorer_native_present",
  "banger_engine_start",
]);

function registry() {
  return window.ForgeSectionRegistry || null;
}

function isAvailable(): boolean {
  return isTauri();
}

function serialize(details: unknown): string {
  try {
    return typeof details === "string" ? details : JSON.stringify(details || {});
  } catch (_) {
    return String(details || "");
  }
}

function verboseLogsEnabled(): boolean {
  try {
    if (window.__FORGE_VERBOSE_LOGS === true) return true;
    if (new URLSearchParams(window.location.search || "").has("forgeVerboseLogs")) return true;
    return window.localStorage?.getItem("forge.verboseLogs") === "true";
  } catch (_) {
    return false;
  }
}

function debugLog(stage: string, details: unknown = ""): Promise<boolean> {
  const payload = serialize(details);
  try {
    if (verboseLogsEnabled()) console.info(`[forge-bridge] ${stage}`, details);
  } catch (_) {}
  if (!isAvailable()) return Promise.resolve(false);
  return tauriInvoke("alpha_debug_log", {
    stage: `bridge.${stage}`,
    details: payload,
  }).then(() => true).catch(() => false);
}

function timeoutPromise(command: string, timeoutMs: number): Promise<never> {
  return new Promise((_, reject) => {
    window.setTimeout(() => reject(new Error(`${command} timed out after ${timeoutMs}ms`)), timeoutMs);
  });
}

function assertAllowed(command: string, options: BridgeOptions = {}): void {
  const section = String(options.section || "legacy").trim().toLowerCase();
  const reg = registry();
  if (options.requiresActiveSection && reg && !reg.isActive(section)) {
    const message = `blocked ${command}: section '${section}' is inactive`;
    stats.blocked += 1;
    stats.lastError = message;
    void debugLog("invoke.blocked", { command, section, reason: "inactive-section" });
    throw new Error(message);
  }
  if (reg?.phase?.() === "boot" && bootBlockedCommands.has(command) && options.bootSafe !== true) {
    const message = `blocked ${command}: not boot-safe`;
    stats.blocked += 1;
    stats.lastError = message;
    void debugLog("invoke.blocked", { command, section, reason: "not-boot-safe" });
    throw new Error(message);
  }
}

async function invoke<T = unknown>(command: string, args: Record<string, unknown> = {}, options: BridgeOptions = {}): Promise<T> {
  if (!isAvailable()) throw new Error("Tauri invoke bridge unavailable");
  assertAllowed(command, options);
  const timeoutMs = Math.max(1, Number(options.timeoutMs || (options.bootSafe ? BOOT_TIMEOUT_MS : DEFAULT_TIMEOUT_MS)));
  const dedupeKey = options.dedupeKey ? `${command}:${options.dedupeKey}` : "";
  if (dedupeKey && inflight.has(dedupeKey)) return inflight.get(dedupeKey) as Promise<T>;
  stats.calls += 1;
  stats.lastCommand = command;
  const startedAt = performance.now ? performance.now() : 0;
  const task = Promise.race([
    tauriInvoke<T>(command, args),
    timeoutPromise(command, timeoutMs),
  ]).then((result) => {
    if (options.trace === true) {
      void debugLog("invoke.ok", {
        command,
        section: options.section || "legacy",
        elapsedMs: startedAt ? Math.round((performance.now() - startedAt) * 10) / 10 : 0,
      });
    }
    return result;
  }).catch((err) => {
    const message = err?.message || String(err);
    stats.errors += 1;
    if (/timed out/i.test(message)) stats.timeouts += 1;
    stats.lastError = message;
    void debugLog("invoke.error", { command, section: options.section || "legacy", message });
    throw err;
  }).finally(() => {
    if (dedupeKey && inflight.get(dedupeKey) === task) inflight.delete(dedupeKey);
  });
  if (dedupeKey) inflight.set(dedupeKey, task);
  return task;
}

function listen(eventName: string, handler: (event: TauriEvent) => void, options: BridgeOptions = {}): Promise<unknown> {
  if (!isAvailable()) return Promise.resolve(null);
  const name = String(eventName || "").trim();
  if (!name) return Promise.resolve(null);
  stats.calls += 1;
  stats.lastCommand = `event:${name}`;
  return tauriListen(name, (event) => {
    try {
      handler(event);
    } catch (err) {
      stats.errors += 1;
      stats.lastError = err instanceof Error ? err.message : String(err);
      if (options.trace === true) void debugLog("event.handler.error", { eventName: name, message: stats.lastError });
      throw err;
    }
  }).catch((err) => {
    stats.errors += 1;
    stats.lastError = err instanceof Error ? err.message : String(err);
    void debugLog("event.listen.error", { eventName: name, message: stats.lastError });
    return null;
  });
}

function getStats(): Record<string, unknown> {
  return { ...stats, inflight: inflight.size };
}

window.ForgeTauriBridge = Object.freeze({
  invoke,
  listen,
  debugLog,
  isAvailable,
  getStats,
  bootBlockedCommands: () => Array.from(bootBlockedCommands),
});

export {};
