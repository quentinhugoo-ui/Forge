# InGen Agent Brief

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

This is the single source of truth for coding agents. Keep it short and current. `CLAUDE.md` imports this file; do not duplicate the doctrine there.

## Mission

InGen is a compact local agent OS. Forge is its content-addressed compute language, formerly KASM in code :

```text
LLM CLI -> BrainCommand/Intent -> Godel verification -> Forge bytecode/Monster compute -> proof/artifact -> Tauri action
```

Prefer shorter circuits. Remove obsolete nodes before adding new ones.


## Hard Rules

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Preserve unrelated user changes.
- Never run recursive delete/move without resolved absolute path guards.
- Do not commit caches, build outputs, datasets, secrets, `.vs/`, `target/`, `.forge-store/` or `.forge-data/`.
- Use `rg` for search and compact command outputs for large files.
- For large/repeated/numerical/document-heavy work, use InGen direct-command discipline first: keep raw data on disk, exchange compact manifests, hashes and artifacts.
- Commit and push meaningful work to GitHub before risky cleanup.

## Coding Doctrine

This doctrine is mandatory, not aspirational. For any non-trivial answer, plan, code change, refactor, architecture decision or cleanup, agents must use it as the main reasoning frame before choosing an implementation path.

Before writing code beyond a purely mechanical one-line edit, research deeply (minimum 5 deep search on web, clone serious GitHub repo to explore last innovations, the current state of the art when internet access is available: recent official docs, papers, release notes and market direction. Treat that current state as the average floor, not the target. Then build as if InGen must live one step ahead of it: surf the incoming wave, not the already-visible one, while staying verifiable locally.

Code at the frontier of the code know at the today date, not at the average. For meaningful work, treat established coding practice as not good enough, make hypothesis of the frontier you can break, if you succeed to easily obviously you didnét break any frontier, you need to fail in your itérations, it's normal in the process of writing innovative code, but Don't fake it, Don't pretend to fail while you're writing established code.

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
- Forge language/runtime: `src/kasm.rs` is the single source of truth for Forge source, bytecode, tensor runtime, FBC v0 and the embedded dialect reference. Current code still uses the old KASM name. DeltaKASM auto-promotes matching `VecI64 -> VSumI64 -> I64` calls through normal `kasm::execute`.
- Monster compute: `src/monster.rs`; keep it focused on two goals only:
  verified Forge math through `/newcompute_`, and background native-ready
  artifact preparation for Banger / the future native Rust renderer / future
  Google-Web DOM/RAM cartography. Scheduling callers should enter through
  `MonsterNode::prepare_forge_source`, which returns one manifest containing
  the route, cache-miss fragments, native-ready outputs and proof hash.
- Tauri backend: `examples/forge_tauri_ui/src-tauri/src/**`
- Tauri UI: `examples/forge_tauri_ui/ui/**`
- Brain runtime pointers and Banger policy: `examples/forge_tauri_ui/src-tauri/src/forge_brain_runtime.rs`
- Canonical runtime/language/Monster architecture and live objectives: `FORGE_NATIVE_BYTECODE.md`.
- Direct agent CLI: `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`
- Direct agent runtime: `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`
- Collection OS kernel: `examples/forge_tauri_ui/src-tauri/src/collection_os.rs`
- InGen Harvester: the general collection execution layer built on Collection OS.
- Real Estate Harvester: the first vertical sector pack/adapter, not the general collection engine.
- UI sections: shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading and banger.
- UI source is TypeScript under `examples/forge_tauri_ui/ui/src/**`; browser JavaScript under `ui/dist/**/*.js` is generated only.
- Native section bridge: WebExplorer and Bloomberg live actions must pass through `ui/src/shell/tauri-bridge.ts` / generated `ui/dist/forge-tauri-bridge.js` and `SECTION_OWNERSHIP.json`.
- CodeAct action layer: the LLM acts by emitting executable programs, not JSON tool-calls. Primary path is `forge_agent` (BrainCommand verbs `recall/plan/create/run/project/replay/commit/explain`). The old MCP `forge.search`/`forge.execute` external server was removed; discovery is `forge_agent about` plus the persisted projection / skill-candidate / verified-program indexes. Details in `FORGE_NATIVE_BYTECODE.md`.
- CodeAct splits into two families. Family A (compute, real Monster runs through `forge_brain_run_actcode`, `requiresActiveSection:false`): `/newcompute_`, `/selectcompute_`, `/compute_<name>_` work in **all** sections; `/newcompute_` is the access command to Monster's universal compute template, not a Banger-only template; `/selectcompute_` is the universal selector for already-created Monster computes; `/newobject_` is Banger-only SDF. Family B (UI-directive, no compute — renders panels/events) works in **all** sections: `/web_` fires a native LLM web-research event; `FORGE_PLAN_JSON` renders/updates the right-panel plan; `FORGE_QUESTIONNAIRE_JSON` renders a question panel above the chat bar; `FORGE_SESSION_TITLE_JSON` renames the current session in the left panel and above the canvas. Banger keeps special aliases for 3D/SDF: `FORGE_BANGER_PLAN_JSON`, `FORGE_BANGER_QUESTIONNAIRE_JSON`, and especially `FORGE_BANGER_MATERIAL_RESEARCH_JSON`, which is a Banger-only computational-engineering material/component list for the right panel. The compute executor rejects Family-B prefixes by design. Brain keeps only stable pointers, not the templates. Details in `FORGE_NATIVE_BYTECODE.md`.
- Monster compute library: SQLite under the Forge store at `brain/computes/compute_library.sqlite`; exact fragment/proof indexes are the authority for compute reuse.

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
- `examples/forge_tauri_ui/src-tauri/src/forge_brain_runtime.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_agent_tools.rs`
- `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`
- `examples/forge_tauri_ui/src-tauri/src/collection_os.rs`

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
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_agent
node examples\forge_tauri_ui\scripts\forge-surface-manifest.mjs --check
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
node examples\forge_tauri_ui\scripts\forge-ui-section-audit.mjs
node examples\forge_tauri_ui\scripts\forge-tauri-bus-audit.mjs --strict
cd examples\forge_tauri_ui; npm.cmd run audit:js-debt
```

## Dependency / Toolchain Maintenance

Nothing updates by itself; updates are always a deliberate, tested decision (pin in `Cargo.lock`, bump on purpose, verify, keep a rollback). Minor/patch bumps are safe via `cargo update`; major/breaking bumps (e.g. Dioxus `0.7 -> 0.8`) are manual, read the release notes first.

- Reminder set **2026-06-02**: later, propose a full refresh of all Forge languages/toolchains (Rust, Dioxus/Leptos + WASM stack, Node/TS for the legacy UI) once the front migration is underway. Do not auto-bump majors; surface a checklist, let the user validate.
- Decision set **2026-06-02**: drop Tauri for native Dioxus desktop (wry/tao, no IPC — see `MIGRATION_FRONT.md` Étape 9 bis) **once Dioxus desktop is mature enough**. The IPC win only lands in this native path; keeping Tauri is pointless once it does. Gate on these maturity criteria before proposing the switch:
  - Dioxus **0.8+** native APIs stable (camera/geo/storage/OAuth) and a stable "1.0" core subset;
  - `dx bundle` packaging at parity (installers, icons, code signing) + a viable auto-updater path;
  - we can **re-secure WebExplorer** ourselves (webview isolation/CSP/navigation hooks for external web content) — this is the decisive blocker for Forge, not packaging.
  - How to check: track Dioxus releases (`https://github.com/DioxusLabs/dioxus/releases`), `cargo outdated -w`, and the 0.8 roadmap; re-run the `MIGRATION_FRONT.md` Étape 1 POC measures on the native target. Propose, never auto-switch.

```powershell
rustup update                         # update the Rust toolchain itself
cargo update                          # safe minor/patch bumps within Cargo.toml ranges
cargo install cargo-outdated cargo-audit   # one-time: install the veille tools
cargo outdated -w                     # list deps behind latest (workspace)
cargo audit                           # security advisories (the `npm audit` of Rust)
cd examples\forge_tauri_ui; npm.cmd outdated; npm.cmd audit   # legacy TS UI side
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

## North Star (long-horizon, gated)

These are direction, not the active checklist (that stays in `ROADMAP.md`). Each phase is a hard gate: do not open phase N+1 before phase N has shipped a mature, verified result. Never push two of these walls at once.

1. **Deliver a complete, mature InGen app.** Prerequisite to everything below. The agentic loop (perception -> intent -> Godel verification -> Forge runtime -> action) must be stable, native and verifiable before any OS or hardware ambition.
2. **Turn InGen into an agentic OS that replaces Windows.** End state: a native (no webview, wgpu/Dioxus direct) mono-app appliance on a minimal Linux kernel — the machine boots into InGen. Reuses the kernel's drivers; InGen is the whole userland. Hard prerequisite: the native front migration (`MIGRATION_FRONT.md`) must be finished first.
3. **Move the mature agentic OS onto local Grace-Blackwell silicon (DGX Spark now / RTX Spark fall 2026).** Goal: replace the cloud LLM dependency with a local brain — run, fine-tune and distill the agentic model in-house on 128 GB unified memory, ARM64. DGX Spark (Linux, NVIDIA-supported drivers, already shipping) is the dev target; RTX Spark (Windows-on-ARM, consumer) is the distribution horizon. Requires an ARM64 InGen build.

**Open question (decisive, unresolved):** can a local model match a frontier cloud model (GPT-5.5 / Claude Opus 4.8)? Honest stance: no raw-scale parity on one box near-term — frontier pre-training compute is the wall. But that is the wrong target. InGen's bet is that **domain specialization + Godel verification + reusable verified programs** let a smaller local model match or beat the frontier *on InGen's agentic tasks*, because the verifier catches errors and content-addressed programs offload reasoning. Treat raw-parity as a non-goal; treat verified task parity as the real, testable objective.
