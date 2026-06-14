import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  agentActionCapabilityDetailManifest,
  createAgentCapabilityAtlas,
  createAgentActionHostManifest,
  createAgentActionRuntimeManifestSummary,
  createComputerUsePolicy,
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
      "computer.clipboard"
    ]);
    expect(summary.availableFamilies).toContain("shell.full");
    expect(summary.plannedFamilies).toContain("windows.settings");
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
    expect(policy.interactionRequiresConfirmation).toBe(true);
    expect(policy.userPresenceMode).toBe("foreground_required_for_risky_gui_actions");
    expect(policy.pacingPolicy).toBe("single_action_then_verify");
    expect(policy.forbiddenPrompts).toContain("credential");
    expect(policy.forbiddenPrompts).toContain("uac");
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
      "automation.rpa"
    ]) {
      expect(byFamily.has(family)).toBe(true);
    }

    expect(atlas.length).toBeGreaterThanOrEqual(15);
    expect(byFamily.get("windows.credentials")?.status).toBe("blocked");
    expect(byFamily.get("windows.credentials")?.approval).toBe("blocked");
    expect(byFamily.get("windows.scheduler")?.approval).toBe("prompt");
    expect(byFamily.get("windows.settings")?.approval).toBe("prompt");
    expect(byFamily.get("browser.cdp")?.status).toBe("planned");
    expect(byFamily.get("virtualization.wsl")?.status).toBe("planned");
    expect(byFamily.get("office.com")?.executableActionIds).toBeUndefined();
    expect(byFamily.get("shell.full")?.executableActionIds).toEqual(["shell.full"]);
    expect(byFamily.get("shell.full")?.notes).toContain("universal Windows escape hatch");
  });

  it("keeps a compact stable routing hint separate from the runtime manifest", () => {
    const hint = agentActionRoutingHint();
    expect(hint).toContain("LOCAL_ACTION_TOOLS v1");
    expect(hint).toContain("families=fs.list fs.search");
    expect(hint).toContain("windows_reach=shell.full can invoke PowerShell");
    expect(hint).toContain("computer_use=Inspect GUI first");
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
    });
  });

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
  });

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
