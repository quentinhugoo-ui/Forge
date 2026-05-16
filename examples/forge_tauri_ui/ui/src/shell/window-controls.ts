import { getCurrentWindow } from "@tauri-apps/api/window";

type TauriWindowApi = {
  readonly minimize?: () => Promise<unknown> | unknown;
  readonly toggleMaximize?: () => Promise<unknown> | unknown;
  readonly maximize?: () => Promise<unknown> | unknown;
  readonly close?: () => Promise<unknown> | unknown;
  readonly startDragging?: () => Promise<unknown> | unknown;
};

declare global {
  interface Window {
    ForgeWindowControls?: {
      install(): void;
      invoke(command: string, payload?: Record<string, unknown>, timeoutMs?: number): Promise<boolean>;
      fallback(command: string): boolean;
      run(command: string): void;
      label(): string;
    };
  }
}

const runtimeUrl = new URL(window.location.href);
const surfaceMode = runtimeUrl.searchParams.get("surface") || "";
const windowLabel = runtimeUrl.searchParams.get("window") || (surfaceMode === "webexplorer" ? "webexplorer" : "main");
const WINDOW_COMMAND_TIMEOUT_MS = 900;
let lastCommand = "";
let lastAt = 0;

function bridge() {
  return window.ForgeShellRuntime?.tauri || window.ForgeTauriBridge || null;
}

function bridgeDebugLog() {
  return window.ForgeTauriBridge?.debugLog || null;
}

function bridgeAvailable(): boolean {
  return Boolean(window.ForgeShellRuntime?.tauri?.invoke || window.ForgeTauriBridge?.invoke || window.ForgeTauriBridge?.isAvailable?.());
}

function trace(stage: string, details: unknown = ""): void {
  try {
    const payload = typeof details === "string" ? details : JSON.stringify(details);
    console.info(`[forge-window] ${stage}`, details);
    const logger = bridgeDebugLog();
    if (logger) void logger(`window.${stage}`, payload);
  } catch (_) {}
}

async function invokeWindowCommand(command: string, payload: Record<string, unknown> = {}, timeoutMs = WINDOW_COMMAND_TIMEOUT_MS): Promise<boolean> {
  const commandBridge = bridge();
  if (!bridgeAvailable()) return false;
  try {
    const args = { ...payload, label: String(payload?.label || windowLabel) };
    window.ForgeShellRuntime?.dispatch?.({ type: "SET_WINDOW_COMMAND", command, label: args.label });
    if (!commandBridge?.invoke) return false;
    await commandBridge.invoke(command, args, { section: "shell", bootSafe: true, timeoutMs, trace: true });
    return true;
  } catch (err) {
    trace("command.error", {
      command,
      label: payload?.label || windowLabel,
      message: err instanceof Error ? err.message : String(err),
    });
    console.warn(`window command failed: ${command}`, err);
    return false;
  }
}

function trackWindowResult(command: string, result: Promise<unknown> | unknown): void {
  if (result && typeof (result as Promise<unknown>).catch === "function") {
    (result as Promise<unknown>).catch((err) => {
      trace("fallback.error", { command, message: err instanceof Error ? err.message : String(err) });
    });
  }
}

function fallbackTauriWindowControl(command: string): boolean {
  const currentWindow = getCurrentWindow() as TauriWindowApi;
  if (!currentWindow) return false;
  try {
    if (command === "minimize_main_window" && typeof currentWindow.minimize === "function") {
      trackWindowResult(command, currentWindow.minimize());
      return true;
    }
    if (command === "toggle_maximize_main_window") {
      if (typeof currentWindow.toggleMaximize === "function") {
        trackWindowResult(command, currentWindow.toggleMaximize());
        return true;
      }
      if (typeof currentWindow.maximize === "function") {
        trackWindowResult(command, currentWindow.maximize());
        return true;
      }
    }
    if (command === "close_main_window" && typeof currentWindow.close === "function") {
      trackWindowResult(command, currentWindow.close());
      return true;
    }
    if (command === "drag_main_window" && typeof currentWindow.startDragging === "function") {
      trackWindowResult(command, currentWindow.startDragging());
      return true;
    }
  } catch (err) {
    console.warn(`window fallback failed: ${command}`, err);
  }
  return false;
}

function runMainWindowControl(command: string): void {
  trace("control.request", { command, label: windowLabel });
  void invokeWindowCommand(command, {}, 2500).then((ok) => {
    if (!ok) fallbackTauriWindowControl(command);
  });
}

function runMainWindowControlOnce(command: string): void {
  const now = performance.now();
  if (command === lastCommand && now - lastAt < 180) return;
  lastCommand = command;
  lastAt = now;
  runMainWindowControl(command);
}

function commandForWindowControlButton(button: Element | null): string {
  if (!button) return "";
  if (button.id === "windowMinimize") return "minimize_main_window";
  if (button.id === "windowMaximize") return "toggle_maximize_main_window";
  if (button.id === "windowClose") return "close_main_window";
  return "";
}

function bindMainWindowControl(button: HTMLElement | null, command: string): void {
  if (!button || button.dataset.windowControlBound === "true") return;
  button.dataset.windowControlBound = "true";
  button.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
    runMainWindowControlOnce(command);
  }, true);
  button.addEventListener("mousedown", (event) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
  }, true);
  button.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
    runMainWindowControlOnce(command);
  }, true);
  button.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
    runMainWindowControlOnce(command);
  }, true);
}

function install(): void {
  if (document.documentElement?.dataset.forgeWindowControlsInstalled === "true") return;
  document.documentElement.dataset.forgeWindowControlsInstalled = "true";

  const titlebar = document.querySelector(".window-titlebar");
  const minimize = document.getElementById("windowMinimize");
  const maximize = document.getElementById("windowMaximize");
  const close = document.getElementById("windowClose");

  titlebar?.addEventListener("mousedown", (event) => {
    const mouseEvent = event as MouseEvent;
    if (mouseEvent.button !== 0) return;
    if ((event.target as Element | null)?.closest?.(".window-btn, .sidebar-toggle, .titlebar-web-btn")) return;
    if (fallbackTauriWindowControl("drag_main_window")) return;
    void invokeWindowCommand("drag_main_window", {}, 700);
  });

  bindMainWindowControl(minimize, "minimize_main_window");
  bindMainWindowControl(maximize, "toggle_maximize_main_window");
  bindMainWindowControl(close, "close_main_window");

  document.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest?.("#windowMinimize, #windowMaximize, #windowClose") || null;
    const command = commandForWindowControlButton(button);
    if (!command) return;
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
    runMainWindowControlOnce(command);
  }, true);

  document.addEventListener("pointerdown", (event) => {
    const button = (event.target as Element | null)?.closest?.("#windowMinimize, #windowMaximize, #windowClose") || null;
    const command = commandForWindowControlButton(button);
    if (!command) return;
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
    runMainWindowControlOnce(command);
  }, true);
}

window.ForgeWindowControls = Object.freeze({
  install,
  invoke: invokeWindowCommand,
  fallback: fallbackTauriWindowControl,
  run: runMainWindowControl,
  label: () => windowLabel,
});

install();

export {};
