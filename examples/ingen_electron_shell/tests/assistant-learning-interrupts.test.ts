import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  brainLearningPromotionPrompt,
  parseBrainLearningInterruptLine,
  stripBrainLearningInterruptMarkup
} from "../src/renderer/assistant-learning-interrupts";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const rendererSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");

describe("assistant learning interrupts", () => {
  it("parses one-line loop-stream learning candidates", () => {
    const interrupt = parseBrainLearningInterruptLine(
      '[[learn type=anti_pattern scope=marketing confidence=0.81 promote=rule,skill]]We keep drifting into generic claims; force audience, pain, mechanism and measurable result.[[/learn]]'
    );

    expect(interrupt).toEqual({
      type: "anti_pattern",
      scope: "marketing",
      confidence: 0.81,
      promote: ["rule", "skill"],
      text: "We keep drifting into generic claims; force audience, pain, mechanism and measurable result."
    });
  });

  it("keeps candidates visible to assistive text without exposing markup", () => {
    expect(stripBrainLearningInterruptMarkup("[[learn type=working_pattern]]Use concrete proof before claims.[[/learn]]"))
      .toBe("Learning interrupt: Use concrete proof before claims.");
  });

  it("turns a promotion click into a structured candidate prompt", () => {
    const interrupt = parseBrainLearningInterruptLine(
      "[[learn type=skill_candidate scope=campaign confidence=0.7]]Reusable campaign critique loop detected.[[/learn]]"
    );
    expect(interrupt).toBeDefined();
    expect(brainLearningPromotionPrompt(interrupt!, "skill")).toContain("BRAIN_LEARNING_PROMOTION v1");
    expect(brainLearningPromotionPrompt(interrupt!, "skill")).toContain("action=skill");
    expect(brainLearningPromotionPrompt(interrupt!, "skill")).toContain("Treat it as a candidate");
  });

  it("injects the Brain boot rule and renderer contract", () => {
    expect(mainSource).toContain("learning_interrupt_markup=[[learn");
    expect(mainSource).toContain("learning_interrupt_guard=Learning interrupts are proposals only");
    expect(rendererSource).toContain("parseBrainLearningInterruptLine(rawLine)");
    expect(rendererSource).toContain("AssistantLearningInterruptCard");
    expect(stylesSource).toContain(".assistantText__learningInterrupt");
  });
});
