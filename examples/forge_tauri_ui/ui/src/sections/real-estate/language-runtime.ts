// @ts-nocheck

function setNodeText(node, text) {
  if (node) node.textContent = text;
}

function setButtonTextByAction(root, action, text) {
  const button = root?.querySelector?.(`[data-action="${action}"]`);
  if (button) button.textContent = text;
}

function setProfileMenuText(profileMenu, action, text) {
  const label = profileMenu?.querySelector?.(`[data-profile-action="${action}"] span`);
  if (label) label.textContent = text;
}

export function createRealEstateLanguageRuntime(deps) {
  function sync() {
    const fr = !!deps.isActive?.();
    document.documentElement.lang = fr ? "fr" : "en";
    if (!deps.hasSource?.()) deps.setNewSessionTitle?.(fr ? "Nouvelle session immo" : "New session");

    setNodeText(document.querySelector(".pin-section > .pin-heading"), fr ? "Épinglés" : "Pinned");
    setNodeText(document.querySelector(".history-heading:not(.webexplorer-history-heading) > span"), fr ? "Récents" : "Recents");
    setNodeText(document.querySelector(".webexplorer-history-section-pinned .pin-heading"), fr ? "Web épinglé" : "Pinned web");
    setNodeText(document.querySelector(".webexplorer-history-heading > span"), fr ? "Historique web" : "Web history");

    const webExplorerClearHistoryBtn = deps.webExplorerClearHistoryBtn?.();
    if (webExplorerClearHistoryBtn) webExplorerClearHistoryBtn.textContent = fr ? "Effacer" : "Clear";

    const forgePinDrop = deps.forgePinDrop?.();
    const forgePinDropText = deps.forgePinDropText?.();
    if (forgePinDrop) forgePinDrop.setAttribute("aria-label", fr ? "Glisser une session immo ici pour l'épingler" : "Drag a calculation here to pin it");
    if (forgePinDropText && !forgePinDrop?.classList?.contains("has-pinned")) {
      forgePinDropText.textContent = fr ? "Glisser pour épingler" : "Drag to pin";
    }
    deps.forgePinMenuBtn?.()?.setAttribute("aria-label", fr ? "Réglages du projet épinglé" : "Pinned project settings");

    const alphaDropZoneTitle = deps.alphaDropZoneTitle?.();
    const alphaDropZoneSub = deps.alphaDropZoneSub?.();
    const alphaDropZone = deps.alphaDropZone?.();
    if (alphaDropZoneTitle) alphaDropZoneTitle.textContent = fr ? "Dépose n'importe quel fichier" : "Drop any file";
    if (alphaDropZoneSub) {
      alphaDropZoneSub.textContent = fr
        ? "Calcul lourd dans n'importe quel domaine immobilier - données agence, biens, mandats, prospects, annonces, veille, fiscalité, juridique, tout. Le LLM reste hors des fichiers et des maths, économisant massivement des tokens."
        : "Heavy compute in any domain - data, code, medical imaging, genomics, anything. The LLM stays out of files and math, saving massive tokens.";
    }
    alphaDropZone?.setAttribute("aria-label", fr ? "Déposer des fichiers agence" : "Upload OHLCV CSV files");
    deps.syncSidebarLabels?.();

    const webExplorerBtn = document.getElementById("webexplorer");
    webExplorerBtn?.setAttribute("aria-label", fr ? "Ouvrir Google" : "Open web explorer");
    webExplorerBtn?.setAttribute("title", fr ? "Ouvrir Google" : "Open web explorer");
    const searchBtn = document.getElementById("forgeSearchBtn");
    searchBtn?.setAttribute("aria-label", fr ? "Rechercher les sessions et projets" : "Search sessions and projects");
    searchBtn?.setAttribute("title", fr ? "Rechercher (Ctrl+K)" : "Search (Ctrl+K)");
    document.getElementById("forgeSearchInput")?.setAttribute("placeholder", fr ? "Rechercher sessions, projets, programmes..." : "Search sessions, projects, programs...");
    document.getElementById("windowMinimize")?.setAttribute("aria-label", fr ? "Réduire" : "Minimize");
    document.getElementById("windowMaximize")?.setAttribute("aria-label", fr ? "Agrandir" : "Maximize");
    document.getElementById("windowClose")?.setAttribute("aria-label", fr ? "Fermer" : "Close");

    deps.forgeCanvasChat?.()?.setAttribute("aria-label", fr ? "Parler à l'assistant agence dans cette session Forge" : "Talk to Codex in this Forge session");
    deps.forgeCanvasChatCommandInput?.()?.setAttribute("aria-label", fr ? "Commande programme" : "Command prefix");
    deps.forgeCanvasChatGeminiInput?.()?.setAttribute("aria-label", fr ? "Consigne Gemini" : "Gemini prompt");
    deps.forgeCanvasChatClaudeInput?.()?.setAttribute("aria-label", fr ? "Consigne Claude" : "Claude prompt");

    const alphaProofToggle = deps.alphaProofToggle?.();
    alphaProofToggle?.setAttribute("aria-label", deps.alphaProofPanelOpen?.()
      ? (fr ? "Fermer le panneau droit" : "Close right panel")
      : (fr ? "Ouvrir le panneau droit" : "Open right panel"));
    alphaProofToggle?.setAttribute("title", deps.alphaProofPanelOpen?.()
      ? (fr ? "Fermer le panneau droit" : "Close right panel")
      : (fr ? "Ouvrir le panneau droit" : "Open right panel"));

    const workspaceMenu = deps.workspaceMenu?.();
    setButtonTextByAction(workspaceMenu, "choose-folder", fr ? "Choisir le dossier agence" : "Choose workspace");
    setButtonTextByAction(workspaceMenu, "show-folder", fr ? "Afficher le dossier agence" : "Show in Explorer");
    setButtonTextByAction(workspaceMenu, "copy-path", fr ? "Copier le chemin du dossier agence" : "Copy folder path");

    const jobMenu = document.getElementById("forgeJobMenu");
    setButtonTextByAction(jobMenu, "pin", fr ? "Épingler le projet" : "Pin project");
    setButtonTextByAction(jobMenu, "rename", fr ? "Renommer" : "Rename");
    setButtonTextByAction(jobMenu, "archive", fr ? "Archiver" : "Archive");
    setButtonTextByAction(jobMenu, "delete", fr ? "Supprimer" : "Delete");

    const profileMenu = deps.profileMenu?.();
    setProfileMenuText(profileMenu, "settings", fr ? "Réglages" : "Settings");
    setProfileMenuText(profileMenu, "api", fr ? "Fournisseurs IA" : "LLM providers");
    setProfileMenuText(profileMenu, "voice-api", fr ? "Voix & clés API" : "Voice & API keys");
    setProfileMenuText(profileMenu, "docs", fr ? "Documentation" : "Documentation");
    setProfileMenuText(profileMenu, "mcp", "MCP");
    setProfileMenuText(profileMenu, "daemon", fr ? "Service local" : "Daemon");
    setProfileMenuText(profileMenu, "archive", fr ? "Archives" : "Archive");
    setProfileMenuText(profileMenu, "edit-profile", fr ? "Modifier le profil" : "Edit profile");

    const cpu = document.getElementById("panelHardwareCpu");
    const gpu = document.getElementById("panelHardwareGpu");
    if (cpu && /^(detection failed|détection échouée)$/i.test(cpu.textContent || "")) cpu.textContent = fr ? "détection échouée" : "detection failed";
    if (gpu && /^(detection failed|détection échouée)$/i.test(gpu.textContent || "")) gpu.textContent = fr ? "détection échouée" : "detection failed";
    document.querySelectorAll(".panel-hardware-text").forEach((node) => {
      if (/^(No GPU detected|Aucun GPU détecté)$/i.test(node.textContent || "")) {
        node.textContent = fr ? "Aucun GPU détecté" : "No GPU detected";
      }
    });
    deps.syncOnboardingCanvas?.();
  }

  return { sync };
}
