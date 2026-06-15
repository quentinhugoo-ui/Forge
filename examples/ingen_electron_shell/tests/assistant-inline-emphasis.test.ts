import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const rendererSource = readFileSync("src/renderer/PanelsChatBottomSlice.tsx", "utf8");
const stylesSource = readFileSync("src/renderer/styles.css", "utf8");

describe("assistant inline emphasis", () => {
  it("renders single-star assistant emphasis as italic text", () => {
    expect(rendererSource).toContain("|\\*[^*\\n]+?\\*|");
    expect(rendererSource).toContain("<em key={`${keyPrefix}-em-${match.index}`}>{token.slice(1, -1)}</em>");
    expect(stylesSource).toContain(".assistantText em");
    expect(stylesSource).toContain("font-style: italic");
  });
});
