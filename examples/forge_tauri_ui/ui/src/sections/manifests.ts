import type { ForgeSectionDefinition } from "../shell/types.js";

export const forgeSectionManifests = Object.freeze([
  { id: "shell", label: "Forge shell", kind: "surface", bootSafe: true, owns: ["hardware"], permissions: ["native-window", "hardware"] },
  { id: "alpha", label: "Alpha canvas", kind: "shell-section", owns: ["canvas", "chatbar", "rightPanel", "jobs"], permissions: ["canvas", "chatbar", "right-panel", "jobs"] },
  { id: "forge", label: "Forge home", kind: "shell-section", owns: ["canvas", "chatbar"], permissions: ["canvas", "chatbar"] },
  { id: "webexplorer", label: "WebExplorer", kind: "surface", owns: ["canvas", "rightPanel"], permissions: ["canvas", "right-panel", "network"] },
  { id: "real-estate", label: "Agence immo", kind: "surface", owns: ["canvas", "chatbar", "rightPanel", "jobs"], permissions: ["canvas", "chatbar", "right-panel", "jobs", "network"], commands: ["real_estate_harvester_snapshot", "real_estate_tool_command_context"] },
  { id: "real-estate-main", label: "Accueil agence immo", kind: "surface", parent: "real-estate", owns: ["canvas", "chatbar"], permissions: ["canvas", "chatbar"] },
  { id: "trading", label: "Trading workspace", kind: "surface", owns: ["canvas", "chatbar", "jobs"], permissions: ["canvas", "chatbar", "jobs", "network"], commands: ["trading_chart_series", "trading_strategy_backtest"] },
  { id: "banger", label: "Banger viewport", kind: "surface", owns: ["canvas", "jobs"], permissions: ["canvas", "jobs", "hardware"], commands: ["banger_run_rust_console"] },
] satisfies readonly ForgeSectionDefinition[]);
