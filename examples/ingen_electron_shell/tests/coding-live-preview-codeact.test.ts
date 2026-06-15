import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = join(process.cwd(), "..", "..");
const brainSource = readFileSync(join(repoRoot, "src", "brain.rs"), "utf8");
const generatedSource = readFileSync(join(process.cwd(), "src", "shared", "generated", "forge-ipc.generated.ts"), "utf8");
const ipcSource = readFileSync(join(process.cwd(), "src", "shared", "ipc-contract.ts"), "utf8");
const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const appSource = readFileSync(join(process.cwd(), "src", "renderer", "App.tsx"), "utf8");
const canvasSource = readFileSync(join(process.cwd(), "src", "renderer", "CanvasSurfacesSlice.tsx"), "utf8");
const panelsSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(process.cwd(), "src", "renderer", "styles.css"), "utf8");

describe("Coding Brain live preview CodeAct", () => {
  it("registers a Coding-only visual preview command after real file writes", () => {
    expect(brainSource).toContain('pub const BRAIN_CODING_LIVE_PREVIEW_COMMAND: &str = "/coding_live_preview_";');
    expect(brainSource).toContain("only after creating or modifying a real local HTML/CSS/JS/React/Vite visual file");
    expect(brainSource).toContain("write one short natural paragraph");
    expect(brainSource).toContain("the live visual is opening in the conversation");
    expect(brainSource).toContain("create or modify a real local file first through AGENT_ACTION_JSON");
    expect(brainSource).toContain("Windows/local action tools remain available in Coding Brain");
    expect(brainSource).toContain("copy-paste instructions");
    expect(brainSource).toContain("do not paste implementation code in the same Brain-switch message");
    expect(generatedSource).toContain("BRAIN_CODING_LIVE_PREVIEW_COMMAND");
    expect(generatedSource).toContain("BRAIN_CODING_LIVE_PREVIEW_COMMAND_DESCRIPTION");
    expect(ipcSource).toContain("BRAIN_CODING_LIVE_PREVIEW_COMMAND");
    expect(mainSource).toContain("visual_artifact_rule=");
    expect(mainSource).toContain("AGENT_ACTION_JSON first");
    expect(mainSource).toContain("Coding Brain conserve les outils Windows/local action");
    expect(mainSource).toContain("n'ecris pas de bloc de code a copier-coller");
    expect(mainSource).toContain("html|css|javascript|typescript|react|vite|page|site|frontend|front-end|composant");
  });

  it("opens a sandboxed canvas preview for verified visual file paths", () => {
    expect(appSource).toContain("function latestCodingLivePreviewTarget");
    expect(appSource).toContain("lastIndexOf(BRAIN_CODING_LIVE_PREVIEW_COMMAND)");
    expect(appSource).toContain("openCodingLivePreview(codingPreviewTarget)");
    expect(appSource).toContain("setCodingLivePreview((current)");
    expect(appSource).toContain("shell--coding-live-preview-open");
    expect(canvasSource).toContain("function CodingLivePreviewFrame");
    expect(canvasSource).toContain("fileUrlFromPreviewPath");
    expect(canvasSource).toContain("window.setInterval(() => setReloadTick");
    expect(canvasSource).toContain('sandbox="allow-scripts allow-forms allow-modals"');
    expect(canvasSource).toContain('referrerPolicy="no-referrer"');
    expect(canvasSource).toContain("codingLivePreviewOpen");
    expect(stylesSource).toContain(".canvasSurfaces--codingLivePreviewOpen");
    expect(stylesSource).toContain(".codingLivePreview__frame");
  });

  it("renders the preview CodeAct as a readable loop-stream event", () => {
    expect(panelsSource).toContain('BRAIN_CODING_LIVE_PREVIEW_COMMAND, "Live preview opened"');
    expect(panelsSource).toContain('return "is opening the live preview";');
  });

  it("forces visual coding snippets back into the Windows tool loop", () => {
    expect(mainSource).toContain("function shouldForceInitialCodingVisualAction");
    expect(mainSource).toContain("assistantLooksLikeSnippetInsteadOfLocalAction");
    expect(mainSource).toContain("CODING_VISUAL_ACTION_FORCED_CONTINUATION v1");
    expect(mainSource).toContain("document_write_text avec path=");
    expect(mainSource).toContain("Interdit dans ce tour: bloc de code Markdown");
    expect(mainSource).toContain("agentActionLoopInitialCodingVisualForcedContinuation");
  });
});
