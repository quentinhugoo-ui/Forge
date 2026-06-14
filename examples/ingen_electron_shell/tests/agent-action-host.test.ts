import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  agentActionCapabilityDetailManifest,
  createAgentCapabilityAtlas,
  createAgentActionHostManifest,
  createAgentActionRuntimeManifestSummary,
  createBrowserWebPolicy,
  createComputerUsePolicy,
  createDeveloperAutomationPolicy,
  createDocumentMediaPolicy,
  createAgentVerificationPolicy,
  createWindowsExecutionPolicy,
  detectAgentActionInstalledTools,
  agentActionEventCommandForRequest,
  agentActionHostPromptManifest,
  agentActionRoutingHint,
  executeAgentActionRequest,
  type AgentActionHostConfig
} from "../src/main/agent-action-host";

async function withTempWorkspace<T>(run: (config: AgentActionHostConfig, root: string) => Promise<T>): Promise<T> {
  const root = join(tmpdir(), `ingen-agent-action-host-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  await mkdir(root, { recursive: true });
  const config: AgentActionHostConfig = {
    workspaceRoot: root,
    workspaceActive: true,
    cwd: root,
    platform: process.platform
  };
  try {
    return await run(config, root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

describe("agent action host", () => {
  const windowsIt = process.platform === "win32" ? it : it.skip;
  const schedulerIt = process.platform === "win32" && spawnSync("schtasks.exe", ["/?"], { encoding: "utf8", stdio: "pipe" }).status === 0 ? it : it.skip;
  const gitIt = spawnSync("git", ["--version"], { encoding: "utf8", stdio: "pipe" }).status === 0 ? it : it.skip;

  it("publishes bounded local-agent capabilities", () => {
    const manifest = createAgentActionHostManifest({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(manifest.schema).toBe("ingen.agent_action_host.manifest.v1");
    expect(manifest.permissions.sandbox).toBe("workspace_or_confirmed_computer");
    expect(manifest.permissions.recursiveDelete).toBe("confirmed_with_absolute_path_guard");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("fs.search");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("fs.delete_tree");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("shell.full");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("shell.readonly");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("browser.inspect_url");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("browser.download");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("browser.playwright_inspect");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("browser.playwright_download");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("document.inspect");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("document.write_json");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("dev.repo_status");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("dev.git_commit");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("dev.git_push");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("dev.github_pr_create");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("virtualization.inspect");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("virtualization.run_command");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("automation.schedule");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("automation.cancel");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("automation.record");
    expect(manifest.capabilityAtlas.length).toBeGreaterThanOrEqual(15);
    expect(manifest.capabilityAtlas.map((capability) => capability.family)).toContain("windows.wmi");
    expect(manifest.capabilityAtlas.map((capability) => capability.family)).toContain("browser.cdp");
    expect(manifest.runtime.schema).toBe("ingen.agent_action_runtime_manifest.summary.v1");
    expect(manifest.runtime.manifestHash).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.runtime.atlasHash).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.runtime.installedToolsHash).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.runtime.windowsExecutionHash).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.runtime.verificationHash).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.runtime.injectionPolicy).toBe("full_on_local_intent_compact_delta_on_continuation");
    expect(manifest.runtime.resultReinjectionPolicy).toBe("compact_tool_result_is_ground_truth_each_round");
    expect(manifest.runtime.executableActionIds).toContain("shell.full");
    expect(manifest.installedTools.length).toBeGreaterThan(5);
    expect(manifest.windowsExecution.schema).toBe("ingen.windows_execution.policy.v1");
    expect(manifest.windowsExecution.adapters).toEqual(["powershell", "cmd", "windows_command", "shell_full"]);
    expect(manifest.windowsExecution.routeCatalog.map((route) => route.id)).toContain("scheduler.schtasks");
    expect(manifest.windowsExecution.routeCatalog.map((route) => route.id)).toContain("settings.ms_settings");
    expect(manifest.verification.schema).toBe("ingen.agent_verification.policy.v1");
    expect(manifest.verification.mutationCompletionRule).toBe("verified_or_blocked");
    expect(manifest.computerUse.schema).toBe("ingen.computer_use.policy.v1");
    expect(manifest.computerUse.executableActions).toContain("computer_inspect");
    expect(manifest.computerUse.executableActions).toContain("computer_ui_tree");
    expect(manifest.computerUse.executableActions).toContain("computer_click");
    expect(manifest.browserWeb.schema).toBe("ingen.browser_web.policy.v1");
    expect(manifest.browserWeb.executableActions).toContain("browser_download");
    expect(manifest.documentMedia.schema).toBe("ingen.document_media.policy.v1");
    expect(manifest.documentMedia.executableActions).toContain("document_write_json");
    expect(manifest.developerAutomation.schema).toBe("ingen.developer_automation.policy.v1");
    expect(manifest.developerAutomation.executableActions).toContain("dev_repo_status");
    expect([...manifest.runtime.installedToolIds, ...manifest.runtime.missingToolIds]).toContain("winget");
    expect(manifest.proofHash).toMatch(/^[a-f0-9]{64}$/);

    const promptManifest = agentActionHostPromptManifest({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });
    expect(promptManifest).toContain("events=fs.list:/agent_list_");
    expect(promptManifest).toContain("fs.delete_tree:/agent_delete_tree_");
    expect(promptManifest).toContain("shell.full:/agent_shell_");
    expect(promptManifest).toContain("dev.github_pr_create:/agent_github_pr_create_");
    expect(promptManifest).toContain("virtualization.inspect:/agent_virtualization_inspect_");
    expect(promptManifest).toContain("automation.schedule:/agent_automation_schedule_");
    expect(promptManifest).toContain("capability_atlas=count:");
    expect(promptManifest).toContain("manifest_hash=");
    expect(promptManifest).toContain("atlas_hash=");
    expect(promptManifest).toContain("windows_execution_hash=");
    expect(promptManifest).toContain("verification_hash=");
    expect(promptManifest).toContain("injection_policy=full_on_local_intent_compact_delta_on_continuation");
    expect(promptManifest).toContain("prompt_budget=compact_by_default_detail_on_selected_capability");
    expect(promptManifest).toContain("result_reinjection=compact_tool_result_is_ground_truth_each_round");
    expect(promptManifest).toContain("token_estimate_full=");
    expect(promptManifest).toContain("token_estimate_compact=");
    expect(promptManifest).toContain("token_estimate_selected_capability=");
    expect(promptManifest).toContain("installed_tools=");
    expect(promptManifest).toContain("missing_tools=");
    expect(promptManifest).toContain("windows_adapters=powershell|cmd|windows_command|shell_full");
    expect(promptManifest).toContain("windows_routes=powershell.inline|cmd.inline|winget.package");
    expect(promptManifest).toContain("windows_timeout=default:");
    expect(promptManifest).toContain("verification_policy=verified_or_blocked");
    expect(promptManifest).toContain("failure_categories=denied|missing_tool|bad_path|timeout|permission|protected_root|command_error|unverifiable|partial_success");
    expect(promptManifest).toContain("retry_strategies=api_cli|powershell|cmd|windows_command|wmi_cim|registry|settings_uri|browser_cdp|gui_computer_use|manual_approval");
    expect(promptManifest).toContain("computer_use=foreground_required_for_risky_gui_actions pacing=single_action_then_verify");
    expect(promptManifest).toContain("browser_web=download:confirmed navigation:confirmed submission:confirmed");
    expect(promptManifest).toContain("document_media=workspace_writes:open computer_writes:confirmed office_com:confirmed");
    expect(promptManifest).toContain("developer_automation=repo_inspect:open checks:confirmed git_mutation:confirmed cloud_writes:confirmed mcp:planned_connector_required automation:confirmed");
    expect(promptManifest).toContain("windows.wmi:planned/none");
    expect(promptManifest).toContain("office.com:planned/prompt");
    expect(promptManifest).toContain("capability_policy=Use the atlas for reasoning");
    expect(promptManifest).toContain("Prefer structured app/API/CLI routes first, then confirmed shell.full, then GUI/computer-use");
    expect(promptManifest).toContain("capability_limits=Planned or blocked atlas entries are not direct AGENT_ACTION_JSON actions");
    expect(promptManifest).toContain("windows_reach=Prefer typed adapters powershell|cmd|windows_command before shell_full");
    expect(promptManifest).toContain("retry=If AGENT_ACTION_RESULT reports failure");
    expect(promptManifest).toContain("loop_stream=When local action is needed");
    expect(promptManifest).toContain("loop_style=Use varied, concrete progress notes");
    expect(promptManifest).toContain("starts with AGENT_ACTION_JSON at column 1");
    expect(promptManifest).toContain("action_request_format=AGENT_ACTION_JSON");
    expect(promptManifest).toContain("tool_truth=Never claim an action was executed");
  }, 10_000);

  it("publishes a compact runtime manifest summary for delta prompt injection", () => {
    const summary = createAgentActionRuntimeManifestSummary({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(summary.manifestHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.atlasHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.installedToolsHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.windowsExecutionHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.verificationHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.executableActionIds).toEqual([
      "fs.list",
      "fs.search",
      "fs.create_directory",
      "fs.rename",
      "fs.move",
      "fs.copy",
      "fs.delete_empty_directory",
      "fs.delete_tree",
      "shell.readonly",
      "shell.full",
      "computer.inspect",
      "computer.appshot",
      "computer.focus_window",
      "computer.clipboard",
      "computer.ui_tree",
      "computer.ocr",
      "computer.click",
      "computer.type_text",
      "computer.scroll",
      "computer.drag",
      "browser.inspect_url",
      "browser.download",
      "browser.open_url",
      "browser.playwright_inspect",
      "browser.screenshot",
      "browser.click",
      "browser.type_text",
      "browser.playwright_download",
      "document.inspect",
      "document.write_text",
      "document.write_json",
      "document.write_csv",
      "document.convert_text",
      "dev.repo_status",
      "dev.git_diff",
      "dev.git_commit",
      "dev.git_push",
      "dev.github_pr_create",
      "dev.run_check",
      "virtualization.inspect",
      "virtualization.run_command",
      "automation.schedule",
      "automation.list",
      "automation.cancel",
      "automation.record"
    ]);
    expect(summary.availableFamilies).toContain("shell.full");
    expect(summary.availableFamilies).toContain("windows.settings");
    expect(summary.blockedFamilies).toContain("windows.credentials");
    expect(summary.approvalGatedFamilies).toContain("browser.cdp");
    expect(summary.promptBudget).toBe("compact_by_default_detail_on_selected_capability");
    expect(summary.promptTokenEstimate.fullManifest).toBeGreaterThan(summary.promptTokenEstimate.compactContinuation);
    expect(summary.promptTokenEstimate.selectedCapabilityDetail).toBeGreaterThan(0);
    expect([...summary.installedToolIds, ...summary.missingToolIds]).toContain("powershell");
    expect(summary.windowsRouteIds).toContain("registry.reg");
    expect(summary.windowsRouteIds).toContain("files.robocopy");
  });

  it("publishes a typed Windows execution route catalog", () => {
    const policy = createWindowsExecutionPolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });
    const byId = new Map(policy.routeCatalog.map((route) => [route.id, route]));

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.confirmationPolicy).toBe("computer_writes_and_shell_full_require_confirmed_true");
    expect(policy.cancellationPolicy).toBe("timeout_kills_child_and_reports_timed_out");
    for (const id of [
      "winget.package",
      "registry.reg",
      "scheduler.schtasks",
      "network.netsh",
      "deployment.dism",
      "services.sc",
      "processes.tasklist",
      "processes.taskkill",
      "files.robocopy",
      "security.icacls",
      "certificates.certutil",
      "events.wevtutil",
      "virtualization.wsl",
      "shell.start_process",
      "settings.ms_settings"
    ]) {
      expect(byId.has(id)).toBe(true);
      expect(byId.get(id)?.readScenario).toBeTruthy();
      expect(byId.get(id)?.gatedWriteScenario).toBeTruthy();
      expect(byId.get(id)?.verification.length).toBeGreaterThan(0);
    }
    expect(byId.get("processes.tasklist")?.approval).toBe("none");
    expect(byId.get("registry.reg")?.approval).toBe("confirmed");
    expect(byId.get("shell.full")?.adapter).toBe("shell_full");
  });

  it("publishes a verification and retry policy for local actions", () => {
    const policy = createAgentVerificationPolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.probeKinds).toContain("filesystem");
    expect(policy.probeKinds).toContain("command_exit");
    expect(policy.failureCategories).toContain("protected_root");
    expect(policy.retryStrategies.map((strategy) => strategy.id)).toContain("powershell");
    expect(policy.retryStrategies.map((strategy) => strategy.id)).toContain("manual_approval");
    expect(policy.protectedBoundaryRule).toBe("block_without_retry");
  });

  it("publishes a foreground computer-use policy", () => {
    const policy = createComputerUsePolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.executableActions).toContain("computer_appshot");
    expect(policy.executableActions).toContain("computer_ui_tree");
    expect(policy.executableActions).toContain("computer_drag");
    expect(policy.interactionRequiresConfirmation).toBe(true);
    expect(policy.userPresenceMode).toBe("foreground_required_for_risky_gui_actions");
    expect(policy.pacingPolicy).toBe("single_action_then_verify");
    expect(policy.forbiddenPrompts).toContain("credential");
    expect(policy.forbiddenPrompts).toContain("uac");
  });

  it("publishes a browser/web policy for verified downloads and gated navigation", () => {
    const policy = createBrowserWebPolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.executableActions).toEqual([
      "browser_inspect_url",
      "browser_download",
      "browser_open_url",
      "browser_playwright_inspect",
      "browser_screenshot",
      "browser_click",
      "browser_type_text",
      "browser_playwright_download"
    ]);
    expect(policy.inspectionRequiresConfirmation).toBe(false);
    expect(policy.downloadRequiresConfirmation).toBe(true);
    expect(policy.navigationRequiresConfirmation).toBe(true);
    expect(policy.submissionRequiresConfirmation).toBe(true);
    expect(policy.credentialPromptPolicy).toBe("never_fill_or_submit_without_user");
    expect(policy.artifactPolicy).toBe("persist_downloads_with_size_and_sha256");
  });

  it("publishes a document/media policy for verified artifacts", () => {
    const policy = createDocumentMediaPolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.executableActions).toEqual([
      "document_inspect",
      "document_write_text",
      "document_write_json",
      "document_write_csv",
      "document_convert_text",
      "document_pdf_extract_text",
      "document_office_inspect",
      "document_office_export_pdf",
      "document_image_ocr",
      "document_media_metadata",
      "document_toolchain_inspect",
      "document_toolchain_install"
    ]);
    expect(policy.workspaceWritesRequireConfirmation).toBe(false);
    expect(policy.computerScopeWritesRequireConfirmation).toBe(true);
    expect(policy.officeComRequiresConfirmation).toBe(true);
    expect(policy.macroPolicy).toBe("blocked_without_explicit_user_approval");
    expect(policy.artifactPolicy).toBe("verify_readback_size_hash_and_parser_status");
  });

  it("publishes a developer/cloud/MCP/automation policy", () => {
    const policy = createDeveloperAutomationPolicy({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(policy.proofHash).toMatch(/^[a-f0-9]{64}$/);
    expect(policy.executableActions).toEqual([
      "dev_repo_status",
      "dev_git_diff",
      "dev_git_commit",
      "dev_git_push",
      "dev_github_pr_create",
      "dev_github_pr_review_submit",
      "dev_run_check",
      "cloud_cli_inspect",
      "cloud_cli_run_readonly",
      "cloud_cli_run_write",
      "windows_setting_inspect",
      "windows_setting_apply",
      "windows_sensitive_inspect",
      "windows_sensitive_apply",
      "process_service_inspect",
      "process_service_control",
      "package_inspect",
      "package_install_update",
      "ci_checks_inspect",
      "ci_run_inspect",
      "ci_rerun_failed",
      "virtualization_inspect",
      "virtualization_run_command",
      "automation_schedule",
      "automation_list",
      "automation_cancel",
      "automation_record"
    ]);
    expect(policy.repoInspectionRequiresConfirmation).toBe(false);
    expect(policy.commandChecksRequireConfirmation).toBe(true);
    expect(policy.gitMutationRequiresConfirmation).toBe(true);
    expect(policy.cloudWritesRequireConfirmation).toBe(true);
    expect(policy.mcpToolCallingStatus).toBe("planned_connector_required");
    expect(policy.automationPersistenceRequiresConfirmation).toBe(true);
    expect(policy.artifactPolicy).toBe("verify_command_exit_git_state_or_ledger_hash");
  });

  it("detects local tool availability and renders selected capability detail on demand", () => {
    const config: AgentActionHostConfig = {
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    };
    const tools = detectAgentActionInstalledTools(config);
    expect(tools.map((tool) => tool.id)).toContain("powershell");
    expect(tools.map((tool) => tool.id)).toContain("winget");

    const detail = agentActionCapabilityDetailManifest(config, "shell.full");
    expect(detail).toContain("AGENT_ACTION_CAPABILITY_DETAIL v1");
    expect(detail).toContain("id=shell.full");
    expect(detail).toContain("family=shell.full");
    expect(detail).toContain("fallbacks=PowerShell|cmd.exe|Windows native CLI|GUI/computer_use when available");
    expect(detail).toContain("verification=command_exit|filesystem|process_state|registry_state|service_state|package_state");
    expect(detail).toContain("rule=Use this detail as context only");
  });

  it("publishes a broad non-executable Windows capability atlas", () => {
    const atlas = createAgentCapabilityAtlas({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });
    const byFamily = new Map(atlas.map((capability) => [capability.family, capability]));

    for (const family of [
      "office.com",
      "browser.cdp",
      "windows.wmi",
      "windows.scheduler",
      "windows.settings",
      "windows.credentials",
      "virtualization.wsl",
      "computer.ui_automation",
      "automation.rpa"
    ]) {
      expect(byFamily.has(family)).toBe(true);
    }

    expect(atlas.length).toBeGreaterThanOrEqual(15);
    expect(byFamily.get("windows.credentials")?.status).toBe("blocked");
    expect(byFamily.get("windows.credentials")?.approval).toBe("blocked");
    expect(byFamily.get("windows.scheduler")?.status).toBe("available");
    expect(byFamily.get("windows.scheduler")?.approval).toBe("prompt");
    expect(byFamily.get("windows.settings")?.approval).toBe("prompt");
    expect(byFamily.get("browser.cdp")?.status).toBe("available");
    expect(byFamily.get("browser.cdp")?.executableActionIds).toEqual([
      "browser.playwright_inspect",
      "browser.screenshot",
      "browser.click",
      "browser.type_text",
      "browser.playwright_download"
    ]);
    expect(byFamily.get("virtualization.wsl")?.status).toBe("available");
    expect(byFamily.get("computer.ui_automation")?.status).toBe("available");
    expect(byFamily.get("computer.ui_automation")?.executableActionIds).toEqual([
      "computer.ui_tree",
      "computer.ocr",
      "computer.click",
      "computer.type_text",
      "computer.scroll",
      "computer.drag"
    ]);
    expect(byFamily.get("virtualization.hyperv_docker")?.status).toBe("available");
    expect(byFamily.get("virtualization.wsl")?.executableActionIds).toEqual(["virtualization.inspect", "virtualization.run_command"]);
    expect(byFamily.get("office.com")?.executableActionIds).toBeUndefined();
    expect(byFamily.get("shell.full")?.executableActionIds).toEqual(["shell.full"]);
    expect(byFamily.get("shell.full")?.notes).toContain("universal Windows escape hatch");
  });

  it("keeps a compact stable routing hint separate from the runtime manifest", () => {
    const hint = agentActionRoutingHint();
    expect(hint).toContain("LOCAL_ACTION_TOOLS v1");
    expect(hint).toContain("families=fs.list fs.search");
    expect(hint).toContain("windows_reach=shell.full can invoke PowerShell");
    expect(hint).toContain("computer_use=Inspect GUI/UIA first");
    expect(hint).toContain("retry=If a tool fails");
    expect(hint).toContain("format=Emit AGENT_ACTION_JSON");
    expect(hint).toContain("loop_style=Write natural progress notes");
    expect(hint).toContain("must start its own line");
    expect(hint).not.toContain("workspace_root=");
    expect(hint).not.toContain("protected_roots=");
  });

  it("keeps filesystem operations inside the workspace", async () => {
    await withTempWorkspace(async (config) => {
      const rejected = await executeAgentActionRequest(config, {
        action: "create_directory",
        path: "../outside"
      });

      expect(rejected.accepted).toBe(false);
      expect(rejected.error?.message).toContain("outside the active workspace");
    });
  });

  it("blocks protected roots without automatic retry routes", async () => {
    await withTempWorkspace(async (config) => {
      const rejected = await executeAgentActionRequest(config, {
        action: "create_directory",
        scope: "computer",
        path: "C:\\Users\\quent\\Documents\\EVE\\MAP\\agent-test",
        confirmed: true
      });

      expect(rejected.accepted).toBe(false);
      expect(rejected.failureCategory).toBe("protected_root");
      expect(rejected.retryRoutes).toEqual([]);
      expect(rejected.error?.message).toContain("protected root");
    });
  });

  it("creates, renames, moves and deletes confirmed empty directories", async () => {
    await withTempWorkspace(async (config) => {
      const created = await executeAgentActionRequest(config, {
        action: "create_directory",
        path: "alpha"
      });
      expect(created.accepted).toBe(true);
      expect(created.value).toBe("chars +0 -0");
      expect(created.verification?.passed).toBe(true);
      expect(created.verification?.probes[0]?.id).toBe("directory.created");
      expect(created.observedChanges).toContain("filesystem:directory_created");

      const alreadyCreated = await executeAgentActionRequest(config, {
        action: "create_directory",
        path: "alpha"
      });
      expect(alreadyCreated.accepted).toBe(true);
      expect(alreadyCreated.value).toBe("directory already exists");

      const renamed = await executeAgentActionRequest(config, {
        action: "rename_path",
        path: "alpha",
        toPath: "beta"
      });
      expect(renamed.accepted).toBe(true);
      expect(renamed.toPath).toBe("beta");
      expect(renamed.value).toBe("chars +0 -0");
      expect(renamed.verification?.passed).toBe(true);
      expect(renamed.verification?.probes.map((probe) => probe.id)).toEqual(["move.destination_exists", "move.source_missing"]);

      await mkdir(join(config.workspaceRoot, "nested"));
      const moved = await executeAgentActionRequest(config, {
        action: "move_path",
        path: "beta",
        toPath: "nested/beta"
      });
      expect(moved.accepted).toBe(true);
      expect(moved.verification?.passed).toBe(true);

      const unconfirmedDelete = await executeAgentActionRequest(config, {
        action: "delete_empty_directory",
        path: "nested/beta"
      });
      expect(unconfirmedDelete.accepted).toBe(false);
      expect(unconfirmedDelete.error?.message).toContain("confirmed:true");

      const deleted = await executeAgentActionRequest(config, {
        action: "delete_empty_directory",
        path: "nested/beta",
        confirmed: true
      });
      expect(deleted.accepted).toBe(true);
      expect(deleted.value).toBe("chars +0 -0");
      expect(deleted.verification?.passed).toBe(true);
      expect(deleted.verification?.probes[0]?.expectation).toBe("missing");
    });
  });

  it("adds path metadata to transcript event commands", () => {
    expect(agentActionEventCommandForRequest({ action: "copy_path", path: "a.txt", toPath: "b.txt" })).toBe('/agent_copy_path_ path="a.txt" toPath="b.txt"');
  });

  it("copies and recursively deletes confirmed directory trees", async () => {
    await withTempWorkspace(async (config) => {
      await mkdir(join(config.workspaceRoot, "source", "deep"), { recursive: true });
      await writeFile(join(config.workspaceRoot, "source", "deep", "note.txt"), "copied", "utf8");

      const copied = await executeAgentActionRequest(config, {
        action: "copy_path",
        path: "source",
        toPath: "copy",
        recursive: true
      });
      expect(copied.accepted).toBe(true);
      expect(copied.verification?.passed).toBe(true);
      expect(copied.verification?.probes.map((probe) => probe.id)).toEqual(["copy.source_exists", "copy.destination_exists"]);
      await expect(readFile(join(config.workspaceRoot, "copy", "deep", "note.txt"), "utf8")).resolves.toBe("copied");

      const unconfirmedDelete = await executeAgentActionRequest(config, {
        action: "delete_tree",
        path: "copy",
        recursive: true
      });
      expect(unconfirmedDelete.accepted).toBe(false);
      expect(unconfirmedDelete.error?.message).toContain("confirmed:true");

      const deleted = await executeAgentActionRequest(config, {
        action: "delete_tree",
        path: "copy",
        recursive: true,
        confirmed: true
      });
      expect(deleted.accepted).toBe(true);
      expect(deleted.verification?.passed).toBe(true);
      expect(deleted.verification?.probes[0]?.id).toBe("tree.deleted");
      await expect(readFile(join(config.workspaceRoot, "copy", "deep", "note.txt"), "utf8")).rejects.toThrow();
    });
  });

  it("allows confirmed computer-scope writes outside the workspace with path guards", async () => {
    await withTempWorkspace(async (config) => {
      const outsideRoot = join(tmpdir(), `ingen-agent-action-host-outside-${Date.now()}-${Math.random().toString(36).slice(2)}`);
      try {
        const rejected = await executeAgentActionRequest(config, {
          action: "create_directory",
          scope: "computer",
          path: outsideRoot
        });
        expect(rejected.accepted).toBe(false);
        expect(rejected.error?.message).toContain("confirmed:true");

        const created = await executeAgentActionRequest(config, {
          action: "create_directory",
          scope: "computer",
          path: outsideRoot,
          confirmed: true
        });
        expect(created.accepted).toBe(true);
        expect(created.path).toBe(outsideRoot);
      } finally {
        await rm(outsideRoot, { recursive: true, force: true });
      }
    });
  });

  it("runs arbitrary commands only after explicit confirmation", async () => {
    await withTempWorkspace(async (config) => {
      const rejected = await executeAgentActionRequest(config, {
        action: "run_command",
        command: process.execPath,
        args: ["-e", "console.log('agent-host')"]
      });
      expect(rejected.accepted).toBe(false);
      expect(rejected.error?.message).toContain("confirmed:true");

      const accepted = await executeAgentActionRequest(config, {
        action: "run_command",
        command: process.execPath,
        args: ["-e", "console.log('agent-host')"],
        confirmed: true
      });
      expect(accepted.accepted).toBe(true);
      expect(accepted.stdoutPreview).toContain("agent-host");
      expect(accepted.executionAdapter).toBe("shell_full");
      expect(accepted.routeId).toBe("shell.full");
      expect(accepted.durationMs).toBeGreaterThanOrEqual(0);
      expect(accepted.timeoutMs).toBeGreaterThanOrEqual(100);
      expect(accepted.timedOut).toBe(false);
      expect(accepted.observedChanges).toContain("command_status:completed");
      expect(accepted.verification?.passed).toBe(true);
      expect(accepted.verification?.probes[0]?.kind).toBe("command_exit");
    });
  });

  windowsIt("inspects GUI state with bounded display and window summaries", async () => {
    await withTempWorkspace(async (config) => {
      const inspected = await executeAgentActionRequest(config, {
        action: "computer_inspect",
        maxResults: 10
      });

      expect(inspected.accepted).toBe(true);
      expect(inspected.computerUse?.schema).toBe("ingen.computer_use.snapshot.v1");
      expect(inspected.computerUse?.action).toBe("inspect");
      expect(inspected.computerUse?.displays.length).toBeGreaterThan(0);
      expect(inspected.computerUse?.accessibilityTreeStatus).toBe("planned");
      expect(inspected.computerUse?.ocrStatus).toBe("planned");
      expect(inspected.verification?.passed).toBe(true);
    });
  });

  windowsIt("inspects a bounded UI Automation tree without input", async () => {
    await withTempWorkspace(async (config) => {
      const inspected = await executeAgentActionRequest(config, {
        action: "computer_ui_tree",
        maxResults: 20
      });

      if (!inspected.accepted) {
        expect(["missing_tool", "timeout", "command_error", "unverifiable"]).toContain(inspected.failureCategory);
        expect(inspected.error?.message).toBeTruthy();
        return;
      }
      expect(inspected.accepted).toBe(true);
      expect(inspected.computerUse?.action).toBe("ui_tree");
      expect(inspected.computerUse?.accessibilityTreeStatus).toBe("available");
      expect(inspected.computerUse?.accessibilityTree?.length).toBeGreaterThan(0);
      expect(inspected.verification?.passed).toBe(true);
    });
  });

  it("requires confirmation for privacy-sensitive GUI and clipboard actions", async () => {
    await withTempWorkspace(async (config) => {
      const appshot = await executeAgentActionRequest(config, {
        action: "computer_appshot"
      });
      expect(appshot.accepted).toBe(false);
      expect(appshot.userPresenceRequired).toBe(true);
      expect(appshot.error?.message).toContain("confirmed:true");

      const focus = await executeAgentActionRequest(config, {
        action: "computer_focus_window",
        windowTitle: "InGen"
      });
      expect(focus.accepted).toBe(false);
      expect(focus.userPresenceRequired).toBe(true);

      const clipboard = await executeAgentActionRequest(config, {
        action: "computer_clipboard_write",
        text: "hello"
      });
      expect(clipboard.accepted).toBe(false);
      expect(clipboard.userPresenceRequired).toBe(true);

      const click = await executeAgentActionRequest(config, {
        action: "computer_click",
        x: 10,
        y: 10
      });
      expect(click.accepted).toBe(false);
      expect(click.userPresenceRequired).toBe(true);

      const typed = await executeAgentActionRequest(config, {
        action: "computer_type_text",
        text: "hello"
      });
      expect(typed.accepted).toBe(false);
      expect(typed.userPresenceRequired).toBe(true);

      const ocr = await executeAgentActionRequest(config, {
        action: "computer_ocr"
      });
      expect(ocr.accepted).toBe(false);
      expect(ocr.userPresenceRequired).toBe(true);
    });
  });

  it("inspects web state and verifies confirmed download artifacts", async () => {
    await withTempWorkspace(async (config) => {
      const html = "<!doctype html><title>Agent Web Fixture</title><a href=\"/file.txt\" download>Download</a><form></form>";
      const url = `data:text/html,${encodeURIComponent(html)}`;

      const inspected = await executeAgentActionRequest(config, {
        action: "browser_inspect_url",
        url
      });
      expect(inspected.accepted).toBe(true);
      expect(inspected.browserPage?.schema).toBe("ingen.browser.page_summary.v1");
      expect(inspected.browserPage?.title).toBe("Agent Web Fixture");
      expect(inspected.browserPage?.linkCount).toBe(1);
      expect(inspected.browserPage?.formCount).toBe(1);
      expect(inspected.browserPage?.domStatus).toBe("available");
      expect(inspected.verification?.passed).toBe(true);

      const playwrightInspected = await executeAgentActionRequest(config, {
        action: "browser_playwright_inspect",
        url
      });
      if (playwrightInspected.accepted) {
        expect(playwrightInspected.browserPage?.domStatus).toBe("available");
        expect(playwrightInspected.browserPage?.networkLogStatus).toBe("available");
        expect(playwrightInspected.browserPage?.domNodeCount).toBeGreaterThan(0);
        expect(playwrightInspected.verification?.passed).toBe(true);
      } else {
        expect(["missing_tool", "timeout", "command_error"]).toContain(playwrightInspected.failureCategory);
        expect(playwrightInspected.error?.message).toBeTruthy();
      }

      const unconfirmed = await executeAgentActionRequest(config, {
        action: "browser_download",
        url,
        path: "downloads/page.html"
      });
      expect(unconfirmed.accepted).toBe(false);
      expect(unconfirmed.userPresenceRequired).toBe(true);
      expect(unconfirmed.error?.message).toContain("confirmed:true");

      const downloaded = await executeAgentActionRequest(config, {
        action: "browser_download",
        url,
        path: "downloads/page.html",
        confirmed: true
      });
      expect(downloaded.accepted).toBe(true);
      expect(downloaded.download?.schema).toBe("ingen.browser.download_artifact.v1");
      expect(downloaded.download?.path).toBe("downloads\\page.html");
      expect(downloaded.download?.sha256).toMatch(/^[a-f0-9]{64}$/);
      expect(downloaded.verification?.passed).toBe(true);
      await expect(readFile(join(config.workspaceRoot, "downloads", "page.html"), "utf8")).resolves.toBe(html);

      for (const request of [
        { action: "browser_screenshot" as const, url, path: "shots/page.png" },
        { action: "browser_click" as const, url, selector: "button" },
        { action: "browser_type_text" as const, url, selector: "input", text: "hello" },
        { action: "browser_playwright_download" as const, url, selector: "a[download]", path: "downloads/playwright.txt" }
      ]) {
        const blocked = await executeAgentActionRequest(config, request);
        expect(blocked.accepted).toBe(false);
        expect(blocked.userPresenceRequired).toBe(true);
        expect(blocked.error?.message).toContain("confirmed:true");
      }
    });
  }, 20_000);

  it("writes, inspects and converts document/data artifacts with readback verification", async () => {
    await withTempWorkspace(async (config) => {
      const writtenJson = await executeAgentActionRequest(config, {
        action: "document_write_json",
        path: "data/report.json",
        content: "{\"answer\":42,\"items\":[\"a\",\"b\"]}"
      });
      expect(writtenJson.accepted).toBe(true);
      expect(writtenJson.documentMedia?.schema).toBe("ingen.document_media.summary.v1");
      expect(writtenJson.documentMedia?.kind).toBe("json");
      expect(writtenJson.documentMedia?.jsonValid).toBe(true);
      expect(writtenJson.documentMedia?.sha256).toMatch(/^[a-f0-9]{64}$/);
      expect(writtenJson.verification?.passed).toBe(true);
      expect(writtenJson.audit?.schema).toBe("ingen.agent_runtime_audit.summary.v1");
      expect(writtenJson.audit?.logSha256).toMatch(/^[a-f0-9]{64}$/);
      const auditLog = await readFile(join(config.workspaceRoot, ".ingen-agent-artifacts", "agent-action-runtime.jsonl"), "utf8");
      expect(auditLog).toContain('"kind":"started"');
      expect(auditLog).toContain('"kind":"result"');
      expect(auditLog).toContain('"kind":"verification"');
      expect(auditLog).toContain('"kind":"summary"');
      await expect(readFile(join(config.workspaceRoot, "data", "report.json"), "utf8")).resolves.toContain("\"answer\": 42");

      const writtenCsv = await executeAgentActionRequest(config, {
        action: "document_write_csv",
        path: "data/table.csv",
        content: "name,value\r\nalpha,1\r\nbeta,2"
      });
      expect(writtenCsv.accepted).toBe(true);
      expect(writtenCsv.documentMedia?.kind).toBe("csv");
      expect(writtenCsv.documentMedia?.csvRows).toBe(3);
      expect(writtenCsv.documentMedia?.csvColumns).toBe(2);

      const writtenMarkdown = await executeAgentActionRequest(config, {
        action: "document_write_text",
        path: "notes/source.md",
        content: "# Title\n\nA [link](https://example.com) and **bold** text.\n"
      });
      expect(writtenMarkdown.accepted).toBe(true);
      expect(writtenMarkdown.documentMedia?.kind).toBe("markdown");
      expect(writtenMarkdown.documentMedia?.markdownHeadingCount).toBe(1);

      const inspected = await executeAgentActionRequest(config, {
        action: "document_inspect",
        path: "notes/source.md"
      });
      expect(inspected.accepted).toBe(true);
      expect(inspected.documentMedia?.parserStatus).toBe("available");
      expect(inspected.documentMedia?.conversionStatus).toBe("available");

      const converted = await executeAgentActionRequest(config, {
        action: "document_convert_text",
        path: "notes/source.md",
        toPath: "notes/source.txt"
      });
      expect(converted.accepted).toBe(true);
      expect(converted.toPath).toBe("notes\\source.txt");
      expect(converted.documentMedia?.kind).toBe("text");
      await expect(readFile(join(config.workspaceRoot, "notes", "source.txt"), "utf8")).resolves.toContain("Title");
    });
  });

  it("executes document/media backends only with runtime proof or a clean block", async () => {
    await withTempWorkspace(async (config) => {
      const pdfObjects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        "<< /Length 44 >>\nstream\nBT /F1 24 Tf 50 100 Td (Hello PDF) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
      ];
      let pdf = "%PDF-1.4\n";
      const offsets: number[] = [];
      for (let index = 0; index < pdfObjects.length; index += 1) {
        offsets.push(Buffer.byteLength(pdf, "utf8"));
        pdf += `${index + 1} 0 obj\n${pdfObjects[index]}\nendobj\n`;
      }
      const xrefOffset = Buffer.byteLength(pdf, "utf8");
      pdf += `xref\n0 ${pdfObjects.length + 1}\n0000000000 65535 f \n`;
      pdf += offsets.map((offset) => `${offset.toString().padStart(10, "0")} 00000 n \n`).join("");
      pdf += `trailer\n<< /Root 1 0 R /Size ${pdfObjects.length + 1} >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
      await writeFile(join(config.workspaceRoot, "sample.pdf"), pdf, "utf8");

      const extracted = await executeAgentActionRequest(config, {
        action: "document_pdf_extract_text",
        path: "sample.pdf",
        toPath: "sample.txt"
      });
      if (extracted.accepted) {
        expect(extracted.documentMedia?.pageCount).toBe(1);
        expect(extracted.stdoutPreview).toContain("Hello PDF");
        expect(extracted.toPath).toBe("sample.txt");
        expect(extracted.verification?.passed).toBe(true);
      } else {
        expect(extracted.verification?.passed).toBe(false);
        expect(extracted.error?.message).toBeTruthy();
      }

      await writeFile(join(config.workspaceRoot, "scan.png"), Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/l6s9TAAAAABJRU5ErkJggg==", "base64"));
      const imageOcrBlocked = await executeAgentActionRequest(config, {
        action: "document_image_ocr",
        path: "scan.png"
      });
      expect(imageOcrBlocked.accepted).toBe(false);
      expect(imageOcrBlocked.userPresenceRequired).toBe(true);

      const imageOcr = await executeAgentActionRequest(config, {
        action: "document_image_ocr",
        path: "scan.png",
        confirmed: true
      });
      if (imageOcr.accepted) {
        expect(imageOcr.commandLine).toContain("tesseract");
        expect(imageOcr.verification?.passed).toBe(true);
        expect(imageOcr.documentMedia?.ocrTextChars).toBeGreaterThanOrEqual(0);
      } else {
        expect(imageOcr.verification?.passed).toBe(false);
        expect(imageOcr.error?.message).toBeTruthy();
      }

      const officeBlocked = await executeAgentActionRequest(config, {
        action: "document_office_inspect",
        path: "sample.docx"
      });
      expect(officeBlocked.accepted).toBe(false);
      expect(officeBlocked.userPresenceRequired).toBe(true);
      expect(officeBlocked.error?.message).toContain("confirmed:true");

      const macroBlocked = await executeAgentActionRequest(config, {
        action: "document_office_inspect",
        path: "sample.docx",
        confirmed: true,
        macroExecutionConfirmed: true
      });
      expect(macroBlocked.accepted).toBe(false);
      expect(macroBlocked.error?.message).toContain("Macro execution is blocked");

      const metadata = await executeAgentActionRequest(config, {
        action: "document_media_metadata",
        path: "scan.png"
      });
      if (metadata.accepted) {
        expect(metadata.commandLine).toContain("ffprobe");
        expect(metadata.verification?.passed).toBe(true);
        expect(metadata.documentMedia?.mediaStreams).toBeGreaterThanOrEqual(0);
      } else {
        expect(metadata.verification?.passed).toBe(false);
        expect(metadata.error?.message).toBeTruthy();
      }

      const toolchain = await executeAgentActionRequest(config, {
        action: "document_toolchain_inspect",
        query: "all"
      });
      expect(toolchain.accepted).toBe(true);
      expect(toolchain.documentToolchain?.schema).toBe("ingen.document_toolchain.summary.v1");
      expect(toolchain.documentToolchain?.tools.map((tool) => tool.id)).toEqual([
        "tesseract",
        "ffprobe",
        "office_word",
        "office_excel",
        "office_powerpoint"
      ]);
      expect(toolchain.verification?.passed).toBe(true);

      const toolchainInstallBlocked = await executeAgentActionRequest(config, {
        action: "document_toolchain_install",
        query: "ocr"
      });
      expect(toolchainInstallBlocked.accepted).toBe(false);
      expect(toolchainInstallBlocked.userPresenceRequired).toBe(true);
      expect(toolchainInstallBlocked.error?.message).toContain("confirmed:true");

      const officeInstallBlocked = await executeAgentActionRequest(config, {
        action: "document_toolchain_install",
        query: "office",
        confirmed: true
      });
      expect(officeInstallBlocked.accepted).toBe(false);
      expect(officeInstallBlocked.documentToolchain?.target).toBe("office");
      expect(officeInstallBlocked.error?.message).toContain("Office install/update is not automated");
    });
  }, 20_000);

  gitIt("inspects Git state, runs confirmed checks and records automation goals", async () => {
    await withTempWorkspace(async (config) => {
      spawnSync("git", ["init"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      spawnSync("git", ["config", "user.email", "agent@example.test"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      spawnSync("git", ["config", "user.name", "Agent Test"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      await writeFile(join(config.workspaceRoot, "tracked.txt"), "one\n", "utf8");
      spawnSync("git", ["add", "tracked.txt"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      spawnSync("git", ["commit", "-m", "Initial"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      await writeFile(join(config.workspaceRoot, "tracked.txt"), "one\ntwo\n", "utf8");
      await writeFile(join(config.workspaceRoot, "untracked.txt"), "new\n", "utf8");

      const status = await executeAgentActionRequest(config, {
        action: "dev_repo_status"
      });
      expect(status.accepted).toBe(true);
      expect(status.developer?.schema).toBe("ingen.developer.repo_summary.v1");
      expect(status.developer?.changedFiles).toBeGreaterThanOrEqual(2);
      expect(status.developer?.unstagedFiles).toBeGreaterThanOrEqual(1);
      expect(status.developer?.untrackedFiles).toBeGreaterThanOrEqual(1);
      expect(status.verification?.passed).toBe(true);

      const diff = await executeAgentActionRequest(config, {
        action: "dev_git_diff"
      });
      expect(diff.accepted).toBe(true);
      expect(diff.developer?.diffStat).toContain("tracked.txt");

      const rejectedCommit = await executeAgentActionRequest(config, {
        action: "dev_git_commit",
        title: "Update tracked fixture",
        paths: ["tracked.txt"]
      });
      expect(rejectedCommit.accepted).toBe(false);
      expect(rejectedCommit.error?.message).toContain("confirmed:true");

      const commit = await executeAgentActionRequest(config, {
        action: "dev_git_commit",
        title: "Update tracked fixture",
        paths: ["tracked.txt"],
        confirmed: true
      });
      expect(commit.accepted).toBe(true);
      expect(commit.routeId).toBe("dev.git_commit");
      expect(commit.developer?.action).toBe("git_commit");
      expect(commit.developer?.commitHash).toMatch(/^[a-f0-9]{40}$/);
      expect(commit.verification?.passed).toBe(true);

      const remotePath = join(config.workspaceRoot, ".git", "agent-action-remote.git");
      spawnSync("git", ["init", "--bare", remotePath], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      spawnSync("git", ["remote", "add", "origin", remotePath], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
      const push = await executeAgentActionRequest(config, {
        action: "dev_git_push",
        remote: "origin",
        confirmed: true
      });
      expect(push.accepted).toBe(true);
      expect(push.routeId).toBe("dev.git_push");
      expect(push.developer?.action).toBe("git_push");
      expect(push.developer?.remote).toBe("origin");
      expect(push.verification?.passed).toBe(true);

      const rejectedPr = await executeAgentActionRequest(config, {
        action: "dev_github_pr_create",
        title: "Test PR"
      });
      expect(rejectedPr.accepted).toBe(false);
      expect(rejectedPr.error?.message).toContain("confirmed:true");

      const missingPrTitle = await executeAgentActionRequest(config, {
        action: "dev_github_pr_create",
        confirmed: true
      });
      expect(missingPrTitle.accepted).toBe(false);
      expect(missingPrTitle.error?.message).toContain("title is required");

      const rejectedCheck = await executeAgentActionRequest(config, {
        action: "dev_run_check",
        command: process.execPath,
        args: ["-e", "console.log('check')"]
      });
      expect(rejectedCheck.accepted).toBe(false);
      expect(rejectedCheck.error?.message).toContain("confirmed:true");

      const check = await executeAgentActionRequest(config, {
        action: "dev_run_check",
        command: process.execPath,
        args: ["-e", "console.log('check-ok')"],
        confirmed: true
      });
      expect(check.accepted).toBe(true);
      expect(check.routeId).toBe("dev.run_check");
      expect(check.stdoutPreview).toContain("check-ok");
      expect(check.developer?.action).toBe("run_check");

      const rejectedAutomation = await executeAgentActionRequest(config, {
        action: "automation_record",
        title: "Check the build every morning"
      });
      expect(rejectedAutomation.accepted).toBe(false);

      const automation = await executeAgentActionRequest(config, {
        action: "automation_record",
        title: "Check the build every morning",
        confirmed: true
      });
      expect(automation.accepted).toBe(true);
      expect(automation.automation?.schema).toBe("ingen.automation.ledger_entry.v1");
      expect(automation.automation?.status).toBe("recorded");
      expect(automation.automation?.proofHash).toMatch(/^[a-f0-9]{64}$/);
      await expect(readFile(join(config.workspaceRoot, ".ingen-agent-artifacts", "automation-ledger.jsonl"), "utf8")).resolves.toContain("Check the build every morning");
    });
  }, 15_000);

  it("inspects cloud CLIs and blocks unsafe cloud commands before execution", async () => {
    await withTempWorkspace(async (config) => {
      const inspected = await executeAgentActionRequest(config, {
        action: "cloud_cli_inspect",
        cloudProvider: "all"
      });
      expect(inspected.accepted).toBe(true);
      expect(inspected.routeId).toBe("cloud.inspect");
      expect(inspected.cloud?.schema).toBe("ingen.cloud_cli.summary.v1");
      expect(inspected.cloud?.resources.length).toBeGreaterThanOrEqual(5);
      expect(inspected.cloud?.redactionStatus).toBe("credentials_redacted");
      expect(inspected.verification?.passed).toBe(true);

      const secretRead = await executeAgentActionRequest(config, {
        action: "cloud_cli_run_readonly",
        cloudProvider: "aws",
        args: ["configure", "get", "aws_secret_access_key"]
      });
      expect(secretRead.accepted).toBe(false);
      expect(secretRead.cloud?.mutationPolicy).toBe("blocked_dangerous");
      expect(secretRead.error?.message).toContain("credentials");

      const destructiveRead = await executeAgentActionRequest(config, {
        action: "cloud_cli_run_readonly",
        cloudProvider: "gcp",
        args: ["compute", "instances", "delete", "vm-1"]
      });
      expect(destructiveRead.accepted).toBe(false);
      expect(destructiveRead.cloud?.mutationPolicy).toBe("blocked_dangerous");

      const unconfirmedWrite = await executeAgentActionRequest(config, {
        action: "cloud_cli_run_write",
        cloudProvider: "github",
        args: ["repo", "edit", "--description", "demo"]
      });
      expect(unconfirmedWrite.accepted).toBe(false);
      expect(unconfirmedWrite.userPresenceRequired).toBe(true);
      expect(unconfirmedWrite.error?.message).toContain("confirmed:true");
    });
  });

  it("runs typed Windows admin adapters and blocks sensitive mutations without confirmation", async () => {
    await withTempWorkspace(async (config) => {
      const settingInspect = await executeAgentActionRequest(config, {
        action: "windows_setting_inspect",
        settingName: "os"
      });
      expect(settingInspect.accepted).toBe(true);
      expect(settingInspect.routeId).toBe("windows.setting_inspect");
      expect(settingInspect.windowsAdmin?.schema).toBe("ingen.windows_admin.summary.v1");
      expect(settingInspect.windowsAdmin?.mutationPolicy).toBe("readonly");

      const settingApply = await executeAgentActionRequest(config, {
        action: "windows_setting_apply",
        path: "HKCU:\\Software\\InGenTest",
        settingName: "Value",
        content: "1"
      });
      expect(settingApply.accepted).toBe(false);
      expect(settingApply.userPresenceRequired).toBe(true);

      const processInspect = await executeAgentActionRequest(config, {
        action: "process_service_inspect"
      });
      expect(processInspect.accepted).toBe(true);
      expect(processInspect.windowsAdmin?.surface).toBe("process_service");

      const serviceControl = await executeAgentActionRequest(config, {
        action: "process_service_control",
        serviceName: "Spooler",
        command: "restart"
      });
      expect(serviceControl.accepted).toBe(false);
      expect(serviceControl.error?.message).toContain("confirmed:true");

      const sensitiveInspect = await executeAgentActionRequest(config, {
        action: "windows_sensitive_inspect",
        settingName: "user_env",
        query: "PATH"
      });
      expect(sensitiveInspect.accepted).toBe(true);
      expect(sensitiveInspect.windowsAdmin?.surface).toBe("sensitive_system");
      expect(sensitiveInspect.windowsAdmin?.mutationPolicy).toBe("readonly");

      const sensitiveApply = await executeAgentActionRequest(config, {
        action: "windows_sensitive_apply",
        settingName: "user_env",
        query: "INGEN_TEST",
        content: "1"
      });
      expect(sensitiveApply.accepted).toBe(false);
      expect(sensitiveApply.userPresenceRequired).toBe(true);

      const defenderBlocked = await executeAgentActionRequest(config, {
        action: "windows_sensitive_apply",
        settingName: "defender",
        content: "disable",
        confirmed: true
      });
      expect(defenderBlocked.accepted).toBe(false);
      expect(defenderBlocked.windowsAdmin?.mutationPolicy).toBe("blocked_dangerous");

      const packageInspect = await executeAgentActionRequest(config, {
        action: "package_inspect"
      });
      expect(packageInspect.accepted).toBe(true);
      expect(packageInspect.windowsAdmin?.surface).toBe("package");
      expect(packageInspect.verification?.passed).toBe(true);

      const packageInstall = await executeAgentActionRequest(config, {
        action: "package_install_update",
        packageId: "Git.Git",
        command: "upgrade"
      });
      expect(packageInstall.accepted).toBe(false);
      expect(packageInstall.userPresenceRequired).toBe(true);

      const ci = await executeAgentActionRequest(config, {
        action: "ci_checks_inspect",
        maxResults: 5
      });
      if (ci.accepted) {
        expect(ci.windowsAdmin?.surface).toBe("ci_review");
        expect(ci.verification?.passed).toBe(true);
      } else {
        expect(ci.error?.message).toBeTruthy();
      }

      const rerunBlocked = await executeAgentActionRequest(config, {
        action: "ci_rerun_failed",
        query: "123"
      });
      expect(rerunBlocked.accepted).toBe(false);
      expect(rerunBlocked.userPresenceRequired).toBe(true);

      const reviewBlocked = await executeAgentActionRequest(config, {
        action: "dev_github_pr_review_submit",
        query: "1",
        command: "comment",
        content: "Looks good."
      });
      expect(reviewBlocked.accepted).toBe(false);
      expect(reviewBlocked.userPresenceRequired).toBe(true);

      const hypervRejected = await executeAgentActionRequest(config, {
        action: "virtualization_run_command",
        provider: "hyperv",
        command: "hostname",
        confirmed: true
      });
      expect(hypervRejected.accepted).toBe(false);
      expect(hypervRejected.error?.message).toContain("vmName is required");
    });
  }, 20_000);

  schedulerIt("creates, lists and cancels a real InGen-owned scheduled task", async () => {
    await withTempWorkspace(async (config) => {
      const taskName = `InGenAgent_AgentActionHost-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      try {
        const rejected = await executeAgentActionRequest(config, {
          action: "automation_schedule",
          title: "Scheduler test",
          command: "cmd.exe",
          args: ["/d", "/s", "/c", "echo ingen-scheduler-test"],
          taskName,
          scheduleType: "ONLOGON"
        });
        expect(rejected.accepted).toBe(false);
        expect(rejected.error?.message).toContain("confirmed:true");

        const scheduled = await executeAgentActionRequest(config, {
          action: "automation_schedule",
          title: "Scheduler test",
          command: "cmd.exe",
          args: ["/d", "/s", "/c", "echo ingen-scheduler-test"],
          taskName,
          scheduleType: "ONLOGON",
          confirmed: true
        });
        if (!scheduled.accepted) {
          expect(["permission", "missing_tool"]).toContain(scheduled.failureCategory);
          expect(scheduled.routeId).toBe("automation.schedule.create");
          expect(scheduled.verification?.passed).toBe(false);
          return;
        }
        expect(scheduled.accepted).toBe(true);
        expect(scheduled.routeId).toBe("automation.schedule");
        expect(scheduled.automation?.status).toBe("scheduled");
        expect(scheduled.automation?.backend).toBe("windows_task_scheduler");
        expect(scheduled.verification?.passed).toBe(true);

        const listed = await executeAgentActionRequest(config, {
          action: "automation_list",
          maxResults: 50
        });
        expect(listed.accepted).toBe(true);
        expect(listed.routeId).toBe("automation.list");
        expect(listed.stdoutPreview).toContain("AgentActionHost");

        const cancelled = await executeAgentActionRequest(config, {
          action: "automation_cancel",
          taskName,
          confirmed: true
        });
        expect(cancelled.accepted).toBe(true);
        expect(cancelled.routeId).toBe("automation.cancel");
        expect(cancelled.automation?.status).toBe("cancelled");
        expect(cancelled.verification?.passed).toBe(true);
        await expect(readFile(join(config.workspaceRoot, ".ingen-agent-artifacts", "automation-ledger.jsonl"), "utf8")).resolves.toContain(
          "windows_task_scheduler"
        );
      } finally {
        await executeAgentActionRequest(config, {
          action: "automation_cancel",
          taskName,
          confirmed: true
        });
      }
    });
  });

  it("inspects virtualization backends and verifies confirmed native fallback execution", async () => {
    await withTempWorkspace(async (config) => {
      const inspected = await executeAgentActionRequest(config, {
        action: "virtualization_inspect",
        provider: "all"
      });
      expect(inspected.accepted).toBe(true);
      expect(inspected.routeId).toBe("virtualization.inspect");
      expect(inspected.virtualization?.schema).toBe("ingen.virtualization.summary.v1");
      expect(inspected.virtualization?.resources.length).toBeGreaterThanOrEqual(3);
      expect(inspected.verification?.passed).toBe(true);

      const rejected = await executeAgentActionRequest(config, {
        action: "virtualization_run_command",
        provider: "docker",
        container: "ingen-missing-container-for-fallback",
        command: process.execPath,
        args: ["-e", "console.log('fallback-ok')"],
        nativeFallback: true
      });
      expect(rejected.accepted).toBe(false);
      expect(rejected.failureCategory).toBe("denied");

      const executed = await executeAgentActionRequest(config, {
        action: "virtualization_run_command",
        provider: "docker",
        container: "ingen-missing-container-for-fallback",
        command: process.execPath,
        args: ["-e", "console.log('fallback-ok')"],
        nativeFallback: true,
        confirmed: true
      });
      expect(executed.accepted).toBe(true);
      expect(executed.routeId).toBe("virtualization.native_fallback");
      expect(executed.exitCode).toBe(0);
      expect(executed.stdoutPreview).toContain("fallback-ok");
      expect(executed.verification?.passed).toBe(true);
    });
  }, 20_000);

  it("reports command timeout and structured execution metadata", async () => {
    await withTempWorkspace(async (config) => {
      const timedOut = await executeAgentActionRequest(config, {
        action: "run_command",
        command: process.execPath,
        args: ["-e", "setTimeout(() => {}, 2000)"],
        confirmed: true,
        timeoutMs: 100
      });

      expect(timedOut.accepted).toBe(false);
      expect(timedOut.executionAdapter).toBe("shell_full");
      expect(timedOut.routeId).toBe("shell.full");
      expect(timedOut.timeoutMs).toBe(100);
      expect(timedOut.timedOut).toBe(true);
      expect(timedOut.failureCategory).toBe("timeout");
      expect(timedOut.retryRoutes).toContain("powershell");
      expect(timedOut.retryRoutes).toContain("cmd");
      expect(timedOut.error?.message).toContain("timed out");
      expect(timedOut.observedChanges).toContain("timed_out:true");
      expect(timedOut.verification?.passed).toBe(false);
    });
  });

  windowsIt("classifies PowerShell, CMD and native Windows command adapters", async () => {
    await withTempWorkspace(async (config) => {
      const powershell = await executeAgentActionRequest(config, {
        action: "run_command",
        command: "powershell.exe",
        args: ["-NoProfile", "-Command", "Write-Output agent-powershell"],
        confirmed: true
      });
      expect(powershell.executionAdapter).toBe("powershell");
      expect(powershell.routeId).toBe("powershell.inline");
      expect(powershell.stdoutPreview).toContain("agent-powershell");

      const cmd = await executeAgentActionRequest(config, {
        action: "run_command",
        command: "cmd.exe",
        args: ["/d", "/s", "/c", "echo agent-cmd"],
        confirmed: true
      });
      expect(cmd.executionAdapter).toBe("cmd");
      expect(cmd.routeId).toBe("cmd.inline");
      expect(cmd.stdoutPreview).toContain("agent-cmd");

      const tasklist = await executeAgentActionRequest(config, {
        action: "run_command",
        command: "tasklist.exe",
        args: ["/?"],
        confirmed: true
      });
      expect(tasklist.executionAdapter).toBe("windows_command");
      expect(tasklist.routeId).toBe("processes.tasklist");
      expect(tasklist.commandLine).toContain("tasklist.exe");
    });
  }, 20_000);

  it("searches workspace content with bounded matches", async () => {
    await withTempWorkspace(async (config) => {
      await writeFile(join(config.workspaceRoot, "note.txt"), "alpha\nneedle here\nomega\n", "utf8");

      const search = await executeAgentActionRequest(config, {
        action: "search",
        query: "needle",
        path: ".",
        maxResults: 5
      });

      expect(search.accepted).toBe(true);
      expect(search.matches?.[0]?.path).toBe("note.txt");
      expect(search.matches?.[0]?.line).toBe(2);
    });
  });
});
