export type ForgeShellMode = "forge" | "agence-immo";

export type ForgeSectionId =
  | "alpha"
  | "forge"
  | "webexplorer"
  | "real-estate"
  | "real-estate-main"
  | "trading"
  | "banger";

export type ForgeShellPhase = "boot" | "ready" | "error";

export type ForgeSectionKind = "shell-section" | "tool" | "surface";
export type ForgeSectionLifecycle = "idle" | "active" | "hidden";
export type ForgeSectionPermission =
  | "canvas"
  | "chatbar"
  | "left-panel"
  | "right-panel"
  | "native-window"
  | "network"
  | "jobs"
  | "hardware";

export interface ForgeSectionDefinition {
  readonly id: ForgeSectionId | string;
  readonly label: string;
  readonly kind: ForgeSectionKind;
  readonly parent?: ForgeSectionId | string;
  readonly bootSafe?: boolean;
  readonly owns?: readonly string[];
  readonly permissions?: readonly ForgeSectionPermission[];
  readonly commands?: readonly string[];
}

export interface ForgeSectionCell {
  readonly id: ForgeSectionId | string;
  readonly label: string;
  readonly kind: ForgeSectionKind;
  readonly parent: ForgeSectionId | string | null;
  readonly bootSafe: boolean;
  readonly owns: readonly string[];
  readonly permissions: readonly ForgeSectionPermission[];
  readonly commands: readonly string[];
  readonly lifecycle: ForgeSectionLifecycle;
  readonly active: boolean;
  readonly projection: Readonly<Record<string, unknown>>;
}

export interface ForgeShellState {
  readonly phase: ForgeShellPhase;
  readonly mode: ForgeShellMode;
  readonly activeSection: ForgeSectionId;
  readonly activeSections: Readonly<Record<string, boolean>>;
  readonly leftPanelCollapsed: boolean;
  readonly canvas: Readonly<Record<string, unknown>>;
  readonly chatbar: Readonly<Record<string, unknown>>;
  readonly rightPanel: Readonly<Record<string, unknown>>;
  readonly jobs: Readonly<Record<string, unknown>>;
  readonly hardware: unknown;
  readonly panels: Readonly<Record<string, boolean>>;
  readonly overlays: Readonly<Record<string, boolean>>;
  readonly window: {
    readonly lastCommand: string;
    readonly label: string;
  };
  readonly onboarding: {
    readonly scope: string;
    readonly status: "idle" | "initializing" | "asking" | "complete" | "error";
    readonly questionId: string;
  };
}

export interface ForgeKernelProjection {
  readonly seq: number;
  readonly shell?: {
    readonly seq?: number;
    readonly phase?: string;
    readonly mode?: ForgeShellMode | string;
    readonly lastEventKind?: string | null;
    readonly last_event_kind?: string | null;
  };
  readonly section?: {
    readonly active?: ForgeSectionId | string;
    readonly activeSection?: ForgeSectionId | string;
    readonly active_section?: ForgeSectionId | string;
    readonly activeSections?: readonly string[];
    readonly active_sections?: readonly string[];
  };
  readonly leftPanel?: {
    readonly collapsed?: boolean;
  };
  readonly left_panel?: {
    readonly collapsed?: boolean;
  };
  readonly canvas?: Readonly<Record<string, unknown>>;
  readonly chatbar?: Readonly<Record<string, unknown>>;
  readonly rightPanel?: Readonly<Record<string, unknown>>;
  readonly right_panel?: Readonly<Record<string, unknown>>;
  readonly jobs?: Readonly<Record<string, unknown>>;
  readonly hardware?: unknown;
  readonly mode: ForgeShellMode | string;
  readonly activeSection?: ForgeSectionId | string;
  readonly active_section?: ForgeSectionId | string;
  readonly activeSections?: readonly string[];
  readonly active_sections?: readonly string[];
  readonly leftPanelCollapsed?: boolean;
  readonly left_panel_collapsed?: boolean;
  readonly panels?: Readonly<Record<string, boolean>>;
  readonly overlays?: Readonly<Record<string, boolean>>;
  readonly lastWindowControl?: string | null;
  readonly last_window_control?: string | null;
  readonly lastWindowLabel?: string | null;
  readonly last_window_label?: string | null;
  readonly onboarding?: {
    readonly scope?: string;
    readonly status?: string;
    readonly questionId?: string;
    readonly question_id?: string;
  };
}

export type ForgeShellEvent =
  | { readonly type: "BOOT_READY" }
  | { readonly type: "BOOT_ERROR" }
  | { readonly type: "SET_MODE"; readonly mode: ForgeShellMode }
  | { readonly type: "SET_REAL_ESTATE_MODE"; readonly active: boolean; readonly webExplorerActive?: boolean }
  | { readonly type: "ACTIVATE_SECTION"; readonly section: ForgeSectionId }
  | { readonly type: "SET_SECTION_ACTIVE"; readonly section: ForgeSectionId | string; readonly active: boolean }
  | { readonly type: "SET_SURFACE_ACTIVE"; readonly section: ForgeSectionId; readonly active: boolean; readonly fallbackSection?: ForgeSectionId }
  | { readonly type: "TOGGLE_LEFT_PANEL" }
  | { readonly type: "SET_CANVAS"; readonly patch: Readonly<Record<string, unknown>> }
  | { readonly type: "SET_CHATBAR"; readonly patch: Readonly<Record<string, unknown>> }
  | { readonly type: "SET_RIGHT_PANEL"; readonly patch: Readonly<Record<string, unknown>> }
  | { readonly type: "SET_JOBS"; readonly patch: Readonly<Record<string, unknown>> }
  | { readonly type: "SET_HARDWARE"; readonly hardware: unknown }
  | { readonly type: "SET_PANEL"; readonly panel: string; readonly open: boolean }
  | { readonly type: "SET_OVERLAY"; readonly overlay: string; readonly open: boolean }
  | { readonly type: "SET_WINDOW_COMMAND"; readonly command: string; readonly label?: string }
  | { readonly type: "SET_ONBOARDING"; readonly scope: string; readonly status: ForgeShellState["onboarding"]["status"]; readonly questionId?: string };

export interface ForgeTauriClient {
  invoke<T = unknown>(command: string, args?: Record<string, unknown>, options?: Record<string, unknown>): Promise<T>;
  listen?(eventName: string, handler: (event: unknown) => void, options?: Record<string, unknown>): Promise<unknown>;
}

declare global {
  interface Window {
    ForgeSectionManifests?: readonly ForgeSectionDefinition[];
    ForgeSectionCells?: readonly ForgeSectionCell[];
    __forgeActiveSection?: () => ForgeSectionId | string;
    __forgeSwitchSection?: (section: ForgeSectionId | string) => void;
  }
}
