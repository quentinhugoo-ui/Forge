# Forge

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

Forge is a local agent workbench: a Rust/KASM compute core, a Tauri UI, an MCP surface for agents, and a persistent brain/memory layer wired to the Godel machinery.

The product direction is simple: shorten the path between intent, memory, proof, compute and action. Do not add pipelines when a shorter verified circuit can exist.

## Current Shape

- Core crate: `scan`, rooted in `src/lib.rs`.
- KASM runtime: `src/kasm/**`.
- Monster compute path: `src/monster/**`.
- Brain and memory: `src/brain.rs`.
- Godel machine: `src/godel/**`.
- Tauri workbench: `examples/forge_tauri_ui/**`.
- MCP binary: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`.
- Main UI sections: base Forge, Google/provider tools, Banger/3D, trading, real estate intelligence.

## Agent Instructions

Agents should read `AGENTS.md`.

`CLAUDE.md` intentionally imports `AGENTS.md` with `@AGENTS.md` so Claude Code and Codex share one doctrine instead of two divergent copies.

The short version:

- protect `C:\Users\quent\Documents\EVE\MAP`,
- preserve unrelated user changes,
- research current state of the art before non-trivial coding when internet is available,
- reduce architecture drag with every change,
- use Forge/MCP discipline for large data, repeated compute and proof artifacts,
- commit and push meaningful work before risky cleanup.

## Quick Checks

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
```

If MSVC is needed on Windows:

```powershell
cmd.exe /s /c 'call "C:\Program Files\Microsoft Visual Studio\2022\CommunityFresh\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 && cargo test brain --lib'
```

## Git Backup Flow

```powershell
git status --short --branch
git add <changed-source-doc-files>
git commit -m "Describe the saved work"
git push
```

Current remote:

```text
origin https://github.com/quentinhugoo-ui/Forge.git
```

Local generated data stays local unless explicitly requested: `.forge-store/`, `.forge-data/`, `data/`, `examples/data/`, datasets, `lab_findings.jsonl`, credentials and tokens.
