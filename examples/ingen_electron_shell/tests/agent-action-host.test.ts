import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  createAgentCapabilityAtlas,
  createAgentActionHostManifest,
  createAgentActionRuntimeManifestSummary,
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
    expect(manifest.runtime.injectionPolicy).toBe("full_on_local_intent_compact_delta_on_continuation");
    expect(manifest.runtime.resultReinjectionPolicy).toBe("compact_tool_result_is_ground_truth_each_round");
    expect(manifest.runtime.executableActionIds).toContain("shell.full");
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
    expect(promptManifest).toContain("injection_policy=full_on_local_intent_compact_delta_on_continuation");
    expect(promptManifest).toContain("prompt_budget=compact_by_default_detail_on_selected_capability");
    expect(promptManifest).toContain("result_reinjection=compact_tool_result_is_ground_truth_each_round");
    expect(promptManifest).toContain("windows.wmi:planned/none");
    expect(promptManifest).toContain("office.com:planned/prompt");
    expect(promptManifest).toContain("capability_policy=Use the atlas for reasoning");
    expect(promptManifest).toContain("Prefer structured app/API/CLI routes first, then confirmed shell.full, then GUI/computer-use");
    expect(promptManifest).toContain("capability_limits=Planned or blocked atlas entries are not direct AGENT_ACTION_JSON actions");
    expect(promptManifest).toContain("windows_reach=shell.full can use PowerShell/cmd");
    expect(promptManifest).toContain("retry=If AGENT_ACTION_RESULT reports failure");
    expect(promptManifest).toContain("loop_stream=When local action is needed");
    expect(promptManifest).toContain("loop_style=Use varied, concrete progress notes");
    expect(promptManifest).toContain("starts with AGENT_ACTION_JSON at column 1");
    expect(promptManifest).toContain("action_request_format=AGENT_ACTION_JSON");
    expect(promptManifest).toContain("tool_truth=Never claim an action was executed");
  });

  it("publishes a compact runtime manifest summary for delta prompt injection", () => {
    const summary = createAgentActionRuntimeManifestSummary({
      workspaceRoot: "C:\\repo",
      workspaceActive: true,
      cwd: "C:\\repo",
      platform: "win32"
    });

    expect(summary.manifestHash).toMatch(/^[a-f0-9]{64}$/);
    expect(summary.atlasHash).toMatch(/^[a-f0-9]{64}$/);
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
      "shell.full"
    ]);
    expect(summary.availableFamilies).toContain("shell.full");
    expect(summary.plannedFamilies).toContain("windows.settings");
    expect(summary.blockedFamilies).toContain("windows.credentials");
    expect(summary.approvalGatedFamilies).toContain("browser.cdp");
    expect(summary.promptBudget).toBe("compact_by_default_detail_on_selected_capability");
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

  it("creates, renames, moves and deletes confirmed empty directories", async () => {
    await withTempWorkspace(async (config) => {
      const created = await executeAgentActionRequest(config, {
        action: "create_directory",
        path: "alpha"
      });
      expect(created.accepted).toBe(true);
      expect(created.value).toBe("chars +0 -0");

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

      await mkdir(join(config.workspaceRoot, "nested"));
      const moved = await executeAgentActionRequest(config, {
        action: "move_path",
        path: "beta",
        toPath: "nested/beta"
      });
      expect(moved.accepted).toBe(true);

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
