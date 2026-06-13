# InGen Electron Shell

Deliberate product-front lane for the Slint -> Electron migration described in
`MIGRATION_FRONT.md`.

This is not the deleted legacy browser shell. It is a clean Electron / React /
TypeScript shell now driven by the migration factory:

- frozen boundaries for `header`, `sidebar_sessions`, `canvas_surfaces`,
  `right_panel`, and `panels_chat_bottom`;
- generated Slint inventory, slice registry, skeleton components and lock docs;
- existing `TitleBar`, `WorkspaceHeader`, sidebar/session and
  panels/chat/bottom slices referenced without rewrite;
- generated token bridge from `ForgeTokens`;
- generated parity and backend binding ledgers;
- strict, versioned IPC types;
- `FORGE_FRONT_SLICE_*=slint|electron|shadow` cutover discipline.

Default mode is `shadow`: Electron can render and produce compact manifests, but
Rust/Slint remain the authority until parity is proven.

## Commands

```powershell
npm install
npm run generate:front-ledger
npm run factory:check
npm run typecheck
npm test
npm run build
```

The Electron security baseline is locked from day zero: `contextIsolation`,
`nodeIntegration: false`, sandboxed renderer, minimal preload, no raw
`ipcRenderer`, and a local custom protocol for production assets.
