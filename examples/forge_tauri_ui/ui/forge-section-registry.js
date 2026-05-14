(function () {
  "use strict";

  const sections = new Map();
  const actionQueues = new Map();
  let shellSection = "alpha";
  let bootPhase = "boot";

  function normalizeId(id) {
    return String(id || "").trim().toLowerCase();
  }

  function nowMs() {
    return Date.now ? Date.now() : 0;
  }

  function register(definition = {}) {
    const id = normalizeId(definition.id);
    if (!id) throw new Error("ForgeSectionRegistry.register requires an id");
    const existing = sections.get(id) || {};
    const next = {
      id,
      label: definition.label || existing.label || id,
      kind: definition.kind || existing.kind || "section",
      parent: definition.parent || existing.parent || "",
      lazy: definition.lazy !== false,
      bootSafe: definition.bootSafe === true || existing.bootSafe === true,
      active: definition.active === true || existing.active === true,
      mounted: existing.mounted === true,
      owns: Array.isArray(definition.owns) ? definition.owns.slice() : (existing.owns || []),
      createdAtMs: existing.createdAtMs || nowMs(),
      updatedAtMs: nowMs(),
    };
    sections.set(id, next);
    return { ...next };
  }

  function setActive(id, active = true) {
    const normalized = normalizeId(id);
    if (!normalized) return null;
    const section = sections.get(normalized) || register({ id: normalized });
    section.active = !!active;
    section.updatedAtMs = nowMs();
    sections.set(normalized, section);
    return { ...section };
  }

  function setShellSection(id) {
    const normalized = normalizeId(id) || "alpha";
    shellSection = normalized;
    for (const section of sections.values()) {
      if (section.kind === "shell-section") {
        section.active = section.id === normalized;
        section.updatedAtMs = nowMs();
      }
    }
    if (sections.has(normalized)) setActive(normalized, true);
    return shellSection;
  }

  function isActive(id) {
    const normalized = normalizeId(id);
    if (!normalized) return false;
    if (normalized === "shell") return true;
    if (normalized === shellSection) return true;
    return sections.get(normalized)?.active === true;
  }

  function markReady() {
    bootPhase = "ready";
  }

  function queueAction(id, action, payload = {}) {
    const normalized = normalizeId(id);
    const name = String(action || "").trim();
    if (!normalized || !name) return null;
    const queue = actionQueues.get(normalized) || [];
    const entry = {
      action: name,
      payload,
      createdAtMs: nowMs(),
    };
    queue.push(entry);
    actionQueues.set(normalized, queue);
    return { ...entry };
  }

  function consumeQueuedActions(id) {
    const normalized = normalizeId(id);
    if (!normalized) return [];
    const queue = actionQueues.get(normalized) || [];
    actionQueues.set(normalized, []);
    return queue.map((entry) => ({ ...entry, payload: { ...(entry.payload || {}) } }));
  }

  function phase() {
    return bootPhase;
  }

  function list() {
    return Array.from(sections.values()).map((section) => ({ ...section }));
  }

  function snapshot() {
    return {
      phase: bootPhase,
      shellSection,
      sections: list(),
      queuedActions: Object.fromEntries(
        Array.from(actionQueues.entries()).map(([id, queue]) => [id, queue.length]),
      ),
    };
  }

  window.ForgeSectionRegistry = Object.freeze({
    register,
    setActive,
    activate: (id) => setActive(id, true),
    deactivate: (id) => setActive(id, false),
    setShellSection,
    isActive,
    markReady,
    queueAction,
    consumeQueuedActions,
    phase,
    list,
    snapshot,
  });
})();
