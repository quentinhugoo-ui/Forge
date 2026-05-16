declare global {
  interface Window {
    readonly __FORGE_VERBOSE_LOGS?: boolean;
    ForgeBoot?: {
      trace(stage: string, details?: unknown): void;
      installGlobalErrorHandlers(): void;
      suppressNativeTitleTooltips(): void;
    };
  }
}

function bridge() {
  return window.ForgeTauriBridge || null;
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

function traceIsUseful(stage: string): boolean {
  if (verboseLogsEnabled()) return true;
  return /(\.error|\.failed|\.blocked|\.timeout|js\.error|unhandledrejection)/i.test(String(stage || ""));
}

function trace(stage: string, details: unknown = ""): void {
  try {
    if (!traceIsUseful(stage)) return;
    const payload = typeof details === "string" ? details : JSON.stringify(details);
    if (verboseLogsEnabled()) console.info(`[forge-boot] ${stage}`, details);
    const logger = bridge()?.debugLog;
    if (logger) void logger(`boot.${stage}`, payload);
  } catch (_) {}
}

function installGlobalErrorHandlers(): void {
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
      reason: typeof reason === "string" ? reason : (reason?.message || String(reason || "unknown rejection")),
    });
  });
}

function stripNativeTitleTooltips(root: Node | Document): void {
  if (!root) return;
  if (root instanceof Element && root.hasAttribute("title")) root.removeAttribute("title");
  if (!(root instanceof Element || root instanceof Document || root instanceof DocumentFragment)) return;
  root.querySelectorAll?.("[title]")?.forEach((node) => node.removeAttribute("title"));
}

function suppressNativeTitleTooltips(): void {
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

export {};
