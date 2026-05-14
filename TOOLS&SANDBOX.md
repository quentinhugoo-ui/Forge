# Forge Tools And Sandboxes

This file is the compact tool contract. It should stay short.

## Core Rule

Use Forge as the compute/proof substrate, not as another chat transcript. Large data stays on disk. Agents exchange compact manifests, hashes, previews, logs and artifacts.

## When To Use Forge/MCP Discipline

Use it for:

- large files or directories,
- repeated or expensive computation,
- scientific/numerical/data-heavy analysis,
- codebase-wide metrics,
- proof/hash/artifact generation,
- any task where a human should be able to reproduce the result.

If a dedicated MCP runner is not available, use shell commands that produce compact, verifiable output: counts, hashes, paths, top offenders, short previews.

## Normal Flow

```text
intent -> inspect small context -> plan compact run -> execute -> read logs/artifacts -> update code/docs -> verify -> commit/push
```

Do not put raw huge datasets, full logs or giant source files into the LLM context.

## Source Commands

Core:

```powershell
cargo check --lib --tests
cargo test brain --lib
```

Tauri:

```powershell
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
```

UI smoke scripts live in:

```text
examples/forge_tauri_ui/scripts/
```

## Git Backup

Remote:

```text
https://github.com/quentinhugoo-ui/Forge.git
```

Safe flow:

```powershell
git status --short --branch
git diff --stat
git add <source-doc-files>
git commit -m "Short useful message"
git push
```

Never add generated caches or secrets to make a push succeed.

## Ignore Policy

Keep out of Git:

- `.vs/`
- `target/`, `target-msvc-tests/`
- `.codex-tmp/`, `.forge-store/`, `.forge-data/`
- `data/`, `examples/data/`, `examples/datasets/`
- `lab_findings.jsonl`
- `*.csv`, `*.parquet`
- `.env*`, tokens, API keys, credentials

## Sandbox Safety

Before recursive delete or move on Windows:

1. Resolve the absolute target path.
2. Check it is under the intended parent.
3. Check it is not a protected path.
4. Use `Remove-Item -LiteralPath` on the resolved target.
5. Verify the result.

Protected path:

```text
C:\Users\quent\Documents\EVE\MAP
```

## Brain/Memory Tool Contract

Memory writes should carry:

- scope,
- memory layer,
- trust score,
- evidence/proof hash when available,
- stable fact key when supersession matters.

Unverified LLM notes are allowed only as unverified notes. They are not proof.

## UI Tool Contract

Prefer existing bridges:

- `forge-section-registry.js`
- `forge-tauri-bridge.js`
- `forge-boot.js`
- `SECTION_OWNERSHIP.json`

Do not create a new UI command path if the registry/bridge can express it.
