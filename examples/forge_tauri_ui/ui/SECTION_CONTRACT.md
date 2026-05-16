# Forge UI Section Contract

UI sections must register through the shared registry/bridge instead of creating isolated control paths. Source is TypeScript; browser JavaScript is generated into `ui/dist/**/*.js`.

## Files

- `ui/src/shell/legacy-section-registry.ts` owns section metadata and emits `dist/forge-section-registry.js`.
- `ui/src/shell/tauri-bridge.ts` owns safe Tauri invocation/listen helpers and emits `dist/forge-tauri-bridge.js`.
- `ui/src/shell/intent-surface.ts` owns the intent/trace/distillation UI contract and is bundled through `dist/forge-shell-runtime.js`.
- `ui/src/shell/click-router.ts` owns shared shell click/shortcut routing.
- `ui/src/shell/boot.ts` owns startup wiring and emits `dist/forge-boot.js`.
- `ui/src/shell/surface.ts`, `ui/src/sections/trading/surface.ts` and `ui/src/sections/banger/surface.ts` are transitional TS surfaces being drained.
- `ui/src/sections/real-estate/*runtime.ts` owns real-estate context, onboarding, language, mode lifecycle and floating panels.
- `SECTION_OWNERSHIP.json` names section owners and boundaries.

## Current Sections

shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading, banger.

## Rules

- A section may own its view state, but shared actions go through the bridge.
- Do not duplicate provider, trading, memory or compute state in multiple section-specific stores.
- Add a section only when it removes complexity from an existing file or exposes a genuinely new product surface.
- Sensitive native commands must declare ownership in `SECTION_OWNERSHIP.json` and require the shared bridge.
- Intent, trace or distilled-clone UI must be TS-owned shell projection state; do not add hand-written JS panels, section-local MCP clients or direct `window.__TAURI__` calls.
- Do not add hand-written `.js` outside generated `ui/dist/**/*.js`; `ui/src/MANUAL_JS_LOCK.md` is the guardrail.
- Keep visible UI text practical; do not add explanatory marketing copy inside tool surfaces.
