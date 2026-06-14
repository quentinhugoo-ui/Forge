import { useEffect, useId, useState, type KeyboardEvent } from "react";
import {
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_RENAME_SESSION_COMMAND,
  type BrainCodeActCommand,
  type HardwareMetric,
  type HardwareTelemetrySnapshot,
  type SidebarSessionItem
} from "../shared/ipc-contract";
import { BrainBlob } from "./brain-blob";
import {
  readBrainAgentMemory,
  readBrainUserLocationMemory,
  readBrainUserMemory,
  writeBrainAgentMemory,
  writeBrainUserLocationMemory,
  writeBrainUserMemory
} from "./brain-user-memory-store";
import { headerShadowStore } from "./header-shadow-store";
import { AirbnbIcon, CubeIcon, GmailIcon, GoogleIcon } from "./module-logos";
import { panelsChatBottomStore } from "./panels-chat-bottom-store";
import { sidebarShadowStore, useSidebarShadowStore } from "./sidebar-shadow-store";

type BrainSpace = "codeacts" | "memory" | "hardware" | "godel" | "personality";

async function fallbackPhotonCitySuggestionLabels(query: string): Promise<string[]> {
  const url = new URL("https://photon.komoot.io/api/");
  url.searchParams.set("q", query);
  url.searchParams.set("limit", "6");
  url.searchParams.set("lang", "en");
  url.searchParams.append("layer", "city");
  url.searchParams.append("layer", "locality");
  const response = await fetch(url.toString());
  if (!response.ok) {
    return [];
  }
  const payload = await response.json() as {
    features?: Array<{
      properties?: {
        city?: unknown;
        country?: unknown;
        name?: unknown;
      };
    }>;
  };
  const seen = new Set<string>();
  return (payload.features ?? [])
    .map((feature) => {
      const city = typeof feature.properties?.city === "string" && feature.properties.city.trim()
        ? feature.properties.city.trim()
        : typeof feature.properties?.name === "string"
          ? feature.properties.name.trim()
          : "";
      const country = typeof feature.properties?.country === "string" ? feature.properties.country.trim() : "";
      return city && country ? `${city}, ${country}` : city;
    })
    .filter((label) => {
      if (!label || seen.has(label)) return false;
      seen.add(label);
      return true;
    })
    .slice(0, 6);
}

/* Stroke glyphs follow the sidebar icon contract: 24-unit viewBox, 1.65 stroke. */
function Glyph({ kind, size = 16 }: { kind: string; size?: number }) {
  const base = {
    className: "brainGlyph",
    viewBox: "0 0 24 24",
    width: size,
    height: size,
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.65,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true
  };
  if (kind === "brain") {
    return (
      <svg {...base} viewBox="2.25 2.25 15.5 15.5">
        <path d="M9.5 4.5c0-.1-.02-.48-.15-.82a1.22 1.22 0 0 0-.32-.5A.76.76 0 0 0 8.5 3a2.91 2.91 0 0 0-1.76.58C6.28 3.94 6 4.43 6 5a.5.5 0 0 1-.66.47c-.18-.06-.35-.02-.53.12-.2.16-.39.45-.53.83-.28.78-.25 1.73.14 2.3A.5.5 0 0 1 4.5 9h.75a2.25 2.25 0 0 1 2.25 2.25v.34m2-7.09v10m0-7H8.42m2.08 7h.75c.69 0 1.25-.56 1.25-1.25v-1.84M9.5 15.47c-.05.12-.22.45-.55.81-.39.41-.89.72-1.45.72-.81 0-1.43-.4-1.86-.94-.44-.55-.64-1.19-.64-1.56a.5.5 0 0 0-.5-.5c-.13 0-.52-.08-.86-.38C3.31 13.34 3 12.86 3 12c0-.98.12-1.63.32-2.03m7.18-5.47c0-.1.02-.48.15-.82.08-.2.18-.37.32-.5A.76.76 0 0 1 11.5 3c.63 0 1.25.2 1.76.58.46.36.74.85.74 1.42a.5.5 0 0 0 .66.47c.18-.06.35-.02.53.12.2.16.39.45.53.83.28.78.25 1.73-.14 2.3A.5.5 0 0 0 16 9.5c.13 0 .26.03.38.1.12.08.22.2.3.37.2.4.32 1.05.32 2.03 0 .86-.31 1.34-.64 1.62-.34.3-.73.38-.86.38a.5.5 0 0 0-.5.5c0 .37-.2 1.01-.64 1.56-.43.54-1.05.94-1.86.94-.56 0-1.06-.31-1.45-.72a3.63 3.63 0 0 1-.55-.81M6.5 7a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm6 2a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Zm-6 4a.5.5 0 1 0 1 0 .5.5 0 0 0-1 0Z" strokeWidth="1.65" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }
  if (kind === "identity-card") {
    return (
      <svg {...base} viewBox="0 0 256 256" fill="currentColor" stroke="none">
        <path d="M75.19 198.4a8 8 0 0 0 11.21-1.6a52 52 0 0 1 83.2 0a8 8 0 1 0 12.8-9.6a67.88 67.88 0 0 0-27.4-21.69a40 40 0 1 0-53.94 0A67.88 67.88 0 0 0 73.6 187.2a8 8 0 0 0 1.59 11.2ZM128 112a24 24 0 1 1-24 24a24 24 0 0 1 24-24Zm72-88H56a16 16 0 0 0-16 16v176a16 16 0 0 0 16 16h144a16 16 0 0 0 16-16V40a16 16 0 0 0-16-16Zm0 192H56V40h144ZM88 64a8 8 0 0 1 8-8h64a8 8 0 0 1 0 16H96a8 8 0 0 1-8-8Z" />
      </svg>
    );
  }
  if (kind === "terminal") {
    return <svg {...base}><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>;
  }
  if (kind === "database") {
    return <svg {...base}><ellipse cx="12" cy="5" rx="9" ry="3" /><path d="M3 5v14a9 3 0 0 0 18 0V5" /><path d="M3 12a9 3 0 0 0 18 0" /></svg>;
  }
  if (kind === "shield-check") {
    return (
      <svg {...base}>
        <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />
        <path d="m9 12 2 2 4-4" />
      </svg>
    );
  }
  if (kind === "masks") {
    return (
      <svg {...base} viewBox="0 0 24 25" strokeWidth="1.5">
        <path strokeLinecap="round" d="M5.445 14.775a1.11 1.11 0 0 1 .777-.59c.339-.061.672.053.928.282m4.086 3.31c-.327.61-.878 1.057-1.555 1.18c-.677.122-1.344-.105-1.855-.565m2.733-4.54c.164-.305.439-.529.777-.59c.34-.06.672.053.928.283m.806-5.903c-1.15 1.086-2.899 1.95-4.94 2.318c-2.04.368-3.97.168-5.415-.45a.5.5 0 0 0-.289-.035c-.284.05-.47.348-.417.663l.938 5.443c.7 4.058 4.1 6.007 5.677 6.704c.522.232 1.098.261 1.658.16s1.092-.33 1.506-.73c1.249-1.208 3.792-4.229 3.092-8.287l-.937-5.443c-.055-.315-.33-.529-.614-.477a.5.5 0 0 0-.26.134" />
        <path d="M14.316 17.5c.363 0 .723-.065 1.06-.215c1.577-.697 4.977-2.646 5.677-6.704l.938-5.443c.054-.315-.133-.612-.417-.663a.5.5 0 0 0-.289.035c-1.444.618-3.375.818-5.416.45c-2.04-.368-3.788-1.232-4.939-2.318a.5.5 0 0 0-.259-.134c-.284-.052-.56.162-.614.477L9.12 8.428c-.083.477-.12.94-.12 1.386" />
      </svg>
    );
  }
  if (kind === "codeact") {
    return (
      <svg {...base}>
        <line x1="11.5" y1="4.5" x2="5.5" y2="19.5" />
        <line x1="10" y1="19.5" x2="19" y2="19.5" />
      </svg>
    );
  }
  if (kind === "archive") {
    return <svg {...base}><rect x="2" y="3" width="20" height="5" /><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" /><path d="M10 12h4" /></svg>;
  }
  if (kind === "globe") {
    return <svg {...base}><circle cx="12" cy="12" r="10" /><line x1="2" y1="12" x2="22" y2="12" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" /></svg>;
  }
  if (kind === "image") {
    return <svg {...base}><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="9" cy="9" r="2" /><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" /></svg>;
  }
  if (kind === "questionnaire") {
    return <svg {...base}><path d="M8 6h13" /><path d="M8 12h13" /><path d="M8 18h13" /><path d="M3 6h.01" /><path d="M3 12h.01" /><path d="M3 18h.01" /></svg>;
  }
  if (kind === "pencil") {
    return <svg {...base}><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" /><path d="m15 5 4 4" /></svg>;
  }
  if (kind === "folder") {
    return (
      <svg {...base}>
        <path d="M3.75 7.25A2.25 2.25 0 0 1 6 5h4.15l2 2H18a2.25 2.25 0 0 1 2.25 2.25v7.5A2.25 2.25 0 0 1 18 19H6a2.25 2.25 0 0 1-2.25-2.25v-9.5Z" fill="currentColor" stroke="none" />
      </svg>
    );
  }
  if (kind === "cpu") {
    return (
      <svg {...base}>
        <rect x="4" y="4" width="16" height="16" rx="2" />
        <rect x="9" y="9" width="6" height="6" />
        <path d="M9 2v2M15 2v2M9 20v2M15 20v2M2 9h2M2 15h2M20 9h2M20 15h2" />
      </svg>
    );
  }
  if (kind === "gauge") {
    return <svg {...base}><path d="M4 14a8 8 0 1 1 16 0" /><path d="M12 14l4-5" /><path d="M5 20h14" /></svg>;
  }
  if (kind === "thermometer") {
    return <svg {...base}><path d="M14 14.76V5a2 2 0 0 0-4 0v9.76a4 4 0 1 0 4 0Z" /><path d="M12 8v7" /></svg>;
  }
  if (kind === "fan") {
    return <svg {...base}><path d="M12 12m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0" /><path d="M12 10c1.7-4.7 5.7-6.2 7.4-4.5c1.4 1.4.8 4.8-3.2 6.5" /><path d="M13.7 13c3.2 3.8 2.5 7.9.1 8.5c-1.9.5-4.6-1.7-3.6-6" /><path d="M10.3 13C5.4 13.9 2.1 11.4 2.7 9c.5-1.9 3.8-3.1 7.2-.1" /></svg>;
  }
  if (kind === "memory") {
    return <svg {...base}><rect x="5" y="7" width="14" height="10" rx="2" /><path d="M8 3v4M12 3v4M16 3v4M8 17v4M12 17v4M16 17v4M3 10h2M3 14h2M19 10h2M19 14h2" /></svg>;
  }
  if (kind === "reuse") {
    return <svg {...base}><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /></svg>;
  }
  if (kind === "zap") {
    return <svg {...base}><path d="M13 2 3 14h7l-1 8 10-12h-7l1-8z" /></svg>;
  }
  if (kind === "layout") {
    return <svg {...base}><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 9h18" /><path d="M9 21V9" /></svg>;
  }
  if (kind === "calendar") {
    return <svg {...base}><rect x="3" y="4" width="18" height="18" rx="2" /><path d="M16 2v4M8 2v4M3 10h18" /></svg>;
  }
  if (kind === "modules") {
    return (
      <svg {...base} viewBox="2 2 20 20" strokeWidth="2">
        <rect height="6" rx="0.86" width="6" x="4" y="4" />
        <rect height="6" rx="0.86" width="6" x="4" y="14" />
        <rect height="6" rx="0.86" width="6" x="14" y="14" />
        <rect height="6" rx="0.86" width="6" x="14" y="4" />
      </svg>
    );
  }
  if (kind === "plug") {
    return <svg {...base}><path d="M12 22v-5" /><path d="M9 8V2" /><path d="M15 8V2" /><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z" /></svg>;
  }
  if (kind === "plus") {
    return <svg {...base}><path d="M12 5v14" /><path d="M5 12h14" /></svg>;
  }
  if (kind === "minus") {
    return <svg {...base}><path d="M5 12h14" /></svg>;
  }
  if (kind === "flask") {
    return <svg {...base}><path d="M10 2v6.6L4.7 18a2 2 0 0 0 1.8 3h11a2 2 0 0 0 1.8-3L14 8.6V2" /><path d="M8.5 2h7" /><path d="M7 15h10" /></svg>;
  }
  if (kind === "code") {
    return <svg {...base}><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></svg>;
  }
  return <svg {...base}><circle cx="12" cy="12" r="9" /></svg>;
}

function CodeActIcon({ command }: { command: BrainCodeActCommand }) {
  if (command === "/gmail_" || command === "/gmail_com") return <GmailIcon />;
  if (command === "/airbnb_") return <AirbnbIcon />;
  if (command === "/googleweb_") return <GoogleIcon />;
  if (command === "/newobject_") return <CubeIcon />;
  if (command === "/questionnaire_") return <Glyph kind="questionnaire" />;
  const stroke: Partial<Record<BrainCodeActCommand, string>> = {
    "/searcharchive_": "archive",
    "/sciencebrain_": "flask",
    "/codingbrain_": "code",
    "/newimage_": "image",
    "/editimage_": "pencil",
    "/workspace_": "folder",
    "/newcompute_": "cpu",
    "/selectcompute_": "reuse",
    "/compute_<name>_": "zap",
    "/web_": "globe",
    "/frontdesign_": "layout",
    "/google_agenda_": "calendar",
    "/brain_": "brain",
    "/newmodule_": "modules",
    "/rust_port_adapter_": "plug",
    "/rust_state_store_": "database"
  };
  return <Glyph kind={stroke[command] ?? "terminal"} />;
}

const BRAIN_SPACES: { id: BrainSpace; label: string; glyph: string }[] = [
  { id: "memory", label: "Memory", glyph: "database" },
  { id: "codeacts", label: "CodeActs", glyph: "codeact" },
  { id: "hardware", label: "Hardware", glyph: "gauge" },
  { id: "godel", label: "Godel", glyph: "shield-check" },
  { id: "personality", label: "Personality", glyph: "masks" }
];

/* Segmented brain: the general brain is the default; the science and coding
   brains own the CodeActs specialized for their domain. The activator
   commands live in the general brain since they are the switches. */
const BRAIN_ACTIVATOR_COMMANDS: BrainCodeActCommand[] = ["/sciencebrain_", "/codingbrain_"];
const GOOGLE_SUITE_COMMANDS: BrainCodeActCommand[] = ["/googleweb_", "/gmail_", "/google_agenda_"];

const SCIENCE_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/newcompute_",
  "/selectcompute_",
  "/compute_<name>_",
  "/newobject_"
];

const CODING_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/workspace_",
  "/newmodule_",
  "/rust_port_adapter_",
  "/rust_state_store_"
];

const BRAIN_SEGMENTS: { id: string; label: string; glyph: string; commands?: BrainCodeActCommand[] }[] = [
  { id: "general", label: "general brain", glyph: "brain" },
  { id: "science", label: "science brain", glyph: "flask", commands: SCIENCE_BRAIN_COMMANDS },
  { id: "coding", label: "coding brain", glyph: "code", commands: CODING_BRAIN_COMMANDS }
];

type BrainCodeActDisplay = { command: BrainCodeActCommand; description: string };

const HIDDEN_BRAIN_CODEACT_COMMANDS = new Set<BrainCodeActCommand>(["/gmail_com", BRAIN_RENAME_SESSION_COMMAND]);

const BRAIN_CODEACT_UI_DESCRIPTIONS: Partial<Record<BrainCodeActCommand, string>> = {
  "/sciencebrain_": "Switch to science mode for math, engineering, simulation, 3D, or technical analysis.",
  "/codingbrain_": "Switch to coding mode for software projects, files, bugs, builds, and developer tasks.",
  "/searcharchive_": "Search past chats and saved sessions when earlier context can help.",
  "/googleweb_": "Search the web for current public information.",
  "/gmail_": "Use Gmail to find messages, summarize email, or prepare replies.",
  "/airbnb_": "Use Airbnb to search for stays by place, dates, guests, and budget.",
  "/newimage_": "Create a new image from a text description.",
  "/editimage_": "Edit an existing image, such as changing its style, colors, objects, or layout.",
  "/google_agenda_": "Use Google Calendar for events, schedules, reminders, and dates.",
  "/brain_": "Save or update useful memory after the user confirms it is correct.",
  "/questionnaire_": "Ask a short set of questions when the task needs clearer choices.",
  "/newcompute_": "Start a new heavy local calculation, such as a simulation or numeric analysis.",
  "/selectcompute_": "Reuse a saved calculation instead of rebuilding the same work.",
  "/compute_<name>_": "Run a known saved calculation by name when it matches the request.",
  "/newobject_": "Create or modify a 3D object, scene, geometry, material, or design asset.",
  "/workspace_": "Ask the user to choose a local project folder before reading or changing files.",
  "/frontdesign_": "Change the app display colors or color palettes when the user asks.",
  "/newmodule_": "Create a small new app module or feature area.",
  "/rust_port_adapter_": "Add a Rust service bridge when a feature needs native backend access.",
  "/rust_state_store_": "Create durable local storage for settings, indexes, credentials, or cached data.",
  "/web_": "Open or control a web page inside the contained browser."
};

const NEW_COMPUTE_DETAIL_SECTIONS = [
  {
    label: "Measured token savings",
    text: "Local Monster GPU test: about 32,821 tokens saved on one fully slotted Li-ion electrochemical thermal safety compute, compared with carrying the full contract, Forge source, artifact, and proof context in LLM text."
  },
  {
    label: "New Compute templates",
    text: "Formula symbolic, numeric model, simulation dynamics, optimization design, uncertainty statistics, tensor/linalg/autodiff, signal/time-series, and graph/sparse/discrete."
  },
  {
    label: "What that means",
    text: "Use the matching template to run symbolic math, numeric engineering models, dynamic simulations, design optimization, uncertainty estimates, tensor or gradient work, signal analysis, or graph/discrete compute."
  },
  {
    label: "Use the results for",
    text: "Feed compact proof-backed results into 3D objects, simulation scenes, biology/DNA exercises, crypto exercises, trading models, real-estate scoring, logistics plans, or research reports."
  }
] as const;
const BRAIN_SESSION_ARCHIVE_INITIAL_COUNT = 6;
const BRAIN_SESSION_ARCHIVE_STEP = 8;

function codeActDisplay(command: BrainCodeActCommand, fallbackDescription = ""): BrainCodeActDisplay {
  return {
    command,
    description: BRAIN_CODEACT_UI_DESCRIPTIONS[command] ?? fallbackDescription
  };
}

function segmentCodeActs(segment: { commands?: BrainCodeActCommand[] }) {
  const elsewhere = new Set<BrainCodeActCommand>([...SCIENCE_BRAIN_COMMANDS, ...CODING_BRAIN_COMMANDS, ...BRAIN_ACTIVATOR_COMMANDS, ...GOOGLE_SUITE_COMMANDS]);
  return BRAIN_CODEACT_COMMAND_DESCRIPTIONS.filter(({ command }) =>
    !HIDDEN_BRAIN_CODEACT_COMMANDS.has(command) && (segment.commands ? segment.commands.includes(command) : !elsewhere.has(command))
  ).map(({ command, description }) => codeActDisplay(command, description));
}

function activatorCodeActs() {
  return BRAIN_ACTIVATOR_COMMANDS.map((command) => codeActDisplay(command));
}

function googleSuiteCodeActs() {
  return GOOGLE_SUITE_COMMANDS.map((command) => codeActDisplay(command));
}

function isRestorableBrainSession(item: SidebarSessionItem): boolean {
  return item.sessionId.startsWith("chat-") || item.sessionId.startsWith("parallel-chat-");
}

function brainSessionArchiveItems(recentItems: SidebarSessionItem[], archivedItems: SidebarSessionItem[]): SidebarSessionItem[] {
  const seen = new Set<string>();
  return [...recentItems, ...archivedItems].filter((item) => {
    const label = item.label.trim();
    if (!label || !isRestorableBrainSession(item)) return false;
    const key = item.sessionId || `${label}:${item.date}:${item.workspaceLabel}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return item.rowVisible || item.archived || item.pinned || item.working;
  });
}

function compactSessionAgeLabel(date: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(date.trim());
  if (!match) return date;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) return date;
  const now = new Date();
  const todayUtc = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const sessionUtc = Date.UTC(year, month - 1, day);
  const deltaDays = Math.floor((todayUtc - sessionUtc) / 86_400_000);
  if (!Number.isFinite(deltaDays)) return date;
  if (deltaDays <= 0) return "today";
  if (deltaDays < 7) return `${deltaDays}d ago`;
  if (deltaDays < 70) return `${Math.floor(deltaDays / 7)}w ago`;
  if (deltaDays < 365) return `${Math.floor(deltaDays / 30.44)}mo ago`;
  return `${Math.floor(deltaDays / 365.25)}y ago`;
}

function BrainSessionArchiveList({
  sessions,
  visibleCount,
  onShowMore,
  onOpenSession
}: {
  sessions: SidebarSessionItem[];
  visibleCount: number;
  onShowMore: () => void;
  onOpenSession: (session: SidebarSessionItem) => void;
}) {
  if (sessions.length === 0) {
    return (
      <div className="brainSessionArchiveList brainSessionArchiveList--empty" aria-label="Saved sessions">
        No saved sessions yet.
      </div>
    );
  }
  const visibleSessions = sessions.slice(0, visibleCount);
  const hiddenCount = Math.max(0, sessions.length - visibleSessions.length);
  return (
    <>
      <div className="brainSessionArchiveList" role="list" aria-label="Saved sessions">
        {visibleSessions.map((session) => (
          <button
            type="button"
            className="brainSessionArchiveItem"
            role="listitem"
            key={session.sessionId || `${session.label}-${session.date}`}
            onClick={() => onOpenSession(session)}
          >
            <span className="brainSessionArchiveItem__line">
              <time className="brainSessionArchiveItem__date" dateTime={session.date} title={session.date}>
                {compactSessionAgeLabel(session.date)}
              </time>
              <span className="brainSessionArchiveItem__title">{session.label}</span>
            </span>
            <span className="brainSessionArchiveItem__meta">
              <span>{session.workspaceLabel || session.section}</span>
              {session.archived ? <span>Archived</span> : null}
            </span>
          </button>
        ))}
      </div>
      {hiddenCount > 0 ? (
        <button type="button" className="brainSessionArchiveMore" onClick={onShowMore}>
          Afficher plus
          <span>{hiddenCount}</span>
        </button>
      ) : null}
    </>
  );
}

function SlotRow({
  glyph,
  icon,
  title,
  text,
  status,
  active = false
}: {
  glyph?: string;
  icon?: React.ReactNode;
  title: string;
  text: string;
  status?: string;
  active?: boolean;
}) {
  return (
    <div className="brainSlotRow" role="listitem">
      <span className="brainRow__icon">{icon ?? <Glyph kind={glyph ?? "terminal"} size={17} />}</span>
      <span className="brainSlotRow__body">
        <strong>{title}</strong>
        <span>{text}</span>
      </span>
      {status ? (
        <span className={active ? "brainStatus brainStatus--active" : "brainStatus"}>
          <i aria-hidden="true" />
          {status}
        </span>
      ) : null}
    </div>
  );
}

function CodeActRow({ command, description }: { command: BrainCodeActCommand; description: string }) {
  const canExpand = command === "/newcompute_";
  const [expanded, setExpanded] = useState(false);
  const detailsId = "brain-new-compute-details";
  return (
    <div className={canExpand ? "brainRow brainRow--expandable" : "brainRow"} role="listitem">
      <span className="brainRow__icon">
        <CodeActIcon command={command} />
      </span>
      <span className="brainRow__commandLine">
        <code>{command}</code>
        {canExpand ? (
          <button
            type="button"
            className="brainRow__expandButton"
            aria-expanded={expanded}
            aria-controls={detailsId}
            aria-label={expanded ? "Hide Codex New Compute details" : "Show Codex New Compute details"}
            onClick={() => setExpanded((isExpanded) => !isExpanded)}
          >
            <Glyph kind={expanded ? "minus" : "plus"} size={14} />
          </button>
        ) : null}
      </span>
      <p>{description}</p>
      {canExpand && expanded ? (
        <div className="brainComputeDetails" id={detailsId} role="region" aria-label="Codex New Compute capabilities">
          <strong>Codex New Compute</strong>
          <div className="brainComputeDetails__grid">
            {NEW_COMPUTE_DETAIL_SECTIONS.map((section) => (
              <span className="brainComputeDetails__item" key={section.label}>
                <b>{section.label}</b>
                <span>{section.text}</span>
              </span>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function BrainMemoryIdentityField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="brainMemoryIdentityField">
      <span className="brainMemoryIdentityField__body">
        <span className="brainMemoryIdentityField__label">{label}</span>
        <span className="brainMemoryIdentityField__control">
          <input
            aria-label={label}
          className="brainMemoryIdentityField__input"
          type="text"
          value={value}
          size={Math.max(10, Math.min(value.length || 10, 24))}
          placeholder="Write here"
          spellCheck={false}
          onChange={(event) => onChange(event.currentTarget.value)}
          />
          <span className="brainMemoryIdentityField__edit" aria-hidden="true">
            <Glyph kind="pencil" size={10} />
          </span>
        </span>
      </span>
    </label>
  );
}

function BrainMemoryLocationField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const listboxId = useId();
  const [focused, setFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");
  const query = value.trim();
  const expanded = focused && suggestions.length > 0;

  useEffect(() => {
    setActiveIndex(0);
  }, [value]);

  useEffect(() => {
    if (!focused || query.length < 2) {
      setSuggestions([]);
      setStatus("idle");
      return undefined;
    }
    let cancelled = false;
    setStatus("loading");
    const timer = window.setTimeout(() => {
      const searchCitySuggestions = window.forgeShell?.searchCitySuggestions;
      const request = searchCitySuggestions
        ? searchCitySuggestions(query).then((result) => result.suggestions.map((suggestion) => suggestion.label))
        : fallbackPhotonCitySuggestionLabels(query);
      request
        .then((labels) => {
          if (cancelled) return;
          setSuggestions(labels);
          setStatus("idle");
        })
        .catch(() => {
          if (cancelled) return;
          setSuggestions([]);
          setStatus("error");
        });
    }, 220);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [focused, query]);

  const chooseSuggestion = (suggestion: string) => {
    onChange(suggestion);
    setFocused(false);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (!expanded) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => Math.min(suggestions.length - 1, current + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => Math.max(0, current - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      chooseSuggestion(suggestions[activeIndex] ?? suggestions[0]);
    } else if (event.key === "Escape") {
      setFocused(false);
    }
  };

  return (
    <label className="brainMemoryIdentityField brainMemoryIdentityField--location">
      <span className="brainMemoryIdentityField__body">
        <span className="brainMemoryIdentityField__label">{label}</span>
        <span className="brainMemoryIdentityField__control brainMemoryIdentityField__control--location">
          <input
            aria-activedescendant={expanded ? `${listboxId}-${activeIndex}` : undefined}
            aria-autocomplete="list"
            aria-controls={expanded ? listboxId : undefined}
            aria-expanded={expanded}
            aria-label={label}
            className="brainMemoryIdentityField__input"
            role="combobox"
            type="text"
            value={value}
            size={Math.max(14, Math.min(value.length || 14, 34))}
            placeholder="City"
            spellCheck={false}
            onBlur={() => window.setTimeout(() => setFocused(false), 120)}
            onChange={(event) => onChange(event.currentTarget.value)}
            onFocus={() => setFocused(true)}
            onKeyDown={onKeyDown}
          />
          <span className="brainMemoryIdentityField__edit" aria-hidden="true">
            <Glyph kind="pencil" size={10} />
          </span>
          {expanded ? (
            <span className="brainMemoryLocationSuggestions" id={listboxId} role="listbox">
              {suggestions.map((suggestion, index) => (
                <button
                  type="button"
                  className={[
                    "brainMemoryLocationSuggestions__option",
                    index === activeIndex ? "brainMemoryLocationSuggestions__option--active" : ""
                  ].filter(Boolean).join(" ")}
                  id={`${listboxId}-${index}`}
                  key={suggestion}
                  role="option"
                  aria-selected={index === activeIndex}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    chooseSuggestion(suggestion);
                  }}
                >
                  {suggestion}
                </button>
              ))}
            </span>
          ) : null}
          {focused && query.length >= 2 && suggestions.length === 0 && status !== "idle" ? (
            <span className="brainMemoryLocationStatus" role="status">
              {status === "loading" ? "..." : "Offline"}
            </span>
          ) : null}
        </span>
      </span>
    </label>
  );
}

function CodeActsSpace() {
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        CodeActs are autonomous commands the agent runs to move faster and do real work beyond chat.
        Some control a web browser in a contained, controlled environment; others create 3D objects or run heavy science and analysis locally, replacing work that would otherwise burn hundreds of millions of tokens.
      </p>
      <div className="brainCanvas__segments">
        {BRAIN_SEGMENTS.map((segment) => (
          <section className="brainSegment" key={segment.id} aria-label={segment.label}>
            <h2 className="brainSegment__head">
              <Glyph kind={segment.glyph} size={14} />
              {segment.label}
            </h2>
            {segment.id === "general" ? (
              <>
                <div className="brainActivators" role="list" aria-label="brain activators">
                  {activatorCodeActs().map(({ command, description }) => (
                    <CodeActRow command={command} description={description} key={command} />
                  ))}
                </div>
                <div className="brainGoogleSuite" role="list" aria-label="Google Suite">
                  <p className="brainCommandPack__label">Google Suite</p>
                  {googleSuiteCodeActs().map(({ command, description }) => (
                    <CodeActRow command={command} description={description} key={command} />
                  ))}
                </div>
              </>
            ) : null}
            <div className="brainCanvas__rows" role="list">
              {segmentCodeActs(segment).map(({ command, description }) => (
                <CodeActRow command={command} description={description} key={command} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

function MemorySpace() {
  const [userMemory, setUserMemory] = useState(() => readBrainUserMemory());
  const [agentMemory, setAgentMemory] = useState(() => readBrainAgentMemory());
  const [locationMemory, setLocationMemory] = useState(() => readBrainUserLocationMemory());
  const [visibleArchiveCount, setVisibleArchiveCount] = useState(BRAIN_SESSION_ARCHIVE_INITIAL_COUNT);
  const { snapshot: sidebarSnapshot } = useSidebarShadowStore();
  const sessions = brainSessionArchiveItems(sidebarSnapshot.recentItems, sidebarSnapshot.archivedItems);

  useEffect(() => {
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: userMemory.preferredFirstName,
      agentFirstName: agentMemory.preferredFirstName,
      userHomeLocation: locationMemory.homeLocation
    });
  }, [agentMemory.preferredFirstName, locationMemory.homeLocation, userMemory.preferredFirstName]);

  useEffect(() => {
    setVisibleArchiveCount((current) => Math.min(Math.max(current, BRAIN_SESSION_ARCHIVE_INITIAL_COUNT), Math.max(sessions.length, BRAIN_SESSION_ARCHIVE_INITIAL_COUNT)));
  }, [sessions.length]);

  const commitUserMemory = (value: string) => {
    const next = writeBrainUserMemory(value);
    setUserMemory(next);
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: next.preferredFirstName,
      agentFirstName: agentMemory.preferredFirstName,
      userHomeLocation: locationMemory.homeLocation
    });
  };

  const commitAgentMemory = (value: string) => {
    const next = writeBrainAgentMemory(value);
    setAgentMemory(next);
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: userMemory.preferredFirstName,
      agentFirstName: next.preferredFirstName,
      userHomeLocation: locationMemory.homeLocation
    });
  };

  const commitLocationMemory = (value: string) => {
    const next = writeBrainUserLocationMemory(value);
    setLocationMemory(next);
    void panelsChatBottomStore.dispatch({
      kind: "update_brain_identity",
      userFirstName: userMemory.preferredFirstName,
      agentFirstName: agentMemory.preferredFirstName,
      userHomeLocation: next.homeLocation
    });
  };

  const openArchivedSession = async (session: SidebarSessionItem) => {
    if (session.sessionId) {
      await sidebarShadowStore.dispatch(
        sidebarShadowStore.command({ kind: "open_session", sessionId: session.sessionId, section: session.section }),
        session.sessionId
      );
      await panelsChatBottomStore.refresh();
      await headerShadowStore.boot();
      return;
    }
    await sidebarShadowStore.dispatch(
      sidebarShadowStore.command({ kind: "navigate", section: session.section }),
      session.label
    );
    await headerShadowStore.boot();
  };

  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        Memory keeps the names and session history the agent can reuse when it helps the current conversation.
      </p>
      <div className="brainCanvas__rows" role="list">
        <section className="brainMemoryIdentity" aria-label="Visitor and agent names">
          <p className="brainMemoryIdentity__label">
            <Glyph kind="identity-card" size={18} />
            <span>Identity</span>
          </p>
          <div className="brainMemoryIdentity__fields">
            <BrainMemoryIdentityField
              label="Your name"
              value={userMemory.preferredFirstName}
              onChange={commitUserMemory}
            />
            <BrainMemoryIdentityField
              label="Agent name"
              value={agentMemory.preferredFirstName}
              onChange={commitAgentMemory}
            />
            <BrainMemoryLocationField
              label="Home city"
              value={locationMemory.homeLocation}
              onChange={commitLocationMemory}
            />
          </div>
        </section>
        <div className="brainSessionArchiveHead" role="listitem">
          <span className="brainRow__icon">
            <Glyph kind="archive" size={17} />
          </span>
          <span className="brainSessionArchiveHead__body">
            <strong>Session archive</strong>
            <span>Saved conversations, decisions, and working context the agent can recall when useful.</span>
          </span>
        </div>
        <BrainSessionArchiveList
          sessions={sessions}
          visibleCount={visibleArchiveCount}
          onShowMore={() => setVisibleArchiveCount((current) => Math.min(sessions.length, current + BRAIN_SESSION_ARCHIVE_STEP))}
          onOpenSession={openArchivedSession}
        />
      </div>
    </div>
  );
}

const HARDWARE_POLL_MS = 1000;
const HARDWARE_REQUEST_TIMEOUT_MS = 6500;
const HARDWARE_GAUGE_HISTORY_LIMIT = 96;
const HARDWARE_GAUGE_MIN_TEMP_SPAN_C = 18;

type HardwareGaugeSample = {
  sampledAt: number;
  temperature: number | null;
  percent: number | null;
};

type HardwareMonitorCardId = "gpu" | "cpu" | "system";

type HardwareMonitorCardView = {
  id: HardwareMonitorCardId;
  title: string;
  detail: string;
  badge: string;
  temperature: HardwareMetric;
  percent: HardwareMetric;
  min: number;
  max: number;
};

type HardwareGaugePoint = {
  x: number;
  y: number;
};

function clampNumber(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function hardwareFallbackMetric(label: string, value: number | null, unit: HardwareMetric["unit"]): HardwareMetric {
  return {
    label,
    value,
    unit,
    status: value === null ? "unavailable" : "ok"
  };
}

function cpuTemperatureGaugeMetric(snapshot: HardwareTelemetrySnapshot): HardwareMetric {
  if (snapshot.thermal.source === "linux-thermal") {
    return { ...snapshot.thermal.systemTemperature, label: "CPU" };
  }
  return hardwareFallbackMetric("CPU", null, "C");
}

function hardwareGpuTitle(name: string | undefined): string {
  if (!name || name.toLowerCase().includes("unavailable")) return "GPU";
  return name;
}

function hardwareCardViews(snapshot: HardwareTelemetrySnapshot): HardwareMonitorCardView[] {
  const primaryGpu = snapshot.gpus[0];
  return [
    {
      id: "gpu",
      title: hardwareGpuTitle(primaryGpu?.name),
      detail: "Température / Util.",
      badge: "GPU",
      temperature: primaryGpu ? { ...primaryGpu.temperature, label: "GPU" } : hardwareFallbackMetric("GPU", null, "C"),
      percent: primaryGpu ? { ...primaryGpu.utilization, label: "GPU" } : hardwareFallbackMetric("GPU", null, "%"),
      min: 25,
      max: 105
    },
    {
      id: "cpu",
      title: snapshot.cpu.model || "CPU",
      detail: "Température / Util.",
      badge: "CPU",
      temperature: cpuTemperatureGaugeMetric(snapshot),
      percent: { ...snapshot.cpu.utilization, label: "CPU" },
      min: 25,
      max: 105
    },
    {
      id: "system",
      title: "Système",
      detail: "Température / RAM",
      badge: "Système",
      temperature: { ...snapshot.thermal.systemTemperature, label: "Système" },
      percent: { ...snapshot.memory.utilization, label: "RAM" },
      min: 25,
      max: 105
    }
  ];
}

function compactGaugeValue(metric: HardwareMetric): string {
  if (metric.value === null) return "--";
  if (metric.unit === "%" || metric.unit === "C") return String(Math.round(metric.value));
  return String(metric.value);
}

function compactGaugeUnit(metric: HardwareMetric): string {
  if (metric.value === null || metric.unit === "count" || metric.unit === "text") return "";
  return metric.unit;
}

function hardwareGaugeLevel(metric: HardwareMetric, min: number, max: number): number {
  if (metric.value === null || max <= min) return 0;
  return clampNumber(((metric.value - min) / (max - min)) * 100, 0, 100);
}

function hardwareTemperatureDomain(
  samples: HardwareGaugeSample[],
  current: number | null,
  fallbackMin: number,
  fallbackMax: number
): { min: number; max: number } {
  const values = [
    ...samples.map((sample) => sample.temperature),
    current
  ].filter((value): value is number => value !== null && Number.isFinite(value));
  if (values.length === 0) return { min: fallbackMin, max: fallbackMax };
  const observedMin = Math.min(...values);
  const observedMax = Math.max(...values);
  const center = (observedMin + observedMax) / 2;
  const span = Math.max(HARDWARE_GAUGE_MIN_TEMP_SPAN_C, observedMax - observedMin + 8);
  let min = center - span / 2;
  let max = center + span / 2;
  if (min < fallbackMin) {
    max += fallbackMin - min;
    min = fallbackMin;
  }
  if (max > fallbackMax) {
    min -= max - fallbackMax;
    max = fallbackMax;
  }
  return {
    min: clampNumber(min, fallbackMin, fallbackMax - 1),
    max: clampNumber(max, fallbackMin + 1, fallbackMax)
  };
}

function hardwareGaugePoints(
  samples: HardwareGaugeSample[],
  readValue: (sample: HardwareGaugeSample) => number | null,
  min: number,
  max: number,
  width = 278,
  height = 82
): HardwareGaugePoint[] {
  const visible = samples.slice(-HARDWARE_GAUGE_HISTORY_LIMIT);
  if (visible.length === 0 || max <= min) return [];
  const step = width / Math.max(1, HARDWARE_GAUGE_HISTORY_LIMIT - 1);
  const startX = width - (visible.length - 1) * step;
  return visible
    .map((sample, index) => {
      const rawValue = readValue(sample);
      const value = rawValue === null ? min : clampNumber(rawValue, min, max);
      const x = startX + index * step;
      const y = height - ((value - min) / (max - min)) * height;
      return { x, y };
    });
}

function hardwareGaugeSeriesPath(
  samples: HardwareGaugeSample[],
  readValue: (sample: HardwareGaugeSample) => number | null,
  min: number,
  max: number,
  width = 278,
  height = 82
): string {
  return hardwareGaugePoints(samples, readValue, min, max, width, height)
    .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x.toFixed(1)} ${point.y.toFixed(1)}`)
    .join(" ");
}

function hardwareGaugeStepPath(
  samples: HardwareGaugeSample[],
  readValue: (sample: HardwareGaugeSample) => number | null,
  min: number,
  max: number,
  width = 278,
  height = 82
): string {
  const points = hardwareGaugePoints(samples, readValue, min, max, width, height);
  return points
    .map((point, index) => {
      if (index === 0) return `M ${point.x.toFixed(1)} ${point.y.toFixed(1)}`;
      const previous = points[index - 1];
      return `H ${((previous.x + point.x) / 2).toFixed(1)} V ${point.y.toFixed(1)} H ${point.x.toFixed(1)}`;
    })
    .join(" ");
}

function hardwareGaugePeakPath(
  samples: HardwareGaugeSample[],
  readValue: (sample: HardwareGaugeSample) => number | null,
  min: number,
  max: number,
  width = 278,
  height = 82
): string {
  const visible = samples.slice(-HARDWARE_GAUGE_HISTORY_LIMIT);
  if (visible.length === 0 || max <= min) return "";
  const step = width / Math.max(1, HARDWARE_GAUGE_HISTORY_LIMIT - 1);
  const startX = width - (visible.length - 1) * step;
  return visible
    .map((sample, index) => {
      const rawValue = readValue(sample);
      if (rawValue === null) return "";
      const value = clampNumber(rawValue, min, max);
      const x = startX + index * step;
      const y = height - ((value - min) / (max - min)) * height;
      return `M ${x.toFixed(1)} ${height} L ${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .filter(Boolean)
    .join(" ");
}

function hardwareGaugeAreaPath(
  samples: HardwareGaugeSample[],
  readValue: (sample: HardwareGaugeSample) => number | null,
  min: number,
  max: number,
  width = 278,
  height = 82
): string {
  const line = hardwareGaugeSeriesPath(samples, readValue, min, max, width, height);
  if (!line) return "";
  const points = hardwareGaugePoints(samples, readValue, min, max, width, height);
  const first = points[0];
  const last = points[points.length - 1];
  return `M ${first.x.toFixed(1)} ${height} ${line.replace(/^M/, "L")} L ${last.x.toFixed(1)} ${height} Z`;
}

function hardwareTemperaturePath(samples: HardwareGaugeSample[], min: number, max: number): string {
  return hardwareGaugeSeriesPath(samples, (sample) => sample.temperature, min, max);
}

function hardwareTemperatureAreaPath(samples: HardwareGaugeSample[], min: number, max: number): string {
  return hardwareGaugeAreaPath(samples, (sample) => sample.temperature, min, max);
}

function hardwareActivityPath(samples: HardwareGaugeSample[]): string {
  return hardwareGaugeStepPath(samples, (sample) => sample.percent, 0, 100);
}

function hardwareActivityAreaPath(samples: HardwareGaugeSample[]): string {
  return hardwareGaugeAreaPath(samples, (sample) => sample.percent, 0, 100);
}

function hardwareActivityPeakPath(samples: HardwareGaugeSample[]): string {
  return hardwareGaugePeakPath(samples, (sample) => sample.percent, 0, 100);
}

function hardwareTemperatureMinMax(samples: HardwareGaugeSample[]): { min: number | null; max: number | null } {
  const values = samples
    .map((sample) => sample.temperature)
    .filter((value): value is number => value !== null && Number.isFinite(value));
  if (values.length === 0) {
    return { min: null, max: null };
  }
  return {
    min: Math.round(Math.min(...values)),
    max: Math.round(Math.max(...values))
  };
}

function rendererFallbackHardwareSnapshot(reason: string): HardwareTelemetrySnapshot {
  const cores = typeof navigator.hardwareConcurrency === "number" ? navigator.hardwareConcurrency : 0;
  const navigatorMemory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  const memoryGb = typeof navigatorMemory === "number" ? navigatorMemory : null;
  return {
    schema: "ingen.hardware.telemetry.snapshot.v1",
    platform: "unknown",
    arch: "renderer",
    hostname: "Renderer fallback",
    sampledAt: new Date().toISOString(),
    cpu: {
      model: "Browser renderer",
      cores,
      utilization: hardwareFallbackMetric("CPU load", null, "%"),
      loadAverage: hardwareFallbackMetric("Load average", null, "count")
    },
    memory: {
      used: hardwareFallbackMetric("RAM used", null, "GB"),
      total: hardwareFallbackMetric("RAM total", memoryGb, "GB"),
      utilization: hardwareFallbackMetric("RAM load", null, "%")
    },
    thermal: {
      systemTemperature: hardwareFallbackMetric("System temperature", null, "C"),
      source: "unavailable"
    },
    gpus: [
      {
        name: "GPU via IPC unavailable",
        vendor: "unknown",
        source: "unavailable",
        utilization: hardwareFallbackMetric("GPU load", null, "%"),
        memoryUsed: hardwareFallbackMetric("VRAM used", null, "GB"),
        memoryTotal: hardwareFallbackMetric("VRAM total", null, "GB"),
        temperature: hardwareFallbackMetric("GPU temperature", null, "C"),
        fanSpeed: hardwareFallbackMetric("Fan speed", null, "%"),
        powerDraw: hardwareFallbackMetric("Power draw", null, "W")
      }
    ],
    topProcesses: [],
    governor: {
      profile: "balanced",
      monsterBudgetPercent: 35,
      bangerBudgetPercent: 30,
      controlAuthority: "app-budget-only",
      fanControl: "locked",
      notes: [reason]
    },
    proofHash: `renderer-fallback-${Date.now().toString(36)}`
  };
}

async function requestHardwareTelemetry(): Promise<HardwareTelemetrySnapshot> {
  const api = window.forgeShell?.getHardwareTelemetrySnapshot;
  if (!api) {
    return rendererFallbackHardwareSnapshot("Hardware IPC is not exposed by the current preload.");
  }
  let timeoutId: number | undefined;
  try {
    return await Promise.race([
      api(),
      new Promise<HardwareTelemetrySnapshot>((resolve) => {
        timeoutId = window.setTimeout(() => {
          resolve(rendererFallbackHardwareSnapshot("Hardware IPC did not answer before the renderer timeout."));
        }, HARDWARE_REQUEST_TIMEOUT_MS);
      })
    ]);
  } finally {
    if (timeoutId !== undefined) {
      window.clearTimeout(timeoutId);
    }
  }
}

function HardwareGaugeCard({ card, samples }: { card: HardwareMonitorCardView; samples: HardwareGaugeSample[] }) {
  const temperatureDomain = hardwareTemperatureDomain(samples, card.temperature.value, card.min, card.max);
  const level = hardwareGaugeLevel(card.temperature, card.min, card.max);
  const temperatureLinePath = hardwareTemperaturePath(samples, temperatureDomain.min, temperatureDomain.max);
  const temperatureAreaPath = hardwareTemperatureAreaPath(samples, temperatureDomain.min, temperatureDomain.max);
  const activityLinePath = hardwareActivityPath(samples);
  const activityAreaPath = hardwareActivityAreaPath(samples);
  const activityPeakPath = hardwareActivityPeakPath(samples);
  const minMax = hardwareTemperatureMinMax(samples);
  const isUnavailable = card.temperature.value === null;
  return (
    <article className={["hardwareGauge", `hardwareGauge--${card.temperature.status}`].join(" ")} aria-label={card.badge}>
      <section className="hardwareGauge__main">
        <header className="hardwareGauge__head">
          <strong>{card.title}</strong>
          <span>{card.detail}</span>
        </header>
        <div className="hardwareGauge__screen">
          <svg viewBox="0 0 278 82" preserveAspectRatio="none" aria-hidden="true">
            <defs>
              <linearGradient id={`hardwareGaugeTempStroke-${card.id}`} x1="0" x2="0" y1="82" y2="0" gradientUnits="userSpaceOnUse">
                <stop offset="0%" stopColor="#f1b735" />
                <stop offset="54%" stopColor="#ff8a00" />
                <stop offset="100%" stopColor="#ff315f" />
              </linearGradient>
              <linearGradient id={`hardwareGaugeTempFill-${card.id}`} x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stopColor="#ff315f" stopOpacity="0.3" />
                <stop offset="62%" stopColor="#ff8a00" stopOpacity="0.12" />
                <stop offset="100%" stopColor="#f1b735" stopOpacity="0.02" />
              </linearGradient>
              <linearGradient id={`hardwareGaugeActivityFill-${card.id}`} x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stopColor="#f1b735" stopOpacity="0.22" />
                <stop offset="100%" stopColor="#f1b735" stopOpacity="0.02" />
              </linearGradient>
            </defs>
            <path className="hardwareGauge__gridLine" d="M 0 20.5 H 278 M 0 41 H 278 M 0 61.5 H 278" />
            {activityAreaPath ? (
              <path className="hardwareGauge__activityArea" d={activityAreaPath} fill={`url(#hardwareGaugeActivityFill-${card.id})`} />
            ) : null}
            {temperatureAreaPath ? (
              <path className="hardwareGauge__temperatureArea" d={temperatureAreaPath} fill={`url(#hardwareGaugeTempFill-${card.id})`} />
            ) : null}
            {activityPeakPath ? <path className="hardwareGauge__activityPeak" d={activityPeakPath} /> : null}
            {activityLinePath ? <path className="hardwareGauge__activityLine" d={activityLinePath} /> : null}
            {temperatureLinePath ? (
              <path className="hardwareGauge__temperatureLine" d={temperatureLinePath} stroke={`url(#hardwareGaugeTempStroke-${card.id})`} />
            ) : null}
          </svg>
        </div>
        <div className="hardwareGauge__range">
          <span>Min : {minMax.min === null ? "--" : `${minMax.min}°`}</span>
          <span>Max : {minMax.max === null ? "--" : `${minMax.max}°`}</span>
        </div>
      </section>
      <div className="hardwareGauge__readout">
        <span>{card.badge}</span>
        <strong>{compactGaugeValue(card.temperature)}</strong>
        {compactGaugeUnit(card.temperature) ? <em>{compactGaugeUnit(card.temperature)}</em> : null}
        <strong className="hardwareGauge__percent">{compactGaugeValue(card.percent)}</strong>
        {compactGaugeUnit(card.percent) ? <em>{compactGaugeUnit(card.percent)}</em> : null}
      </div>
      <div className="hardwareGauge__level" aria-hidden="true">
        <i style={{ height: `${isUnavailable ? 0 : level}%` }} />
      </div>
    </article>
  );
}

function HardwareSpace() {
  const [snapshot, setSnapshot] = useState<HardwareTelemetrySnapshot | null>(null);
  const [error, setError] = useState("");
  const [history, setHistory] = useState<Record<HardwareMonitorCardId, HardwareGaugeSample[]>>({
    gpu: [],
    cpu: [],
    system: []
  });

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const refresh = () => {
      void requestHardwareTelemetry()
        .then((next) => {
          if (cancelled) return;
          setSnapshot(next ?? null);
          setError(next ? "" : "Telemetry unavailable");
          if (next) {
            const sampledAt = new Date(next.sampledAt).getTime();
            const sampleTime = Number.isFinite(sampledAt) ? sampledAt : Date.now();
            const cards = hardwareCardViews(next);
            setHistory((current) => {
              const updated = { ...current };
              for (const card of cards) {
                updated[card.id] = [
                  ...(updated[card.id] ?? []),
                  {
                    sampledAt: sampleTime,
                    temperature: card.temperature.value,
                    percent: card.percent.value
                  }
                ].slice(-HARDWARE_GAUGE_HISTORY_LIMIT);
              }
              return updated;
            });
          }
        })
        .catch((caught) => {
          if (cancelled) return;
          setError(caught instanceof Error ? caught.message : "Telemetry unavailable");
        })
        .finally(() => {
          if (!cancelled) {
            timer = window.setTimeout(refresh, HARDWARE_POLL_MS);
          }
        });
    };
    refresh();
    return () => {
      cancelled = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
  }, []);

  if (!snapshot) {
    return (
      <div className="brainCanvas__space">
        <div className="hardwareDashboard hardwareDashboard--loading" role="status">
          {error || "Loading telemetry"}
        </div>
      </div>
    );
  }

  const cards = hardwareCardViews(snapshot);

  return (
    <div className="brainCanvas__space">
      <div className="hardwareDashboard">
        <section className="hardwareGaugeBoard" aria-label="Hardware surveillance">
          {cards.map((card) => (
            <HardwareGaugeCard card={card} samples={history[card.id] ?? []} key={card.id} />
          ))}
        </section>
      </div>
    </div>
  );
}

function GodelSpace() {
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        Godel is the verification machine between intent and execution.
      </p>
      <p className="brainCanvas__pipeline">
        BrainCommand <i>-&gt;</i> Godel <i>-&gt;</i> Forge bytecode <i>-&gt;</i> Monster <i>-&gt;</i> proof
      </p>
      <div className="brainCanvas__rows" role="list">
        <SlotRow
          glyph="shield-check"
          title="Semantic verification"
          text="Every CodeAct command is checked against its typed contract before bytecode is emitted."
          status="active"
          active
        />
        <SlotRow
          glyph="terminal"
          title="Proof hashes"
          text="Monster compute returns verifiable artifacts with content-addressed proofs, not generated answers."
          status="active"
          active
        />
      </div>
    </div>
  );
}

function PersonalitySpace() {
  const memory = readBrainUserMemory();
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        How the agent addresses you, and how far it is allowed to act.
      </p>
      <div className="brainCanvas__rows" role="list">
        <SlotRow
          glyph="masks"
          title={memory.preferredFirstName.trim() || "No name set"}
          text="Preferred first name, used across welcome messages and session prose."
          status={memory.trust.replaceAll("_", " ")}
          active
        />
        <SlotRow
          glyph="pencil"
          title="Tone"
          text="Compact, technical, proof-first. Custom tone profiles land here."
          status="soon"
        />
        <SlotRow
          glyph="shield-check"
          title="Autonomy"
          text="Side-effect actions — send, pay, delete — always stay user-confirmed."
          status="soon"
        />
      </div>
    </div>
  );
}

export function BrainCanvas({ onClose }: { onClose?: () => void }) {
  const [space, setSpace] = useState<BrainSpace>("memory");
  return (
    <section className="profileCanvas brainCanvas" aria-label="Brain canvas">
      <BrainBlob />
      <header className="brainCanvas__head">
        <button type="button" className="brainCanvas__close" aria-label="Close Brain" title="Close Brain" onClick={onClose}>
          <span aria-hidden="true" />
        </button>
        <span className="brainCanvas__mark"><Glyph kind="brain" size={26} /></span>
        <h1>Brain</h1>
      </header>
      <div className="brainCanvas__tabs" role="tablist" aria-label="Brain spaces">
        {BRAIN_SPACES.map(({ id, label, glyph }) => (
          <button
            type="button"
            role="tab"
            aria-selected={space === id}
            key={id}
            onClick={() => setSpace(id)}
          >
            <Glyph kind={glyph} size={20} />
            {label}
          </button>
        ))}
      </div>
      {space === "codeacts" ? <CodeActsSpace /> : null}
      {space === "memory" ? <MemorySpace /> : null}
      {space === "hardware" ? <HardwareSpace /> : null}
      {space === "godel" ? <GodelSpace /> : null}
      {space === "personality" ? <PersonalitySpace /> : null}
    </section>
  );
}
