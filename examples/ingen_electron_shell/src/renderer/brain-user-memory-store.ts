export interface BrainUserMemorySlot {
  schema: "ingen.brain.memory.user_identity.v1";
  scope: "brain.memory.user.identity";
  stableKey: "user.identity.first_name";
  preferredFirstName: string;
  trust: "unset" | "seeded_profile" | "user_confirmed" | "llm_inferred_unverified";
  evidence: string;
}

export interface BrainAgentMemorySlot {
  schema: "ingen.brain.memory.agent_identity.v1";
  scope: "brain.memory.agent.identity";
  stableKey: "agent.identity.first_name";
  preferredFirstName: string;
  trust: "unset" | "seeded_product" | "user_confirmed";
  evidence: string;
}

export interface BrainUserLocationMemorySlot {
  schema: "ingen.brain.memory.user_location.v1";
  scope: "brain.memory.user.location";
  stableKey: "user.location.home";
  homeLocation: string;
  trust: "unset" | "user_confirmed" | "llm_inferred_unverified";
  evidence: string;
}

export type BrainLearningMemoryCategory = "lesson" | "skill" | "task";
type LegacyBrainLearningMemoryCategory = "anti_pattern" | "conduct_rule";
type BrainLearningMemoryStoredCategory = BrainLearningMemoryCategory | LegacyBrainLearningMemoryCategory;
export type BrainDurableCandidateSource = "manual" | "agent_learning_interrupt";

export interface BrainLearningMemoryEntry {
  schema: "ingen.brain.memory.learning_registry.v1";
  id: string;
  category: BrainLearningMemoryCategory;
  text: string;
  source: BrainDurableCandidateSource;
  trust: "user_confirmed" | "agent_candidate";
  evidence: string;
  createdAt: string;
  updatedAt: string;
}

export interface BrainCustomCodeActEntry {
  schema: "ingen.brain.codeact.custom_registry.v1";
  id: string;
  command: string;
  description: string;
  template: string;
  source: BrainDurableCandidateSource;
  trust: "user_confirmed" | "agent_candidate";
  evidence: string;
  createdAt: string;
  updatedAt: string;
}

export interface BrainResearchParallelRequestDetail {
  schema: "ingen.brain.research.parallel_request.v1";
  topic: string;
  source: "learning_interrupt" | "brain_manual";
  evidence: string;
  createdAt: string;
}

const USER_STORAGE_KEY = "ingen.brain.memory.user_identity.v1";
const AGENT_STORAGE_KEY = "ingen.brain.memory.agent_identity.v1";
const USER_LOCATION_STORAGE_KEY = "ingen.brain.memory.user_location.v1";
const LEARNING_REGISTRY_STORAGE_KEY = "ingen.brain.memory.learning_registry.v1";
const CUSTOM_CODEACT_REGISTRY_STORAGE_KEY = "ingen.brain.codeact.custom_registry.v1";
export const BRAIN_AGENT_MEMORY_UPDATED_EVENT = "ingen:brain-agent-memory-updated";
export const BRAIN_LEARNING_MEMORY_UPDATED_EVENT = "ingen:brain-learning-memory-updated";
export const BRAIN_CUSTOM_CODEACTS_UPDATED_EVENT = "ingen:brain-custom-codeacts-updated";
export const BRAIN_RESEARCH_PARALLEL_REQUEST_EVENT = "ingen:brain-research-parallel-request";

const LEARNING_MEMORY_CATEGORIES = new Set<BrainLearningMemoryStoredCategory>([
  "lesson",
  "skill",
  "task",
  "anti_pattern",
  "conduct_rule"
]);

const MAX_LEARNING_MEMORY_ENTRIES = 120;
const MAX_CUSTOM_CODEACT_ENTRIES = 80;
const MAX_LEARNING_TEXT_LENGTH = 1200;
const MAX_CODEACT_FIELD_LENGTH = 2200;
const MAX_DURABLE_MANIFEST_LENGTH = 7000;
const DURABLE_MANIFEST_CATEGORY_ORDER: BrainLearningMemoryCategory[] = ["lesson", "skill", "task"];
const DURABLE_MANIFEST_CATEGORY_KEYS: Record<BrainLearningMemoryCategory, string> = {
  lesson: "lessons",
  skill: "skills",
  task: "tasks"
};

const fallbackBrainUserMemory: BrainUserMemorySlot = {
  schema: "ingen.brain.memory.user_identity.v1",
  scope: "brain.memory.user.identity",
  stableKey: "user.identity.first_name",
  preferredFirstName: "",
  trust: "unset",
  evidence: "brain_identity_editor:user_first_name_unset"
};

const fallbackBrainAgentMemory: BrainAgentMemorySlot = {
  schema: "ingen.brain.memory.agent_identity.v1",
  scope: "brain.memory.agent.identity",
  stableKey: "agent.identity.first_name",
  preferredFirstName: "",
  trust: "unset",
  evidence: "brain_identity_editor:agent_first_name_unset"
};

const fallbackBrainUserLocationMemory: BrainUserLocationMemorySlot = {
  schema: "ingen.brain.memory.user_location.v1",
  scope: "brain.memory.user.location",
  stableKey: "user.location.home",
  homeLocation: "",
  trust: "unset",
  evidence: "brain_memory_editor:user_home_location_unset"
};

function trimmedSingleLine(value: string, maxLength: number): string {
  return value.replace(/\s+/g, " ").trim().slice(0, maxLength);
}

function trimmedMultiline(value: string, maxLength: number): string {
  return value
    .replace(/\r\n?/g, "\n")
    .replace(/[ \t]+\n/g, "\n")
    .trim()
    .slice(0, maxLength);
}

function nowIso(): string {
  return new Date().toISOString();
}

function localId(prefix: string): string {
  const random = typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID().slice(0, 8)
    : Math.random().toString(36).slice(2, 10);
  return `${prefix}_${Date.now().toString(36)}_${random}`;
}

function readJsonArray<T>(storageKey: string, isEntry: (value: unknown) => value is T): T[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) {
      window.localStorage.setItem(storageKey, JSON.stringify([]));
      return [];
    }
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter(isEntry) : [];
  } catch {
    return [];
  }
}

function writeJsonArray<T>(storageKey: string, entries: T[]): T[] {
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(storageKey, JSON.stringify(entries));
    } catch {
      // Keep the caller state usable even if localStorage is temporarily unavailable.
    }
  }
  return entries;
}

function dispatchBrainStoreEvent<T>(eventName: string, detail: T) {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent<T>(eventName, { detail }));
  }
}

function isBrainUserMemorySlot(value: unknown): value is BrainUserMemorySlot {
  const candidate = value as Partial<BrainUserMemorySlot>;
  return (
    candidate?.schema === "ingen.brain.memory.user_identity.v1" &&
    candidate.scope === "brain.memory.user.identity" &&
    candidate.stableKey === "user.identity.first_name" &&
    typeof candidate.preferredFirstName === "string"
  );
}

function isBrainAgentMemorySlot(value: unknown): value is BrainAgentMemorySlot {
  const candidate = value as Partial<BrainAgentMemorySlot>;
  return (
    candidate?.schema === "ingen.brain.memory.agent_identity.v1" &&
    candidate.scope === "brain.memory.agent.identity" &&
    candidate.stableKey === "agent.identity.first_name" &&
    typeof candidate.preferredFirstName === "string"
  );
}

function isBrainUserLocationMemorySlot(value: unknown): value is BrainUserLocationMemorySlot {
  const candidate = value as Partial<BrainUserLocationMemorySlot>;
  return (
    candidate?.schema === "ingen.brain.memory.user_location.v1" &&
    candidate.scope === "brain.memory.user.location" &&
    candidate.stableKey === "user.location.home" &&
    typeof candidate.homeLocation === "string"
  );
}

function normalizeBrainLearningMemoryCategory(value: unknown): BrainLearningMemoryCategory | null {
  if (value === "anti_pattern" || value === "conduct_rule") return "lesson";
  if (value === "lesson" || value === "skill" || value === "task") return value;
  return null;
}

function normalizeBrainLearningMemoryEntry(value: unknown): BrainLearningMemoryEntry | null {
  const candidate = value as Partial<BrainLearningMemoryEntry>;
  const category = normalizeBrainLearningMemoryCategory(candidate?.category);
  if (
    candidate?.schema === "ingen.brain.memory.learning_registry.v1" &&
    typeof candidate.id === "string" &&
    typeof candidate.text === "string" &&
    typeof candidate.source === "string" &&
    typeof candidate.trust === "string" &&
    typeof candidate.evidence === "string" &&
    typeof candidate.createdAt === "string" &&
    typeof candidate.updatedAt === "string" &&
    category &&
    LEARNING_MEMORY_CATEGORIES.has(candidate.category as BrainLearningMemoryStoredCategory)
  ) {
    return { ...candidate, category } as BrainLearningMemoryEntry;
  }
  return null;
}

function isBrainLearningMemoryEntry(value: unknown): value is BrainLearningMemoryEntry {
  return Boolean(normalizeBrainLearningMemoryEntry(value));
}

function isBrainCustomCodeActEntry(value: unknown): value is BrainCustomCodeActEntry {
  const candidate = value as Partial<BrainCustomCodeActEntry>;
  return (
    candidate?.schema === "ingen.brain.codeact.custom_registry.v1" &&
    typeof candidate.id === "string" &&
    typeof candidate.command === "string" &&
    typeof candidate.description === "string" &&
    typeof candidate.template === "string" &&
    typeof candidate.source === "string" &&
    typeof candidate.trust === "string" &&
    typeof candidate.evidence === "string" &&
    typeof candidate.createdAt === "string" &&
    typeof candidate.updatedAt === "string"
  );
}

export function readBrainUserMemory(): BrainUserMemorySlot {
  if (typeof window === "undefined") return fallbackBrainUserMemory;
  try {
    const raw = window.localStorage.getItem(USER_STORAGE_KEY);
    if (!raw) {
      window.localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(fallbackBrainUserMemory));
      return fallbackBrainUserMemory;
    }
    const parsed: unknown = JSON.parse(raw);
    return isBrainUserMemorySlot(parsed) ? parsed : fallbackBrainUserMemory;
  } catch {
    return fallbackBrainUserMemory;
  }
}

export function writeBrainUserMemory(preferredFirstName: string): BrainUserMemorySlot {
  const next: BrainUserMemorySlot = {
    ...fallbackBrainUserMemory,
    preferredFirstName,
    trust: "user_confirmed",
    evidence: "brain_identity_editor:user_first_name"
  };
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Keep the in-memory edit even if localStorage is temporarily unavailable.
    }
  }
  return next;
}

export function readBrainLearningMemoryEntries(): BrainLearningMemoryEntry[] {
  return readJsonArray(LEARNING_REGISTRY_STORAGE_KEY, isBrainLearningMemoryEntry)
    .map(normalizeBrainLearningMemoryEntry)
    .filter((entry): entry is BrainLearningMemoryEntry => Boolean(entry));
}

export function writeBrainLearningMemoryEntries(entries: BrainLearningMemoryEntry[]): BrainLearningMemoryEntry[] {
  const next = writeJsonArray(
    LEARNING_REGISTRY_STORAGE_KEY,
    entries
      .map(normalizeBrainLearningMemoryEntry)
      .filter((entry): entry is BrainLearningMemoryEntry => Boolean(entry))
      .slice(0, MAX_LEARNING_MEMORY_ENTRIES)
  );
  dispatchBrainStoreEvent(BRAIN_LEARNING_MEMORY_UPDATED_EVENT, next);
  return next;
}

export function appendBrainLearningMemoryEntry(input: {
  category: BrainLearningMemoryCategory;
  text: string;
  source: BrainDurableCandidateSource;
  evidence?: string;
  trust?: BrainLearningMemoryEntry["trust"];
}): BrainLearningMemoryEntry[] {
  const text = trimmedMultiline(input.text, MAX_LEARNING_TEXT_LENGTH);
  if (!text) {
    return readBrainLearningMemoryEntries();
  }
  const timestamp = nowIso();
  const nextEntry: BrainLearningMemoryEntry = {
    schema: "ingen.brain.memory.learning_registry.v1",
    id: localId("brain_learning"),
    category: input.category,
    text,
    source: input.source,
    trust: input.trust ?? (input.source === "manual" ? "user_confirmed" : "agent_candidate"),
    evidence: input.evidence ?? `brain_learning_registry:${input.source}`,
    createdAt: timestamp,
    updatedAt: timestamp
  };
  const current = readBrainLearningMemoryEntries();
  return writeBrainLearningMemoryEntries([nextEntry, ...current].slice(0, MAX_LEARNING_MEMORY_ENTRIES));
}

export function removeBrainLearningMemoryEntry(id: string): BrainLearningMemoryEntry[] {
  return writeBrainLearningMemoryEntries(readBrainLearningMemoryEntries().filter((entry) => entry.id !== id));
}

export function normalizeBrainCustomCodeActCommand(value: string): string {
  const cleaned = trimmedSingleLine(value, 72)
    .replace(/[^\w/-]+/g, "_")
    .replace(/_+/g, "_")
    .replace(/^\/+/, "")
    .replace(/^_+|_+$/g, "");
  const body = cleaned || "agent_draft";
  return `/${body.endsWith("_") ? body : `${body}_`}`;
}

export function readBrainCustomCodeActs(): BrainCustomCodeActEntry[] {
  return readJsonArray(CUSTOM_CODEACT_REGISTRY_STORAGE_KEY, isBrainCustomCodeActEntry);
}

export function writeBrainCustomCodeActs(entries: BrainCustomCodeActEntry[]): BrainCustomCodeActEntry[] {
  const next = writeJsonArray(
    CUSTOM_CODEACT_REGISTRY_STORAGE_KEY,
    entries.filter(isBrainCustomCodeActEntry).slice(0, MAX_CUSTOM_CODEACT_ENTRIES)
  );
  dispatchBrainStoreEvent(BRAIN_CUSTOM_CODEACTS_UPDATED_EVENT, next);
  return next;
}

export function appendBrainCustomCodeAct(input: {
  command: string;
  description: string;
  template?: string;
  source: BrainDurableCandidateSource;
  evidence?: string;
  trust?: BrainCustomCodeActEntry["trust"];
}): BrainCustomCodeActEntry[] {
  const command = normalizeBrainCustomCodeActCommand(input.command);
  const description = trimmedMultiline(input.description, MAX_CODEACT_FIELD_LENGTH);
  const template = trimmedMultiline(input.template ?? "", MAX_CODEACT_FIELD_LENGTH);
  if (!description && !template) {
    return readBrainCustomCodeActs();
  }
  const timestamp = nowIso();
  const nextEntry: BrainCustomCodeActEntry = {
    schema: "ingen.brain.codeact.custom_registry.v1",
    id: localId("brain_codeact"),
    command,
    description: description || "Agent drafted CodeAct",
    template,
    source: input.source,
    trust: input.trust ?? (input.source === "manual" ? "user_confirmed" : "agent_candidate"),
    evidence: input.evidence ?? `brain_custom_codeact:${input.source}`,
    createdAt: timestamp,
    updatedAt: timestamp
  };
  const current = readBrainCustomCodeActs();
  return writeBrainCustomCodeActs([nextEntry, ...current].slice(0, MAX_CUSTOM_CODEACT_ENTRIES));
}

export function removeBrainCustomCodeAct(id: string): BrainCustomCodeActEntry[] {
  return writeBrainCustomCodeActs(readBrainCustomCodeActs().filter((entry) => entry.id !== id));
}

function durableManifestMemoryLine(entry: BrainLearningMemoryEntry, index: number): string {
  const key = DURABLE_MANIFEST_CATEGORY_KEYS[entry.category];
  return `${key}[${index}]=trust=${entry.trust} source=${entry.source} text=${JSON.stringify(trimmedSingleLine(entry.text, 360))}`;
}

function durableManifestCodeActLine(entry: BrainCustomCodeActEntry, index: number): string {
  return [
    `codeact_draft[${index}]`,
    `command=${JSON.stringify(normalizeBrainCustomCodeActCommand(entry.command))}`,
    `trust=${entry.trust}`,
    `source=${entry.source}`,
    `description=${JSON.stringify(trimmedSingleLine(entry.description, 320))}`,
    entry.template ? `template=${JSON.stringify(trimmedMultiline(entry.template, 620))}` : ""
  ].filter(Boolean).join(" ");
}

export function brainDurableMemoryManifestFromEntries(
  learningEntries: BrainLearningMemoryEntry[],
  customCodeActs: BrainCustomCodeActEntry[]
): string {
  const validLearningEntries = learningEntries
    .map(normalizeBrainLearningMemoryEntry)
    .filter((entry): entry is BrainLearningMemoryEntry => Boolean(entry));
  const validCodeActs = customCodeActs.filter(isBrainCustomCodeActEntry);
  if (validLearningEntries.length === 0 && validCodeActs.length === 0) {
    return "";
  }
  const lines = [
    "BRAIN_DURABLE_MEMORY_MANIFEST v1",
    "source=brain_page_learning_memory",
    "injection_policy=session_boot_and_after_context_compaction",
    "research_policy=Research branches are live work only; do not persist research unless it is later promoted into lessons, skills, tasks, or CodeAct drafts.",
    `learning_entries=${validLearningEntries.length}`,
    `codeact_drafts=${validCodeActs.length}`,
    "rule=Treat user_confirmed entries as durable user Brain context. Treat agent_candidate entries as useful but still revisable candidates; obey them unless the user overrides them."
  ];
  for (const category of DURABLE_MANIFEST_CATEGORY_ORDER) {
    const categoryEntries = validLearningEntries.filter((entry) => entry.category === category).slice(0, 8);
    categoryEntries.forEach((entry, index) => lines.push(durableManifestMemoryLine(entry, index + 1)));
  }
  validCodeActs.slice(0, 6).forEach((entry, index) => lines.push(durableManifestCodeActLine(entry, index + 1)));
  return trimmedMultiline(lines.join("\n"), MAX_DURABLE_MANIFEST_LENGTH);
}

export function brainDurableMemoryManifest(): string {
  return brainDurableMemoryManifestFromEntries(readBrainLearningMemoryEntries(), readBrainCustomCodeActs());
}

export function dispatchBrainResearchParallelRequest(detail: Omit<BrainResearchParallelRequestDetail, "schema" | "createdAt">) {
  const topic = trimmedMultiline(detail.topic, MAX_LEARNING_TEXT_LENGTH);
  if (!topic || typeof window === "undefined") {
    return;
  }
  dispatchBrainStoreEvent<BrainResearchParallelRequestDetail>(BRAIN_RESEARCH_PARALLEL_REQUEST_EVENT, {
    schema: "ingen.brain.research.parallel_request.v1",
    topic,
    source: detail.source,
    evidence: detail.evidence,
    createdAt: nowIso()
  });
}

export function readBrainAgentMemory(): BrainAgentMemorySlot {
  if (typeof window === "undefined") return fallbackBrainAgentMemory;
  try {
    const raw = window.localStorage.getItem(AGENT_STORAGE_KEY);
    if (!raw) {
      window.localStorage.setItem(AGENT_STORAGE_KEY, JSON.stringify(fallbackBrainAgentMemory));
      return fallbackBrainAgentMemory;
    }
    const parsed: unknown = JSON.parse(raw);
    return isBrainAgentMemorySlot(parsed) ? parsed : fallbackBrainAgentMemory;
  } catch {
    return fallbackBrainAgentMemory;
  }
}

export function writeBrainAgentMemory(preferredFirstName: string): BrainAgentMemorySlot {
  const next: BrainAgentMemorySlot = {
    ...fallbackBrainAgentMemory,
    preferredFirstName,
    trust: "user_confirmed",
    evidence: "brain_identity_editor:agent_first_name"
  };
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(AGENT_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Keep the in-memory edit even if localStorage is temporarily unavailable.
    }
    window.dispatchEvent(new CustomEvent<BrainAgentMemorySlot>(BRAIN_AGENT_MEMORY_UPDATED_EVENT, { detail: next }));
  }
  return next;
}

export function readBrainUserLocationMemory(): BrainUserLocationMemorySlot {
  if (typeof window === "undefined") return fallbackBrainUserLocationMemory;
  try {
    const raw = window.localStorage.getItem(USER_LOCATION_STORAGE_KEY);
    if (!raw) {
      window.localStorage.setItem(USER_LOCATION_STORAGE_KEY, JSON.stringify(fallbackBrainUserLocationMemory));
      return fallbackBrainUserLocationMemory;
    }
    const parsed: unknown = JSON.parse(raw);
    return isBrainUserLocationMemorySlot(parsed) ? parsed : fallbackBrainUserLocationMemory;
  } catch {
    return fallbackBrainUserLocationMemory;
  }
}

export function writeBrainUserLocationMemory(homeLocation: string): BrainUserLocationMemorySlot {
  const next: BrainUserLocationMemorySlot = {
    ...fallbackBrainUserLocationMemory,
    homeLocation,
    trust: "user_confirmed",
    evidence: "brain_memory_editor:user_home_location"
  };
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(USER_LOCATION_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Keep the in-memory edit even if localStorage is temporarily unavailable.
    }
  }
  return next;
}
