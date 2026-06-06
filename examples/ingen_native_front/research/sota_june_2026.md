# SOTA Notes - June 2026

Scope: Migration Front, native Rust + Slint shell, wgpu Banger path and WRY
WebExplorer peripheral.

## Checked Sources

| Source | Finding |
|---|---|
| Slint 1.16.1 release | Current local target is on the latest Slint 1.16 patch line. Keep using Slint as the native shell, but do not assume finished Banger texture interop. |
| Slint Rust docs | Rust integration is stable enough for component callbacks/properties; state must remain Rust-owned and projected into Slint. |
| Slint 1.16 local sources | `KeyBinding` exists with `keys` and `activated`. The correct pinned syntax is `keys: @keys(Control+Tab); activated => { ... }`. |
| wgpu latest docs/releases | Forge should treat wgpu 29 as the local floor for Banger experiments; old examples can be misleading around render-pass/resource APIs. |
| WRY 0.55.1 docs | `WebViewBuilder` supports `build_as_child`, `with_bounds`, `with_navigation_handler`, `with_ipc_handler`, `with_html`; focus, resize and origin policy remain product gates. |
| Slint Rust event-loop docs | Real background jobs must update Slint through the UI event loop boundary. Stage 3 therefore starts with deterministic sync service snapshots before promoting async services. |
| Servo repository/release state | Servo is active again and positions itself as an embeddable Rust web engine, with latest release activity in 2026. Treat it as a future strategic option, not the Stage 0 dependency. |

## Technical Walls

- Design parity: a compiling Slint shell does not preserve the existing Tauri
  product UI by itself.
- GPU interop: Slint shell rendering and Forge/Banger wgpu 29 texture ownership
  still need a proven bridge or explicit fallback route.
- WebView child lifecycle: WRY child views compile, but focus, z-order, resize
  and crash isolation must be proven manually on Windows.
- Web security: WRY HTML/custom-protocol origins differ per platform; the native
  WebExplorer must own navigation policy and never trust page JS as app UI.
- State authority: Slint properties are projection only; the canonical UI state
  must live in Rust and replay deterministically.
- Keyboard shortcuts: use `KeyBinding { keys: @keys(...); activated => { ... } }`
  on the pinned 1.16.1 toolchain. `Ctrl` is invalid; use `Control`.

## Second-Wave Resolution

- Preserve the current Tauri design as an inventory gate before any native
  replacement claim.
- Use Slint only for native app UI; keep WebView2 as a contained peripheral.
- Keep Banger on direct Rust/wgpu, separate from Slint until the texture bridge
  is proven.
- Add a Rust state reducer now so Stage 1 does not become a visual-only mock.
- Promote features only when the report/projection hash can prove the path.
- Use fake direct-service snapshots first; promote real Brain/Monster/WebExplorer
  services only when the event-loop handoff and non-freezing behavior are tested.
- For fake long jobs, use background threads/channels plus non-blocking polling
  from a Slint timer. Promote to real async runtimes only after service-specific
  cancellation, backpressure and UI event-loop handoff are tested.
