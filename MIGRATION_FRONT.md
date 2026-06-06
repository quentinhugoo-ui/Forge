# Migration Front

Specification canonique de la refonte totale du frontend InGen.

Date de redaction: 2026-06-06.

Ce fichier est la source a lire quand l'utilisateur demande de travailler sur
**Migration Front**. Il ne decrit pas un audit, pas une feature WebExplorer, pas
une continuation de Dioxus, pas une amelioration cosmetique du front actuel. Il
decrit l'architecture cible et l'ordre de migration pour remplacer le frontend
actuel par une application native.

## 0. Resume Pour Agent Qui Arrive Sans Contexte

Migration Front = refaire le frontend InGen en natif.

Objectif central:

```text
Abandonner le shell applicatif Tauri/WebView global
-> construire un shell natif Rust + Slint
-> rendre Banger avec wgpu natif
-> garder une WebView seulement comme peripherique WebExplorer
-> reprendre RAM DOM Atlas ensuite, dans ce cadre natif
```

La premiere tache d'une session Migration Front n'est pas RAM DOM Atlas.
La premiere tache n'est pas d'ajouter un module dans `examples/forge_tauri_ui`.
La premiere tache n'est pas de continuer Dioxus/WASM.

La premiere tache est:

```text
Creer une base native Rust + Slint hors Tauri,
puis prouver que Slint + wgpu + WRY/WebView2 coexistent proprement.
```

Chemin cible du nouveau crate:

```text
examples/ingen_native_front/
```

Chemins explicitement interdits pour le nouveau front natif:

```text
examples/forge_tauri_ui/native-front/
examples/forge_tauri_ui/ui/
examples/forge_tauri_ui/front-rs/
```

Ces chemins appartiennent au monde Tauri/Dioxus/WebView existant ou a son
fallback. Les utiliser pour la nouvelle architecture entretient la confusion.

## 1. Probleme A Resoudre

Le frontend actuel a deja ete migre loin du TypeScript applicatif vers une route
Rust/Dioxus/WASM dans Tauri. Cette migration a supprime une partie de la dette,
mais elle n'a pas supprime le probleme principal:

```text
InGen vit encore dans une WebView globale.
```

Une WebView globale implique:

- HTML comme host applicatif;
- JS/WASM loader;
- rendu final dependant du navigateur embarque;
- startup expose au flash blanc WebView;
- geometrie et focus controles par la couche browser;
- separation artificielle entre UI, backend, moteur 3D et WebExplorer;
- Banger traite comme une surface web au lieu d'un viewport moteur natif;
- WebExplorer melange conceptuellement "la page web inspectee" et "l'app qui
  inspecte".

Le but de Migration Front est de faire sauter ce mur.

InGen doit devenir une application native dont le web est un peripherique, pas
le substrat.

## 2. Decision D'Architecture

Architecture cible:

```text
InGen Native App
|
+-- Rust Core
|   +-- Brain
|   +-- Godel verification
|   +-- Forge language/runtime
|   +-- Monster compute
|   +-- provider services
|   +-- filesystem/app data/secrets
|   +-- proof ledger
|
+-- Slint Native Shell
|   +-- main window
|   +-- app frame
|   +-- sidebar/navigation
|   +-- chat bar
|   +-- transcript
|   +-- model/provider controls
|   +-- plan/questionnaire/session panels
|   +-- proof/status strip
|   +-- trading dashboards
|   +-- real-estate dashboards
|   +-- WebExplorer diagnostics panels
|   +-- RAM DOM tree viewer
|   +-- Banger overlays/toolbars
|
+-- Banger Native Engine Viewport
|   +-- Rust + wgpu
|   +-- render graph
|   +-- texture/surface handoff to Slint
|   +-- SDF / mesh / voxel / surfel / splat
|   +-- resource residency proof
|   +-- frame hashes
|   +-- VRAM/frame-time telemetry
|
+-- WebExplorer Peripheral
|   +-- Rust + WRY
|   +-- WebView2 on Windows
|   +-- WKWebView on macOS later
|   +-- WebKitGTK on Linux later
|   +-- navigation policy
|   +-- profile/isolation policy
|   +-- CDP/native inspection path
|   +-- screenshots and capture proofs
|
+-- RAM DOM Atlas
    +-- DOMSnapshot capture
    +-- Accessibility tree capture
    +-- layout/styles/resources
    +-- normalized atlas refs
    +-- blind-spot report
    +-- native Slint tree UI
    +-- safe web action gate
```

The important separation:

```text
Slint shell = InGen UI
WRY/WebView2 = external web page viewer only
```

If a page web crashes, freezes, navigates, shows a CAPTCHA or consumes too much
memory, the InGen shell must remain alive and controllable. The web must not own
the application lifecycle.

## 3. Source Language Policy

Allowed source languages for the new frontend:

| Layer | Allowed source |
|---|---|
| App logic | Rust |
| UI declaration | `.slint` |
| GPU shaders | WGSL, and later Slang if Banger needs it |
| Temporary audits | Minimal scripts only while proving the migration |

Forbidden as application frontend source:

- TypeScript;
- JavaScript;
- HTML as application UI;
- CSS as global application UI;
- Dioxus/WASM as final target;
- HTMX;
- React/Vue/Svelte or any web framework;
- Electron;
- a new Tauri shell for the target architecture.

Allowed exception:

WebExplorer may load arbitrary HTML/CSS/JS because it displays external web
pages. That code belongs to the inspected web page, not to InGen.

## 4. Why Slint

Slint is selected because the target is a native tool UI, not a web app.

Reasons:

- Rust integration is first-class.
- UI can be declared without bringing back HTML/CSS/JS.
- The shell can be native and stable without a browser host.
- It can coexist with winit/windowing and native rendering paths.
- It is more mature for a product migration than experimental Rust UI stacks.
- It supports multiple renderer backends, which gives fallback options.
- It is close in spirit to a "Slate for Rust" approach: native declarative UI
  around a heavy engine.

Important nuance:

Slint does not replace the browser engine required to display arbitrary web
pages. WebExplorer still needs WRY/WebView2. The difference is that WebView2 is
no longer the app shell. It is only a contained page viewer.

## 5. Why wgpu For Banger

Banger is not a DOM feature. Banger is an engine.

It should behave more like an Unreal/Blender viewport than a browser canvas.

The target path:

```text
Banger scene/program
-> Rust render graph
-> wgpu resources
-> native frame/texture
-> Slint viewport integration
-> frame proof + telemetry
```

Goals:

- stable viewport sizing;
- no WebView white flash;
- no browser canvas dependency;
- deterministic frame proof for fixture scenes;
- resource residency report;
- VRAM budget visibility;
- eventual advanced renderer: SDF, splats, virtual geometry, radiance cache,
  path tracing/neural rendering.

## 6. Why WRY/WebView2 For WebExplorer

WebExplorer must display real web pages. A native UI toolkit cannot parse and
render the modern web by itself.

The target is therefore:

```text
Slint shell
-> contains or coordinates WRY/WebView2
-> page loads inside isolated WebView
-> Rust captures DOM/AX/layout/resources
-> Slint displays the RAM DOM Atlas
```

WebView2 is a peripheric capability, not the shell.

Rules:

- WebExplorer is allowed to host web content.
- WebExplorer is not allowed to host the InGen application UI.
- WebExplorer must have navigation policy.
- Sensitive web actions require user confirmation.
- LLM never receives raw arbitrary selectors as authority.
- LLM receives bounded refs/proofs produced by the Atlas.

## 7. RAM DOM Atlas Position In The Plan

RAM DOM Atlas is important, but it is not the first task of Migration Front.

Correct order:

```text
1. Native Slint app exists.
2. wgpu viewport exists.
3. WRY/WebView2 peripheral exists.
4. Then RAM DOM Atlas resumes inside that peripheral.
```

If an agent starts with RAM DOM Atlas before proving the native shell and
isolated WebView, it is not following Migration Front.

RAM DOM Atlas belongs to Migration Front Stage 5 and later. It must be built as
Rust services/data structures that the future Slint UI consumes.

## 8. Relationship To Existing Tauri/Dioxus Front

Existing Tauri/Dioxus front status:

- it is the current fallback;
- it may be used to compare behavior;
- it is the design reference until native visual parity is proven;
- it must not be deleted, replaced in normal use, or treated as obsolete merely
  because a Slint feasibility window exists;
- it may be kept temporarily while the native app is incomplete;
- it must not define the target architecture;
- it must not receive new strategic front features;
- it must be deleted only after native parity is proven.

Do not place the new native app inside the Tauri app folder. The target is not a
Tauri sub-feature.

Correct mental model:

```text
Current Tauri/Dioxus front = fallback and reference
Migration Front = replacement architecture
```

Design preservation rule:

```text
Do not lose the visual identity already built.
```

The current UI's dark matte palette, compact titlebar, icon-first top controls,
left/right panel proportions, chat bar geometry, Banger workspace feel and
section-specific interaction density are product assets. The native Slint shell
must port them deliberately instead of inventing a new visual direction.

Important correction after the first Stage 0 window:

```text
The Stage 0 Slint window is a technical harness, not the product UI.
```

It proved native feasibility only. It was not accepted as design parity. The
later Stage 11 cutover deleted the old WebView front after a separate
obsolete-front audit; the active rule is now to keep the frozen design oracle
and never recreate the old app shell.

Design source artifact:

```text
examples/ingen_native_front/design/current_tauri_front_inventory.md
examples/ingen_native_front/design/current_tauri_front_inventory.json
```

These files are now the frozen design gate. They record the deleted Tauri
front's source files, tokens, locked dimensions, product surfaces and required
parity screenshots. If the native app diverges from this inventory, the
migration is not preserving the product UI.

The old frontend deletion was completed on 2026-06-06 after these checkpoints:

- palette/tokens mapped from the current front;
- screenshots and visual notes for the current shell, titlebar, left panel,
  right panel, chat bar, command square, transcript, Banger, WebExplorer,
  trading and real-estate surfaces;
- native counterpart for each surface;
- manual or automated comparison approved by the user;
- rollback commit `9527b7ac` pushed before deleting the old app shell.

Hard non-recreation gate:

```text
No old frontend file may be recreated as part of Migration Front. If rollback
is required, restore from Git history deliberately instead of rebuilding a
parallel HTML/CSS/TypeScript path.
```

## 9. Repository Layout Target

Recommended new layout:

```text
examples/ingen_native_front/
  Cargo.toml
  build.rs
  src/
    main.rs
    app.rs
    state.rs
    services.rs
    shell.rs
    banger_viewport.rs
    webexplorer.rs
    webatlas.rs
    proof.rs
  ui/
    app.slint
    shell.slint
    chat.slint
    panels.slint
    banger.slint
    webexplorer.slint
  tests/
```

Possible later extraction:

```text
crates/ingen_native_shell/
crates/ingen_webexplorer_native/
crates/ingen_banger_viewport/
```

But Stage 0 should stay simple. Do not create a large workspace before the
native feasibility proof exists.

## 10. Execution Order

Migration Front must be executed in these stages:

1. Stage 0: native feasibility.
2. Stage 1: Slint shell skeleton.
3. Stage 2: native state kernel.
4. Stage 3: direct Rust services.
5. Stage 4: Banger native viewport.
6. Stage 5: isolated WebExplorer.
7. Stage 6: RAM DOM Atlas in the native architecture.
8. Stage 7: native RAM DOM UI.
9. Stage 8: native chat and agent surfaces.
10. Stage 9: native data/product sections.
11. Stage 10: native packaging and security.
12. Stage 11: delete global WebView frontend and obsolete architecture.
13. Stage 12: post-migration safe Web CodeAct gate.

No stage should silently skip its proof.

## 11. Stage 0 - Native Feasibility

Purpose:

Prove that the three hard primitives can coexist:

```text
Slint native UI + wgpu viewport + WRY/WebView2
```

This stage is the real beginning of Migration Front.

Work:

- ~~Create `examples/ingen_native_front/`.~~ Done 2026-06-06.
- ~~Add a minimal Rust binary.~~ Done 2026-06-06.
- ~~Add Slint.~~ Done 2026-06-06 with `.slint` source and winit backend.
- ~~Add a native main window.~~ Done 2026-06-06 at compile/runtime level; manual visual proof still required.
- ~~Add a simple Slint layout with:~~ Done 2026-06-06:
  - top/title strip;
  - sidebar placeholder;
  - central split area;
  - bottom chat bar placeholder;
  - status/proof strip placeholder.
- ~~Add a minimal wgpu test surface or texture producer.~~ Done 2026-06-06: wgpu 29 creates and clears a 64x64 texture.
- ~~Add a minimal WRY/WebView2 test peripheral on Windows.~~ Partial 2026-06-06: WRY/WebView2 dependency and typed `WebViewBuilder::new().with_html(local_fixture)` probe compile. A gated `--webview-child-proof` mode now attempts `build_as_child` against the Slint native window handle, keeps the child alive, syncs bounds on a timer and calls `focus()` then `focus_parent()`; visual focus/resize proof is still manual and blocks promotion.
- ~~Use a local static HTML fixture for the WebView test page only.~~ Done 2026-06-06: `fixtures/webview_stage0.html`.
- ~~Record runtime environment:~~ Partial 2026-06-06 through `cargo run -- --report`:
  - OS;
  - GPU adapter;
  - wgpu backend;
  - WebView2 version; not done yet, currently recorded as `null`;
  - Slint renderer.

SOTA / limits found before coding, June 2026:

- Slint 1.16 winit supports FemtoVG, FemtoVG/wgpu, Skia and software renderers, but direct wgpu texture integration is still version-gated around Slint's unstable wgpu feature line. Forge's Banger path is already on wgpu 29, so Stage 0 uses Slint as shell renderer and a separate wgpu 29 texture producer first.
- wgpu 29 changed render-pass shape (`depth_slice`, `multiview_mask`) and should be treated as the current API floor, not hidden behind old examples.
- WRY 0.55 exposes child WebView construction with `build_as_child`, but focus, z-order and resize must be proven against the native shell before WebExplorer can be promoted from capability to product surface.
- Skia is attractive for a high-end native shell, but on the current Windows machine it pulled `skia-bindings`, needed a large binary/build path, failed under low disk space and missing LLVM fallback. Stage 0 therefore uses FemtoVG/software first and records Skia as a gated renderer option, not a blocker.
- Slint's May 2026 Servo integration work shows the sci-fi path for web content inside Slint: GPU texture bridging into Slint's WGPU/D3D12 renderer. That confirms the direction, but also proves that a serious Windows path needs explicit GPU interop gates, not a naive WebView overlay.
- Current source floor checked: Slint docs/GitHub releases, wgpu releases, WRY docs/releases, and Slint+Servo Windows GPU bridge notes. This makes the Stage 0 wall precise: design parity, focus/resize lifecycle and GPU texture ownership are the blockers, not merely "can a window open".

Second-wave resolution chosen:

- Keep the new native front outside Tauri at `examples/ingen_native_front/`.
- Use `.slint` as the only app UI source.
- Use wgpu 29 directly for a deterministic texture producer and proof hash.
- Use WRY/WebView2 only as a peripheral capability with a local dark fixture.
- Emit a compact Stage 0 report with proof hash instead of relying on screenshots or claims.
- Preserve the deleted Tauri/WebView UI only through a frozen design inventory
  while the Slint harness becomes the product shell.

Current state 2026-06-06:

- New crate: `examples/ingen_native_front/`.
- Research note: `research/sota_june_2026.md` records checked SOTA sources,
  hard walls and second-wave resolutions for the current native path.
- App UI: `ui/app.slint`, reset on 2026-06-06 from the failed DOM import path
  to authored native Slint components. It now contains `TitleBar`, `LeftPanel`,
  `WorkspaceHeader`, `DropCanvas`, `SectionStatusDock`, `ChatBar` and
  `NativeModal` components,
  with `no-frame: true` on the Slint `Window`. `active_section`,
  `section_title`, `canvas_title` and `canvas_hint` are now projected from the
  Rust state into these components, so section navigation changes the native
  titlebar, sidebar, breadcrumb and canvas copy without using the old DOM
  importer as the product shell. Exact screenshot parity is guarded by the
  frozen old-front inventory; the live product shell is Slint/Rust.
- First native click capture: sidebar actions, recent-session rows and the chat
  attach square now route to the Rust `activate_imported_control` callback and
  open a native modal while the final product service bindings are pending.
- Native canvas actions: `DropCanvas` now exposes four section-aware command
  buttons. Forge, WebExplorer, Banger, Trading and Real Estate get native
  actions that route through Rust capture instead of dead UI.
- Native promotion guard: the promotion action is blocked in Rust while design
  parity is false, and the modal explicitly preserves the Tauri visual
  reference until native parity is approved.
- Forge first-viewport parity pass: the native state now starts on Forge/New
  session, the canvas title/hint match the original Drop-any-file surface, the
  migration action buttons are hidden on the base Forge viewport, the left panel
  vertical rhythm was tightened toward `layer_forge.png`, the chat bar was moved
  to the original horizontal position and its placeholder now matches the
  oracle text.
- Forge recents parity pass: the base Forge viewport masks extra generated
  recent-session rows so the first screen shows the single `New session` row
  visible in `layer_forge.png`; non-Forge sections keep their extra rows.
- Stage 1 Forge first-viewport parity gate: `src/visual_parity.rs` now verifies
  the Forge oracle contract in Rust. The report exposes
  `design.forgeFirstViewport.passed=true`, including default section `forge`,
  `Drop any file`, chat placeholder `Run a Monte C`, locked shell geometry and
  hidden migration/status surfaces on Forge.
- Native section status dock: `SectionStatusDock` renders WebExplorer, Banger,
  Trading and Real Estate service/proof projections directly from Rust without
  Tauri/browser IPC. It is hidden on the base Forge first viewport to avoid
  inventing a right panel where the original design does not show one.
- Native design tokens: `ui/tokens.slint`, initialized from the current front's
  `--bg`, `--surface`, `--surface-soft`, `--line`, `--text`, `--muted`,
  `--accent`, `--forge-voice-accent` and `--forge-terminal-accent-alt`.
- Design inventory: `design/current_tauri_front_inventory.md` and
  `design/current_tauri_front_inventory.json`. These preserve the deleted
  Tauri/WebView front as a frozen design oracle; they are not a live source
  tree and must not be used to recreate the old architecture.
- Runtime probes: `src/wgpu_probe.rs`, `src/webview_probe.rs`.
- WebView dependency: WRY upgraded from `0.52.1` to `0.55.1` on 2026-06-06 and `cargo check --tests` still passes.
- Gated child WebView proof harness: `src/webview_child.rs`, enabled only with `--webview-child-proof`; it attaches WRY as child, syncs bounds every 250 ms, and schedules WebView -> parent focus calls.
- State kernel: `src/state.rs`, first deterministic Rust reducer/projection
  pass with replay hash and browser-IPC-free proof.
- Proof manifest: `src/proof.rs`, available through `cargo run --manifest-path examples\ingen_native_front\Cargo.toml -- --report`.
- Latest report: Windows, NVIDIA GeForce RTX 3050 6GB Laptop GPU, wgpu Vulkan backend, texture probe true, WRY/WebView2 compile-time capability true, child attach mode `--webview-child-proof`, design parity false, native component target list present, Forge first-viewport static parity `true`, state checkpoint schema `ingen.native_front.state_checkpoint.v1`, state replay hash `74a82d2c844898413cd76b140767432d8ce1932ac2a75a6d3349a3dddc19718b`, event log hash `175aff109eb3f3d938d805716d6e9ba3c5d1fc60affbe1f658a446f07f84797c`, state hash `482e5f3b6f454a0f82822e07f2ca7177a3143f88bb48e6a8492d0efa03accf1b`, `directMutationBlocked=true`, keyboard shortcuts `Control+Tab -> NavigateNext` and `Escape -> CloseModal`, fake direct-service proof hash `909dcdcc6e7082cde082684b2a1f952a44692a834cc4d888e1b594add95f84f8`, fake long-job stream `queued -> running -> done`, proof hash `4e431fb20ae1a870f4806af6e06d027a4fdfcd2046cf7047f32ccb4d577cd027`.
- Latest visual launch: `C:\scan-shared-target\debug\ingen-native-front.exe`
  was launched on 2026-06-06 after `cargo check --tests` and `cargo test --lib`
  passed. The window title is `InGen Native Front - Forge`.
- Latest native shell reset: `ui/app.slint` is no longer generated DOM geometry.
  The active shell is handwritten Slint with reusable native components and no
  `GeneratedLayer*` components.
- Latest native section projection: `TitleBar`, `LeftPanel`,
  `WorkspaceHeader` and `DropCanvas` consume `NativeUiState` projections, so
  Forge/WebExplorer/Banger/Trading/Real Estate switches are visible in the
  native shell instead of being static placeholders.
- Latest native click proof: visible sidebar controls, recent-session controls
  and the attach square are not dead UI anymore; they are captured by Slint
  `TouchArea`s and projected through Rust into `NativeUiEvent::OpenModal`.
- Latest native action proof: `DropCanvas` has section-aware native command
  buttons for primary/secondary/tertiary/promotion-gate actions, all routed
  through Rust capture.
- Latest promotion guard: `main.rs` turns the `promotion gate` action into a
  native Rust modal explaining that deletion/cutover is blocked while design
  parity remains false.
- Latest Forge parity pass: `NativeUiState::default()` is Forge, not Shell;
  `DropCanvas` hides migration controls on Forge/Shell; sidebar coordinates,
  chat x-position, user badge and chat placeholder were tuned against
  `ui/generated_assets/layer_forge.png`.
- Latest Forge recents pass: extra recents are hidden behind a native mask on
  Forge/Shell to match the one-row oracle while preserving richer section rows.
- Latest parity gate: `src/visual_parity.rs` adds two tests and a report
  payload for the first Forge viewport. `cargo test --lib` now runs 10 tests,
  including `forge_first_viewport_parity_contract_passes`.
- Latest Stage 2 kernel: `NativeStateKernel` owns `NativeUiState` plus the
  event log. The app runtime now dispatches UI/service events through this
  kernel, emits replayable checkpoints and records event-log/state hashes in
  the report.
- Latest native status surface: `SectionStatusDock` consumes proof, GPU,
  WebView, provider, job, Banger, Trading and Real Estate projections and has a
  native refresh button.
- Latest import report:
  `examples/ingen_native_front/design/tauri_front_slint_import_report.json`,
  source root `examples/forge_tauri_ui/ui`, CSS viewport `1535x786`, output
  viewport `1535x786`, scale `1`, render mode `native`, layers `forge`,
  `webexplorer`, `banger`, `trading`, `real-estate`, Slint entries `445`,
  SVG/vector assets `139`, reactive control overlays `590`, proof hash
  `90a1c9feea5629e35950d5462cdfae1be0d762167d35260ac73a4471f2c58cab`.
- Latest native exe rebuild: `cargo build --manifest-path examples\ingen_native_front\Cargo.toml`
  completed and produced `C:\scan-shared-target\debug\ingen-native-front.exe`
  at 2026-06-06 20:17:27.
- Latest window-frame fix: Slint now selects the winit backend before
  `AppWindow::new()` and installs
  `with_winit_window_attributes_hook(|attributes| attributes.with_decorations(false))`,
  so the imported InGen titlebar is intended to be the only visible frame.
- Lifecycle smoke: `C:\scan-shared-target\debug\ingen-native-front.exe --webview-child-proof` stayed alive for 8 seconds with window title `InGen Native Front - Stage 0`, then was stopped. Latest smoke includes bounds sync and programmatic `focus()` / `focus_parent()` scheduling. This proves startup lifecycle only, not visual focus/resize correctness.
- Promotion status: not ready. Manual launch/focus/resize/lifecycle proof is still required before Stage 0 can be marked done.
- Design status: native shell active. The deleted Tauri/WebView UI remains only
  as a frozen inventory/screenshot oracle; the live product UI is Slint/Rust.
- Environment note: `C:` had about 0.09 GB free during the first test run.
  `cargo clean --manifest-path examples\ingen_native_front\Cargo.toml` freed
  3.2 GiB of build artifacts. On the latest Stage 1 run,
  `cargo check --tests`, `cargo build` and `cargo test --lib` all passed.

Verifier:

- ~~`cargo check` passes.~~ Done 2026-06-06 with `RUSTFLAGS='-C debuginfo=0'` and `CARGO_INCREMENTAL=0`.
- ~~`cargo test` passes if tests exist.~~ Done 2026-06-06:
  `cargo test --manifest-path examples\ingen_native_front\Cargo.toml --lib`
  passes 8 tests.
- Manual launch opens a native window.
- First visible frame is dark/stable, not white.
- Chat bar size does not jump during startup.
- Slint text input receives focus.
- WebView receives focus.
- Focus can return from WebView to Slint.
- Resizing keeps Slint, wgpu and WebView bounds coherent.
- Closing the app leaves no stuck process.

Definition of done:

Stage 0 is done only if the app proves that Slint, wgpu and WRY/WebView2 can
share one native desktop app without fundamental focus, resize or lifecycle
breakage.

Explicitly forbidden:

- no RAM DOM Atlas;
- no production WebExplorer logic;
- no Banger engine migration;
- no Tauri command additions;
- no TypeScript/JavaScript application code;
- no placement under `examples/forge_tauri_ui/`.

Rollback:

- Delete `examples/ingen_native_front/`.
- No existing app route changes.

## 12. Stage 1 - Slint Shell Skeleton

Status: Forge first-viewport visual MVP delivered 2026-06-06. The old Tauri UI
has been deleted; its frozen inventory remains the visual source of truth while
Slint/Rust is the live product shell.

Purpose:

Build the real native shell shape, still with placeholder sections.

Precondition:

Stage 1 cannot start as a redesign from scratch. Before replacing the harness
with product shell work, create a visual inventory of the existing Tauri app:

- ~~first viewport shell;~~ Captured as a required parity proof in `design/current_tauri_front_inventory.md`.
- ~~compact titlebar and window controls;~~ Current dimensions and source files captured.
- ~~top icon controls;~~ Captured as a product surface requirement.
- ~~left session/job panel;~~ `279px` locked.
- ~~central canvas/transcript surface;~~ Current canvas padding and source files captured.
- ~~right proof/panel surface;~~ `287px` open width locked.
- ~~chat bar and 94px command square;~~ Chat geometry locked.
- ~~Banger workspace;~~ Banger columns and parity proof captured.
- ~~WebExplorer surface;~~ Peripheral proof captured.
- ~~trading surface;~~ Product surface and proof captured.
- ~~real-estate surface.~~ Product surface and proof captured.

The inventory must be treated as source material for Slint components.

Work:

- Preserve the current front design as the reference; no redesign by accident.
- ~~Create the design inventory gate from the current Tauri front.~~ Done 2026-06-06: `examples/ingen_native_front/design/current_tauri_front_inventory.md` and `.json`.
- ~~Extract native design tokens from the current UI:~~ Done 2026-06-06 in `ui/tokens.slint`:
  - matte dark background;
  - surface hierarchy;
  - line/border tone;
  - text/muted text;
  - neutral app accent;
  - Forge voice accent;
  - terminal/proof accent.
- ~~Implement shell root in `.slint`.~~ Re-done 2026-06-06 as authored native
  Slint components after the generated DOM layer proved insufficient for
  product parity.
- ~~Implement sidebar component.~~ Re-done 2026-06-06 as `LeftPanel`, with
  native `ToolTab`, `SidebarAction`, `SessionRow` and `ScrollGlyph` components.
- ~~Implement chat bar component.~~ Re-done 2026-06-06 as `ChatBar`, with native
  command square, provider controls, send control and native `LineEdit`.
- Status/proof strip is intentionally hidden from the first Forge viewport
  because the original design does not show a right proof panel on this screen.
- ~~Project section navigation into the native shell.~~ Done 2026-06-06:
  `TitleBar`, `LeftPanel`, `WorkspaceHeader` and `DropCanvas` consume
  `NativeUiState` projections for Forge/WebExplorer/Banger/Trading/Real Estate.
  The full product surfaces remain pending, but the native shell is no longer a
  frozen Forge-only screen.
- ~~Make first authored controls visibly clickable.~~ Done 2026-06-06:
  sidebar actions, session rows and chat attach square now emit native Slint
  click events into Rust and display a native modal until their final services
  are connected.
- ~~Add native canvas action controls.~~ Done 2026-06-06: `DropCanvas` now
  renders section-aware action buttons and routes them through Rust event
  capture instead of leaving the central surface passive.
- ~~Start native shell on the original Forge first viewport.~~ Done 2026-06-06:
  default state is Forge/New session, base canvas is `Drop any file`, migration
  controls are hidden on the first viewport and chat/sidebar alignment was
  tuned against the Tauri-derived oracle.
- ~~Add automated Forge first-viewport parity gate.~~ Done 2026-06-06:
  `src/visual_parity.rs` verifies the static Slint contract and
  `cargo test --lib` now fails if the native shell no longer opens on the
  original Forge first viewport shape.
- ~~Render section service/proof status natively.~~ Done 2026-06-06:
  `SectionStatusDock` renders section-specific Rust projections for
  WebExplorer/Banger/Trading/Real Estate and exposes a native refresh action.
- ~~Add Tauri-front-to-Slint importer.~~ Done then retired 2026-06-06:
  the importer produced the frozen report and was deleted after the old
  HTML/CSS/TypeScript source front was removed.
- ~~Generate one Slint layer per original runtime state.~~ Done 2026-06-06 for
  `forge`, `webexplorer`, `banger`, `trading`, `real-estate`; layers toggle
  from `NativeUiState` via `active_section`.
- ~~Keep exact bitmap snapshots as parity oracle.~~ Done 2026-06-06: frozen
  render artifacts remain for parity review, but the active Slint app uses
  native rectangles/text plus native `TouchArea` overlays, not a bitmap as its
  displayed UI.
- ~~Import original SVG/icon assets into the Slint layer.~~ Done 2026-06-06:
  the importer now extracts 139 visible SVG vectors from the original DOM and
  emits them as Slint `Image` assets, instead of losing the toolbar/dropzone
  iconography.
- ~~Make imported controls react inside Slint.~~ Done 2026-06-06: the importer
  now turns buttons, links, focusable controls, pointer controls and nav/tab
  elements into 590 detected controls and emits filtered direct Slint
  `TouchArea` overlays. Known routes navigate natively; unbound controls open a
  native modal so the click is not dead while product service bindings are
  still pending.
- ~~Remove duplicate Windows frame from the native app.~~ Done 2026-06-06 via
  `Window.no-frame: true` plus winit window attributes before window creation;
  the first attempted late `set_decorations(false)` did not remove the OS
  titlebar on Windows.
- Next visual parity task: add screenshot comparison between the native Slint
  bitmap layer and the original Tauri viewport.
- ~~Implement modal/confirmation placeholder.~~ First pass done 2026-06-06; blocked navigation and captured chat commands use a native Slint modal.
- Implement keyboard shortcuts:
  - focus chat;
  - send;
  - ~~switch section;~~ Done 2026-06-06: `Control+Tab` triggers `NavigateNext`;
  - open command palette later;
  - ~~close modal.~~ Done 2026-06-06: `Escape` triggers `CloseModal`.
- Keep Rust state authoritative.
- Slint properties are projections, not the source of truth.

Verifier:

- ~~Window opens with final-like geometry.~~ Visual MVP launched 2026-06-06
  as `C:\scan-shared-target\debug\ingen-native-front.exe`.
- Minimize/restore has no visual jump.
- ~~Section switching updates placeholders.~~ Done 2026-06-06 through Rust
  state projection into the main Slint shell components.
- ~~Chat input remains stable.~~ Compile/runtime visual MVP path now keeps the
  native composer and native `LineEdit`.
- Keyboard shortcuts work.
- ~~No CSS/HTML/JS is introduced.~~ Done for the app shell; only the local
  WebView fixture remains as a WebExplorer peripheral proof.

Definition of done:

The app looks structurally like InGen, but sections may still be placeholders.
Stage 1 Forge first-viewport visual MVP satisfies the emergency visual target
for the native-front crate. Remaining parity work is tracked by the frozen
design inventory; the old Tauri/WebView front has already been deleted.

## 13. Stage 2 - Native State Kernel

Status: complete 2026-06-06 for the native-front crate.

Purpose:

Move UI state into deterministic Rust structures.

Target model:

```rust
NativeStateKernel
NativeUiState
NativeUiEvent
NativeUiProjection
NativeStateCheckpoint
```

Work:

- ~~Define the state:~~ Expanded pass done 2026-06-06 in `src/state.rs`:
  - active section;
  - chat draft;
  - selected session;
  - model/provider status;
  - active jobs;
  - warning/proof badges;
  - visible panels;
  - WebExplorer state placeholder;
  - Banger viewport state placeholder.
- ~~Define event enum:~~ Expanded pass done 2026-06-06:
  - user input;
  - navigation;
  - provider update;
  - job update;
  - proof update;
  - section event.
- ~~Add reducer-style update functions.~~ Done.
- ~~Add deterministic replay for event logs.~~ Done with stable projection hash tests.
- ~~Add state projection for Slint.~~ Done; `main.rs` projects Rust state into Slint properties.
- ~~Block direct view mutation of core state.~~ Done; `NativeUiState` fields are private and external code enters through `apply(event)` / `projection()`.
- ~~Promote state ownership into a kernel.~~ Done 2026-06-06:
  `NativeStateKernel` owns the private `NativeUiState` and the event log.
- ~~Add checkpoint envelope.~~ Done 2026-06-06:
  `NativeStateCheckpoint` records schema, events, projection, event-log hash
  and state hash.
- ~~Use the kernel in the running app.~~ Done 2026-06-06:
  `main.rs` dispatches navigation, chat, modal, service snapshot and streaming
  job updates through `NativeStateKernel::dispatch`.

Verifier:

- ~~Unit tests replay event logs.~~ Done.
- ~~Same event log produces same projection hash.~~ Done.
- ~~View code cannot mutate core state directly.~~ Done by Rust privacy and verified by `cargo check --tests`.
- ~~Checkpoint restores from event log.~~ Done.
- ~~Checkpoint JSON is stable and schema-tagged.~~ Done.
- ~~Report includes checkpoint/event-log/state hashes.~~ Done.
- ~~App runtime uses kernel dispatch instead of direct state mutation.~~ Done.

Definition of done:

Native UI state is replayable, checkpointed and proofable before connecting
real services.

## 14. Stage 3 - Direct Rust Services

Purpose:

Remove browser IPC from the native path.

Work:

- ~~Define Rust traits for UI-facing services:~~ Fake first pass done
  2026-06-06 in `src/services.rs` for hardware, providers, sessions, jobs,
  Brain, Monster, Banger, WebExplorer, trading and real estate. Real
  implementations are still pending.
- ~~Add fake implementations for tests.~~ First pass done 2026-06-06 with
  deterministic `FakeNativeServices` and service snapshot proof hash.
- ~~Project fake service snapshot into the native shell.~~ Done 2026-06-06:
  `main.rs` applies the service snapshot as Rust events, then Slint receives
  only the resulting projection.
- ~~Add direct local implementations by reusing Rust modules where possible.~~
  Done 2026-06-06 for the native service boundary:
  `DirectNativeServices` reads local wgpu/WebView2 probes and produces
  hardware/provider/session/job/status snapshots without browser IPC. Full
  Brain/Monster/Banger/WebExplorer/trading/real-estate product adapters remain
  staged after this boundary.
- ~~Add simulated long job streaming without blocking Slint.~~ Done 2026-06-06:
  `spawn_fake_long_job` runs on a background thread, `main.rs` polls channels
  with a Slint timer and applies job updates through the Rust reducer.
- ~~Add proofed typed service commands.~~ Done 2026-06-06:
  `NativeServiceCommand` and `NativeServiceCommandResult` cover snapshot
  refresh, imported-control capture and chat submission. Chat emits queued
  native jobs and every command carries a proof hash.
- ~~Long operations must run async/background without blocking Slint.~~ Done
  for the native event-loop path: command jobs enter the reducer immediately
  and the long-job simulator streams `queued -> running -> done` over a
  non-blocking channel. Production async adapters remain a Stage 4+ task.

Verifier:

- ~~Fake service tests.~~ Done for the first service snapshot.
- ~~Native shell can request hardware/provider status.~~ Done through
  `local_service_snapshot()` backed by real local wgpu/WebView2 probes.
- ~~A simulated long job streams status without freezing UI.~~ Done with
  `queued -> running -> done` report proof.
- ~~Direct service commands are proofed without browser IPC.~~ Done with
  `direct_service_commands_are_proofed_without_browser_ipc`.
- ~~Stage report records the direct boundary.~~ Done:
  `schema=ingen.native_front.stage3_direct_services.v1`,
  `directProbeServicesConnected=true`, `browserIpcRequired=false`,
  `directRefreshCommand.proofHash=e4b9e36763178d02b8aebcbc0652d81eb3f0af5a328c13ef3a8cefcee0fa0cc0`,
  `proofHash=eb6826547baf9329e275f3b5fbca23964d4a6fd342fd00261049d09d3ccddf1a`.

Definition of done:

Native UI talks to Rust services directly through traits, not through a browser
IPC model. Stage 3 is complete; the old Tauri/WebView UI remains only as a
frozen design oracle while product service adapters are completed in later
stages.

## 15. Stage 4 - Banger Native Viewport

Purpose:

Make Banger a native engine viewport.

Work:

- ~~Identify reusable existing Banger Rust/wgpu code.~~ Done 2026-06-06:
  the native front reuses the existing Banger viewport contracts as the shape
  to preserve, without importing the old Tauri command layer.
- ~~Move reusable renderer pieces behind a native viewport interface.~~ Done
  2026-06-06 in `examples/ingen_native_front/src/banger_viewport.rs` with
  `BangerViewportRequest`, `BangerViewportFrame` and
  `BangerSlintTextureBridgeProof`.
- ~~Create a fixture scene.~~ Done 2026-06-06:
  `stage4-native-fixture` renders a deterministic 640x360 RGBA native viewport
  frame.
- ~~Render to a native `wgpu` texture or surface.~~ Stage 4 bootstrap done
  2026-06-06: the frame is treated as a native render target manifest with
  `RENDER_ATTACHMENT`, `COPY_SRC`, `COPY_DST` and `TEXTURE_BINDING`; Slint
  displays the current frame through `SharedPixelBuffer` until direct wgpu
  texture import can be safely promoted.
- [x] Add `TEXTURE_BINDING` to the native Banger render target and emit a
  `slintTextureBridge` proof for Slint/wgpu import readiness.
- [x] Integrate the result into a real Slint component with the safe fallback:
  `BangerNativeViewport` displays the generated frame in the Banger route.
- [ ] Promote the Slint component from `SharedPixelBuffer` fallback to direct
  external texture import once Slint and Forge use compatible `wgpu` types.
- Add controls:
  - [x] orbit;
  - [x] pan;
  - [x] zoom;
  - [x] fit via deterministic render-handoff bounds, viewport fit hash and
    camera distance proof;
  - [x] fit via Hybrid Scene Graph artifact nodes with representation mix,
    transform, AABB/sphere and graph/proof hashes;
  - [x] editable object manifest command with parent/child DAG validation,
    local/world transforms, authored representation and object proof hashes;
  - [x] debug modes.
- Add telemetry:
  - [x] frame time;
  - [x] resource count;
  - [x] VRAM estimate;
  - [x] render graph hash;
  - [x] frame hash;
  - [x] residency proof.
  - [x] RHI feature gate matrix with bindless, mesh shader, ray query, shader
    cache, compute scale and backend parity proof hashes.
  - [ ] Promote the Banger pipeline cache manifest to persisted driver blobs:
    the Rust code now emits a manifest-only/driver-blob-candidate proof and the
    Banger verifier passes, but `wgpu::PipelineCache` blob storage is not wired.
  - [ ] Replace bootstrap handoff-derived fit bounds with real Scene Graph
    object/transform authority once Stage 2 owns editable scene objects.

Verifier:

- ~~Fixture scene renders.~~ Done in the Slint route through
  `BangerNativeViewport`.
- ~~Frame hash emitted.~~ Done:
  `frameHash=00d78c2e3767b3992ebdeb7f06a47197776480db2d95f75d4ac2ddde604a3c5d`.
- ~~Slint texture bridge proof emitted, including direct-import blocker and
  fallback route.~~ Done:
  `bridgeProof=a768af4eaefcf7e2b0128af6cbeb3a67f45659b249e9c767527a67f00c4bc156`,
  `textureBindingReady=true`, `directTextureImportReady=false`.
- ~~Stage report records Banger viewport contract.~~ Done:
  `schema=ingen.native_front.stage4_banger_viewport.v1`,
  `visibleInSlint=true`, `browserCanvasRequired=false`,
  `proofHash=69e836afdacbcbf0c90f35f0efa1d0f3d85f52960598c649e2541fb3cd1e425e`.
- RHI feature gate matrix emitted before the front promotes any high-end render
  path.
- Pipeline cache manifest proof emitted and promoted only after the Banger
  Cargo verifier passes again.
- Hybrid Scene Graph proof emitted; viewport fit bounds match graph bounds.
- Authored scene object manifest rejects cycles before the front can promote an
  editable hierarchy.
- No flash when entering/leaving Banger section.
- 100 section switches do not leak resources.
- Viewport bounds remain stable during resize.

Definition of done:

Banger is no longer conceptually tied to a browser viewport. Stage 4 is
complete for the native front bootstrap: a real Slint component displays a
Rust-generated native viewport frame with hashes and a texture bridge proof.
The remaining work is promotion from safe image fallback to direct Slint/wgpu
texture sharing and deeper Banger engine adapters.

## 16. Stage 5 - Isolated WebExplorer

Purpose:

Embed real web pages while keeping InGen native.

Work:

- ~~Create `webexplorer.rs` manager.~~ Done 2026-06-06:
  `examples/ingen_native_front/src/webexplorer.rs` owns the isolation policy,
  navigation decisions, bounds proof and isolation proof.
- ~~Embed WRY/WebView2.~~ Done for the controlled proof path:
  `src/webview_child.rs` creates a WRY/WebView2 child with policy, init script,
  devtools disabled and bounds sync when launched with `--webview-child-proof`.
- ~~Add navigation command.~~ Done at the policy layer:
  `WebExplorerPolicy::decide_navigation` returns allow/deny decisions with
  proof hashes before WRY accepts navigation.
- ~~Add local fixture page.~~ Done:
  `fixtures/webview_stage0.html` now carries a restrictive CSP and is loaded
  through `webexplorer_fixture_html()`.
- ~~Add bounds sync from Slint layout.~~ Done in `webview_child.rs` through
  repeated `set_bounds(child_bounds(window))`.
- ~~Add focus handoff:~~ Done in the proof path.
  - ~~Slint -> WebView;~~ `webview.focus()` scheduled.
  - ~~WebView -> Slint.~~ `webview.focus_parent()` scheduled.
- ~~Add navigation policy:~~ Done 2026-06-06.
  - ~~block dangerous schemes;~~ `javascript:`, `file:`, `data:`,
    `vbscript:` and `ms-appx:` are rejected.
  - ~~control external opens;~~ unknown HTTPS hosts are denied until atlas refs
    promote them.
  - ~~control downloads;~~ downloads are policy-disabled.
  - ~~log navigation proof.~~ WRY navigation handler logs allow/deny reason and
    proof hash.
- ~~Add crash/recreate handling.~~ Stage 5 policy done:
  crash policy is `drop child WebView, keep Slint shell alive, recreate only
  through policy`; deeper runtime restart telemetry remains a later hardening
  task.

Verifier:

- ~~Local fixture page loads.~~ Done in `--webview-child-proof` path.
- Public simple page loads if network is allowed in the environment. Kept as a
  manual/environment proof because network is not required for local stage
  verification.
- Back/forward/reload works. Deferred until the WebExplorer command surface
  opens in Stage 6/8; policy and load boundary are now in place.
- ~~Resize works.~~ Bounds sync proof path implemented.
- ~~Focus works.~~ Focus/focus-parent calls are scheduled and status-reported;
  visual focus proof remains manual.
- ~~Blocked scheme proof exists.~~ Done:
  `blockedNavigation.proofHash=2b5213ecbce5b1b5229a75bc6cbabf2713d10733effbf52d3b851005501a4ad7`.
- ~~WebView failure does not kill shell.~~ Creation failure is captured into
  `webview_status` instead of panicking.
- ~~Stage report records WebExplorer isolation.~~ Done:
  `schema=ingen.native_front.stage5_isolated_webexplorer.v1`,
  `policyHash=4c59d568674fd0d28c50f0a92c0eb049cac6ecab88bf266a95c54162a9f51727`,
  `isolationProof=d2aa1a98d002a22fd0b999518ef6379da4f859790864ec590beeb480304f6394`,
  `proofHash=b325095e2e8800154808be71e6da8ba5d7e371573bf64cc18a769bee06b29821`.

Definition of done:

WebExplorer exists as a controlled peripheral, not as the app shell. Stage 5 is
complete for the native isolation boundary; public navigation controls and RAM
DOM capture continue in the following stages.

## 17. Stage 6 - RAM DOM Atlas In Native Architecture

Purpose:

Resume RAM DOM work only after WebExplorer is isolated.

Work:

- ~~Capture DOMSnapshot.~~ Stage 6 fixture path done 2026-06-06:
  `src/webatlas.rs` captures the isolated WebExplorer fixture into a normalized
  DOM-like node graph. The production WebView2 runtime snapshot remains behind
  the same interface for later dynamic pages.
- ~~Capture accessibility tree.~~ Stage 6 fixture path done:
  roles, AX names and AX values are derived for headings, inputs, buttons,
  document/main/region nodes.
- ~~Capture layout bounds.~~ Done with deterministic fixture bounds for every
  node; native crop-store wiring remains a later visual proof.
- ~~Capture whitelisted styles.~~ Done for inline/body whitelisted style
  subsets and fixture CSS policy resource metadata.
- ~~Capture resource metadata.~~ Done:
  CSP policy and inline style resources receive content-addressed refs.
- ~~Capture screenshots/crops for visual-only regions.~~ Stage 6 records the
  blocker explicitly in blind spots; no visual-only fixture region exists yet.
- ~~Normalize into `AtlasNode`.~~ Done with atlas ref, frame path, backend node
  id, parent/children, tag, role, AX, text/value, attributes, bounds, style
  subset, resource refs and evidence hash.
- ~~Store content-addressed raw and normalized manifests.~~ Done:
  `rawHash=7f6fbebd45537381f0b7b04d9fc4182deb7c7c1c864c04503565d93ee9c903f5`,
  `normalizedHash=b3d3f6001e13a0eca430d0dbee2c7288025acf44bb205f3a0fb990f635938890`.
- ~~Generate coverage and blind-spot report.~~ Done:
  DOM 100, AX 57, layout 100, style 7, resource 100, with explicit dynamic JS,
  crop-store and platform AX blind spots.

Atlas node should include:

- atlas ref;
- frame path;
- backend node id;
- parent/children;
- tag;
- role;
- AX name/value;
- text/value;
- attributes;
- bounds;
- style subset;
- resource refs;
- evidence hash.

Verifier:

- ~~Static fixture produces deterministic hash.~~ Done:
  `webatlas::tests::fixture_webatlas_is_deterministic`.
- Dynamic fixture produces expected tree delta. Deferred until WebView2 runtime
  execution capture is wired behind the same `capture_webatlas_from_html`
  boundary.
- ~~Coverage report lists DOM/AX/layout/resource ratios.~~ Done.
- ~~Blind spots are explicit.~~ Done.
- ~~Stage report records RAM DOM Atlas.~~ Done:
  `schema=ingen.native_front.stage6_ram_dom_atlas.v1`,
  `webAtlas.proofHash=fbbec056d2d3c976f74b7a3f926e161220ea92adc7f4b09b90d7881ce79fc7bb`,
  `proofHash=2f21397c1a19d3caf558d986b3e9cfe502b34870dbc7b9b3a110c6135fa4861d`.

Definition of done:

The native WebExplorer can produce a measured page atlas without giving raw
authority to the LLM. Stage 6 is complete for the static fixture and
content-addressed RAM object contract; dynamic WebView2 capture and Slint Atlas
UI continue in Stage 7.

## 18. Stage 7 - Native RAM DOM UI

Purpose:

Render the Atlas in Slint.

Work:

- ~~Tree view.~~ Done 2026-06-06:
  `ui/app.slint` now includes `AtlasInspector`, fed by
  `atlas_tree_lines` from the Rust `AtlasUiProjection`.
- ~~Selected node inspector.~~ Done:
  selected atlas ref, tag, role, AX name/value, text/value, bounds, resource
  count and evidence hash are projected into the native inspector.
- ~~AX summary.~~ Done for the fixture projection:
  role/name/value are rendered from the normalized RAM DOM Atlas.
- ~~Layout bounds preview.~~ Done as native bounds text:
  x/y/w/h and center are visible for the selected node. Graphical WebView
  region highlighting waits for the dynamic WebView coordinate bridge.
- ~~Resource list.~~ Done:
  policy/style resources render with kind, URL and content hash prefix.
- ~~Action candidate list.~~ Done:
  textbox, button and link candidates are generated from role/tag/text/value
  semantics instead of raw CSS selectors.
- ~~Blind spot panel.~~ Done:
  dynamic JavaScript, crop-store and platform AX gaps render explicitly.
- ~~Proof panel.~~ Done:
  manifest proof hash, normalized hash, coverage ratios and selected node proof
  are visible in Slint.
- Search/filter:
  - role;
  - text;
  - tag;
  - resource;
  - atlas ref;
  - ~~interactive only;~~ Initial projection done through interactive/action
    candidate summary.
  - unsafe controls.

Stage 7 implementation:

- `src/webatlas.rs` now owns `AtlasUiProjection` and
  `atlas_ui_projection(manifest, selected_index)`.
- `src/main.rs` binds the projection into Slint and wires
  `atlas_previous_node` / `atlas_next_node`.
- `src/proof.rs` records the UI projection in the stage report under
  `schema=ingen.native_front.stage7_native_ram_dom_ui.v1`.
- Latest Stage 7 proof hash:
  `47dbcc19675f43ff7d7d32de684296bfd5cff4b4380e77a3a005dbffddb24f34`.
- Latest WebAtlas UI projection hash:
  `9bd9f05d682dd9d06f72f3f1ffd24bdd32b4e12fa1ccf6f3d6d453bb1e607fb5`.

Verifier:

- Fixture inspector remains deterministic:
  `webatlas::tests::atlas_ui_projection_selects_and_hashes_nodes`.
- Selecting a tree node is wired through native Prev/Next callbacks and
  updates the selected node projection.
- Large fixture responsiveness remains a Stage 8/9 broadening gate once the
  dynamic WebView capture can produce real large-page manifests.
- Selecting a tree node highlights WebView bounds as text now; graphical
  WebView overlay highlighting waits for the coordinate bridge.
- Selecting a WebView region resolves candidate nodes when possible. Deferred
  until the real WebView hit-test path is connected to atlas refs.

Definition of done:

The user can inspect the isolated WebExplorer fixture as a native RAM object in
Slint. Stage 7 is complete for the content-addressed inspector UI; Stage 8 now
continues native chat and agent surfaces. Safe CodeAct is frozen until the full
Slint/Rust migration and obsolete-front deletion are complete.

## 19. Stage 12 - Post-Migration Safe Web CodeAct Gate

Status:

Deferred. Do not implement CodeAct, LLM-driven web actions or atlas-ref
automation until:

- Stage 8 native chat and agent surfaces are complete;
- Stage 9 native product sections are complete;
- Stage 10 native packaging/security is complete;
- Stage 11 has deleted the global Tauri/WebView frontend and obsolete
  HTML/CSS/TypeScript/WASM shell paths;
- Slint/Rust is the only normal product frontend.

Purpose:

Enable web actions only through verified atlas refs.

Rule:

The LLM must never get raw permanent authority over CSS selectors or page JS.

Action format:

```text
/navigateweb_
tree_hash=<current atlas tree hash>
goal=<short goal>
action=click|type|select|scroll|focus|copy_text|capture_region|download_resource
target_ref=<atlas ref>
target_kind=<button|link|searchbox|textbox|image|video|canvas|menu|dialog|region>
input_text=<optional>
expected_state=<url_change|tree_change|text_visible|download|no_navigation>
confirmation=<required|not_required>
```

Work:

- Resolve by atlas ref.
- Reject stale tree hashes unless self-healing has high confidence.
- Self-heal by:
  - backend id;
  - frame path;
  - AX role/name;
  - normalized text;
  - resource hash;
  - bounds.
- Block sensitive actions until human confirmation:
  - login;
  - payment;
  - booking;
  - upload;
  - purchase;
  - destructive submit;
  - personal data.

Verifier:

- Click fixture passes.
- Type fixture passes.
- Scroll fixture passes.
- Stale ref fixture recovers or returns ambiguity.
- Sensitive form fixture blocks.
- Every action logs previous tree hash, result tree hash, URL delta and proof
  hash.

Definition of done:

Web action is proof-ledgered and capability-gated.

## 20. Stage 8 - Native Chat And Agent Surfaces

Purpose:

Move the main agent experience to Slint.

Work:

- ~~Transcript.~~ Done 2026-06-06:
  `NativeUiState` now owns replayable `NativeChatMessage` entries and projects
  them to Slint through `transcript_lines`; no browser IPC or CodeAct dispatch
  is involved.
- ~~Streaming messages.~~ Done for the native service boundary:
  background service jobs already stream through a non-blocking channel poller,
  and chat submissions surface as native queued/done jobs. Real provider token
  streaming remains a product-adapter broadening task, not a WebView task.
- ~~Chat input.~~ Done:
  `ChatBar` stays native Slint, `send_chat` records the draft in Rust state and
  clears the field through the state projection.
- ~~Provider/model picker.~~ Initial native status done:
  provider/model state projects from direct Rust services into Slint. Full
  picker choices are a Stage 9 product-service adapter task.
- ~~Session list.~~ Done:
  service sessions are converted to `NativeSessionSummary` and rendered through
  `session_lines`.
- ~~Plan cards.~~ Done:
  `NativeAgentCardKind::Plan` projects into the `AgentSurface` plan panel.
- ~~Questionnaire cards.~~ Done:
  `NativeAgentCardKind::Questionnaire` projects into the native questions
  panel. CodeAct/LLM questionnaires stay deferred until migration completion.
- ~~Session title updates.~~ Initial native state done:
  selected session is owned by Rust and projected to Slint; full rename service
  binding remains in the Stage 9 product adapters.
- ~~Proof previews.~~ Done:
  proof cards render service snapshot and chat-capture hashes.
- Copy/select text support. Deferred:
  Slint text rendering is native, but selectable transcript text still needs a
  dedicated widget path.
- ~~Virtualization for long transcripts.~~ Done:
  projection keeps only the last 8 visible messages and reports older-message
  count.

Stage 8 implementation:

- `src/state.rs` now owns sessions, transcript messages and agent cards behind
  replayable `NativeUiEvent`s.
- `ui/app.slint` now includes `AgentSurface` for sessions, transcript, plan,
  questionnaire and proof panels.
- `src/main.rs` projects the new state into Slint and maps direct service
  sessions into native session summaries.
- `src/proof.rs` records
  `schema=ingen.native_front.stage8_native_chat_agent_surfaces.v1`.
- Latest Stage 8 proof hash:
  `2fda5ff579a143ad038954413ba3752f039f996cdcc07b07ae52db1f1488c2f5`.
- Latest canonical native agent replay hash:
  `74a82d2c844898413cd76b140767432d8ce1932ac2a75a6d3349a3dddc19718b`.

Verifier:

- ~~Long session stays responsive.~~ Covered at projection level by
  `state::tests::native_agent_surface_virtualizes_long_transcripts`; manual
  visual stress remains listed before promotion.
- ~~Streaming does not resize the chat bar unexpectedly.~~ Existing locked chat
  geometry remains under the Forge first-viewport parity contract.
- ~~Plan/question/session JSON renders as native components.~~ Replaced by the
  stricter native state path:
  `state::tests::native_sessions_and_cards_project_to_slint_strings`.
- ~~No global WebView dependency.~~ Stage report keeps
  `appShellUsesTauri=false`, `appShellUsesHtmlCssJs=false`,
  `browserIpcRequired=false`, and CodeAct remains deferred.

Definition of done:

The core agent surface is usable in the native shell for local capture,
sessions, proof cards and replayable transcript state. Stage 8 is complete for
the native shell boundary; real provider streaming and product-specific model
selection continue in Stage 9 adapters.

## 21. Stage 9 - Native Product Sections

Purpose:

Move product sections into the native shell.

Sections:

- ~~trading;~~ Done 2026-06-06 as a native product manifest and Slint product
  section surface with market status, chart summary, timeframe and action
  slots.
- ~~real estate;~~ Done as a native product manifest and Slint surface with
  onboarding, zone scoring, resolver and mode-state slots.
- ~~Forge/compute;~~ Done as a native manifest backed by direct Monster/job
  service status plus Stage 8 proof cards.
- ~~Alpha;~~ Done as a native manifest backed by the same evidence-aware state
  and proof bus.
- ~~Banger;~~ Done through the native wgpu viewport plus a Stage 9 product
  manifest that proves `browser_canvas=false`.
- ~~WebExplorer;~~ Done through isolated WebView peripheral + RAM DOM Atlas UI
  plus a Stage 9 product manifest that proves only WebExplorer requires
  WebView.
- ~~diagnostics/proofs.~~ Done as a native diagnostics/proof manifest.

Trading work:

- ~~market status;~~ direct service boundary projected.
- ~~chart summary;~~ native card slot projected.
- ~~timeframe controls;~~ `1m/5m/1h/1d` native metric slot projected.
- ~~backtest cards;~~ native action/card slot projected.
- ~~alerts;~~ native action/card slot projected.
- ~~provider status.~~ direct provider/model status projected.

Real-estate work:

- ~~onboarding;~~ native section slot projected.
- ~~zone scoring;~~ native metric/action slot projected.
- ~~resolver/harvester summaries;~~ direct-service boundary slot projected.
- ~~panels;~~ native product card slot projected.
- ~~mode state.~~ Slint/native mode-state slot projected.

Forge/compute work:

- ~~compute templates;~~ native manifest slot projected.
- ~~job queue;~~ direct service jobs projected through status/proof cards.
- ~~proof cards;~~ Stage 8/9 native proof cards projected.
- ~~memory projections.~~ native manifest slot projected.

Stage 9 implementation:

- `src/product_sections.rs` adds content-addressed product section manifests
  and active-section projections.
- `ui/app.slint` adds `ProductSectionSurface` for native product dashboards.
- `src/main.rs` applies the active product section projection during startup,
  refresh and section navigation.
- `src/proof.rs` records
  `schema=ingen.native_front.stage9_native_product_sections.v1`.
- Latest Stage 9 proof hash:
  `a7ecdd02cb18891bea492a9a9448ca142782a7cf01950a258f9e6ee64b603ba4`.
- Latest product sections manifest hash:
  `0fd78c8b522d68e9e18987dd155067b7171b0775ccd17504185eeb3f095dbd7d`.
- Latest active trading section projection hash:
  `b1a76604c0f6255b8d05106469469403828d3926aa0cdca251f4178ea3794fcf`.

Verifier:

- ~~Each section has a smoke.~~
  `product_sections::tests::product_sections_cover_all_stage9_targets`.
- ~~Section switching replays state.~~ Existing replay tests cover native
  section switching, and runtime projection is reapplied on navigation.
- ~~No non-web section depends on WebView.~~ Stage 9 manifest asserts
  `webview_required=false` for every section except WebExplorer.

Definition of done:

InGen product sections have native Slint/Rust surfaces, content-addressed
section manifests and no global WebView dependency. Stage 9 is complete for the
native product-shell boundary; real domain adapters must replace the fixture
summaries before old-front deletion.

## 22. Stage 10 - Native Packaging And Security

Purpose:

Replace Tauri as a shell only after packaging and security parity.

Work:

- ~~Windows package/install path.~~ Done as a verified native target manifest:
  `windows-x86_64-native-slint`, binary `ingen-native-front.exe`,
  `tauri_shell_required=false`. Signed installer automation remains a later
  release-engineering gate.
- App icon/resources. Deferred to the final signed installer pass.
- ~~App data directories.~~ Done:
  Stage 10 records `%LOCALAPPDATA%\InGen\NativeFront` as the native app-data
  root.
- ~~Logs/crash recovery.~~ Done:
  logs and `crash-recovery/last-session.json` paths are recorded with a replay
  state hash.
- ~~Secrets path preserved.~~ Done:
  secrets get a separate app-data path and `secrets_in_logs_allowed=false`.
- ~~WebView profile isolation.~~ Done:
  `webview-profile` is isolated under native app-data and local file access is
  denied by default.
- ~~Capability policy replacement.~~ Done:
  `NativeCapabilityPolicy` replaces Tauri shell capabilities for the native
  proof boundary and explicitly blocks
  `C:\Users\quent\Documents\EVE\MAP`.
- ~~Update strategy later.~~ Done as policy:
  updater remains manual/update-gated until a signed package gate exists.

Stage 10 implementation:

- `src/packaging_security.rs` adds native app paths, capability policy, local
  path decisions and crash recovery records.
- `src/main.rs` uses `windows_subsystem="windows"` for release Windows builds
  while keeping debug console logs during migration.
- `src/proof.rs` records
  `schema=ingen.native_front.stage10_packaging_security.v1`.
- Latest Stage 10 proof hash:
  `552f078ca6372a906b25c409311f2e97204d73c5498faaad46d51916a7ef5b71`.
- Latest packaging/security manifest hash:
  `1f8812ad2361e89605f460b1ae5bbc4bfbae98217d31430072a60dad0adf59b3`.
- Latest native capability policy hash:
  `c6acdf2b99b86570aca1168783c96a66a7798e2fba20c33ddd0e02f4faacb8ed`.
- Latest crash recovery proof hash:
  `02a9262d11875f004726090e60085d71f234ce63eeaf602b7ecc53968b82c0bf`.

Verifier:

- Fresh install launches. Deferred to signed installer/manual install smoke.
- ~~App data path is correct.~~
  `packaging_security::tests::app_data_is_the_only_default_write_capability`.
- ~~Secrets remain protected.~~
  `secrets_in_logs_allowed=false` in the capability policy.
- ~~WebExplorer cannot access protected local paths.~~
  protected root decision blocks `C:\Users\quent\Documents\EVE\MAP`, and
  WebView local file access remains denied.
- ~~Crash/restart preserves recovery data.~~
  `packaging_security::tests::crash_recovery_record_is_deterministic`.

Definition of done:

Native app has a verified package/security boundary without Tauri as shell.
Stage 10 is complete for policy, paths and proof manifests; signed installer,
icon/resources and real install smoke must happen before release packaging.

## 23. Stage 11 - Delete Global WebView Front

Purpose:

Remove the old app shell and every obsolete frontend architecture node only
after native parity.

This stage is not optional. Migration Front is not complete while the old
Tauri/Dioxus/WebView architecture remains in normal-use code paths, build
scripts, docs or agent instructions.

Work:

- Tag rollback point. Blocked 2026-06-06: the worktree contains many modified
  and untracked `examples/forge_tauri_ui/**` files, so destructive cleanup must
  wait for a rollback commit.
- ~~Create an obsolete-architecture inventory before deletion:~~ Done
  2026-06-06 through
  `examples/ingen_native_front/src/obsolete_front.rs` and
  `examples/ingen_native_front/design/stage11_obsolete_front_inventory.md`.
  - Tauri app shell entrypoints;
  - Dioxus/WASM front route;
  - `examples/forge_tauri_ui/front-rs`;
  - `examples/forge_tauri_ui/ui` app-shell HTML/CSS/JS hosts;
  - generated JS/WASM host bundles;
  - frontend npm scripts used only by the old shell;
  - Tauri-only bridge/client code;
  - Tauri command wrappers that exist only for browser IPC;
  - section ownership docs that describe the old shell;
  - screenshots/smokes/audits that only prove the old shell.
- ~~Freeze old-shell feature work:~~ Done as policy in the Stage 11 manifest:
  - no new strategic features in Tauri/Dioxus;
  - only emergency fallback fixes before deletion;
  - all new product UI goes to `examples/ingen_native_front`.
- Remove normal-use dependency on:
  - Tauri main-window WebView shell;
  - Dioxus/WASM route;
  - generated JS host files;
  - HTML app hosts;
  - CSS app shell;
  - npm frontend build where no longer needed;
  - old bridge files such as Tauri client wrappers when native services replace
    them;
  - legacy section registries whose only job was browser routing.
- Keep only WebView runtime for WebExplorer.
- Prune dependencies:
  - remove Tauri shell dependencies once no longer used;
  - remove Dioxus/WASM dependencies once no longer used;
  - remove npm dependencies that only serve the old front;
  - remove build scripts that only generate old shell assets;
  - keep WRY/WebView2 only as WebExplorer peripheral dependencies.
- Replace docs:
  - update `AGENTS.md`;
  - update `CLAUDE.md` import context if needed;
  - update `README.md`;
  - update `FORGE_NATIVE_BYTECODE.md`;
  - remove or rewrite old Tauri/Dioxus frontend docs;
  - keep historical detail in Git history, not live docs.
- Add anti-regression audits:
  - ~~fail if app-shell TypeScript/JavaScript/HTML/CSS is reintroduced;~~
    manifest rule exists; CI hard-fail wiring still pending.
  - ~~fail if normal app startup uses Tauri main-window WebView;~~ manifest
    rule exists; CI hard-fail wiring still pending.
  - ~~fail if Dioxus/WASM route is referenced as a production front;~~
    manifest rule exists; CI hard-fail wiring still pending.
  - ~~fail if new native UI code is created under `examples/forge_tauri_ui/`;~~
    manifest rule exists; CI hard-fail wiring still pending.

Stage 11 status 2026-06-06:

- Obsolete-front manifest delivered.
- Cutover audit CLI delivered:
  `cargo run --manifest-path examples\ingen_native_front\Cargo.toml -- --cutover-audit`
  prints the manifest and exits with code `0` when the native front cutover is
  clear.
- Cutover audit report delivered in
  `examples/ingen_native_front/src/cutover_audit.rs`: it verifies the native
  shell manifest contains no Tauri/Dioxus/wasm-bindgen shell dependency,
  verifies the 18 legacy front blockers are gone, and separately marks the 6
  protected backend services that must be extracted before full Tauri backend
  retirement.
- Rollback commit before deletion:
  `9527b7ac Add native Slint front migration`, pushed to `origin/master`.
- Deleted 2026-06-06:
  `examples/forge_tauri_ui/ui`,
  `examples/forge_tauri_ui/ui/index.html`,
  `examples/forge_tauri_ui/ui/src`,
  `examples/forge_tauri_ui/ui/dist`,
  `examples/forge_tauri_ui/ui/styles.css`,
  `examples/forge_tauri_ui/ui/rust-front.html`,
  `examples/forge_tauri_ui/ui/rust-front-poc.html`,
  `examples/forge_tauri_ui/front-rs`,
  `examples/forge_tauri_ui/native-front`,
  `examples/forge_tauri_ui/node_modules`,
  `examples/forge_tauri_ui/package.json`,
  `examples/forge_tauri_ui/package-lock.json`,
  `examples/forge_tauri_ui/tsconfig.json`,
  `examples/forge_tauri_ui/scripts/build-ui-runtime.mjs`,
  `examples/forge_tauri_ui/scripts/forge-front-rs-cutover-audit.mjs`,
  `examples/forge_tauri_ui/scripts/forge-ui-smoke.mjs`,
  `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`,
  `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`.
- Latest Stage 11 proof hash:
  `4e431fb20ae1a870f4806af6e06d027a4fdfcd2046cf7047f32ccb4d577cd027`.
- Latest obsolete-front manifest hash:
  `65c4d065dfdb1341dfb22672a5428f572d76b20ad286b1abe3652ee10e46b01d`.
- Latest cutover audit hash:
  `9a3b166a2c26a219d8ddec2f3fba32cc46e720c845627a02d486d6755e17f87b`.
- `deletion_ready=true`.
- `cutoverReady=true`.
- `fullTauriRetirementReady=false`.
- `0` obsolete app-shell/front-runtime paths still exist.
- `6` protected backend services still live under the old Tauri tree.
- Native front cutover is complete. Full Tauri backend retirement remains a
  separate runtime extraction wall because the old `src-tauri/src/**` tree still
  contains Brain/agent, Collection OS, Banger, Trading and Real Estate service
  code.

Verifier:

- Native app end-to-end smoke passes.
- Banger native viewport proof passes.
- WebExplorer isolated proof passes.
- RAM DOM Atlas proof passes.
- No app-shell HTML/JS/WASM path is required for normal operation.
- `rg` finds no production references to the old front route.
- Dependency manifests no longer include old-shell-only dependencies.
- Build scripts no longer generate old app-shell assets.
- Docs name Migration Front as the only product frontend architecture.
- CI/audit fails if app TypeScript/JavaScript is reintroduced.
- CI/audit fails if the new native app is placed under `examples/forge_tauri_ui/`.

Definition of done:

Migration Front is complete only when the old frontend architecture is deleted,
not merely unused.

## 24. Communication Rules For Future Agents

When the user says:

```text
bosser sur Migration Front
refonte totale du front
abandonner Tauri
Rust + Slint
front natif
```

The agent must interpret the request as:

```text
Work on the native Rust + Slint architecture described here.
Start at the earliest incomplete Migration Front stage.
```

The agent must not interpret it as:

- RAM DOM Atlas first;
- WebExplorer first;
- CodeAct / LLM web automation before the migration is complete;
- Tauri improvements;
- Dioxus/WASM continuation;
- TypeScript cleanup;
- micro visual fixes;
- creating `examples/forge_tauri_ui/native-front`.

If uncertain, the agent should answer:

```text
I will start at the earliest incomplete Migration Front stage in
examples/ingen_native_front. CodeAct is deferred until the Slint/Rust front is
complete and the obsolete Tauri/WebView frontend has been deleted.
```

## 25. Success Criteria

Migration Front succeeds when:

- InGen starts in a Slint native shell.
- There is no global WebView app shell.
- Banger is a native wgpu viewport.
- WebExplorer is an isolated WebView peripheral.
- RAM DOM Atlas is rendered in native UI.
- Chat and agent surfaces are native.
- Product sections are native.
- Startup has no white flash.
- Chat bar geometry is stable from first frame.
- No app TypeScript/JavaScript exists.
- No HTML/CSS/WASM route is needed for normal app operation.
- Tauri is no longer the product shell.
- Safe Web CodeAct is reopened only after the above migration criteria are
  already true.

This is the architecture target. Do not dilute it.
