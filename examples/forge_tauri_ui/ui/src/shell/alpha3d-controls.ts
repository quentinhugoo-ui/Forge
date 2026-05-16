type Alpha3dControlsState = any;

type Alpha3dControlsOptions = {
  readonly canvas: HTMLCanvasElement | null;
  readonly state: Alpha3dControlsState;
  readonly pickPointAt: (clientX: number, clientY: number) => void;
  readonly scheduleRender: () => void;
};

const AUTO_ROTATE_SPEED = (2 * Math.PI) / 240;
const AUTO_ROTATE_RESUME_DELAY = 1500;

export function createAlpha3dControls(options: Alpha3dControlsOptions) {
  let autoRotateLastT = 0;
  let dragReleasedAt = 0;
  let autoRotateRaf = 0;
  let bound = false;

  const autoRotateLoop = (t: number) => {
    autoRotateRaf = 0;
    if (!options.state.open) {
      autoRotateLastT = 0;
      return;
    }
    if (autoRotateLastT === 0) autoRotateLastT = t;
    const dt = (t - autoRotateLastT) / 1000;
    autoRotateLastT = t;
    const idleEnough =
      !options.state.drag && (t - dragReleasedAt) > AUTO_ROTATE_RESUME_DELAY;
    if (idleEnough && dt > 0 && dt < 0.5) {
      options.state.camera.yaw += dt * AUTO_ROTATE_SPEED;
      options.scheduleRender();
    }
    autoRotateRaf = requestAnimationFrame(autoRotateLoop);
  };

  const startAutoRotate = () => {
    if (autoRotateRaf) return;
    autoRotateLastT = 0;
    autoRotateRaf = requestAnimationFrame(autoRotateLoop);
  };

  const stopAutoRotate = () => {
    if (autoRotateRaf) cancelAnimationFrame(autoRotateRaf);
    autoRotateRaf = 0;
    autoRotateLastT = 0;
  };

  const bind = () => {
    if (bound || !options.canvas) return;
    bound = true;
    options.canvas.addEventListener("mousedown", (event) => {
      if (event.button !== 0) return;
      options.state.drag = {
        x: event.clientX,
        y: event.clientY,
        startX: event.clientX,
        startY: event.clientY,
        moved: false,
      };
    });
    window.addEventListener("mousemove", (event) => {
      if (!options.state.drag) return;
      const dx = event.clientX - options.state.drag.x;
      const dy = event.clientY - options.state.drag.y;
      if (
        Math.hypot(
          event.clientX - options.state.drag.startX,
          event.clientY - options.state.drag.startY,
        ) > 4
      ) {
        options.state.drag.moved = true;
      }
      options.state.drag.x = event.clientX;
      options.state.drag.y = event.clientY;
      options.state.camera.yaw += dx * 0.008;
      options.state.camera.pitch += dy * 0.008;
      options.state.camera.pitch = Math.max(0.05, Math.min(1.45, options.state.camera.pitch));
      options.scheduleRender();
    });
    window.addEventListener("mouseup", (event) => {
      if (options.state.drag && !options.state.drag.moved && event.target === options.canvas) {
        options.pickPointAt(event.clientX, event.clientY);
      }
      if (options.state.drag) dragReleasedAt = performance.now();
      options.state.drag = null;
    });
    options.canvas.addEventListener(
      "wheel",
      (event) => {
        event.preventDefault();
        options.state.camera.dist *= 1 + event.deltaY * 0.0012;
        options.state.camera.dist = Math.max(0.6, Math.min(4.5, options.state.camera.dist));
        options.scheduleRender();
      },
      { passive: false },
    );
  };

  return { bind, startAutoRotate, stopAutoRotate };
}
