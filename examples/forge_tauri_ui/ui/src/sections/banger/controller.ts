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

export interface ForgeBangerRuntimeBridgeDeps {
  invoke?(command: string, args?: Record<string, unknown>, options?: Record<string, unknown>): Promise<unknown> | null;
  setGpuStatus?(label: string, tone: string): void;
  onRuntimeStatus?(status: Record<string, unknown> | null): void;
  setRuntimeDataset?(patch: { ready?: boolean; programs?: number; caches?: number }): void;
  log?(stage: string, payload?: unknown): void;
}

export interface ForgeBangerRuntimeBridge {
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown | null>;
  applyBackendStatus(status: Record<string, unknown> | null): void;
  applyRuntimeStatus(status: Record<string, unknown> | null): void;
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

export function createForgeBangerRuntimeBridge(deps: ForgeBangerRuntimeBridgeDeps = {}): ForgeBangerRuntimeBridge {
  const invoke = async (command: string, args: Record<string, unknown> = {}): Promise<unknown | null> => {
    const runtimeInvoke = deps.invoke;
    if (!runtimeInvoke) return null;
    try {
      return await runtimeInvoke(command, args, { section: "banger" });
    } catch (err) {
      console.warn(`[banger] backend ${command} failed:`, err);
      return null;
    }
  };

  const applyBackendStatus = (status: Record<string, unknown> | null): void => {
    if (!status || !deps.setGpuStatus) return;
    if (status.state === "active") {
      const tag = status.backend ? `${status.backend}` : "active";
      const name = status.adapter_name ? ` · ${status.adapter_name}` : "";
      deps.setGpuStatus(`GPU ${tag}${name}`, "active");
    } else if (status.state === "stopped") {
      deps.setGpuStatus("GPU paused", "paused");
    } else {
      deps.setGpuStatus("GPU idle", "paused");
    }
  };

  const applyRuntimeStatus = (status: Record<string, unknown> | null): void => {
    if (!status) return;
    deps.onRuntimeStatus?.(status);
    const warmed = !!status.backendReady && !!status.atlasAttached;
    const programs = Number(status.installedPrograms || 0);
    const cacheEntries = Number(status.runCacheEntries || 0) + Number(status.inspectCacheEntries || 0);
    if (deps.setGpuStatus) {
      if (warmed) deps.setGpuStatus(`KASM warm · atlas ready · ${programs} progs`, "active");
      else deps.setGpuStatus("KASM cold", "paused");
    }
    deps.setRuntimeDataset?.({
      ready: warmed,
      programs,
      caches: cacheEntries,
    });
    deps.log?.("runtime status", status);
  };

  return Object.freeze({
    invoke,
    applyBackendStatus,
    applyRuntimeStatus,
  });
}

declare global {
  interface Window {
    ForgeBangerController?: {
      create(deps: ForgeBangerControllerDeps): ForgeBangerController;
    };
    ForgeBangerRuntimeBridge?: {
      create(deps: ForgeBangerRuntimeBridgeDeps): ForgeBangerRuntimeBridge;
    };
  }
}

window.ForgeBangerController = Object.freeze({
  create: createForgeBangerController,
});

window.ForgeBangerRuntimeBridge = Object.freeze({
  create: createForgeBangerRuntimeBridge,
});
