import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  createAgentActionHostManifest,
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
    expect(promptManifest).toContain("loop_stream=When local action is needed");
    expect(promptManifest).toContain("action_request_format=AGENT_ACTION_JSON");
    expect(promptManifest).toContain("tool_truth=Never claim an action was executed");
  });

  it("keeps a compact stable routing hint separate from the runtime manifest", () => {
    const hint = agentActionRoutingHint();
    expect(hint).toContain("LOCAL_ACTION_TOOLS v1");
    expect(hint).toContain("families=fs.list fs.search");
    expect(hint).toContain("format=Emit AGENT_ACTION_JSON");
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

      const renamed = await executeAgentActionRequest(config, {
        action: "rename_path",
        path: "alpha",
        toPath: "beta"
      });
      expect(renamed.accepted).toBe(true);
      expect(renamed.toPath).toBe("beta");

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
    });
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
