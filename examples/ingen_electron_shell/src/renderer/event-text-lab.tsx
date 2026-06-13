import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Copy, FolderPlus, List, MoveRight, Pencil, Search, Terminal, Trash2 } from "lucide-react";
import {
  AGENT_COPY_PATH_COMMAND,
  AGENT_CREATE_DIRECTORY_COMMAND,
  AGENT_DELETE_EMPTY_DIRECTORY_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
  AGENT_LIST_COMMAND,
  AGENT_MOVE_PATH_COMMAND,
  AGENT_READONLY_SHELL_COMMAND,
  AGENT_RENAME_PATH_COMMAND,
  AGENT_SEARCH_COMMAND,
  AGENT_SHELL_COMMAND,
  agentActionEventText,
  type AgentActionEventCommand
} from "./agent-action-events";
import {
  BRAIN_BRAIN_COMMAND,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_GOOGLE_AGENDA_COMMAND,
  BRAIN_NAMED_COMPUTE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_RUST_PORT_ADAPTER_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_WEB_COMMAND
} from "../shared/ipc-contract";
import "./event-text-lab.css";

type EventCommand =
  | typeof BRAIN_NEWCOMPUTE_COMMAND
  | typeof BRAIN_SELECTCOMPUTE_COMMAND
  | typeof BRAIN_NAMED_COMPUTE_COMMAND
  | "/compute_atomic_science_"
  | typeof BRAIN_WEB_COMMAND
  | typeof BRAIN_FRONTDESIGN_COMMAND
  | "FORGE_PLAN_JSON"
  | typeof BRAIN_GOOGLE_AGENDA_COMMAND
  | "FORGE_QUESTIONNAIRE_JSON"
  | typeof BRAIN_BRAIN_COMMAND
  | typeof BRAIN_SEARCHARCHIVE_COMMAND
  | typeof BRAIN_NEWOBJECT_COMMAND
  | "FORGE_BANGER_PLAN_JSON"
  | "FORGE_BANGER_QUESTIONNAIRE_JSON"
  | "FORGE_BANGER_MATERIAL_RESEARCH_JSON"
  | typeof BRAIN_RUST_PORT_ADAPTER_COMMAND
  | AgentActionEventCommand;

interface SuccessStep {
  before: string;
  pill: string;
  after: string;
  line2: string;
  command: EventCommand;
  eventText: string;
}

const templates = [
  ["/formula_symbolic", "CAS, expressions, proofs, and exact transformations"],
  ["/numeric_model", "deterministic formulas, units, bounds, and scalar outputs"],
  ["/simulation_dynamics", "states, time, equations, conditions, and integrators"],
  ["/optimization_design", "objectives, constraints, variables, and stopping criteria"],
  ["/uncertainty_statistics", "distributions, estimators, tolerances, and validation"],
  ["/tensor_linalg_autodiff", "tensors, linalg, gradients, jacobians, and layout"],
  ["/signal_timeseries", "series, FFT, filters, windows, and market signals"],
  ["/graph_sparse_discrete", "graphs, sparse data, bit ops, hashes, and discrete constraints"]
] as const;

const steps: SuccessStep[] = [
  {
    before: "Value compounds through",
    pill: "compute_library.sqlite",
    after: "instead of starting from zero.",
    line2: "Each compute saves a reusable asset and reduces token spend.",
    command: BRAIN_SELECTCOMPUTE_COMMAND,
    eventText: "saved compute reused from the library"
  },
  {
    before: "The business accelerates when",
    pill: "/compute_atomic_science_",
    after: "becomes a sellable circuit.",
    line2: "The atomic model turns local physics into a reusable artifact.",
    command: "/compute_atomic_science_",
    eventText: "atomic science compute executed"
  },
  {
    before: "Web research becomes",
    pill: "source-backed",
    after: ": the model stops guessing the market.",
    line2: "It triggers native collection and attaches sources to the reasoning.",
    command: BRAIN_WEB_COMMAND,
    eventText: "native web research event created"
  },
  {
    before: "The front end becomes",
    pill: "front_design",
    after: ": visible commercial proof.",
    line2: "Every visual decision is treated as a contract, not a fragile impression.",
    command: BRAIN_FRONTDESIGN_COMMAND,
    eventText: "front design contract projected"
  },
  {
    before: "Strategy becomes",
    pill: "FORGE_PLAN_JSON",
    after: "and appears immediately.",
    line2: "The plan is not buried in text: the user drives a clear trajectory.",
    command: "FORGE_PLAN_JSON",
    eventText: "side plan updated"
  },
  {
    before: "The calendar becomes",
    pill: "Google Calendar",
    after: ": appointments land without friction.",
    line2: "The LLM prepares the event, syncs the details, then confirms the action.",
    command: BRAIN_GOOGLE_AGENDA_COMMAND,
    eventText: "Google Calendar event created"
  },
  {
    before: "The product converts",
    pill: "questions",
    after: "into useful decisions at the right moment.",
    line2: "Less friction, fewer back-and-forth loops, more clarity in the session.",
    command: "FORGE_QUESTIONNAIRE_JSON",
    eventText: "structured questionnaire displayed"
  },
  {
    before: "Every session becomes",
    pill: "Brain-indexed",
    after: ": memory follows intent.",
    line2: "Context stays discoverable, and product memory gains quality.",
    command: BRAIN_BRAIN_COMMAND,
    eventText: "Brain memory indexed"
  },
  {
    before: "Past work becomes",
    pill: BRAIN_SEARCHARCHIVE_COMMAND,
    after: ": archived context returns on demand.",
    line2: "The agent retrieves proof, neighbor turns, and attachment references before it acts.",
    command: BRAIN_SEARCHARCHIVE_COMMAND,
    eventText: "archive memory search returned bounded context"
  },
  {
    before: "Banger turns",
    pill: "SDF object",
    after: "into controllable 3D creation.",
    line2: "The user no longer manipulates menus; they direct an engineering engine.",
    command: BRAIN_NEWOBJECT_COMMAND,
    eventText: "Banger SDF object created"
  },
  {
    before: "The scene advances with",
    pill: "Banger plan",
    after: "as an execution plan.",
    line2: "It becomes clear, shareable, and concrete enough for a client deliverable.",
    command: "FORGE_BANGER_PLAN_JSON",
    eventText: "Banger plan rendered in the panel"
  },
  {
    before: "Creation stays",
    pill: "collaborative",
    after: ": the engine asks before inventing.",
    line2: "When a decision is missing, it avoids weak assumptions and preserves trust.",
    command: "FORGE_BANGER_QUESTIONNAIRE_JSON",
    eventText: "structured Banger questions displayed"
  },
  {
    before: "The render gains",
    pill: "material research",
    after: "and professional depth.",
    line2: "Materials become engineering research, not disposable decoration.",
    command: "FORGE_BANGER_MATERIAL_RESEARCH_JSON",
    eventText: "material and component list computed"
  },
  {
    before: "The platform becomes",
    pill: "extensible",
    after: ": a vertical receives its Rust adapter.",
    line2: "The kernel stays intact, and the product does not dilute itself.",
    command: BRAIN_RUST_PORT_ADAPTER_COMMAND,
    eventText: "Rust adapter template prepared"
  },
  {
    before: "Local work becomes",
    pill: "fs.search",
    after: ": the agent inspects the computer with bounded evidence.",
    line2: "Search is visible as a tool event before any file operation follows.",
    command: AGENT_SEARCH_COMMAND,
    eventText: agentActionEventText(AGENT_SEARCH_COMMAND)
  },
  {
    before: "Filesystem changes become",
    pill: "fs.copy",
    after: ": explicit, traceable operations.",
    line2: "Moves, copies, renames, and directory creation use the same transcript grammar.",
    command: AGENT_COPY_PATH_COMMAND,
    eventText: agentActionEventText(AGENT_COPY_PATH_COMMAND)
  },
  {
    before: "Destructive work becomes",
    pill: "confirmed",
    after: ": deletion is visible before the result lands.",
    line2: "Recursive deletes keep their own event instead of disappearing into prose.",
    command: AGENT_DELETE_TREE_COMMAND,
    eventText: agentActionEventText(AGENT_DELETE_TREE_COMMAND)
  },
  {
    before: "System work becomes",
    pill: "shell.full",
    after: ": a confirmed command, not hidden magic.",
    line2: "PowerShell, cmd, batch, and native Windows tools appear as shell events.",
    command: AGENT_SHELL_COMMAND,
    eventText: agentActionEventText(AGENT_SHELL_COMMAND)
  }
];

function Pill({ children }: { children: string }) {
  return <code className="labPill">{children}</code>;
}

function GenericIcon({ done = true }: { done?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      {done ? <path d="m8.2 12.2 2.5 2.5 5-5.2" /> : <path d="M9 12h6" />}
    </svg>
  );
}

function NewComputeIcon() {
  return (
    <svg className="boundedFill" viewBox="0 0 32 32" aria-hidden="true">
      <path d="M19 2c1.306 0 2.418.835 2.83 2h1.67A3.5 3.5 0 0 1 27 7.5v8.354a2.488 2.488 0 0 0-.843-.713l-.249-.13a4.575 4.575 0 0 0-.908-.356V7.5A1.5 1.5 0 0 0 23.5 6h-1.67A3.001 3.001 0 0 1 19 8h-6a3.001 3.001 0 0 1-2.83-2H8.5A1.5 1.5 0 0 0 7 7.5v19A1.5 1.5 0 0 0 8.5 28h7.61a2.506 2.506 0 0 0-.584 2H8.5A3.5 3.5 0 0 1 5 26.5v-19A3.5 3.5 0 0 1 8.5 4h1.67A3.001 3.001 0 0 1 13 2h6Zm-6 2a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-6Zm17.703 20.707a1 1 0 1 0-1.414-1.414l-1.612 1.611l-.649-1.043a1.822 1.822 0 0 0-2.613-.516a1 1 0 0 0 1.006 1.719l.804 1.293l-1.932 1.932a1 1 0 0 0 1.415 1.414L27.31 28.1l.65 1.045a1.816 1.816 0 0 0 2.645.484a1 1 0 0 0-1.043-1.695l-.8-1.286l1.941-1.94Zm-9.865-5.92c.155-2.152 2.463-3.441 4.377-2.445l.249.13a1 1 0 1 1-.924 1.773l-.248-.129a1 1 0 0 0-1.46.815l-.076 1.07H24a1 1 0 0 1 0 2h-1.388l-.448 6.208c-.155 2.153-2.463 3.442-4.377 2.446l-.249-.13a1 1 0 1 1 .924-1.773l.248.129a1 1 0 0 0 1.46-.815L20.606 22H20a1 1 0 1 1 0-2h.75l.088-1.212Z" />
    </svg>
  );
}

function AtomicIcon() {
  return (
    <svg className="atomicReactIcon" viewBox="0 0 500 500" aria-hidden="true">
      <g className="atomicReactLogo" transform="translate(250 250) scale(0.12) translate(2.85 -0.656)">
        <path className="atomicReactRing atomicReactRingThree" pathLength="1" d="M-537.8,310.5C-741.3,-41.1,-879.9,-396.9,-937.7,-692.2C-1007.3,-1048,-958.1,-1320.1,-817.9,-1401.2C-671.8,-1485.8,-395.4,-1387.7,-105.4,-1130.2C113.9,-935.5,339,-642.7,532.2,-308.9C730.3,33.3,878.8,379.9,936.5,671.6C1009.6,1040.8,950.4,1322.9,807.1,1405.8C668,1486.3,413.6,1401.1,142.2,1167.6C-87,970.3,-332.3,665.5,-537.8,310.5L-537.8,310.5Z" />
        <path className="atomicReactRing atomicReactRingTwo" pathLength="1" d="M-538.3,-308C-335.4,-659.9,-96.3,-957.7,130.7,-1155.2C404.2,-1393.2,664.5,-1486.4,804.8,-1405.5C951,-1321.2,1004.1,-1032.7,925.8,-652.8C866.6,-365.6,725.4,-24.3,532.7,309.8C335.2,652.3,109,954,-114.9,1149.8C-398.2,1397.5,-672.2,1487.1,-815.6,1404.4C-954.8,1324.1,-1008,1061.1,-941.3,709.4C-884.9,412,-743.3,47.3,-538.3,-308L-538.3,-308Z" />
        <path className="atomicReactRing atomicReactRingOne" pathLength="1" d="M-2.8,-617.4C403.4,-617.4,780.8,-559.1,1065.3,-461.2C1408.1,-343.2,1618.9,-164.3,1618.9,-2.3C1618.9,166.5,1395.5,356.6,1027.4,478.5C749.1,570.7,382.8,618.9,-2.8,618.9C-398.2,618.9,-772.5,573.7,-1054,477.5C-1410.1,355.8,-1624.6,163.3,-1624.6,-2.3C-1624.6,-163,-1423.3,-340.5,-1085.3,-458.3C-799.7,-557.8,-413,-617.4,-2.8,-617.4L-2.8,-617.4Z" />
        <path className="atomicReactDot" d="M-2.8,-304.8C164.321,-304.8,299.8,-169.321,299.8,-2.2C299.8,164.921,164.321,300.4,-2.8,300.4C-169.921,300.4,-305.4,164.921,-305.4,-2.2C-305.4,-169.321,-169.921,-304.8,-2.8,-304.8Z" />
      </g>
    </svg>
  );
}

function BrainIcon() {
  return (
    <svg className="brainIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path className="brainStem" d="M12 18V5" />
      <path className="brainSide" d="M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
      <path className="brainTop" d="M12 5A3 3 0 1 1 17.598 6.5" />
      <path className="brainTop" d="M12 5A3 3 0 1 0 6.402 6.5" />
      <path d="M17.997 5.125a4 4 0 0 1 2.526 5.77" />
      <path className="brainLow" d="M18 18a4 4 0 0 0 2-7.464" />
      <path d="M19.967 17.483A4 4 0 1 1 12 18a4 4 0 1 1-7.967-.517" />
      <path className="brainLow" d="M6 18a4 4 0 0 1-2-7.464" />
      <path d="M6.003 5.125a4 4 0 0 0-2.526 5.77" />
    </svg>
  );
}

function SearchArchiveIcon() {
  return (
    <svg className="searchArchiveIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path className="searchArchiveBox" d="M4.5 8.5h9.8v8.8a1.7 1.7 0 0 1-1.7 1.7H6.2a1.7 1.7 0 0 1-1.7-1.7V8.5Z" />
      <path className="searchArchiveLid" d="M3.8 6.1h11.2v2.4H3.8V6.1Z" />
      <path className="searchArchiveLine" d="M7.3 11.1h4.1" />
      <circle className="searchArchiveLens" cx="15.5" cy="14.5" r="3.2" />
      <path className="searchArchiveHandle" d="m17.9 16.9 2.5 2.5" />
    </svg>
  );
}

function NewObjectIcon() {
  return (
    <span className="cubeIconBox" aria-hidden="true">
      <span className="cubeSpinner">
        {Array.from({ length: 6 }, (_, index) => (
          <span key={index} />
        ))}
      </span>
    </span>
  );
}

function CalendarIcon() {
  return (
    <span className="calendarIconSwitch" aria-hidden="true">
      <svg className="calendarIcon calendarBusyIcon" viewBox="0 0 24 24">
        <path d="M8 2v4" />
        <path d="M16 2v4" />
        <rect height="18" rx="2" width="18" x="3" y="4" />
        <path d="M3 10h18" />
        {[8, 12, 16, 8, 12, 16].map((cx, index) => (
          <circle className="calendarDot" cx={cx} cy={index < 3 ? 14 : 18} key={`${cx}-${index}`} r="1" />
        ))}
      </svg>
      <svg className="calendarIcon calendarDoneIcon" viewBox="0 0 24 24">
        <path d="M8 2v4" />
        <path d="M16 2v4" />
        <rect height="18" rx="2" width="18" x="3" y="4" />
        <path d="M3 10h18" />
        <path className="calendarCheck" d="m9 16 2 2 4-4" />
      </svg>
    </span>
  );
}

function AgentActionIcon({ command }: { command: AgentActionEventCommand }) {
  if (command === AGENT_LIST_COMMAND) return <List />;
  if (command === AGENT_SEARCH_COMMAND) return <Search />;
  if (command === AGENT_CREATE_DIRECTORY_COMMAND) return <FolderPlus />;
  if (command === AGENT_RENAME_PATH_COMMAND) return <Pencil />;
  if (command === AGENT_MOVE_PATH_COMMAND) return <MoveRight />;
  if (command === AGENT_COPY_PATH_COMMAND) return <Copy />;
  if (command === AGENT_DELETE_EMPTY_DIRECTORY_COMMAND || command === AGENT_DELETE_TREE_COMMAND) return <Trash2 />;
  if (command === AGENT_READONLY_SHELL_COMMAND || command === AGENT_SHELL_COMMAND) return <Terminal />;
  return <GenericIcon />;
}

function EventIcon({ command }: { command: EventCommand }) {
  if (command.startsWith("/agent_")) return <AgentActionIcon command={command as AgentActionEventCommand} />;
  if (command === BRAIN_NEWCOMPUTE_COMMAND) return <NewComputeIcon />;
  if (command === "/compute_atomic_science_") return <AtomicIcon />;
  if (command === BRAIN_BRAIN_COMMAND) return <BrainIcon />;
  if (command === BRAIN_SEARCHARCHIVE_COMMAND) return <SearchArchiveIcon />;
  if (command === BRAIN_NEWOBJECT_COMMAND) return <NewObjectIcon />;
  if (command === BRAIN_GOOGLE_AGENDA_COMMAND) return <CalendarIcon />;
  return <GenericIcon />;
}

function EventLine({ command, text, animated = false }: { command: EventCommand; text: string; animated?: boolean }) {
  return (
    <div className={animated ? "eventLine eventLineAnimated" : "eventLine"}>
      <code className="commandLabel">{command}</code>
      <span className="eventGlyph">
        <EventIcon command={command} />
      </span>
      <span className="eventText">{text}</span>
    </div>
  );
}

function NewComputeTree() {
  return (
    <div className="newComputeTree" aria-label="NewCompute template tree">
      <svg className="treeSvg" viewBox="0 0 33 181" preserveAspectRatio="none" aria-hidden="true">
        <path className="treeTrunk" d="M1 0 V181" />
        {templates.map((_, index) => {
          const y = 12 + index * 24;
          return <path className="treeBranch" d={`M1 ${y} H32`} key={index} style={{ animationDelay: `${1020 + index * 92}ms` }} />;
        })}
      </svg>
      <div className="templateRows">
        {templates.map(([command, summary], index) => (
          <div className="templateRow" key={command} style={{ animationDelay: `${1120 + index * 92}ms` }}>
            <code>{command}</code>
            <span>{summary}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function NewComputeBlock() {
  return (
    <section className="newComputeBlock">
      <p className="intro introLineOne">
        InGen turns <Pill>proof hash</Pill> into a measurable asset: the product moves forward.
      </p>
      <p className="intro introLineTwo">Proof stays local, and every session increases our advantage.</p>
      <EventLine command="/newcompute_" text="opens the Monster template selector" animated />
      <NewComputeTree />
    </section>
  );
}

function SuccessStepView({ step }: { step: SuccessStep }) {
  return (
    <section className="successStep">
      <p>
        {step.before} <Pill>{step.pill}</Pill> {step.after}
      </p>
      <p>{step.line2}</p>
      <EventLine command={step.command} text={step.eventText} />
    </section>
  );
}

function EventTextLab() {
  const [run, setRun] = useState(0);

  return (
    <main className="eventLab">
      <button className="replayButton" type="button" onClick={() => setRun((value) => value + 1)}>
        Replay
      </button>
      <div className="labMark" aria-hidden="true">IG</div>
      <article className="transcript" key={run}>
        <NewComputeBlock />
        {steps.map((step) => (
          <SuccessStepView key={step.command} step={step} />
        ))}
      </article>
    </main>
  );
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("Missing #root mount node");
}

createRoot(root).render(
  <StrictMode>
    <EventTextLab />
  </StrictMode>
);
