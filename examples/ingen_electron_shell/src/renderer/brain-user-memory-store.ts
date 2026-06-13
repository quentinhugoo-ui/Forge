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

const USER_STORAGE_KEY = "ingen.brain.memory.user_identity.v1";
const AGENT_STORAGE_KEY = "ingen.brain.memory.agent_identity.v1";
const USER_LOCATION_STORAGE_KEY = "ingen.brain.memory.user_location.v1";
export const BRAIN_AGENT_MEMORY_UPDATED_EVENT = "ingen:brain-agent-memory-updated";

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
