import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  brainDurableMemoryManifestFromEntries,
  type BrainCustomCodeActEntry,
  type BrainLearningMemoryEntry,
  type BrainSpecializedBrainEntry
} from "../src/renderer/brain-user-memory-store";
import {
  brainSpecializedBrainModificationsFromText,
  brainSpecializedCodeActsFromNewBrainText,
  brainLearningCodeActImplementationTask,
  brainLearningMemoryCategoryForPromotion,
  brainLearningPromotionPrompt,
  brainLearningResearchPrompt,
  parseBrainLearningInterruptLine,
  specializedBrainCodeActCommand,
  stripBrainLearningInterruptMarkup
} from "../src/renderer/assistant-learning-interrupts";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const rendererSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const learningSource = readFileSync(join(root, "src", "renderer", "assistant-learning-interrupts.ts"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const memoryStoreSource = readFileSync(join(root, "src", "renderer", "brain-user-memory-store.ts"), "utf8");
const ipcSource = readFileSync(join(root, "src", "shared", "generated", "forge-ipc.generated.ts"), "utf8");
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
      promote: ["lesson", "skill"],
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
    expect(codeAct?.promote).toEqual(["task"]);
    expect(brainLearningMemoryCategoryForPromotion(antiPattern!, "lesson")).toBe("lesson");
    expect(brainLearningMemoryCategoryForPromotion(antiPattern!, "research")).toBeNull();
    expect(brainLearningMemoryCategoryForPromotion(codeAct!, "codeact")).toBeNull();
    expect(brainLearningCodeActImplementationTask(codeAct!)).toContain("Implement executable CodeAct before use");
    expect(brainLearningResearchPrompt(antiPattern!)).toContain("RESEARCH_PARALLEL_QUERY v1");
  });

  it("derives specialized Brain activator CodeActs from /newbrain_ without root catalog writes", () => {
    const drafts = brainSpecializedCodeActsFromNewBrainText([
      '/newbrain_ brain_name="musician" title="Musician Brain"',
      'purpose="Store durable composition, arrangement and practice lessons."',
      'activation_triggers="songwriting, harmony, arrangement"',
      'initial_skills="turn motif into arrangement"',
      'initial_codeacts=\'/websearch_ query="music arrangement references" output="compact_answer_url_citation_manifest"\'',
      'token_budget="900"'
    ].join("\n"));

    expect(specializedBrainCodeActCommand("musician")).toBe("/musicianbrain_");
    expect(drafts).toHaveLength(1);
    expect(drafts[0]?.command).toBe("/musicianbrain_");
    expect(drafts[0]?.brainName).toBe("musician");
    expect(drafts[0]?.title).toBe("Musician Brain");
    expect(drafts[0]?.activationTriggers).toBe("songwriting, harmony, arrangement");
    expect(drafts[0]?.initialSkills).toBe("turn motif into arrangement");
    expect(drafts[0]?.initialCodeActs).toContain("/websearch_");
    expect(drafts[0]?.tokenBudget).toBe("900");
    expect(drafts[0]?.description).toContain("Host-generated from explicit /newbrain_ fields");
    expect(drafts[0]?.description).toContain("root catalog stays read-only");
    expect(drafts[0]?.description).not.toContain("General Brain");
    expect(drafts[0]?.template).not.toContain("General Brain");
    expect(drafts[0]?.template).toContain('generated_by="host_from_newbrain"');
    expect(drafts[0]?.template).toContain('initial_skills="turn motif into arrangement"');
  });

  it("parses named specialized Brain updates from /modify templates", () => {
    const modifications = brainSpecializedBrainModificationsFromText([
      '/modify"musician"brain_ entry_kind="lesson"',
      'observation="We over-arrange before preserving the motif."',
      'replacement_rule="Preserve the motif in one bare pass before arranging."',
      'trigger="song arrangement critique"'
    ].join("\n"));

    expect(modifications).toHaveLength(1);
    expect(modifications[0]?.brainName).toBe("musician");
    expect(modifications[0]?.kind).toBe("lesson");
    expect(modifications[0]?.content).toContain("Observed error");
    expect(modifications[0]?.content).toContain("Replacement rule");
  });

  it("keeps workflows as skills and only executable commands as CodeActs", () => {
    const modifications = brainSpecializedBrainModificationsFromText([
      '/modify"marketing"brain_ entry_kind="codeact"',
      'content="Campaign critique workflow: identify offer, audience, channel and metric before writing ideas."',
      '/modify"marketing"brain_ entry_kind="codeact"',
      "content='/campaign_brief_ offer=\"...\" audience=\"...\"'",
      '/modify"marketing"brain_ entry_kind="codeact"',
      "content='/websearch_ query=\"current campaign benchmarks\" output=\"compact_answer_url_citation_manifest\"'"
    ].join("\n"));

    expect(modifications).toHaveLength(3);
    expect(modifications[0]?.kind).toBe("skill");
    expect(modifications[1]?.kind).toBe("task");
    expect(modifications[2]?.kind).toBe("codeact");
  });

  it("formats durable Brain learning memory for boot manifest injection", () => {
    const now = "2026-06-15T00:00:00.000Z";
    const learningEntries: BrainLearningMemoryEntry[] = [
      {
        schema: "ingen.brain.memory.learning_registry.v1",
        id: "learning-1",
        category: "lesson",
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
        command: "/musicianbrain_",
        description: "Activate Musician Brain as a specialized Brain scope.",
        template: "/musicianbrain_\nbrain_name=\"musician\"",
        source: "host_generated_newbrain",
        trust: "agent_candidate",
        evidence: "test",
        createdAt: now,
        updatedAt: now
      }
    ];
    const specializedBrains: BrainSpecializedBrainEntry[] = [
      {
        schema: "ingen.brain.memory.specialized_brain.v1",
        id: "specialized-brain-1",
        brainName: "musician",
        title: "Musician Brain",
        purpose: "Store durable composition, arrangement and practice lessons.",
        activationTriggers: ["songwriting", "harmony", "arrangement"],
        activationCommand: "/musicianbrain_",
        tokenBudget: 900,
        lessons: ["Preserve the motif before arranging."],
        skills: ["Turn motif into arrangement."],
        tasks: ["Draft a harmony checklist."],
        codeActs: ["/websearch_ query=\"music arrangement references\" output=\"compact_answer_url_citation_manifest\""],
        source: "host_generated_newbrain",
        trust: "agent_candidate",
        evidence: "test",
        activeAt: now,
        createdAt: now,
        updatedAt: now
      }
    ];
    const manifest = brainDurableMemoryManifestFromEntries(learningEntries, codeActs, specializedBrains);
    expect(manifest).toContain("BRAIN_DURABLE_MEMORY_MANIFEST v1");
    expect(manifest).toContain("injection_policy=session_boot_and_after_context_compaction");
    expect(manifest).toContain("specialized_brain_policy=The root catalog is read-only");
    expect(manifest).toContain("newbrain_template_example=/newbrain_");
    expect(manifest).toContain('brain_name="marketing"');
    expect(manifest).toContain('token_budget="1200"');
    expect(manifest).toContain('modifybrain_template_example=/modify"marketing"brain_');
    expect(manifest).toContain('entry_kind="lesson"');
    expect(manifest).toContain("entry_kind_policy=skill is an LLM-only reusable workflow");
    expect(manifest).toContain('modifybrain_skill_example=/modify"marketing"brain_');
    expect(manifest).toContain('modifybrain_codeact_example=/modify"marketing"brain_');
    expect(manifest).toContain('output="append_to_specialized_brain"');
    expect(manifest).toContain("specialized_brain[1]");
    expect(manifest).toContain("active_specialized_brain");
    expect(manifest).toContain('name="musician"');
    expect(manifest).toContain('command="/musicianbrain_"');
    expect(manifest).toContain("lessons[1]");
    expect(manifest).toContain("skills[1]");
    expect(manifest).toContain("codeact[1]");
    expect(manifest).toContain("codeact[1]=executable");
    expect(manifest).toContain("codeact_draft[1]");
    expect(manifest).toContain("source=host_generated_newbrain");
    expect(manifest).toContain("research_policy=Research branches are live work only");
  });

  it("injects the Brain boot rule and renderer contract", () => {
    expect(mainSource).toContain("learning_interrupt_markup=[[learn");
    expect(mainSource).toContain("learning_interrupt_guard=Learning interrupts are proposals only");
    expect(mainSource).toContain("General Brain is immutable and read-only");
    expect(mainSource).toContain("brainDurableMemoryContextManifest()");
    expect(mainSource).toContain("normalizeBrainDurableMemoryManifest(command.value)");
    expect(ipcSource).toContain("This command never targets the root read-only catalog");
    expect(ipcSource).not.toContain("pollute the General Brain");
    expect(ipcSource).not.toContain("mutate the immutable General Brain");
    expect(learningSource).toContain("root catalog stays read-only");
    expect(learningSource).not.toContain("General Brain stays read-only");
    expect(rendererSource).toContain("brainSpecializedCodeActsFromNewBrainText(text)");
    expect(rendererSource).toContain("brainSpecializedBrainModificationsFromText(text)");
    expect(rendererSource).toContain("activateBrainSpecializedBrain(command");
    expect(rendererSource).toContain("source: \"host_generated_newbrain\"");
    expect(rendererSource).toContain("parseBrainLearningInterruptLine(rawLine)");
    expect(rendererSource).toContain("AssistantLearningInterruptCard");
    expect(rendererSource).toContain("appendBrainLearningMemoryEntry");
    expect(rendererSource).toContain("dispatchBrainResearchParallelRequest");
    expect(storeSource).toContain("brainDurableMemoryManifest()");
    expect(storeSource).toContain("BRAIN_SPECIALIZED_BRAINS_UPDATED_EVENT");
    expect(memoryStoreSource).toContain("ingen.brain.memory.specialized_brain.v1");
    expect(memoryStoreSource).toContain("active_specialized_brain");
    expect(memoryStoreSource).toContain("BRAIN_CODEACT_COMMANDS");
    expect(memoryStoreSource).toContain("entry_kind_policy=skill is an LLM-only reusable workflow");
    expect(storeSource).toContain("BRAIN_LEARNING_MEMORY_UPDATED_EVENT");
    expect(stylesSource).toContain(".assistantText__learningInterrupt");
  });
});
