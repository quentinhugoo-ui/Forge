import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const rendererSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");

describe("composer LLM selector", () => {
  it("keeps provider logos below the chat bar next to the model label", () => {
    expect(rendererSource).toContain("llmSelectorOpen");
    expect(rendererSource).toContain("bottomControls__llmSelector");
    expect(rendererSource).toContain("bottomControls__llmRail");
    expect(rendererSource).toContain("bottomControls__llmActive");
    expect(rendererSource).toContain("bottomControls__llmProvider");
    expect(rendererSource).toContain("setLlmSelectorOpen((open) => !open)");
    expect(rendererSource).toContain('aria-label={`Change LLM provider, current provider ${activeProvider.label}`}');
    expect(rendererSource).not.toContain("composer" + "__providers");
    expect(rendererSource).not.toContain("provider" + "Dot");

    expect(stylesSource).toContain(".bottomControls__llmRail");
    expect(stylesSource).toContain("right: calc(100% + 4px)");
    expect(stylesSource).toContain("transform: translateX(8px) scale(0.88)");
    expect(stylesSource).toContain(".bottomControls__llmSelector--open .bottomControls__llmProvider");
    expect(stylesSource).toContain("transition-delay: calc(var(--llm-provider-index, 0) * 28ms)");
    expect(stylesSource).not.toContain(".composer" + "__providers");
    expect(stylesSource).not.toContain(".provider" + "Dot");
  });
});
