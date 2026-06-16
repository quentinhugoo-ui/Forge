# InGen Agent Brief

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

This is the single source of truth for coding agents. Keep it short and current. `CLAUDE.md` imports this file; do not duplicate the doctrine there.

## Mission

InGen is a compact local agent OS. Forge is its content-addressed compute language, formerly KASM in code:

```text
LLM CLI -> BrainCommand/Intent -> Godel verification -> Forge bytecode/Monster compute -> proof/artifact -> Rust services -> Electron action
```

Prefer shorter circuits. Remove obsolete nodes before adding new ones.

## Reasoning Boundary

The LLM owns the reasoning monopoly. The application must never pretend to
reason, infer intent, choose strategy, or replace the LLM's judgment with hidden
application-side intelligence. The app has no brain.

The application may only execute narrow contracts, route explicit LLM decisions,
enforce safety gates, validate schemas, collect observations, verify results and
return compact proof artifacts to the LLM. If a decision requires reasoning, the
runtime must hand the evidence back to the LLM instead of deciding silently.

All CodeActs from every Brain, including General, Science, Coding and named
specialized Brains, are part of the loop-stream action vocabulary, not
decorative prompt text. The runtime may expose, parse, render and execute these
LLM-chosen CodeAct boundaries, but it must not select any Brain CodeAct on the
LLM's behalf.

## Hard Rules

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Preserve unrelated user changes.
- Never run recursive delete/move without resolved absolute path guards.
- Do not commit caches, build outputs, datasets, secrets, `.vs/`, `target/`, `.forge-store/`, `.forge-data/` or `.codex-targets/`.
- Use `rg` for search and compact command outputs for large files.
- For large/repeated/numerical/document-heavy work, use InGen direct-command discipline first: keep raw data on disk, exchange compact manifests, hashes and artifacts.
- Commit and push meaningful work to GitHub before risky cleanup.
- `master` is the live code line used by the desktop app. Work directly on `master` by default, commit meaningful changes and push `origin/master` frequently. Use a branch or worktree only for risky, large, parallel or explicitly isolated work.

## Coding Doctrine

This doctrine is mandatory. For any non-trivial answer, plan, code change, refactor, architecture decision or cleanup, agents must use it as the main reasoning frame.

Before writing code beyond a purely mechanical one-line edit, research deeply when internet is available: current official docs, papers, release notes, serious repos and market direction. Treat the current state as the floor, then build as if InGen must live one step ahead while staying verifiable locally.

Frontier code must be ambitious and disciplined:

- prefer designs that collapse steps, remove actors or turn repeated decisions into verified reusable programs,
- prototype experimental ideas behind narrow interfaces, deterministic tests, proof hashes, benchmarks or feature gates,
- promote an experiment only when it beats the current path on clarity, speed, capability or verifiability,
- delete failed experiments quickly instead of documenting around them,
- never trade data safety, user approval, reproducibility or semantic verification for novelty.

Doctrine checklist:

1. Name the wall being pushed: latency, memory, context size, proof quality, UI branching, sandbox reach, agent autonomy or developer experience.
2. State the frontier hypothesis in one compact sentence.
3. Use current sources as the floor when the work is not mechanical.
4. Keep experiments behind a narrow interface, feature gate, deterministic test, proof hash, benchmark or compact manifest.
5. Promote only if the new path beats the current one.
6. Delete failed experiments quickly.
7. If the task is only analysis, still apply this checklist to the recommendation.

Every code change must reduce architectural drag:

- delete obsolete code before adding new actors,
- remove useless middlemen and duplicate nodes,
- fuse functions when separation no longer buys clarity or safety,
- shorten the path from intent to result,
- add the fewest new files, branches, abstractions and runtime steps possible,
- prefer one proven circuit over parallel pipelines.

## Work Style

1. Read the local shape with `rg`, `git status`, small file previews and targeted code search.
2. Prefer the smallest file set that solves the request.
3. Keep docs smaller after the change whenever possible.
4. Use content-addressed references, hashes and proof summaries for large artifacts.
5. Verify with the narrowest meaningful command, then broaden if risk requires it.
6. If docs and code disagree, inspect code and update docs.
7. Live frontend objectives belong in `MIGRATION_FRONT.md`; runtime/language/Monster objectives belong in `FORGE_NATIVE_BYTECODE.md`.
8. For every Monster/Forge math round, run a real store-backed `/newcompute_` app-runtime stress test through `forge_brain_run_actcode`, Monster execution, typed buffers, proof/differential status and compute-library reuse. Unit tests can support this gate, but they do not replace it.

## Current Architecture

- Core library: `src/lib.rs`
- Brain/memory: `src/brain.rs`
- Godel machine: `src/godel.rs`
- Forge language/runtime: `src/kasm.rs` is the single source of truth for Forge source, bytecode, tensor runtime, FBC v0 and the embedded dialect reference.
- Monster compute: `src/monster.rs`; keep it focused on verified Forge math through `/newcompute_` and native-ready artifact preparation for Banger / future renderer lanes.
- Product frontend: `examples/ingen_electron_shell/**`; Electron/React shell, strict preload IPC, Rust backend projection, Banger native surface policy and Rust-owned WebExplorer host.
- Native/shared services: `examples/ingen_native_services/**`; direct Rust adapters live here.
- Deleted legacy app trees must not be recreated.
- Brain runtime pointers and Banger policy: native Rust service adapters only.
- Canonical runtime/language/Monster architecture and live objectives: `FORGE_NATIVE_BYTECODE.md`.
- Direct agent CLI/runtime: native Rust direct-command surface.
- Collection OS kernel: shared Rust service surface.
- InGen Harvester: the general collection execution layer built on Collection OS.
- Real Estate Harvester: the first vertical sector pack/adapter, not the general collection engine.
- WebExplorer and any external web content are contained peripherals only, never the global app shell.
- CodeAct action layer: the LLM acts by emitting **CodeAct commands** and filling typed templates, not JSON tool-calls and not user-facing "programs". Primary path is `forge_agent`.
- CodeAct compute (`/newcompute_`, `/selectcompute_`, `/compute_<name>_`, `/newobject_`) enters Monster. UI-directive blocks (`/web_`, `FORGE_PLAN_JSON`, `FORGE_QUESTIONNAIRE_JSON`, `FORGE_SESSION_TITLE_JSON`, Banger JSON aliases) render through Electron IPC projections and do not enter the compute executor.
- Monster compute library: SQLite under the Forge store at `brain/computes/compute_library.sqlite`; exact fragment/proof indexes are the authority for compute reuse.

Core files:

- `src/brain.rs`
- `src/godel.rs`
- `src/apply.rs`
- `src/monster.rs`
- `examples/ingen_native_services/src/banger_native_engine.rs`
- `examples/ingen_electron_shell/src/main/main.ts`
- `examples/ingen_electron_shell/src/renderer/App.tsx`
- `examples/ingen_electron_shell/src/preload/preload.ts`

## Frontend Discipline

`MIGRATION_FRONT.md` is the source of truth for frontend work.

The product shell is Electron/React connected by strict IPC to the Rust Forge / Monster / Brain core. Banger heavy 3D is a native child surface contract. WebExplorer is a proprietary Rust-owned native WebView host. Browser content remains contained and must never become the whole application shell.

Typography is part of the design contract: normal UI text uses Geist, and technical/proof/code-like text uses Geist Mono. Fonts and icons live under the Electron shell assets.

Animations are product runtime concerns. Use CSS transitions for shell/sidebar/chat motion, and keep heavy 3D/shader motion in Banger or Rust-owned native surfaces. Respect reduced motion where relevant.

Use these coordination files before adding new frontend actors:

- `examples/ingen_electron_shell/src/renderer/App.tsx`
- `examples/ingen_electron_shell/src/renderer/styles.css`
- `examples/ingen_electron_shell/src/main/main.ts`
- `examples/ingen_electron_shell/src/preload/preload.ts`
- `examples/ingen_electron_shell/src/shared/ipc-contract.ts`
- `examples/ingen_electron_shell/contract/src/main.rs`
- `examples/ingen_electron_shell/scripts/final-cutover-audit.mjs`

## Fast Master Workflow

`master` is the live integration line and the desktop app reads from the main Forge worktree. The default path is simple: agents edit `master` directly, run targeted checks, commit intentional changes and push `origin/master`.

Do not create task worktrees by default. Use a branch or worktree only when the task is risky, large, parallel, experimental or explicitly marked as isolated by the user.

For every task:

1. Start with `git status --short --branch` and name exact dirty files.
2. Preserve user changes. If dirty files overlap the task, inspect them and continue carefully or ask.
3. Edit the smallest file set that solves the request.
4. Run the narrowest meaningful checks. Run `npm.cmd run typecheck` when TypeScript changed.
5. Commit meaningful successful work on `master`.
6. Push `origin/master` after meaningful progress so GitHub remains the backup.

Never leave successful source changes uncommitted without naming the exact path and reason. Never discard user changes. If a branch/worktree was explicitly used, it is temporary: merge successful work back to `master`, push `master`, then remove the temporary branch/worktree.

## Partial Commit Rule

Agents must commit only the lines they intentionally changed.

Before staging, inspect `git diff`. If a touched file contains unrelated edits, use partial staging with `git add -p` or an exact index patch. Never run `git add <file>` on a file that contains mixed changes from another session.

If a hunk mixes the agent's work with unrelated lines, split it with `git add -p` or stage an exact patch. If it cannot be separated cleanly, stop and report the overlapping file and lines instead of committing someone else's work.

Avoid whole-file formatting on files that may contain concurrent edits unless the task explicitly owns the whole file.

## Useful Checks

```powershell
.\scripts\forge-cargo.ps1 check --lib --tests
.\scripts\forge-cargo.ps1 test brain --lib
.\scripts\forge-cargo.ps1 check --manifest-path examples\ingen_native_services\Cargo.toml
cd examples\ingen_electron_shell
npm.cmd test
npm.cmd run build
```

## Dependency / Toolchain Maintenance

Nothing updates by itself; updates are always a deliberate, tested decision. Minor/patch bumps are safe via `cargo update`; major/breaking bumps are manual and require release notes first.

```powershell
rustup update
cargo update
cargo install cargo-outdated cargo-audit
cargo outdated -w
cargo audit
```

For Electron dependencies, pin deliberately in `examples/ingen_electron_shell/package.json`, run `npm.cmd install`, `npm.cmd test` and `npm.cmd run build`.

## Git Safety

```powershell
git status --short --branch
git diff --stat
git add <source-doc-files>
git commit -m "Short useful message"
git push
```

The GitHub `master` branch is a clean snapshot history. The older local history with large files is kept locally as `archive/master-large-history-before-github-20260514`.

## Product Strategy

### Ce que Forge ne vend pas

La couche LLM et l'orchestration agentique generale ne sont pas monetisables. Le marche a decide que c'est gratuit. Concurrencer Claude Code ou Cursor sur le coding generaliste n'a pas de sens economique.

Ne pas construire de features generiques pour rivaliser avec des outils generiques. Chaque heure de dev sur une feature generique est une heure de moins sur la profondeur qui differencie InGen.

### Les deux vraies valeurs d'InGen

**Valeur 1 - Moteurs de compute (Monster)**

Monster permet a un LLM de lancer des calculs lourds et complexes dans des domaines professionnels varies sans depenser des millions de tokens. Le LLM emet une commande CodeAct comme `/newcompute_`, remplit le template type, puis InGen compile ce contrat en Forge interne et Monster l'execute localement sur le GPU de l'utilisateur. Le LLM recoit un artefact verifiable avec proof hash, pas une reponse generee.

**Valeur 2 - Moteur 3D / ingenierie computationnelle (Banger)**

Pouvoir utiliser un moteur de creation 3D et d'ingenierie computationnelle de niveau Blender/Unreal juste en discutant avec un LLM. L'utilisateur decrit, le LLM pilote Banger, le moteur execute.

### Deux produits distincts

**1. InGen - OS agentique gratuit (Freemium)**

Gratuit comme OpenClaw, Hermes, Unreal Engine ou un navigateur web. Monster et Banger tournent sur le GPU de l'utilisateur. La couche LLM reste a la charge de l'utilisateur.

Toujours gratuit : web search, coding, Monster compute local, Banger 3D local, Brain memory.

Declenche l'abonnement : delegation de compute vers RunPod ou autre GPU externe.

**2. Verticaux - applications clones full-orientees (B2B / SaaS)**

Applications separees, rearchitecturées autour d'un domaine cible. L'OS kernel est partage, mais toute la surface produit est reconstruite pour le domaine.

- **Forge Trading** : broker OANDA, backtests, alertes, analyse marche.
- **Forge Immo** : scoring zones, donnees DVF, alertes marche immobilier, analyse patrimoniale.

## Documentation Rule

Docs are context for agents, not an archive. Historical detail belongs in Git history. If a doc becomes noisy, compress it. Live frontend objectives belong in `MIGRATION_FRONT.md`; runtime/language/Monster work belongs in `FORGE_NATIVE_BYTECODE.md`.

## North Star

1. Deliver a complete, mature InGen app.
2. Turn InGen into an agentic OS that replaces Windows.
3. Move the mature agentic OS onto local Grace-Blackwell silicon.
