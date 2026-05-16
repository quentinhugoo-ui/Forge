// @ts-nocheck

export function createRealEstatePanelRuntime(deps) {
  let toolsPanelCloseTimer = 0;
  let contactsPanelCloseTimer = 0;

  const toolsPanel = () => deps.toolsPanel?.();
  const contactsPanel = () => deps.contactsPanel?.();
  const toolsScroller = () => toolsPanel()?.querySelector?.(".real-estate-tool-groups");
  const contactsScroller = () => contactsPanel()?.querySelector?.(".real-estate-tool-groups");

  function renderTools() {
    deps.tools?.renderToolPanel?.(
      toolsPanel(),
      deps.toolsScrollbar?.(),
      deps.toolsScrollbarThumb?.(),
      deps.bindScrollbar,
    );
  }

  function renderContacts() {
    deps.tools?.renderCrmPanel?.(
      contactsPanel(),
      deps.contactsScrollbar?.(),
      deps.contactsScrollbarThumb?.(),
      deps.bindScrollbar,
    );
  }

  function syncBounds() {
    const viewportHeight = Math.max(
      0,
      window.visualViewport?.height || window.innerHeight || document.documentElement?.clientHeight || 0,
    );
    let bottom = 210;
    const reserveAbove = (node, margin = 22) => {
      if (!node || node.hidden) return;
      const rect = node.getBoundingClientRect?.();
      if (!rect || rect.width <= 0 || rect.height <= 0 || rect.top <= 0 || rect.top >= viewportHeight) return;
      bottom = Math.max(bottom, Math.ceil(viewportHeight - rect.top + margin));
    };
    reserveAbove(deps.chat?.(), 22);
    reserveAbove(deps.modelAnchor?.(), 14);
    bottom = Math.max(176, Math.min(340, bottom));
    document.documentElement?.style?.setProperty?.("--real-estate-tools-panel-bottom", `${bottom}px`);
    deps.queueScrollbarSync?.(toolsScroller());
    deps.queueScrollbarSync?.(contactsScroller());
  }

  function setFloatingPanelVisibility(panel, open, closeTimerRef) {
    if (!panel) return 0;
    if (closeTimerRef) window.clearTimeout(closeTimerRef);
    if (open) {
      panel.hidden = false;
      panel.classList.remove("is-closing");
      requestAnimationFrame(() => {
        panel.classList.add("is-visible");
        deps.queueScrollbarSync?.(panel.querySelector?.(".real-estate-tool-groups"));
      });
      return 0;
    }
    panel.classList.remove("is-visible");
    panel.classList.add("is-closing");
    return window.setTimeout(() => {
      panel.classList.remove("is-closing");
      panel.hidden = true;
    }, 190);
  }

  function setToolsOpen(open) {
    const next = deps.isActive?.() && !!open;
    deps.dispatch?.({ type: "SET_OVERLAY", overlay: "real-estate-tools", open: next });
    toolsPanelCloseTimer = setFloatingPanelVisibility(toolsPanel(), next, toolsPanelCloseTimer);
    document.body.classList.toggle("real-estate-tools-open", next);
    deps.dispatch?.({ type: "SET_PANEL", panel: "real-estate-tools", open: next });
    deps.toolsButton?.()?.setAttribute("aria-expanded", next ? "true" : "false");
    if (next) {
      syncBounds();
      requestAnimationFrame(syncBounds);
      setContactsOpen(false);
    }
  }

  function setContactsOpen(open) {
    const next = deps.isActive?.() && !!open;
    deps.dispatch?.({ type: "SET_OVERLAY", overlay: "real-estate-contacts", open: next });
    contactsPanelCloseTimer = setFloatingPanelVisibility(contactsPanel(), next, contactsPanelCloseTimer);
    document.body.classList.toggle("real-estate-crm-open", next);
    deps.dispatch?.({ type: "SET_PANEL", panel: "real-estate-contacts", open: next });
    deps.contactsButton?.()?.setAttribute("aria-expanded", next ? "true" : "false");
    if (next) {
      syncBounds();
      requestAnimationFrame(syncBounds);
      setToolsOpen(false);
    }
  }

  function handleToolClick(event) {
    const item = event.target?.closest?.(".real-estate-tool-item");
    if (!item) return;
    const command = item.dataset.command || "";
    if (command) deps.setComposerCommand?.(command);
  }

  function install() {
    renderTools();
    renderContacts();
    syncBounds();

    if (typeof ResizeObserver !== "undefined") {
      const panelBoundsObserver = new ResizeObserver(syncBounds);
      [deps.chat?.(), deps.modelAnchor?.()].filter(Boolean).forEach((node) => {
        panelBoundsObserver.observe(node);
      });
    }

    window.addEventListener("resize", syncBounds, { passive: true });
    window.visualViewport?.addEventListener?.("resize", syncBounds, { passive: true });
    window.visualViewport?.addEventListener?.("scroll", syncBounds, { passive: true });

    deps.runtime?.registerAction?.("toggle-real-estate-tools", () => {
      if (document.body.classList.contains("real-estate-tools-open")) {
        setToolsOpen(false);
      } else {
        setToolsOpen(true);
      }
      deps.refreshStatus?.();
    });
    deps.runtime?.registerAction?.("close-real-estate-tools", () => setToolsOpen(false));
    deps.runtime?.registerAction?.("toggle-real-estate-contacts", () => {
      setContactsOpen(!document.body.classList.contains("real-estate-crm-open"));
    });
    deps.runtime?.registerAction?.("close-real-estate-contacts", () => setContactsOpen(false));

    toolsPanel()?.addEventListener("click", handleToolClick);
    contactsPanel()?.addEventListener("click", handleToolClick);
  }

  return {
    install,
    renderTools,
    renderContacts,
    syncBounds,
    setToolsOpen,
    setContactsOpen,
    handleToolClick,
  };
}
