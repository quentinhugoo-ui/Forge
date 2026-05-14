# Forge Agent Brief

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

Read this before changing Forge. Keep this file short and current.

## Read Order

1. `README.md` for the project map.
2. `CLAUDE.md` for agent discipline.
3. `ROADMAP.md` for current priorities.
4. `TOOLS&SANDBOX.md` for compute/sandbox rules.
5. `CARNET.md` for compact decisions.

## Mission

Forge is a compact local agent OS:

```text
LLM intent -> brain/memory -> Godel verification -> KASM/Monster compute -> proof/artifact -> Tauri/MCP action
```

Prefer shorter circuits. Remove obsolete nodes before adding new ones.

## Hard Rules

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Preserve unrelated user changes.
- Never run recursive delete/move without resolved path guards.
- Do not commit caches, build outputs, datasets or secrets.
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

## Current Architecture

- Core library: `src/lib.rs`
- Brain/memory: `src/brain.rs`
- Godel machine: `src/godel/**`
- KASM runtime: `src/kasm/**`
- Monster compute: `src/monster/**`
- Tauri backend: `examples/forge_tauri_ui/src-tauri/src/**`
- Tauri UI: `examples/forge_tauri_ui/ui/**`
- MCP server: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`

## Useful Checks

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
```

## Documentation Rule

Docs are context for agents, not an archive. If a doc becomes noisy, compress it. Historical detail belongs in Git history.
