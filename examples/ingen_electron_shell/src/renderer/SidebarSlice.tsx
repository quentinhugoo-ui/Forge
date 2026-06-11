import { Fragment, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import {
  closestCenter,
  DndContext,
  DragOverlay,
  KeyboardSensor,
  pointerWithin,
  PointerSensor,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
  type DragMoveEvent,
  type DragStartEvent,
  type DraggableSyntheticListeners
} from "@dnd-kit/core";
import { snapCenterToCursor } from "@dnd-kit/modifiers";
import {
  arrayMove,
  rectSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { LlmProviderRuntimeEvent, ProfileCanvas, SidebarSessionItem, SidebarToolControl } from "../shared/ipc-contract";
import { sidebarShadowStore, useSidebarShadowStore } from "./sidebar-shadow-store";
import { headerShadowStore } from "./header-shadow-store";
import { panelsChatBottomStore } from "./panels-chat-bottom-store";
import { ProfileCoverBanner } from "./ProfileCoverBanner";
import { ProviderLogo } from "./ProviderLogo";

const SIDEBAR_ICON_SIZE = 16;
const SESSION_ARCHIVE_ICON_SIZE = 12;

function SidebarIcon({ tool }: { tool: Pick<SidebarToolControl, "id" | "icon" | "label"> }) {
  const svgProps = (className = "sidebarIcon", viewBox = "0 0 24 24") => ({
    className,
    viewBox,
    width: SIDEBAR_ICON_SIZE,
    height: SIDEBAR_ICON_SIZE,
    fill: "none",
    xmlns: "http://www.w3.org/2000/svg",
    "aria-hidden": true
  } as const);

  if (tool.id === "new-session") {
    return (
      <svg {...svgProps("sidebarIcon sidebarIcon--plus")}>
        <g className="plusIcon__mark" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.65" vectorEffect="non-scaling-stroke">
          <path d="M12 3.75v16.5" />
          <path d="M3.75 12h16.5" />
        </g>
      </svg>
    );
  }

  if (tool.id === "pool") {
    return (
      <svg {...svgProps()} fill="currentColor">
        <path d="M24 15.9c0-2.8-1.5-5-3.7-6.1C21.3 8.8 22 7.5 22 6c0-2.8-2.2-5-5-5-2.1 0-3.8 1.2-4.6 3H11.6C10.8 2.2 9.1 1 7 1 4.2 1 2 3.2 2 6c0 1.5.7 2.8 1.7 3.8C1.5 10.9 0 13.2 0 15.9V20h5v3h14v-3h5v-4.1ZM17 3c1.7 0 3 1.3 3 3 0 1.6-1.3 3-3 3 0-1.9-1.1-3.5-2.7-4.4C14.8 3.6 15.8 3 17 3ZM15 9c0 1.7-1.3 3-3 3S9 10.7 9 9s1.3-3 3-3 3 1.3 3 3ZM7 3c1.2 0 2.2.6 2.7 1.6C8.1 5.5 7 7.1 7 9c-1.7 0-3-1.3-3-3s1.3-3 3-3ZM5.1 18H2v-2.1C2 13.1 4.1 11 7 11h.3c.3.7.8 1.3 1.3 1.8-1.9 1-3.2 2.9-3.5 5.2ZM17 21H7v-2.1c0-2.8 2.2-4.9 5-4.9 2.9 0 5 2.1 5 4.9V21ZM22 18h-3.1c-.3-2.3-1.7-4.2-3.7-5.2.6-.5 1-1.1 1.3-1.8h.4c2.9 0 5 2.1 5 4.9V18Z" />
      </svg>
    );
  }

  if (tool.id === "modules") {
    return (
      <svg {...svgProps("sidebarIcon sidebarIcon--modules", "2 2 20 20")} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
        <g className="modulesIcon__settled" strokeWidth="2">
          <rect height="6" rx="0.86" width="6" x="4" y="4" />
          <rect height="6" rx="0.86" width="6" x="4" y="14" />
          <rect height="6" rx="0.86" width="6" x="14" y="14" />
        </g>
        <g className="modulesIcon__traveler" strokeWidth="2">
          <rect height="6" rx="0.86" width="6" x="14" y="4" />
        </g>
      </svg>
    );
  }

  if (tool.id === "assets") {
    return (
      <svg {...svgProps("sidebarIcon", "1 4 21 16")}>
        <path d="M4 9V6.47214C4 6.16165 4.07229 5.85542 4.21115 5.57771L5 4H10L11 6H21C21.5523 6 22 6.44772 22 7V18C22 19.1046 21.1046 20 20 20H18" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
        <path d="M17.2362 9H2.30925C1.64988 9 1.17099 9.62698 1.34449 10.2631L3.59806 18.5262C3.83537 19.3964 4.62569 20 5.52759 20H19.6908C20.3501 20 20.829 19.373 20.6555 18.7369L18.201 9.73688C18.0823 9.30182 17.6872 9 17.2362 9Z" stroke="currentColor" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }

  if (tool.id === "automations") {
    return (
      <svg {...svgProps("sidebarIcon sidebarIcon--clock", "2 2 20 20")} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="9" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
        <line className="clockIcon__hour" x1="12" x2="12" y1="12" y2="6.5" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
        <line className="clockIcon__minute" x1="12" x2="16" y1="12" y2="12" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }

  if (tool.id === "brain") {
    return (
      <svg {...svgProps("sidebarIcon", "2.25 2.25 15.5 15.5")}>
        <path d="M9.5 4.5c0-.1-.02-.48-.15-.82a1.22 1.22 0 0 0-.32-.5A.76.76 0 0 0 8.5 3a2.91 2.91 0 0 0-1.76.58C6.28 3.94 6 4.43 6 5a.5.5 0 0 1-.66.47c-.18-.06-.35-.02-.53.12-.2.16-.39.45-.53.83-.28.78-.25 1.73.14 2.3A.5.5 0 0 1 4.5 9h.75a2.25 2.25 0 0 1 2.25 2.25v.34m2-7.09v10m0-7H8.42m2.08 7h.75c.69 0 1.25-.56 1.25-1.25v-1.84M9.5 15.47c-.05.12-.22.45-.55.81-.39.41-.89.72-1.45.72-.81 0-1.43-.4-1.86-.94-.44-.55-.64-1.19-.64-1.56a.5.5 0 0 0-.5-.5c-.13 0-.52-.08-.86-.38C3.31 13.34 3 12.86 3 12c0-.98.12-1.63.32-2.03m7.18-5.47c0-.1.02-.48.15-.82.08-.2.18-.37.32-.5A.76.76 0 0 1 11.5 3c.63 0 1.25.2 1.76.58.46.36.74.85.74 1.42a.5.5 0 0 0 .66.47c.18-.06.35-.02.53.12.2.16.39.45.53.83.28.78.25 1.73-.14 2.3A.5.5 0 0 0 16 9.5c.13 0 .26.03.38.1.12.08.22.2.3.37.2.4.32 1.05.32 2.03 0 .86-.31 1.34-.64 1.62-.34.3-.73.38-.86.38a.5.5 0 0 0-.5.5c0 .37-.2 1.01-.64 1.56-.43.54-1.05.94-1.86.94-.56 0-1.06-.31-1.45-.72a3.63 3.63 0 0 1-.55-.81M6.5 7a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm6 2a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm-6 4a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Z" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }

  return (
    <span
      className="sidebarIcon"
      style={{
        maskImage: `url("/shell-assets/${tool.icon}")`,
        WebkitMaskImage: `url("/shell-assets/${tool.icon}")`
      }}
      aria-hidden="true"
    />
  );
}

/* PoolClaw header icons — normalized to the sidebar icon contract:
   24-unit viewBox, 16px render, 1.65 non-scaling stroke (matches New Session/Modules/etc). */
function NavIcon({ kind }: { kind: string }) {
  const base = {
    className: "navGlyph",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.65,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true
  };
  if (kind === "dm") {
    return <svg width={16} height={16} {...base}><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></svg>;
  }
  if (kind === "friends") {
    return (
      <svg width={16} height={16} {...base}>
        <path d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3z" />
        <path d="M8 11c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3z" />
        <path d="M8 14c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" />
        <path d="M16 14c-.29 0-.62.02-.97.05C16.19 14.89 18 16.02 18 18v2h6v-2c0-2.66-5.33-4-8-4z" />
      </svg>
    );
  }
  if (kind === "bell") {
    return (
      <svg width={16} height={16} {...base}>
        <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
        <path d="M13.73 21a2 2 0 0 1-3.46 0" />
      </svg>
    );
  }
  if (kind === "chevron") {
    return <svg className="navGlyph" width={10} height={10} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" aria-hidden="true"><polyline points="6 9 12 15 18 9" /></svg>;
  }
  const small = { ...base, width: 16, height: 16 };
  if (kind === "persona") {
    return <svg {...small}><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>;
  }
  if (kind === "pool") {
    return <svg {...small}><circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" /></svg>;
  }
  if (kind === "wallet") {
    return <svg {...small}><rect x="2" y="5" width="20" height="14" rx="2" /><path d="M16 12h.01" /><path d="M2 10h20" /></svg>;
  }
  if (kind === "contacts") {
    return <svg {...small}><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M23 21v-2a4 4 0 0 0-3-3.87" /><path d="M16 3.13a4 4 0 0 1 0 7.75" /></svg>;
  }
  if (kind === "agent") {
    return <svg {...small}><rect x="2" y="2" width="20" height="20" rx="4" /><path d="M8 9h8" /><path d="M8 13h6" /></svg>;
  }
  if (kind === "jobs") {
    return <svg {...small}><rect x="2" y="7" width="20" height="14" rx="2" /><path d="M16 7V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v2" /><line x1="12" y1="12" x2="12" y2="16" /><line x1="10" y1="14" x2="14" y2="14" /></svg>;
  }
  if (kind === "ip") {
    return <svg {...small}><polyline points="22 7 13.5 15.5 8.5 10.5 2 17" /><polyline points="16 7 22 7 22 13" /></svg>;
  }
  if (kind === "globe") {
    return <svg {...small}><circle cx="12" cy="12" r="10" /><line x1="2" y1="12" x2="22" y2="12" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" /></svg>;
  }
  if (kind === "settings") {
    return <svg {...small}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" /></svg>;
  }
  if (kind === "logout") {
    return <svg {...small}><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" /><polyline points="16 17 21 12 16 7" /><line x1="21" y1="12" x2="9" y2="12" /></svg>;
  }
  return null;
}

function sessionArchiveId(item: SidebarSessionItem): string {
  return item.sessionId || item.label;
}

function ArchiveSessionIcon() {
  return (
    <svg
      className="sessionArchiveIcon"
      fill="none"
      height={SESSION_ARCHIVE_ICON_SIZE}
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      viewBox="2 3 20 20"
      width={SESSION_ARCHIVE_ICON_SIZE}
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect className="sessionArchiveIcon__lid" height="5" width="20" x="2" y="3" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      <path className="sessionArchiveIcon__box" d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      <path className="sessionArchiveIcon__line" d="M10 12h4" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function WorkspaceFolderIcon() {
  return (
    <svg className="recentsProject__icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M3.75 7.25A2.25 2.25 0 0 1 6 5h4.15l2 2H18a2.25 2.25 0 0 1 2.25 2.25v7.5A2.25 2.25 0 0 1 18 19H6a2.25 2.25 0 0 1-2.25-2.25v-9.5Z"
        fill="currentColor"
      />
    </svg>
  );
}

function WorkspacePinIcon({ filled = false }: { filled?: boolean }) {
  return (
    <svg className={filled ? "recentsProject__pin recentsProject__pin--filled" : "recentsProject__pin"} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      {filled ? (
        <path
          fill="currentColor"
          d="m14.5 3.05l6.45 6.45l-1.41 1.41l-.7-.7l-2.94 2.94l-1.4 3.76l-2 2l-3.5-3.5l-3.79 3.79l-.71-.7l3.79-3.8l-4.2-4.2l2-2l3.76-1.4l2.94-2.94l-.7-.7z"
        />
      ) : (
        <path
          d="m15 4.5l-4 4L7 10l-1.5 1.5l7 7L14 17l1.5-4l4-4M9 15l-4.5 4.5M14.5 4L20 9.5"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
        />
      )}
    </svg>
  );
}

function WorkspaceMoreIcon() {
  return (
    <svg className="recentsProject__moreIcon" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="5" cy="12" r="1.6" />
      <circle cx="12" cy="12" r="1.6" />
      <circle cx="19" cy="12" r="1.6" />
    </svg>
  );
}

function sessionFallbackWorkspaceLabel(section: SidebarSessionItem["section"]): string {
  if (section === "webexplorer") return "WebExplorer";
  if (section === "banger") return "Banger";
  if (section === "trading") return "Forge Trading";
  if (section === "real-estate") return "Forge Immo";
  if (section === "alpha") return "Alpha";
  if (section === "shell") return "Shell";
  return "Forge";
}

function sessionWorkspaceLabel(item: SidebarSessionItem): string {
  return item.workspaceLabel?.trim() || sessionFallbackWorkspaceLabel(item.section);
}

type WorkspaceSessionGroup = { key: string; label: string; items: SidebarSessionItem[] };
const PINNED_WORKSPACE_STORAGE_KEY = "ingen.sidebar.sessions.pinnedWorkspace.v1";

function readPinnedWorkspaceKey(): string {
  if (typeof window === "undefined") {
    return "";
  }
  try {
    return window.localStorage.getItem(PINNED_WORKSPACE_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

function persistPinnedWorkspaceKey(key: string): void {
  try {
    if (key === "") {
      window.localStorage.removeItem(PINNED_WORKSPACE_STORAGE_KEY);
      return;
    }
    window.localStorage.setItem(PINNED_WORKSPACE_STORAGE_KEY, key);
  } catch {
    // Best effort: pinned workspace is a local sidebar preference.
  }
}

function groupSessionsByWorkspace(items: SidebarSessionItem[]): WorkspaceSessionGroup[] {
  const groups = new Map<string, { key: string; label: string; items: SidebarSessionItem[] }>();
  for (const item of items) {
    if (!item.rowVisible) continue;
    const label = sessionWorkspaceLabel(item);
    const key = label.toLocaleLowerCase();
    const group = groups.get(key);
    if (group) {
      group.items.push(item);
    } else {
      groups.set(key, { key, label, items: [item] });
    }
  }
  return [...groups.values()];
}

function orderWorkspaceGroups(groups: WorkspaceSessionGroup[], pinnedWorkspaceKey: string): WorkspaceSessionGroup[] {
  if (pinnedWorkspaceKey === "") {
    return groups;
  }
  return [...groups].sort((a, b) => {
    if (a.key === pinnedWorkspaceKey) return -1;
    if (b.key === pinnedWorkspaceKey) return 1;
    return 0;
  });
}

function SessionRow({
  item,
  selected,
  onOpen,
  onArchive
}: {
  item: SidebarSessionItem;
  selected: boolean;
  onOpen: () => void;
  onArchive: () => void;
}) {
  const [archiving, setArchiving] = useState(false);

  if (!item.rowVisible) return null;

  const archive = () => {
    if (archiving) return;
    setArchiving(true);
    window.setTimeout(onArchive, 220);
  };

  return (
    <div className={[
      "sessionRow",
      selected ? "sessionRow--selected" : "",
      item.working ? "sessionRow--working" : "",
      archiving ? "sessionRow--archiving" : ""
    ].filter(Boolean).join(" ")} role="listitem">
      <button
        type="button"
        className="sessionRow__main"
        onClick={onOpen}
        aria-label={item.working ? `Open ${item.label}, working` : `Open ${item.label}`}
      >
        <span className="sessionRow__label">{item.label}</span>
      </button>
      {item.working ? (
        <span className="sessionRow__workStatus" aria-label={`${item.label} is working`} role="status">
          <span className="sessionRow__loaderViewbox" aria-hidden="true">
            <span className="loader" />
          </span>
        </span>
      ) : (
        <button type="button" className="sessionRow__archive" aria-label={`Archive ${item.label}`} onClick={archive}>
          <ArchiveSessionIcon />
        </button>
      )}
    </div>
  );
}

function SessionsCanvas({
  items,
  archivedItems,
  mode
}: {
  items: SidebarSessionItem[];
  archivedItems: SidebarSessionItem[];
  mode: "recents" | "archived";
}) {
  const list = mode === "archived" ? archivedItems : items;
  const groups = groupSessionsByWorkspace(list);
  return (
    <section id="sessions-menu" className="sessionsCanvas" aria-label="Sessions menu">
      <div className="sessionsCanvas__inner">
        <h1>Sessions</h1>
        <div className="sessionsCanvas__tabs" role="tablist" aria-label="Session lists">
          <button
            type="button"
            role="tab"
            aria-selected={mode === "recents"}
            onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "switch_sessions_mode", mode: "recents" }), "sessions-recents")}
          >
            Recents
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === "archived"}
            onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "switch_sessions_mode", mode: "archived" }), "sessions-archived")}
          >
            Archived
          </button>
        </div>
        <div className="sessionsCanvas__list" role="list">
          {groups.map((group) => (
            <section className="sessionsCanvasProject" key={`${mode}-${group.key}`} role="listitem" aria-label={`${group.label} sessions`}>
              <div className="sessionsCanvasProject__header">
                <WorkspaceFolderIcon />
                <span>{group.label}</span>
              </div>
              <div className="sessionsCanvasProject__sessions" role="list">
                {group.items.map((item) => (
                  <button
                    type="button"
                    className="sessionExplorerLine"
                    key={`${mode}-${item.sessionId || item.label}`}
                    onClick={() => {
                      if (item.sessionId) {
                        void dispatchSidebarCommand(
                          sidebarShadowStore.command({ kind: "open_session", sessionId: item.sessionId, section: item.section }),
                          `sessions-${item.sessionId}`
                        );
                      } else {
                        void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "navigate", section: item.section }), `sessions-${item.label}`);
                      }
                    }}
                  >
                    <span>{item.label}</span>
                    <code>{item.date} / {item.section}</code>
                  </button>
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </section>
  );
}

type LlmProviderId = "codex" | "claude" | "openrouter";
type LlmTerminalPersistedState = {
  schema: "ingen.llm_provider_terminal.v1";
  selectedProvider: LlmProviderId;
  activeProvider: LlmProviderId | "";
  providers: Partial<Record<LlmProviderId, LlmProviderRuntimeEvent>>;
};

const LLM_TERMINAL_STORAGE_KEY = "ingen.llmProviderTerminal.v1";

const LLM_PROVIDER_PENDING: Record<LlmProviderId, string[]> = {
  codex: [
    "request OpenAI subscription OAuth",
    "waiting native OAuth window"
  ],
  claude: [
    "request Claude Code CLI login",
    "open Claude Code auth login",
    "waiting Claude Code auth status",
    "waiting official Claude account session",
    "not ready"
  ],
  openrouter: [
    "request OpenRouter OAuth PKCE",
    "open official OpenRouter auth flow",
    "waiting local callback and credential seal",
    "waiting eve_reader confirmation",
    "not ready"
  ]
};

const LLM_PROVIDER_BRIDGE_MISSING = [
  "native bridge unavailable",
  "restart Electron shell or use packaged app",
  "not ready"
];

const LLM_PROVIDER_META: Record<LlmProviderId, {
  tab: string;
  title: string;
  logoProvider: "openai" | "anthropic" | "openrouter";
  cliLabel: string;
  version?: string;
  command: string;
  auth: string;
  plan: string;
}> = {
  codex: {
    tab: "Codex",
    title: "Codex OpenAI",
    logoProvider: "openai",
    cliLabel: "OAuth Direct",
    command: "OpenAI subscription OAuth",
    auth: "ChatGPT OAuth",
    plan: "ChatGPT subscription access"
  },
  claude: {
    tab: "Claude",
    title: "Claude Code",
    logoProvider: "anthropic",
    cliLabel: "Claude Code CLI",
    version: "v0.1.0",
    command: "claude auth login",
    auth: "official CLI login",
    plan: "Claude account session"
  },
  openrouter: {
    tab: "OpenRouter",
    title: "OpenRouter",
    logoProvider: "openrouter",
    cliLabel: "OpenRouter OAuth",
    version: "v0.1.0",
    command: "OpenRouter OAuth PKCE",
    auth: "OAuth PKCE",
    plan: "provider routing key"
  }
};

function llmEventClassName(line: string): string {
  if (line === "ready") {
    return "llmTerminal__ready";
  }
  if (line.includes("confirmed")) {
    return "llmTerminal__confirm";
  }
  if (line === "failed" || line === "not ready" || line.includes("unavailable") || line.includes("failed")) {
    return "llmTerminal__danger";
  }
  return "llmTerminal__eventText";
}

function isLlmProviderId(value: unknown): value is LlmProviderId {
  return value === "codex" || value === "claude" || value === "openrouter";
}

function sanitizeLlmRuntimeEvent(value: unknown): LlmProviderRuntimeEvent | undefined {
  if (!value || typeof value !== "object") return undefined;
  const candidate = value as Partial<LlmProviderRuntimeEvent>;
  if (!isLlmProviderId(candidate.provider)) return undefined;
  const events = Array.isArray(candidate.events) ? candidate.events.filter((line): line is string => typeof line === "string" && line.trim().length > 0) : [];
  return {
    provider: candidate.provider,
    events: events.length > 0 ? events : ["awaiting secure login"],
    models: Array.isArray(candidate.models) ? candidate.models.filter((line): line is string => typeof line === "string" && line.trim().length > 0) : [],
    reasoning: Array.isArray(candidate.reasoning) ? candidate.reasoning.filter((line): line is string => typeof line === "string" && line.trim().length > 0) : [],
    quotaLabel: typeof candidate.quotaLabel === "string" ? candidate.quotaLabel : "",
    proofHash: typeof candidate.proofHash === "string" ? candidate.proofHash : "visual-terminal-store"
  };
}

function readPersistedLlmTerminal(): LlmTerminalPersistedState | undefined {
  try {
    const raw = window.localStorage.getItem(LLM_TERMINAL_STORAGE_KEY);
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as Partial<LlmTerminalPersistedState>;
    if (parsed.schema !== "ingen.llm_provider_terminal.v1" || !isLlmProviderId(parsed.selectedProvider)) {
      return undefined;
    }
    const providers: Partial<Record<LlmProviderId, LlmProviderRuntimeEvent>> = {};
    for (const provider of Object.keys(LLM_PROVIDER_META) as LlmProviderId[]) {
      const event = sanitizeLlmRuntimeEvent(parsed.providers?.[provider]);
      if (event) {
        providers[provider] = event;
      }
    }
    return {
      schema: "ingen.llm_provider_terminal.v1",
      selectedProvider: parsed.selectedProvider,
      activeProvider: isLlmProviderId(parsed.activeProvider) ? parsed.activeProvider : "",
      providers
    };
  } catch {
    return undefined;
  }
}

function persistLlmTerminal(state: LlmTerminalPersistedState): void {
  try {
    window.localStorage.setItem(LLM_TERMINAL_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Visual state persistence is best effort; provider secrets are never stored here.
  }
}

function LlmTerminalScreen({
  provider,
  active,
  events,
  step,
  launched,
  connected,
  onLogin,
  onReset
}: {
  provider: LlmProviderId;
  active: boolean;
  events: string[];
  step: number;
  launched: boolean;
  connected: boolean;
  onLogin: (provider: LlmProviderId) => void;
  onReset: (provider: LlmProviderId) => void;
}) {
  const meta = LLM_PROVIDER_META[provider];
  const lines = active ? events.slice(0, step) : ["awaiting secure login"];
  return (
    <div className="llmTerminal__screen" role="log" aria-live="polite" aria-label={`${meta.title} terminal`}>
      <div className="llmTerminal__mast">
        <span className={`llmTerminal__providerLogo llmTerminal__providerLogo--${provider}`} aria-hidden="true">
          <ProviderLogo provider={meta.logoProvider} size={18} />
        </span>
        <div className="llmTerminal__identity">
          <p><strong>{meta.title}</strong> <span className="llmTerminal__muted">{meta.cliLabel}{meta.version ? ` ${meta.version}` : ""}</span></p>
          <p><span className="llmTerminal__keyword">{meta.auth}</span> <span className="llmTerminal__muted">/ {meta.plan}</span></p>
        </div>
      </div>
      <p className="llmTerminal__guard"><span className="llmTerminal__dim">guard</span> renderer never stores secrets; native credential path owns the session.</p>
      <div className="llmTerminal__actions">
        <button
          type="button"
          className="llmTerminal__reset"
          onClick={() => onReset(provider)}
        >
          <span className="llmTerminal__prompt">&gt;</span>
          <span>reset</span>
        </button>
      </div>
      <button
        type="button"
        className={[
          "llmTerminal__login",
          launched ? "llmTerminal__login--launched" : "",
          connected ? "llmTerminal__login--connected" : ""
        ].filter(Boolean).join(" ")}
        disabled={connected}
        onClick={() => {
          if (!connected) {
            onLogin(provider);
          }
        }}
      >
        <span className="llmTerminal__prompt">&gt;</span>
        <span className={connected ? "llmTerminal__keyword llmTerminal__connectedWord" : "llmTerminal__keyword llmTerminal__loginWord"}>
          {connected ? "connected" : "login"}
        </span>
        <span>{connected ? `${meta.title} session persisted` : meta.command}</span>
      </button>
      <div className="llmTerminal__events">
        {lines.map((line) => (
          <p key={`${provider}-${line}`}>
            <span className="llmTerminal__eventPrompt">&gt;</span>
            <span className={llmEventClassName(line)}>{line}</span>
          </p>
        ))}
      </div>
    </div>
  );
}

function LlmProviderTerminal() {
  const persistedTerminal = readPersistedLlmTerminal();
  const [selectedProvider, setSelectedProvider] = useState<LlmProviderId>(persistedTerminal?.selectedProvider ?? "codex");
  const [activeProvider, setActiveProvider] = useState<LlmProviderId | "">(persistedTerminal?.activeProvider ?? "");
  const [providerEvents, setProviderEvents] = useState<Partial<Record<LlmProviderId, LlmProviderRuntimeEvent>>>(persistedTerminal?.providers ?? {});
  const [step, setStep] = useState(() => {
    const provider = persistedTerminal?.providers?.[persistedTerminal.selectedProvider ?? "codex"];
    return provider?.events.includes("ready") ? provider.events.length : 0;
  });

  useEffect(() => {
    const applyProviderEvent = (event: LlmProviderRuntimeEvent, select = true) => {
      if (select) {
        setActiveProvider(event.provider);
      }
      if (select) {
        setSelectedProvider(event.provider);
      }
      setProviderEvents((current) => ({
        ...current,
        [event.provider]: {
          ...event,
          events: event.events.includes("ready") || event.events.some((line) => line.includes("reset"))
            ? event.events
            : Array.from(new Set([...(current[event.provider]?.events ?? []), ...event.events]))
        }
      }));
      setStep(event.events.includes("ready") ? event.events.length : 0);
    };

    void window.forgeShell?.getLlmProviderRuntimeSnapshot?.().then((snapshot) => {
      setProviderEvents((current) => {
        const merged: Partial<Record<LlmProviderId, LlmProviderRuntimeEvent>> = { ...current };
        for (const provider of Object.keys(snapshot) as LlmProviderId[]) {
          const incoming = snapshot[provider];
          merged[provider] = incoming;
        }
        const connected = (Object.values(merged) as LlmProviderRuntimeEvent[]).filter((event) => event.events.includes("ready"));
        if (connected.length > 0) {
          const preferred = merged.codex?.events.includes("ready") ? merged.codex : connected[0];
          setActiveProvider(preferred.provider);
          setSelectedProvider(preferred.provider);
          setStep(preferred.events.length);
        }
        return merged;
      });
    });

    if (!window.forgeShell?.onLlmProviderEvent) return;
    return window.forgeShell.onLlmProviderEvent((event) => {
      const shouldSelect = event.events.includes("ready") || !event.events.every((line) => line === "awaiting secure login");
      applyProviderEvent(event, shouldSelect);
    });
  }, []);

  useEffect(() => {
    persistLlmTerminal({
      schema: "ingen.llm_provider_terminal.v1",
      selectedProvider,
      activeProvider,
      providers: providerEvents
    });
  }, [activeProvider, providerEvents, selectedProvider]);

  const selectedRuntime = providerEvents[selectedProvider];
  const visibleEvents = selectedRuntime?.events ?? LLM_PROVIDER_PENDING[selectedProvider];
  const selectedConnected = visibleEvents.includes("ready");
  const selectedIsActive = Boolean(selectedRuntime) || activeProvider === selectedProvider;

  useEffect(() => {
    if (!selectedIsActive) return;
    const total = visibleEvents.length;
    if (step >= total) return;
    const timer = window.setTimeout(() => setStep((value) => Math.min(value + 1, total)), step === 0 ? 80 : 520);
    return () => window.clearTimeout(timer);
  }, [selectedIsActive, visibleEvents.length, step]);

  const login = async (provider: LlmProviderId) => {
    setActiveProvider(provider);
    setProviderEvents((current) => ({
      ...current,
      [provider]: {
        provider,
        events: [LLM_PROVIDER_PENDING[provider][0]],
        models: [],
        reasoning: [],
        quotaLabel: "pending",
        proofHash: "pending"
      }
    }));
    setStep(0);
    if (!window.forgeShell?.connectLlmProvider) {
      setProviderEvents((current) => ({
        ...current,
        [provider]: {
          provider,
          events: LLM_PROVIDER_BRIDGE_MISSING,
          models: [],
          reasoning: [],
          quotaLabel: "unavailable",
          proofHash: "bridge-missing"
        }
      }));
      setStep(0);
      return;
    }
    const result = await window.forgeShell.connectLlmProvider(provider).catch(() => undefined);
    if (!result) {
      setProviderEvents((current) => ({
        ...current,
        [provider]: {
          provider,
          events: ["native provider launch failed", "not ready"],
          models: [],
          reasoning: [],
          quotaLabel: "unavailable",
          proofHash: "launch-failed"
        }
      }));
      setStep(0);
      return;
    }
    setProviderEvents((current) => ({
      ...current,
      [provider]: {
        provider,
        events: result.accepted ? result.events : [...result.events, "failed", "not ready"],
        models: result.models,
        reasoning: result.reasoning,
        quotaLabel: result.quotaLabel,
        proofHash: result.proofHash
      }
    }));
    setStep(0);
  };

  const resetProvider = async (provider: LlmProviderId) => {
    const resetEvent: LlmProviderRuntimeEvent = {
      provider,
      events: [`${LLM_PROVIDER_META[provider].tab} reset requested`, "awaiting secure login"],
      models: [],
      reasoning: [],
      quotaLabel: "reset pending",
      proofHash: "renderer-reset"
    };
    setActiveProvider(provider);
    setSelectedProvider(provider);
    setProviderEvents((current) => ({
      ...current,
      [provider]: resetEvent
    }));
    persistLlmTerminal({
      schema: "ingen.llm_provider_terminal.v1",
      selectedProvider: provider,
      activeProvider: provider,
      providers: {
        ...providerEvents,
        [provider]: resetEvent
      }
    });
    setStep(0);
    const result = await window.forgeShell?.resetLlmProvider?.(provider).catch(() => undefined);
    if (!result) {
      setProviderEvents((current) => ({
        ...current,
        [provider]: {
          provider,
          events: [`${LLM_PROVIDER_META[provider].tab} reset bridge unavailable`, "not ready"],
          models: [],
          reasoning: [],
          quotaLabel: "unavailable",
          proofHash: "reset-bridge-missing"
        }
      }));
      return;
    }
    setProviderEvents((current) => ({
      ...current,
      [provider]: {
        provider,
        events: result.events,
        models: result.models,
        reasoning: result.reasoning,
        quotaLabel: result.quotaLabel,
        proofHash: result.proofHash
      }
    }));
    setStep(0);
  };

  const selectProvider = (provider: LlmProviderId) => {
    setSelectedProvider(provider);
    setActiveProvider(providerEvents[provider] ? provider : "");
    setStep(0);
  };

  return (
    <section className="profileCanvas profileCanvas--llm" aria-label="LLM Provider terminal">
      <div className="llmTerminal">
        <div className="llmTerminal__tabs" role="tablist" aria-label="LLM provider tabs">
          {(Object.keys(LLM_PROVIDER_META) as LlmProviderId[]).map((provider) => (
            <button
              type="button"
              role="tab"
              aria-selected={selectedProvider === provider}
              className={selectedProvider === provider ? "llmTerminal__tab llmTerminal__tab--active" : "llmTerminal__tab"}
              key={provider}
              onClick={() => selectProvider(provider)}
            >
              <span className={`llmTerminal__tabLogo llmTerminal__tabLogo--${provider}`} aria-hidden="true">
                <ProviderLogo provider={LLM_PROVIDER_META[provider].logoProvider} size={14} />
              </span>
              <span>{LLM_PROVIDER_META[provider].tab}</span>
            </button>
          ))}
        </div>
        <LlmTerminalScreen
          provider={selectedProvider}
          active={selectedIsActive}
          events={visibleEvents}
          step={step}
          launched={activeProvider === selectedProvider}
          connected={selectedConnected}
          onLogin={login}
          onReset={resetProvider}
        />
      </div>
    </section>
  );
}

function ProfileCanvasSurface({ canvas }: { canvas: ProfileCanvas }) {
  if (canvas === "llm") {
    return <LlmProviderTerminal />;
  }
  if (canvas !== "profile") return null;
  return (
    <section className="profileCanvas" aria-label="Profile canvas">
      <div className="profileCanvas__banner">
        <ProfileCoverBanner />
      </div>
      <div className="profileCanvas__avatar">QH</div>
      <div className="profileCanvas__identity">
        <h1>@Quentin</h1>
        <p>Paris, France / OpenClaw / Member since June 2026 / // MY HUMAN / @ quentin@ingen.local</p>
        <div className="profileCanvas__stats">
          <span><strong>128</strong> connections</span>
          <span><strong>42</strong> completed pools</span>
          <span><strong>7</strong> IP shares</span>
        </div>
      </div>
      <div className="profileCanvas__grid">
        <article><code>// Projects</code><h2>Forge native profile canvas</h2><p>Repo surface kept as an Electron product route.</p></article>
        <article><code>// Specialty</code><h2>Agentic OS / verified compute</h2><p>Godel verification, Forge bytecode, Monster execution and local native projection.</p></article>
        <article><code>// Pool Activity</code><h2>Native front cutover</h2><p>Autonomous / proof ready.</p></article>
      </div>
    </section>
  );
}

async function dispatchSidebarCommand(
  command: ReturnType<typeof sidebarShadowStore.command>,
  focusTarget: string,
  syncHeader = false
) {
  await sidebarShadowStore.dispatch(command, focusTarget);
  if (command.kind === "open_session") {
    await panelsChatBottomStore.refresh();
  }
  if (syncHeader) {
    await headerShadowStore.boot();
  }
}

const MODULE_LOGO_FRAME = 32;
// Every logo covers the same optical area of the shared frame regardless of its
// aspect ratio, so a wide wordmark (Uber Eats) and a tall glyph (Airbnb) read
// as the same size everywhere the logo is rendered.
const MODULE_LOGO_AREA = MODULE_LOGO_FRAME * MODULE_LOGO_FRAME * 0.62;

function ModuleLogoFrame({
  viewBox,
  children,
  ...innerSvgProps
}: { viewBox: string; children: React.ReactNode } & React.SVGProps<SVGSVGElement>) {
  const [minX, minY, width, height] = viewBox.split(" ").map(Number);
  void minX;
  void minY;
  const scale = Math.min(
    Math.sqrt(MODULE_LOGO_AREA / (width * height)),
    MODULE_LOGO_FRAME / width,
    MODULE_LOGO_FRAME / height
  );
  const drawnWidth = width * scale;
  const drawnHeight = height * scale;
  return (
    <svg
      className="sidebarModule__logo"
      viewBox={`0 0 ${MODULE_LOGO_FRAME} ${MODULE_LOGO_FRAME}`}
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <svg
        x={(MODULE_LOGO_FRAME - drawnWidth) / 2}
        y={(MODULE_LOGO_FRAME - drawnHeight) / 2}
        width={drawnWidth}
        height={drawnHeight}
        viewBox={viewBox}
        preserveAspectRatio="xMidYMid meet"
        overflow="visible"
        {...innerSvgProps}
      >
        {children}
      </svg>
    </svg>
  );
}

function GmailIcon() {
  return (
    <ModuleLogoFrame viewBox="0 0 256 193">
      <path fill="#4285F4" d="M58.182 192.05V93.14L27.507 65.077L0 49.504v125.091c0 9.658 7.825 17.455 17.455 17.455h40.727Z" />
      <path fill="#34A853" d="M197.818 192.05h40.727c9.659 0 17.455-7.826 17.455-17.455V49.505l-31.156 17.837l-27.026 25.798v98.91Z" />
      <path fill="#EA4335" d="m58.182 93.14l-4.174-38.647l4.174-36.989L128 69.868l69.818-52.364l4.669 34.992l-4.669 40.644L128 145.504z" />
      <path fill="#FBBC04" d="M197.818 17.504V93.14L256 49.504V26.231c0-21.585-24.64-33.89-41.89-20.945l-16.292 12.218Z" />
      <path fill="#C5221F" d="m0 49.504l26.759 20.07L58.182 93.14V17.504L41.89 5.286C24.61-7.66 0 4.646 0 26.23v23.273Z" />
    </ModuleLogoFrame>
  );
}

function ModuleOpenIcon() {
  return (
    <svg className="sidebarAction__open" width={16} height={16} viewBox="0 0 24 24" aria-hidden="true">
      <path
        fill="none"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        d="M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4m-8-2l8-8m0 0v5m0-5h-5"
      />
    </svg>
  );
}

function OutlookIcon() {
  return (
    <ModuleLogoFrame viewBox="2 2 28 28">
      <path fill="#0072c6" d="M19.484 7.937v5.477l1.916 1.205a.489.489 0 0 0 .21 0l8.238-5.554a1.174 1.174 0 0 0-.959-1.128Z" />
      <path fill="#0072c6" d="m19.484 15.457l1.747 1.2a.522.522 0 0 0 .543 0c-.3.181 8.073-5.378 8.073-5.378v10.066a1.408 1.408 0 0 1-1.49 1.555h-8.874v-7.443Zm-9.044-2.525a1.609 1.609 0 0 0-1.42.838a4.131 4.131 0 0 0-.526 2.218A4.05 4.05 0 0 0 9.02 18.2a1.6 1.6 0 0 0 2.771.022a4.014 4.014 0 0 0 .515-2.2a4.369 4.369 0 0 0-.5-2.281a1.536 1.536 0 0 0-1.366-.809Z" />
      <path fill="#0072c6" d="M2.153 5.155v21.427L18.453 30V2Zm10.908 14.336a3.231 3.231 0 0 1-2.7 1.361a3.19 3.19 0 0 1-2.64-1.318A5.459 5.459 0 0 1 6.706 16.1a5.868 5.868 0 0 1 1.036-3.616a3.267 3.267 0 0 1 2.744-1.384a3.116 3.116 0 0 1 2.61 1.321a5.639 5.639 0 0 1 1 3.484a5.763 5.763 0 0 1-1.035 3.586Z" />
    </ModuleLogoFrame>
  );
}

function AmazonIcon() {
  return (
    <ModuleLogoFrame viewBox="0 0 32 32" fillRule="evenodd">
      <path fill="#f90" d="M28.312 28.26C25.003 30.7 20.208 32 16.08 32c-5.8 0-11.002-2.14-14.945-5.703-.3-.28-.032-.662.34-.444C5.73 28.33 11 29.82 16.426 29.82a29.73 29.73 0 0 0 11.406-2.332c.56-.238 1.03.367.48.773m1.376-1.575c-.42-.54-2.796-.255-3.86-.13-.325.04-.374-.243-.082-.446 1.9-1.33 4.994-.947 5.356-.5s-.094 3.56-1.87 5.044c-.273.228-.533.107-.4-.196.4-.996 1.294-3.23.87-3.772" />
      <path fill="#ededed" d="M18.43 13.864c0 1.692.043 3.103-.812 4.605-.7 1.22-1.8 1.973-3.005 1.973-1.667 0-2.644-1.27-2.644-3.145 0-3.7 3.316-4.373 6.462-4.373v.94m4.38 10.584c-.287.257-.702.275-1.026.104-1.44-1.197-1.704-1.753-2.492-2.895-2.382 2.43-4.074 3.157-7.158 3.157-3.658 0-6.498-2.254-6.498-6.767 0-3.524 1.905-5.924 4.63-7.097 2.357-1.038 5.65-1.22 8.165-1.5V8.9c0-1.032.08-2.254-.53-3.145-.525-.8-1.54-1.13-2.437-1.13-1.655 0-3.127.85-3.487 2.608-.073.4-.36.776-.757.794L7 7.555c-.354-.08-.75-.366-.647-.9C7.328 1.54 11.945 0 16.074 0c2.113 0 4.874.562 6.54 2.162 2.113 1.973 1.912 4.605 1.912 7.47V16.4c0 2.034.843 2.925 1.637 4.025.275.4.336.86-.018 1.154a184.26 184.26 0 0 0-3.328 2.883l-.006-.012" />
    </ModuleLogoFrame>
  );
}

function UberEatsIcon() {
  return (
    <ModuleLogoFrame viewBox="0 0 510 360">
      <g>
        <path
          fill="#ededed"
          d="m496 66.7v-22.3h-8.5c-13.5 0-23.5 6.2-29.5 15.9v-15h-24.2v121.1h24.4v-68.8c0-18.8 11.6-30.9 27.6-30.9zm-175.6 28c4.4-18.5 19.6-30.9 37.7-30.9s33.4 12.3 37.5 30.9zm38.2-51.7c-36 0-63.4 28.7-63.4 62.9 0 36.1 28.5 63.2 65.6 63.2 22.5 0 40.9-9.7 53.2-25.9l-17.7-12.8c-9.2 12.1-21.3 17.8-35.6 17.8-20.8 0-37.5-14.7-40.9-34.4h100.4v-7.8c.1-36.2-26-63-61.6-63m-139.6 105.2c-23.7 0-42.6-18.8-42.6-42 0-23.5 19.1-42 42.6-42 23.2 0 42.3 18.5 42.3 42 .1 23.2-19 42-42.3 42m-66.5 18.3h24.2v-15.2c11.1 11.2 26.9 18 44 18 36.3 0 64.8-28.3 64.8-63.2 0-35.1-28.5-63.4-64.8-63.4-17.2 0-32.7 6.9-43.8 18v-60.5h-24.4zm-85.9-19.5c23.5 0 41.6-17.8 41.6-44.2v-102.6h25.4v166.2h-25.2v-15.4c-11.4 11.6-27.1 18.3-44.8 18.3-36.3 0-64.1-25.9-64.1-65.1v-104h25.5v102.6c0 26.9 17.9 44.2 41.6 44.2"
        />
      </g>
      <g transform="translate(-547 190)">
        <path
          fill="#06c167"
          d="m787.3 105.2c0-21-16.8-37.5-38-37.5-20.9 0-38 16.5-38 37.5s17.1 37.5 38 37.5c21.2 0 38-16.5 38-37.5m31-61.4v122.9h-31.6v-11.1c-11 9.1-24.9 14.2-40 14.2-37.4 0-66.6-28.7-66.6-64.6 0-35.8 29.3-64.6 66.6-64.6 15.1 0 29 5.1 40 14.2v-11zm105 95h-23.8c-7.2 0-11.9-3.1-11.9-9.7v-57.5h35.6v-27.8h-35.6v-35h-31.9v35h-24v27.9h24v65.3c0 16.5 11.9 29.6 33.3 29.6h34.2v-27.8zm72 31c36.5 0 57.1-17.1 57.1-40.7 0-16.8-12.2-29.3-37.7-34.7l-27-5.4c-15.6-2.8-20.6-5.7-20.6-11.4 0-7.4 7.5-11.9 21.4-11.9 15.1 0 26.1 4 29.3 17.6h31.6c-1.7-25.6-20.6-42.7-58.8-42.7-33 0-56.2 13.4-56.2 39.3 0 17.9 12.8 29.6 40.3 35.3l30.1 6.8c11.9 2.3 15.1 5.4 15.1 10.2 0 7.7-9 12.5-23.5 12.5-18.2 0-28.7-4-32.7-17.6h-31.9c4.7 25.6 24.1 42.7 63.5 42.7m-442.7-169.6h119.1v28.4h-86.9v40.7h84.6v27.6h-84.6v41.2h86.9v28.4h-119.1z"
        />
      </g>
    </ModuleLogoFrame>
  );
}

function AirbnbIcon() {
  return (
    <ModuleLogoFrame viewBox="0 0 256 275">
      <path
        fill="#FF385C"
        d="M252.154 194.867c-1.231-3.456-2.67-6.8-4.039-9.898c-2.107-4.766-4.314-9.541-6.449-14.157l-.169-.366c-19.04-41.23-39.475-83.026-60.738-124.222l-.903-1.75c-2.169-4.206-4.411-8.556-6.712-12.83a83.351 83.351 0 0 0-9.875-15.198a45.98 45.98 0 0 0-15.808-12.133a46.072 46.072 0 0 0-38.935.005a45.976 45.976 0 0 0-15.804 12.137a83.712 83.712 0 0 0-9.87 15.195c-2.32 4.313-4.584 8.703-6.773 12.949l-.838 1.625c-21.264 41.2-41.699 82.994-60.738 124.221l-.278.6c-2.098 4.54-4.267 9.236-6.339 13.922c-1.37 3.096-2.806 6.437-4.039 9.902a60.699 60.699 0 0 0-3.274 29.588a58.455 58.455 0 0 0 11.835 27.646a58.603 58.603 0 0 0 24.027 18.129a59.593 59.593 0 0 0 22.481 4.349c2.42 0 4.839-.142 7.243-.422a73.906 73.906 0 0 0 27.645-9.327c11.152-6.265 22.165-15.446 34.196-28.566c12.031 13.12 23.044 22.301 34.196 28.566a73.89 73.89 0 0 0 27.645 9.327a62.62 62.62 0 0 0 7.244.422a59.586 59.586 0 0 0 22.48-4.349a58.609 58.609 0 0 0 24.027-18.13a58.453 58.453 0 0 0 11.836-27.645a60.752 60.752 0 0 0-3.274-29.59ZM128 209.17c-14.893-18.878-24.45-36.409-27.804-51.106a45.195 45.195 0 0 1-.956-16.85a27.533 27.533 0 0 1 4.432-11.52a30.688 30.688 0 0 1 10.772-8.802a30.762 30.762 0 0 1 27.116.002a30.685 30.685 0 0 1 10.77 8.803a27.548 27.548 0 0 1 4.432 11.522a45.21 45.21 0 0 1-.96 16.856C152.444 172.77 142.89 190.296 128 209.17Zm110.032 12.802a40.874 40.874 0 0 1-8.275 19.33a40.977 40.977 0 0 1-16.8 12.677a42.823 42.823 0 0 1-21.088 2.758a55.703 55.703 0 0 1-21.055-7.191c-9.926-5.577-19.974-14.138-31.28-26.696c17.999-22.195 29.239-42.652 33.4-60.873a62.51 62.51 0 0 0 1.197-23.421a44.91 44.91 0 0 0-7.307-18.776a48.223 48.223 0 0 0-17.075-14.405a48.313 48.313 0 0 0-43.495-.002a48.205 48.205 0 0 0-17.075 14.403a44.91 44.91 0 0 0-7.308 18.771a62.535 62.535 0 0 0 1.19 23.412c4.16 18.229 15.4 38.69 33.406 60.892c-11.307 12.557-21.355 21.118-31.281 26.696a55.7 55.7 0 0 1-21.055 7.19a42.827 42.827 0 0 1-21.089-2.758a40.978 40.978 0 0 1-16.8-12.677a40.872 40.872 0 0 1-8.273-19.33a43.049 43.049 0 0 1 2.437-21.231c.983-2.761 2.132-5.471 3.556-8.69c2.015-4.555 4.153-9.185 6.221-13.661l.278-.602C49.394 136.792 69.716 95.23 90.864 54.255l.842-1.631c2.153-4.178 4.38-8.497 6.626-12.67a67.774 67.774 0 0 1 7.758-12.115a28.411 28.411 0 0 1 9.8-7.594a28.462 28.462 0 0 1 34.015 7.59a67.46 67.46 0 0 1 7.76 12.111c2.225 4.136 4.432 8.416 6.567 12.555l.904 1.756c21.147 40.97 41.469 82.531 60.404 123.535l.17.369c2.104 4.552 4.28 9.257 6.328 13.891c1.426 3.224 2.577 5.936 3.557 8.687a43.081 43.081 0 0 1 2.437 21.233Z"
      />
    </ModuleLogoFrame>
  );
}

function WhatsappIcon() {
  return (
    <ModuleLogoFrame viewBox="0 0 24 24">
      <path
        fill="#25D366"
        d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 0 1-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 0 1-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 0 1 2.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0 0 12.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 0 0 5.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 0 0-3.48-8.413"
      />
    </ModuleLogoFrame>
  );
}

function CubeIcon() {
  return (
    <ModuleLogoFrame
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
      <path d="m3.27 6.96 8.73 5.05 8.73-5.05" />
      <path d="M12 22.08V12" />
    </ModuleLogoFrame>
  );
}

function PoolLadderIcon() {
  return (
    <svg className="sidebarPoolCard__logo" viewBox="0 -0.5 17 17" aria-hidden="true">
      <g fill="currentColor">
        <path d="M3.168 9.982c-.891 0-1.607-.338-1.968-.624c-.199-.159-.225-.441-.059-.634c.167-.189.461-.214.66-.059c.053.041 1.26.946 2.87-.032c2.147-1.308 4.494-.063 4.594-.011c.018.009 1.94 1.013 3.675.007c2.269-1.32 3.867-.011 3.933.047c.194.164.214.447.041.634c-.175.188-.472.203-.665.039c-.041-.035-1.154-.919-2.82.048c-2.214 1.284-4.531.06-4.629.008c-.02-.012-1.958-1.031-3.625-.013c-.712.434-1.401.59-2.007.59Z" />
        <path d="M3.175 12.994c-1.148 0-1.91-.566-1.957-.604c-.208-.158-.249-.455-.091-.662c.158-.209.451-.248.657-.094c.057.042 1.279.919 2.903-.031c2.132-1.246 4.462-.062 4.561-.011c.018.009 1.958.984 3.708.007c2.254-1.258 3.833-.009 3.9.045c.202.165.234.463.071.666c-.162.205-.459.235-.662.072c-.043-.034-1.171-.892-2.853.045c-2.195 1.225-4.498.058-4.598.007c-.018-.009-1.973-.998-3.654-.011c-.732.427-1.407.571-1.985.571Z" />
        <path d="M11.217 15.986c-1.326.001-2.353-.547-2.417-.584c-.019-.01-1.957-1.037-3.623-.011c-1.763 1.082-3.373.452-3.976-.034c-.2-.16-.226-.445-.06-.639c.167-.19.461-.219.66-.059c.053.041 1.26.954 2.87-.033c2.147-1.32 4.494-.065 4.594-.011c.019.009 1.94 1.022 3.675.007c2.269-1.331 3.867-.009 3.933.046c.194.167.214.453.041.641c-.175.188-.472.204-.665.04c-.041-.037-1.154-.926-2.82.047c-.757.444-1.525.59-2.212.59Z" />
        <path d="M10.31 4.972H4.665c-.276 0-.5-.21-.5-.469s.224-.469.5-.469h5.645c.276 0 .5.21.5.469s-.224.469-.5.469Z" />
        <path d="M10.31 7H4.665c-.276 0-.5-.224-.5-.5s.224-.5.5-.5h5.645c.276 0 .5.224.5.5s-.224.5-.5.5Z" />
        <path d="M4.485 9.708c-.276 0-.454-.224-.454-.5V2.281c-.033-.048-.379-.344-.888-.344h-.251c-.51 0-.81.296-.849.373c0 .276-.221.485-.497.485s-.498-.238-.498-.515c0-.704.793-1.234 1.844-1.234h.251c1.05 0 1.842.53 1.842 1.234v6.927c0 .277-.223.501-.5.501Z" />
        <path d="M10.5 9.729c-.276 0-.5-.224-.5-.5V2.248c-.033-.048-.256-.311-.766-.311h-.252c-.509 0-.886.263-.925.34c0 .276-.221.485-.497.485c-.277 0-.497-.238-.497-.515c0-.704.792-1.234 1.843-1.234h.251c1.051 0 1.843.53 1.843 1.234v6.981c0 .278-.224.501-.5.501Z" />
      </g>
    </svg>
  );
}

function PoolSwimIcon() {
  return (
    <svg className="sidebarPoolCard__logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M11 13c.5.5 2.13-.112 3.262-.5C15.722 12 17.5 12.5 17 12c-1.639-1.639-2-2-2-2s-4.5 2.5-4 3ZM2 20c2 0 3-1 5-1s3 1 5 1s3-1 5-1s3 1 5 1M2 16c2 0 3-1 5-1s3 1 5 1s3-1 5-1s3 1 5 1M17.5 4l-5.278 3L15.5 10.5L12 12M19.222 10a1 1 0 1 0 0-2a1 1 0 0 0 0 2Z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
      />
    </svg>
  );
}

function PoolSearchAgentsIcon() {
  return (
    <svg className="sidebarPoolCard__logo" viewBox="0 0 24 24" aria-hidden="true">
      <path
        fill="currentColor"
        d="M1 20v-2.8q0-.85.438-1.562T2.6 14.55q1.55-.775 3.15-1.162T9 13q.8 0 1.613.088t1.612.287q-.325.425-.562.888t-.388.962q-.575-.125-1.137-.175T9 15q-1.4 0-2.775.338T3.5 16.35q-.225.125-.363.35T3 17.2v.8h8.075q.1.55.275 1.05t.45.95zm8-8q-1.65 0-2.825-1.175T5 8t1.175-2.825T9 4t2.825 1.175T13 8t-1.175 2.825T9 12m10-4q0 1.65-1.175 2.825T15 12q-.275 0-.7-.062t-.7-.138q.675-.8 1.038-1.775T15 8t-.362-2.025T13.6 4.2q.35-.125.7-.163T15 4q1.65 0 2.825 1.175T19 8M9 10q.825 0 1.413-.587T11 8t-.587-1.412T9 6t-1.412.588T7 8t.588 1.413T9 10m8 9q.85 0 1.413-.5T19 17q.025-.85-.562-1.425T17 15t-1.425.575T15 17t.575 1.425T17 19m0 2q-1.65 0-2.825-1.175T13 17t1.175-2.825T17 13t2.825 1.175T21 17q0 .575-.137 1.088t-.413.962L23 21.6L21.6 23l-2.55-2.55q-.45.275-.962.413T17 21"
      />
    </svg>
  );
}

function PoolChatIcon() {
  return (
    <svg className="sidebarPoolCard__logo" viewBox="0 0 520 512" aria-hidden="true">
      <path
        fill="currentColor"
        d="M463 226q6 20 6 36q0 63-68 107l-17 15q4 64 9 77q-12-6-54-36q-18-20-38-20h-64q-9 9-43 22q53 21 107 21q1 0 19 16t48.5 32t60.5 16q7 0 9-11t0-26.5t-4.5-31.5t-5.5-27l-3-11q89-56 89-143q0-67-43-113q-1 7-.5 20t-1 31.5T463 226zM79 427q30 0 62-21.5t51.5-43T213 341q90 0 152-46t62-118q0-73-62.5-125T213 0Q125 0 62.5 52T0 177q0 86 90 143q-2 7-5.5 18T75 372.5t-5.5 39T79 427zM43 177q0-56 49.5-95T213 43t121 39t50 95t-51 89t-120 33q-22 0-42 25q-35 35-52 45q1-2 11-36l11-32l-28-17q-70-44-70-107z"
      />
    </svg>
  );
}

function PoolDrawer() {
  const openCreatePool = () => {
    void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Create pool" }), "pool-create");
  };
  const openJoinPool = () => {
    void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Join a pool" }), "pool-join");
  };
  const openSearchAgents = () => {
    void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Search agents" }), "pool-search-agents");
  };
  const openChat = () => {
    void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Chat" }), "pool-chat");
  };

  return (
    <div className="sidebarPoolCards" role="list">
      <button type="button" className="sidebarPoolCard" role="listitem" onClick={openCreatePool}>
        <PoolLadderIcon />
        <span>Create pool</span>
      </button>
      <button type="button" className="sidebarPoolCard" role="listitem" onClick={openJoinPool}>
        <PoolSwimIcon />
        <span>Join a pool</span>
      </button>
      <button type="button" className="sidebarPoolCard" role="listitem" onClick={openSearchAgents}>
        <PoolSearchAgentsIcon />
        <span>Search agents</span>
      </button>
      <button type="button" className="sidebarPoolCard" role="listitem" onClick={openChat}>
        <PoolChatIcon />
        <span>Chat</span>
      </button>
    </div>
  );
}

function DrawerEditButton({
  active = false,
  onClick
}: {
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button type="button" className="sidebarDrawer__edit" aria-label="Edit modules" aria-pressed={active} onClick={onClick}>
      <svg width={15} height={15} viewBox="0 0 24 24" aria-hidden="true">
        <path
          fill="currentColor"
          d="M15.748 2.947a2 2 0 0 1 2.828 0l2.475 2.475a2 2 0 0 1 0 2.829L9.158 20.144l-6.38 1.076l1.077-6.38L15.748 2.947Zm-.229 3.057l2.475 2.475l1.643-1.643l-2.475-2.474l-1.643 1.642Zm1.06 3.89l-2.474-2.475l-8.384 8.384l-.503 2.977l2.977-.502l8.385-8.385Z"
        />
      </svg>
    </button>
  );
}

const PLACEHOLDER_MODULES = Array.from({ length: 9 }, (_, index) => index + 1);
const MODULE_ORDER_STORAGE_KEY = "ingen.sidebar.modules.order.v2";
const MODULE_HIDDEN_STORAGE_KEY = "ingen.sidebar.modules.hidden.v1";
const moduleCollisionDetection: CollisionDetection = (args) => {
  const pointerCollisions = pointerWithin(args);
  return pointerCollisions.length > 0 ? pointerCollisions : closestCenter(args);
};

const SIDEBAR_MODULES = [
  { id: "gmail", label: "gmail" },
  { id: "outlook", label: "outlook" },
  { id: "amazon", label: "amazon" },
  { id: "uber-eats", label: "uber eats" },
  { id: "airbnb", label: "airbnb" },
  { id: "whatsapp", label: "whatsapp" },
  ...PLACEHOLDER_MODULES.map((index) => ({ id: `module-${index}`, label: "module" }))
] as const;

export type SidebarModuleId = (typeof SIDEBAR_MODULES)[number]["id"];
type SidebarModuleDefinition = (typeof SIDEBAR_MODULES)[number];

export const MODULE_DRAG_ZONE_EVENT = "ingen:module-drag-zone";

export type ModuleDragZoneDetail =
  | { phase: "start"; moduleId: SidebarModuleId }
  | { phase: "move"; over: boolean }
  | { phase: "end" };

function emitModuleDragZone(detail: ModuleDragZoneDetail) {
  window.dispatchEvent(new CustomEvent<ModuleDragZoneDetail>(MODULE_DRAG_ZONE_EVENT, { detail }));
}

function dragClientPoint(activatorEvent: Event, delta: { x: number; y: number }): { x: number; y: number } | null {
  if (!(activatorEvent instanceof MouseEvent)) {
    return null;
  }
  return { x: activatorEvent.clientX + delta.x, y: activatorEvent.clientY + delta.y };
}

// The chat/canvas slice covers the whole drop surface: transcript canvas + composer.
function pointInModuleDropZone(point: { x: number; y: number } | null): boolean {
  if (!point) {
    return false;
  }
  const zone = document.querySelector(".panelsChatBottom");
  if (!zone) {
    return false;
  }
  const rect = zone.getBoundingClientRect();
  return rect.width > 0 && point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom;
}

function defaultModuleOrder(): SidebarModuleId[] {
  return SIDEBAR_MODULES.map((module) => module.id);
}

function normalizeModuleOrder(ids: unknown): SidebarModuleId[] {
  const defaultOrder = defaultModuleOrder();
  const validIds = new Set(defaultOrder);
  const incoming = Array.isArray(ids)
    ? ids.filter((id): id is SidebarModuleId => typeof id === "string" && validIds.has(id as SidebarModuleId))
    : [];
  return [...incoming, ...defaultOrder.filter((id) => !incoming.includes(id))];
}

function readModuleOrder(): SidebarModuleId[] {
  if (typeof window === "undefined") {
    return defaultModuleOrder();
  }
  try {
    return normalizeModuleOrder(JSON.parse(window.localStorage.getItem(MODULE_ORDER_STORAGE_KEY) ?? "[]"));
  } catch {
    return defaultModuleOrder();
  }
}

function persistModuleOrder(ids: SidebarModuleId[]): void {
  try {
    window.localStorage.setItem(MODULE_ORDER_STORAGE_KEY, JSON.stringify(ids));
  } catch {
    // Best effort: module order is a local UI preference.
  }
}

function readHiddenModuleIds(): SidebarModuleId[] {
  if (typeof window === "undefined") {
    return [];
  }
  try {
    const validIds = new Set(defaultModuleOrder());
    const ids = JSON.parse(window.localStorage.getItem(MODULE_HIDDEN_STORAGE_KEY) ?? "[]");
    return Array.isArray(ids)
      ? ids.filter((id): id is SidebarModuleId => typeof id === "string" && validIds.has(id as SidebarModuleId))
      : [];
  } catch {
    return [];
  }
}

function persistHiddenModuleIds(ids: SidebarModuleId[]): void {
  try {
    window.localStorage.setItem(MODULE_HIDDEN_STORAGE_KEY, JSON.stringify(ids));
  } catch {
    // Best effort: hidden modules are a local UI preference.
  }
}

export function ModuleLogo({ id }: { id: SidebarModuleId }) {
  if (id === "gmail") return <GmailIcon />;
  if (id === "outlook") return <OutlookIcon />;
  if (id === "amazon") return <AmazonIcon />;
  if (id === "uber-eats") return <UberEatsIcon />;
  if (id === "airbnb") return <AirbnbIcon />;
  if (id === "whatsapp") return <WhatsappIcon />;
  return <CubeIcon />;
}

function ModuleButton({
  module,
  className,
  style,
  listeners,
  attributes,
  setNodeRef,
  editing = false,
  onRemove,
  onSelect
}: {
  module: SidebarModuleDefinition;
  className: string;
  style?: React.CSSProperties;
  listeners?: DraggableSyntheticListeners;
  attributes?: React.HTMLAttributes<HTMLDivElement>;
  setNodeRef?: (node: HTMLDivElement | null) => void;
  editing?: boolean;
  onRemove?: (id: SidebarModuleId) => void;
  onSelect?: (id: SidebarModuleId) => void;
}) {
  const stopRemoveActivation = (event: React.PointerEvent<HTMLButtonElement> | React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
  };

  return (
    <div
      ref={setNodeRef}
      className={className}
      style={style}
      role="listitem"
      {...attributes}
      {...listeners}
      onClick={editing || !onSelect ? undefined : () => onSelect(module.id)}
    >
      <span className="sidebarModule__logoFrame">
        <ModuleLogo id={module.id} />
        {editing ? (
          <button
            type="button"
            className="sidebarModule__remove"
            aria-label={`Remove ${module.label}`}
            onPointerDown={stopRemoveActivation}
            onMouseDown={stopRemoveActivation}
            onClick={(event) => {
              event.stopPropagation();
              onRemove?.(module.id);
            }}
          >
            <svg viewBox="0 0 10 10" aria-hidden="true">
              <path d="M2 2l6 6M8 2L2 8" />
            </svg>
          </button>
        ) : null}
      </span>
      <span className="sidebarModule__label">{module.label}</span>
    </div>
  );
}

function SortableModuleButton({ module, onSelect }: { module: SidebarModuleDefinition; onSelect?: (id: SidebarModuleId) => void }) {
  const {
    attributes,
    isDragging,
    listeners,
    setNodeRef,
    transform,
    transition
  } = useSortable({
    id: module.id,
    transition: {
      duration: 290,
      easing: "cubic-bezier(0.16, 1, 0.3, 1)"
    }
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition
  };

  return (
    <ModuleButton
      module={module}
      className={isDragging ? "sidebarModule sidebarModule--dragging" : "sidebarModule"}
      style={style}
      attributes={attributes as React.HTMLAttributes<HTMLDivElement>}
      listeners={listeners}
      setNodeRef={setNodeRef}
      onSelect={isDragging ? undefined : onSelect}
    />
  );
}

function ModulesDrawer({
  onModuleSelect,
  onModuleDrop
}: {
  onModuleSelect?: (id: SidebarModuleId) => void;
  onModuleDrop?: (id: SidebarModuleId) => void;
}) {
  const [moduleOrder, setModuleOrder] = useState(readModuleOrder);
  const [hiddenModuleIds, setHiddenModuleIds] = useState(readHiddenModuleIds);
  const [editingModules, setEditingModules] = useState(false);
  const [activeModuleId, setActiveModuleId] = useState<SidebarModuleId | null>(null);
  const moduleById = new Map(SIDEBAR_MODULES.map((module) => [module.id, module]));
  const hiddenModuleIdSet = new Set(hiddenModuleIds);
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 7
      }
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates
    })
  );
  const visibleModuleOrder = moduleOrder.filter((id) => !hiddenModuleIdSet.has(id));
  const orderedModules = visibleModuleOrder.map((id) => moduleById.get(id)).filter((module): module is SidebarModuleDefinition => Boolean(module));
  const activeModule = activeModuleId ? moduleById.get(activeModuleId) : undefined;

  const handleDragStart = (event: DragStartEvent) => {
    if (editingModules) {
      return;
    }
    setActiveModuleId(event.active.id as SidebarModuleId);
    emitModuleDragZone({ phase: "start", moduleId: event.active.id as SidebarModuleId });
  };

  const handleDragMove = (event: DragMoveEvent) => {
    emitModuleDragZone({
      phase: "move",
      over: pointInModuleDropZone(dragClientPoint(event.activatorEvent, event.delta))
    });
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveModuleId(null);
    emitModuleDragZone({ phase: "end" });
    if (pointInModuleDropZone(dragClientPoint(event.activatorEvent, event.delta))) {
      onModuleDrop?.(active.id as SidebarModuleId);
      return;
    }
    if (!over || active.id === over.id) {
      return;
    }
    setModuleOrder((current) => {
      const visible = current.filter((id) => !hiddenModuleIdSet.has(id));
      const oldIndex = visible.indexOf(active.id as SidebarModuleId);
      const newIndex = visible.indexOf(over.id as SidebarModuleId);
      if (oldIndex < 0 || newIndex < 0) {
        return current;
      }
      const nextVisible = arrayMove(visible, oldIndex, newIndex);
      const next = [...nextVisible, ...current.filter((id) => hiddenModuleIdSet.has(id))];
      persistModuleOrder(next);
      return next;
    });
  };

  const handleDragCancel = () => {
    setActiveModuleId(null);
    emitModuleDragZone({ phase: "end" });
  };

  const removeModule = (id: SidebarModuleId) => {
    setHiddenModuleIds((current) => {
      if (current.includes(id)) {
        return current;
      }
      const next = [...current, id];
      persistHiddenModuleIds(next);
      return next;
    });
    setActiveModuleId((current) => (current === id ? null : current));
  };

  if (editingModules) {
    return (
      <>
        <div className="sidebarModules sidebarModules--editing" role="list">
          {orderedModules.map((module) => (
            <ModuleButton
              module={module}
              key={module.id}
              className="sidebarModule sidebarModule--editing"
              editing
              onRemove={removeModule}
            />
          ))}
        </div>
        <DrawerEditButton active onClick={() => setEditingModules(false)} />
      </>
    );
  }

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={moduleCollisionDetection}
        onDragStart={handleDragStart}
        onDragMove={handleDragMove}
        onDragEnd={handleDragEnd}
        onDragCancel={handleDragCancel}
      >
        <SortableContext items={visibleModuleOrder} strategy={rectSortingStrategy}>
          <div className="sidebarModules" role="list">
            {orderedModules.map((module) => (
              <SortableModuleButton module={module} key={module.id} onSelect={onModuleSelect} />
            ))}
          </div>
        </SortableContext>
        {/* Portaled: .leftPanel is overflow:hidden + transformed, which would clip
            and offset the fixed overlay once the drag leaves the sidebar. */}
        {createPortal(
          <DragOverlay adjustScale={false} modifiers={[snapCenterToCursor]}>
            {activeModule ? (
              <ModuleButton module={activeModule} className="sidebarModule sidebarModule--overlay" />
            ) : null}
          </DragOverlay>,
          document.body
        )}
      </DndContext>
      <DrawerEditButton onClick={() => setEditingModules(true)} />
    </>
  );
}

export function SidebarSlice({
  open,
  onNewSession,
  onModuleSelect,
  onModuleDrop
}: {
  open: boolean;
  onNewSession?: () => void;
  onModuleSelect?: (id: SidebarModuleId) => void;
  onModuleDrop?: (id: SidebarModuleId) => void;
}) {
  const { snapshot } = useSidebarShadowStore();
  const [pinnedWorkspaceKey, setPinnedWorkspaceKey] = useState(readPinnedWorkspaceKey);
  const [workspaceMenuKey, setWorkspaceMenuKey] = useState("");
  const visibleRecent = snapshot.recentItems.filter((item) => item.rowVisible);
  const recentWorkspaceGroups = orderWorkspaceGroups(groupSessionsByWorkspace(visibleRecent), pinnedWorkspaceKey);
  const drawerOpen = snapshot.toolControls.some((tool) => tool.drawer !== "" && tool.selected);
  const togglePinnedWorkspace = (key: string) => {
    const next = pinnedWorkspaceKey === key ? "" : key;
    setPinnedWorkspaceKey(next);
    persistPinnedWorkspaceKey(next);
    setWorkspaceMenuKey("");
  };
  const archiveWorkspace = (group: WorkspaceSessionGroup) => {
    setWorkspaceMenuKey("");
    void (async () => {
      for (const item of group.items) {
        const id = sessionArchiveId(item);
        await sidebarShadowStore.archiveSession(id, `archive-${id}`);
      }
    })();
  };

  return (
    <>
      <aside
        id="left-panel"
        className={drawerOpen ? "leftPanel leftPanel--drawer-open" : "leftPanel"}
        aria-label="Sidebar and sessions"
        aria-hidden={!open}
      >
          <nav className="sidebarTools" aria-label="Primary sidebar tools">
            {snapshot.toolControls.filter((tool) => tool.visible).map((tool) => {
              const command =
                tool.id === "new-session"
                  ? sidebarShadowStore.command({ kind: "navigate", section: "forge" })
                  : tool.id === "brain"
                    ? sidebarShadowStore.command({ kind: "open_profile_canvas", canvas: "brain" })
                    : tool.id === "automations"
                      ? sidebarShadowStore.command({ kind: "activate_control", label: "Automations" })
                      : sidebarShadowStore.command({ kind: "set_active_drawer", drawer: tool.drawer });
              const hasDrawer = tool.drawer !== "";
              return (
                <Fragment key={tool.id}>
                  <button
                    type="button"
                    className="sidebarAction"
                    aria-label={tool.label}
                    aria-expanded={hasDrawer ? tool.selected : undefined}
                    onClick={() => {
                      if (tool.id === "new-session") {
                        onNewSession?.();
                      }
                      void dispatchSidebarCommand(command, tool.id, command.kind === "navigate" || command.kind === "open_profile_canvas");
                    }}
                  >
                    <SidebarIcon tool={tool} />
                    <span>{tool.label}</span>
                    {hasDrawer && tool.selected ? <ModuleOpenIcon /> : null}
                  </button>
                  {hasDrawer ? (
                    <div
                      className={[
                        "sidebarDrawer",
                        tool.id === "modules" ? "sidebarDrawer--modules" : "",
                        tool.selected ? "sidebarDrawer--open" : "",
                      ].filter(Boolean).join(" ")}
                      role="region"
                      aria-label={`${tool.label} panel`}
                      aria-hidden={!tool.selected}
                    >
                      {tool.id === "pool" ? <PoolDrawer /> : null}
                      {tool.id === "modules" ? <ModulesDrawer onModuleSelect={onModuleSelect} onModuleDrop={onModuleDrop} /> : null}
                    </div>
                  ) : null}
                </Fragment>
              );
            })}
          </nav>

          <section className="recentsPanel" aria-label="Projects">
            <h2>Projects</h2>
            <div className="recentsPanel__list" role="list">
              {recentWorkspaceGroups.map((group) => (
                <section className="recentsProject" key={group.key} role="listitem" aria-label={`${group.label} sessions`}>
                  <div className="recentsProject__header">
                    {pinnedWorkspaceKey === group.key ? (
                      <button
                        type="button"
                        className="recentsProject__pinButton"
                        aria-label={`Unpin ${group.label}`}
                        onClick={() => togglePinnedWorkspace(group.key)}
                      >
                        <WorkspacePinIcon filled />
                      </button>
                    ) : (
                      <span className="recentsProject__pinSlot" aria-hidden="true" />
                    )}
                    <WorkspaceFolderIcon />
                    <span>{group.label}</span>
                    <button
                      type="button"
                      className="recentsProject__menuButton"
                      aria-label={`${group.label} project actions`}
                      aria-haspopup="menu"
                      aria-expanded={workspaceMenuKey === group.key}
                      onClick={() => setWorkspaceMenuKey((current) => (current === group.key ? "" : group.key))}
                    >
                      <WorkspaceMoreIcon />
                    </button>
                    {workspaceMenuKey === group.key ? (
                      <div className="projectMiniMenu" role="menu" aria-label={`${group.label} project menu`}>
                        <button type="button" role="menuitem" onClick={() => togglePinnedWorkspace(group.key)}>
                          <WorkspacePinIcon />
                          <span>{pinnedWorkspaceKey === group.key ? "Unpin project" : "Pin project"}</span>
                        </button>
                        <button type="button" role="menuitem" onClick={() => archiveWorkspace(group)}>
                          <ArchiveSessionIcon />
                          <span>Archive project</span>
                        </button>
                      </div>
                    ) : null}
                  </div>
                  <div className="recentsProject__sessions" role="list">
                    {group.items.map((item) => (
                      <SessionRow
                        key={item.sessionId || item.label}
                        item={item}
                        selected={item.sessionId !== "" && item.sessionId === snapshot.recentSessionId}
                        onOpen={() => {
                          if (item.sessionId) {
                            void dispatchSidebarCommand(
                              sidebarShadowStore.command({ kind: "open_session", sessionId: item.sessionId, section: item.section }),
                              item.sessionId,
                              true
                            );
                          } else {
                            void dispatchSidebarCommand(sidebarShadowStore.command({ kind: "navigate", section: item.section }), item.label, true);
                          }
                        }}
                        onArchive={() => {
                          const id = sessionArchiveId(item);
                          void sidebarShadowStore.archiveSession(id, `archive-${id}`);
                        }}
                      />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </section>

          {snapshot.archiveConfirm.open ? (
            <div className="archiveConfirm" role="dialog" aria-label="Archive this session?">
              <strong>Archive this session?</strong>
              <span>{snapshot.archiveConfirm.candidateLabel}</span>
              <div>
                <button type="button" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "cancel_archive" }), "archive-cancel")}>Cancel</button>
                <button type="button" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "confirm_archive" }), "archive-confirm")}>Yes, archive</button>
              </div>
            </div>
          ) : null}

          <section className="profileDock" aria-label="Account menu">
            {snapshot.profileOpen ? (
              <div className="userMenu" role="menu" aria-label="User menu">
                <div className="userMenu__header">
                  <span className="userMenu__username">@Quentin</span>
                  <span className="userMenu__email">quentin@ingen.local</span>
                </div>
                <button type="button" className="userMenu__item" role="menuitem" onClick={() => void dispatchSidebarCommand(sidebarShadowStore.command({ kind: "open_profile_canvas", canvas: "profile" }), "profile-profile", true)}>
                  <NavIcon kind="persona" /><span>Profile</span>
                </button>
                <div className="userMenu__divider" />
                <button type="button" className="userMenu__item userMenu__item--disabled" role="menuitem" disabled>
                  <NavIcon kind="wallet" /><span>Wallet</span><span className="userMenu__soon">soon</span>
                </button>
                <button type="button" className="userMenu__item" role="menuitem" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Contacts" }), "menu-contacts")}>
                  <NavIcon kind="contacts" /><span>Contacts</span>
                </button>
                <button
                  type="button"
                  className="userMenu__item"
                  role="menuitem"
                  onClick={() => void dispatchSidebarCommand(sidebarShadowStore.command({ kind: "open_profile_canvas", canvas: "llm" }), "menu-llm-providers", true)}
                >
                  <NavIcon kind="agent" /><span>LLM providers</span>
                </button>
                <button type="button" className="userMenu__item userMenu__item--disabled" role="menuitem" disabled>
                  <NavIcon kind="jobs" /><span>Market &amp; Jobs</span><span className="userMenu__soon">soon</span>
                </button>
                <button type="button" className="userMenu__item userMenu__item--disabled" role="menuitem" disabled>
                  <NavIcon kind="ip" /><span>IP Market</span><span className="userMenu__soon">soon</span>
                </button>
                <div className="userMenu__divider" />
                <div className="userMenu__item userMenu__lang" role="menuitem">
                  <NavIcon kind="globe" />
                  <span>Language</span>
                  <select className="userMenu__langSelect" defaultValue="en" aria-label="Select language">
                    <option value="en">English</option>
                    <option value="fr">French</option>
                    <option value="es">Spanish</option>
                    <option value="de">Deutsch</option>
                  </select>
                </div>
                <div className="userMenu__divider" />
                <button type="button" className="userMenu__item" role="menuitem" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Settings" }), "menu-settings")}>
                  <NavIcon kind="settings" /><span>Settings</span>
                </button>
                <button type="button" className="userMenu__item userMenu__item--danger" role="menuitem" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Log Out" }), "menu-logout")}>
                  <NavIcon kind="logout" /><span>Log Out</span>
                </button>
              </div>
            ) : null}

            <div className="navNotif" aria-label="Notifications">
              <button type="button" className="navNotif__btn" aria-label="Messages" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Messages" }), "nav-dm")}>
                <NavIcon kind="dm" />
              </button>
              <button type="button" className="navNotif__btn" aria-label="Friend requests" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Friends" }), "nav-friends")}>
                <NavIcon kind="friends" />
              </button>
              <button type="button" className="navNotif__btn" aria-label="Notifications" onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "activate_control", label: "Notifications" }), "nav-bell")}>
                <NavIcon kind="bell" />
              </button>
            </div>

            <button
              type="button"
              className="navAvatarBtn"
              aria-label="Toggle user menu"
              aria-expanded={snapshot.profileOpen}
              aria-haspopup="true"
              onClick={() => void sidebarShadowStore.dispatch(sidebarShadowStore.command({ kind: "toggle_profile_menu" }), "profile-menu")}
            >
              <span className="navAvatarBtn__initials">Q</span>
              <span className="navAvatarBtn__username">Quentin</span>
              <NavIcon kind="chevron" />
            </button>
          </section>
      </aside>

      {snapshot.profileCanvas === "sessions" ? (
        <SessionsCanvas items={snapshot.recentItems} archivedItems={snapshot.archivedItems} mode={snapshot.sessionsMenuMode} />
      ) : null}
      <ProfileCanvasSurface canvas={snapshot.profileCanvas} />
    </>
  );
}
