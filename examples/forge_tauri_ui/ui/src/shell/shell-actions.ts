export type ForgeShellActionName =
  | "toggle-sidebar"
  | "toggle-right-panel"
  | "close-right-panel"
  | "profile-toggle"
  | "profile-action"
  | "docs-close"
  | "nav-forge"
  | "nav-alpha"
  | "toggle-real-estate"
  | "real-estate-home"
  | "toggle-webexplorer"
  | "open-trading"
  | "toggle-banger"
  | "close-banger"
  | "open-search"
  | "close-search"
  | "toggle-search"
  | "real-estate-new-session"
  | "toggle-real-estate-tools"
  | "close-real-estate-tools"
  | "toggle-real-estate-contacts"
  | "close-real-estate-contacts"
  | "real-estate-automations"
  | "real-estate-properties"
  | "provider-close"
  | "provider-launch-codex"
  | "provider-launch-claude"
  | "provider-launch-oanda"
  | "provider-workbench-launch"
  | "provider-workbench-refresh"
  | "provider-refresh-all"
  | "provider-oanda-send"
  | "provider-oanda-reset"
  | "provider-openai-connect"
  | "provider-claude-connect"
  | "provider-claude-refresh"
  | "provider-voice-save-key"
  | "provider-voice-clear-key"
  | "library-toggle"
  | "library-close"
  | "ActCode-toggle"
  | "ActCode-close"
  | "programs-toggle"
  | "programs-close"
  | "program-create-toggle"
  | "program-create-cancel"
  | "atlas-refresh"
  | "alpha-3d-toggle"
  | "mars-lens-toggle"
  | "mars-lens-close"
  | "alpha-split-toggle"
  | "alpha-3d-new-map"
  | "library-filter"
  | "ActCode-filter"
  | "programs-filter"
  | "atlas-subtab"
  | "document-click"
  | "escape-overlays";

export interface ForgeShellActionPayload {
  readonly source?: string;
  readonly kind?: "click" | "shortcut";
  readonly key?: string;
  readonly dataset?: Readonly<Record<string, string>>;
  readonly target?: EventTarget | null;
}

export type ForgeShellActionHandler = (payload: ForgeShellActionPayload) => void;

export function createOverlayToggleAction(options: {
  readonly isOpen: () => boolean;
  readonly open: () => void;
  readonly close: () => void;
}): ForgeShellActionHandler {
  return () => {
    if (options.isOpen()) options.close();
    else options.open();
  };
}

export function createCloseAction(close: () => void): ForgeShellActionHandler {
  return () => close();
}

export function createAsyncAction(effect: () => Promise<unknown> | unknown): ForgeShellActionHandler {
  return () => {
    void effect();
  };
}

export function createToggleAction(options: {
  readonly isActive: () => boolean;
  readonly activate: () => void;
  readonly deactivate: () => void;
  readonly canToggle?: () => boolean;
}): ForgeShellActionHandler {
  return () => {
    if (options.canToggle && !options.canToggle()) return;
    if (options.isActive()) options.deactivate();
    else options.activate();
  };
}

export function createActionSequence(
  ...steps: ReadonlyArray<() => void>
): ForgeShellActionHandler {
  return () => {
    for (const step of steps) step();
  };
}

export function createDatasetFilterAction(options: {
  readonly selector: string;
  readonly readFilter?: (payload: ForgeShellActionPayload) => string;
  readonly setFilter: (filter: string) => void;
  readonly render: () => void;
}): ForgeShellActionHandler {
  return (payload) => {
    const filter = options.readFilter?.(payload) || String(payload?.dataset?.filter || "all");
    document.querySelectorAll<HTMLElement>(options.selector).forEach((button) => {
      button.classList.toggle("active", button.dataset.filter === filter);
    });
    options.setFilter(filter);
    options.render();
  };
}

export function bindTextFilterInput(
  input: HTMLInputElement | null,
  apply: (value: string) => void,
): void {
  input?.addEventListener("input", () => {
    apply(input.value);
  });
}

export function bindEventAction(
  fields: ReadonlyArray<EventTarget | null | undefined>,
  eventName: string,
  action: EventListener,
): void {
  fields
    .filter(Boolean)
    .forEach((field) => {
      field.addEventListener(eventName, action);
    });
}

export function bindEventActions(
  fields: ReadonlyArray<EventTarget | null | undefined>,
  eventNames: ReadonlyArray<string>,
  action: EventListener,
): void {
  for (const eventName of eventNames) {
    bindEventAction(fields, eventName, action);
  }
}

export function bindInputAction(
  fields: ReadonlyArray<EventTarget | null | undefined>,
  action: () => void,
): void {
  bindEventAction(fields, "input", action);
}

export function resetAndFocusTextInput(
  input: HTMLInputElement | HTMLTextAreaElement | null,
  reset?: () => void,
): void {
  if (!input) return;
  input.value = "";
  reset?.();
  input.focus();
}

export function bindEnterAction(
  input: HTMLInputElement | HTMLTextAreaElement | null,
  action: () => Promise<unknown> | unknown,
): void {
  input?.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") return;
    event.preventDefault();
    void action();
  });
}

export function bindInputLifecycle(
  fields: ReadonlyArray<EventTarget | null | undefined>,
  action: () => void,
): void {
  fields
    .filter(Boolean)
    .forEach((field) => {
      field.addEventListener("input", action);
      field.addEventListener("change", action);
      field.addEventListener("blur", action);
    });
}

export function bindClickAction(
  element: HTMLElement | null,
  action: (event: MouseEvent) => void,
): void {
  element?.addEventListener("click", (event) => {
    action(event);
  });
}

export function bindActivatableAction(
  element: HTMLElement | null,
  action: () => void,
): void {
  bindClickAction(element, (event) => {
    event.preventDefault();
    action();
  });
  element?.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    action();
  });
}

export function consumeEvent(event: Event): void {
  event.preventDefault();
  event.stopPropagation();
}

export function runFirstMatchingAction(
  candidates: ReadonlyArray<readonly [boolean, () => void]>,
): boolean {
  for (const [matches, action] of candidates) {
    if (!matches) continue;
    action();
    return true;
  }
  return false;
}

export function handleOutsideDismiss(
  candidates: ReadonlyArray<readonly [boolean, () => void]>,
): void {
  for (const [matches, action] of candidates) {
    if (!matches) continue;
    action();
  }
}

export function runMappedAction(
  key: string,
  actions: Readonly<Record<string, () => void>>,
): boolean {
  const action = actions[key];
  if (!action) return false;
  action();
  return true;
}

export function providerStatusTextBlob(status: unknown): string {
  const source = typeof status === "object" && status
    ? status as { authSource?: unknown; message?: unknown }
    : {};
  return `${String(source.authSource || "")}\n${String(source.message || "")}`.toLowerCase();
}

export function providerStatusNeedsNode(status: unknown): boolean {
  const source = typeof status === "object" && status
    ? status as { message?: unknown }
    : {};
  return /node\.js|npm/i.test(String(source.message || ""));
}

export function providerStatusNeedsRepair(status: unknown): boolean {
  return /found but not executable|found on disk, but forge could not execute|permissions|access denied|acces refuse/i
    .test(providerStatusTextBlob(status));
}

export function providerCliEffectiveInstalled(status: unknown): boolean {
  const source = typeof status === "object" && status
    ? status as { installed?: unknown; connected?: unknown }
    : {};
  return !!source.installed || !!source.connected || providerStatusNeedsRepair(status);
}

export function providerCliFriendlySource(kind: string, status: unknown): string {
  const source = typeof status === "object" && status ? status as { connected?: unknown } : {};
  const connected = !!source.connected;
  const installed = providerCliEffectiveInstalled(status);
  const blob = providerStatusTextBlob(status);
  if (kind === "codex") {
    return connected ? "OpenAI OAuth" : "browser sign-in";
  }
  if (connected) {
    if (kind === "claude") {
      if (blob.includes("oauth") || blob.includes("claude.ai")) return "Claude.ai login";
      if (blob.includes("credential")) return "saved local login";
      return "Claude ready";
    }
    return "OpenAI login";
  }
  if (providerStatusNeedsRepair(status)) return "automatic repair";
  if (installed) return "local setup";
  return "automatic setup";
}

export function providerCliFriendlyHint(
  kind: string,
  status: unknown,
  options: {
    readonly busy?: boolean;
    readonly selectedModel?: string;
    readonly displayName?: string;
  } = {},
): string {
  const source = typeof status === "object" && status ? status as { connected?: unknown } : {};
  const busy = !!options.busy;
  const connected = !!source.connected;
  const installed = providerCliEffectiveInstalled(status);
  const selectedModel = options.selectedModel || "";
  const display = options.displayName || kind || "Provider";
  const modelText = selectedModel ? ` Selected model: ${selectedModel}.` : "";
  if (kind === "codex") {
    if (busy) return `Checking OpenAI OAuth in Forge.${modelText}`;
    if (connected) return `OpenAI OAuth is ready for ${display} direct in Forge.${modelText}`;
    return `Open the embedded OAuth console to connect your ChatGPT subscription, then refresh Forge.${modelText}`;
  }
  if (busy) return `Checking ${display} in Forge.${modelText}`;
  if (connected) return `${display} is ready in Forge.${modelText}`;
  if (providerStatusNeedsRepair(status)) {
    return `${display} is being repaired automatically in Forge.${modelText}`;
  }
  if (providerStatusNeedsNode(status)) {
    return `Node.js is required before Forge can finish the automatic ${display} setup.${modelText}`;
  }
  if (installed) {
    return `${display} is preparing its local connection in Forge.${modelText}`;
  }
  return `${display} is preparing. Forge will install and connect it automatically when the environment is ready.${modelText}`;
}

export function providerWorkbenchStateText(status: unknown): string {
  const source = typeof status === "object" && status ? status as { connected?: unknown } : {};
  const connected = !!source.connected;
  const installed = providerCliEffectiveInstalled(status);
  if (connected) return "ready";
  if (installed) return "auth";
  return "missing";
}

export function runVisibleClosers(
  candidates: ReadonlyArray<readonly [boolean, () => void]>,
): void {
  for (const [visible, close] of candidates) {
    if (visible) close();
  }
}
