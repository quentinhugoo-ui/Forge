# Forge Roadmap

This is only the live roadmap. Historical plans belong in Git history, not in the active agent context.

Rule: if an agent establishes a list of objectives, it writes the live objectives here, then removes each objective when it is done. Do not keep parallel TODO lists in side documents.

## Current North Star

Forge should become a compact local agent OS:

```text
brain/memory + Godel verification + KASM/Monster compute + Tauri/MCP actions
```

The winning architecture is not the one with the most subsystems. It is the one where an agent can move from intent to verified action with the fewest nodes.

## Current Priorities

1. Cut code size through verified compression.
   - Target: shrink tracked source/docs from roughly 298k lines toward 60k without losing public behavior.
   - Structural target: reduce visible file/folder count by moving runtime state out of the repo and fusing module folders when Rust paths stay stable.
   - Wall: context size and UI/backend branching, not formatting or file layout.
   - Hypothesis: repeated decisions should become registries, schemas, scenarios or Forge/KASM programs with proof manifests.
   - Verifier: command/section/tool surface manifest, structure metrics, smoke checks, hashes and before/after line/branch metrics.
   - First-pass delivery sequence completed: reduction proof dashboard, pinned third-party/generated artifacts, command-surface registries, UI branch compression and scenario-runner fusion.

2. Stabilize the brain/memory/Godel loop.
   - Keep semantic, episodic and procedural memory evidence-aware.
   - Keep LLM notes scoped and trust-scored.
   - Keep Godel substitutions strict: same IO, same semantic fingerprint, enough samples, no unchecked external backend.
   - Research current state-of-the-art memory/agent techniques before rewrites, then implement the shorter verified circuit.

3. Protect the project from data loss.
   - GitHub remote is `https://github.com/quentinhugoo-ui/Forge.git`.
   - Push meaningful work before destructive cleanup.
   - Keep build/cache/data folders out of Git.

4. Reduce UI branching.
   - Prefer `forge-section-registry.js` and `forge-tauri-bridge.js` over direct ad hoc section wiring.
   - Split only when it removes duplicated behavior or makes ownership clearer.
   - Current active sections are shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading and banger.
   - WebExplorer and Bloomberg native windows stay behind section ownership and bridge gates.

5. Keep trading as a verified compute surface.
   - Market data stays outside Git.
   - Strategy/backtest outputs should be reproducible from compact configs and hashes.
   - Live trading actions need explicit user approval and clear provider state.

6. Keep real estate and Google/provider intelligence connected to memory.
   - Harvesters produce anchored events and memory notes, not loose text.
   - Google OAuth/provider tools are inputs to the same memory/compute route, not a separate agent brain.
   - Local caches remain local unless explicitly exported.

## Next Useful Moves

- Use the public-surface manifest before deletion passes so command, MCP, UI, event and storage behavior stays provable.
- Keep managed vendor artifacts hash-pinned; replace them only with package-lock-backed bundles when upgrading.
- Finish moving MCP/Tauri command descriptions and frontend invokes toward typed registries that generate bindings and surface checks.
- Continue converting lab runners toward one scenario runner with hashed configs and proof summaries.
- Add a small brain smoke command that proves memory write, semantic recall and Godel verification in one run.
- Add a docs-size check that warns when active docs exceed a small line budget.
- Add a doc/code map smoke check that verifies README/AGENTS/runtime architecture mention active modules, sections and MCP tool groups.
- Make MCP tool descriptions action-first and short enough for agents to actually use.
- Add a safe backup command that stages source/docs, excludes caches, runs checks and pushes.
- Continue shrinking `main.rs`, `app.js`, `trading.js` and `styles.css` only where extraction removes real duplication.

## Later

- KASM program marketplace with proof manifests.
- KASM-backed memory compaction: repeated agent decisions become verified reusable programs.
- KASM-backed browser/sandbox integration for sealed web actions, if it shortens WebExplorer rather than adding a parallel browser stack.
- Avoid naming proliferation. One compute/memory/proof path.
- Optional Git LFS policy for large artifacts, only after explicit approval.

## Cut List

Remove or avoid:

- old doc blocks that describe obsolete pivots,
- duplicate "brain" implementations,
- memory records without evidence or scope,
- UI state that cannot be replayed through Tauri/MCP,
- middlemen, wrappers and functions that only forward or rename work,
- hidden generated files in source commits,
- broad cleanup commands,
- new pipelines that only rename existing flow.

## Recently Done

- GitHub backup configured and pushed with a clean snapshot history.
- `.vs/` ignored.
- EVE Online client/cache removed while protecting `C:\Users\quent\Documents\EVE\MAP`.
- Brain/memory/Godel code hardened with strict substitution verification.
- Active docs compacted so agents can read the whole doctrine without drowning in stale context.
- Docs audited against current code: WebExplorer, Bloomberg, real estate contacts, MCP brain tools and section bridge are now represented.
- Public-surface manifest verifier added for code-size reduction passes.
- Provider terminal commands/startup compressed behind shared helpers while preserving Tauri command names.
- Dead Codex canvas exec path and obsolete full dynamic tool schema removed.
- Three.js vendor source replaced by pinned local `three@0.170.0` minified artifacts; hashes: core `08fd7545d13d2c7fb65ab691530a802dafefd638596501854f267d0fb13c39e7`, GLTFLoader `90e6b59228df29a7a746b1de6ec248caaed7ebde241b822674346d4d2b6ad810`, BufferGeometryUtils `2100c6759b2a8a7da9f4f427e07dea3b5f426da1c1b4a3234b0d20ec8d630790`.
- Lab runner focus routing compressed into table-driven dispatch/help while preserving public focus aliases; current surface proof is emitted by `node examples\forge_tauri_ui\scripts\forge-surface-manifest.mjs --check`.
- Mars GeoNode seeding now runs through one shared Atlas path for Tauri and MCP, with corrupt Atlas repair preserved.
- Provider terminal UI routing, DOM lookup, state cache, login polling/connect flow and terminal control handlers collapsed into one provider runtime while preserving manifest-visible dynamic commands.
- MCP visible tool schemas now share a constructor for repeated `name`/`description`/`inputSchema` wrappers across the full `tools/list`; manifest extraction recognizes both literal and compact helper forms.
- Provider status commands, terminal ANSI setup, pty launch/failure flow and canvas runtime catalogs now share small Rust circuits; model listing probes only the requested CLI.
- Surface manifest now reports the reduction objective directly: baseline 298k, target 60k, progress, remaining lines and top file budget pressure.
- `assets/vendor` is now a closed hash-pinned artifact registry; xterm is tracked alongside Three and `xterm.css` is minified while licenses stay separate.
- MCP tool aliases and internal tool routes now live in compact registries instead of the `tools/call` match; manifest extraction proves all 126 aliases still resolve.
- Provider UI and canvas chat DOM lookups now use registries; provider terminal CSS uses `:is()` grouping and the OANDA terminal fields/mark are data-driven.
- WebExplorer reader/clone/extract clicks now route through one delegated surface handler, and provider/OANDA terminal story CSS shares one grouped visual grammar with only real differences left local.
- Trading lab scenario focus dispatch now routes same-signature runners through one proof-preserving table, while `core`, `ui-frame` and `strategy-dag-cache` remain explicit special cases.
- Surface manifest now reports tracked/physical file and directory counts plus structural pressure, so deletion/fusion passes have a local proof target.
- Ignored runtime/cache stores moved out of the repo into the local app store/quarantine path; workspace physical footprint dropped from 2309 files/375 dirs to 214 files/48 dirs before source fusion.
- `kasm::numeric` and `agent::extract` were fused into stable inline modules, preserving public Rust module paths while removing four physical files and two source folders.
- Core Rust module folders were flattened further: `agent`, `godel`, `kasm`, `meta` and `monster` now compile as stable single-file modules, while `src/kasm.td` and `src/meta.td` preserve the dialect specs.
- Workspace structure is now 136 physical files / 43 physical dirs, and source pressure dropped to `src:22f/1d`; full `cargo test --lib --tests` is green after the fusion pass.
