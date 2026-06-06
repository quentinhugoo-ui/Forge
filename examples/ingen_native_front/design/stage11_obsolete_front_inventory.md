# Stage 11 Obsolete Front Inventory

Date: 2026-06-06.

Purpose: record the deletion of the global Tauri/WebView application shell after
the native Slint/Rust shell was protected by a rollback point.

Rollback commit before deletion:

```text
9527b7ac Add native Slint front migration
```

## Deletion Candidates

These paths are obsolete for normal product startup:

| Path | Class | Action |
|---|---|---|
| `examples/forge_tauri_ui/ui` | Global WebView UI tree | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/index.html` | HTML app host | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/src` | TypeScript app shell | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/dist` | Generated browser runtime | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/styles.css` | CSS app shell | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/rust-front.html` | Legacy WASM host | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/rust-front-poc.html` | Legacy WASM host | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/front-rs` | Dioxus/WASM front | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/native-front` | Misplaced native front | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/node_modules` | npm front dependencies | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/package.json` | npm front build | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/package-lock.json` | npm front build | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/tsconfig.json` | TypeScript front config | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/scripts/build-ui-runtime.mjs` | Browser runtime build | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/scripts/forge-ui-smoke.mjs` | Legacy WebView smoke | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/scripts/forge-front-rs-cutover-audit.mjs` | Legacy front audit | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md` | Legacy section doc | Deleted 2026-06-06. |
| `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json` | Legacy section doc | Deleted 2026-06-06. |

## Protected Backend Paths

Do not bulk-delete these as part of app-shell cleanup. They contain backend,
runtime or domain logic that must be extracted, moved or deliberately retired:

- `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_brain_runtime.rs`
- `examples/forge_tauri_ui/src-tauri/src/collection_os.rs`
- `examples/forge_tauri_ui/src-tauri/src/trading_core.rs`
- `examples/forge_tauri_ui/src-tauri/src/real_estate_harvester.rs`

Extracted on 2026-06-06:

- `examples/forge_tauri_ui/src-tauri/src/banger_native_engine.rs` ->
  `examples/ingen_native_services/src/banger_native_engine.rs`

## Current Status

Native front cutover is complete:

- `obsoleteFront.deletionReady=true`
- `cutoverReady=true`
- `fullTauriRetirementReady=false`
- obsolete app-shell paths remaining: `0`

Full Tauri backend retirement is still blocked because `5` protected backend
paths above still live under `examples/forge_tauri_ui/src-tauri/src/**`.

## Anti-Regression Rules

- Normal startup must use `examples/ingen_native_front`, not Tauri main-window
  WebView.
- No new product UI source under `examples/forge_tauri_ui/ui/src`.
- No Dioxus/WASM route under `examples/forge_tauri_ui/front-rs`.
- No app-shell HTML/CSS/TypeScript/JavaScript dependency for normal operation.
- WRY/WebView2 remains allowed only for isolated WebExplorer peripheral.
