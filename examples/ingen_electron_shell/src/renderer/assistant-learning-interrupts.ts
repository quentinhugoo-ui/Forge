export type BrainLearningInterruptType =
  | "anti_pattern"
  | "working_pattern"
  | "user_preference"
  | "domain_rule"
  | "skill_candidate"
  | "codeact_candidate"
  | "research_candidate";

export type BrainLearningPromotionAction = "remember" | "rule" | "skill" | "task" | "codeact" | "research";

export interface BrainLearningInterrupt {
  type: BrainLearningInterruptType;
  scope: string;
  confidence: number | null;
  promote: BrainLearningPromotionAction[];
  text: string;
}

const LEARNING_INTERRUPT_PATTERN = /^\s*\[\[learn\b([^\]]*)\]\]([\s\S]{1,1400}?)\[\[\/learn\]\]\s*$/;
const KNOWN_TYPES = new Set<BrainLearningInterruptType>([
  "anti_pattern",
  "working_pattern",
  "user_preference",
  "domain_rule",
  "skill_candidate",
  "codeact_candidate",
  "research_candidate"
]);
const KNOWN_PROMOTIONS = new Set<BrainLearningPromotionAction>(["remember", "rule", "skill", "task", "codeact", "research"]);

const DEFAULT_PROMOTIONS: Record<BrainLearningInterruptType, BrainLearningPromotionAction[]> = {
  anti_pattern: ["rule", "task"],
  working_pattern: ["skill", "rule"],
  user_preference: ["remember", "rule"],
  domain_rule: ["rule", "skill"],
  skill_candidate: ["skill", "task"],
  codeact_candidate: ["codeact", "task"],
  research_candidate: ["research", "remember"]
};

function cleanLearningValue(value: string): string {
  return value.trim().replace(/^["'`]+|["'`,;]+$/g, "");
}

function safeLearningToken(value: string, fallback: string): string {
  const cleaned = cleanLearningValue(value)
    .replace(/[^\w./:-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 80);
  return cleaned || fallback;
}

function parseLearningAttributes(source: string): Map<string, string> {
  const attributes = new Map<string, string>();
  const pattern = /\b([a-zA-Z][\w-]*)\s*=\s*("([^"]*)"|'([^']*)'|([^\s]+))/g;
  for (const match of source.matchAll(pattern)) {
    attributes.set(match[1].toLowerCase(), match[3] ?? match[4] ?? match[5] ?? "");
  }
  return attributes;
}

function parseLearningType(value: string | undefined): BrainLearningInterruptType {
  const candidate = safeLearningToken(value ?? "", "working_pattern") as BrainLearningInterruptType;
  return KNOWN_TYPES.has(candidate) ? candidate : "working_pattern";
}

function parseConfidence(value: string | undefined): number | null {
  if (!value) {
    return null;
  }
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return null;
  }
  return Math.max(0, Math.min(1, parsed));
}

function parsePromotionList(value: string | undefined, type: BrainLearningInterruptType): BrainLearningPromotionAction[] {
  const requested = (value ?? "")
    .split(/[|,]/)
    .map((item) => safeLearningToken(item, ""))
    .filter((item): item is BrainLearningPromotionAction => KNOWN_PROMOTIONS.has(item as BrainLearningPromotionAction));
  const unique = [...new Set(requested)];
  return unique.length > 0 ? unique : DEFAULT_PROMOTIONS[type];
}

export function parseBrainLearningInterruptLine(line: string): BrainLearningInterrupt | undefined {
  const match = LEARNING_INTERRUPT_PATTERN.exec(line);
  if (!match) {
    return undefined;
  }
  const text = match[2].replace(/\s+/g, " ").trim();
  if (!text) {
    return undefined;
  }
  const attributes = parseLearningAttributes(match[1] ?? "");
  const type = parseLearningType(attributes.get("type"));
  return {
    type,
    scope: safeLearningToken(attributes.get("scope") ?? "", "session"),
    confidence: parseConfidence(attributes.get("confidence")),
    promote: parsePromotionList(attributes.get("promote"), type),
    text
  };
}

export function stripBrainLearningInterruptMarkup(text: string): string {
  return text.replace(new RegExp(LEARNING_INTERRUPT_PATTERN.source, "gm"), (_full, _attributes, body: string) => `Learning interrupt: ${body.trim()}`);
}

export function brainLearningTypeLabel(type: BrainLearningInterruptType): string {
  return type
    .replace(/_/g, " ")
    .replace(/^\w/, (letter) => letter.toUpperCase());
}

export function brainLearningPromotionLabel(action: BrainLearningPromotionAction): string {
  if (action === "remember") return "Remember";
  if (action === "rule") return "Rule";
  if (action === "skill") return "Skill";
  if (action === "task") return "Task";
  if (action === "codeact") return "CodeAct";
  return "Research";
}

export function brainLearningPromotionPrompt(interrupt: BrainLearningInterrupt, action: BrainLearningPromotionAction): string {
  return [
    "BRAIN_LEARNING_PROMOTION v1",
    `action=${action}`,
    `type=${interrupt.type}`,
    `scope=${interrupt.scope}`,
    `confidence=${interrupt.confidence === null ? "unknown" : interrupt.confidence.toFixed(2)}`,
    `candidate=${JSON.stringify(interrupt.text)}`,
    "Instruction: propose the smallest verified promotion for this learning interrupt. Treat it as a candidate, not as already persisted."
  ].join("\n");
}
