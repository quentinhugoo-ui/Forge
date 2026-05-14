# Forge Runtime Architecture

This document explains the operational architecture agents should use: slash commands, MCP tools, sandbox boundaries, and the persistent brain/memory path. Keep it compact.

## One Route

Forge should behave like one short circuit:

```text
user intent
-> slash/MCP command
-> Forge local compute or brain tool
-> compact hashes/proofs/artifacts
-> Tauri/UI action through section bridge
-> optional memory commit
```

Do not create a second path for memory, browser actions, trading metrics or file analysis if this route can carry it.

## Slash Commands

Slash commands are the user/agent-facing language. There are two families:

- MCP slash commands, which map to tools in `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`.
- UI-local slash intents, which exist inside WebExplorer, trading and real-estate toolbars, then converge back to MCP/brain/native bridge routes when work must persist or touch local compute.

MCP commands:

| Slash | MCP tool(s) | Use |
| --- | --- | --- |
| `/metric` | `capabilities`, `program_compile_validate_route`, `mapping_metrics` | Find/validate metric, visual or compute routes before creating work. |
| `/create_` | `create` | Create a reusable compute or visual program, stored by content hash. |
| `/program_` | `run` | Run pending uploads, existing programs, plan-only compute, CPU/GPU jobs and proof/artifact workflows. |
| `/visualprogram_` | `visual_program`, `mapping_model`, `mapping_analysis` | Build or inspect 2D/3D visual programs without sending raw rows to the LLM. |
| `/geo`, `/minigeo` | `geonode` | Create/update Atlas GeoNode or MiniGeoNode anchors. |
| brain commands | `brain_recall`, `brain_commit`, `brain_compare`, `brain_sleep`, `brain_explain` | Use the Forge brain instead of inventing another memory store. |

Other visible MCP tools: `about`, `jobs`, `sessions`, `documents`, `mapping`, `profile`, `atlas`, `update_session`, `read`, `logs`, `cancel`.

UI-local slash examples:

- Trading: `/vwap`, `/ema`, `/sma`, `/bollinger`, `/supertrend`, `/indicator`, `/alert_`, `/order`, `/strategy_`, `/backtest_`, `/dataset_`, `/map_`, `/lens_`.
- WebExplorer/real estate: `/connexion`, `/gmail_agence`, `/extraire_agence`, `/prix_marche`, `/preuves`, `/crawler_zone`, `/prioriser_actions`, `/comparer_zones`, `/plan_action`, `/validation_humaine`, `/programme`, `/metrique`, `/carte_visuelle`, `/geo`, `/micro_geo`.

These UI-local commands are not a reason to add a new backend. They should either stay local and reversible, call an existing native command through the section bridge, or become a compact Forge MCP/brain operation.

## MCP Discipline

Use Forge/MCP for:

- large files or directories,
- repeated or expensive computation,
- scientific/numerical/data-heavy analysis,
- codebase-wide metrics,
- proof/hash/artifact generation,
- anything a human should be able to reproduce.

Default output should be compact: manifests, hashes, refs, previews, logs, proof summaries and artifact paths. Raw CSVs, full logs, source dumps and point clouds stay on disk.

If a dedicated MCP runner is not available, imitate this locally with `rg`, counts, hashes, bounded previews and exact paths.

## Sandbox Levels

| Level | Surface | Rule |
| --- | --- | --- |
| L0 LLM context | Chat/model | Never load huge raw data or secrets. Ask Forge/local commands for compact evidence. |
| L1 Forge compute | MCP/store/KASM/Monster | Read local data, compute locally, return refs/proofs. |
| L2 Tauri bridge | `forge-tauri-bridge.js` | Shared UI/native commands go through section ownership checks. |
| L3 Native surfaces | WebExplorer, Bloomberg live | Require active section and bridge gates before present/hide. |
| L4 Shell/filesystem | PowerShell/Git/Cargo | Resolve paths before cleanup; commit/push before risky operations. |

Protected path:

```text
C:\Users\quent\Documents\EVE\MAP
```

Before recursive delete or move on Windows:

1. Resolve the absolute target path.
2. Check it is under the intended parent.
3. Check it is not protected.
4. Use `Remove-Item -LiteralPath` on the resolved target.
5. Verify the result.

## Brain And Memory

There is one brain path. Do not add a parallel note store.

Memory writes should carry:

- `scope` such as `basic`, `google_suite`, `banger`, `trading`, `real_estate` or `webexplorer`,
- `memory_layer`: `semantic`, `episodic` or `procedural`,
- `trust_score`,
- evidence/proof hash when available,
- stable `fact_key` when newer facts should supersede older facts.

Unverified LLM notes stay explicitly unverified. Tool-backed or proof-backed notes can be anchored. Godel-related substitutions must go through semantic verification before becoming trusted shortcuts.

Brain tool roles:

- `brain_recall`: retrieve scoped refs/previews.
- `brain_commit`: write bounded notes or verified programs.
- `brain_compare`: compare KASM programs by semantic fingerprint.
- `brain_sleep`: tighten and converge explicit program hashes.
- `brain_explain`: explain a brain hash or `refs/brain/*` ref.

## UI And Native Bridge

Current owned sections:

```text
shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading, banger
```

Coordination files:

- `examples/forge_tauri_ui/ui/forge-section-registry.js`
- `examples/forge_tauri_ui/ui/forge-tauri-bridge.js`
- `examples/forge_tauri_ui/ui/forge-boot.js`
- `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`
- `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`

Sensitive native commands such as `webexplorer_native_present` and `bloomberg_live_native_present` must declare ownership in `SECTION_OWNERSHIP.json`, require the shared bridge, and require the active section.

## Verification

Core:

```powershell
cargo check --lib --tests
cargo test brain --lib
```

Tauri/MCP:

```powershell
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
```

UI:

```powershell
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
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

Keep out of Git: `.vs/`, `target/`, `target-msvc-tests/`, `.codex-tmp/`, `.forge-store/`, `.forge-data/`, `data/`, `examples/data/`, `examples/datasets/`, `lab_findings.jsonl`, `*.csv`, `*.parquet`, `.env*`, tokens, API keys and credentials.
