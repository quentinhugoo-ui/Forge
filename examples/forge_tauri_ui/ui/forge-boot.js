(function () {
  "use strict";

  function bridge() {
    return window.ForgeTauriBridge || null;
  }

  function verboseLogsEnabled() {
    try {
      if (window.__FORGE_VERBOSE_LOGS === true) return true;
      if (new URLSearchParams(window.location.search || "").has("forgeVerboseLogs")) return true;
      return window.localStorage?.getItem("forge.verboseLogs") === "true";
    } catch (_) {
      return false;
    }
  }

  function traceIsUseful(stage) {
    if (verboseLogsEnabled()) return true;
    return /(\.error|\.failed|\.blocked|\.timeout|js\.error|unhandledrejection)/i.test(String(stage || ""));
  }

  function trace(stage, details = "") {
    try {
      if (!traceIsUseful(stage)) return;
      const payload = typeof details === "string" ? details : JSON.stringify(details);
      if (verboseLogsEnabled()) console.info(`[forge-boot] ${stage}`, details);
      if (bridge()?.debugLog) {
        void bridge().debugLog(`boot.${stage}`, payload);
        return;
      }
      const invoke = window.__TAURI__?.core?.invoke;
      if (!invoke) return;
      void invoke("alpha_debug_log", { stage: `boot.${stage}`, details: payload }).catch(() => {});
    } catch (_) {}
  }

  function installGlobalErrorHandlers() {
    if (document.documentElement?.dataset.forgeBootErrorsInstalled === "true") return;
    document.documentElement.dataset.forgeBootErrorsInstalled = "true";

    window.addEventListener("error", (event) => {
      trace("js.error", {
        message: event?.message || "unknown error",
        filename: event?.filename || "",
        lineno: event?.lineno || 0,
        colno: event?.colno || 0,
      });
    });

    window.addEventListener("unhandledrejection", (event) => {
      const reason = event?.reason;
      trace("js.unhandledrejection", {
        reason: typeof reason === "string"
          ? reason
          : (reason?.message || String(reason || "unknown rejection")),
      });
    });
  }

  function stripNativeTitleTooltips(root) {
    if (!root) return;
    if (root instanceof Element && root.hasAttribute("title")) {
      root.removeAttribute("title");
    }
    if (!(root instanceof Element || root instanceof Document || root instanceof DocumentFragment)) return;
    root.querySelectorAll?.("[title]")?.forEach((node) => {
      node.removeAttribute("title");
    });
  }

  function suppressNativeTitleTooltips() {
    if (document.documentElement?.dataset.forgeTitleSuppressorInstalled === "true") return;
    document.documentElement.dataset.forgeTitleSuppressorInstalled = "true";
    stripNativeTitleTooltips(document);
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "title" && mutation.target instanceof Element) {
          mutation.target.removeAttribute("title");
          continue;
        }
        if (mutation.type !== "childList") continue;
        mutation.addedNodes.forEach((node) => stripNativeTitleTooltips(node));
      }
    });
    observer.observe(document.documentElement, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["title"],
    });
  }

  window.ForgeBoot = Object.freeze({
    trace,
    installGlobalErrorHandlers,
    suppressNativeTitleTooltips,
  });

  installGlobalErrorHandlers();
  suppressNativeTitleTooltips();
})();
