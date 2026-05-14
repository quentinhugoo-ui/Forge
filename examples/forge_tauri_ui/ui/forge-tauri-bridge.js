(function () {
  "use strict";

  const DEFAULT_TIMEOUT_MS = 15000;
  const BOOT_TIMEOUT_MS = 5000;
  const inflight = new Map();
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

  function rawInvoke() {
    return window.__TAURI__?.core?.invoke || null;
  }

  function registry() {
    return window.ForgeSectionRegistry || null;
  }

  function isAvailable() {
    return typeof rawInvoke() === "function";
  }

  function serialize(details) {
    try {
      return typeof details === "string" ? details : JSON.stringify(details || {});
    } catch (_) {
      return String(details || "");
    }
  }

  function debugLog(stage, details = "") {
    const invoke = rawInvoke();
    const payload = serialize(details);
    try {
      console.info(`[forge-bridge] ${stage}`, details);
    } catch (_) {}
    if (!invoke) return Promise.resolve(false);
    return invoke("alpha_debug_log", {
      stage: `bridge.${stage}`,
      details: payload,
    }).then(() => true).catch(() => false);
  }

  function timeoutPromise(command, timeoutMs) {
    return new Promise((_, reject) => {
      window.setTimeout(() => reject(new Error(`${command} timed out after ${timeoutMs}ms`)), timeoutMs);
    });
  }

  function assertAllowed(command, options = {}) {
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

  async function invoke(command, args = {}, options = {}) {
    const tauriInvoke = rawInvoke();
    if (!tauriInvoke) throw new Error("Tauri invoke bridge unavailable");
    assertAllowed(command, options);
    const timeoutMs = Math.max(1, Number(options.timeoutMs || (options.bootSafe ? BOOT_TIMEOUT_MS : DEFAULT_TIMEOUT_MS)));
    const dedupeKey = options.dedupeKey ? `${command}:${options.dedupeKey}` : "";
    if (dedupeKey && inflight.has(dedupeKey)) return inflight.get(dedupeKey);
    stats.calls += 1;
    stats.lastCommand = command;
    const startedAt = performance.now ? performance.now() : 0;
    const task = Promise.race([
      tauriInvoke(command, args),
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
      void debugLog("invoke.error", {
        command,
        section: options.section || "legacy",
        message,
      });
      throw err;
    }).finally(() => {
      if (dedupeKey && inflight.get(dedupeKey) === task) inflight.delete(dedupeKey);
    });
    if (dedupeKey) inflight.set(dedupeKey, task);
    return task;
  }

  function getStats() {
    return { ...stats, inflight: inflight.size };
  }

  window.ForgeTauriBridge = Object.freeze({
    invoke,
    debugLog,
    isAvailable,
    getStats,
    bootBlockedCommands: () => Array.from(bootBlockedCommands),
  });
})();
