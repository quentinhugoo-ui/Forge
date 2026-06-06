# Stage 11 Obsolete Front Inventory

Date: 2026-06-06.

Purpose: delete the global Tauri/WebView application shell only after the native
Slint/Rust shell is protected by a rollback point and the old backend services
that still matter are extracted or explicitly kept.

## Deletion Candidates

These paths are obsolete for normal product startup:

| Path | Class | Action |
|---|---|---|
| `examples/forge_tauri_ui/ui/index.html` | HTML app host | Delete after rollback commit. |
| `examples/forge_tauri_ui/ui/src` | TypeScript app shell | Delete after rollback commit. |
| `examples/forge_tauri_ui/ui/dist` | Generated browser runtime | Delete after rollback commit. |
| `examples/forge_tauri_ui/ui/styles.css` | CSS app shell | Delete after rollback commit. |
| `examples/forge_tauri_ui/ui/rust-front.html` | Legacy WASM host | Delete after rollback commit. |
| `examples/forge_tauri_ui/ui/rust-front-poc.html` | Legacy WASM host | Delete after rollback commit. |
| `examples/forge_tauri_ui/front-rs` | Dioxus/WASM front | Delete after rollback commit. |
| `examples/forge_tauri_ui/package.json` | npm front build | Delete after rollback commit. |
| `examples/forge_tauri_ui/package-lock.json` | npm front build | Delete after rollback commit. |
| `examples/forge_tauri_ui/scripts/build-ui-runtime.mjs` | Browser runtime build | Delete after rollback commit. |
| `examples/forge_tauri_ui/scripts/forge-ui-smoke.mjs` | Legacy WebView smoke | Delete after rollback commit. |
| `examples/forge_tauri_ui/scripts/forge-front-rs-cutover-audit.mjs` | Legacy front audit | Delete after rollback commit. |

## Protected Backend Paths

Do not bulk-delete these as part of app-shell cleanup. They contain backend,
runtime or domain logic that must be extracted, moved or deliberately retired:

- `examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_brain_runtime.rs`
- `examples/forge_tauri_ui/src-tauri/src/collection_os.rs`
- `examples/forge_tauri_ui/src-tauri/src/banger_native_engine.rs`
- `examples/forge_tauri_ui/src-tauri/src/trading_core.rs`
- `examples/forge_tauri_ui/src-tauri/src/real_estate_harvester.rs`

## Current Blocker

Deletion is not safe yet because `examples/forge_tauri_ui/**` contains many
modified and untracked files in the current worktree. Removing those paths now
would destroy work that is not isolated in a rollback commit.

Stage 11 must therefore proceed in this order:

1. Commit or otherwise preserve the native migration work and any valuable
   old-front changes.
2. Extract protected backend services that still matter.
3. Delete only the obsolete app-shell paths above.
4. Run the Stage 11 anti-regression audit.

## Anti-Regression Rules

- Normal startup must use `examples/ingen_native_front`, not Tauri main-window
  WebView.
- No new product UI source under `examples/forge_tauri_ui/ui/src`.
- No Dioxus/WASM route under `examples/forge_tauri_ui/front-rs`.
- No app-shell HTML/CSS/TypeScript/JavaScript dependency for normal operation.
- WRY/WebView2 remains allowed only for isolated WebExplorer peripheral.
