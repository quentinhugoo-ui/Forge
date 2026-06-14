# Migration Front

Status: Electron product shell is the only live frontend lane.

This document is the source of truth for frontend work. It is not an archive; historical migration detail belongs in Git history.

## Product Shell

The product frontend lives in:

```text
examples/ingen_electron_shell/**
```

The shell is Electron + React + TypeScript, connected to Rust through strict IPC:

```text
React renderer
-> preload context bridge
-> typed IPC contract
-> Electron main process
-> Rust backend projection
-> Forge / Monster / Brain services
```

The frontend must not introduce a second product shell. Browser content is a contained peripheral, not the application host.

## Current Authority

- Header, sidebars, canvas, right panel, chat bar and bottom controls are Electron renderer surfaces.
- Backend truth is Rust service projection plus typed IPC snapshots.
- Banger heavy rendering remains a native child-surface contract.
- WebExplorer remains a Rust-owned native WebView host.
- UI JSON directive blocks render through Electron stores and components.

## Hard Rules

- Do not recreate deleted frontend trees.
- Do not add a parallel browser shell, Tauri shell or ad-hoc web app.
- Do not bypass preload IPC with raw renderer access.
- Do not expose raw `ipcRenderer.send`, `sendSync` or Node integration to the renderer.
- Do not move Banger heavy rendering into DOM canvas as the product authority.
- Do not make WebExplorer the global app shell.
- Keep docs smaller after frontend cleanup; no historical migration archive in live docs.

## Frontend Files

Primary product files:

```text
examples/ingen_electron_shell/src/main/main.ts
examples/ingen_electron_shell/src/preload/preload.ts
examples/ingen_electron_shell/src/shared/ipc-contract.ts
examples/ingen_electron_shell/contract/src/main.rs
examples/ingen_electron_shell/src/renderer/App.tsx
examples/ingen_electron_shell/src/renderer/styles.css
examples/ingen_electron_shell/src/renderer/SidebarSlice.tsx
examples/ingen_electron_shell/src/renderer/CanvasSurfacesSlice.tsx
examples/ingen_electron_shell/src/renderer/RightPanelSlice.tsx
examples/ingen_electron_shell/src/renderer/PanelsChatBottomSlice.tsx
examples/ingen_electron_shell/scripts/final-cutover-audit.mjs
```

Generated IPC files:

```text
examples/ingen_electron_shell/src/shared/generated/forge-ipc.generated.ts
examples/ingen_electron_shell/src/shared/generated/forge-ipc.manifest.generated.json
examples/ingen_electron_shell/src/shared/generated/final-cutover-audit.generated.json
```

Assets:

```text
examples/ingen_electron_shell/public/shell-assets/**
examples/ingen_electron_shell/public/fonts/**
```

## IPC Contract

The Rust contract generator is canonical for shared IPC types:

```powershell
cd examples\ingen_electron_shell
npm.cmd run generate:ipc
```

Every renderer command must cross the preload bridge through a typed method. One user action maps to one explicit IPC handler.

Security defaults:

- `contextIsolation: true`
- `nodeIntegration: false`
- `sandbox: true`
- `webSecurity: true`
- guarded window opening
- guarded navigation

## UI Discipline

- Normal UI text uses Geist.
- Technical/proof/code text uses Geist Mono.
- Icon-only buttons need accessible labels and stable dimensions.
- Header and sidebar controls must not shift layout when their icons change.
- Sidebars animate with smooth CSS transitions and must respect reduced motion.
- Half-screen docking must keep canvas, chat bar and bottom controls aligned with sidebar state.
- Right panel stays visually empty unless a feature explicitly owns its content.
- Product state belongs in stores backed by typed snapshots, not static fixtures.

## Compute Frugality

Every recurring renderer workload must be treated like a Forge action:

- define a stable input contract and content address before heavy work,
- skip CPU, GPU, RAM and UI updates when the address is unchanged,
- materialize frame-wide constants once per content address instead of repeating
  them per pixel or per UI pass,
- measure impact outside the product UI with hardware counters when validating a
  renderer optimization,
- keep the production path optimized by default.

The Brain blob is the reference pattern: frame state is content-addressed,
duplicated frames are not submitted to WebGPU/WebGL, and the hue-rotation matrix
is a per-frame Forge artifact consumed by WebGPU/WebGL instead of being rebuilt
inside every fragment. Stable WebGPU draw commands are recorded once as a render
bundle and replayed instead of re-encoded every frame. The KASM frame spheres
also produce a content-addressed conservative scissor rectangle, so pixels
outside the possible blob volume do not execute the raymarch shader. The blob
canvas fills the Brain canvas instead of living in its own cropped HTML frame.
Once WebGPU is active, the hidden CSS fallback stops its own morph and gleam
animations.

## Verification

Minimum frontend gate:

```powershell
cd examples\ingen_electron_shell
npm.cmd test
npm.cmd run build
```

Cutover audit must report:

```text
status: cutover_complete
blockers: 0
warnings: 0
```

The audit also checks that the product shell has no dependency on removed frontend paths or names.

## Launch

Visible app:

```powershell
examples\ingen_electron_shell\run_ingen_electron_shell.vbs
```

Manual debug:

```powershell
examples\ingen_electron_shell\run_ingen_electron_shell.cmd
```

## Live Objectives

1. Keep Electron shell stable as the only product front.
2. Finish real backend wiring for remaining placeholder-looking surfaces.
3. Promote Banger native child surface from contract proof to interactive product surface.
4. Promote WebExplorer native host from policy contract to interactive product peripheral.
5. Keep visual polish iterative, verified and scoped to the Electron renderer.
