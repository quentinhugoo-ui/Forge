import { BRAIN_CODEACT_COMMANDS } from "../shared/ipc-contract";

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

export interface BrainPersonalityMemorySlot {
  schema: "ingen.brain.memory.personality.v1";
  scope: "brain.memory.personality";
  stableKey: "brain.personality.loop_stream_voice";
  manifest: string;
  trust: "default_product" | "user_confirmed";
  evidence: string;
}

export type BrainLearningMemoryCategory = "lesson" | "skill" | "task";
type LegacyBrainLearningMemoryCategory = "anti_pattern" | "conduct_rule";
type BrainLearningMemoryStoredCategory = BrainLearningMemoryCategory | LegacyBrainLearningMemoryCategory;
export type BrainDurableCandidateSource = "manual" | "agent_learning_interrupt" | "host_generated_newbrain";

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

export type BrainSpecializedBrainEntryKind = "lesson" | "skill" | "task" | "codeact";

export interface BrainSpecializedBrainEntry {
  schema: "ingen.brain.memory.specialized_brain.v1";
  id: string;
  brainName: string;
  title: string;
  purpose: string;
  activationTriggers: string[];
  activationCommand: string;
  tokenBudget: number;
  lessons: string[];
  skills: string[];
  tasks: string[];
  codeActs: string[];
  source: BrainDurableCandidateSource;
  trust: "user_confirmed" | "agent_candidate";
  evidence: string;
  activeAt: string;
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
const PERSONALITY_STORAGE_KEY = "ingen.brain.memory.personality.v1";
const LEARNING_REGISTRY_STORAGE_KEY = "ingen.brain.memory.learning_registry.v1";
const CUSTOM_CODEACT_REGISTRY_STORAGE_KEY = "ingen.brain.codeact.custom_registry.v1";
const SPECIALIZED_BRAIN_REGISTRY_STORAGE_KEY = "ingen.brain.memory.specialized_brain_registry.v1";
export const BRAIN_AGENT_MEMORY_UPDATED_EVENT = "ingen:brain-agent-memory-updated";
export const BRAIN_PERSONALITY_MEMORY_UPDATED_EVENT = "ingen:brain-personality-memory-updated";
export const BRAIN_LEARNING_MEMORY_UPDATED_EVENT = "ingen:brain-learning-memory-updated";
export const BRAIN_CUSTOM_CODEACTS_UPDATED_EVENT = "ingen:brain-custom-codeacts-updated";
export const BRAIN_SPECIALIZED_BRAINS_UPDATED_EVENT = "ingen:brain-specialized-brains-updated";
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
const MAX_SPECIALIZED_BRAIN_ENTRIES = 48;
const MAX_SPECIALIZED_BRAIN_ITEM_ENTRIES = 24;
const MAX_LEARNING_TEXT_LENGTH = 1200;
const MAX_CODEACT_FIELD_LENGTH = 2200;
const MAX_SPECIALIZED_BRAIN_FIELD_LENGTH = 1600;
const MAX_PERSONALITY_MANIFEST_LENGTH = 5000;
const MAX_DURABLE_MANIFEST_LENGTH = 9000;
const EXECUTABLE_CODEACT_COMMANDS = new Set<string>(BRAIN_CODEACT_COMMANDS as readonly string[]);
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

export const DEFAULT_BRAIN_PERSONALITY_MANIFEST = [
  "BRAIN_PERSONALITY_MANIFEST v1",
  "voice=Parle comme un agent desktop present, capable et attentif; pas comme un journal de statut.",
  "loop_stream=Dans chaque paragraphe de travail, rends visible ce que tu viens de comprendre, pourquoi cela change la prochaine action, puis quelle preuve tu attends.",
  "tone=Naturel, clair, un peu chaleureux quand c'est utile, sans surjouer l'emotion ni pretendre etre humain.",
  "pivots=Quand une piste echoue, explique le signal utile de l'echec puis le detour choisi.",
  "success=Quand une action marche, nomme la preuve concrete: chemin, resultat accepte, compteur, exit code, artefact ou verification.",
  "rhythm=Varie les ouvertures; evite de commencer chaque paragraphe par 'Je vais' ou par des labels mecaniques.",
  "boundaries=Reste honnete: ne promets pas une action sans event ou outil, ne declare jamais un succes sans preuve runtime."
].join("\n");

const fallbackBrainPersonalityMemory: BrainPersonalityMemorySlot = {
  schema: "ingen.brain.memory.personality.v1",
  scope: "brain.memory.personality",
  stableKey: "brain.personality.loop_stream_voice",
  manifest: DEFAULT_BRAIN_PERSONALITY_MANIFEST,
  trust: "default_product",
  evidence: "brain_personality_editor:default_voice_contract"
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

export function normalizeBrainSpecializedBrainName(value: string): string {
  return trimmedSingleLine(value, 96)
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 48);
}

export function brainSpecializedBrainActivationCommand(brainName: string): string {
  const slug = normalizeBrainSpecializedBrainName(brainName);
  if (!slug) {
    return "";
  }
  return `/${slug.endsWith("brain") ? slug : `${slug}brain`}_`;
}

export function brainSpecializedBrainNameFromActivationCommand(command: string): string {
  const trimmed = normalizeBrainCustomCodeActCommand(command);
  const match = /^\/([a-z0-9_]+)brain_$/.exec(trimmed);
  return match ? normalizeBrainSpecializedBrainName(match[1] ?? "") : "";
}

function splitSpecializedBrainList(value: string | string[] | undefined): string[] {
  const rawItems = Array.isArray(value) ? value : (value ?? "").split("|");
  return rawItems
    .map((item) => trimmedMultiline(item, MAX_SPECIALIZED_BRAIN_FIELD_LENGTH))
    .filter(Boolean)
    .slice(0, MAX_SPECIALIZED_BRAIN_ITEM_ENTRIES);
}

function isBrainPersonalityMemorySlot(value: unknown): value is BrainPersonalityMemorySlot {
  const candidate = value as Partial<BrainPersonalityMemorySlot>;
  return (
    candidate?.schema === "ingen.brain.memory.personality.v1" &&
    candidate.scope === "brain.memory.personality" &&
    candidate.stableKey === "brain.personality.loop_stream_voice" &&
    typeof candidate.manifest === "string"
  );
}

export function normalizeBrainPersonalityManifest(value: string): string {
  const compact = trimmedMultiline(value, MAX_PERSONALITY_MANIFEST_LENGTH);
  return compact || DEFAULT_BRAIN_PERSONALITY_MANIFEST;
}

function firstCodeActCommand(value: string): string {
  return /^\/[a-zA-Z0-9][a-zA-Z0-9_]*_/.exec(value.trim())?.[0] ?? "";
}

function isExecutableSpecializedBrainCodeAct(value: string): boolean {
  return EXECUTABLE_CODEACT_COMMANDS.has(firstCodeActCommand(value));
}

function splitSpecializedBrainExecutableCodeActs(value: string | string[] | undefined): string[] {
  return splitSpecializedBrainList(value).filter(isExecutableSpecializedBrainCodeAct);
}

function splitSpecializedBrainTriggers(value: string | string[] | undefined): string[] {
  const rawItems = Array.isArray(value) ? value : (value ?? "").split(/[|,]/);
  return rawItems
    .map((item) => trimmedSingleLine(item, 160))
    .filter(Boolean)
    .slice(0, 16);
}

function titleFromSpecializedBrainName(brainName: string): string {
  const words = (normalizeBrainSpecializedBrainName(brainName) || "specialized_brain").split("_").filter(Boolean);
  return `${words.map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(" ")} Brain`;
}

function normalizeSpecializedBrainItemList(items: unknown): string[] {
  if (!Array.isArray(items)) {
    return [];
  }
  return items
    .filter((item): item is string => typeof item === "string")
    .map((item) => trimmedMultiline(item, MAX_SPECIALIZED_BRAIN_FIELD_LENGTH))
    .filter(Boolean)
    .slice(0, MAX_SPECIALIZED_BRAIN_ITEM_ENTRIES);
}

function normalizeSpecializedBrainCodeActList(items: unknown): string[] {
  return normalizeSpecializedBrainItemList(items).filter(isExecutableSpecializedBrainCodeAct);
}

function mergeSpecializedBrainItems(existing: string[], incoming: string[]): string[] {
  return [...incoming, ...existing.filter((item) => !incoming.includes(item))].slice(0, MAX_SPECIALIZED_BRAIN_ITEM_ENTRIES);
}

function normalizeBrainSpecializedBrainEntry(value: unknown): BrainSpecializedBrainEntry | null {
  const candidate = value as Partial<BrainSpecializedBrainEntry>;
  const brainName = normalizeBrainSpecializedBrainName(candidate?.brainName ?? "");
  const activationCommand = normalizeBrainCustomCodeActCommand(candidate?.activationCommand ?? brainSpecializedBrainActivationCommand(brainName));
  if (
    candidate?.schema === "ingen.brain.memory.specialized_brain.v1" &&
    typeof candidate.id === "string" &&
    brainName &&
    activationCommand &&
    typeof candidate.title === "string" &&
    typeof candidate.purpose === "string" &&
    typeof candidate.source === "string" &&
    typeof candidate.trust === "string" &&
    typeof candidate.evidence === "string" &&
    typeof candidate.createdAt === "string" &&
    typeof candidate.updatedAt === "string"
  ) {
    return {
      schema: "ingen.brain.memory.specialized_brain.v1",
      id: candidate.id,
      brainName,
      title: trimmedSingleLine(candidate.title, 160) || titleFromSpecializedBrainName(brainName),
      purpose: trimmedMultiline(candidate.purpose, MAX_SPECIALIZED_BRAIN_FIELD_LENGTH),
      activationTriggers: splitSpecializedBrainTriggers(candidate.activationTriggers),
      activationCommand,
      tokenBudget: Math.max(240, Math.min(4000, Math.round(Number(candidate.tokenBudget) || 1200))),
      lessons: normalizeSpecializedBrainItemList(candidate.lessons),
      skills: normalizeSpecializedBrainItemList(candidate.skills),
      tasks: normalizeSpecializedBrainItemList(candidate.tasks),
      codeActs: normalizeSpecializedBrainCodeActList(candidate.codeActs),
      source: candidate.source as BrainDurableCandidateSource,
      trust: candidate.trust === "user_confirmed" ? "user_confirmed" : "agent_candidate",
      evidence: candidate.evidence,
      activeAt: typeof candidate.activeAt === "string" ? candidate.activeAt : "",
      createdAt: candidate.createdAt,
      updatedAt: candidate.updatedAt
    };
  }
  return null;
}

function isBrainSpecializedBrainEntry(value: unknown): value is BrainSpecializedBrainEntry {
  return Boolean(normalizeBrainSpecializedBrainEntry(value));
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

export function upsertBrainCustomCodeAct(input: {
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
  const current = readBrainCustomCodeActs();
  const existing = current.find((entry) => normalizeBrainCustomCodeActCommand(entry.command) === command);
  if (existing?.source === "manual") {
    return current;
  }
  if (existing) {
    const timestamp = nowIso();
    const nextEntry: BrainCustomCodeActEntry = {
      ...existing,
      command,
      description: description || existing.description,
      template,
      source: input.source,
      trust: input.trust ?? existing.trust,
      evidence: input.evidence ?? existing.evidence,
      updatedAt: timestamp
    };
    return writeBrainCustomCodeActs([nextEntry, ...current.filter((entry) => entry.id !== existing.id)]);
  }
  return appendBrainCustomCodeAct(input);
}

export function removeBrainCustomCodeAct(id: string): BrainCustomCodeActEntry[] {
  return writeBrainCustomCodeActs(readBrainCustomCodeActs().filter((entry) => entry.id !== id));
}

export function readBrainSpecializedBrains(): BrainSpecializedBrainEntry[] {
  return readJsonArray(SPECIALIZED_BRAIN_REGISTRY_STORAGE_KEY, isBrainSpecializedBrainEntry)
    .map(normalizeBrainSpecializedBrainEntry)
    .filter((entry): entry is BrainSpecializedBrainEntry => Boolean(entry));
}

export function writeBrainSpecializedBrains(entries: BrainSpecializedBrainEntry[]): BrainSpecializedBrainEntry[] {
  const next = writeJsonArray(
    SPECIALIZED_BRAIN_REGISTRY_STORAGE_KEY,
    entries
      .map(normalizeBrainSpecializedBrainEntry)
      .filter((entry): entry is BrainSpecializedBrainEntry => Boolean(entry))
      .slice(0, MAX_SPECIALIZED_BRAIN_ENTRIES)
  );
  dispatchBrainStoreEvent(BRAIN_SPECIALIZED_BRAINS_UPDATED_EVENT, next);
  return next;
}

export function readBrainSpecializedBrainByName(brainName: string): BrainSpecializedBrainEntry | null {
  const normalizedName = normalizeBrainSpecializedBrainName(brainName);
  return readBrainSpecializedBrains().find((entry) => entry.brainName === normalizedName) ?? null;
}

export function readBrainSpecializedBrainByActivationCommand(command: string): BrainSpecializedBrainEntry | null {
  const normalizedCommand = normalizeBrainCustomCodeActCommand(command);
  return readBrainSpecializedBrains().find((entry) => entry.activationCommand === normalizedCommand) ?? null;
}

export function upsertBrainSpecializedBrain(input: {
  brainName: string;
  title?: string;
  purpose?: string;
  activationTriggers?: string | string[];
  tokenBudget?: number | string;
  lessons?: string | string[];
  skills?: string | string[];
  tasks?: string | string[];
  codeActs?: string | string[];
  source: BrainDurableCandidateSource;
  evidence?: string;
  trust?: BrainSpecializedBrainEntry["trust"];
}): BrainSpecializedBrainEntry[] {
  const brainName = normalizeBrainSpecializedBrainName(input.brainName);
  if (!brainName) {
    return readBrainSpecializedBrains();
  }
  const current = readBrainSpecializedBrains();
  const existing = current.find((entry) => entry.brainName === brainName);
  const timestamp = nowIso();
  const nextEntry: BrainSpecializedBrainEntry = {
    schema: "ingen.brain.memory.specialized_brain.v1",
    id: existing?.id ?? localId("specialized_brain"),
    brainName,
    title: trimmedSingleLine(input.title ?? existing?.title ?? titleFromSpecializedBrainName(brainName), 160),
    purpose: trimmedMultiline(input.purpose ?? existing?.purpose ?? "", MAX_SPECIALIZED_BRAIN_FIELD_LENGTH),
    activationTriggers: splitSpecializedBrainTriggers(input.activationTriggers ?? existing?.activationTriggers ?? []),
    activationCommand: brainSpecializedBrainActivationCommand(brainName),
    tokenBudget: Math.max(240, Math.min(4000, Math.round(Number(input.tokenBudget ?? existing?.tokenBudget ?? 1200) || 1200))),
    lessons: mergeSpecializedBrainItems(existing?.lessons ?? [], splitSpecializedBrainList(input.lessons)),
    skills: mergeSpecializedBrainItems(existing?.skills ?? [], splitSpecializedBrainList(input.skills)),
    tasks: mergeSpecializedBrainItems(existing?.tasks ?? [], splitSpecializedBrainList(input.tasks)),
    codeActs: mergeSpecializedBrainItems(existing?.codeActs ?? [], splitSpecializedBrainExecutableCodeActs(input.codeActs)),
    source: input.source,
    trust: input.trust ?? existing?.trust ?? (input.source === "manual" ? "user_confirmed" : "agent_candidate"),
    evidence: input.evidence ?? existing?.evidence ?? `brain_specialized_registry:${input.source}`,
    activeAt: existing?.activeAt ?? "",
    createdAt: existing?.createdAt ?? timestamp,
    updatedAt: timestamp
  };
  return writeBrainSpecializedBrains([nextEntry, ...current.filter((entry) => entry.brainName !== brainName)]);
}

export function activateBrainSpecializedBrain(brainNameOrCommand: string, evidence = "brain_specialized_registry:activation"): BrainSpecializedBrainEntry[] {
  const commandName = brainSpecializedBrainNameFromActivationCommand(brainNameOrCommand);
  const brainName = commandName || normalizeBrainSpecializedBrainName(brainNameOrCommand);
  if (!brainName) {
    return readBrainSpecializedBrains();
  }
  const current = readBrainSpecializedBrains();
  const existing = current.find((entry) => entry.brainName === brainName);
  if (!existing) {
    return current;
  }
  const timestamp = nowIso();
  return writeBrainSpecializedBrains([
    {
      ...existing,
      evidence,
      activeAt: timestamp,
      updatedAt: timestamp
    },
    ...current.filter((entry) => entry.brainName !== brainName)
  ]);
}

export function appendBrainSpecializedBrainItem(input: {
  brainName: string;
  kind: BrainSpecializedBrainEntryKind;
  content: string;
  source: BrainDurableCandidateSource;
  evidence?: string;
  trust?: BrainSpecializedBrainEntry["trust"];
}): BrainSpecializedBrainEntry[] {
  const brainName = normalizeBrainSpecializedBrainName(input.brainName);
  const content = trimmedMultiline(input.content, MAX_SPECIALIZED_BRAIN_FIELD_LENGTH);
  if (!brainName || !content) {
    return readBrainSpecializedBrains();
  }
  const effectiveKind = input.kind === "codeact" && !isExecutableSpecializedBrainCodeAct(content) ? "task" : input.kind;
  const current = readBrainSpecializedBrains();
  const existing = current.find((entry) => entry.brainName === brainName);
  if (!existing) {
    const seed: {
      lessons?: string;
      skills?: string;
      tasks?: string;
      codeActs?: string;
    } = {};
    if (effectiveKind === "lesson") seed.lessons = content;
    if (effectiveKind === "skill") seed.skills = content;
    if (effectiveKind === "task") seed.tasks = content;
    if (effectiveKind === "codeact") seed.codeActs = content;
    return upsertBrainSpecializedBrain({
      brainName,
      title: titleFromSpecializedBrainName(brainName),
      ...seed,
      source: input.source,
      trust: input.trust,
      evidence: input.evidence
    });
  }
  const timestamp = nowIso();
  const appendUnique = (items: string[]) => [content, ...items.filter((item) => item !== content)].slice(0, MAX_SPECIALIZED_BRAIN_ITEM_ENTRIES);
  const nextEntry: BrainSpecializedBrainEntry = {
    ...existing,
    lessons: effectiveKind === "lesson" ? appendUnique(existing.lessons) : existing.lessons,
    skills: effectiveKind === "skill" ? appendUnique(existing.skills) : existing.skills,
    tasks: effectiveKind === "task" ? appendUnique(existing.tasks) : existing.tasks,
    codeActs: effectiveKind === "codeact" ? appendUnique(existing.codeActs) : existing.codeActs,
    source: input.source,
    trust: input.trust ?? existing.trust,
    evidence: input.evidence ?? existing.evidence,
    updatedAt: timestamp
  };
  return writeBrainSpecializedBrains([nextEntry, ...current.filter((entry) => entry.brainName !== brainName)]);
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

function durableManifestSpecializedBrainCatalogLine(entry: BrainSpecializedBrainEntry, index: number): string {
  return [
    `specialized_brain[${index}]`,
    `name=${JSON.stringify(entry.brainName)}`,
    `command=${JSON.stringify(entry.activationCommand)}`,
    `title=${JSON.stringify(trimmedSingleLine(entry.title, 120))}`,
    entry.activationTriggers.length > 0 ? `triggers=${JSON.stringify(entry.activationTriggers.slice(0, 6).join(", "))}` : "",
    entry.purpose ? `purpose=${JSON.stringify(trimmedSingleLine(entry.purpose, 240))}` : ""
  ].filter(Boolean).join(" ");
}

function durableManifestSpecializedBrainItemLine(kind: BrainSpecializedBrainEntryKind, item: string, index: number): string {
  if (kind === "codeact") {
    return `codeact[${index}]=executable ${JSON.stringify(trimmedSingleLine(item, 520))}`;
  }
  return `${kind}[${index}]=${JSON.stringify(trimmedSingleLine(item, 360))}`;
}

export function readBrainPersonalityMemory(): BrainPersonalityMemorySlot {
  if (typeof window === "undefined") return fallbackBrainPersonalityMemory;
  try {
    const raw = window.localStorage.getItem(PERSONALITY_STORAGE_KEY);
    if (!raw) {
      window.localStorage.setItem(PERSONALITY_STORAGE_KEY, JSON.stringify(fallbackBrainPersonalityMemory));
      return fallbackBrainPersonalityMemory;
    }
    const parsed: unknown = JSON.parse(raw);
    if (!isBrainPersonalityMemorySlot(parsed)) {
      return fallbackBrainPersonalityMemory;
    }
    return {
      ...fallbackBrainPersonalityMemory,
      ...parsed,
      manifest: normalizeBrainPersonalityManifest(parsed.manifest),
      trust: parsed.trust === "user_confirmed" ? "user_confirmed" : "default_product"
    };
  } catch {
    return fallbackBrainPersonalityMemory;
  }
}

export function writeBrainPersonalityMemory(manifest: string): BrainPersonalityMemorySlot {
  const normalized = normalizeBrainPersonalityManifest(manifest);
  const next: BrainPersonalityMemorySlot = {
    ...fallbackBrainPersonalityMemory,
    manifest: normalized,
    trust: normalized === DEFAULT_BRAIN_PERSONALITY_MANIFEST ? "default_product" : "user_confirmed",
    evidence: normalized === DEFAULT_BRAIN_PERSONALITY_MANIFEST
      ? "brain_personality_editor:default_voice_contract"
      : "brain_personality_editor:user_voice_contract"
  };
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(PERSONALITY_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Keep the in-memory edit even if localStorage is temporarily unavailable.
    }
  }
  dispatchBrainStoreEvent(BRAIN_PERSONALITY_MEMORY_UPDATED_EVENT, next);
  return next;
}

export function brainDurableMemoryManifestFromEntries(
  learningEntries: BrainLearningMemoryEntry[],
  customCodeActs: BrainCustomCodeActEntry[],
  specializedBrains: BrainSpecializedBrainEntry[] = []
): string {
  const validLearningEntries = learningEntries
    .map(normalizeBrainLearningMemoryEntry)
    .filter((entry): entry is BrainLearningMemoryEntry => Boolean(entry));
  const validCodeActs = customCodeActs.filter(isBrainCustomCodeActEntry);
  const validSpecializedBrains = specializedBrains
    .map(normalizeBrainSpecializedBrainEntry)
    .filter((entry): entry is BrainSpecializedBrainEntry => Boolean(entry));
  if (validLearningEntries.length === 0 && validCodeActs.length === 0 && validSpecializedBrains.length === 0) {
    return "";
  }
  const activeSpecializedBrain = [...validSpecializedBrains]
    .filter((entry) => entry.activeAt)
    .sort((left, right) => right.activeAt.localeCompare(left.activeAt))[0] ?? null;
  const lines = [
    "BRAIN_DURABLE_MEMORY_MANIFEST v1",
    "source=brain_page_learning_memory",
    "injection_policy=session_boot_and_after_context_compaction",
    "specialized_brain_policy=The root catalog is read-only. Use /newbrain_ to create a named specialized Brain, /<name>brain_ to activate it, and /modify\"<name>\"brain_ to append explicit lessons, skills, tasks, or executable CodeAct references to that named Brain only.",
    "entry_kind_policy=skill is an LLM-only reusable workflow or prompt procedure. task is follow-up work, including requests to implement a future native CodeAct. codeact is only for commands with an existing executable app/front/runtime/MCP handler listed in BRAIN_CODEACT_COMMANDS; never store plain reasoning templates as codeact.",
    "newbrain_template_example=/newbrain_ brain_name=\"marketing\" title=\"Marketing Brain\" purpose=\"Store durable campaign strategy lessons, anti-patterns, reusable workflows and executable tool routes.\" activation_triggers=\"campaign strategy, copywriting, funnel iteration\" initial_lessons=\"over-broad audience targeting -> define one concrete ICP before writing copy\" initial_rules=\"identify offer, audience, channel and metric before campaign ideas\" initial_skills=\"turn failed campaign notes into improved brief\" initial_tasks=\"review campaign learnings before next launch\" initial_codeacts='/websearch_ query=\"current campaign benchmark sources\" output=\"compact_answer_url_citation_manifest\"' token_budget=\"1200\"",
    "modifybrain_template_example=/modify\"marketing\"brain_ brain_name=\"marketing\" entry_kind=\"lesson\" observation=\"campaign ideas were proposed before defining the ICP\" replacement_rule=\"define one concrete ICP before campaign ideas\" trigger=\"campaign brainstorming or copy iteration\" exceptions=\"user already supplied a precise ICP\" content=\"Observed error: proposing campaigns before ICP -> Replacement rule: define one concrete ICP before campaign ideas.\" evidence=\"session pattern or user-confirmed correction\" output=\"append_to_specialized_brain\"",
    "modifybrain_skill_example=/modify\"marketing\"brain_ brain_name=\"marketing\" entry_kind=\"skill\" content=\"Campaign critique workflow: identify offer, audience, channel and metric before writing campaign ideas.\" trigger=\"campaign brainstorming\" output=\"append_to_specialized_brain\"",
    "modifybrain_codeact_example=/modify\"marketing\"brain_ brain_name=\"marketing\" entry_kind=\"codeact\" content='/websearch_ query=\"current campaign benchmark sources\" output=\"compact_answer_url_citation_manifest\"' evidence=\"known executable command from BRAIN_CODEACT_COMMANDS\" output=\"append_to_specialized_brain\"",
    "research_policy=Research branches are live work only; do not persist research unless it is later promoted into lessons, skills, tasks, or CodeAct drafts.",
    `learning_entries=${validLearningEntries.length}`,
    `codeact_drafts=${validCodeActs.length}`,
    `specialized_brains=${validSpecializedBrains.length}`,
    "rule=Treat user_confirmed entries as durable user Brain context. Treat agent_candidate entries as useful but still revisable candidates; obey them unless the user overrides them."
  ];
  validSpecializedBrains.slice(0, 8).forEach((entry, index) => lines.push(durableManifestSpecializedBrainCatalogLine(entry, index + 1)));
  if (activeSpecializedBrain) {
    lines.push(
      [
        "active_specialized_brain",
        `name=${JSON.stringify(activeSpecializedBrain.brainName)}`,
        `command=${JSON.stringify(activeSpecializedBrain.activationCommand)}`,
        `title=${JSON.stringify(trimmedSingleLine(activeSpecializedBrain.title, 120))}`,
        `token_budget=${activeSpecializedBrain.tokenBudget}`
      ].join(" ")
    );
    activeSpecializedBrain.lessons.slice(0, 8).forEach((item, index) => lines.push(durableManifestSpecializedBrainItemLine("lesson", item, index + 1)));
    activeSpecializedBrain.skills.slice(0, 8).forEach((item, index) => lines.push(durableManifestSpecializedBrainItemLine("skill", item, index + 1)));
    activeSpecializedBrain.tasks.slice(0, 8).forEach((item, index) => lines.push(durableManifestSpecializedBrainItemLine("task", item, index + 1)));
    activeSpecializedBrain.codeActs.slice(0, 6).forEach((item, index) => lines.push(durableManifestSpecializedBrainItemLine("codeact", item, index + 1)));
  }
  for (const category of DURABLE_MANIFEST_CATEGORY_ORDER) {
    const categoryEntries = validLearningEntries.filter((entry) => entry.category === category).slice(0, 8);
    categoryEntries.forEach((entry, index) => lines.push(durableManifestMemoryLine(entry, index + 1)));
  }
  validCodeActs.slice(0, 6).forEach((entry, index) => lines.push(durableManifestCodeActLine(entry, index + 1)));
  return trimmedMultiline(lines.join("\n"), MAX_DURABLE_MANIFEST_LENGTH);
}

export function brainDurableMemoryManifest(): string {
  return brainDurableMemoryManifestFromEntries(readBrainLearningMemoryEntries(), readBrainCustomCodeActs(), readBrainSpecializedBrains());
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
