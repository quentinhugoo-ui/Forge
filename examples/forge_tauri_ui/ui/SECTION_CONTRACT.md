# Forge UI Section Contract

UI sections must register through the shared registry/bridge instead of creating isolated control paths.

## Files

- `forge-section-registry.js` owns section metadata.
- `forge-tauri-bridge.js` owns safe Tauri invocation helpers.
- `forge-boot.js` owns startup wiring.
- `SECTION_OWNERSHIP.json` names section owners and boundaries.

## Current Sections

shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading, banger.

## Rules

- A section may own its view state, but shared actions go through the bridge.
- Do not duplicate provider, trading, memory or compute state in multiple section-specific stores.
- Add a section only when it removes complexity from an existing file or exposes a genuinely new product surface.
- Sensitive native commands must declare ownership in `SECTION_OWNERSHIP.json` and require the shared bridge.
- Keep visible UI text practical; do not add explanatory marketing copy inside tool surfaces.
