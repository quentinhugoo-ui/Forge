// @ts-nocheck

export function createRealEstateModeRuntime(deps) {
  const active = () => !!deps.getActive?.();

  function sync(options = {}) {
    const skipHeavy = options?.skipHeavy === true;
    const isActive = active();
    document.body.classList.toggle("real-estate-mode", isActive);
    deps.modeButton?.()?.classList?.toggle("is-active", isActive);
    deps.modeButton?.()?.setAttribute("aria-pressed", isActive ? "true" : "false");
    const homeButton = deps.homeSectionButton?.();
    if (homeButton) homeButton.hidden = !isActive;
    homeButton?.classList?.toggle("is-active", isActive && !deps.isWebExplorerActive?.());
    deps.sections?.()?.setActive?.("real-estate", isActive);
    deps.sections?.()?.setActive?.("real-estate-main", isActive && !deps.isWebExplorerActive?.());

    if (isActive && !skipHeavy) {
      deps.resetInactiveTradingChartSurface?.();
      deps.refreshAlphaAfterSectionBoundary?.();
      deps.setInitProgress?.(Math.max(Number(deps.getInitProgress?.() || 0), 3));
      deps.syncInitCanvas?.();
      deps.scheduleInitLoop?.();
      deps.requestLlmStatusRefresh?.();
      if (deps.llmInstallReady?.()) void deps.refreshOnboardingState?.({ announce: true });
    }

    if (!isActive) {
      deps.setToolsOpen?.(false);
      deps.setContactsOpen?.(false);
      deps.setOnboardingState?.(null);
      const timer = deps.getInitTimer?.();
      if (timer) window.clearInterval(timer);
      deps.setInitTimer?.(0);
      deps.setInitProgress?.(0);
      deps.setStatusRefreshInFlight?.(false);
      deps.alphaDropZone?.()?.querySelector?.(".real-estate-init-progress")?.remove();
      document.body.classList.remove("real-estate-onboarding-active");
      document.body.classList.remove("real-estate-llm-initializing");
    }

    deps.syncLanguage?.();
    deps.resetJobListRenderKey?.();
    deps.renderJobs?.();
    deps.renderWebExplorerHistoryPanel?.(true);
    deps.updateWorkspaceBreadcrumb?.();
    deps.syncCanvasChatSendState?.();
  }

  function setActive(nextActive) {
    const previousActive = active();
    const next = !!nextActive;
    deps.setActive?.(next);
    deps.persistActive?.(next);

    if (next) {
      try { deps.closeBoom?.(); } catch (_) {}
      try { deps.closeTrading?.(); } catch (_) {}
    }

    const selectedJob = deps.currentJob?.();
    const selectedBelongsToRealEstate = selectedJob ? deps.jobIsRealEstate?.(selectedJob) : false;
    if (selectedJob && selectedBelongsToRealEstate !== next) {
      deps.startNewSession?.();
    } else if (previousActive !== next && !selectedJob && !deps.hasAlphaSource?.()) {
      deps.setNewSessionTitle?.(next ? "Nouvelle session immo" : "New session");
    }

    if (previousActive !== next && deps.composerEmpty?.()) {
      deps.resetPlaceholderAnimation?.();
    }

    sync();
    deps.dispatchMode?.({ active: next, webExplorerActive: !!deps.isWebExplorerActive?.() });
    deps.refreshHarvesterStatus?.();
  }

  return {
    sync,
    setActive,
    toggle: () => setActive(!active()),
  };
}
