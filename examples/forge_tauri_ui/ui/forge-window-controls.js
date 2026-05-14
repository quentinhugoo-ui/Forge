(function () {
  "use strict";

  const runtimeUrl = new URL(window.location.href);
  const surfaceMode = runtimeUrl.searchParams.get("surface") || "";
  const windowLabel = runtimeUrl.searchParams.get("window")
    || (surfaceMode === "webexplorer" ? "webexplorer" : "main");
  const WINDOW_COMMAND_TIMEOUT_MS = 900;
  let lastCommand = "";
  let lastAt = 0;

  function bridge() {
    return window.ForgeTauriBridge || null;
  }

  function trace(stage, details = "") {
    try {
      const payload = typeof details === "string" ? details : JSON.stringify(details);
      console.info(`[forge-window] ${stage}`, details);
      if (bridge()?.debugLog) {
        void bridge().debugLog(`window.${stage}`, payload);
        return;
      }
      const invoke = window.__TAURI__?.core?.invoke;
      if (!invoke) return;
      void invoke("alpha_debug_log", { stage: `window.${stage}`, details: payload }).catch(() => {});
    } catch (_) {}
  }

  async function invokeWindowCommand(command, payload = {}, timeoutMs = WINDOW_COMMAND_TIMEOUT_MS) {
    const invoke = window.__TAURI__?.core?.invoke;
    if (!invoke && !bridge()?.isAvailable?.()) return false;
    try {
      const args = { ...payload, label: payload?.label || windowLabel };
      if (bridge()?.invoke) {
        await bridge().invoke(command, args, {
          section: "shell",
          bootSafe: true,
          timeoutMs,
          trace: true,
        });
      } else {
        await Promise.race([
          invoke(command, args),
          new Promise((_, reject) => window.setTimeout(() => reject(new Error("window command timeout")), timeoutMs)),
        ]);
      }
      return true;
    } catch (err) {
      trace("command.error", {
        command,
        label: payload?.label || windowLabel,
        message: err?.message || String(err),
      });
      console.warn(`window command failed: ${command}`, err);
      return false;
    }
  }

  function fallbackTauriWindowControl(command) {
    const currentWindow =
      window.__TAURI__?.window?.getCurrentWindow?.()
      || window.__TAURI__?.window?.appWindow
      || null;
    if (!currentWindow) return false;
    const track = (result) => {
      if (result && typeof result.catch === "function") {
        result.catch((err) => {
          trace("fallback.error", {
            command,
            message: err?.message || String(err),
          });
        });
      }
    };
    try {
      if (command === "minimize_main_window" && typeof currentWindow.minimize === "function") {
        track(currentWindow.minimize());
        return true;
      }
      if (command === "toggle_maximize_main_window") {
        if (typeof currentWindow.toggleMaximize === "function") {
          track(currentWindow.toggleMaximize());
          return true;
        }
        if (typeof currentWindow.maximize === "function") {
          track(currentWindow.maximize());
          return true;
        }
      }
      if (command === "close_main_window" && typeof currentWindow.close === "function") {
        track(currentWindow.close());
        return true;
      }
      if (command === "drag_main_window" && typeof currentWindow.startDragging === "function") {
        track(currentWindow.startDragging());
        return true;
      }
    } catch (err) {
      console.warn(`window fallback failed: ${command}`, err);
    }
    return false;
  }

  function runMainWindowControl(command) {
    trace("control.request", { command, label: windowLabel });
    void invokeWindowCommand(command, {}, 2500).then((ok) => {
      if (!ok) fallbackTauriWindowControl(command);
    });
  }

  function runMainWindowControlOnce(command) {
    const now = performance.now();
    if (command === lastCommand && now - lastAt < 180) return;
    lastCommand = command;
    lastAt = now;
    runMainWindowControl(command);
  }

  function commandForWindowControlButton(button) {
    if (!button) return "";
    if (button.id === "windowMinimize") return "minimize_main_window";
    if (button.id === "windowMaximize") return "toggle_maximize_main_window";
    if (button.id === "windowClose") return "close_main_window";
    return "";
  }

  function bindMainWindowControl(button, command) {
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

  function install() {
    if (document.documentElement?.dataset.forgeWindowControlsInstalled === "true") return;
    document.documentElement.dataset.forgeWindowControlsInstalled = "true";

    const titlebar = document.querySelector(".window-titlebar");
    const minimize = document.getElementById("windowMinimize");
    const maximize = document.getElementById("windowMaximize");
    const close = document.getElementById("windowClose");

    titlebar?.addEventListener("mousedown", (event) => {
      if (event.button !== 0) return;
      if (event.target?.closest?.(".window-btn, .sidebar-toggle, .titlebar-web-btn")) return;
      if (fallbackTauriWindowControl("drag_main_window")) return;
      void invokeWindowCommand("drag_main_window", {}, 700);
    });

    bindMainWindowControl(minimize, "minimize_main_window");
    bindMainWindowControl(maximize, "toggle_maximize_main_window");
    bindMainWindowControl(close, "close_main_window");

    document.addEventListener("click", (event) => {
      const button = event.target?.closest?.("#windowMinimize, #windowMaximize, #windowClose");
      if (!button) return;
      const command = commandForWindowControlButton(button);
      if (!command) return;
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation?.();
      runMainWindowControlOnce(command);
    }, true);

    document.addEventListener("pointerdown", (event) => {
      const button = event.target?.closest?.("#windowMinimize, #windowMaximize, #windowClose");
      if (!button) return;
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
})();
