# Forge

Canonical backup: https://github.com/quentinhugoo-ui/Forge.git

Forge is a local agent workbench: a Rust/KASM compute core, a Tauri UI, an MCP surface for agents, and a persistent brain/memory layer wired to the Godel machinery. The project goal is not to add more pipelines. The goal is to shorten the path between intent, memory, compute, proof, and UI action.

## Current Shape

- Core crate: `scan`, rooted in `src/lib.rs`.
- KASM runtime: `src/kasm/**`, with program types, interpreter, SSA, tensor, strategy, proof, numeric, MLIR and self-hosting pieces.
- Monster compute path: `src/monster/**`, used for synthesis, atlas, lab experiments, dispatch, GPU routing and performance structures.
- Brain and memory: `src/brain.rs`, integrated through `src/apply.rs`, `src/monster/exec.rs`, `src/monster/dispatch.rs` and Tauri/MCP tools.
- Godel machine: `src/godel/**`, used for verified self-improvement, rewrite/applicator/verifier loops and content-addressed fabric ideas.
- Tauri workbench: `examples/forge_tauri_ui/**`.
- MCP binary: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`.
- Main UI sections: base Forge, Google/provider tools, Banger/3D, trading, real estate intelligence.

## Agent Entry Points

Read these first, in this order:

1. `AGENTS.md` - short operating brief for Codex/Claude/Gemini.
2. `CLAUDE.md` - agent discipline and protected zones.
3. `ROADMAP.md` - current priorities only.
4. `TOOLS&SANDBOX.md` - compute, MCP and sandbox contract.
5. `CARNET.md` - compact decision log.

Do not treat old long docs as active doctrine. If the docs and code disagree, inspect the code and update the short docs.

## Non-Negotiables

- Protect user data and local work. Never delete recursively without resolving and checking the absolute target path.
- Keep `C:\Users\quent\Documents\EVE\MAP` protected.
- Do not commit caches, build outputs, secrets, `.vs/`, `target/`, `lab_findings.jsonl`, `.forge-store/`, `.forge-data/` or external datasets.
- Prefer short-circuit architecture: remove duplicate nodes, useless middlemen and dead branches before adding new ones.
- Before writing code beyond a purely mechanical one-line edit, research the latest state of the art when internet access is available, then aim beyond it with code that is simpler, locally verifiable and easier to operate.
- Large files, repeated compute, numerical/data-heavy work and proof artifacts should go through Forge/MCP or a compact command result, not raw LLM reading.
- GitHub is the safety net. Commit and push meaningful work before risky cleanup.

## Quick Checks

Core checks:

```powershell
cargo check --lib --tests
cargo test brain --lib
```

Tauri checks:

```powershell
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
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

The GitHub `master` branch is a clean snapshot history. The older local history with large files is kept locally as `archive/master-large-history-before-github-20260514`.

## Architecture Rule

Forge should feel like one system:

```text
LLM intent -> brain/memory -> Godel verification -> KASM/Monster compute -> proof/artifact -> UI/MCP action
```

Avoid:

- parallel memory stores that do not share evidence,
- UI-only state that cannot be replayed,
- agent notes without scope, trust score or evidence,
- useless middlemen between intent, memory, proof, compute and action,
- pipelines that copy data instead of referencing content-addressed artifacts,
- docs that become a second codebase.

## Local Data Policy

Local generated data stays local unless explicitly requested:

- `.forge-store/`, `.forge-data/`, `data/`
- `examples/data/`, `examples/datasets/`
- `*.csv`, `*.parquet`
- `lab_findings.jsonl`
- credentials and tokens

If a large artifact must be preserved, prefer a compact manifest with hashes and reproduction steps, or Git LFS only after an explicit decision.
