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

### 2. MCP Surface Reduction

### 3. ForgeSlash Intent Language

### 4. Distillation, Cache And Mini-Clone Path

### 5. Brain, Memory And Godel

### 6. KASM Interop And Proprietary Bytecode

### 7. FBC/KASM Sandbox Guards

### 8. Tauri UI And TypeScript Migration

### 9. Trading Compute Surface

### 10. Real Estate And Provider Intelligence

### 11. Repo Compression And Safety

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
- Integrated LLM provider floor is Claude Sonnet/Opus 4.6+, GPT 5.3 Codex+, and Gemini 3+ for authoring direct runtime/sandbox programs.
- Collection OS kernel exists at `examples/forge_tauri_ui/src-tauri/src/collection_os.rs`; real-estate onboarding consumes its plan route and emits collection proof fields.
- WebExplorer can compile observed UI nodes into bounded collection commands, cache action choices and replay validated actions through the native bridge.
- HTTP/onboarding fetch paths classify rate limits, CAPTCHA, access-denied and empty-render blocks into `BlockProof` instead of trying to bypass them.
- FBC app-wide lab covers shell, alpha, forge, WebExplorer, real-estate, real-estate-main, trading, banger and sensitive commands under one ledger root.
- KASM interop importer exists in `src/kasm.rs::interop`; it parses WIT-like contracts, lowers simple MLIR `func.func`/`arith.*`/constant-bound `scf.for` into KASM, and refuses rich WIT ABI types without Forge-owned lowering.
- Runtime/cache stores stay out of Git; protected path remains `C:\Users\quent\Documents\EVE\MAP`.
