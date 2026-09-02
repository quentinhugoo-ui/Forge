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

interface GraphenLeadAttachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  createdAt: number;
}

interface GraphenLeadMessage {
  role: "user" | "assistant" | "system";
  content?: string;
  text?: string;
}

interface GraphenLead {
  id: string;
  conversationId: string;
  nameOrCompany: string;
  phone: string;
  email: string;
  siteUrl: string;
  status: string;
  createdAt: number;
  updatedAt: number;
  messages: GraphenLeadMessage[];
  lastReply: string;
  attachments: GraphenLeadAttachment[];
}

interface GraphenLeadsApi {
  getGraphenLeads: () => Promise<{ leads: GraphenLead[]; error?: string }>;
  openGraphenLeadAttachment: (id: string, filename: string) => Promise<{ accepted: boolean; error?: string }>;
}

declare global {
  interface Window {
    forgeShell?: ForgeShellApi & GraphenLeadsApi;
    forgeTerminal?: ForgeTerminalApi;
    forgeWindowControls?: ForgeWindowControlsApi;
  }
}

export {};
