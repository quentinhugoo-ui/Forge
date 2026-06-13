import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { createPortal } from "react-dom";

// First-send animation: drain the wave field and dissolve the welcome copy as
// pixels. The composer and send button are intentionally not drawn as targets.

export interface ComposerSendBurstHandle {
  fire: (target: HTMLElement | null, onPeak: () => void) => void;
}

const BLOCK = 1.5;
const STEP = 2.5;
const FILL_MS = 3400;
const WAVE_DRAIN_EVENT = "ingen:wave-drain";

interface ColorSample {
  r: number;
  g: number;
  b: number;
}

interface WelcomePixel extends ColorSample {
  x: number;
  y: number;
  vanishAt: number;
}

interface WelcomeSource {
  pixels: WelcomePixel[];
  element: HTMLElement;
}

function easeOutBack(t: number): number {
  const c1 = 1.70158;
  const c3 = c1 + 1;
  return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2);
}

function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

function prefersReducedMotion(): boolean {
  return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function emitWaveDrain(level: number): void {
  window.dispatchEvent(new CustomEvent(WAVE_DRAIN_EVENT, { detail: { level } }));
}

function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function parseRgb(value: string): ColorSample {
  const match = value.match(/rgba?\(([^)]+)\)/i);
  if (!match) {
    return { r: 248, g: 250, b: 255 };
  }
  const parts = match[1].split(",").map((part) => Number.parseFloat(part.trim()));
  return {
    r: clampByte(parts[0] ?? 248),
    g: clampByte(parts[1] ?? 250),
    b: clampByte(parts[2] ?? 255)
  };
}

function rectVisible(rect: DOMRect): boolean {
  return rect.width > 2 && rect.height > 2 && rect.bottom > 0 && rect.right > 0 && rect.left < window.innerWidth && rect.top < window.innerHeight;
}

function findWelcomeText(): HTMLElement | null {
  let selected: HTMLElement | null = null;
  let selectedArea = 0;
  for (const element of Array.from(document.querySelectorAll<HTMLElement>(".profileCoverWelcomeText"))) {
    if (element.dataset.burstConsumed === "1") {
      continue;
    }
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    if (style.display === "none" || style.visibility === "hidden" || Number(style.opacity) <= 0 || !rectVisible(rect)) {
      continue;
    }
    const area = rect.width * rect.height;
    if (area > selectedArea) {
      selected = element;
      selectedArea = area;
    }
  }
  return selected;
}

function sampleWelcomePixels(step: number): WelcomeSource | null {
  const host = findWelcomeText();
  if (!host) {
    return null;
  }
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    return null;
  }
  const pixels: WelcomePixel[] = [];
  const pitch = Math.max(2, Math.round(step));
  for (const element of Array.from(host.querySelectorAll<HTMLElement>("strong, span"))) {
    const text = (element.textContent ?? "").trim();
    const rect = element.getBoundingClientRect();
    if (!text || rect.width < 2 || rect.height < 2) {
      continue;
    }
    const width = Math.ceil(rect.width);
    const height = Math.ceil(rect.height);
    canvas.width = width;
    canvas.height = height;
    const style = getComputedStyle(element);
    ctx.clearRect(0, 0, width, height);
    ctx.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    ctx.letterSpacing = style.letterSpacing === "normal" ? "0px" : style.letterSpacing;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "#ffffff";
    ctx.fillText(text, width / 2, height / 2);
    const data = ctx.getImageData(0, 0, width, height).data;
    const color = parseRgb(style.color);
    for (let y = 0; y < height; y += pitch) {
      for (let x = 0; x < width; x += pitch) {
        if (data[(y * width + x) * 4 + 3] > 96) {
          pixels.push({ x: rect.left + x, y: rect.top + y, r: color.r, g: color.g, b: color.b, vanishAt: FILL_MS });
        }
      }
    }
  }
  if (pixels.length < 12) {
    return null;
  }
  return { pixels, element: host };
}

function shuffledIndexes(count: number): Int32Array {
  const indexes = new Int32Array(count);
  for (let i = 0; i < count; i++) {
    indexes[i] = i;
  }
  for (let i = count - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    const tmp = indexes[i];
    indexes[i] = indexes[j];
    indexes[j] = tmp;
  }
  return indexes;
}

export const ComposerSendBurst = forwardRef<ComposerSendBurstHandle>(function ComposerSendBurst(_props, ref) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rafRef = useRef(0);

  useEffect(() => () => window.cancelAnimationFrame(rafRef.current), []);

  useImperativeHandle(ref, () => ({
    fire(target, onPeak) {
      const canvas = canvasRef.current;
      const targetRect = target?.getBoundingClientRect();
      if (!canvas || !target || !targetRect || targetRect.width <= 0 || targetRect.height <= 0 || prefersReducedMotion()) {
        onPeak();
        return;
      }
      window.cancelAnimationFrame(rafRef.current);
      const pixelRatio = Math.min(globalThis.devicePixelRatio || 1, 2);
      const width = window.innerWidth;
      const height = window.innerHeight;
      canvas.style.display = "block";
      canvas.style.left = "0px";
      canvas.style.top = "0px";
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      canvas.width = Math.round(width * pixelRatio);
      canvas.height = Math.round(height * pixelRatio);
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        onPeak();
        return;
      }
      ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      ctx.imageSmoothingEnabled = false;
      const welcomeSource = sampleWelcomePixels(STEP);
      if (welcomeSource) {
        welcomeSource.element.style.opacity = "0";
        welcomeSource.element.dataset.burstConsumed = "1";
      }
      const welcomePixels = welcomeSource?.pixels ?? [];
      const welcomeOrder = shuffledIndexes(welcomePixels.length);
      for (let i = 0; i < welcomeOrder.length; i++) {
        const pixelIndex = welcomeOrder[i];
        welcomePixels[pixelIndex].vanishAt = welcomeOrder.length > 1 ? (i / (welcomeOrder.length - 1)) * FILL_MS : 0;
      }
      let committed = false;
      const startedAt = performance.now();
      const clear = () => {
        window.cancelAnimationFrame(rafRef.current);
        ctx.clearRect(0, 0, width, height);
        canvas.style.display = "none";
      };
      const draw = (now: number) => {
        ctx.clearRect(0, 0, width, height);
        const elapsed = now - startedAt;
        const progress = Math.min(1, elapsed / FILL_MS);
        emitWaveDrain(progress);
        if (welcomeSource) {
          for (const pixel of welcomePixels) {
            if (elapsed >= pixel.vanishAt) {
              continue;
            }
            ctx.fillStyle = `rgb(${pixel.r},${pixel.g},${pixel.b})`;
            ctx.fillRect(pixel.x - BLOCK / 2, pixel.y - BLOCK / 2, BLOCK, BLOCK);
          }
        }
        if (elapsed >= FILL_MS + 200) {
          emitWaveDrain(1);
          if (!committed) {
            committed = true;
            onPeak();
          }
          clear();
          return;
        }
        rafRef.current = window.requestAnimationFrame(draw);
      };
      rafRef.current = window.requestAnimationFrame(draw);
    }
  }), []);

  return createPortal(
    <canvas ref={canvasRef} className="composerSendBurst" aria-hidden="true" style={{ display: "none" }} />,
    document.body
  );
});
