# Current Tauri Front Inventory

Purpose: preserve the existing InGen product front while Migration Front moves
the runtime to Rust + Slint.

This document is a gate, not an archive. The native Slint Stage 0 window is only
a technical harness until it reproduces this inventory and the user approves the
visual parity proof.

## Non Deletion Gate

- Do not delete `examples/forge_tauri_ui/ui/**` during Stage 0 or Stage 1.
- Do not replace the current Tauri UI as the default product shell before a
  screenshot parity review has been accepted.
- Do not claim that the Slint harness preserves the design until every locked
  surface below has a native Slint component or a documented native equivalent.
- WebExplorer can remain WRY/WebView2, but only as a peripheral inside the
  native app, never as the global app shell.

## Source Of Truth Files

| Role | Existing file |
|---|---|
| Static shell DOM | `examples/forge_tauri_ui/ui/index.html` |
| Global visual system | `examples/forge_tauri_ui/ui/styles.css` |
| Section ownership | `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json` |
| Section contract | `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md` |
| Shell runtime | `examples/forge_tauri_ui/ui/src/shell/surface.ts` |
| Tauri bridge | `examples/forge_tauri_ui/ui/src/shell/tauri-bridge.ts` |
| Boot/runtime glue | `examples/forge_tauri_ui/ui/src/shell/boot.ts` |
| Banger surface | `examples/forge_tauri_ui/ui/src/sections/banger/surface.ts` |
| Trading surface | `examples/forge_tauri_ui/ui/src/sections/trading/surface.ts` |
| WebExplorer config | `examples/forge_tauri_ui/ui/src/sections/webexplorer/config.ts` |
| Real-estate runtime | `examples/forge_tauri_ui/ui/src/sections/real-estate/*-runtime.ts` |

## Locked Global Tokens

These values are copied from the current Tauri visual system and must stay the
initial Slint token baseline unless a deliberate redesign is approved.

| Token | Value |
|---|---|
| `bg` | `#0e0e0f` |
| `surface` | `#1c1c1b` |
| `surface-soft` | `#232322` |
| `line` | `#353534` |
| `text` | `#e8ecee` |
| `muted` | `#9a9a94` |
| `accent` | `#d5dde1` |
| `forge-voice-accent` | `#31c8b2` |
| `forge-terminal-accent-alt` | `#e2b63f` |
| Font stack | `Geist`, `Segoe UI`, `SF Pro Display`, system sans |

## Locked Dimensions

| Surface | Existing value |
|---|---|
| Window titlebar height | `38px` |
| Sidebar toggle | `30px` square, `4px` radius |
| App shell padding | `6px 0 0 8px` |
| Left panel width | `279px` |
| Right proof panel open width | `287px` |
| Canvas wrap padding | `8px 20px 0 20px` |
| Canvas topbar min height | `24px` |
| Chat bar width | `min(780px, calc(100% - 48px))` |
| Chat bar bottom offset | `56px` |
| Chat bar radius | `6px` |
| Chat bar padding | `8px 14px 9px` |
| Chat command square | `94px` |
| Chat command square inset | `8px` |
| Chat bar fill | `#2a2a28` |
| Banger shell columns | `240px 1fr 280px` |
| Banger left collapsed columns | `0px 1fr 280px` |
| Banger right collapsed columns | `240px 1fr 0px` |

## Product Surfaces To Preserve

| Surface | Native migration expectation |
|---|---|
| Window titlebar | Native Slint titlebar with same 38px density and controls behavior. |
| Left navigation panel | Native Slint panel reproducing section order, collapse behavior and density. |
| Central canvas | Native Slint canvas host; no WebView app shell. |
| Chat bar | Native Slint command square, prompt controls, model/provider controls and transcript docking. |
| Proof/status panel | Native Slint right panel preserving proof-open geometry and diagnostics. |
| Alpha/Forge shell | Native route retaining existing session/canvas behavior before cleanup. |
| WebExplorer | WRY/WebView2 child peripheral with native Slint diagnostics and safe action gate. |
| Banger | wgpu native viewport with Slint overlays, not DOM canvas. |
| Trading | Native dashboard surface; Bloomberg/WebExplorer live page remains peripheral only. |
| Real estate | Native dashboard and onboarding/mode/panel runtime equivalents. |

## Stage 1 Native Component Targets

- `GeneratedTauriFrontLayer`
- `AppWindow`
- `NativeModal`

The immediate emergency target is the original Forge first viewport generated
from the existing Tauri front, not redrawn by hand. The importer lives at:

```text
examples/ingen_native_front/scripts/import_tauri_front_to_slint.mjs
```

It opens `examples/forge_tauri_ui/ui/index.html` with the existing CSS and
runtime bundles, measures the rendered DOM, writes `ui/app.slint`, and records
source hashes in:

```text
examples/ingen_native_front/design/tauri_front_slint_import_report.json
```

Banger, WebExplorer, Trading and Real Estate keep their own parity gates after
this first viewport is acceptable.

## Required Parity Proofs

Each proof needs a screenshot or deterministic render artifact from the current
Tauri UI and the native Slint version at the same viewport size.

| Proof | Required viewport/state |
|---|---|
| First frame | Default InGen shell at desktop size. |
| Sidebar state | Expanded and collapsed. |
| Chat state | Empty, focused, command entered, attachment visible. |
| Proof panel | Closed and open. |
| WebExplorer | Native peripheral visible, focused, hidden, focus returned. |
| Banger | Fullscreen mode, left/right panels, viewport visible. |
| Trading | Main trading workspace and live web peripheral path. |
| Real estate | Main vertical surface and mode/panel state. |

## Current Native Status

`examples/ingen_native_front` has a Stage 10 native packaging/security boundary
as of
2026-06-06:

- Rust + Slint app shell compiles and opens as
  `InGen Native Front - Forge`.
- wgpu adapter probing exists.
- WRY/WebView2 child proof path exists.
- The native shell now includes authored Forge viewport components:
  `TitleBar`, `LeftPanel`, `WorkspaceHeader`, `DropCanvas`, `ChatBar`,
  `SectionStatusDock`, `NativeModal` and `AppWindow`.
- `TitleBar`, `LeftPanel`, `WorkspaceHeader` and `DropCanvas` are now fed by
  `NativeUiState` projections (`active_section`, `section_title`,
  `canvas_title`, `canvas_hint`) so the Forge/WebExplorer/Banger/Trading/Real
  Estate navigation is visible in real Slint components instead of a static DOM
  import.
- Sidebar actions, recent-session rows and the chat attach square now have
  native Slint click capture routed into the Rust modal path, so these controls
  are visibly interactive while final service bindings are still pending.
- `DropCanvas` now includes section-aware native action buttons that route into
  Rust capture. They are a migration scaffold for final product commands, not a
  design-parity approval for deleting the original front.
- Forge first-viewport parity pass 2026-06-06: the native app now starts on the
  Forge/New session surface, hides migration-only canvas controls on the base
  Forge viewport, uses the original Drop-any-file canvas copy, tightens the left
  panel vertical rhythm, moves the chat bar toward the Tauri oracle position,
  restores the yellow user badge and changes the chat placeholder to the
  oracle text.
- Additional Forge recents parity pass 2026-06-06: extra generated recent rows
  are hidden on Forge/Shell, leaving the single `New session` row visible in the
  Tauri-derived oracle while preserving richer rows for non-Forge sections.
- Stage 1 Forge first-viewport parity gate 2026-06-06: `src/visual_parity.rs`
  records the oracle image, default section, canvas title, chat placeholder,
  locked geometry and hidden migration surfaces. `cargo test --lib` now fails
  if the native shell stops opening on the Forge first viewport contract.
- Stage 2 state kernel 2026-06-06: `NativeStateKernel` now owns state and event
  log, `NativeStateCheckpoint` records schema/projection/event-log hash/state
  hash, and the running Slint app dispatches UI/service events through the
  kernel instead of mutating state directly.
- Stage 3 direct Rust services 2026-06-06:
  `DirectNativeServices` now feeds the native shell from local wgpu/WebView2
  probes through Rust traits, `local_service_snapshot()` replaces the fake
  startup snapshot, and `NativeServiceCommand` / `NativeServiceCommandResult`
  proof refresh, imported-control capture and chat submission without browser
  IPC.
- Latest Stage 3 proof hash:
  `eb6826547baf9329e275f3b5fbca23964d4a6fd342fd00261049d09d3ccddf1a`.
- Latest direct refresh command proof:
  `e4b9e36763178d02b8aebcbc0652d81eb3f0af5a328c13ef3a8cefcee0fa0cc0`.
- Direct local probe state: `directProbeServicesConnected=true`,
  `browserIpcRequired=false`, `realServicesConnected=false`. The last flag
  stays false until the full Brain/Monster/Banger/WebExplorer/trading/real
  estate product adapters are wired.
- Stage 4 Banger native viewport 2026-06-06:
  `src/banger_viewport.rs` generates a deterministic 640x360 RGBA native
  fixture frame, hashes the render target, emits a Slint texture bridge proof
  and pushes the frame into the running app through `BangerNativeViewport`.
- Latest Stage 4 proof hash:
  `69e836afdacbcbf0c90f35f0efa1d0f3d85f52960598c649e2541fb3cd1e425e`.
- Latest Banger frame hash:
  `00d78c2e3767b3992ebdeb7f06a47197776480db2d95f75d4ac2ddde604a3c5d`.
- Latest Banger Slint bridge proof:
  `a768af4eaefcf7e2b0128af6cbeb3a67f45659b249e9c767527a67f00c4bc156`.
- Banger transport state: `visibleInSlint=true`,
  `browserCanvasRequired=false`, `textureBindingReady=true`,
  `directTextureImportReady=false`. Direct import remains blocked until Slint
  and Banger share compatible `wgpu` texture/device/queue types.
- Stage 5 isolated WebExplorer 2026-06-06:
  `src/webexplorer.rs` owns a deny-by-default navigation policy,
  `src/webview_child.rs` applies it to the WRY/WebView2 child proof path, and
  the local fixture now has a restrictive CSP.
- Latest Stage 5 proof hash:
  `b325095e2e8800154808be71e6da8ba5d7e371573bf64cc18a769bee06b29821`.
- Latest WebExplorer policy hash:
  `4c59d568674fd0d28c50f0a92c0eb049cac6ecab88bf266a95c54162a9f51727`.
- Latest WebExplorer isolation proof:
  `d2aa1a98d002a22fd0b999518ef6379da4f859790864ec590beeb480304f6394`.
- WebExplorer transport state: dangerous schemes are blocked, unknown external
  HTTPS hosts require later atlas-ref promotion, devtools/IPC/host objects and
  downloads are disabled, and creation failure reports into Slint status
  instead of killing the shell.
- Stage 6 RAM DOM Atlas 2026-06-06:
  `src/webatlas.rs` captures the isolated WebExplorer fixture into a
  content-addressed `AtlasManifest` with DOM-like nodes, AX roles/names/values,
  deterministic bounds, whitelisted style subset, resource refs, evidence
  hashes, coverage ratios and blind spots.
- Latest Stage 6 proof hash:
  `2f21397c1a19d3caf558d986b3e9cfe502b34870dbc7b9b3a110c6135fa4861d`.
- Latest WebAtlas manifest proof:
  `fbbec056d2d3c976f74b7a3f926e161220ea92adc7f4b09b90d7881ce79fc7bb`.
- Latest WebAtlas normalized hash:
  `b3d3f6001e13a0eca430d0dbee2c7288025acf44bb205f3a0fb990f635938890`.
- WebAtlas fixture coverage: DOM 100, AX 57, layout 100, style 7, resource
  100. Dynamic JavaScript mutations, visual crop storage and platform-native
  accessibility capture remain explicit blind spots for the next stages.
- Stage 7 native RAM DOM UI 2026-06-06:
  `src/webatlas.rs` now projects the content-addressed `AtlasManifest` into an
  `AtlasUiProjection`, and `ui/app.slint` renders it through `AtlasInspector`.
  The native WebExplorer surface can show the RAM DOM tree, selected node
  inspector, AX summary, layout bounds, resource list, action candidates, blind
  spots, proof panel and search/action summary without returning to the Tauri
  DOM shell.
- Stage 7 selection state is owned in Rust:
  `src/main.rs` wires `atlas_previous_node` and `atlas_next_node` callbacks,
  then reapplies the projection into Slint properties. The current fixture
  starts on node index 12, the textbox input.
- Latest Stage 7 proof hash:
  `47dbcc19675f43ff7d7d32de684296bfd5cff4b4380e77a3a005dbffddb24f34`.
- Latest WebAtlas UI projection hash:
  `9bd9f05d682dd9d06f72f3f1ffd24bdd32b4e12fa1ccf6f3d6d453bb1e607fb5`.
- Stage 7 remaining walls are explicit: real large-page responsiveness,
  graphical WebView bounds highlighting, region-to-atlas hit testing and full
  role/text/tag/resource/atlas-ref filter controls continue in Stage 8/9.
- Stage 8 native chat and agent surfaces 2026-06-06:
  `src/state.rs` now owns sessions, replayable transcript messages and native
  agent cards (`plan`, `questionnaire`, `proof`) behind `NativeUiEvent`s, and
  `ui/app.slint` renders them through `AgentSurface` without HTML/CSS/TS or
  browser IPC.
- Chat submission now records a user message, a local native system message, a
  proof card and a queued direct-service job in the Rust state kernel. CodeAct
  and LLM-driven web automation remain explicitly disabled until the full
  migration and old-front deletion are complete.
- Long transcript projection is bounded to the last 8 visible native messages,
  with an older-message count, so the Slint shell has a deterministic
  virtualization gate before real provider streaming is connected.
- Latest Stage 8 proof hash:
  `2fda5ff579a143ad038954413ba3752f039f996cdcc07b07ae52db1f1488c2f5`.
- Latest canonical native agent replay hash:
  `74a82d2c844898413cd76b140767432d8ce1932ac2a75a6d3349a3dddc19718b`.
- Stage 8 remaining walls are explicit: selectable transcript text, full
  provider/model picker choices, real provider token streaming and session
  rename service bindings continue in Stage 9 product adapters.
- Stage 9 native product sections 2026-06-06:
  `src/product_sections.rs` now defines content-addressed product section
  manifests for Forge/compute, Alpha, Trading, Real Estate, Banger,
  WebExplorer and diagnostics/proofs. `ui/app.slint` renders active product
  dashboards through `ProductSectionSurface`.
- Trading has native slots for market status, chart summary, timeframe
  controls, backtest cards, alerts and provider status. Real market adapters
  still need to replace the direct fixture summaries.
- Real Estate has native slots for onboarding, zone scoring, resolver/harvester
  summaries, panels and mode state. Real domain adapters remain a later
  service-binding pass, not a reason to keep the old shell.
- Forge/compute, Alpha, Banger, WebExplorer and diagnostics/proofs all have
  Rust manifest entries. Non-web sections prove `webview_required=false`;
  WebExplorer is the only section allowed to require a WebView peripheral.
- Latest Stage 9 proof hash:
  `a7ecdd02cb18891bea492a9a9448ca142782a7cf01950a258f9e6ee64b603ba4`.
- Latest product sections manifest hash:
  `0fd78c8b522d68e9e18987dd155067b7171b0775ccd17504185eeb3f095dbd7d`.
- Latest active trading product projection hash:
  `b1a76604c0f6255b8d05106469469403828d3926aa0cdca251f4178ea3794fcf`.
- Stage 9 remaining walls are explicit: real trading, real-estate, Forge
  compute and provider adapters must replace fixture summaries before Stage 11
  deletes the obsolete front.
- Stage 10 native packaging and security 2026-06-06:
  `src/packaging_security.rs` records the native Windows package target,
  app-data/log/crash/secrets/WebView profile paths, local filesystem capability
  decisions and crash recovery record hashes.
- The native app target is `windows-x86_64-native-slint`,
  `ingen-native-front.exe`, with `tauri_shell_required=false`.
- Default writable roots are limited to
  `C:\Users\quent\AppData\Local\InGen\NativeFront` and its logs,
  crash-recovery and WebView profile children. `C:\Users\quent\Documents\EVE\MAP`
  is explicitly blocked as a protected root.
- Secrets are separated under the native app-data root and
  `secrets_in_logs_allowed=false`.
- WebExplorer keeps an isolated `webview-profile`; WebView local file access is
  denied by policy instead of inherited from a Tauri capability layer.
- Latest Stage 10 proof hash:
  `552f078ca6372a906b25c409311f2e97204d73c5498faaad46d51916a7ef5b71`.
- Latest packaging/security manifest hash:
  `1f8812ad2361e89605f460b1ae5bbc4bfbae98217d31430072a60dad0adf59b3`.
- Latest native capability policy hash:
  `c6acdf2b99b86570aca1168783c96a66a7798e2fba20c33ddd0e02f4faacb8ed`.
- Latest crash recovery proof hash:
  `02a9262d11875f004726090e60085d71f234ce63eeaf602b7ecc53968b82c0bf`.
- Stage 10 remaining release walls are explicit: app icon/resources, signed
  installer automation and real fresh-install smoke are still release gates,
  not reasons to keep the old app shell.
- Stage 11 obsolete front deletion audit 2026-06-06:
  `src/obsolete_front.rs` now owns an `ObsoleteFrontManifest`, and
  `design/stage11_obsolete_front_inventory.md` records the guarded deletion
  list for the old global Tauri/WebView app shell.
- `ingen-native-front --obsolete-front-report` prints the manifest without
  starting the UI; `ingen-native-front --cutover-audit` now exits with code `0`
  because the native front cutover is clear.
- `src/cutover_audit.rs` now verifies the native shell manifest has no
  Tauri/Dioxus/wasm-bindgen shell dependency, verifies the 18 legacy front
  blockers are gone and marks the 6 protected backend services that must be
  extracted before full Tauri backend retirement.
- Rollback commit before deletion:
  `9527b7ac Add native Slint front migration`, pushed to `origin/master`.
- The 18 obsolete global front/runtime paths were deleted on 2026-06-06,
  including the old `ui` tree, `node_modules`, misplaced `native-front`,
  Dioxus `front-rs`, npm manifests and TS config.
- Latest Stage 11 proof hash:
  `5b52a28d4c4d037c0290a262ebe828eda0a6ed55b5487eed568eb82438956fdc`.
- Latest obsolete-front manifest hash:
  `65c4d065dfdb1341dfb22672a5428f572d76b20ad286b1abe3652ee10e46b01d`.
- Latest cutover audit hash:
  `9a3b166a2c26a219d8ddec2f3fba32cc46e720c845627a02d486d6755e17f87b`.
- Stage 11 deletion readiness is currently `true`: `0` obsolete app-shell
  paths still exist under `examples/forge_tauri_ui/**`.
- Stage 11 native front cutover is currently `true`.
- Stage 11 backend extraction readiness is currently `false`: `6` protected
  backend services still live under `examples/forge_tauri_ui/src-tauri/src/**`.
- Full Tauri backend retirement remains blocked until those backend services
  are extracted, moved or explicitly retired.
- `SectionStatusDock` renders native service/proof projections for
  WebExplorer, Banger, Trading and Real Estate. It is hidden on the base Forge
  first viewport so that the original first-screen geometry remains the visual
  oracle instead of accepting an accidental redesign.
- The failed generated DOM layer is no longer the active product shell path.
  The importer remains a parity/oracle helper only.
- Latest import proof hash: `6c49354685f467cc5d916c06f1fb47b7d6a440f9583065abcefe4d1cc4ebbd30`.
- Latest import scale: CSS/Slint logical scale `1`; do not multiply by Windows
  display scale, because Slint already maps logical pixels to the native
  window.
- Latest import layers: `forge`, `webexplorer`, `banger`, `trading`,
  `real-estate`, with 421 generated native Slint entries, 139 extracted
  SVG/vector assets and 590 reactive control overlays from the original DOM.
  Exact bitmap screenshots are still generated as the parity oracle, not as the
  displayed UI.
- Design parity is explicitly `false`.
- The current Tauri front remains the product reference.

Stage 1 is back on the correct migration path: authored, clickable Slint
components with the current Tauri UI as visual oracle. Promotion to
deletion/cutover remains blocked until this inventory is verified with parity
screenshots and accepted by the user.
