# Forge Roadmap

This file is the live execution checklist. It is not an essay and not a strategy archive.

Rules:

- Add concrete actions only.
- Every action must name the files/modules to touch and the verification command or smoke.
- Remove actions when they are done.
- Do not keep parallel TODO lists in side documents.
- MCP is external compatibility only. The internal path is direct Forge CLI/runtime.

## Target Circuit To Deliver

```text
LLM CLI inside Forge
-> forge_agent
-> ForgeSlash/Intent
-> policy/Godel gate
-> direct runtime host routes
-> KASM/FBC/Monster compute or brain/memory
-> compact projection/proof/artifact
-> Tauri action
```

## Execution Checklist

### 1. Direct CLI Runtime

- [ ] Add `forge_agent replay`.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`, `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`.
  - Input: `execution_hash`, `projection_hash`, `trace_hash` or `projection_ref`.
  - Output: compact persisted projection, `cache_hit=true`, `raw_data_returned=false`.
  - Verify: plan/safe once, replay once, same `execution_hash`.

- [ ] Add direct CLI smoke covering `about -> plan -> safe -> approve -> read`.
  - Touch: Rust test or small smoke binary under `examples/forge_tauri_ui/src-tauri/src/bin`.
  - Output: intent hash, policy hash, execution hash, projection hash, optional program hash.
  - Verify: `cargo test --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_agent forge_agent_runtime`.

- [ ] Add direct CLI `read kind=programs` and `read kind=direct_runs` examples to `about`.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`.
  - Done when: `forge_agent about` advertises exact commands for projection/program/run inspection.
  - Verify: `cargo run --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_agent -- about`.

- [ ] Move direct store path resolution behind one runtime function.
  - Touch: `forge_agent_runtime.rs`, `forge_agent.rs`, `forge_agent_tools.rs`.
  - Remove duplicate caller-side store resolution where runtime can own it safely.
  - Verify: `cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_agent`.

- [ ] Add direct CLI KASM interop compile route.
  - Touch: `forge_agent.rs`, `forge_agent_runtime.rs`, `src/kasm.rs`.
  - Input: ForgeSlash ref to WIT/MLIR artifact, not raw path authority.
  - Output: contract hash, KASM program hash, proof hash, no raw source dump.
  - Verify: one direct CLI smoke returns `mcp_in_primary_path=false`.

### 2. MCP Surface Reduction

- [ ] Replace MCP-owned exact cache code with shared runtime calls only.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`.
  - Remove any remaining duplicate cache lookup/persist helpers after parity is proven.
  - Verify: `cargo test --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp mcp_surface_tests`.

- [ ] Move rich Metric DSL program creation out of `forge_mcp.rs`.
  - Touch: `forge_agent_runtime.rs`, `forge_mcp.rs`.
  - Runtime function shape: `direct_create_rich_program(store_path, args, actor) -> compact Value`.
  - MCP should become a parameter adapter only.
  - Verify: existing create tests still return same `program_hash` shape.

- [ ] Move rich Metric DSL execution out of `forge_mcp.rs`.
  - Touch: `forge_agent_runtime.rs`, `forge_mcp.rs`.
  - Runtime function shape: `direct_run_rich_program(store_path, args, actor) -> compact Value`.
  - Preserve cache hits, metrics hashes, proof paths and bounded previews.
  - Verify: MCP run tests plus one direct CLI approved run smoke.

- [ ] Move MCP projection persistence wrappers to shared runtime.
  - Touch: `forge_agent_runtime.rs`, `forge_mcp.rs`.
  - Keep one index format under `intent-projections/index.json`.
  - Verify: persisted projection round trips by `execution_hash` from both MCP and CLI.

- [ ] Make broad MCP catalog explicitly legacy-only in tests.
  - Touch: `forge_mcp.rs` tests.
  - Assert default visible tools are only `forge.search`, `forge.execute`, `forge.read_projection`, `forge.cancel`.
  - Verify: broad catalog visible only when `FORGE_MCP_SURFACE=broad` or legacy env is set.

- [ ] Delete MCP helpers that only forward to direct runtime.
  - Touch: `forge_mcp.rs`.
  - Remove wrapper functions after their callers use runtime directly.
  - Verify: `rg "lookup_cached_intent|persist_forge_intent_projection" forge_mcp.rs` returns no obsolete duplicates.

### 3. ForgeSlash Intent Language

- [ ] Add `replay` verb or canonical `project replay=true`.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/forge_intent.rs`.
  - Accept only content refs/hashes, not filesystem paths.
  - Verify: parse tests for valid replay and raw-path denial.

- [ ] Add `compile` verb for KASM/FBC imports.
  - Touch: `forge_intent.rs`, `forge_agent_runtime.rs`.
  - Allowed args: `artifact_ref`, `kind`, `target`, `max_bytes`.
  - Verify: parse + policy tests.

- [ ] Add numeric argument normalization.
  - Touch: `forge_intent.rs`.
  - Done when integer-looking values canonicalize as stable JSON integers or runtime helpers accept both integer/float JSON.
  - Verify: content hash stability for `limit=3` and equivalent canonical AST.

- [ ] Add argument aliases for common direct refs.
  - Touch: `forge_intent.rs`.
  - Add `program_ref`, `run_ref`, `artifact_ref`, `projection_ref` where relevant.
  - Verify: each alias maps to compact direct route without raw path authority.

- [ ] Add route budget tests.
  - Touch: `forge_intent.rs`.
  - Deny too many commands, oversized args and duplicate keys.
  - Verify: existing parser tests plus new budget denial cases.

### 4. Distillation, Cache And Mini-Clone Path

- [ ] Add a `distillation-candidates` store.
  - Touch: `forge_agent_runtime.rs`.
  - Store repeated successful intents by `intent_hash`, mode, route, proof hash, hit count, last success timestamp.
  - Verify: two identical safe executions increment hit count without raw data.

- [ ] Promote read-only repeated actions to exact-cache replay.
  - Touch: `forge_agent_runtime.rs`.
  - Rule: same `intent_hash + mode + budget`, no side effects, valid projection hash.
  - Verify: second CLI call returns `cache_hit=true`.

- [ ] Promote repeated procedural memory actions to skill candidates.
  - Touch: `src/brain.rs`, `forge_agent_runtime.rs`.
  - Store scope, evidence hash, examples, rollback-to-LLM rule.
  - Verify: memory action without evidence stays `unverified` and cannot promote.

- [ ] Promote repeated compute routes to verified program candidates.
  - Touch: `forge_agent_runtime.rs`, `src/kasm.rs` or `src/fbc.rs`.
  - Require output hash, proof hash, semantic fingerprint and test vector hash.
  - Verify: candidate remains `pending_execution_evidence` until all fields exist.

- [ ] Add router-example promotion before any local model training.
  - Touch: `forge_agent_runtime.rs`, optional `src/brain.rs`.
  - Store route examples with holdout result, shadow-eval status and rollback rule.
  - Verify: router examples are never used when shadow eval fails.

- [ ] Add local mini-model gate as disabled-by-default manifest.
  - Touch: `forge_agent_runtime.rs`.
  - Manifest fields: dataset hash, license/provider review, eval hash, rollback hash, explicit enable flag.
  - Verify: no mini-model execution path can run unless manifest is explicitly enabled and verified.

- [ ] Add distillation status to compact projections.
  - Touch: `forge_agent_runtime.rs`.
  - Output: exact-cache candidate, skill candidate, verified-program candidate, router candidate, mini-model blocked/enabled.
  - Verify: projection remains bounded and `raw_data_returned=false`.

### 5. Brain, Memory And Godel

- [ ] Add one brain smoke command through direct CLI.
  - Touch: `src/brain.rs`, `src/godel.rs`, `forge_agent_runtime.rs`, `forge_agent.rs`.
  - Flow: commit semantic note -> recall -> Godel verification -> compact proof.
  - Output: note hash, evidence hash, recall hash, verification hash.
  - Verify: `cargo test brain --lib` plus CLI smoke.

- [ ] Enforce evidence fields on semantic memory writes.
  - Touch: `src/brain.rs`.
  - Deny scoped semantic notes missing trust/evidence unless marked `unverified`.
  - Verify: accepted verified note and denied missing-evidence note.

- [ ] Add stable supersession keys.
  - Touch: `src/brain.rs`.
  - Newer facts supersede older facts by stable key, scope and layer.
  - Verify: recall returns newest trusted fact and marks older facts superseded.

- [ ] Attach Godel substitution proof to repeated memory decisions.
  - Touch: `src/godel.rs`, `src/brain.rs`.
  - Output: substitution candidate hash, verifier hash, rejection reason on failure.
  - Verify: invalid semantic substitution fails closed.

- [ ] Convert one repeated memory operation into a reusable proof manifest.
  - Touch: `src/brain.rs`, `forge_agent_runtime.rs`.
  - Done when: repeated recall/commit produces same proof hash over two runs.
  - Verify: deterministic test.

### 6. KASM Interop And Proprietary Bytecode

- [ ] Extend WIT parser beyond single-line `func`.
  - Touch: `src/kasm.rs::interop`.
  - Support multi-line `world`, import/export and inline interface blocks.
  - Refuse rich ABI types until Forge owns lowering.
  - Verify: `cargo test interop --lib`.

- [ ] Add WIT contract hash projection.
  - Touch: `src/kasm.rs`.
  - Output: package, worlds, interfaces, functions, `contract_hash`.
  - Verify: whitespace/comments do not change hash.

- [ ] Lower MLIR `vector.*` subset to existing KASM vector ops.
  - Touch: `src/kasm.rs`.
  - Admit only static shapes and supported element types.
  - Verify: executable vector test and fail-closed unsupported test.

- [ ] Lower constant-bound `scf.if`.
  - Touch: `src/kasm.rs`.
  - Lower simple conditionals to bounded DAG/select nodes.
  - Verify: one accepted test and one unsupported nested-region denial.

- [ ] Add KASM interop lab runner.
  - Touch: `examples/lab_runner_kasm_interop.rs` or compact extension of `examples/lab_runner_fbc.rs`.
  - Print WIT contract hash, MLIR report, program hash, node count, output hash.
  - Verify: `cargo run --example lab_runner_kasm_interop`.

- [ ] Add no-external-runtime audit.
  - Touch: `src/kasm.rs` tests or a small audit script.
  - Deny `wasmtime`, `wasmer`, `llvm-sys`, `mlir`, `inkwell`, `cranelift`, `mlir-opt`, external process execution.
  - Verify: `cargo test interop --lib`.

### 7. FBC/KASM Sandbox Guards

- [ ] Audit every sensitive Tauri command for guard metadata.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/main.rs`, audit script or Rust test.
  - Cover native present/hide, Bloomberg live, jobs, hardware, artifact reads, network-source calls.
  - Verify: audit fails when a sensitive command lacks guard metadata.

- [ ] Move guard ledger writes to one host path.
  - Touch: `examples/forge_tauri_ui/src-tauri/src/forge_fbc_host.rs`, `main.rs`.
  - Remove duplicated proof wrappers in individual commands.
  - Verify: `cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml`.

- [ ] Add capability handles for artifact, memory, UI projection and model provider.
  - Touch: `src/fbc.rs`.
  - Deny raw path, URL and token input in tests.
  - Verify: `cargo test fbc --lib`.

- [ ] Connect KASM interop outputs to FBC proof envelope.
  - Touch: `src/fbc.rs`, `src/kasm.rs`.
  - Record `contract_hash -> kasm_program_hash -> verifier_hash -> proof_hash`.
  - Verify: deterministic stable proof hash.

### 8. Tauri UI And TypeScript Migration

- [ ] Keep all hand-written UI source in TypeScript only.
  - Touch: `examples/forge_tauri_ui/ui/src/**`.
  - Do not edit generated `ui/dist/**/*.js` by hand.
  - Verify: `cd examples\forge_tauri_ui; npm.cmd run audit:js-debt`.

- [ ] Shrink `ui/src/shell/surface.ts` by extracting repeated behavior into existing shell modules.
  - Touch: `ui/src/shell/**`.
  - Do not add a new shell path.
  - Verify: `node examples\forge_tauri_ui\scripts\forge-surface-manifest.mjs --check`.

- [ ] Shrink trading section through ownership extraction only.
  - Touch: `ui/src/sections/trading/**`.
  - Keep live trading actions behind provider state and approval.
  - Verify: `node examples\forge_tauri_ui\scripts\forge-ui-section-audit.mjs`.

- [ ] Shrink banger section through section-runtime extraction only.
  - Touch: `ui/src/sections/banger/**`.
  - Preserve current ownership contract.
  - Verify: `node examples\forge_tauri_ui\scripts\forge-ui-smoke.mjs`.

- [ ] Keep WebExplorer and Bloomberg native actions behind bridge gates.
  - Touch: `ui/src/shell/tauri-bridge.ts`, `ui/SECTION_OWNERSHIP.json`, backend command declarations if needed.
  - Verify: `node examples\forge_tauri_ui\scripts\forge-tauri-bus-audit.mjs --strict`.

### 9. Trading Compute Surface

- [ ] Add reproducible trading scenario manifests.
  - Touch: trading scripts/runtime only, not market datasets.
  - Store config hash, input ref hash, output hash and proof hash.
  - Verify: replay from manifest produces same hashes.

- [ ] Add approval proof for every live trading action.
  - Touch: Tauri backend, trading section bridge.
  - Include provider state, user approval, action hash and timestamp bucket.
  - Verify: live action denied without approval and accepted with approval.

- [ ] Keep market datasets and caches out of Git.
  - Touch: `.gitignore` only if needed.
  - Verify: `git status --short` shows no market data/cache files.

### 10. Real Estate And Provider Intelligence

- [ ] Convert harvested real-estate events into anchored memory notes.
  - Touch: `examples/forge_tauri_ui/scripts/real-estate-evidence-memory-builder.mjs`, `src/brain.rs` only if needed.
  - Output: note hash, evidence hash, source hash.
  - Verify: builder emits memory artifacts without raw source dumps.

- [ ] Route provider intelligence through shared memory/compute path.
  - Touch: provider tools and `ui/src/sections/real-estate/**`.
  - No provider-specific agent brain.
  - Verify: provider result produces bounded projection and memory evidence hash.

- [ ] Connect real-estate ToolCells to app-wide proof envelope.
  - Touch: ToolCell runner/scripts, `src/fbc.rs`, `src/kasm.rs`.
  - Verify: app-wide proof includes real-estate cells plus non-real-estate cells under one ledger root.

### 11. Repo Compression And Safety

- [ ] Add structure metrics command.
  - Touch: script under `examples/forge_tauri_ui/scripts` only if no existing script fits.
  - Output: source/doc line counts, file counts, excluded generated/cache dirs.
  - Verify: command does not read `C:\Users\quent\Documents\EVE\MAP`.

- [ ] Add doc/code map smoke.
  - Touch: script or test.
  - Verify: `AGENTS.md`, `FORGE_RUNTIME_ARCHITECTURE.md`, `SECTION_CONTRACT.md`, `ROADMAP.md` mention active modules, sections and generated bundle policy.

- [ ] Compress obsolete docs.
  - Touch: docs only.
  - Remove stale historical narratives; keep current contracts and commands.
  - Verify: doc/code map smoke passes.

- [ ] Delete obsolete wrappers after direct runtime parity.
  - Touch: modules found by `rg "legacy|compat|forward|wrapper"`.
  - Only delete after tests prove the direct route owns behavior.
  - Verify: relevant cargo/node checks plus `git diff --stat`.

- [ ] Protect destructive cleanup behind Git backup.
  - Before cleanup: `git status --short --branch`, `git diff --stat`, meaningful commit, push.
  - Never recursive delete/move without resolved absolute path guard.

## Remove Or Avoid

- [ ] Remove objectives that do not name files and verifiers.
- [ ] Remove old doc blocks that describe obsolete pivots.
- [ ] Remove duplicate brain implementations or memory stores.
- [ ] Remove memory records without evidence, scope or trust state.
- [ ] Remove UI state that cannot replay through Tauri/direct runtime.
- [ ] Remove agent actions that require MCP when an equivalent direct Forge command exists.
- [ ] Remove hand-written JavaScript outside generated `ui/dist/**/*.js`.
- [ ] Remove middlemen, wrappers and functions that only forward or rename work.
- [ ] Avoid hidden generated files in source commits.
- [ ] Avoid broad cleanup commands.
- [ ] Avoid new pipelines that only rename an existing flow.

## Current Verified State

- Hand-written frontend JavaScript is gone. UI source is TypeScript under `examples/forge_tauri_ui/ui/src/**`; browser bundles are generated under `ui/dist/**/*.js`.
- Compact MCP facade is default visible external surface: `forge.search`, `forge.execute`, `forge.read_projection`, `forge.cancel`.
- Direct agent CLI exists at `examples/forge_tauri_ui/src-tauri/src/bin/forge_agent.rs`; it persists non-`about` projections by default, exact-hits `plan`/`safe` by `intent_hash + mode + budget`, and returns projections with `mcp_in_primary_path=false`.
- Shared direct runtime exists at `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`; MCP calls it as an adapter for orchestration/cache paths. It owns direct projection persistence/read/list, exact cache lookup, direct program creation/run, and compact program/run read-list routes.
- FBC app-wide lab covers shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading, banger and sensitive commands under one ledger root.
- KASM interop importer exists in `src/kasm.rs::interop`; it parses WIT-like contracts, lowers simple MLIR `func.func`/`arith.*`/constant-bound `scf.for` into KASM, and refuses rich WIT ABI types without Forge-owned lowering.
- Runtime/cache stores stay out of Git; protected path remains `C:\Users\quent\Documents\EVE\MAP`.
