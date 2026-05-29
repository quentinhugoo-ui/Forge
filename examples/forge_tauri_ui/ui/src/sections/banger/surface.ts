import {
  createForgeBangerController,
  createForgeBangerRuntimeBridge,
} from "./controller.js";

// Banger surface — INGEN COMPUTE §19.4 raymarch driver.
//
// The UI starts/stops the wgpu BangerEngine on open/close and runs a
// requestAnimationFrame loop that asks the backend for an SDF frame
// (banger_sdf_frame), then blits the RGBA8 bytes onto the BOOM canvas
// via putImageData. No mesh, no triangle, no vertex buffer crosses the
// boundary — only the GPU's pixel output.

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

  // Cap the GPU render size so the readback round-trip stays fast even
  // on integrated graphics. The canvas is upscaled by the browser when
  // its CSS dimensions exceed the pixel buffer.
  const MAX_RENDER_DIM = 512;

  let rafId: number | null = null;
  let renderStartMs = 0;
  let frameInFlight = false;
  const fpsSamples: number[] = [];
  let lastFrameMs = 0;

  const base64ToBytes = (b64: string): Uint8Array => {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
    return out;
  };

  const renderTargetSize = (): { width: number; height: number } => {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    let w = Math.max(1, Math.floor(rect.width * dpr));
    let h = Math.max(1, Math.floor(rect.height * dpr));
    const scale = Math.min(1, MAX_RENDER_DIM / Math.max(w, h));
    w = Math.max(1, Math.floor(w * scale));
    h = Math.max(1, Math.floor(h * scale));
    return { width: w, height: h };
  };

  const updateFpsCounter = (now: number): void => {
    if (lastFrameMs) {
      const dt = now - lastFrameMs;
      if (dt > 0) {
        fpsSamples.push(1000 / dt);
        if (fpsSamples.length > 30) fpsSamples.shift();
        if (statFps) {
          const avg = fpsSamples.reduce((a, b) => a + b, 0) / fpsSamples.length;
          statFps.textContent = avg.toFixed(0);
        }
      }
    }
    lastFrameMs = now;
  };

  const paintFrame = async (): Promise<void> => {
    if (frameInFlight) return;
    if (!isVisible()) return;

    const { width: renderW, height: renderH } = renderTargetSize();
    if (canvas.width !== renderW) canvas.width = renderW;
    if (canvas.height !== renderH) canvas.height = renderH;
    const elapsedSec = (performance.now() - renderStartMs) / 1000;

    frameInFlight = true;
    let frame: { width?: number; height?: number; pixelsB64?: string } | null = null;
    try {
      frame = (await runtimeBridge.invoke("banger_sdf_frame", {
        request: { width: renderW, height: renderH, timeSeconds: elapsedSec },
      })) as { width?: number; height?: number; pixelsB64?: string } | null;
    } finally {
      frameInFlight = false;
    }

    if (frame?.pixelsB64 && frame.width && frame.height) {
      const bytes = base64ToBytes(frame.pixelsB64);
      const expected = frame.width * frame.height * 4;
      if (bytes.length === expected) {
        const ctx = canvas.getContext("2d");
        if (ctx) {
          const clamped = new Uint8ClampedArray(bytes.length);
          clamped.set(bytes);
          const img = new ImageData(clamped, frame.width, frame.height);
          ctx.putImageData(img, 0, 0);
          updateFpsCounter(performance.now());
        }
      } else {
        console.warn(`[banger] sdf frame size mismatch: got ${bytes.length}, expected ${expected}`);
      }
    }
  };

  const renderLoop = (): void => {
    rafId = null;
    if (!isVisible()) return;
    void paintFrame().finally(() => {
      if (isVisible()) {
        rafId = window.requestAnimationFrame(renderLoop);
      }
    });
  };

  const startRendering = (): void => {
    if (rafId !== null) return;
    renderStartMs = performance.now();
    lastFrameMs = 0;
    fpsSamples.length = 0;
    rafId = window.requestAnimationFrame(renderLoop);
  };

  const stopRendering = (): void => {
    if (rafId !== null) {
      window.cancelAnimationFrame(rafId);
      rafId = null;
    }
    if (statFps) statFps.textContent = "—";
  };

  type ControllerRuntime = NonNullable<Parameters<typeof createForgeBangerController>[0]["runtime"]>;
  const controller = createForgeBangerController({
    runtime: (window.ForgeShellRuntime ?? null) as ControllerRuntime | null,
    button: boomBtn,
    isVisible,
    isBlocked: () => false,
    open: () => {
      setHidden(false);
      void runtimeBridge
        .invoke("banger_engine_start")
        .then((status) => {
          runtimeBridge.applyBackendStatus(status as Record<string, unknown> | null);
          startRendering();
        });
    },
    close: () => {
      setHidden(true);
      stopRendering();
      void runtimeBridge
        .invoke("banger_engine_stop")
        .then((status) => runtimeBridge.applyBackendStatus(status as Record<string, unknown> | null));
    },
    syncButton,
  });

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

  // The legacy mesh stats no longer have a triangle pipeline behind
  // them. The FPS counter is repurposed by the SDF render loop above;
  // verts/faces stay as em-dashes until the SDF tree exposes its own
  // metrics (step count, hit ratio, fragment cost per pixel).
  if (statVerts) statVerts.textContent = "—";
  if (statFaces) statFaces.textContent = "—";
  if (statFps) statFps.textContent = "—";

  controller.syncButton();
})();
