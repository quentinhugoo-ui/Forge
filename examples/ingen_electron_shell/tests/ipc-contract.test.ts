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
  isAgentRuntimeEvent,
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
    const agentRuntimeEventMethod: keyof ForgeShellApi = "onAgentRuntimeEvent";
    expect(agentRuntimeEventMethod).toBe("onAgentRuntimeEvent");
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
    expect(isAgentActionRequest({ action: "run_command", scope: "computer", command: "powershell.exe", args: ["-NoProfile"], executionAdapter: "powershell", timeoutMs: 1000, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "run_command", command: "cmd.exe", executionAdapter: "cmd", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "run_command", command: "cmd.exe", executionAdapter: "telnet", confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "computer_inspect", maxResults: 10 })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_appshot", path: ".ingen-agent-artifacts/shot.png", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_focus_window", windowTitle: "InGen", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_clipboard_write", text: "hello", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_ui_tree", maxResults: 50 })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_click", x: 10, y: 20, button: "left", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_drag", x: 10, y: 20, toX: 30, toY: 40, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_scroll", deltaY: -240, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_type_text", text: "hello", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_inspect_url", url: "https://example.com" })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_download", url: "https://example.com/file.zip", path: "downloads/file.zip", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_open_url", url: "https://example.com", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_playwright_inspect", url: "https://example.com" })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_screenshot", url: "https://example.com", path: "shots/page.png", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_click", url: "https://example.com", selector: "button", confirmed: true, formSubmissionConfirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_type_text", url: "https://example.com", selector: "input[name=q]", text: "hello", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "browser_playwright_download", url: "https://example.com", selector: "a[download]", path: "downloads/file.txt", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "document_inspect", path: "report.md" })).toBe(true);
    expect(isAgentActionRequest({ action: "document_write_text", path: "report.md", content: "# Report" })).toBe(true);
    expect(isAgentActionRequest({ action: "document_write_json", path: "data.json", content: "{\"ok\":true}" })).toBe(true);
    expect(isAgentActionRequest({ action: "document_convert_text", path: "report.md", toPath: "report.txt" })).toBe(true);
    expect(isAgentActionRequest({ action: "document_pdf_extract_text", path: "report.pdf", toPath: "report.txt" })).toBe(true);
    expect(isAgentActionRequest({ action: "document_office_inspect", path: "report.docx", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "document_office_export_pdf", path: "report.docx", toPath: "report.pdf", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "document_image_ocr", path: "scan.png", query: "eng", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "document_media_metadata", path: "clip.mp4" })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_repo_status" })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_git_diff" })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_git_commit", title: "Commit message", paths: ["src/file.ts"], confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_git_push", remote: "origin", headBranch: "feature/test", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_github_pr_create", title: "PR", content: "Body", baseBranch: "master", headBranch: "feature/test", draft: true, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "dev_run_check", command: "npm.cmd", args: ["test"], confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "cloud_cli_inspect", cloudProvider: "all" })).toBe(true);
    expect(isAgentActionRequest({ action: "cloud_cli_run_readonly", cloudProvider: "aws", args: ["sts", "get-caller-identity"], account: "123456789012" })).toBe(true);
    expect(isAgentActionRequest({ action: "cloud_cli_run_write", cloudProvider: "gcp", args: ["run", "deploy", "svc"], project: "demo", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "windows_setting_inspect", settingName: "os" })).toBe(true);
    expect(isAgentActionRequest({ action: "windows_setting_apply", path: "HKCU:\\Software\\InGenTest", settingName: "Value", content: "1", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "process_service_inspect", query: "node" })).toBe(true);
    expect(isAgentActionRequest({ action: "process_service_control", serviceName: "Spooler", command: "restart", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "package_inspect", packageId: "Git.Git" })).toBe(true);
    expect(isAgentActionRequest({ action: "package_install_update", packageId: "Git.Git", command: "upgrade", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "ci_checks_inspect", headBranch: "master" })).toBe(true);
    expect(isAgentActionRequest({ action: "ci_run_inspect", query: "123" })).toBe(true);
    expect(isAgentActionRequest({ action: "virtualization_inspect", provider: "all", maxResults: 10 })).toBe(true);
    expect(isAgentActionRequest({ action: "virtualization_run_command", provider: "wsl", distro: "Ubuntu", command: "true", nativeFallback: true, confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "virtualization_run_command", provider: "docker", container: "dev", command: "node", args: ["--version"], confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "automation_schedule", title: "Daily check", command: "cmd.exe", args: ["/c", "echo ok"], taskName: "DailyCheck", scheduleType: "ONCE", startTime: "23:59", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "automation_list", maxResults: 10 })).toBe(true);
    expect(isAgentActionRequest({ action: "automation_cancel", taskName: "InGenAgent_DailyCheck", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "automation_record", title: "Daily build check", confirmed: true })).toBe(true);
    expect(isAgentActionRequest({ action: "computer_focus_window", windowTitle: 42, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "computer_click", x: 10.5, y: 20, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "computer_click", x: 10, y: 20, button: "primary", confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "browser_inspect_url", url: 42 })).toBe(false);
    expect(isAgentActionRequest({ action: "browser_click", url: "https://example.com", selector: 42, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "browser_click", url: "https://example.com", selector: "button", confirmed: true, formSubmissionConfirmed: "yes" })).toBe(false);
    expect(isAgentActionRequest({ action: "document_write_text", path: "report.md", content: 42 })).toBe(false);
    expect(isAgentActionRequest({ action: "document_office_inspect", path: "report.docx", macroExecutionConfirmed: "yes" })).toBe(false);
    expect(isAgentActionRequest({ action: "dev_git_commit", title: "Bad", paths: [42], confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "dev_github_pr_create", title: "Bad", draft: "yes", confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "cloud_cli_inspect", cloudProvider: "digitalocean" })).toBe(false);
    expect(isAgentActionRequest({ action: "cloud_cli_run_readonly", cloudProvider: "aws", tenant: 42 })).toBe(false);
    expect(isAgentActionRequest({ action: "package_inspect", packageId: 42 })).toBe(false);
    expect(isAgentActionRequest({ action: "process_service_control", serviceName: 42, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "virtualization_inspect", provider: "xhyve" })).toBe(false);
    expect(isAgentActionRequest({ action: "virtualization_run_command", provider: "wsl", nativeFallback: "yes", confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "automation_schedule", title: "Bad", command: "cmd.exe", startTime: 2359, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "automation_record", title: 42, confirmed: true })).toBe(false);
    expect(isAgentActionRequest({ action: "delete_empty_directory", path: "tmp", confirmed: "yes" })).toBe(false);
    expect(isAgentActionRequest({ action: "list", scope: "galaxy", path: "." })).toBe(false);
    expect(isAgentActionRequest({ action: "raw_shell", command: "powershell.exe" })).toBe(false);
  });

  it("accepts versioned agent runtime events and rejects loose variants", () => {
    expect(
      isAgentRuntimeEvent({
        schema: "ingen.agent_runtime.event.v1",
        kind: "tool_call_started",
        sessionId: "session-1",
        messageId: "assistant-1",
        sequence: 1,
        at: 1710000000000,
        toolCall: {
          id: "tool-1",
          name: "fs.list",
          request: { action: "list", path: "." },
          status: "pending",
          startedAt: 1710000000000
        },
        proofHash: "runtime-proof"
      })
    ).toBe(true);
    expect(isAgentRuntimeEvent({ kind: "tool_call_started", sessionId: "session-1" })).toBe(false);
    expect(
      isAgentRuntimeEvent({
        schema: "ingen.agent_runtime.event.v1",
        kind: "raw_json",
        sessionId: "session-1",
        sequence: 1,
        at: 1710000000000,
        proofHash: "runtime-proof"
      })
    ).toBe(false);
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
