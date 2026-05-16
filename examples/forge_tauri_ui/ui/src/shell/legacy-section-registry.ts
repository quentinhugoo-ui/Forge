type LegacySectionDefinition = {
  readonly id?: string;
  readonly label?: string;
  readonly kind?: string;
  readonly parent?: string;
  readonly lazy?: boolean;
  readonly bootSafe?: boolean;
  readonly active?: boolean;
  readonly owns?: readonly string[];
};

type LegacySection = {
  id: string;
  label: string;
  kind: string;
  parent: string;
  lazy: boolean;
  bootSafe: boolean;
  active: boolean;
  owns: readonly string[];
  mounted: boolean;
  createdAtMs: number;
  updatedAtMs: number;
};

type QueuedAction = {
  readonly action: string;
  readonly payload: Record<string, unknown>;
  readonly createdAtMs: number;
};

declare global {
  interface Window {
    ForgeSectionRegistry?: {
      register(definition: LegacySectionDefinition): LegacySection;
      setActive(id: string, active?: boolean): LegacySection | null;
      activate(id: string): LegacySection | null;
      deactivate(id: string): LegacySection | null;
      setShellSection(id: string): string;
      isActive(id: string): boolean;
      markReady(): void;
      queueAction(id: string, action: string, payload?: Record<string, unknown>): QueuedAction | null;
      consumeQueuedActions(id: string): QueuedAction[];
      phase(): string;
      list(): LegacySection[];
      snapshot(): Record<string, unknown>;
    };
  }
}

const sections = new Map<string, LegacySection>();
const actionQueues = new Map<string, QueuedAction[]>();
let shellSection = "alpha";
let bootPhase = "boot";

function normalizeId(id: unknown): string {
  return String(id || "").trim().toLowerCase();
}

function nowMs(): number {
  return Date.now ? Date.now() : 0;
}

function cloneSection(section: LegacySection): LegacySection {
  return { ...section, owns: section.owns.slice() };
}

function register(definition: LegacySectionDefinition = {}): LegacySection {
  const id = normalizeId(definition.id);
  if (!id) throw new Error("ForgeSectionRegistry.register requires an id");
  const existing = sections.get(id);
  const next: LegacySection = {
    id,
    label: definition.label || existing?.label || id,
    kind: definition.kind || existing?.kind || "section",
    parent: definition.parent || existing?.parent || "",
    lazy: definition.lazy !== false,
    bootSafe: definition.bootSafe === true || existing?.bootSafe === true,
    active: definition.active === true || existing?.active === true,
    mounted: existing?.mounted === true,
    owns: Array.isArray(definition.owns) ? definition.owns.slice() : (existing?.owns || []),
    createdAtMs: existing?.createdAtMs || nowMs(),
    updatedAtMs: nowMs(),
  };
  sections.set(id, next);
  return cloneSection(next);
}

function setActive(id: string, active = true): LegacySection | null {
  const normalized = normalizeId(id);
  if (!normalized) return null;
  const section = sections.get(normalized) || register({ id: normalized });
  section.active = Boolean(active);
  section.updatedAtMs = nowMs();
  sections.set(normalized, section);
  return cloneSection(section);
}

function setShellSection(id: string): string {
  const normalized = normalizeId(id) || "alpha";
  shellSection = normalized;
  for (const section of sections.values()) {
    if (section.kind === "shell-section") {
      section.active = section.id === normalized;
      section.updatedAtMs = nowMs();
    }
  }
  if (sections.has(normalized)) setActive(normalized, true);
  return shellSection;
}

function isActive(id: string): boolean {
  const normalized = normalizeId(id);
  if (!normalized) return false;
  if (normalized === "shell") return true;
  if (normalized === shellSection) return true;
  return sections.get(normalized)?.active === true;
}

function markReady(): void {
  bootPhase = "ready";
}

function queueAction(id: string, action: string, payload: Record<string, unknown> = {}): QueuedAction | null {
  const normalized = normalizeId(id);
  const name = String(action || "").trim();
  if (!normalized || !name) return null;
  const queue = actionQueues.get(normalized) || [];
  const entry = { action: name, payload, createdAtMs: nowMs() };
  queue.push(entry);
  actionQueues.set(normalized, queue);
  return { ...entry, payload: { ...entry.payload } };
}

function consumeQueuedActions(id: string): QueuedAction[] {
  const normalized = normalizeId(id);
  if (!normalized) return [];
  const queue = actionQueues.get(normalized) || [];
  actionQueues.set(normalized, []);
  return queue.map((entry) => ({ ...entry, payload: { ...(entry.payload || {}) } }));
}

function list(): LegacySection[] {
  return Array.from(sections.values()).map(cloneSection);
}

function snapshot(): Record<string, unknown> {
  return {
    phase: bootPhase,
    shellSection,
    sections: list(),
    queuedActions: Object.fromEntries(Array.from(actionQueues.entries()).map(([id, queue]) => [id, queue.length])),
  };
}

window.ForgeSectionRegistry = Object.freeze({
  register,
  setActive,
  activate: (id: string) => setActive(id, true),
  deactivate: (id: string) => setActive(id, false),
  setShellSection,
  isActive,
  markReady,
  queueAction,
  consumeQueuedActions,
  phase: () => bootPhase,
  list,
  snapshot,
});

export {};
