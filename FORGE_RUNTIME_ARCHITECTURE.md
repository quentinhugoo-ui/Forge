# Forge Runtime Architecture

This document explains the operational architecture agents should use: direct Forge CLI commands, slash commands, compatibility MCP tools, sandbox boundaries, and the persistent brain/memory path. Keep it compact.

## One Route

Forge should behave like one short circuit:

```text
user intent
-> LLM CLI inside Forge
-> ForgeSlash/Intent program
-> Godel/policy gate
-> Forge local compute, brain route or verified bytecode route
-> compact hashes/proofs/artifacts
-> Tauri/UI action through section bridge
-> optional memory commit
```

Do not create a second path for memory, browser actions, trading metrics or file analysis if this route can carry it.

## Direct Agent OS Contract

Forge's primary agent interface is direct command execution, not MCP. The internal loop is:

```text
forge_agent plan|run|approve
-> ForgeSlash parser/compiler
-> policy/Godel gate
-> direct route executor
-> projection/cache/promotion evidence
```

The first direct entrypoint is `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`. It supports `about`, `plan`, `safe` and `approve`, producing compact projections with `mcp_in_primary_path=false`. It persists non-`about` projections by default unless `--no-persist` is passed.

The shared engine is `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`. Projection compilation, safe execution orchestration, execution reports, compact step proofs, approved side-effect hash gates, compact projection persistence/read/list, exact cache lookup, direct content-addressed program creation, direct manifest runs and compact program/run read-list routes live there. `forge_mcp.rs` is now an adapter around that runtime for these paths. `forge_agent plan` and `forge_agent safe` check exact local cache hits by `intent_hash + mode + budget` before recompute. `forge_agent safe` can execute plan-only run planning, brain recall/explain, projection reads and direct program/run inspection; `forge_agent approve` can execute approved brain commits, direct program creation and direct manifest runs. The remaining hostcall to drain from the MCP adapter is the rich Metric DSL/visual-program executor for advanced legacy programs.

MCP must not become the internal action language. If a workflow can be expressed as ForgeSlash and executed by direct Forge routes, it belongs behind `forge_agent` first.

## MCP Compatibility Contract

MCP is the external transport and compatibility boundary for outside LLM clients. The target public MCP surface is 2-4 visible tools:

- `forge.search`: compact discovery over capabilities, examples, slash commands and reusable programs.
- `forge.execute`: run a ForgeSlash/Intent program through validation, routing, local compute and projection.
- `forge.read_projection`: optional bounded read of an existing compact projection.
- `forge.cancel`: optional safe cancellation path.

All other MCP tools should become internal routes, compiler targets or exceptional direct tools. Keep a direct MCP tool visible only when a measured workflow proves it is simpler, safer or faster than the direct ForgeSlash/CLI path.

`forge.search` is backed by a compact intent index, not by dumping tool schemas. Its entries group current routes into target actions such as `discover`, `execute`, `projection`, `brain`, `geo`, `profile` and `cancel`, each with tags, slash aliases, current routes and one copy-ready ForgeSlash example. Search returns scored candidates plus an executable `next_call` and `route_plan`; if no candidate matches, it falls back to discovery instead of returning a dead end.

## Slash Commands

Slash commands are the user/agent-facing language and the source syntax for the intent compiler. There are three families:

- Direct Forge agent commands, which enter through `forge_agent` and should become the default runtime path.
- MCP slash commands, which are compatibility wrappers in `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`.
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

Current broad MCP tools are transitional compatibility routes, not the future default surface. `about`, `jobs`, `sessions`, `documents`, `mapping`, `profile`, `atlas`, `update_session`, `read`, `logs`, `cancel` and the domain-specific helpers should be lowered behind the compact intent facade as parity is proven.

ForgeSlash v0 source contract:

```text
/forge
recall scope=<scope>
plan intent="<goal>" input=@latest
create title=<short> program_kind=<compute_program|visual_program> goal="<goal>"
run input=@latest intent="<goal>" plan_only=true
project job_id=<id> max_bytes=4096
commit scope=<scope> kind=<semantic|episodic|procedural> observation="<bounded note>"
explain hash=<hash>
```

Allowed verbs are `recall`, `plan`, `create`, `run`, `project`, `commit` and `explain`. Values are quoted strings, booleans, numbers, bare tokens or bounded refs such as `@latest`, `@pending`, `@job:<id>`, `@program:<hash>` and `@artifact:<hash>`. Raw filesystem paths are not valid ForgeSlash v0 authority; execution must resolve refs through Forge and then pass Godel/policy gates. Parsed programs and commands are content-addressed from their canonical AST, with arguments sorted by key so equivalent intent text hashes to the same route.

The v0 compiler lowers verbs to existing routes only: `recall -> brain_recall`, `plan -> run` with `plan_only=true`, `create -> create`, `run -> run`, `project -> read`, `commit -> brain_commit`, and `explain -> brain_explain`. This is a route plan, not a second executor.

The compact facade names are visible only to outside MCP clients, while the old route names stay callable as legacy/internal aliases: `forge.search -> forge_intent_search`, `forge.execute -> forge_intent_execute`, `forge.read_projection -> read`, and `forge.cancel -> cancel`.

`tools/list` now defaults to the final four-tool facade. `FORGE_MCP_SURFACE=broad`, `FORGE_MCP_LEGACY_SURFACE=1` or `FORGE_MCP_BROAD_SURFACE=1` restores the transitional broad catalog for compatibility debugging. `forge.execute` defaults to `mode=plan`; `mode=execute_safe` or `execute_safe=true` may execute only read-only and `plan_only` lowered routes (`run` planning, `read`, `brain_recall`, `brain_explain`). Side-effect routes such as `create`, real `run` and `brain_commit` are skipped with explicit step proofs unless the caller uses the approved execution gate.

`mode=execute_approved` is the side-effect gate. It requires `approve_side_effects=true`, `approved_intent_hash` and `approved_policy_hash` matching a previously projected intent. Non-plan `run` has a second lock, `allow_run_side_effects=true`, so program creation or memory commits cannot accidentally claim/run pending user jobs. Rejected approval returns `mode=approval_required`; accepted approval returns `forge_intent_execution_report_v0` with `mode=execute_approved`.

Approved side effects return compact step evidence. `create` exposes the created `program_hash`; `brain_commit` exposes `note_hash`, `memory_layer` and verification status. Intent execution calls internal brain routes directly for projection values instead of wrapping them in the broad MCP response envelope.

The server exposes `compact_cutover_readiness` in `about` and policy payloads. Status `ready_as_current_default` means the compact facade, aliases, projection replay, exact cache and approval gate are live, and the broad catalog is hidden from `tools/list` by default.

Safe execution emits `forge_intent_execution_report_v0`: step counts, skipped/error counts, `executed_steps_hash` and a domain-separated SHA-256 `execution_hash`. Individual safe steps carry `result_hash`, a compact `result_summary` and only a budgeted result or preview. These hashes are evidence for replay/cache/router decisions; they are not sufficient to promote side-effect behavior, which still needs output/proof hashes from the real executor.

Intent projections are persisted under the Forge store as `intent-projections/<projection_hash>.json`, where the projection hash is the `execution_hash` when present, otherwise the trace or intent hash. Persist also updates `intent-projections/index.json`, a bounded compact index of recent projections. `forge.read_projection` can list recent projections or read them back by `execution_hash`, `trace_hash`, `intent_hash`, `projection_hash` or `projection_ref`; `forge.search` reads the same index so successful intents become discoverable replay candidates. Both routes return the compact intent facade envelope instead of the broad legacy MCP policy payload.

`forge.execute` and the direct `forge_agent plan|safe` commands check that same index before recompute. An exact cache hit requires the same `intent_hash`, same execution mode and a stored preview budget greater than or equal to the requested `max_bytes`; the returned projection is marked with `cache_hit=true` and `cache_reason=exact_intent_mode_and_budget`.

Before execution, the compiled intent must pass the v0 policy report: step budget, argument byte budget, route allowlist, side-effect allowlist and raw-path rejection. The report is content-hashed and exposed in the MCP `about` smoke; execution code must treat a failed report as a hard stop.

Every accepted compiled intent should emit a compact `TraceCard`. In v0, before real execution is wired, the card records `planned_policy_ok` or `blocked_policy_failed`, intent/policy/command hashes, lowered routes, side-effect count, argument bytes and a first distillation candidate. Later execution may fill `proof_hash` and `output_hash`, but raw data must still stay outside LLM context.

The v0 distillation analyzer is conservative: policy failures never promote; read-only routes prefer exact cache; `run`/`create` routes can become verified programs only after execution evidence; memory/procedural routes can become skills only after scoped evidence; mini-models are not selected until cheaper layers fail with proof.

Verified-program promotion needs a `PromotionManifest`. For `run`/`create`, the manifest stays `pending_execution_evidence` until execution supplies output hash, proof hash, test vectors and semantic fingerprint. Read-only projections may be exact-cache ready immediately because they replay bounded refs instead of new behavior.

Procedural skill promotion needs a separate `SkillPromotionManifest`. It only applies to procedural/memory traces, requires scope, evidence hash and examples, and should remain pending until human/reviewable evidence exists. Non-skill targets must not emit installable skills.

Router/model promotion is stricter still. A `RouterPromotionManifest` may only create local router examples after holdout traces, shadow evaluation and rollback-to-LLM evidence. Model training is blocked by default and requires explicit provider/license review before any future promotion path can enable it.

The common return shape is `ForgeProjection`: intent hash, policy hash, trace hash, command/proof/output hashes, route counts, promotion statuses and bounded preview budget. It must set `raw_data_returned=false`; raw files, logs and artifacts remain in Forge/CAS and are referenced by hash or bounded refs.

UI-local slash examples:

- Trading: `/vwap`, `/ema`, `/sma`, `/bollinger`, `/supertrend`, `/indicator`, `/alert_`, `/order`, `/strategy_`, `/backtest_`, `/dataset_`, `/map_`, `/lens_`.
- WebExplorer/real estate: `/connexion`, `/gmail_agence`, `/extraire_agence`, `/prix_marche`, `/preuves`, `/crawler_zone`, `/prioriser_actions`, `/comparer_zones`, `/plan_action`, `/validation_humaine`, `/programme`, `/metrique`, `/carte_visuelle`, `/geo`, `/micro_geo`.

These UI-local commands are not a reason to add a new backend. They should either stay local and reversible, call an existing native command through the section bridge, or become a compact Forge MCP/brain operation.

## Direct Command Discipline

Use direct Forge commands first, and MCP only when the caller is an external MCP client. Use Forge for:

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
| L2 Tauri bridge | `ui/src/shell/tauri-bridge.ts` / `ui/dist/forge-tauri-bridge.js` | Shared UI/native commands go through section ownership checks. |
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

- `examples/forge_tauri_ui/ui/src/shell/legacy-section-registry.ts`
- `examples/forge_tauri_ui/ui/src/shell/tauri-bridge.ts`
- `examples/forge_tauri_ui/ui/src/shell/boot.ts`
- `examples/forge_tauri_ui/ui/src/shell/click-router.ts`
- `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`
- `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`

Sensitive native commands such as `webexplorer_native_present` and `bloomberg_live_native_present` must declare ownership in `SECTION_OWNERSHIP.json`, require the shared bridge, and require the active section.

Frontend source is TypeScript only. Hand-written JavaScript was removed from `ui/`; generated browser bundles live under `ui/dist/**/*.js`. The remaining large TS surfaces are transitional and should shrink only through verified extraction:

- `ui/src/shell/surface.ts`
- `ui/src/sections/trading/surface.ts`
- `ui/src/sections/banger/surface.ts`

Real-estate mode logic is no longer a left-panel one-off inside the shell. It is split into section runtimes for context, onboarding, language, mode lifecycle and floating panels under `ui/src/sections/real-estate/*runtime.ts`.

## Native Bytecode Direction

Long-term, Forge should not depend on WASM, MLIR, gVisor or microVMs as its core plugin layer. Those systems are inspiration, not the center. The target is a proprietary Forge Native Bytecode (`FBC` / `KASM2`) that applies across UI, trading, banger, WebExplorer, real estate, scrapers, automations, agents, memory, files, compute and plugins:

```text
Forge Intent
-> KASM / Forge Bytecode
-> Forge VM
-> verifier
-> capability sandbox
-> CPU/GPU backend
-> proof ledger
-> UI projection
```

The design goal is to beat microVMs inside the Forge domain by being narrower: no guest OS, no syscalls, no raw filesystem/network/secrets, no native pointers, only verified bytecode, sealed capabilities, declared hostcalls and deterministic proof envelopes. This does not claim hardware-level isolation against kernel/CPU/side-channel adversaries; microVM or hardware isolation can remain a fallback for native code that cannot be expressed or verified as FBC.

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
node examples\forge_tauri_ui\scripts\forge-surface-manifest.mjs --check
```

UI:

```powershell
npm.cmd --prefix examples\forge_tauri_ui run typecheck
npm.cmd --prefix examples\forge_tauri_ui run audit:js-debt
node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs
node examples\forge_tauri_ui\scripts\forge-ui-section-audit.mjs
node examples\forge_tauri_ui\scripts\forge-tauri-bus-audit.mjs --strict
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

Keep out of Git: `.vs/`, `target/`, `target-msvc-tests/`, `.codex-tmp/`, `.forge-store/`, `.forge-data/`, `data/`, `examples/data/`, `examples/datasets/`, `lab_findings.jsonl`, `*.csv`, `*.parquet`, `.env*`, tokens, API keys and credentials. Runtime state defaults outside the repo at `%APPDATA%\com.forge.ui\forge-store` on Windows; `FORGE_STORE_DIR` is the explicit override.
