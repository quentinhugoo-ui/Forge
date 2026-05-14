# ROADMAP

This file is only the live roadmap. Historical plans belong in Git history, not in the active agent context.

## Current North Star

Forge should become a compact local agent OS:

```text
brain/memory + Godel verification + KASM/Monster compute + Tauri/MCP actions
```

The winning architecture is not the one with the most subsystems. It is the one where an agent can move from intent to verified action with the fewest nodes.

## Now

1. Stabilize the brain/memory/Godel loop.
   - Keep semantic, episodic and procedural memory evidence-aware.
   - Keep LLM notes scoped and trust-scored.
   - Keep Godel substitutions strict: same IO, same semantic fingerprint, enough samples, no unchecked external backend.
   - Research current state-of-the-art memory/agent techniques before rewrites, then implement the shorter verified circuit.

2. Protect the project from data loss.
   - GitHub remote is `https://github.com/quentinhugoo-ui/Forge.git`.
   - Push meaningful work before destructive cleanup.
   - Keep build/cache/data folders out of Git.

3. Reduce UI branching.
   - Prefer `forge-section-registry.js` and `forge-tauri-bridge.js` over direct ad hoc section wiring.
   - Split only when it removes duplicated behavior or makes ownership clearer.

4. Keep trading as a verified compute surface.
   - Market data stays outside Git.
   - Strategy/backtest outputs should be reproducible from compact configs and hashes.
   - Live trading actions need explicit user approval and clear provider state.

5. Keep real estate intelligence connected to memory.
   - Harvesters produce anchored events and memory notes, not loose text.
   - Local caches remain local unless explicitly exported.

## Next

- Add a small brain smoke command that proves memory write, semantic recall and Godel verification in one run.
- Add a docs-size check that warns when active docs exceed a small line budget.
- Make MCP tool descriptions action-first and short enough for agents to actually use.
- Add a safe backup command that stages source/docs, excludes caches, runs checks and pushes.
- Continue shrinking `main.rs`, `app.js`, `trading.js` and `styles.css` only where extraction removes real duplication.

## Later

- KASM program marketplace with proof manifests.
- KASM-backed memory compaction: repeated agent decisions become verified reusable programs.
- KASM/KASM-like browser or KASM sandbox integration for sealed web actions.
- KASM + KASM? No: avoid naming proliferation. One compute/memory proof path.
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

## Done Recently

- GitHub backup configured and pushed with a clean snapshot history.
- `.vs/` ignored.
- EVE Online client/cache removed while protecting `C:\Users\quent\Documents\EVE\MAP`.
- Brain/memory/Godel code hardened with strict substitution verification.
- Active docs compacted so agents can read the whole doctrine without drowning in stale context.
