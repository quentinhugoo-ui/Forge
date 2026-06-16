import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const rendererSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");

describe("composer height", () => {
  it("keeps the one-line composer height stable when the first character is typed", () => {
    expect(rendererSource).toContain("COMPOSER_SINGLE_LINE_HEIGHT_TOLERANCE_PX");
    expect(rendererSource).toContain("const COMPOSER_MAX_INPUT_HEIGHT = 132");
    expect(rendererSource).toContain("function composerMeasuredInputHeight");
    expect(rendererSource).toContain("measuredHeight <= compactHeight + COMPOSER_SINGLE_LINE_HEIGHT_TOLERANCE_PX");
    expect(rendererSource).toContain("return compactHeight");
    expect(rendererSource).toContain("composerMeasuredInputHeight(node, compactHeight)");
    expect(rendererSource).not.toContain("const inputHeight = Math.min(node.scrollHeight, COMPOSER_MAX_INPUT_HEIGHT)");
    expect(stylesSource).toContain("max-height: 132px;");
    expect(stylesSource).toContain("overflow-y: auto;");
    expect(stylesSource).toContain("scrollbar-color: color-mix(in oklab, var(--forge-line), #000 22%) transparent");
    expect(stylesSource).toContain(".composer__input::-webkit-scrollbar");
    expect(stylesSource).toContain("width: 2px;");
    expect(stylesSource).toContain("padding: 14px 12px 31px 8px;");
  });
});
