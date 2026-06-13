import { mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  createAgentActionHostManifest,
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
    expect(manifest.permissions.sandbox).toBe("workspace");
    expect(manifest.permissions.recursiveDelete).toBe("blocked");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("fs.search");
    expect(manifest.capabilities.map((capability) => capability.id)).toContain("shell.readonly");
    expect(manifest.proofHash).toMatch(/^[a-f0-9]{64}$/);
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

  it("creates, renames, moves and deletes only confirmed empty directories", async () => {
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
