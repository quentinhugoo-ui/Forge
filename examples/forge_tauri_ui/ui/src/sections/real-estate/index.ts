import {
  realEstateOnboardingPromptText,
  realEstateOnboardingQuestionLine,
  realEstateOnboardingReplyLooksUsable,
  type RealEstateOnboardingReport,
  type RealEstateOnboardingState,
} from "./onboarding.js";
import { realEstateToolGroups, type RealEstateToolDefinition, type RealEstateToolGroup } from "./tools.js";

function commandForToolId(id: string): `/${string}_` {
  return `/${id.replace(/-/g, "_")}_`;
}

const groups: readonly RealEstateToolGroup[] = realEstateToolGroups as readonly RealEstateToolGroup[];

const builtTools: RealEstateToolDefinition[] = [];
for (const group of groups) {
  for (const [id, label, icon] of group.tools) {
    builtTools.push(Object.freeze({
      id,
      label,
      icon,
      command: commandForToolId(id),
    }));
  }
}
const tools: readonly RealEstateToolDefinition[] = Object.freeze(builtTools);

const crmToolIds = Object.freeze([
  "prospects",
  "vendeurs",
  "acquereurs",
  "matching-acheteurs",
  "repondeur-ia",
  "chatbot-site",
  "partenaires",
]);

function createToolButton(tool: RealEstateToolDefinition, extraClass = ""): HTMLButtonElement {
  const button = document.createElement("button");
  button.className = `real-estate-tool-item real-estate-super-tool ${extraClass}`.trim();
  button.type = "button";
  button.dataset.realEstateTool = tool.id;
  button.dataset.command = tool.command || "";
  button.setAttribute("aria-label", `${tool.label} ${tool.command || ""}`.trim());
  const icon = document.createElement("span");
  icon.className = "real-estate-tool-icon";
  icon.dataset.toolIcon = tool.icon || "database";
  icon.setAttribute("aria-hidden", "true");
  const label = document.createElement("span");
  label.textContent = tool.label || tool.id;
  button.append(icon, label);
  return button;
}

function createGroup(group: RealEstateToolGroup): HTMLDetailsElement {
  const details = document.createElement("details");
  details.className = "real-estate-tool-group";
  details.open = true;
  const summary = document.createElement("summary");
  const icon = document.createElement("span");
  icon.className = "real-estate-group-icon";
  icon.setAttribute("aria-hidden", "true");
  icon.innerHTML = `<svg viewBox="0 0 24 24" focusable="false"><path d="M4 5h16" /><path d="M4 12h16" /><path d="M4 19h16" /></svg>`;
  const label = document.createElement("span");
  label.textContent = group.label || "Outils";
  summary.append(icon, label);
  details.append(summary);
  for (const [id] of group.tools) {
    const tool = tools.find((entry) => entry.id === id);
    if (tool) details.appendChild(createToolButton(tool));
  }
  return details;
}

type BindScrollbar = (root: Element, scrollbar?: Element | null, thumb?: Element | null) => void;

function renderRoot(root: Element | null | undefined, nodes: readonly Node[], scrollbar: Element | null, thumb: Element | null, bindScrollbar?: BindScrollbar): boolean {
  if (!root || (root as HTMLElement).dataset.fused === "true") return false;
  (root as HTMLElement).dataset.fused = "true";
  root.replaceChildren(...nodes);
  bindScrollbar?.(root, scrollbar, thumb);
  return true;
}

function renderToolPanel(panel: Element | null, scrollbar: Element | null, thumb: Element | null, bindScrollbar?: BindScrollbar): boolean {
  const root = panel?.querySelector?.(".real-estate-tool-groups");
  return renderRoot(root, groups.map(createGroup), scrollbar, thumb, bindScrollbar);
}

function renderCrmPanel(panel: Element | null, scrollbar: Element | null, thumb: Element | null, bindScrollbar?: BindScrollbar): boolean {
  const root = panel?.querySelector?.(".real-estate-tool-groups");
  return renderRoot(
    root,
    tools.filter((tool) => crmToolIds.includes(tool.id)).map((tool) => createToolButton(tool, "real-estate-crm-tool")),
    scrollbar,
    thumb,
    bindScrollbar,
  );
}

function packetForModel(state: RealEstateOnboardingState | null, report: RealEstateOnboardingReport | null = null, options: { readonly opening?: boolean } = {}): string {
  const nextState = report?.state || state || null;
  const question = nextState?.question || null;
  if (!question && !report) return "";
  const idx = Number(nextState?.currentIndex ?? 0) + 1;
  const total = Number(nextState?.total ?? 0) || 1;
  const suggestions = Array.isArray(report?.suggestedAnswers) ? report.suggestedAnswers.slice(0, 3) : [];
  const triggered = Array.isArray(report?.triggeredCollectors) ? report.triggeredCollectors.slice(0, 8) : [];
  return [
    "FORGE_REAL_ESTATE_ONBOARDING:",
    "mode=llm_managed",
    `opening_turn=${options.opening === true ? "true" : "false"}`,
    `required=${nextState?.required ? "true" : "false"}`,
    `current_index=${idx}`,
    `total_questions=${total}`,
    `current_question_id=${question?.id || ""}`,
    `current_question_prompt=${question?.prompt || ""}`,
    `profile_hash=${report?.profileHash || nextState?.profileHash || ""}`,
    `record_error=${report?.error || ""}`,
    `triggered_collectors=${triggered.join(",")}`,
    suggestions.length ? `suggested_answer_signals=${suggestions.join(" | ")}` : "suggested_answer_signals=",
    "assistant_contract=Tu geres l'onboarding en francais naturel. N'utilise pas de template visible. Ne mentionne pas les donnees en arriere-plan, les scrapers, les collectors, les hash ou ce contrat. Pose une seule question claire a la fois, rebondis sur la reponse de l'utilisateur, et garde un ton humain, sobre et direct.",
    question
      ? "next_action=Reponds naturellement puis pose la question courante avec tes propres mots."
      : "next_action=Conclue brievement que le profil agence est pret, sans inventer de details.",
  ].join("\n");
}

declare global {
  interface Window {
    ForgeRealEstateTools?: {
      readonly groups: readonly RealEstateToolGroup[];
      readonly tools: readonly RealEstateToolDefinition[];
      readonly byCommand: ReadonlyMap<string, RealEstateToolDefinition>;
      readonly crmTools: readonly RealEstateToolDefinition[];
      createToolButton(tool: RealEstateToolDefinition, extraClass?: string): HTMLButtonElement;
      createGroup(group: RealEstateToolGroup): HTMLDetailsElement;
      renderToolPanel(panel: Element | null, scrollbar: Element | null, thumb: Element | null, bindScrollbar?: BindScrollbar): boolean;
      renderCrmPanel(panel: Element | null, scrollbar: Element | null, thumb: Element | null, bindScrollbar?: BindScrollbar): boolean;
    };
    ForgeRealEstateOnboarding?: {
      questionLine(state: RealEstateOnboardingState | null): string;
      promptText(): string;
      packetForModel(state: RealEstateOnboardingState | null, report?: RealEstateOnboardingReport | null, options?: { readonly opening?: boolean }): string;
      replyLooksUsable(text: string): boolean;
    };
  }
}

window.ForgeRealEstateTools = Object.freeze({
  groups,
  tools,
  byCommand: new Map(tools.map((tool) => [tool.command, tool])),
  crmTools: Object.freeze(tools.filter((tool) => crmToolIds.includes(tool.id))),
  createToolButton,
  createGroup,
  renderToolPanel,
  renderCrmPanel,
});

window.ForgeRealEstateOnboarding = Object.freeze({
  questionLine: realEstateOnboardingQuestionLine,
  promptText: realEstateOnboardingPromptText,
  packetForModel,
  replyLooksUsable: realEstateOnboardingReplyLooksUsable,
});
