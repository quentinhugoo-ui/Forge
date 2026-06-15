import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  brainDurableMemoryManifestFromEntries,
  type BrainCustomCodeActEntry,
  type BrainLearningMemoryEntry
} from "../src/renderer/brain-user-memory-store";
import {
  brainLearningCodeActCommand,
  brainLearningMemoryCategoryForPromotion,
  brainLearningPromotionPrompt,
  brainLearningResearchPrompt,
  parseBrainLearningInterruptLine,
  stripBrainLearningInterruptMarkup
} from "../src/renderer/assistant-learning-interrupts";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const rendererSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
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

  it("routes durable promotions away from live research", () => {
    const antiPattern = parseBrainLearningInterruptLine(
      "[[learn type=anti_pattern scope=campaign confidence=0.91 promote=rule,research]]We keep skipping audience proof.[[/learn]]"
    );
    const codeAct = parseBrainLearningInterruptLine(
      "[[learn type=codeact_candidate scope=campaign confidence=0.76 promote=codeact]]Draft a reusable campaign critique action.[[/learn]]"
    );
    expect(antiPattern).toBeDefined();
    expect(codeAct).toBeDefined();
    expect(brainLearningMemoryCategoryForPromotion(antiPattern!, "rule")).toBe("anti_pattern");
    expect(brainLearningMemoryCategoryForPromotion(antiPattern!, "research")).toBeNull();
    expect(brainLearningCodeActCommand(codeAct!)).toMatch(/^\/agent_campaign_/);
    expect(brainLearningResearchPrompt(antiPattern!)).toContain("RESEARCH_PARALLEL_QUERY v1");
  });

  it("formats durable Brain learning memory for boot manifest injection", () => {
    const now = "2026-06-15T00:00:00.000Z";
    const learningEntries: BrainLearningMemoryEntry[] = [
      {
        schema: "ingen.brain.memory.learning_registry.v1",
        id: "learning-1",
        category: "anti_pattern",
        text: "Do not approve a campaign before audience, pain, proof and measurable result are explicit.",
        source: "manual",
        trust: "user_confirmed",
        evidence: "test",
        createdAt: now,
        updatedAt: now
      },
      {
        schema: "ingen.brain.memory.learning_registry.v1",
        id: "learning-2",
        category: "skill",
        text: "Reusable campaign critique loop.",
        source: "agent_learning_interrupt",
        trust: "agent_candidate",
        evidence: "test",
        createdAt: now,
        updatedAt: now
      }
    ];
    const codeActs: BrainCustomCodeActEntry[] = [
      {
        schema: "ingen.brain.codeact.custom_registry.v1",
        id: "codeact-1",
        command: "/agent_campaign_critique_",
        description: "Critique a campaign draft with the durable marketing checklist.",
        template: "/agent_campaign_critique_\ninput=\"...\"",
        source: "manual",
        trust: "user_confirmed",
        evidence: "test",
        createdAt: now,
        updatedAt: now
      }
    ];
    const manifest = brainDurableMemoryManifestFromEntries(learningEntries, codeActs);
    expect(manifest).toContain("BRAIN_DURABLE_MEMORY_MANIFEST v1");
    expect(manifest).toContain("injection_policy=session_boot_and_after_context_compaction");
    expect(manifest).toContain("errors_to_avoid[1]");
    expect(manifest).toContain("skills[1]");
    expect(manifest).toContain("codeact_draft[1]");
    expect(manifest).toContain("research_policy=Research branches are live work only");
  });

  it("injects the Brain boot rule and renderer contract", () => {
    expect(mainSource).toContain("learning_interrupt_markup=[[learn");
    expect(mainSource).toContain("learning_interrupt_guard=Learning interrupts are proposals only");
    expect(mainSource).toContain("brainDurableMemoryContextManifest()");
    expect(mainSource).toContain("normalizeBrainDurableMemoryManifest(command.value)");
    expect(rendererSource).toContain("parseBrainLearningInterruptLine(rawLine)");
    expect(rendererSource).toContain("AssistantLearningInterruptCard");
    expect(rendererSource).toContain("appendBrainLearningMemoryEntry");
    expect(rendererSource).toContain("dispatchBrainResearchParallelRequest");
    expect(storeSource).toContain("brainDurableMemoryManifest()");
    expect(storeSource).toContain("BRAIN_LEARNING_MEMORY_UPDATED_EVENT");
    expect(stylesSource).toContain(".assistantText__learningInterrupt");
  });
});
