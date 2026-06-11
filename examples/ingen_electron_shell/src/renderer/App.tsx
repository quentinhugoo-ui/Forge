import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type ComposerUploadPreview,
  type HeaderControl,
  type NativeSection
} from "../shared/ipc-contract";
import tokens from "../shared/generated/design-tokens.generated.json";
import { CanvasSurfacesSlice } from "./CanvasSurfacesSlice";
import { PanelsChatBottomSlice } from "./PanelsChatBottomSlice";
import { ProfileCoverBanner } from "./ProfileCoverBanner";
import { RightPanelSlice } from "./RightPanelSlice";
import { readBrainUserMemory } from "./brain-user-memory-store";
import { headerShadowStore, useHeaderShadowStore } from "./header-shadow-store";
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
  const { snapshot: sidebarSnapshot } = useSidebarShadowStore();
  const { snapshot: panelsChatSnapshot } = usePanelsChatBottomStore();
  const [canvasSplitOpen, setCanvasSplitOpen] = useState(false);
  const [canvasFilesOpen, setCanvasFilesOpen] = useState(false);
  const [canvasTerminalOpen, setCanvasTerminalOpen] = useState(false);
  const [canvasPlanetsOpen, setCanvasPlanetsOpen] = useState(false);
  const [canvasWebExplorerOpen, setCanvasWebExplorerOpen] = useState(false);
  const [composerModuleId, setComposerModuleId] = useState<SidebarModuleId | null>(null);
  const toggleComposerModule = useCallback((id: SidebarModuleId) => {
    setComposerModuleId((current) => (current === id ? null : id));
  }, []);
  const dropComposerModule = useCallback((id: SidebarModuleId) => {
    setComposerModuleId(id);
  }, []);
  const [parallelPrompts, setParallelPrompts] = useState<string[]>([""]);
  const [brainUserMemory] = useState(() => readBrainUserMemory());
  const [welcomeMessage, setWelcomeMessage] = useState(() => selectWelcomeMessage(brainUserMemory.preferredFirstName));
  const [homeCanvasResetId, setHomeCanvasResetId] = useState(0);
  const [workspaceFolder, setWorkspaceFolder] = useState<string | null>(null);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceNotice, setWorkspaceNotice] = useState<string | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const workspaceNoticeTimerRef = useRef<number | null>(null);
  const chooseWorkspace = useCallback(async () => {
    const result = await globalThis.window?.forgeShell?.chooseWorkspaceFolder?.();
    if (result && !result.canceled && result.folderName) {
      setWorkspaceFolder(result.folderName);
    }
  }, []);
  useEffect(() => {
    let active = true;
    void globalThis.window?.forgeShell?.getWorkspaceFolder?.().then((result) => {
      if (active && result && !result.canceled && result.folderName) {
        setWorkspaceFolder(result.folderName);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  const activeProfileCanvas = sidebarSnapshot.profileCanvas || snapshot.profileCanvas;
  const isLlmProviderCanvas = activeProfileCanvas === "llm";
  const renderPanelsChatBottom = globalThis.location?.port !== "5176" && !isLlmProviderCanvas;
  const canvasSurfaceOpen =
    canvasSplitOpen ||
    canvasFilesOpen ||
    canvasTerminalOpen ||
    canvasPlanetsOpen ||
    canvasWebExplorerOpen ||
    parallelPrompts.length > 1;
  const activeSessionName = useMemo(() => {
    const items = [...sidebarSnapshot.recentItems, ...sidebarSnapshot.archivedItems];
    if (!sidebarSnapshot.recentSessionId) {
      return "New session";
    }
    return items.find((item) => item.sessionId !== "" && item.sessionId === sidebarSnapshot.recentSessionId)?.label || "New session";
  }, [sidebarSnapshot.archivedItems, sidebarSnapshot.recentItems, sidebarSnapshot.recentSessionId]);
  const sessionHasStarted = useMemo(
    () => panelsChatSnapshot.transcript.some((message) => message.role === "user" || message.role === "assistant"),
    [panelsChatSnapshot.transcript]
  );
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
    isLlmProviderCanvas ? "shell--llm-provider" : ""
  ].join(" ");

  useEffect(() => {
    let mounted = true;
    void sidebarShadowStore
      .dispatch(sidebarShadowStore.command({ kind: "navigate", section: "forge" }), "startup-new-session")
      .then(async () => {
        if (!mounted) {
          return;
        }
        await Promise.all([headerShadowStore.boot(), sidebarShadowStore.boot()]);
      });
    return () => {
      mounted = false;
    };
  }, []);

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
      if (canvasPlanetsOpen) {
        setCanvasPlanetsOpen(false);
      }
      if (canvasWebExplorerOpen) {
        setCanvasWebExplorerOpen(false);
      }
      if (parallelPrompts.length > 1) {
        setParallelPrompts([""]);
      }
    }
  }, [canvasFilesOpen, canvasPlanetsOpen, canvasSurfaceOpen, canvasTerminalOpen, canvasWebExplorerOpen, parallelPrompts.length]);

  const addParallelCanvas = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    setParallelPrompts((prompts) => (prompts.length >= 4 ? prompts : [...prompts, ""]));
  }, []);

  const openCanvasFiles = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    setCanvasFilesOpen(true);
  }, []);

  const openCanvasTerminal = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    setCanvasTerminalOpen(true);
  }, []);

  const resetNewSessionCanvas = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasPlanetsOpen(false);
    setCanvasWebExplorerOpen(false);
    setParallelPrompts([""]);
    setWelcomeMessage(selectWelcomeMessage(brainUserMemory.preferredFirstName));
    setHomeCanvasResetId((id) => id + 1);
    void panelsChatBottomStore.dispatch({ kind: "new_session" });
  }, [brainUserMemory.preferredFirstName]);

  const openCanvasPlanets = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasWebExplorerOpen(false);
    setParallelPrompts([""]);
    setCanvasPlanetsOpen(true);
  }, []);

  const openCanvasWebExplorer = useCallback(() => {
    setCanvasSplitOpen(false);
    setCanvasFilesOpen(false);
    setCanvasTerminalOpen(false);
    setCanvasPlanetsOpen(false);
    setParallelPrompts([""]);
    setCanvasWebExplorerOpen(true);
  }, []);

  const closeCanvasWebExplorer = useCallback(() => {
    setCanvasWebExplorerOpen(false);
  }, []);

  useEffect(() => {
    return globalThis.window?.forgeShell?.onNativeWebExplorerCodeAct?.(() => {
      openCanvasWebExplorer();
    });
  }, [openCanvasWebExplorer]);

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
        if (isLlmProviderCanvas) {
          await closeProfileCanvas();
        }
        setCanvasSplitOpen((open) => !open);
        return;
      }
      if (control.id === "webexplorer-workspace") {
        if (isLlmProviderCanvas) {
          await closeProfileCanvas();
        }
        setCanvasSplitOpen(true);
        openCanvasPlanets();
        return;
      }
      await headerShadowStore.dispatchControl(control);
      if (isLlmProviderCanvas) {
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
    [closeProfileCanvas, isLlmProviderCanvas, openCanvasPlanets]
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

  const topControls = useMemo(() => snapshot.topControls.filter((control) => control.visible), [snapshot]);
  const workspaceControls = useMemo(
    () => snapshot.workspaceControls.filter((control) => control.visible),
    [snapshot]
  );

  return (
    <main className={shellClassName} style={cssTokenStyle()}>
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
        <div className="hardwarePill" aria-label="Hardware status" title={`CPU: ${snapshot.cpuLabel}\nGPU: ${snapshot.gpuLabel}`}>
          <span>CPU</span>
          <code>{snapshot.cpuLabel}</code>
          <i />
          <span>GPU</span>
          <code>{snapshot.gpuLabel}</code>
        </div>
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

      <section className="workspaceHeader" aria-label="Workspace header">
        {isLlmProviderCanvas ? (
          <button
            type="button"
            className="workspaceHeader__close"
            aria-label="Close LLM Provider"
            title="Close LLM Provider"
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
          {isLlmProviderCanvas ? (
            <strong className="workspaceHeader__group workspaceHeader__group--page">LLM Provider</strong>
          ) : (
            <>
              <button
                type="button"
                className="workspaceHeader__group workspaceHeader__group--pick"
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
                  ? canvasSplitOpen
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
                      ? canvasSplitOpen
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

      {!isLlmProviderCanvas && !sessionHasStarted ? (
        <section className="canvasCover" aria-label="Forge home canvas">
          <ProfileCoverBanner key={`home-canvas-${homeCanvasResetId}`} leftPanelOpen={snapshot.leftPanelOpen} welcomeMessage={welcomeMessage} />
        </section>
      ) : null}

      <SidebarSlice
        open={snapshot.leftPanelOpen}
        onNewSession={resetNewSessionCanvas}
        onModuleSelect={toggleComposerModule}
        onModuleDrop={dropComposerModule}
      />
      <RightPanelSlice open={snapshot.rightPanelOpen} />

      {!isLlmProviderCanvas ? (
        <CanvasSurfacesSlice
          split={canvasSurfaceOpen}
          actionsOpen={canvasSplitOpen}
          filesOpen={canvasFilesOpen}
          terminalOpen={canvasTerminalOpen}
          planetsOpen={canvasPlanetsOpen}
          webExplorerOpen={canvasWebExplorerOpen}
          parallelPrompts={parallelPrompts}
          sessionFiles={sessionFiles}
          sessionName={activeSessionName}
          onFilesOpen={openCanvasFiles}
          onFilesClose={() => setCanvasFilesOpen(false)}
          onTerminalOpen={openCanvasTerminal}
          onTerminalClose={() => setCanvasTerminalOpen(false)}
          onPlanetsOpen={openCanvasPlanets}
          onPlanetsClose={() => setCanvasPlanetsOpen(false)}
          onWebExplorerOpen={openCanvasWebExplorer}
          onWebExplorerClose={closeCanvasWebExplorer}
          onParallelAdd={addParallelCanvas}
        />
      ) : null}
      {renderPanelsChatBottom ? (
        <PanelsChatBottomSlice
          parallelPrompts={parallelPrompts}
          onParallelPromptChange={updateParallelPrompt}
          webExplorerOpen={canvasWebExplorerOpen}
          composerModule={composerModuleId}
        />
      ) : null}
    </main>
  );
}
