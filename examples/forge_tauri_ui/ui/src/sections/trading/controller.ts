import type { ForgeShellActionHandler, ForgeShellActionName } from "../../shell/shell-actions.js";
import "./catalog.js";
import {
  normalizeTradingChatSubbarMode,
  normalizeTradingSubbarSection,
  tradingActivatePatch,
  tradingDeactivatePatch,
  type TradingActivatePatch,
  type TradingDeactivatePatch,
  type TradingSubbarMode,
  type TradingSubbarSection,
} from "./state.js";

type Runtime = {
  registerAction(name: ForgeShellActionName, handler: ForgeShellActionHandler): ForgeShellActionName;
};

export interface ForgeTradingControllerDeps {
  readonly runtime?: Runtime | null;
  readonly button?: HTMLElement | null;
  isActive(): boolean;
  isBlocked(): boolean;
  isWebExplorerActive(): boolean;
  activate(): void;
  deactivate(): void;
  focus(): void;
  trace?(stage: string, details?: Record<string, unknown>): void;
}

export interface ForgeTradingController {
  open(): void;
  toggle(): void;
  close(): void;
  focus(): void;
  publishActive(active: boolean): void;
  syncButton(): void;
}

export function createForgeTradingController(deps: ForgeTradingControllerDeps): ForgeTradingController {
  const publishActive = (active: boolean): void => {
    window.ForgeShellRuntime?.dispatch({ type: "SET_SURFACE_ACTIVE", section: "trading", active, fallbackSection: "alpha" });
  };

  const syncButton = (): void => {
    const button = deps.button;
    if (!button) return;
    const active = deps.isActive();
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-pressed", active ? "true" : "false");
    button.setAttribute("title", active ? "Close Trading workspace" : "Trading workspace");
    if (!active) requestAnimationFrame(() => button.blur());
  };

  const focus = (): void => {
    deps.focus();
    publishActive(true);
    syncButton();
  };

  const close = (): void => {
    deps.deactivate();
    publishActive(false);
    syncButton();
  };

  const open = (): void => {
    if (deps.isBlocked()) return;
    deps.trace?.("trading.workspace.open", { active: deps.isActive() });
    if (deps.isActive()) {
      focus();
      return;
    }
    deps.activate();
    publishActive(true);
    syncButton();
  };

  const toggle = (): void => {
    if (!deps.isActive()) {
      open();
      return;
    }
    if (deps.isWebExplorerActive()) {
      focus();
      return;
    }
    close();
  };

  const controller = Object.freeze({
    open,
    toggle,
    close,
    focus,
    publishActive,
    syncButton,
  });

  deps.runtime?.registerAction("open-trading", () => controller.open());
  return controller;
}

declare global {
  interface Window {
    ForgeTradingController?: {
      create(deps: ForgeTradingControllerDeps): ForgeTradingController;
    };
    ForgeTradingState?: {
      activatePatch(): TradingActivatePatch;
      deactivatePatch(): TradingDeactivatePatch;
      normalizeChatSubbarMode(mode: unknown): TradingSubbarMode;
      normalizeSubbarSection(section: unknown): TradingSubbarSection;
    };
  }
}

window.ForgeTradingController = Object.freeze({
  create: createForgeTradingController,
});

window.ForgeTradingState = Object.freeze({
  activatePatch: tradingActivatePatch,
  deactivatePatch: tradingDeactivatePatch,
  normalizeChatSubbarMode: normalizeTradingChatSubbarMode,
  normalizeSubbarSection: normalizeTradingSubbarSection,
});
