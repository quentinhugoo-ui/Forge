import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");

describe("Self-Directed palette", () => {
  it("uses the same royal aurora spectrum across the trigger, menu and drafting states", () => {
    expect(stylesSource).toContain(
      "--self-directed-spectrum: linear-gradient(110deg, #5eead4 0%, #38bdf8 22%, #4338ca 46%, #6b21a8 66%, #9f1239 84%, #e9c46a 100%)"
    );
    expect(stylesSource).toContain("--self-directed-purple: #6b21a8;");
    expect(stylesSource).toContain("--self-directed-wine: #9f1239;");
    expect(stylesSource).toContain(".permissionModeTrigger--selfDirected > span");
    expect(stylesSource).toContain(".bottomControls .permissionModeOption.permissionModeOption--selfDirected::after");
    expect(stylesSource).toContain(".permissionModeOption--selfDirectedExpanded");
    expect(stylesSource).toContain(".panelsChatBottom--selfDirected .composerQuestionnaire");
    expect(stylesSource).toContain(".composer--selfDirectedDrafting::before");
    expect(stylesSource).toContain("background: var(--self-directed-spectrum)");
    expect(stylesSource).not.toContain(
      "linear-gradient(90deg, #34d399, #22d3ee, #38bdf8, #818cf8, #c084fc, #facc15, #fb923c, #ef4444, #f472b6, #34d399)"
    );
  });
});
