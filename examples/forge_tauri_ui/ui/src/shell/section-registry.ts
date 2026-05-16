import type { ForgeSectionCell, ForgeSectionDefinition, ForgeSectionLifecycle, ForgeShellState } from "./types.js";

export class ForgeTypedSectionRegistry {
  readonly #sections = new Map<string, ForgeSectionDefinition>();
  readonly #cells = new Map<string, ForgeSectionCell>();

  register(section: ForgeSectionDefinition): ForgeSectionDefinition {
    const id = section.id.trim();
    if (!id) throw new Error("section id is required");
    const normalized = Object.freeze({
      ...section,
      id,
      bootSafe: section.bootSafe === true,
      owns: Object.freeze([...(section.owns || [])]),
      permissions: Object.freeze([...(section.permissions || [])]),
      commands: Object.freeze([...(section.commands || [])]),
    });
    this.#sections.set(id, normalized);
    const previous = this.#cells.get(id);
    this.#cells.set(id, freezeCell(normalized, previous?.lifecycle || "idle", previous?.active === true));
    return normalized;
  }

  get(id: string): ForgeSectionDefinition | null {
    return this.#sections.get(id) ?? null;
  }

  list(): readonly ForgeSectionDefinition[] {
    return Object.freeze(Array.from(this.#sections.values()));
  }

  cells(): readonly ForgeSectionCell[] {
    return Object.freeze(Array.from(this.#cells.values()));
  }

  applyState(state: ForgeShellState): readonly ForgeSectionCell[] {
    for (const section of this.#sections.values()) {
      const active = sectionIsActive(section.id, state);
      const lifecycle: ForgeSectionLifecycle = active ? "active" : this.#cells.get(section.id)?.lifecycle === "active" ? "hidden" : "idle";
      this.#cells.set(section.id, freezeCell(section, lifecycle, active, projectionForSection(section, state)));
    }
    return this.cells();
  }
}

function sectionIsActive(id: string, state: ForgeShellState): boolean {
  if (id === state.activeSection) return true;
  if (state.activeSections[id] === true) return true;
  if (id === "real-estate") return state.mode === "agence-immo";
  return false;
}

function freezeCell(
  section: ForgeSectionDefinition,
  lifecycle: ForgeSectionLifecycle,
  active: boolean,
  projection: Readonly<Record<string, unknown>> = Object.freeze({}),
): ForgeSectionCell {
  return Object.freeze({
    id: section.id,
    label: section.label,
    kind: section.kind,
    parent: section.parent || null,
    bootSafe: section.bootSafe === true,
    owns: Object.freeze([...(section.owns || [])]),
    permissions: Object.freeze([...(section.permissions || [])]),
    commands: Object.freeze([...(section.commands || [])]),
    lifecycle,
    active,
    projection,
  });
}

function projectionForSection(section: ForgeSectionDefinition, state: ForgeShellState): Readonly<Record<string, unknown>> {
  const projection: Record<string, unknown> = {
    mode: state.mode,
    activeSection: state.activeSection,
    phase: state.phase,
  };
  for (const owner of section.owns || []) {
    if (owner === "canvas") projection.canvas = state.canvas;
    else if (owner === "chatbar") projection.chatbar = state.chatbar;
    else if (owner === "rightPanel") projection.rightPanel = state.rightPanel;
    else if (owner === "jobs") projection.jobs = state.jobs;
    else if (owner === "hardware") projection.hardware = state.hardware;
  }
  return Object.freeze(projection);
}
