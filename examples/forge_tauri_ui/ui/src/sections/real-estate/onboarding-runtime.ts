// @ts-nocheck
import {
  realEstateOnboardingPromptText,
  realEstateOnboardingQuestionLine,
  realEstateOnboardingReplyLooksUsable,
} from "./onboarding.js";

export function createRealEstateOnboardingRuntime(deps) {
  const get = (name) => deps.get?.(name);
  const set = (name, value) => deps.set?.(name, value);

  const active = () => (
    deps.getModeActive?.()
    && llmInstallReady()
    && get("state")?.required
    && get("state")?.question
  );

  function syncMachine(status, questionId = "") {
    const normalizedStatus = String(status || "idle");
    const normalizedQuestionId = String(questionId || "");
    const signature = `${normalizedStatus}:${normalizedQuestionId}`;
    if (signature === get("machineSignature")) return;
    set("machineSignature", signature);
    deps.dispatchOnboarding?.(normalizedStatus, normalizedQuestionId);
  }

  function llmInstallRows() {
    return (deps.getProviderRows?.() || []).map(([id, status]) => ({
      id,
      checked: !!status,
      ready: !!status && deps.providerCliEffectiveInstalled?.(status),
    }));
  }

  function llmInstallReady() {
    const rows = llmInstallRows();
    return rows.every((row) => row.checked && row.ready);
  }

  function llmInstallProgressTarget() {
    const rows = llmInstallRows();
    const checked = rows.filter((row) => row.checked).length;
    const ready = rows.filter((row) => row.ready).length;
    return Math.min(100, Math.round(8 + checked * 10 + ready * 20));
  }

  function requestLlmStatusRefresh({ force = false } = {}) {
    if (!deps.getModeActive?.() || get("statusRefreshInFlight")) return;
    const now = Date.now();
    if (!force && now - Number(get("statusLastRefreshAt") || 0) < 15000) return;
    set("statusRefreshInFlight", true);
    set("statusLastRefreshAt", now);
    Promise.allSettled([
      deps.refreshOpenAiProviderStatus?.({ silent: true }),
      deps.refreshCliProviderStatuses?.({ silent: true }),
    ]).then(() => {
      syncCanvas();
      if (llmInstallReady()) void refreshState({ announce: true });
    }).finally(() => {
      set("statusRefreshInFlight", false);
    });
  }

  function ensureInitProgressEl() {
    const dropZone = deps.alphaDropZone?.();
    if (!dropZone) return null;
    let root = dropZone.querySelector(".real-estate-init-progress");
    if (root) return root;
    root = document.createElement("div");
    root.className = "real-estate-init-progress";
    root.innerHTML = `<span class="real-estate-init-progress-bar"></span>`;
    dropZone.appendChild(root);
    return root;
  }

  function syncInitCanvas() {
    const initializing = deps.getModeActive?.() && !llmInstallReady();
    document.body.classList.toggle("real-estate-llm-initializing", initializing);
    const dropZone = deps.alphaDropZone?.();
    if (!initializing) {
      dropZone?.querySelector?.(".real-estate-init-progress")?.remove();
      return false;
    }
    syncMachine("initializing");
    const nextProgress = Math.max(
      Number(get("initProgress") || 0),
      Math.min(llmInstallProgressTarget(), Number(get("initProgress") || 0) + 0.65),
    );
    set("initProgress", nextProgress);
    const title = deps.alphaDropZoneTitle?.();
    const sub = deps.alphaDropZoneSub?.();
    if (title) title.textContent = "Initialisation de Forge";
    if (sub) sub.textContent = "";
    if (dropZone) dropZone.setAttribute("aria-label", "Initialisation de Forge");
    const progress = ensureInitProgressEl();
    const bar = progress?.querySelector?.(".real-estate-init-progress-bar");
    if (bar) bar.style.width = `${Math.max(4, Math.min(100, nextProgress))}%`;
    return true;
  }

  function scheduleInitLoop() {
    if (!deps.getModeActive?.() || get("initTimer")) return;
    requestLlmStatusRefresh({ force: true });
    const timer = window.setInterval(() => {
      if (!deps.getModeActive?.()) {
        window.clearInterval(get("initTimer"));
        set("initTimer", 0);
        return;
      }
      if (llmInstallReady()) {
        set("initProgress", 100);
        syncInitCanvas();
        window.clearInterval(get("initTimer"));
        set("initTimer", 0);
        void refreshState({ announce: true });
        return;
      }
      syncInitCanvas();
      requestLlmStatusRefresh();
    }, 1100);
    set("initTimer", timer);
  }

  function questionLine(state = get("state")) {
    return realEstateOnboardingQuestionLine(state) || "";
  }

  function promptText(state = get("state")) {
    return realEstateOnboardingPromptText(state) || "";
  }

  function syncCanvas() {
    if (syncInitCanvas()) return;
    const isActive = active();
    document.body.classList.toggle("real-estate-onboarding-active", isActive);
    syncMachine(isActive ? "asking" : "idle", isActive ? get("state")?.question?.id : "");
    if (!isActive) return;
    const title = deps.alphaDropZoneTitle?.();
    const sub = deps.alphaDropZoneSub?.();
    const dropZone = deps.alphaDropZone?.();
    if (title) title.textContent = questionLine();
    if (sub) sub.textContent = promptText();
    if (dropZone) dropZone.setAttribute("aria-label", "Répondre à l'onboarding agence via la barre de chat");
  }

  function packetForModel(state = get("state"), report = null, options = {}) {
    if (!deps.getModeActive?.()) return "";
    return deps.packetForModel?.(state, report, options) || "";
  }

  function targetForTurn() {
    const targets = deps.canvasChatRuntimeTargets?.("") || [];
    return targets[0] || {
      runtime: "codex",
      label: "Codex",
      modelRef: deps.selectedOpenAiModelRef?.() || "gpt-5.3-codex",
      reasoningEffort: deps.selectedCanvasReasoningEffort?.("codex"),
      text: "",
    };
  }

  function replyLooksUsable(text) {
    return realEstateOnboardingReplyLooksUsable(String(text || ""));
  }

  async function requestLlmTurn(state = get("state")) {
    const questionId = state?.question?.id || "";
    if (!deps.getModeActive?.() || !questionId || get("llmTurnInFlight")) return;
    if (get("announcedQuestion") === questionId) return;
    if (deps.canvasChatBusyInCurrentSession?.()) return;
    if (!deps.forgeCanInvoke?.()) return;
    set("announcedQuestion", questionId);
    set("llmTurnInFlight", true);
    const target = targetForTurn();
    const turnId = `forge-real-estate-onboarding-${Date.now()}`;
    let sessionJobId = deps.currentAlphaSessionJobId?.();
    deps.setAlphaActiveTab?.("forge");
    deps.setCanvasChatBusy?.(true);
    deps.setCanvasChatPendingAssistants?.([target]);
    try {
      if (!sessionJobId && deps.isEmptyAlphaContext?.()) {
        await deps.startAlphaEmptySession?.();
        sessionJobId = deps.currentAlphaSessionJobId?.();
      }
      const response = await deps.forgeInvoke?.("forge_canvas_assistant_turn", {
        request: {
          message: packetForModel(state, null, { opening: true }),
          jobId: sessionJobId || deps.currentAlphaSessionJobId?.() || null,
          modelRef: target.modelRef,
          reasoningEffort: target.reasoningEffort,
          runtime: target.runtime,
          maxLogLines: 12,
          turnId,
          privacyScope: "agence_immo",
        },
      }, { section: "real-estate", timeoutMs: 120000 });
      const assistantMessage = String(response?.assistantMessage || "").trim();
      if (replyLooksUsable(assistantMessage)) {
        deps.appendCanvasChatMessage?.("assistant", assistantMessage, {
          turnId,
          sessionJobId: sessionJobId || "",
          agentLabel: deps.canvasChatTargetLabel?.(deps.canvasResponseRuntime?.(response, target)),
          provider: response?.provider || null,
          toolEvents: Array.isArray(response?.toolEvents) ? response.toolEvents : [],
        });
      } else {
        set("announcedQuestion", "");
      }
    } catch (err) {
      console.warn("[real-estate] LLM onboarding turn unavailable", err);
      set("announcedQuestion", "");
    } finally {
      set("llmTurnInFlight", false);
      deps.setCanvasChatPendingAssistants?.([]);
      deps.setCanvasChatBusy?.(false);
      deps.syncAlphaDropSurface?.();
    }
  }

  async function refreshState(options = {}) {
    if (deps.getModeActive?.() && !llmInstallReady()) {
      syncInitCanvas();
      scheduleInitLoop();
      return get("state");
    }
    if (!deps.getModeActive?.() || !deps.forgeTauriInvoke || get("loading")) return get("state");
    set("loading", true);
    try {
      const state = await deps.forgeTauriInvoke("real_estate_onboarding_state", {}, {
        section: "real-estate",
        timeoutMs: 6000,
        dedupeKey: "onboarding-state",
      });
      set("state", state || null);
      syncCanvas();
      const questionId = state?.question?.id || "";
      if (options.announce && questionId && questionId !== get("announcedQuestion")) {
        void requestLlmTurn(state);
      }
      return get("state");
    } catch (err) {
      console.warn("[real-estate] onboarding state unavailable", err);
      return get("state");
    } finally {
      set("loading", false);
    }
  }

  async function recordAnswer(answer) {
    const question = get("state")?.question;
    if (!question || !deps.forgeTauriInvoke) return null;
    try {
      const report = await deps.forgeTauriInvoke("real_estate_onboarding_answer", {
        questionId: question.id,
        answer,
      }, {
        section: "real-estate",
        timeoutMs: 16000,
        dedupeKey: `onboarding-${question.id}`,
      });
      set("state", report?.state || get("state"));
      set("announcedQuestion", report?.state?.question?.id || "");
      syncCanvas();
      deps.refreshRealEstateHarvesterStatus?.();
      return report || null;
    } catch (err) {
      console.warn("[real-estate] onboarding answer not recorded", err);
      return {
        state: get("state"),
        error: String(err?.message || err || ""),
      };
    }
  }

  return {
    active,
    llmInstallRows,
    llmInstallReady,
    llmInstallProgressTarget,
    requestLlmStatusRefresh,
    syncInitCanvas,
    scheduleInitLoop,
    questionLine,
    promptText,
    syncCanvas,
    packetForModel,
    targetForTurn,
    replyLooksUsable,
    requestLlmTurn,
    refreshState,
    recordAnswer,
  };
}
