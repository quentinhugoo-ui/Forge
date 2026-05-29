import {
  createForgeBangerController,
  createForgeBangerRuntimeBridge,
} from "./controller.js";

// Banger surface — SDF migration placeholder.
//
// The triangle-mesh pipeline (BOOM import, STL/GLB/GLTF parsers, scene
// mesh renderer, FPS counters, axis gizmo) was removed when Banger
// migrated to signed distance fields per INGEN COMPUTE. This file keeps
// the section lifecycle alive (BOOM titlebar button, engine start/stop)
// while the WGSL raymarcher (INGEN COMPUTE §19.4) lands.

(function () {
  "use strict";

  const $ = (id: string): HTMLElement | null => document.getElementById(id);

  const boomBtn = $("bangerBoomBtn");
  const view = $("bangerView");
  const canvas = $("bangerCanvas") as HTMLCanvasElement | null;
  const exitBtn = $("bangerExitBtn");
  const statVerts = $("bangerStatVerts");
  const statFaces = $("bangerStatFaces");
  const statFps = $("bangerStatFps");

  if (!boomBtn || !view || !canvas) {
    console.warn("[banger] required DOM missing — section disabled");
    return;
  }

  type TauriInvoke = (
    cmd: string,
    args?: Record<string, unknown>,
    opts?: Record<string, unknown>,
  ) => Promise<unknown>;

  const resolveInvoke = (): TauriInvoke | null => {
    const shell = (window as unknown as { ForgeShellRuntime?: { tauri?: { invoke?: TauriInvoke } } }).ForgeShellRuntime;
    if (shell?.tauri?.invoke) return shell.tauri.invoke;
    const bridge = (window as unknown as { ForgeTauriBridge?: { invoke?: TauriInvoke } }).ForgeTauriBridge;
    return bridge?.invoke ?? null;
  };

  const runtimeBridge = createForgeBangerRuntimeBridge({
    invoke: (cmd, args, opts) => {
      const inv = resolveInvoke();
      if (!inv) return Promise.resolve(null);
      return inv(cmd, args && typeof args === "object" ? args : {}, opts || {});
    },
    log: (stage, payload) => console.info(`[banger] ${stage}`, payload),
  });

  const isVisible = (): boolean => view.hasAttribute("hidden") === false;
  const setHidden = (hidden: boolean): void => {
    if (hidden) view.setAttribute("hidden", "");
    else view.removeAttribute("hidden");
  };

  const syncButton = (active: boolean): void => {
    boomBtn.classList.toggle("is-active", active);
    boomBtn.setAttribute("aria-pressed", active ? "true" : "false");
  };

  const paintPlaceholder = (): void => {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(rect.width * dpr));
    const h = Math.max(1, Math.floor(rect.height * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = "#0a0d12";
    ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = "#566173";
    ctx.font = `${Math.round(12 * dpr)}px ui-monospace, SFMono-Regular, Menlo, monospace`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText("Banger SDF — raymarcher pending (INGEN COMPUTE §19.4)", w / 2, h / 2);
  };

  type ControllerRuntime = NonNullable<Parameters<typeof createForgeBangerController>[0]["runtime"]>;
  const controller = createForgeBangerController({
    runtime: (window.ForgeShellRuntime ?? null) as ControllerRuntime | null,
    button: boomBtn,
    isVisible,
    isBlocked: () => false,
    open: () => {
      setHidden(false);
      paintPlaceholder();
      void runtimeBridge
        .invoke("banger_engine_start")
        .then((status) => runtimeBridge.applyBackendStatus(status as Record<string, unknown> | null));
    },
    close: () => {
      setHidden(true);
      void runtimeBridge
        .invoke("banger_engine_stop")
        .then((status) => runtimeBridge.applyBackendStatus(status as Record<string, unknown> | null));
    },
    syncButton,
  });

  // Surface the lifecycle to the shell so other sections can react.
  const publishActive = (active: boolean): void => {
    controller.publishActive(active);
  };

  boomBtn.addEventListener("click", () => {
    const willOpen = !isVisible();
    controller.toggle();
    publishActive(willOpen);
  });
  exitBtn?.addEventListener("click", () => {
    controller.close();
    publishActive(false);
  });

  // The mesh-era stat counters no longer have a triangle pipeline behind
  // them; show em-dashes until the SDF raymarcher reports its own metrics
  // (steps per pixel, fragment cost, hit/miss ratio).
  if (statVerts) statVerts.textContent = "—";
  if (statFaces) statFaces.textContent = "—";
  if (statFps) statFps.textContent = "—";

  window.addEventListener("resize", () => {
    if (isVisible()) paintPlaceholder();
  });

  controller.syncButton();
})();
