# Forge Agent Brief

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

This is the single source of truth for coding agents. Keep it short and current. `CLAUDE.md` imports this file; do not duplicate the doctrine there.

## Mission

Forge is a compact local agent OS:

```text
LLM intent -> brain/memory -> Godel verification -> KASM/Monster compute -> proof/artifact -> Tauri/MCP action
```

Prefer shorter circuits. Remove obsolete nodes before adding new ones.

## Hard Rules

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Preserve unrelated user changes.
- Never run recursive delete/move without resolved absolute path guards.
- Do not commit caches, build outputs, datasets, secrets, `.vs/`, `target/`, `.forge-store/`, `.forge-data/` or `lab_findings.jsonl`.
- Use `rg` for search and compact command outputs for large files.
- For large/repeated/numerical/document-heavy work, use Forge/MCP discipline: keep raw data on disk, exchange compact manifests, hashes and artifacts.
- Commit and push meaningful work to GitHub before risky cleanup.

## Coding Doctrine

Before writing code beyond a purely mechanical one-line edit, research the current state of the art when internet access is available: recent official docs, papers, serious repos, release notes and market direction. Then build as if Forge must live one step ahead of that state of the art, while staying verifiable locally.

Every code change must reduce architectural drag:

- delete obsolete code before adding new actors,
- remove useless middlemen and duplicate nodes,
- fuse functions when separation no longer buys clarity or safety,
- shorten the path from intent to result,
- add the fewest new files, branches, abstractions and runtime steps possible,
- prefer one proven circuit over parallel pipelines.

## Work Style

1. Read the local shape first with `rg`, `git status`, small file previews and targeted code search.
2. Prefer the smallest file set that solves the request.
3. Keep docs smaller after the change whenever possible.
4. Use content-addressed references, hashes and proof summaries for large artifacts.
5. Verify with the narrowest meaningful command, then broaden if risk requires it.
6. If docs and code disagree, inspect code and update docs.
7. If you establish a list of objectives, put the live list in `ROADMAP.md` and remove objectives when they are done.

## Current Architecture

- Core library: `src/lib.rs`
- Brain/memory: `src/brain.rs`
- Godel machine: `src/godel/**`
- KASM runtime: `src/kasm/**`
- Monster compute: `src/monster/**`
- Tauri backend: `examples/forge_tauri_ui/src-tauri/src/**`
- Tauri UI: `examples/forge_tauri_ui/ui/**`
- Runtime architecture: `FORGE_RUNTIME_ARCHITECTURE.md`
- UI sections: shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading and banger.
- Native section bridge: WebExplorer and Bloomberg live actions must pass through `forge-tauri-bridge.js` and `SECTION_OWNERSHIP.json`.
- MCP server: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`
- MCP visible tools: about, capabilities, create, program_compile_validate_route, geonode, run, jobs, sessions, documents, mapping, mapping_metrics, mapping_model, visual_program, mapping_analysis, profile, atlas, brain_recall, brain_commit, brain_compare, brain_sleep, brain_explain, update_session, read, logs, cancel.

## Brain, Memory And Godel

The brain/memory layer must stay evidence-aware:

- semantic notes need scope, layer, trust score and evidence/proof hash when possible,
- unverified LLM memory stays marked as unverified,
- newer facts supersede older facts by stable keys,
- Godel substitution must pass strict semantic verification before use,
- no external model/backend becomes trusted just because it produced plausible text.

Core files:

- `src/brain.rs`
- `src/godel/**`
- `src/apply.rs`
- `src/monster/exec.rs`
- `src/monster/dispatch.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_agent_tools.rs`
- `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`

## UI Discipline

The Tauri UI is already large. Do not add a new UI path if an existing section/registry/bridge can carry the feature.

Use these coordination files before adding new actors:

- `examples/forge_tauri_ui/ui/forge-section-registry.js`
- `examples/forge_tauri_ui/ui/forge-tauri-bridge.js`
- `examples/forge_tauri_ui/ui/forge-boot.js`
- `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`
- `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`

Current heavy UI files are `app.js`, `trading.js`, `styles.css` and Tauri `main.rs`. Shrink them only when extraction removes duplication or a real ownership conflict.

## Useful Checks

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
```

## Git Safety

```powershell
git status --short --branch
git diff --stat
git add <source-doc-files>
git commit -m "Short useful message"
git push
```

The GitHub `master` branch is a clean snapshot history. The older local history with large files is kept locally as `archive/master-large-history-before-github-20260514`.

## Documentation Rule

Docs are context for agents, not an archive. Historical detail belongs in Git history. If a doc becomes noisy, compress it. Live objectives belong in `ROADMAP.md`, not in private side lists.
