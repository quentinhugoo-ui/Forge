import type { ForgeShellActionHandler, ForgeShellActionName } from "../../shell/shell-actions.js";
import "./catalog.js";

type Runtime = {
  registerAction(name: ForgeShellActionName, handler: ForgeShellActionHandler): ForgeShellActionName;
};

export interface ForgeBangerControllerDeps {
  readonly runtime?: Runtime | null;
  readonly button?: HTMLElement | null;
  isVisible(): boolean;
  isBlocked(): boolean;
  open(): void;
  close(): void;
  syncButton(active: boolean): void;
}

export interface ForgeBangerController {
  open(): void;
  close(): void;
  toggle(): void;
  publishActive(active: boolean): void;
  syncButton(): void;
}

export function createForgeBangerController(deps: ForgeBangerControllerDeps): ForgeBangerController {
  const publishActive = (active: boolean): void => {
    window.ForgeShellRuntime?.dispatch({ type: "SET_SURFACE_ACTIVE", section: "banger", active, fallbackSection: "alpha" });
  };

  const syncButton = (): void => {
    deps.syncButton(deps.isVisible());
  };

  const open = (): void => {
    if (deps.isBlocked() || deps.isVisible()) return;
    deps.open();
    syncButton();
  };

  const close = (): void => {
    if (!deps.isVisible()) return;
    deps.close();
    syncButton();
  };

  const toggle = (): void => {
    if (deps.isVisible()) close();
    else open();
  };

  const controller = Object.freeze({
    open,
    close,
    toggle,
    publishActive,
    syncButton,
  });

  deps.runtime?.registerAction("toggle-banger", () => controller.toggle());
  deps.runtime?.registerAction("close-banger", () => controller.close());
  return controller;
}

declare global {
  interface Window {
    ForgeBangerController?: {
      create(deps: ForgeBangerControllerDeps): ForgeBangerController;
    };
  }
}

window.ForgeBangerController = Object.freeze({
  create: createForgeBangerController,
});
