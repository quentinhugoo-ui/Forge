# Forge Native Bytecode

Forge Native Bytecode (`FBC` / `KASM2`) is the target proprietary execution layer for Forge. It is not yet the universal runtime, but `src/fbc.rs` now carries a compact v0 ToolCell compiler, capability host context, optimizer, backend selector, verifier, deterministic interpreter, proof envelope and proof ledger entry for early KASM2 trials.

## Mission

Replace plugin sprawl with one verified local circuit:

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

This applies to the whole app: shell/UI, trading, banger, WebExplorer, real estate, scrapers, automations, agents, memory, files, compute programs and plugins.

## Non-Goals

- Do not make WASM, Wasmtime, MLIR, gVisor or Firecracker a core dependency.
- Do not claim hardware-level isolation against kernel, CPU, driver or side-channel adversaries.
- Do not create a second brain, plugin marketplace or execution path that bypasses Forge proofs.
- Do not expose filesystem paths, URLs, secrets, native pointers or shell commands to bytecode programs.

## Design

FBC/KASM2 should be narrower than a microVM guest OS:

- no guest OS,
- no syscalls,
- no raw filesystem/network/secrets,
- no native pointers,
- no unbounded loops,
- no undeclared hostcalls.

Programs receive sealed capabilities, not authority:

```text
cap:file:hash
cap:artifact:hash
cap:memory:scope
cap:network:source_id
cap:event:schema
cap:ui:projection
cap:gpu:budget
cap:model:provider_scope
```

Every run must produce a proof envelope:

```text
programHash
bytecodeHash
verifierHash
inputHash
outputHash
capabilityHash
hostcallHash
fuelUsed
memoryPeak
backend
deterministicReplayHash
proofHash
```

Verifier refusals are also runs. They emit the same envelope with `fuelUsed=0`, `memoryPeak=0`, the verifier errors as the denied output payload, and a stable denial `proofHash`.

## Layers

1. Instruction set: scalar, vector, tensor, graph/dataflow, parser, crypto/hash, bounded control flow, event/projection and memory/evidence ops.
2. Capability handles: typed, sealed, non-forgeable, content-addressed where possible.
3. Verifier: rejects unbounded loops, undeclared hostcalls, raw authority, schema mismatch, excess fuel/memory and nondeterministic behavior where determinism is required.
4. Sandbox VM: deterministic interpreter first; no JIT until the verifier and proof story justify it.
5. Optimizer/distiller: fold constants, remove dead ops, fuse operations, cache by content hash, route CPU/GPU and replace repeated LLM/script behavior with shorter bytecode.
6. Backends: interpreter for safety, KASM/Rust for deterministic compute, GPU kernels for heavy numeric work, optional microVM fallback for native code that cannot be expressed as FBC.
7. Proof ledger/projection: return compact hashes, refs, bounded previews and actions; raw data stays in Forge stores.

## First Useful Slice

The first implementation is intentionally small:

- `ForgeBytecodeProgram`
- `ForgeOpcode`
- `ForgeCapability`
- `ForgeHostCall`
- `ForgeVerifierReport`
- `ForgeRunProof`
- `ForgeVmConfig`
- `ForgeVmOutput`
- `ForgeOptimizerReport`
- `ForgeBackendSelection`
- `ForgePipelineOutput`
- `ForgeToolCellSpec`
- `ForgeCompiledToolCell`
- `ForgeHostContext`
- `ForgeCapabilityBinding`
- `ForgeProofLedgerEntry`
- `ForgeToolCellBatchRecord`
- `ForgeToolCellBatchOutput`
- `ForgeToolCellRegistry`
- `ForgeAppRegistry`

Minimum functions:

- build/hash a program,
- verify a program,
- execute in a deterministic interpreter,
- build a proof envelope,
- compile a ToolCell spec into FBC,
- bind ToolCell input through a sealed capability handle,
- read capability bytes only through declared `read_capability`,
- verify capability content hashes before use,
- project a sealed ToolCell dataflow artifact into bounded evidence refs and ranked actions,
- fuse static `push_* -> hash_top` into a pre-hashed constant,
- fuse verified `push_capability -> read_capability -> hash_top` into a pre-hashed constant,
- select a proof-compatible backend,
- emit a compact proof projection JSON,
- emit a compact proof ledger entry,
- execute a ToolCell batch against one shared sealed graph artifact,
- chain per-ToolCell ledger entries into a stable `ledgerRootHash`,
- parse the real-estate ToolCell registry JSON into FBC ToolCell specs,
- execute the full registry as one FBC batch,
- parse `SECTION_OWNERSHIP.json` into app-wide FBC cells for sections and sensitive native commands,
- execute the app-wide registry as one FBC batch outside the real-estate pipeline.

Minimum tests:

- stable hash/proof for `hash_bytes`,
- denied raw filesystem/network capability,
- stable denial proof for verifier refusals,
- stable UI intent transition projection,
- optimizer preserves output while reducing fuel,
- ToolCell pipeline produces stable proof projection,
- ToolCell raw filesystem permission is denied as authority,
- capability reads require host bindings and content-hash parity,
- ToolCell graph artifact projection selects only focused records and excludes unrelated raw sources,
- ToolCell graph artifact tampering is denied before projection,
- proof ledger entries are stable for accepted and denied runs,
- mixed ToolCell batches preserve OK and denied records under one ledger root.
- registry parsing preserves default permissions, denied capabilities, schemas and commands,
- registry batch execution is deterministic across all registered ToolCells.
- app-wide registry execution covers shell, alpha, forge, WebExplorer, real-estate, trading, banger and sensitive bridge commands under one ledger root.

Current lab proof:

```powershell
cargo run --example lab_runner_fbc -- toolcell
cargo run --example lab_runner_fbc -- batch
cargo run --example lab_runner_fbc -- registry
cargo run --example lab_runner_fbc -- app-write
```

The single ToolCell lab exercises `ToolCell -> sealed manifest capability + sealed graph artifact capability -> FBC -> contextual optimizer -> backend selector -> verifier -> interpreter -> bounded evidence/ranked-action projection -> proof projection -> proof ledger`.

The batch lab exercises multiple ToolCells over the same graph artifact, including denied members, and returns a compact batch projection with one `ledgerRootHash`.

The registry lab reads `examples/forge_tauri_ui/source-registry/real-estate-tool-cells.json`, compiles every registered ToolCell into FBC, and emits one registry-level ledger root.

`real-estate-tool-cell-runner.mjs --engine=forge_bytecode_v0 --refresh-fbc` now materializes the Rust FBC registry batch and returns the `.fbc.json` artifact for the requested ToolCell. The real-estate source pipeline's `toolcells` stage uses this path, and the evidence memory builder accepts the FBC artifacts while ignoring the batch manifest.

The app-wide lab reads `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`, compiles every section and sensitive bridge command into FBC cells, and writes outputs under `examples/forge_tauri_ui/.forge-data/forge-app/fbc_outputs`. This is the first non-immo FBC batch path.

The Tauri backend now exposes the same circuit as `forge_fbc_runtime_snapshot`. It reads the app section ownership registry, executes the FBC verifier/interpreter, writes compact artifacts under the Forge store `fbc/app`, appends a unified job-ledger event, and records the snapshot into the kernel proof panel. This makes FBC visible to the whole app without forcing every existing UI/trading/banger/immo path to migrate at once.

The MCP server also exposes the experimental `/fbc_` route through `fbc_runtime`. It runs the same app-wide FBC snapshot from the agent surface and returns only compact hashes, projections and artifact refs. In compact MCP mode it is discoverable through `forge.search` as a hidden/experimental route rather than adding another default visible tool.

The host-side FBC runtime is centralized in `examples/forge_tauri_ui/src-tauri/src/forge_fbc_host.rs`. Tauri commands, kernel projection and MCP reads call this module instead of each inventing their own proof wrapper. It executes the compact FBC program, returns a bounded JSON projection, and appends `fbc_host_ledger.jsonl` events when a Forge store path is available.

The kernel UI command path now routes through that host runtime: every non-snapshot `forge_kernel.apply(op, payload)` asks FBC to emit a bounded `kernel_project` host action, decodes the emitted `{ op, payload }`, records the `proofHash`, and only then mutates and appends the kernel event. The latest FBC proof is exposed in the kernel projection as `lastFbcProof`.

`JobReadProjection` is the second interpreted hostcall. `forge.read_projection { job_id }` now asks the same host runtime to build and verify a `job_read_projection` FBC program, emit a bounded job projection query, and only then select a compact job summary from the store. This is the first `forge.read_projection` path where the read contract itself comes from FBC rather than from ad hoc MCP JSON.

The first sensitive host actions are also guarded before execution, not merely after-the-fact logged: `webexplorer_native_present`, `webexplorer_native_hide`, `bloomberg_live_native_present`, `bloomberg_live_native_hide`, `get_hardware_info`, and `list_forge_jobs` now execute a compact FBC guard, append a host ledger event when the app store is reachable, and record the preverified proof into the kernel proof panel before touching native webviews, hardware projection or job manifests.

`forge.read_projection` can now read `kind:"fbc"` / `ref:"fbc/app"` from the Forge store. It returns the app-wide FBC ledger root, counts and bounded records without returning raw artifacts.

## Rule

FBC wins only if it shortens Forge. If a bytecode layer adds actors without replacing scripts, ad hoc plugins, duplicated MCP routes or fragile UI state, it is architectural drag and should not be promoted.
