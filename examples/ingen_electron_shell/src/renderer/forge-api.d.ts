import type { ForgeShellApi, ForgeTerminalApi } from "../shared/ipc-contract";

interface ForgeWindowControlsApi {
  minimize: () => Promise<boolean>;
  toggleMaximize: () => Promise<boolean>;
  close: () => Promise<boolean>;
}

declare global {
  interface Window {
    forgeShell?: ForgeShellApi;
    forgeTerminal?: ForgeTerminalApi;
    forgeWindowControls?: ForgeWindowControlsApi;
  }
}

export {};
