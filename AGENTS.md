# Forge Agent Brief

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

This is the single source of truth for coding agents. Keep it short and current. `CLAUDE.md` imports this file; do not duplicate the doctrine there.

## Mission

Forge is a compact local agent OS:

```text
LLM CLI -> ForgeSlash/Intent -> Godel verification -> KASM/FBC/Monster compute -> proof/artifact -> Tauri action
```

Prefer shorter circuits. Remove obsolete nodes before adding new ones.

## Hard Rules

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Preserve unrelated user changes.
- Never run recursive delete/move without resolved absolute path guards.
- Do not commit caches, build outputs, datasets, secrets, `.vs/`, `target/`, `.forge-store/`, `.forge-data/` or `lab_findings.jsonl`.
- Use `rg` for search and compact command outputs for large files.
- For large/repeated/numerical/document-heavy work, use Forge direct-command discipline first: keep raw data on disk, exchange compact manifests, hashes and artifacts. MCP is only the external compatibility bridge.
- Commit and push meaningful work to GitHub before risky cleanup.

## Coding Doctrine

This doctrine is mandatory, not aspirational. For any non-trivial answer, plan, code change, refactor, architecture decision or cleanup, agents must use it as the main reasoning frame before choosing an implementation path.

Before writing code beyond a purely mechanical one-line edit, research deeply the current state of the art when internet access is available: recent official docs, papers, serious repos, release notes and market direction. Treat that current state as the average floor, not the target. Then build as if Forge must live one step ahead of it: surf the incoming wave, not the already-visible one, while staying verifiable locally.

Code at the frontier, not at the average. For meaningful work, identify the wall being broken: latency, memory, context size, proof quality, UI branching, sandbox reach, agent autonomy or developer experience. Treat established practice as the floor, then push one wall with a small hypothesis, a local verifier, and a rollback path.

Frontier code must be ambitious and disciplined:

- prefer designs that collapse steps, remove actors or turn repeated decisions into verified reusable programs,
- prototype experimental ideas behind narrow interfaces, deterministic tests, proof hashes, benchmarks or feature gates,
- promote an experiment only when it beats the current path on clarity, speed, capability or verifiability,
- delete failed experiments quickly instead of documenting around them,
- never trade data safety, user approval, reproducibility or semantic verification for novelty.

Doctrine integration checklist:

1. Name the wall being pushed before proposing or editing: latency, memory, context size, proof quality, UI branching, sandbox reach, agent autonomy or developer experience.
2. State the frontier hypothesis in one compact sentence: what shorter verified circuit should replace the current path.
3. Search current official docs, papers, serious repos, release notes or market direction when internet is available and the work is not mechanical; use them as the floor, then identify the next wave Forge should meet before it becomes mainstream.
4. Keep the experiment behind a narrow interface, feature gate, deterministic test, proof hash, benchmark or compact manifest.
5. Promote only if the new path beats the current one on clarity, speed, capability or verifiability.
6. Delete failed experiments quickly; do not preserve them by adding explanatory docs around dead code.
7. If the task is only analysis, still apply this checklist to the recommendation so the answer does not collapse into average refactoring advice.

Every code change must reduce architectural drag:

- delete obsolete code before adding new actors,
- remove useless middlemen and duplicate nodes,
- fuse functions when separation no longer buys clarity or safety,
- shorten the path from intent to result,
- add the fewest new files, branches, abstractions and runtime steps possible,
- prefer one proven circuit over parallel pipelines.

## Work Style

1. For non-trivial work, apply the Coding Doctrine checklist first: wall, frontier hypothesis, verifier and rollback path.
2. Read the local shape with `rg`, `git status`, small file previews and targeted code search.
3. Prefer the smallest file set that solves the request.
4. Keep docs smaller after the change whenever possible.
5. Use content-addressed references, hashes and proof summaries for large artifacts.
6. Verify with the narrowest meaningful command, then broaden if risk requires it.
7. If docs and code disagree, inspect code and update docs.
8. If you establish a list of objectives, put the live list in `ROADMAP.md` and remove objectives when they are done.

## Current Architecture

- Core library: `src/lib.rs`
- Brain/memory: `src/brain.rs`
- Godel machine: `src/godel.rs`
- KASM runtime: `src/kasm.rs` plus dialect spec `src/kasm.td`
- Monster compute: `src/monster.rs`
- Tauri backend: `examples/forge_tauri_ui/src-tauri/src/**`
- Tauri UI: `examples/forge_tauri_ui/ui/**`
- Runtime architecture: `FORGE_RUNTIME_ARCHITECTURE.md`
- Native bytecode direction: `FORGE_NATIVE_BYTECODE.md`
- Direct agent CLI: `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`
- Direct agent runtime: `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`
- UI sections: shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading and banger.
- UI source is TypeScript under `examples/forge_tauri_ui/ui/src/**`; browser JavaScript under `ui/dist/**/*.js` is generated only.
- Native section bridge: WebExplorer and Bloomberg live actions must pass through `ui/src/shell/tauri-bridge.ts` / generated `ui/dist/forge-tauri-bridge.js` and `SECTION_OWNERSHIP.json`.
- MCP server: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`
- MCP surface contract: MCP is transport for external LLM clients, not the Forge OS action language. The internal primary path is `forge_agent` direct CLI plus ForgeSlash/KASM/FBC. The external compatibility surface is `forge.search`, `forge.execute`, `forge.read_projection`, `forge.cancel`; the broad MCP catalog remains callable as legacy/internal compatibility via `FORGE_MCP_SURFACE=broad` or `FORGE_MCP_LEGACY_SURFACE=1`.

## Brain, Memory And Godel

The brain/memory layer must stay evidence-aware:

- semantic notes need scope, layer, trust score and evidence/proof hash when possible,
- unverified LLM memory stays marked as unverified,
- newer facts supersede older facts by stable keys,
- Godel substitution must pass strict semantic verification before use,
- no external model/backend becomes trusted just because it produced plausible text.

Core files:

- `src/brain.rs`
- `src/godel.rs`
- `src/apply.rs`
- `src/monster.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_agent_tools.rs`
- `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`

## UI Discipline

The Tauri UI is already large. Do not add a new UI path if an existing section/registry/bridge can carry the feature.

Use these coordination files before adding new actors:

- `examples/forge_tauri_ui/ui/src/shell/legacy-section-registry.ts`
- `examples/forge_tauri_ui/ui/src/shell/tauri-bridge.ts`
- `examples/forge_tauri_ui/ui/src/shell/boot.ts`
- `examples/forge_tauri_ui/ui/src/shell/click-router.ts`
- `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`
- `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`

No hand-written JavaScript is allowed outside generated `ui/dist/**/*.js`; see `examples/forge_tauri_ui/ui/src/MANUAL_JS_LOCK.md`.

Current heavy UI source files are `ui/src/shell/surface.ts`, `ui/src/sections/trading/surface.ts`, `ui/src/sections/banger/surface.ts`, `ui/styles.css` and Tauri `main.rs`. Shrink them only when extraction removes duplication or a real ownership conflict.

Real-estate shell logic is split into section runtimes:

- `ui/src/sections/real-estate/runtime-context.ts`
- `ui/src/sections/real-estate/onboarding-runtime.ts`
- `ui/src/sections/real-estate/language-runtime.ts`
- `ui/src/sections/real-estate/mode-runtime.ts`
- `ui/src/sections/real-estate/panel-runtime.ts`

## Useful Checks

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
node examples\forge_tauri_ui\scripts\forge-surface-manifest.mjs --check
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
node examples\forge_tauri_ui\scripts\forge-ui-section-audit.mjs
node examples\forge_tauri_ui\scripts\forge-tauri-bus-audit.mjs --strict
cd examples\forge_tauri_ui; npm.cmd run audit:js-debt
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
