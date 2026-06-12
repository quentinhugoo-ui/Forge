import { useState } from "react";
import { BRAIN_CODEACT_COMMAND_DESCRIPTIONS, type BrainCodeActCommand } from "../shared/ipc-contract";
import { readBrainUserMemory } from "./brain-user-memory-store";
import { AirbnbIcon, CubeIcon, GmailIcon, GoogleIcon } from "./module-logos";

type BrainSpace = "codeacts" | "memory" | "godel" | "personality";

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
  { id: "codeacts", label: "CodeActs", glyph: "codeact" },
  { id: "memory", label: "Memory", glyph: "database" },
  { id: "godel", label: "Godel", glyph: "shield-check" },
  { id: "personality", label: "Personality", glyph: "masks" }
];

/* Segmented brain: the general brain is the default; the science and coding
   brains own the CodeActs specialized for their domain. The activator
   commands live in the general brain since they are the switches. */
const BRAIN_ACTIVATOR_COMMANDS: BrainCodeActCommand[] = ["/sciencebrain_", "/codingbrain_"];

const SCIENCE_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/newcompute_",
  "/selectcompute_",
  "/compute_<name>_",
  "/newobject_"
];

const CODING_BRAIN_COMMANDS: BrainCodeActCommand[] = [
  "/workspace_",
  "/frontdesign_",
  "/newmodule_",
  "/rust_port_adapter_",
  "/rust_state_store_"
];

const BRAIN_SEGMENTS: { id: string; label: string; glyph: string; commands?: BrainCodeActCommand[] }[] = [
  { id: "general", label: "general brain", glyph: "brain" },
  { id: "science", label: "science brain", glyph: "flask", commands: SCIENCE_BRAIN_COMMANDS },
  { id: "coding", label: "coding brain", glyph: "code", commands: CODING_BRAIN_COMMANDS }
];

function segmentCodeActs(segment: { commands?: BrainCodeActCommand[] }) {
  const elsewhere = new Set([...SCIENCE_BRAIN_COMMANDS, ...CODING_BRAIN_COMMANDS, ...BRAIN_ACTIVATOR_COMMANDS]);
  return BRAIN_CODEACT_COMMAND_DESCRIPTIONS.filter(({ command }) =>
    segment.commands ? segment.commands.includes(command) : !elsewhere.has(command)
  );
}

function activatorCodeActs() {
  return BRAIN_CODEACT_COMMAND_DESCRIPTIONS.filter(({ command }) => BRAIN_ACTIVATOR_COMMANDS.includes(command));
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
  status: string;
  active?: boolean;
}) {
  return (
    <div className="brainSlotRow" role="listitem">
      <span className="brainRow__icon">{icon ?? <Glyph kind={glyph ?? "terminal"} size={17} />}</span>
      <span className="brainSlotRow__body">
        <strong>{title}</strong>
        <span>{text}</span>
      </span>
      <span className={active ? "brainStatus brainStatus--active" : "brainStatus"}>
        <i aria-hidden="true" />
        {status}
      </span>
    </div>
  );
}

function CodeActRow({ command, description }: { command: BrainCodeActCommand; description: string }) {
  return (
    <div className="brainRow" role="listitem">
      <span className="brainRow__icon">
        <CodeActIcon command={command} />
      </span>
      <code>{command}</code>
      <p>{description}</p>
    </div>
  );
}

function CodeActsSpace() {
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        The agent acts by emitting CodeAct commands — typed contracts projected from the Rust Brain.
        The general brain is always on; the science and coding brains activate on demand.
      </p>
      <div className="brainCanvas__segments">
        {BRAIN_SEGMENTS.map((segment) => (
          <section className="brainSegment" key={segment.id} aria-label={segment.label}>
            <h2 className="brainSegment__head">
              <Glyph kind={segment.glyph} size={14} />
              {segment.label}
            </h2>
            {segment.id === "general" ? (
              <div className="brainActivators" role="list" aria-label="brain activators">
                <p className="brainActivators__label">brain switches</p>
                {activatorCodeActs().map(({ command, description }) => (
                  <CodeActRow command={command} description={description} key={command} />
                ))}
              </div>
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
  const memory = readBrainUserMemory();
  return (
    <div className="brainCanvas__space">
      <p className="brainCanvas__spaceIntro">
        Evidence-aware memory. Each slot keeps its scope, trust level and the evidence that wrote it.
      </p>
      <div className="brainCanvas__rows" role="list">
        <SlotRow
          glyph="database"
          title={memory.preferredFirstName}
          text="user.identity.first_name — preferred first name, seeded from the local profile."
          status={memory.trust.replaceAll("_", " ")}
          active
        />
        <SlotRow
          glyph="archive"
          title="Session archive"
          text="Past sessions and decisions, recalled on demand through /searcharchive_."
          status="indexed"
          active
        />
        <SlotRow
          glyph="zap"
          title="Compute library"
          text="Verified Monster computes saved for reuse through /selectcompute_."
          status="indexed"
          active
        />
        <SlotRow
          glyph="brain"
          title="Next slots"
          text="New memory is written through /brain_ once evidence is confirmed."
          status="awaiting evidence"
        />
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
          title={memory.preferredFirstName}
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

/* Organic gooey blob (Uiverse, andrew-manzyk): blurred polygons rotating inside
   an SVG mask, sharpened by a high-contrast filter. Sphere shell removed; the
   bare effect floats behind the page text as a slow ambient motion.
   Rendered at native 520px (no transform scale) so the blur/contrast filters
   stay sharp, with inner margins so the glow fades before the box edges. */
function BrainBlob() {
  return (
    <div className="brainBlob" aria-hidden="true">
      <svg width="520" height="520" viewBox="0 0 520 520">
        <mask id="brain-blob-mask">
          <polygon points="156,78 364,104 286,234" fill="#fff" />
          <polygon points="130,156 312,130 234,312" fill="#fff" />
          <polygon points="182,208 364,234 260,390" fill="#fff" />
          <polygon points="104,234 260,182 208,364" fill="#fff" />
          <polygon points="260,156 416,234 286,338" fill="#fff" />
          <polygon points="208,260 390,208 312,390" fill="#fff" />
          <polygon points="156,286 338,286 234,416" fill="#fff" />
        </mask>
      </svg>
      <div className="brainBlob__box" />
    </div>
  );
}

export function BrainCanvas() {
  const [space, setSpace] = useState<BrainSpace>("codeacts");
  return (
    <section className="profileCanvas brainCanvas" aria-label="Brain canvas">
      <BrainBlob />
      <header className="brainCanvas__head">
        <span className="brainCanvas__mark"><Glyph kind="brain" size={26} /></span>
        <div>
          <h1>Brain</h1>
          <p className="brainCanvas__sub">memory / CodeActs / Godel / personality</p>
        </div>
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
            <Glyph kind={glyph} size={14} />
            {label}
          </button>
        ))}
      </div>
      {space === "codeacts" ? <CodeActsSpace /> : null}
      {space === "memory" ? <MemorySpace /> : null}
      {space === "godel" ? <GodelSpace /> : null}
      {space === "personality" ? <PersonalitySpace /> : null}
    </section>
  );
}
