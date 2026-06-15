import type {
  ForgeShellApi,
  ForgeTerminalApi,
  WidgetWallpaperSampleBounds,
  WidgetWallpaperSampleResult
} from "../shared/ipc-contract";

interface ForgeWindowControlsApi {
  minimize: () => Promise<boolean>;
  toggleMaximize: () => Promise<boolean>;
  setWidgetMode?: (enabled: boolean, delayMs?: number) => Promise<boolean>;
  setWidgetHitRegions?: (regions: Array<{ x: number; y: number; width: number; height: number }>) => Promise<boolean>;
  setWidgetPanelExpanded?: (enabled: boolean) => Promise<boolean>;
  setWidgetClickThrough?: (enabled: boolean) => Promise<boolean>;
  setWidgetTaskbarAutoHide?: (enabled: boolean) => Promise<boolean>;
  toggleWidgetTaskbar?: () => Promise<boolean>;
  sampleWidgetWallpaper?: (bounds: WidgetWallpaperSampleBounds) => Promise<WidgetWallpaperSampleResult>;
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
