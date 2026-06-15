import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";

type TooltipPlacement = "left" | "right";

interface TooltipState {
  text: string;
  placement: TooltipPlacement;
  x: number;
  y: number;
  maxWidth: number;
}

const TOOLTIP_GAP = 9;
const TOOLTIP_ESTIMATED_WIDTH = 180;
const TOOLTIP_EDGE_MARGIN = 10;
const TOOLTIP_ID = "ingen-global-tooltip";

function tooltipTextFor(element: HTMLElement): string {
  return element.getAttribute("data-tooltip")?.trim() ?? "";
}

function tooltipTargetFrom(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) {
    return null;
  }
  const element = target.closest<HTMLElement>("[data-tooltip]");
  if (!element || element.closest("[data-tooltip-disabled='true']")) {
    return null;
  }
  return tooltipTextFor(element) ? element : null;
}

function tooltipPlacementFor(rect: DOMRect): TooltipPlacement {
  if (rect.left >= TOOLTIP_ESTIMATED_WIDTH + TOOLTIP_GAP + TOOLTIP_EDGE_MARGIN) {
    return "left";
  }
  return "right";
}

function tooltipStateFor(element: HTMLElement, textOverride?: string): TooltipState | null {
  const text = textOverride ?? tooltipTextFor(element);
  if (!text) {
    return null;
  }
  const rect = element.getBoundingClientRect();
  const placement = tooltipPlacementFor(rect);
  if (placement === "left") {
    return {
      text,
      placement,
      x: rect.left - TOOLTIP_GAP,
      y: rect.top + rect.height / 2,
      maxWidth: Math.max(96, rect.left - TOOLTIP_GAP - TOOLTIP_EDGE_MARGIN)
    };
  }
  return {
    text,
    placement,
    x: rect.right + TOOLTIP_GAP,
    y: rect.top + rect.height / 2,
    maxWidth: Math.max(96, window.innerWidth - rect.right - TOOLTIP_GAP - TOOLTIP_EDGE_MARGIN)
  };
}

export function GlobalTooltip() {
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const activeElementRef = useRef<HTMLElement | null>(null);
  const activeNativeTitleRef = useRef<string | null>(null);
  const activeDescribedByRef = useRef<string | null | undefined>(undefined);

  const restoreActiveAttributes = useCallback(() => {
    const active = activeElementRef.current;
    if (active && activeNativeTitleRef.current !== null) {
      active.setAttribute("title", activeNativeTitleRef.current);
    }
    if (active) {
      if (activeDescribedByRef.current !== undefined && activeDescribedByRef.current !== null) {
        active.setAttribute("aria-describedby", activeDescribedByRef.current);
      } else {
        active.removeAttribute("aria-describedby");
      }
    }
    activeElementRef.current = null;
    activeNativeTitleRef.current = null;
    activeDescribedByRef.current = undefined;
  }, []);

  const showFor = useCallback(
    (element: HTMLElement | null) => {
      if (!element) {
        restoreActiveAttributes();
        setTooltip(null);
        return;
      }
      if (activeElementRef.current !== element) {
        restoreActiveAttributes();
      }
      const text = tooltipTextFor(element);
      const title = element.getAttribute("title");
      if (title !== null) {
        activeNativeTitleRef.current = title;
        element.removeAttribute("title");
      }
      if (activeDescribedByRef.current === undefined) {
        activeDescribedByRef.current = element.getAttribute("aria-describedby");
        element.setAttribute("aria-describedby", TOOLTIP_ID);
      }
      activeElementRef.current = element;
      setTooltip(tooltipStateFor(element, text));
    },
    [restoreActiveAttributes]
  );

  useEffect(() => {
    const showFromEvent = (event: Event) => showFor(tooltipTargetFrom(event.target));
    const hideFromEvent = (event: Event) => {
      const active = activeElementRef.current;
      if (!active) {
        setTooltip(null);
        return;
      }
      const nextTarget = "relatedTarget" in event ? event.relatedTarget : null;
      if (nextTarget instanceof Node && active.contains(nextTarget)) {
        return;
      }
      restoreActiveAttributes();
      setTooltip(null);
    };
    const refresh = () => {
      const active = activeElementRef.current;
      if (!active) {
        return;
      }
      const text = active.getAttribute("data-tooltip")?.trim() || "";
      setTooltip(tooltipStateFor(active, text));
    };
    const dismissOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      restoreActiveAttributes();
      setTooltip(null);
    };

    document.addEventListener("pointerover", showFromEvent, true);
    document.addEventListener("pointerout", hideFromEvent, true);
    document.addEventListener("focusin", showFromEvent, true);
    document.addEventListener("focusout", hideFromEvent, true);
    document.addEventListener("keydown", dismissOnEscape, true);
    window.addEventListener("scroll", refresh, true);
    window.addEventListener("resize", refresh);
    return () => {
      document.removeEventListener("pointerover", showFromEvent, true);
      document.removeEventListener("pointerout", hideFromEvent, true);
      document.removeEventListener("focusin", showFromEvent, true);
      document.removeEventListener("focusout", hideFromEvent, true);
      document.removeEventListener("keydown", dismissOnEscape, true);
      window.removeEventListener("scroll", refresh, true);
      window.removeEventListener("resize", refresh);
      restoreActiveAttributes();
    };
  }, [restoreActiveAttributes, showFor]);

  if (!tooltip) {
    return null;
  }

  return (
    <div
      className={["globalTooltip", `globalTooltip--${tooltip.placement}`].join(" ")}
      id={TOOLTIP_ID}
      role="tooltip"
      style={
        {
          "--global-tooltip-x": `${Math.round(tooltip.x)}px`,
          "--global-tooltip-y": `${Math.round(tooltip.y)}px`,
          "--global-tooltip-max-width": `${Math.round(tooltip.maxWidth)}px`
        } as CSSProperties
      }
    >
      {tooltip.text}
    </div>
  );
}
