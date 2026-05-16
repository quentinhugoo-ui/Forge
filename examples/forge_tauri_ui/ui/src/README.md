# Forge UI TypeScript Migration

This folder is the UI source of truth. Browser JavaScript is generated from TypeScript into `ui/dist/`; hand-written `.js` files outside generated/vendor folders are forbidden.

Rules:

- shell state is changed only by typed events in `shell/shell-machine.ts`;
- sections register through the typed registry instead of adding global click handlers;
- Tauri calls go through one client facade;
- intent, trace and distillation debug surfaces use `shell/intent-surface.ts` and the shell runtime projection, never hand-written JS or direct browser-side MCP/Tauri calls;
- transitional surface cells in `shell/surface.ts` may mirror state into the runtime, but must not become the owner of new sections;
- every migrated slice gets a smoke-test contract before more UI is moved.
