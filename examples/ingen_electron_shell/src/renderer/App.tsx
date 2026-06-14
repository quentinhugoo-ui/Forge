import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BRAIN_AIRBNB_COMMAND,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_MAPS_COMMAND,
  BRAIN_WORKSPACE_COMMAND,
  type NativeWebExplorerCodeAct,
  type ComposerUploadPreview,
  type HeaderControl,
  type NativeSection
} from "../shared/ipc-contract";
import tokens from "../shared/generated/design-tokens.generated.json";
import { CanvasSurfacesSlice, type CanvasToolPane } from "./CanvasSurfacesSlice";
import { PanelsChatBottomSlice } from "./PanelsChatBottomSlice";
import { ProfileCoverBanner } from "./ProfileCoverBanner";
import { RightPanelSlice } from "./RightPanelSlice";
import { primaryAssistantGeoEntityLabel } from "./assistant-geo-entities";
import { readBrainAgentMemory, readBrainUserMemory } from "./brain-user-memory-store";
import { HeaderSurfaceRouter } from "./HeaderSurfaceRouter";
import { headerShadowStore, useHeaderShadowStore } from "./header-shadow-store";
import { headerSurfaceStore, useHeaderSurfaceStore } from "./header-surface-store";
import { panelsChatBottomStore, usePanelsChatBottomStore } from "./panels-chat-bottom-store";
import { SidebarSlice, type SidebarModuleId } from "./SidebarSlice";
import { sidebarShadowStore, useSidebarShadowStore } from "./sidebar-shadow-store";
import { selectWelcomeMessage } from "./welcome-message-store";

function cssTokenStyle(): React.CSSProperties {
  return Object.fromEntries(
    Object.entries(tokens.colors).map(([name, value]) => [`--forge-${name}`, value])
  ) as React.CSSProperties;
}

function iconClass(control: Pick<HeaderControl, "icon" | "id">): string {
  if (control.id === "plan") return "shellIcon--nav-plan";
  if (control.id === "webexplorer-workspace") return "shellIcon--nav-web";
  if (control.id === "right-panel") return "shellIcon--nav-panel";

  switch (control.icon) {
    case "panel-left":
      return "shellIcon--panel";
    case "search":
      return "shellIcon--search";
    case "globe":
      return "shellIcon--google";
    case "box":
      return "shellIcon--banger";
    case "chart":
      return "shellIcon--trading";
    default:
      return "shellIcon--nav-forge";
  }
}

function webExplorerCodeActModule(event: NativeWebExplorerCodeAct): SidebarModuleId | null {
  if (event.command === BRAIN_AIRBNB_COMMAND) return "airbnb";
  if (event.command === BRAIN_GMAIL_COMMAND || event.command === BRAIN_GMAIL_COM_COMMAND) return "gmail";
  return null;
}

function windowGlyphClass(command: HeaderControl["command"]): string {
  if (command === "window_minimize") return "windowGlyph windowGlyph--min";
  if (command === "window_toggle_maximize") return "windowGlyph windowGlyph--max";
  return "windowGlyph windowGlyph--close";
}

const WINDOW_CONTROLS = [
  { id: "window-minimize", label: "Minimize", command: "window_minimize" },
  { id: "window-maximize", label: "Maximize", command: "window_toggle_maximize" },
  { id: "window-close", label: "Close", command: "window_close" }
] as const;

const GOOGLE_EARTH_DOM_DEFAULT_URL =
  "https://earth.google.com/web/@48.56768844,29.71746065,-845.33787847a,4386237.90060282d,35y,64.15278862h,59.46514162t,0.00000084r/data=CgRCAggBOgMKATBCAggASg0I____________ARAA";
const WIDGET_SIDE_EXIT_MS = 420;
const WIDGET_HEADER_EXIT_MS = 360;
const WIDGET_CANVAS_EXIT_MS = 860;
const WIDGET_SURFACE_CLOSE_DELAY_MS = WIDGET_SIDE_EXIT_MS + WIDGET_HEADER_EXIT_MS + WIDGET_CANVAS_EXIT_MS;
const WIDGET_NATIVE_SHRINK_LEAD_MS = 80;
const WIDGET_NATIVE_SETTLE_MS = 260;
const WIDGET_HANDOFF_SETTLE_MS = WIDGET_NATIVE_SHRINK_LEAD_MS + WIDGET_NATIVE_SETTLE_MS;
const WIDGET_NATIVE_SHRINK_DELAY_MS = WIDGET_SURFACE_CLOSE_DELAY_MS + WIDGET_NATIVE_SHRINK_LEAD_MS;
const WIDGET_VISUAL_SETTLE_DELAY_MS = WIDGET_SURFACE_CLOSE_DELAY_MS + WIDGET_HANDOFF_SETTLE_MS;

type WidgetLayoutLock = {
  chatLeft: number;
  chatWidth: number;
  bottomLeft: number;
  bottomWidth: number;
};

type WidgetHitRegion = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type WidgetMinimizingPhase = "" | "sides" | "header" | "canvas";

const WIDGET_HIT_REGION_TARGETS = [
  { selector: ".composer", padding: 6 },
  { selector: ".widgetWindowsButton", padding: 1 },
  { selector: ".bottomControls button", padding: 1 },
  { selector: ".permissionModeControl", padding: 1 },
  { selector: ".composerQuestionnaire", padding: 1 },
  { selector: ".permissionModeMenu", padding: 1 }
] as const;

function readWidgetLayoutLock(): WidgetLayoutLock | null {
  const composerRect = document.querySelector(".composer")?.getBoundingClientRect();
  if (!composerRect) {
    return null;
  }
  const bottomControlsRect = document.querySelector(".bottomControls")?.getBoundingClientRect();
  const chatWidth = Math.round(composerRect.width);
  const bottomWidth = Math.round(bottomControlsRect?.width ?? composerRect.width);
  return {
    chatLeft: Math.round((window.innerWidth - chatWidth) / 2),
    chatWidth,
    bottomLeft: Math.round((window.innerWidth - bottomWidth) / 2),
    bottomWidth
  };
}

function widgetHitRegionForElement(element: Element, padding: number): WidgetHitRegion | null {
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return null;
  }
  const left = Math.max(0, Math.floor(rect.left - padding));
  const top = Math.max(0, Math.floor(rect.top - padding));
  const right = Math.min(window.innerWidth, Math.ceil(rect.right + padding));
  const bottom = Math.min(window.innerHeight, Math.ceil(rect.bottom + padding));
  if (right - left < 8 || bottom - top < 8) {
    return null;
  }
  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top
  };
}

function readWidgetHitRegions(): WidgetHitRegion[] {
  const regions: WidgetHitRegion[] = [];
  const seen = new Set<Element>();
  for (const target of WIDGET_HIT_REGION_TARGETS) {
    document.querySelectorAll(target.selector).forEach((element) => {
      if (seen.has(element)) {
        return;
      }
      seen.add(element);
      const region = widgetHitRegionForElement(element, target.padding);
      if (region) {
        regions.push(region);
      }
    });
  }
  return regions;
}

function shellStyleWithWidgetLock(lock: WidgetLayoutLock | null): React.CSSProperties {
  const style = cssTokenStyle();
  if (!lock) {
    return style;
  }
  return {
    ...style,
    "--widget-chat-left": `${lock.chatLeft}px`,
    "--widget-chat-width": `${lock.chatWidth}px`,
    "--widget-bottom-left": `${lock.bottomLeft}px`,
    "--widget-bottom-width": `${lock.bottomWidth}px`
  } as React.CSSProperties;
}

function waitForWidgetMotion(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function clearDocumentWidgetModeClasses(): void {
  document.documentElement.classList.remove("ingen-widget-mode");
  document.body.classList.remove("ingen-widget-mode");
}

function sectionGroup(section: NativeSection): string {
  if (section === "trading") {
    return "PAPERTRADING";
  }
  if (section === "real-estate") {
    return "Forge";
  }
  return "Choose workspace";
}

export function App() {
  const { snapshot } = useHeaderShadowStore();
  const { snapshot: headerSurfaceSnapshot } = useHeaderSurfaceStore();
  const { snapshot: sidebarSnapshot } = useSidebarShadowStore();
  const { snapshot: panelsChatSnapshot } = usePanelsChatBottomStore();
  const [canvasSplitOpen, setCanvasSplitOpen] = useState(false);
  const [canvasFilesOpen, setCanvasFilesOpen] = useState(false);
  const [canvasTerminalOpen, setCanvasTerminalOpen] = useState(false);
  const [canvasActivePane, setCanvasActivePane] = useState<CanvasToolPane | "">("");
  const [canvasPlanetsOpen, setCanvasPlanetsOpen] = useState(false);
  const [canvasWebExplorerOpen, setCanvasWebExplorerOpen] = useState(false);
  const [canvasMapsOpen, setCanvasMapsOpen] = useState(false);
  const [widgetMode, setWidgetMode] = useState(false);
  const [widgetModeTransitioning, setWidgetModeTransitioning] = useState(false);
  const [widgetMinimizingPhase, setWidgetMinimizingPhase] = useState<WidgetMinimizingPhase>("");
  const [widgetLayoutLock, setWidgetLayoutLock] = useState<WidgetLayoutLock | null>(null);
  const canvasMapsOpenRef = useRef(false);
  const [webExplorerParallelIndex, setWebExplorerParallelIndex] = useState(0);
  const [mapsParallelIndex, setMapsParallelIndex] = useState(0);
  const [mapsWebviewUrl, setMapsWebviewUrl] = useState(GOOGLE_EARTH_DOM_DEFAULT_URL);
  const [webExplorerModuleId, setWebExplorerModuleId] = useState<SidebarModuleId | null>(null);
  const [composerModuleId, setComposerModuleId] = useState<SidebarModuleId | null>(null);
  const [parallelSidebarBirth, setParallelSidebarBirth] = useState<{ sessionId: string; token: number } | null>(null);
  const toggleComposerModule = useCallback((id: SidebarModuleId) => {
    setComposerModuleId((current) => (current === id ? null : id));
  }, []);
  const dropComposerModule = useCallback((id: SidebarModuleId) => {
    setComposerModuleId(id);
  }, []);
  const [parallelPrompts, setParallelPrompts] = useState<string[]>([""]);
  const [brainUserMemory] = useState(() => readBrainUserMemory());
  const [brainAgentMemory] = useState(() => readBrainAgentMemory());
  const [welcomeMessage, setWelcomeMessage] = useState(() => selectWelcomeMessage(brainUserMemory.preferredFirstName));
  const [homeCanvasResetId, setHomeCanvasResetId] = useState(0);
  const [workspaceFolder, setWorkspaceFolder] = useState<string | null>(null);
  const [workspaceGateActive, setWorkspaceGateActive] = useState(false);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceNotice, setWorkspaceNotice] = useState<string | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const workspaceNoticeTimerRef = useRef<number | null>(null);
  const parallelSidebarBirthTimerRef = useRef<number | null>(null);
  const widgetSurfaceCloseTimerRef = useRef<number | null>(null);
  const widgetModeSequenceRef = useRef(0);
  const widgetModeTransitioningRef = useRef(false);
  const previousActiveSessionIdRef = useRef(panelsChatSnapshot.activeSessionId);
  const mapsOwnerSessionIdRef = useRef<string | null>(null);
  useEffect(() => {
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: brainUserMemory.preferredFirstName,
      agentFirstName: brainAgentMemory.preferredFirstName
    });
  }, [brainAgentMemory.preferredFirstName, brainUserMemory.preferredFirstName]);
  const chooseWorkspace = useCallback(async () => {
    const result = await globalThis.window?.forgeShell?.chooseWorkspaceFolder?.();
    if (result && !result.canceled && result.folderName) {
      setWorkspaceFolder(result.folderName);
      setWorkspaceGateActive(false);
    }
  }, []);
  useEffect(() => {
    let active = true;
    void globalThis.window?.forgeShell?.getWorkspaceFolder?.().then((result) => {
      if (active && result && !result.canceled && result.folderName) {
        setWorkspaceFolder(result.folderName);
        setWorkspaceGateActive(false);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  const activeProfileCanvas = sidebarSnapshot.profileCanvas || snapshot.profileCanvas;
  const isLlmProviderCanvas = activeProfileCanvas === "llm";
  const isBrainCanvas = activeProfileCanvas === "brain";
  // Brain behaves like LLM Provider: a full page replacing the whole canvas,
  // closable from the workspace-header cross.
  const isFullPageCanvas = isLlmProviderCanvas || isBrainCanvas;
  const isBangerPage = snapshot.activeSection === "banger" && !isFullPageCanvas;
  const renderPanelsChatBottom = globalThis.location?.port !== "5176" && !isFullPageCanvas;
  const canvasSurfaceOpen =
    canvasSplitOpen ||
    canvasFilesOpen ||
    canvasTerminalOpen ||
    canvasPlanetsOpen ||
    canvasWebExplorerOpen ||
    canvasMapsOpen ||
    parallelPrompts.length > 1;
  const latestAssistantText = useMemo(() => {
    return panelsChatSnapshot.transcript.filter((message) => message.role === "assistant").at(-1)?.text ?? "";
  }, [panelsChatSnapshot.transcript]);
  const latestAssistantGeoEntityLabel = useMemo(() => {
    return primaryAssistantGeoEntityLabel(latestAssistantText);
  }, [latestAssistantText]);
  const activeSessionName = useMemo(() => {
    const items = [...sidebarSnapshot.recentItems, ...sidebarSnapshot.archivedItems];
    if (!sidebarSnapshot.recentSessionId) {
      return "New session";
    }
    return items.find((item) => item.sessionId !== "" && item.sessionId === sidebarSnapshot.recentSessionId)?.label || "New session";
  }, [sidebarSnapshot.archivedItems, sidebarSnapshot.recentItems, sidebarSnapshot.recentSessionId]);
  useEffect(() => {
    if (canvasMapsOpen) {
      return;
    }
    mapsOwnerSessionIdRef.current = null;
    void globalThis.window?.forgeShell?.hideNativeMaps?.();
  }, [canvasMapsOpen]);
  useEffect(() => {
    if (!isFullPageCanvas || !canvasMapsOpen) {
      return;
    }
    mapsOwnerSessionIdRef.current = null;
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setMapsParallelIndex(0);
    void globalThis.window?.forgeShell?.hideNativeMaps?.();
  }, [canvasMapsOpen, isFullPageCanvas]);
  const sessionHasStarted = useMemo(
    () => panelsChatSnapshot.transcript.some((message) => message.role === "user" || message.role === "assistant"),
    [panelsChatSnapshot.transcript]
  );
  const restoredParallelPromptCount = useMemo(() => {
    if (panelsChatSnapshot.parallelLanes.length === 0) {
      return 1;
    }
    const maxLaneIndex = panelsChatSnapshot.parallelLanes.reduce((max, lane) => Math.max(max, lane.index), 0);
    return Math.min(4, Math.max(2, maxLaneIndex + 1));
  }, [panelsChatSnapshot.parallelLanes]);
  useEffect(() => {
    const previousActiveSessionId = previousActiveSessionIdRef.current;
    const activeSessionChanged = previousActiveSessionId !== panelsChatSnapshot.activeSessionId;
    previousActiveSessionIdRef.current = panelsChatSnapshot.activeSessionId;
    if (
      activeSessionChanged &&
      canvasMapsOpen &&
      mapsOwnerSessionIdRef.current !== (panelsChatSnapshot.activeSessionId || "draft")
    ) {
      mapsOwnerSessionIdRef.current = null;
      canvasMapsOpenRef.current = false;
      setCanvasMapsOpen(false);
      setMapsParallelIndex(0);
      void globalThis.window?.forgeShell?.hideNativeMaps?.();
    }

    if (restoredParallelPromptCount > 1 && (activeSessionChanged || parallelPrompts.length < restoredParallelPromptCount)) {
      setParallelPrompts((prompts) => Array.from({ length: restoredParallelPromptCount }, (_value, index) => prompts[index] ?? ""));
      return;
    }
    if (
      restoredParallelPromptCount === 1 &&
      activeSessionChanged &&
      panelsChatSnapshot.activeSessionId &&
      parallelPrompts.length > 1
    ) {
      setParallelPrompts([""]);
      setCanvasWebExplorerOpen(false);
      setWebExplorerParallelIndex(0);
    }
  }, [canvasMapsOpen, panelsChatSnapshot.activeSessionId, parallelPrompts.length, restoredParallelPromptCount]);
  const sessionFiles = useMemo<ComposerUploadPreview[]>(() => {
    const seen = new Set<string>();
    const files: ComposerUploadPreview[] = [];
    for (const message of panelsChatSnapshot.transcript) {
      for (const file of message.attachments ?? []) {
        if (!seen.has(file.id)) {
          seen.add(file.id);
          files.push(file);
        }
      }
    }
    return files;
  }, [panelsChatSnapshot.transcript]);
  const shellClassName = [
    "shell",
    snapshot.leftPanelOpen ? "shell--left-open" : "shell--left-collapsed",
    snapshot.rightPanelOpen ? "shell--right-open" : "shell--right-collapsed",
    canvasSurfaceOpen ? "shell--canvas-split" : "shell--canvas-single",
    canvasFilesOpen || canvasTerminalOpen ? "shell--canvas-files-open" : "",
    parallelPrompts.length > 1 ? "shell--parallel-canvas-open" : "",
    canvasWebExplorerOpen ? "shell--webexplorer-canvas-open" : "",
    canvasMapsOpen ? "shell--maps-canvas-open" : "",
    isLlmProviderCanvas ? "shell--llm-provider" : "",
    isBrainCanvas ? "shell--brain-canvas" : "",
    isBangerPage ? "shell--banger-page" : "",
    widgetMinimizingPhase !== "" ? "shell--widget-minimizing shell--widget-minimizing-sides" : "",
    widgetMinimizingPhase === "header" || widgetMinimizingPhase === "canvas" ? "shell--widget-minimizing-header" : "",
    widgetMinimizingPhase === "canvas" ? "shell--widget-minimizing-canvas" : "",
    widgetMode ? "shell--widget-mode" : "",
    workspaceGateActive ? "shell--workspace-required" : ""
  ].join(" ");

  useEffect(() => {
    const widgetSurfaceVisible = widgetMode || widgetMinimizingPhase !== "";
    document.documentElement.classList.toggle("ingen-widget-mode", widgetSurfaceVisible);
    document.body.classList.toggle("ingen-widget-mode", widgetSurfaceVisible);
    return () => {
      clearDocumentWidgetModeClasses();
    };
  }, [widgetMinimizingPhase, widgetMode]);

  useEffect(() => {
    const api = globalThis.window?.forgeWindowControls;
    if (!api?.setWidgetHitRegions) {
      return undefined;
    }
    const setWidgetHitRegions = api.setWidgetHitRegions.bind(api);
    const setWidgetClickThrough = api.setWidgetClickThrough?.bind(api);
    if (!widgetMode) {
      void setWidgetHitRegions([]);
      void setWidgetClickThrough?.(false);
      return undefined;
    }

    let animationFrame = 0;
    let armTimer = 0;
    const hitRegionsReadyAt = performance.now() + WIDGET_NATIVE_SHRINK_LEAD_MS + 120;
    void setWidgetClickThrough?.(false);
    void setWidgetHitRegions([]);
    const scheduleHitRegionSync = () => {
      const remainingDelay = hitRegionsReadyAt - performance.now();
      if (remainingDelay > 0) {
        if (armTimer === 0) {
          armTimer = window.setTimeout(() => {
            armTimer = 0;
            scheduleHitRegionSync();
          }, Math.ceil(remainingDelay));
        }
        return;
      }
      if (animationFrame !== 0) {
        return;
      }
      animationFrame = window.requestAnimationFrame(() => {
        animationFrame = 0;
        const regions = readWidgetHitRegions();
        void setWidgetHitRegions(regions);
      });
    };

    const settleTimers = [WIDGET_NATIVE_SHRINK_LEAD_MS + 120, WIDGET_HANDOFF_SETTLE_MS + 120].map((delay) =>
      window.setTimeout(scheduleHitRegionSync, delay)
    );
    const resizeObserver = new ResizeObserver(scheduleHitRegionSync);
    resizeObserver.observe(document.body);
    const mutationObserver = new MutationObserver(scheduleHitRegionSync);
    mutationObserver.observe(document.body, {
      attributeFilter: ["aria-expanded", "class", "style"],
      attributes: true,
      childList: true,
      subtree: true
    });
    window.addEventListener("resize", scheduleHitRegionSync);
    window.addEventListener("click", scheduleHitRegionSync, true);
    window.addEventListener("keyup", scheduleHitRegionSync, true);

    return () => {
      if (animationFrame !== 0) {
        window.cancelAnimationFrame(animationFrame);
      }
      if (armTimer !== 0) {
        window.clearTimeout(armTimer);
      }
      settleTimers.forEach((timer) => window.clearTimeout(timer));
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      window.removeEventListener("resize", scheduleHitRegionSync);
      window.removeEventListener("click", scheduleHitRegionSync, true);
      window.removeEventListener("keyup", scheduleHitRegionSync, true);
      void setWidgetHitRegions([]);
      void setWidgetClickThrough?.(false);
    };
  }, [widgetLayoutLock, widgetMode]);

  useEffect(() => {
    if (workspaceFolder) {
      setWorkspaceGateActive(false);
      return;
    }
    const latestAssistant = latestAssistantText ? { text: latestAssistantText } : undefined;
    if (latestAssistant?.text.includes(BRAIN_WORKSPACE_COMMAND)) {
      setWorkspaceGateActive(true);
    }
  }, [latestAssistantText, workspaceFolder]);

  useEffect(() => {
    let mounted = true;
    void sidebarShadowStore
      .dispatch(sidebarShadowStore.command({ kind: "navigate", section: "forge" }), "startup-new-session")
      .then(async () => {
        if (!mounted) {
          return;
        }
        await Promise.all([headerShadowStore.boot(), sidebarShadowStore.boot(), headerSurfaceStore.refresh()]);
      });
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    void headerSurfaceStore.refresh();
  }, [snapshot.activeSection, snapshot.profileCanvas]);

  useEffect(() => {
    if (!workspaceMenuOpen && workspaceNotice === null) {
      return;
    }
    const closeOnPointerDown = (event: PointerEvent) => {
      if (workspaceMenuRef.current?.contains(event.target as Node)) {
        return;
      }
      setWorkspaceMenuOpen(false);
      setWorkspaceNotice(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setWorkspaceMenuOpen(false);
        setWorkspaceNotice(null);
      }
    };
    window.addEventListener("pointerdown", closeOnPointerDown);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("pointerdown", closeOnPointerDown);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [workspaceMenuOpen, workspaceNotice]);

  useEffect(() => {
    return () => {
      if (workspaceNoticeTimerRef.current !== null) {
        window.clearTimeout(workspaceNoticeTimerRef.current);
      }
      if (parallelSidebarBirthTimerRef.current !== null) {
        window.clearTimeout(parallelSidebarBirthTimerRef.current);
      }
      if (widgetSurfaceCloseTimerRef.current !== null) {
        window.clearTimeout(widgetSurfaceCloseTimerRef.current);
      }
    };
  }, []);

  const runWorkspaceMenuAction = useCallback(async (action: "show" | "copyPath" | "copyBranch") => {
    setWorkspaceMenuOpen(false);
    setWorkspaceNotice(null);
    if (workspaceNoticeTimerRef.current !== null) {
      window.clearTimeout(workspaceNoticeTimerRef.current);
      workspaceNoticeTimerRef.current = null;
    }
    const noticeMessage = action === "copyPath" ? "Path copied" : action === "copyBranch" ? "Branch name copied" : null;
    if (noticeMessage !== null) {
      setWorkspaceNotice(noticeMessage);
      workspaceNoticeTimerRef.current = window.setTimeout(() => {
        setWorkspaceNotice(null);
        workspaceNoticeTimerRef.current = null;
      }, 2200);
    }
    const api = globalThis.window?.forgeShell;
    const result =
      action === "show"
        ? await api?.showWorkspaceInExplorer?.()
        : action === "copyPath"
          ? await api?.copyWorkspacePath?.()
          : await api?.copyWorkspaceBranchName?.();
    if (result?.accepted === false) {
      if (noticeMessage !== null) {
        setWorkspaceNotice(null);
      }
      console.warn("[workspace-menu] action rejected", result.error?.message ?? result);
    }
  }, []);

  useEffect(() => {
    if (!canvasSurfaceOpen) {
      if (canvasFilesOpen) {
        setCanvasFilesOpen(false);
      }
      if (canvasTerminalOpen) {
        setCanvasTerminalOpen(false);
      }
      if (canvasActivePane) {
        setCanvasActivePane("");
      }
      if (canvasPlanetsOpen) {
        setCanvasPlanetsOpen(false);
      }
      if (canvasWebExplorerOpen) {
        setCanvasWebExplorerOpen(false);
      }
      if (canvasMapsOpen) {
        canvasMapsOpenRef.current = false;
        setCanvasMapsOpen(false);
      }
      if (parallelPrompts.length > 1) {
        setParallelPrompts([""]);
      }
    }
  }, [canvasActivePane, canvasFilesOpen, canvasMapsOpen, canvasPlanetsOpen, canvasSurfaceOpen, canvasTerminalOpen, canvasWebExplorerOpen, parallelPrompts.length]);

  const triggerParallelSidebarBirth = useCallback(() => {
    const sessionId = panelsChatSnapshot.activeSessionId || sidebarSnapshot.recentSessionId;
    if (!sessionId) {
      return;
    }
    if (parallelSidebarBirthTimerRef.current !== null) {
      window.clearTimeout(parallelSidebarBirthTimerRef.current);
    }
    setParallelSidebarBirth((current) => ({
      sessionId,
      token: (current?.token ?? 0) + 1
    }));
    parallelSidebarBirthTimerRef.current = window.setTimeout(() => {
      setParallelSidebarBirth(null);
      parallelSidebarBirthTimerRef.current = null;
    }, 900);
  }, [panelsChatSnapshot.activeSessionId, sidebarSnapshot.recentSessionId]);

  const addParallelCanvas = useCallback(() => {
    if (parallelPrompts.length >= 4) {
      return;
    }
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasActivePane("");
    setCanvasPlanetsOpen(false);
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setParallelPrompts((prompts) => [...prompts, ""]);
    triggerParallelSidebarBirth();
  }, [parallelPrompts.length, triggerParallelSidebarBirth]);

  const removableParallelIndexes = useMemo(() => {
    const laneHasStarted = (index: number) => {
      const messages = index === 0
        ? panelsChatSnapshot.transcript
        : panelsChatSnapshot.parallelLanes.find((lane) => lane.index === index)?.transcript ?? [];
      return messages.some((message) => message.role === "user" || message.role === "assistant");
    };

    return parallelPrompts.map((prompt, index) => {
      if (index === 0 || parallelPrompts.length <= 1) {
        return false;
      }
      for (let cursor = index; cursor < parallelPrompts.length; cursor += 1) {
        if ((parallelPrompts[cursor] ?? "").trim() || laneHasStarted(cursor)) {
          return false;
        }
      }
      return true;
    });
  }, [panelsChatSnapshot.parallelLanes, panelsChatSnapshot.transcript, parallelPrompts]);

  const removeParallelCanvas = useCallback((index: number) => {
    if (index <= 0) {
      return;
    }
    const laneHasStarted = (laneIndex: number) => {
      const messages = panelsChatSnapshot.parallelLanes.find((lane) => lane.index === laneIndex)?.transcript ?? [];
      return messages.some((message) => message.role === "user" || message.role === "assistant");
    };
    setParallelPrompts((prompts) => {
      if (index >= prompts.length) {
        return prompts;
      }
      for (let cursor = index; cursor < prompts.length; cursor += 1) {
        if ((prompts[cursor] ?? "").trim() || laneHasStarted(cursor)) {
          return prompts;
        }
      }
      const nextPrompts = prompts.filter((_prompt, promptIndex) => promptIndex !== index);
      return nextPrompts.length > 1 ? nextPrompts : [""];
    });
    setWebExplorerParallelIndex((current) => (current > index ? current - 1 : current === index ? Math.max(0, index - 1) : current));
    setMapsParallelIndex((current) => (current > index ? current - 1 : current === index ? Math.max(0, index - 1) : current));
    if (webExplorerParallelIndex === index) {
      setCanvasWebExplorerOpen(false);
    }
    if (mapsParallelIndex === index) {
      canvasMapsOpenRef.current = false;
      setCanvasMapsOpen(false);
    }
  }, [mapsParallelIndex, panelsChatSnapshot.parallelLanes, webExplorerParallelIndex]);

  const openCanvasFiles = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setCanvasFilesOpen(true);
    setCanvasActivePane("files");
  }, []);

  const openCanvasTerminal = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setCanvasTerminalOpen(true);
    setCanvasActivePane("terminal");
  }, []);

  const closeCanvasFiles = useCallback(() => {
    setCanvasFilesOpen(false);
    setCanvasActivePane((pane) => (pane === "files" ? (canvasTerminalOpen ? "terminal" : "") : pane));
  }, [canvasTerminalOpen]);

  const closeCanvasTerminal = useCallback(() => {
    setCanvasTerminalOpen(false);
    setCanvasActivePane((pane) => (pane === "terminal" ? (canvasFilesOpen ? "files" : "") : pane));
  }, [canvasFilesOpen]);

  const resetNewSessionCanvas = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasActivePane("");
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setParallelPrompts([""]);
    setWelcomeMessage(selectWelcomeMessage(brainUserMemory.preferredFirstName));
    setHomeCanvasResetId((id) => id + 1);
    void panelsChatBottomStore.dispatch({ kind: "new_session" });
  }, [brainUserMemory.preferredFirstName]);

  const openCanvasPlanets = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasActivePane("");
    setCanvasWebExplorerOpen(false);
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setParallelPrompts([""]);
    setCanvasPlanetsOpen(true);
  }, []);

  const openCanvasWebExplorer = useCallback((parallelSessionIndex = 0, options?: { keepMapsOpen?: boolean }) => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasActivePane("");
    setCanvasPlanetsOpen(false);
    if (!options?.keepMapsOpen) {
      canvasMapsOpenRef.current = false;
      setCanvasMapsOpen(false);
    }
    setWebExplorerParallelIndex(parallelSessionIndex);
    if (options?.keepMapsOpen && parallelPrompts.length <= 1) {
      setParallelPrompts([""]);
    } else if (parallelPrompts.length <= 1) {
      setParallelPrompts([""]);
    }
    setCanvasWebExplorerOpen(true);
  }, [parallelPrompts.length]);

  const closeCanvasWebExplorer = useCallback(() => {
    setCanvasWebExplorerOpen(false);
    setWebExplorerParallelIndex(0);
    setWebExplorerModuleId(null);
  }, []);

  const openCanvasMaps = useCallback((parallelSessionIndex = 0) => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasActivePane("");
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    setMapsParallelIndex(parallelSessionIndex);
    mapsOwnerSessionIdRef.current = panelsChatSnapshot.activeSessionId || "draft";
    canvasMapsOpenRef.current = true;
    if (parallelPrompts.length <= 1) {
      setParallelPrompts([""]);
    }
    setCanvasMapsOpen(true);
  }, [panelsChatSnapshot.activeSessionId, parallelPrompts.length]);

  const closeCanvasMaps = useCallback(() => {
    mapsOwnerSessionIdRef.current = null;
    canvasMapsOpenRef.current = false;
    setCanvasMapsOpen(false);
    setMapsParallelIndex(0);
    void globalThis.window?.forgeShell?.hideNativeMaps?.();
  }, []);

  useEffect(() => {
    return globalThis.window?.forgeShell?.onNativeWebExplorerCodeAct?.((event) => {
      const moduleId = webExplorerCodeActModule(event);
      const eventKeywords = (event as { keywords?: unknown }).keywords;
      const hasTravelFallbackKeyword = Array.isArray(eventKeywords) && eventKeywords.includes("host_geographic_travel_fallback");
      const keepMapsOpen = moduleId === "airbnb" && (canvasMapsOpenRef.current || hasTravelFallbackKeyword);
      if (keepMapsOpen && !canvasMapsOpenRef.current) {
        openCanvasMaps(event.parallelSessionIndex ?? 0);
      }
      setWebExplorerModuleId(moduleId);
      openCanvasWebExplorer(event.parallelSessionIndex ?? 0, { keepMapsOpen });
    });
  }, [openCanvasMaps, openCanvasWebExplorer]);

  useEffect(() => {
    return globalThis.window?.forgeShell?.onNativeMapsCodeAct?.((event) => {
      setMapsWebviewUrl(event.url || GOOGLE_EARTH_DOM_DEFAULT_URL);
      openCanvasMaps(event.parallelSessionIndex ?? 0);
    });
  }, [openCanvasMaps]);

  useEffect(() => {
    const latestAssistant = latestAssistantText ? { text: latestAssistantText } : undefined;
    if (latestAssistant?.text.includes("AIRBNB_RESULT")) {
      const keepMapsOpen = latestAssistant.text.includes("MAPS_RESULT") || latestAssistant.text.includes(BRAIN_MAPS_COMMAND);
      if (keepMapsOpen && !canvasMapsOpenRef.current) {
        openCanvasMaps(0);
      }
      setWebExplorerModuleId("airbnb");
      openCanvasWebExplorer(0, { keepMapsOpen });
      return;
    }
    if (latestAssistant?.text.includes("GMAIL_RESULT")) {
      setWebExplorerModuleId("gmail");
      openCanvasWebExplorer(0);
      return;
    }
    if (latestAssistant?.text.includes("GOOGLEWEB_RESULT")) {
      setWebExplorerModuleId(null);
      openCanvasWebExplorer(0);
      return;
    }
    if (latestAssistant?.text.includes("MAPS_RESULT")) {
      openCanvasMaps(0);
    }
  }, [latestAssistantText, openCanvasMaps, openCanvasWebExplorer]);

  const updateParallelPrompt = useCallback((index: number, value: string) => {
    setParallelPrompts((prompts) => prompts.map((prompt, promptIndex) => (promptIndex === index ? value : prompt)));
  }, []);

  const closeProfileCanvas = useCallback(async () => {
    await sidebarShadowStore.dispatch(
      sidebarShadowStore.command({ kind: "open_profile_canvas", canvas: "" }),
      "workspace-close-profile-canvas"
    );
    await Promise.all([sidebarShadowStore.boot(), headerShadowStore.boot()]);
  }, []);

  const dispatch = useCallback(
    async (control: Pick<HeaderControl, "id" | "command" | "route">) => {
      if (control.id === "right-panel") {
        if (isFullPageCanvas) {
          await closeProfileCanvas();
        }
        if (canvasFilesOpen || canvasTerminalOpen) {
          setCanvasFilesOpen(false);
          setCanvasTerminalOpen(false);
          setCanvasActivePane("");
          setCanvasSplitOpen(false);
          return;
        }
        setCanvasSplitOpen((open) => !open);
        return;
      }
      if (control.id === "webexplorer-workspace") {
        if (isFullPageCanvas) {
          await closeProfileCanvas();
        }
        setCanvasSplitOpen(true);
        openCanvasPlanets();
        return;
      }
      await headerShadowStore.dispatchControl(control);
      if (isFullPageCanvas) {
        await Promise.all([sidebarShadowStore.boot(), headerShadowStore.boot()]);
      }
      if (control.command === "open_sessions_canvas") {
        const sidebarSnapshot = sidebarShadowStore.getSnapshot().snapshot;
        if (sidebarSnapshot.profileCanvas === "sessions" && sidebarSnapshot.sessionsMenuMode === "archived") {
          await sidebarShadowStore.dispatch(
            sidebarShadowStore.command({ kind: "open_profile_canvas", canvas: "" }),
            "sessions-archived-toggle"
          );
          return;
        }
        await sidebarShadowStore.dispatch(
          sidebarShadowStore.command({ kind: "switch_sessions_mode", mode: "archived" }),
          "sessions-archived"
        );
      }
    },
    [canvasFilesOpen, canvasTerminalOpen, closeProfileCanvas, isFullPageCanvas, openCanvasPlanets]
  );

  const dispatchWindowControl = useCallback(
    async (control: Pick<HeaderControl, "id" | "command">) => {
      const api = globalThis.window?.forgeWindowControls;
      if (!api) {
        console.error("[window-controls] preload API missing");
        return;
      }
      try {
        if (control.command === "window_minimize") {
          console.info("[window-controls] minimize requested");
          await api.minimize();
          return;
        }
        if (control.command === "window_toggle_maximize") {
          console.info("[window-controls] maximize requested");
          await api.toggleMaximize();
          return;
        }
        if (control.command === "window_close") {
          console.info("[window-controls] close requested");
          await api.close();
        }
      } catch (error) {
        console.error("[window-controls] native command failed", error);
      }
    },
    []
  );

  const releaseWidgetModeTransition = useCallback((sequenceToken: number, delayMs = 0) => {
    const release = () => {
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      widgetModeTransitioningRef.current = false;
      setWidgetModeTransitioning(false);
    };
    if (delayMs > 0) {
      window.setTimeout(release, delayMs);
      return;
    }
    release();
  }, []);

  const setWidgetModeEnabled = useCallback((enabled: boolean) => {
    if (widgetModeTransitioningRef.current) {
      return;
    }
    widgetModeTransitioningRef.current = true;
    setWidgetModeTransitioning(true);
    const sequenceToken = widgetModeSequenceRef.current + 1;
    widgetModeSequenceRef.current = sequenceToken;
    if (widgetSurfaceCloseTimerRef.current !== null) {
      window.clearTimeout(widgetSurfaceCloseTimerRef.current);
      widgetSurfaceCloseTimerRef.current = null;
    }
    setWorkspaceMenuOpen(false);
    setWorkspaceNotice(null);
    if (!enabled) {
      void (async () => {
        const windowControls = globalThis.window?.forgeWindowControls;
        const nativeWidgetRestore = windowControls?.setWidgetMode?.(false).catch((error: unknown) => {
          console.warn("Failed to restore native widget mode", error);
          return false;
        });
        clearDocumentWidgetModeClasses();
        setWidgetMinimizingPhase("");
        setWidgetMode(false);
        setWidgetLayoutLock(null);
        const nativeWidgetRestored = await nativeWidgetRestore;
        if (widgetModeSequenceRef.current !== sequenceToken) {
          return;
        }
        if (nativeWidgetRestored === false) {
          console.warn("Native widget restore was not accepted.");
        }
        releaseWidgetModeTransition(sequenceToken, 420);
      })();
      return;
    }

    void (async () => {
      setWidgetLayoutLock(readWidgetLayoutLock());
      setWidgetMinimizingPhase("sides");
      setWidgetMode(false);
      const windowControls = globalThis.window?.forgeWindowControls;
      const nativeWidgetModeReady = windowControls?.setWidgetMode?.(true, WIDGET_NATIVE_SHRINK_DELAY_MS)
        .catch((error: unknown) => {
          console.warn("Failed to arm native widget mode", error);
          return false;
        });
      if (snapshot.leftPanelOpen) {
        void headerShadowStore
          .dispatchControl({ id: "left-panel", command: "toggle_left_panel" })
          .then(() => Promise.all([headerShadowStore.boot(), sidebarShadowStore.boot()]));
      }

      await waitForWidgetMotion(WIDGET_SIDE_EXIT_MS);
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      setWidgetMinimizingPhase("header");

      await waitForWidgetMotion(WIDGET_HEADER_EXIT_MS);
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      setWidgetMinimizingPhase("canvas");

      await waitForWidgetMotion(WIDGET_CANVAS_EXIT_MS);
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      setWidgetMode(true);
      await waitForWidgetMotion(WIDGET_HANDOFF_SETTLE_MS);
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      setWidgetMinimizingPhase("");
      setCanvasSplitOpen(false);
      setCanvasFilesOpen(false);
      setCanvasTerminalOpen(false);
      setCanvasActivePane("");
      setCanvasPlanetsOpen(false);
      setCanvasWebExplorerOpen(false);
      canvasMapsOpenRef.current = false;
      setCanvasMapsOpen(false);
      setParallelPrompts([""]);
      void globalThis.window?.forgeShell?.hideNativeWebExplorer?.();
      void globalThis.window?.forgeShell?.hideNativeMaps?.();
      if (activeProfileCanvas) {
        void closeProfileCanvas();
      }
      const nativeWidgetAccepted = await nativeWidgetModeReady;
      if (widgetModeSequenceRef.current !== sequenceToken) {
        return;
      }
      if (nativeWidgetAccepted === false) {
        console.warn("Native widget mode was not accepted.");
      }
      releaseWidgetModeTransition(sequenceToken);
    })();
  }, [activeProfileCanvas, closeProfileCanvas, releaseWidgetModeTransition, snapshot.leftPanelOpen]);
  const topControls = useMemo(() => snapshot.topControls.filter((control) => control.visible), [snapshot]);
  const workspaceControls = useMemo(
    () => snapshot.workspaceControls.filter((control) => control.visible),
    [snapshot]
  );

  return (
    <main className={shellClassName} style={shellStyleWithWidgetLock(widgetLayoutLock)}>
      <section className="titlebar" aria-label="InGen top controls">
        <div className="titlebar__cluster">
          {topControls.slice(0, 5).map((control) => {
            return (
              <button
                type="button"
                className={control.selected ? "iconButton iconButton--selected" : "iconButton"}
                aria-label={control.label}
                aria-controls={
                  control.command === "toggle_left_panel"
                    ? "left-panel"
                    : control.command === "open_sessions_canvas"
                      ? "sessions-menu"
                      : undefined
                }
                aria-expanded={control.command === "toggle_left_panel" ? snapshot.leftPanelOpen : undefined}
                title={control.label}
                key={control.id}
                onClick={() => void dispatch(control)}
              >
                <span className={`shellIcon ${iconClass(control)}`} aria-hidden="true" />
              </button>
            );
          })}
        </div>
        <div className="titlebar__drag" aria-hidden="true" />
        <div className="titlebar__window">
          {WINDOW_CONTROLS.map((control) => {
            return (
              <button
                type="button"
                className={control.id === "window-close" ? "windowButton windowButton--danger" : "windowButton"}
                aria-label={control.label}
                title={control.label}
                key={control.id}
                onPointerDown={(event) => {
                  if (event.button !== 0) {
                    return;
                  }
                  event.preventDefault();
                  event.stopPropagation();
                  void dispatchWindowControl(control);
                }}
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                }}
                onKeyDown={(event) => {
                  if (event.key !== "Enter" && event.key !== " ") {
                    return;
                  }
                  event.preventDefault();
                  event.stopPropagation();
                  void dispatchWindowControl(control);
                }}
              >
                <span className={windowGlyphClass(control.command)} aria-hidden="true" />
              </button>
            );
          })}
        </div>
      </section>

      <button
        type="button"
        className="widgetWindowsButton"
        aria-hidden={!widgetMode}
        aria-label="Afficher ou masquer la barre des taches Windows"
        tabIndex={widgetMode ? 0 : -1}
        title="Afficher ou masquer la barre des taches Windows"
        onClick={() => {
          void globalThis.window?.forgeWindowControls?.toggleWidgetTaskbar?.();
        }}
      >
        <svg viewBox="0 0 88 88" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" focusable="false">
          <path d="m0 12.402 35.687-4.86.016 34.423-35.67.203zm35.67 33.529.028 34.453L.028 75.48.026 45.7zm4.326-39.025L87.314 0v41.527l-47.318.376zm47.329 39.349-.011 41.34-47.318-6.678-.066-34.739z" />
        </svg>
      </button>

      <section className="workspaceHeader" aria-label="Workspace header">
        {isFullPageCanvas && !isBrainCanvas ? (
          <button
            type="button"
            className="workspaceHeader__close"
            aria-label={isBrainCanvas ? "Close Brain" : "Close LLM Provider"}
            title={isBrainCanvas ? "Close Brain" : "Close LLM Provider"}
            onPointerDown={(event) => {
              if (event.button !== 0) {
                return;
              }
              event.preventDefault();
              event.stopPropagation();
              void closeProfileCanvas();
            }}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
          >
            <span className="workspaceHeader__closeIcon" aria-hidden="true" />
          </button>
        ) : (
          <div className="workspaceHeader__menuHost" ref={workspaceMenuRef}>
            <button
              type="button"
              className="workspaceHeader__markButton"
              aria-label="Workspace actions"
              aria-haspopup="menu"
              aria-expanded={workspaceMenuOpen}
              onClick={() => {
                setWorkspaceNotice(null);
                setWorkspaceMenuOpen((open) => !open);
              }}
            >
              <svg className="workspaceHeader__mark" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
                <path d="M3.75 7.25A2.25 2.25 0 0 1 6 5h4.15l2 2H18a2.25 2.25 0 0 1 2.25 2.25v7.5A2.25 2.25 0 0 1 18 19H6a2.25 2.25 0 0 1-2.25-2.25v-9.5Z" />
              </svg>
            </button>
            {workspaceMenuOpen ? (
              <div className="workspaceMiniMenu" role="menu" aria-label="Workspace actions">
                <button type="button" role="menuitem" onClick={() => void runWorkspaceMenuAction("show")}>
                  Show in Explorer
                </button>
                <button type="button" role="menuitem" onClick={() => void runWorkspaceMenuAction("copyPath")}>
                  Copy path
                </button>
                <button type="button" role="menuitem" onClick={() => void runWorkspaceMenuAction("copyBranch")}>
                  Copy branch name
                </button>
              </div>
            ) : null}
            {workspaceNotice !== null && !workspaceMenuOpen ? (
              <div className="workspaceMiniNotice" role="status" aria-live="polite">
                <span className="workspaceMiniNotice__icon" aria-hidden="true">i</span>
                <span>{workspaceNotice}</span>
              </div>
            ) : null}
          </div>
        )}
        <div className="workspaceHeader__crumb">
          {isFullPageCanvas ? (
            <strong className="workspaceHeader__group workspaceHeader__group--page">{isBrainCanvas ? "Brain" : "LLM Provider"}</strong>
          ) : (
            <>
              <button
                type="button"
                className={workspaceGateActive ? "workspaceHeader__group workspaceHeader__group--pick workspaceHeader__group--required" : "workspaceHeader__group workspaceHeader__group--pick"}
                onClick={() => void chooseWorkspace()}
              >
                {workspaceFolder ?? sectionGroup(snapshot.activeSection)}
              </button>
              <span className="workspaceHeader__slash">/</span>
              <strong className={snapshot.activeSection === "trading" ? "workspaceHeader__title accent" : "workspaceHeader__title"}>
                {activeSessionName}
              </strong>
            </>
          )}
        </div>
        <div className="workspaceHeader__actions">
          {workspaceControls.map((control) => {
            const selected =
              control.id === "plan"
                ? snapshot.rightPanelOpen
                : control.id === "right-panel"
                  ? canvasSplitOpen || canvasFilesOpen || canvasTerminalOpen
                  : control.id === "webexplorer-workspace"
                    ? canvasPlanetsOpen
                  : control.selected;
            return (
              <button
                type="button"
                className={selected ? "navIconButton navIconButton--selected" : "navIconButton"}
                aria-label={control.label}
                aria-controls={
                  control.id === "plan" || control.id === "right-panel" || control.id === "webexplorer-workspace"
                    ? control.id === "plan"
                      ? "right-panel"
                      : "split-canvas"
                    : undefined
                }
                aria-expanded={
                  control.id === "plan"
                    ? snapshot.rightPanelOpen
                    : control.id === "right-panel"
                      ? canvasSplitOpen || canvasFilesOpen || canvasTerminalOpen
                      : control.id === "webexplorer-workspace"
                        ? canvasPlanetsOpen
                      : undefined
                }
                key={control.id}
                onClick={() => void dispatch(control)}
              >
                <span className={`shellIcon ${iconClass(control)}`} aria-hidden="true" />
              </button>
            );
          })}
        </div>
      </section>

      {workspaceGateActive ? (
        <div className="workspaceRequiredVeil" aria-hidden="true" onClick={() => void chooseWorkspace()} />
      ) : null}

      {isBangerPage ? (
        <HeaderSurfaceRouter snapshot={headerSurfaceSnapshot} />
      ) : null}

      {!isFullPageCanvas && !isBangerPage && !sessionHasStarted ? (
        <section className="canvasCover" aria-label="Forge home canvas">
          <ProfileCoverBanner key={`home-canvas-${homeCanvasResetId}`} leftPanelOpen={snapshot.leftPanelOpen} welcomeMessage={welcomeMessage} />
        </section>
      ) : null}

      <SidebarSlice
        open={snapshot.leftPanelOpen}
        activeParallelLaneCount={parallelPrompts.length}
        parallelBirthAnimationSessionId={parallelSidebarBirth?.sessionId ?? ""}
        parallelBirthAnimationKey={parallelSidebarBirth?.token ?? 0}
        onNewSession={resetNewSessionCanvas}
        onModuleSelect={toggleComposerModule}
        onModuleDrop={dropComposerModule}
        onCloseProfileCanvas={() => void closeProfileCanvas()}
      />
      <RightPanelSlice open={snapshot.rightPanelOpen} />

      {!isFullPageCanvas && !isBangerPage ? (
        <CanvasSurfacesSlice
          split={canvasSurfaceOpen}
          actionsOpen={canvasSplitOpen}
          filesOpen={canvasFilesOpen}
          terminalOpen={canvasTerminalOpen}
          activePane={canvasActivePane}
          planetsOpen={canvasPlanetsOpen}
          webExplorerOpen={canvasWebExplorerOpen}
          webExplorerParallelIndex={webExplorerParallelIndex}
          webExplorerModuleId={webExplorerModuleId}
          mapsOpen={canvasMapsOpen}
          mapsParallelIndex={mapsParallelIndex}
          mapsUrl={mapsWebviewUrl}
          mapsSearchQuery={latestAssistantGeoEntityLabel}
          leftPanelOpen={snapshot.leftPanelOpen}
          parallelPrompts={parallelPrompts}
          removableParallelIndexes={removableParallelIndexes}
          sessionFiles={sessionFiles}
          sessionName={activeSessionName}
          onFilesOpen={openCanvasFiles}
          onFilesClose={closeCanvasFiles}
          onTerminalOpen={openCanvasTerminal}
          onTerminalClose={closeCanvasTerminal}
          onActivePaneChange={setCanvasActivePane}
          onPlanetsOpen={openCanvasPlanets}
          onPlanetsClose={() => setCanvasPlanetsOpen(false)}
          onWebExplorerOpen={openCanvasWebExplorer}
          onWebExplorerClose={closeCanvasWebExplorer}
          onMapsClose={closeCanvasMaps}
          onParallelAdd={addParallelCanvas}
          onParallelRemove={removeParallelCanvas}
        />
      ) : null}
      {renderPanelsChatBottom ? (
        <PanelsChatBottomSlice
          composerOnly={isBangerPage}
          parallelPrompts={parallelPrompts}
          onParallelPromptChange={updateParallelPrompt}
          webExplorerOpen={canvasWebExplorerOpen}
          composerModule={composerModuleId ?? (canvasWebExplorerOpen ? webExplorerModuleId : null)}
          onComposerModuleChange={setComposerModuleId}
          widgetMode={widgetMode || widgetMinimizingPhase !== ""}
          widgetModeTransitioning={widgetModeTransitioning}
          onWidgetModeChange={setWidgetModeEnabled}
        />
      ) : null}
    </main>
  );
}
