import { useMemo } from "react";
import { BRAIN_PLAN_COMMAND } from "../shared/ipc-contract";

interface RightPanelSliceProps {
  open: boolean;
  agentName?: string;
  planSourceText?: string;
}

interface PlanPanelStep {
  label: string;
  text: string;
}

interface PlanPanelState {
  active: boolean;
  title: string;
  context: string;
  currentFocus: string;
  stopCondition: string;
  steps: PlanPanelStep[];
}

function unescapeSlotValue(value: string): string {
  return value
    .replace(/\\n/g, " ")
    .replace(/\\"/g, "\"")
    .replace(/\\'/g, "'")
    .replace(/\\\\/g, "\\")
    .replace(/\s+/g, " ")
    .trim();
}

function codeActSlotValue(line: string, name: string): string {
  const pattern = new RegExp(`${name}=("((?:\\\\.|[^"])*)"|'((?:\\\\.|[^'])*)'|([^\\s]+))`, "i");
  const match = line.match(pattern);
  return unescapeSlotValue(match?.[2] ?? match?.[3] ?? match?.[4] ?? "");
}

function latestPlanCodeActLine(sourceText: string): string {
  const commandIndex = sourceText.lastIndexOf(BRAIN_PLAN_COMMAND);
  if (commandIndex < 0) {
    return "";
  }
  return sourceText.slice(commandIndex).split(/\r?\n/, 1)[0] ?? "";
}

function planStepsFromLine(line: string): PlanPanelStep[] {
  const explicitSteps = Array.from({ length: 8 }, (_, index) => {
    const value = codeActSlotValue(line, `step${index + 1}`);
    return value ? { label: String(index + 1), text: value } : null;
  }).filter((step): step is PlanPanelStep => Boolean(step));

  if (explicitSteps.length > 0) {
    return explicitSteps;
  }

  const compactSteps = codeActSlotValue(line, "steps");
  if (!compactSteps) {
    return [];
  }
  return compactSteps
    .split(/\s*[|;]\s*/g)
    .map((step) => step.trim())
    .filter(Boolean)
    .slice(0, 8)
    .map((text, index) => ({ label: String(index + 1), text }));
}

function planPanelStateFromText(sourceText: string): PlanPanelState {
  const line = latestPlanCodeActLine(sourceText);
  if (!line) {
    return {
      active: false,
      title: "Plan",
      context: "",
      currentFocus: "",
      stopCondition: "",
      steps: []
    };
  }

  const title = codeActSlotValue(line, "title") || codeActSlotValue(line, "name") || "Action plan";
  return {
    active: true,
    title,
    context: codeActSlotValue(line, "context") || codeActSlotValue(line, "goal"),
    currentFocus: codeActSlotValue(line, "current_focus") || codeActSlotValue(line, "focus"),
    stopCondition: codeActSlotValue(line, "stop_condition") || codeActSlotValue(line, "done_when"),
    steps: planStepsFromLine(line)
  };
}

function EmptyPlanIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M6.2 5.9h11.6" />
      <path d="M6.2 12h11.6" />
      <path d="M6.2 18.1h11.6" />
      <path d="M3.9 5.9h.08" />
      <path d="M3.9 12h.08" />
      <path d="M3.9 18.1h.08" />
      <rect x="2.7" y="4.7" width="2.4" height="2.4" rx="0.65" />
      <rect x="2.7" y="10.8" width="2.4" height="2.4" rx="0.65" />
      <rect x="2.7" y="16.9" width="2.4" height="2.4" rx="0.65" />
    </svg>
  );
}

export function RightPanelSlice({ open, agentName = "Agent", planSourceText = "" }: RightPanelSliceProps) {
  const plan = useMemo(() => planPanelStateFromText(planSourceText), [planSourceText]);
  const assistantLabel = agentName.trim() || "Agent";

  return (
    <aside
      id="right-panel"
      className={`rightPanel ${plan.active ? "rightPanel--planActive" : "rightPanel--planIdle"}`}
      aria-label="Plan sidebar"
      aria-hidden={!open}
    >
      <div className="rightPanel__inner">
        <header className="rightPanel__header">
          <span className="rightPanel__eyebrow">Plan</span>
          <strong>{plan.title}</strong>
        </header>

        {plan.active ? (
          <div className="rightPanelPlan" aria-label="Current action plan">
            <div className="rightPanelWorkingEvent" role="status" aria-live="polite">
              <span className="sessionRow__loaderViewbox rightPanelWorkingEvent__loaderViewbox" aria-hidden="true">
                <span className="loader" />
              </span>
              <span>
                <strong>{assistantLabel}</strong> is shaping the action plan...
              </span>
            </div>

            {plan.context ? <p className="rightPanelPlan__context">{plan.context}</p> : null}

            {plan.steps.length > 0 ? (
              <ol className="rightPanelPlanSteps">
                {plan.steps.map((step) => (
                  <li className="rightPanelPlanStep" key={`${step.label}-${step.text}`}>
                    <span className="rightPanelPlanStep__index">{step.label}</span>
                    <span className="rightPanelPlanStep__text">{step.text}</span>
                  </li>
                ))}
              </ol>
            ) : (
              <p className="rightPanelPlan__empty">The outline is being prepared.</p>
            )}

            {plan.currentFocus || plan.stopCondition ? (
              <div className="rightPanelPlanMeta">
                {plan.currentFocus ? (
                  <section className="rightPanelPlanMeta__item">
                    <span>Current focus</span>
                    <strong>{plan.currentFocus}</strong>
                  </section>
                ) : null}
                {plan.stopCondition ? (
                  <section className="rightPanelPlanMeta__item">
                    <span>Done when</span>
                    <strong>{plan.stopCondition}</strong>
                  </section>
                ) : null}
              </div>
            ) : null}
          </div>
        ) : (
          <div className="rightPanelEmpty" role="status">
            <EmptyPlanIcon />
            <strong>No plan yet</strong>
          </div>
        )}
      </div>
    </aside>
  );
}
