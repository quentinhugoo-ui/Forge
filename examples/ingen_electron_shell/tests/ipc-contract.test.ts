import { describe, expect, it } from "vitest";
import manifest from "../src/shared/generated/forge-ipc.manifest.generated.json";
import {
  FORGE_ELECTRON_IPC_CONTRACT_SOURCE,
  FORGE_ELECTRON_IPC_VERSION,
  CANVAS_SURFACES_COMMAND_KIND,
  HEADER_COMMAND_KIND,
  HEADER_SURFACE_KIND,
  PANELS_CHAT_BOTTOM_COMMAND_KIND,
  RIGHT_PANEL_COMMAND_KIND,
  isHeaderCommand,
  isHeaderSurfaceSnapshot,
  isCanvasSurfacesCommand,
  isCanvasSurfacesSnapshot,
  isRightPanelCommand,
  isRightPanelSnapshot,
  isNativeSection,
  isPanelsChatBottomCommand,
  isAgentActionRequest,
  type ForgeShellApi,
  type HeaderCommand,
  type HeaderSurfaceSnapshot,
  type CanvasSurfacesSnapshot,
  type RightPanelSnapshot
} from "../src/shared/ipc-contract";

describe("typed header IPC contract", () => {
  it("loads the generated Rust-owned IPC contract", () => {
    expect(FORGE_ELECTRON_IPC_CONTRACT_SOURCE).toBe("rust:ingen-electron-ipc-contract");
    expect(manifest.schema).toBe("ingen.electron.ipc_contract_manifest.v1");
    expect(manifest.source).toBe("examples/ingen_electron_shell/contract/src/main.rs");
    expect(manifest.proof_hash).toMatch(/^[a-f0-9]{64}$/);
    expect(HEADER_COMMAND_KIND).toContain("navigate_workspace");
    expect(HEADER_SURFACE_KIND).toContain("banger_native_child");
    expect(PANELS_CHAT_BOTTOM_COMMAND_KIND).toContain("send_chat");
    expect(CANVAS_SURFACES_COMMAND_KIND).toContain("request_native_surface");
    expect(RIGHT_PANEL_COMMAND_KIND).toContain("select_tab");
    const searchArchiveMethod: keyof ForgeShellApi = "searchArchive";
    expect(searchArchiveMethod).toBe("searchArchive");
    const agentActionMethod: keyof ForgeShellApi = "executeAgentAction";
    expect(agentActionMethod).toBe("executeAgentAction");
  });

  it("accepts versioned commands with request ids", () => {
    const command: HeaderCommand = {
      version: FORGE_ELECTRON_IPC_VERSION,
      requestId: "test-1",
      kind: "navigate_workspace",
      section: "webexplorer"
    };
    expect(isHeaderCommand(command)).toBe(true);
  });

  it("rejects navigation outside native section variants", () => {
    expect(
      isHeaderCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "test-2",
        kind: "navigate_workspace",
        section: "external-browser"
      })
    ).toBe(false);
    expect(isNativeSection("banger")).toBe(true);
    expect(isNativeSection("external-browser")).toBe(false);
  });

  it("rejects unversioned or loose payloads", () => {
    expect(isHeaderCommand({ kind: "open_banger" })).toBe(false);
    expect(isHeaderCommand({ version: FORGE_ELECTRON_IPC_VERSION, requestId: "x", kind: "raw_json" })).toBe(false);
    expect(
      isHeaderCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "x",
        kind: "open_banger",
        section: "banger"
      })
    ).toBe(false);
  });

  it("accepts generated header surface snapshots", () => {
    const snapshot: HeaderSurfaceSnapshot = {
      schema: "ingen.electron.header.surface_snapshot.v1",
      version: FORGE_ELECTRON_IPC_VERSION,
      mode: "shadow",
      activeSection: "banger",
      profileCanvas: "",
      surfaces: [
        {
          id: "banger-native-child-surface",
          kind: "banger_native_child",
          label: "Banger native viewport",
          route: "banger",
          authority: "electron-shadow",
          status: "native_pending",
          slot: { x: 287, y: 96, width: 1248, height: 690 },
          nativeContract: "wgpu-child-window-frame-hash",
          sourceComponent: "BangerNativeViewport",
          summary: "Electron reserves the slot only.",
          proofHash: "surface-proof"
        }
      ],
      proofHash: "snapshot-proof"
    };

    expect(isHeaderSurfaceSnapshot(snapshot)).toBe(true);
    expect(isHeaderSurfaceSnapshot({ ...snapshot, activeSection: "external" })).toBe(false);
  });

  it("accepts typed panels/chat/bottom commands and rejects loose variants", () => {
    expect(
      isPanelsChatBottomCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "pcb-1",
        kind: "send_chat",
        value: "/newcompute_",
        attachmentIds: ["upload-proof-1"]
      })
    ).toBe(true);
    expect(
      isPanelsChatBottomCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "pcb-bad-attachments",
        kind: "send_chat",
        value: "/newcompute_",
        attachmentIds: ["ok", 3]
      })
    ).toBe(false);
    expect(
      isPanelsChatBottomCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "pcb-2",
        kind: "select_llm",
        provider: "external-browser"
      })
    ).toBe(false);
    expect(
      isPanelsChatBottomCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "pcb-3",
        kind: "raw_json"
      })
    ).toBe(false);
  });

  it("accepts bounded agent action host requests", () => {
    expect(isAgentActionRequest({ action: "list", path: "." })).toBe(true);
    expect(isAgentActionRequest({ action: "search", query: "needle", maxResults: 25 })).toBe(true);
    expect(isAgentActionRequest({ action: "delete_empty_directory", path: "tmp", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "delete_tree", path: "tmp", recursive: true, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "run_command", scope: "computer", command: "powershell.exe", args: ["-NoProfile"], timeoutMs: 1000, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "delete_empty_directory", path: "tmp", confirmed: "yes" })).toBe(false);
    expect(isAgentActionRequest({ action: "list", scope: "galaxy", path: "." })).toBe(false);
    expect(isAgentActionRequest({ action: "raw_shell", command: "powershell.exe" })).toBe(false);
  });

  it("accepts typed canvas surface snapshots and rejects loose commands", () => {
    const snapshot: CanvasSurfacesSnapshot = {
      schema: "ingen.electron.canvas_surfaces.snapshot.v1",
      version: FORGE_ELECTRON_IPC_VERSION,
      mode: "shadow",
      activeSection: "webexplorer",
      profileCanvas: "",
      activeSurfaceId: "webexplorer-webview-host",
      surfaces: [
        {
          id: "webexplorer-webview-host",
          kind: "webexplorer_webview",
          label: "Rust WebView host",
          route: "webexplorer",
          status: "native_pending",
          sourceComponent: "GoogleWebViewCanvas",
          nativeContract: "rust-owned-webview-policy-host",
          authority: "electron-shadow",
          headline: "WebExplorer native web peripheral",
          detail: "Rust owns policy and capture.",
          proofHash: "surface-proof"
        }
      ],
      nativeSurfacePolicy: { banger: "child-window", webexplorer: "rust-owned-webview" },
      proofHash: "snapshot-proof"
    };

    expect(isCanvasSurfacesSnapshot(snapshot)).toBe(true);
    expect(
      isCanvasSurfacesCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "cvs-1",
        kind: "request_native_surface",
        section: "webexplorer"
      })
    ).toBe(true);
    expect(
      isCanvasSurfacesCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "cvs-2",
        kind: "request_native_surface",
        section: "external-browser"
      })
    ).toBe(false);
    expect(isCanvasSurfacesSnapshot({ ...snapshot, profileCanvas: "external" })).toBe(false);
  });

  it("accepts typed right panel snapshots and rejects loose commands", () => {
    const snapshot: RightPanelSnapshot = {
      schema: "ingen.electron.right_panel.snapshot.v1",
      version: FORGE_ELECTRON_IPC_VERSION,
      mode: "shadow",
      activeSection: "forge",
      profileCanvas: "",
      open: true,
      activeTab: "status",
      title: "Section status dock",
      summary: "Right panel shadow projection.",
      tabs: [{ id: "status", label: "Status", selected: true, count: 2 }],
      lines: [{ label: "Jobs", value: "queued=0", severity: "ok", proofHash: "jobs-proof" }],
      actions: [{ id: "refresh", label: "Refresh", command: "refresh", enabled: true }],
      proofHash: "snapshot-proof"
    };

    expect(isRightPanelSnapshot(snapshot)).toBe(true);
    expect(
      isRightPanelCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "rp-1",
        kind: "select_tab",
        target: "native"
      })
    ).toBe(true);
    expect(
      isRightPanelCommand({
        version: FORGE_ELECTRON_IPC_VERSION,
        requestId: "rp-2",
        kind: "raw_json"
      })
    ).toBe(false);
    expect(isRightPanelSnapshot({ ...snapshot, profileCanvas: "external" })).toBe(false);
  });
});
