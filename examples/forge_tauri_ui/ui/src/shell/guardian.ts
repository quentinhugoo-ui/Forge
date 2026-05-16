const REAL_ESTATE_MODE_STORAGE_KEY = "forge.realEstate.mode.v1";
const FALLBACK_DELAY_MS = 180;

declare global {
  interface Window {
    __forgeSetRealEstateModeActive?: (active: boolean) => void;
    __forgeOpenWebExplorer?: () => void;
    __forgeCloseWebExplorer?: () => void;
    __forgeOpenSearch?: () => void;
    __forgeSetSidebarCollapsed?: (collapsed: boolean) => void;
    ForgeShellGuardian?: { install(): void };
  }
}

function buttonFromEvent(event: Event, selector: string): Element | null {
  const target = event.target;
  return target instanceof Element ? target.closest(selector) : null;
}

function boolAttr(node: Element | null, name: string): boolean {
  return node?.getAttribute?.(name) === "true";
}

function log(stage: string, details: Record<string, unknown> = {}): void {
  try {
    void window.ForgeTauriBridge?.debugLog?.(`shell_guardian.${stage}`, details);
  } catch (_) {}
}

function fallbackRealEstate(beforePressed: boolean): void {
  const button = document.getElementById("realEstateModeBtn");
  if (!button || boolAttr(button, "aria-pressed") !== beforePressed) return;
  const next = !beforePressed;
  log("real_estate.fallback", { next });
  if (typeof window.__forgeSetRealEstateModeActive === "function") {
    window.__forgeSetRealEstateModeActive(next);
    return;
  }
  try {
    window.localStorage?.setItem?.(REAL_ESTATE_MODE_STORAGE_KEY, next ? "1" : "0");
  } catch (_) {}
  document.body.classList.toggle("real-estate-mode", next);
  button.classList.toggle("is-active", next);
  button.setAttribute("aria-pressed", next ? "true" : "false");
}

function fallbackWebExplorer(beforePressed: boolean): void {
  const button = document.getElementById("webexplorer");
  if (!button || boolAttr(button, "aria-pressed") !== beforePressed) return;
  const next = !beforePressed;
  log("webexplorer.fallback", { next });
  if (next && typeof window.__forgeOpenWebExplorer === "function") window.__forgeOpenWebExplorer();
  else if (!next && typeof window.__forgeCloseWebExplorer === "function") window.__forgeCloseWebExplorer();
  else {
    button.classList.toggle("is-active", next);
    button.setAttribute("aria-pressed", next ? "true" : "false");
    document.body.classList.toggle("webexplorer-surface", next);
  }
}

function fallbackSearch(wasOpen: boolean): void {
  const overlay = document.getElementById("forgeSearchOverlay");
  const nowOpen = !!overlay && overlay.hidden === false;
  if (nowOpen !== wasOpen) return;
  log("search.fallback");
  if (typeof window.__forgeOpenSearch === "function") window.__forgeOpenSearch();
}

function fallbackSidebar(beforeCollapsed: boolean): void {
  const body = document.body;
  if (!body || body.classList.contains("sidebar-collapsed") !== beforeCollapsed) return;
  log("sidebar.fallback");
  if (typeof window.__forgeSetSidebarCollapsed === "function") window.__forgeSetSidebarCollapsed(!beforeCollapsed);
  else body.classList.toggle("sidebar-collapsed", !beforeCollapsed);
}

function installHeaderFallbackGuard(): void {
  document.addEventListener("click", (event) => {
    const realEstate = buttonFromEvent(event, "#realEstateModeBtn");
    if (realEstate) {
      const before = boolAttr(realEstate, "aria-pressed");
      window.setTimeout(() => fallbackRealEstate(before), FALLBACK_DELAY_MS);
      return;
    }
    const webExplorer = buttonFromEvent(event, "#webexplorer");
    if (webExplorer) {
      const before = boolAttr(webExplorer, "aria-pressed");
      window.setTimeout(() => fallbackWebExplorer(before), FALLBACK_DELAY_MS);
      return;
    }
    const search = buttonFromEvent(event, "#forgeSearchBtn");
    if (search) {
      const overlay = document.getElementById("forgeSearchOverlay");
      const wasOpen = !!overlay && overlay.hidden === false;
      window.setTimeout(() => fallbackSearch(wasOpen), FALLBACK_DELAY_MS);
      return;
    }
    const sidebar = buttonFromEvent(event, "#alphaSidebarToggle");
    if (sidebar) {
      const before = document.body?.classList.contains("sidebar-collapsed") || false;
      window.setTimeout(() => fallbackSidebar(before), FALLBACK_DELAY_MS);
    }
  }, true);
}

function install(): void {
  if (document.documentElement?.dataset.forgeShellGuardianInstalled === "true") return;
  document.documentElement.dataset.forgeShellGuardianInstalled = "true";
  installHeaderFallbackGuard();
  window.addEventListener("error", (event) => {
    log("runtime.error", { message: event.message || "", source: event.filename || "", line: event.lineno || 0 });
  });
  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    log("runtime.rejection", { reason: String(reason?.message || reason || "") });
  });
}

window.ForgeShellGuardian = Object.freeze({ install });
if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", install, { once: true });
else install();

export {};
