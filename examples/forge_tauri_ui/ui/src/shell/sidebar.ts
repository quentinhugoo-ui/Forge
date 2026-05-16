type ForgeSidebarDeps = {
  readonly runtime?: {
    registerAction?(name: "toggle-sidebar", handler: () => void): unknown;
  } | null;
  readonly getContent: () => Element | null;
  readonly getToggle: () => HTMLElement | null;
  readonly isFrench: () => boolean;
  readonly refreshLayout: () => void;
};

type ForgeSidebarCell = {
  install(deps: ForgeSidebarDeps): void;
  setCollapsed(collapsed: boolean): void;
  toggle(): void;
  syncLabels(): void;
  isCollapsed(): boolean;
};

declare global {
  interface Window {
    ForgeSidebarCell?: ForgeSidebarCell;
    __forgeSetSidebarCollapsed?: (collapsed: boolean) => void;
  }
}

let activeDeps: ForgeSidebarDeps | null = null;
let collapsed = false;

function label(): string {
  const french = activeDeps?.isFrench?.() === true;
  if (french) return collapsed ? "Afficher le panneau gauche" : "Masquer le panneau gauche";
  return collapsed ? "Show left panel" : "Hide left panel";
}

function render(options: { readonly refreshLayout: boolean }): void {
  const deps = activeDeps;
  const content = deps?.getContent?.() || null;
  const toggle = deps?.getToggle?.() || null;
  content?.classList.toggle("sidebar-collapsed", collapsed);
  toggle?.classList.toggle("collapsed", collapsed);
  toggle?.setAttribute("aria-expanded", String(!collapsed));
  const text = label();
  toggle?.setAttribute("aria-label", text);
  toggle?.setAttribute("title", text);
  if (options.refreshLayout) deps?.refreshLayout?.();
}

function setCollapsed(next: boolean): void {
  collapsed = next === true;
  render({ refreshLayout: true });
}

function toggle(): void {
  setCollapsed(!collapsed);
}

function syncLabels(): void {
  render({ refreshLayout: false });
}

function install(nextDeps: ForgeSidebarDeps): void {
  activeDeps = nextDeps;
  window.__forgeSetSidebarCollapsed = setCollapsed;
  nextDeps.runtime?.registerAction?.("toggle-sidebar", toggle);
  syncLabels();
}

window.ForgeSidebarCell = Object.freeze({
  install,
  setCollapsed,
  toggle,
  syncLabels,
  isCollapsed: () => collapsed,
});
