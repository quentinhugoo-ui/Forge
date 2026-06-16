# Forge

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

Forge is a local agent workbench: a Rust/KASM compute core, an Electron/React product shell backed by native Rust services, a direct agent CLI (BrainCommand in, verified compact projection out), and a persistent brain/memory layer wired to the Godel machinery.

The product direction is simple: shorten the path between intent, memory, proof, compute and action. Do not add pipelines when a shorter verified circuit can exist.

## Current Shape

- Core crate: `scan`, rooted in `src/lib.rs`.
- KASM runtime: `src/kasm.rs` plus dialect spec `src/kasm.td`.
- Monster compute path: `src/monster.rs`.
- Brain and memory: `src/brain.rs`.
- Godel machine: `src/godel.rs`.
- Product frontend: `examples/ingen_electron_shell/**`.
- Native/shared services: `examples/ingen_native_services/**`.
- Direct agent/service surface: native Rust crates only.
- Main UI sections: Forge shell, Alpha, WebExplorer peripheral, Banger/3D, trading, and real-estate/real-estate-main.
- Runtime integrations: Google OAuth, provider terminals, OANDA trading, native WebExplorer, Bloomberg live, Banger GPU lifecycle, real estate harvester.

## Agent Instructions

Agents should read `AGENTS.md`.

`CLAUDE.md` points to `AGENTS.md` so Claude Code and Codex share one doctrine instead of two divergent copies.

For the CodeAct action layer, BrainCommand, Forge language, Monster pipeline and live objectives, read `FORGE_NATIVE_BYTECODE.md`.

The short version:

- protect `C:\Users\quent\Documents\EVE\MAP`,
- preserve unrelated user changes,
- research current state of the art before non-trivial coding when internet is available,
- reduce architecture drag with every change,
- use Forge direct-CLI discipline (forge_agent / BrainCommand) for large data, repeated compute and proof artifacts,
- commit and push meaningful work before risky cleanup.

For the exact live architecture and agent rules, `AGENTS.md` wins over this README.

## Quick Checks

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\ingen_native_services\Cargo.toml
cd examples\ingen_electron_shell
npm.cmd run typecheck
npm.cmd run build
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

Local generated data stays local unless explicitly requested. Runtime state defaults to `%APPDATA%\com.forge.ui\forge-store` on Windows, or to `FORGE_STORE_DIR` when set; workspace `.forge-store/` is legacy/override-only and must not be committed. Keep `.forge-data/`, `data/`, `examples/data/`, datasets, `lab_findings.jsonl`, credentials and tokens out of Git.
