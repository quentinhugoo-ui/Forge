export type ForgeIntentFacadeTool =
  | "forge.search"
  | "forge.execute"
  | "forge.read_projection"
  | "forge.cancel";

export type ForgeIntentOptInTool =
  | "forge_intent_search"
  | "forge_intent_execute";

export const forgeIntentSurfaceContract = Object.freeze({
  status: "step_16_ts_owned_contract",
  owner: "ui/src/shell/intent-surface.ts",
  runtimeBundle: "ui/dist/forge-shell-runtime.js",
  compactSurfaceEnv: "FORGE_MCP_COMPACT_SURFACE=1",
  optInEnv: "FORGE_INTENT_MCP_SURFACE=1",
  defaultVisibleFacade: [
    "forge.search",
    "forge.execute",
    "forge.read_projection",
    "forge.cancel",
  ] satisfies readonly ForgeIntentFacadeTool[],
  optInMcpTools: [
    "forge_intent_search",
    "forge_intent_execute",
  ] satisfies readonly ForgeIntentOptInTool[],
  uiRule: "Intent, trace and distillation debug surfaces are TS-owned shell projections; they do not create direct JS listeners, raw Tauri IPC or browser-side MCP runners.",
  allowedBridge: "window.ForgeShellRuntime.tauri through ui/src/shell/tauri-bridge.ts",
  forbidden: [
    "hand-written ui/*.js intent panels",
    "global Tauri direct invoke/listen",
    "section-local MCP clients",
    "duplicate intent runners outside ForgeSlash/forge_mcp.rs",
  ],
});
