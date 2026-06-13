import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rename, rm, rmdir, stat } from "node:fs/promises";
import { isAbsolute, relative, resolve } from "node:path";
import type {
  AgentActionCapability,
  AgentActionHostManifest,
  AgentActionPathEntry,
  AgentActionRequest,
  AgentActionResult,
  AgentActionSearchMatch,
  IpcError
} from "../shared/ipc-contract.js";

const PROTECTED_ROOTS = ["C:\\Users\\quent\\Documents\\EVE\\MAP"];
const MAX_PREVIEW_CHARS = 24_000;

export interface AgentActionHostConfig {
  workspaceRoot: string;
  workspaceActive: boolean;
  cwd: string;
  platform: NodeJS.Platform;
}

function hashJson(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

function actionError(code: IpcError["code"], message: string, input: unknown): IpcError {
  return {
    code,
    message,
    proofHash: hashJson({ code, message, input })
  };
}

function result(
  config: AgentActionHostConfig,
  request: AgentActionRequest,
  patch: Omit<Partial<AgentActionResult>, "schema" | "action" | "cwd" | "proofHash">
): AgentActionResult {
  const envelope: AgentActionResult = {
    schema: "ingen.agent_action_host.result.v1",
    accepted: patch.accepted ?? false,
    action: request.action,
    cwd: config.cwd,
    path: patch.path,
    toPath: patch.toPath,
    items: patch.items,
    matches: patch.matches,
    commandLine: patch.commandLine,
    exitCode: patch.exitCode,
    stdoutPreview: patch.stdoutPreview,
    stderrPreview: patch.stderrPreview,
    value: patch.value,
    proofHash: "",
    error: patch.error
  };
  envelope.proofHash = hashJson({ ...envelope, proofHash: "" });
  return envelope;
}

function canonicalPath(value: string): string {
  return process.platform === "win32" ? resolve(value).toLowerCase() : resolve(value);
}

function sameOrInside(root: string, candidate: string): boolean {
  const rootPath = canonicalPath(root);
  const candidatePath = canonicalPath(candidate);
  if (candidatePath === rootPath) {
    return true;
  }
  const rel = relative(rootPath, candidatePath);
  return rel !== "" && !rel.startsWith("..") && !isAbsolute(rel);
}

function resolveWorkspacePath(config: AgentActionHostConfig, input = "."): string | IpcError {
  const candidate = resolve(config.workspaceRoot, input);
  if (!sameOrInside(config.workspaceRoot, candidate)) {
    return actionError("bad_payload", "Agent action path is outside the active workspace.", { input, workspace: config.workspaceRoot });
  }
  for (const protectedRoot of PROTECTED_ROOTS) {
    if (sameOrInside(protectedRoot, candidate) || sameOrInside(candidate, protectedRoot)) {
      return actionError("bad_payload", "Agent action rejected because it targets a protected root.", { input, protectedRoot });
    }
  }
  return candidate;
}

function pathKind(entry: { isDirectory(): boolean; isFile(): boolean }): AgentActionPathEntry["kind"] {
  if (entry.isDirectory()) return "directory";
  if (entry.isFile()) return "file";
  return "other";
}

function clampMaxResults(value: unknown, fallback: number): number {
  return Math.max(1, Math.min(500, typeof value === "number" && Number.isInteger(value) ? value : fallback));
}

export function createAgentActionHostManifest(config: AgentActionHostConfig): AgentActionHostManifest {
  const capabilities: AgentActionCapability[] = [
    {
      id: "fs.list",
      title: "List workspace files",
      status: "available",
      risk: "read",
      underlyingTools: ["node:fs/readdir", "PowerShell Get-ChildItem", "rg --files"],
      requiresApproval: false,
      writes: false,
      description: "Enumerate files and directories inside the active workspace only."
    },
    {
      id: "fs.search",
      title: "Search workspace content",
      status: "available",
      risk: "read",
      underlyingTools: ["rg", "node fallback"],
      requiresApproval: false,
      writes: false,
      description: "Search file contents inside the active workspace with bounded output."
    },
    {
      id: "fs.create_directory",
      title: "Create directory",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/mkdir", "PowerShell New-Item"],
      requiresApproval: false,
      writes: true,
      description: "Create one non-recursive directory inside the active workspace."
    },
    {
      id: "fs.rename",
      title: "Rename path",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/rename", "PowerShell Rename-Item"],
      requiresApproval: false,
      writes: true,
      description: "Rename one file or directory inside the active workspace."
    },
    {
      id: "fs.move",
      title: "Move path",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/rename", "PowerShell Move-Item"],
      requiresApproval: false,
      writes: true,
      description: "Move one file or directory between two workspace-contained paths."
    },
    {
      id: "fs.delete_empty_directory",
      title: "Delete empty directory",
      status: "available",
      risk: "destructive",
      underlyingTools: ["node:fs/rm", "PowerShell Remove-Item"],
      requiresApproval: true,
      writes: true,
      description: "Delete only an empty directory; recursive delete is blocked in this tranche."
    },
    {
      id: "shell.readonly",
      title: "Run read-only command",
      status: "available",
      risk: "read",
      underlyingTools: ["rg", "git status", "git diff", "git branch", "git rev-parse"],
      requiresApproval: false,
      writes: false,
      description: "Execute a small allowlist of read-only workspace inspection commands."
    },
    {
      id: "browser.playwright",
      title: "Contained browser automation",
      status: "planned",
      risk: "external_ui",
      underlyingTools: ["Playwright", "Electron WebContentsView", "CDP"],
      requiresApproval: true,
      writes: false,
      description: "Drive WebExplorer through a contained browser harness; external submissions remain confirmation-gated."
    },
    {
      id: "computer_use",
      title: "Computer use",
      status: "planned",
      risk: "external_ui",
      underlyingTools: ["screenshot", "Win32 SendInput", "UI Automation"],
      requiresApproval: true,
      writes: true,
      description: "Screen/click/keyboard control is reserved for a later confirmation-gated native adapter."
    },
    {
      id: "mcp",
      title: "MCP tools",
      status: "planned",
      risk: "external_ui",
      underlyingTools: ["tools/list", "tools/call"],
      requiresApproval: true,
      writes: false,
      description: "Expose external systems through MCP-compatible tool metadata and structured calls."
    }
  ];
  const manifest: AgentActionHostManifest = {
    schema: "ingen.agent_action_host.manifest.v1",
    workspace: {
      active: config.workspaceActive,
      root: config.workspaceRoot,
      cwd: config.cwd,
      protectedRoots: PROTECTED_ROOTS
    },
    permissions: {
      sandbox: "workspace",
      recursiveDelete: "blocked",
      shell: "readonly_allowlist",
      browser: "contained_webexplorer",
      computerUse: "planned_confirmation_required"
    },
    capabilities,
    proofHash: ""
  };
  manifest.proofHash = hashJson({ ...manifest, proofHash: "" });
  return manifest;
}

export function agentActionHostPromptManifest(config: AgentActionHostConfig): string {
  const manifest = createAgentActionHostManifest(config);
  return [
    "AGENT_ACTION_HOST v1",
    `schema=${manifest.schema}`,
    `workspace_active=${manifest.workspace.active}`,
    `workspace_root=${manifest.workspace.root}`,
    `sandbox=${manifest.permissions.sandbox}`,
    `recursive_delete=${manifest.permissions.recursiveDelete}`,
    `shell=${manifest.permissions.shell}`,
    `protected_roots=${manifest.workspace.protectedRoots.join("|")}`,
    "available=fs.list fs.search fs.create_directory fs.rename fs.move fs.delete_empty_directory shell.readonly",
    "planned=browser.playwright computer_use mcp",
    "rule=Prefer structured filesystem/search actions before shell. Recursive delete, protected roots, external submissions and full computer-use require explicit human confirmation.",
    `proof=${manifest.proofHash}`
  ].join("\n");
}

async function listAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const resolved = resolveWorkspacePath(config, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const entries = await readdir(resolved, { withFileTypes: true });
  const maxResults = clampMaxResults(request.maxResults, 100);
  const items = entries
    .slice(0, maxResults)
    .map((entry) => ({
      name: entry.name,
      path: relative(config.workspaceRoot, resolve(resolved, entry.name)) || ".",
      kind: pathKind(entry)
    }));
  return result(config, request, { accepted: true, path: relative(config.workspaceRoot, resolved) || ".", items });
}

function parseRgMatches(root: string, output: string, maxResults: number): AgentActionSearchMatch[] {
  const matches: AgentActionSearchMatch[] = [];
  for (const line of output.split(/\r?\n/)) {
    if (matches.length >= maxResults) break;
    const match = /^(.*?):(\d+):(.*)$/.exec(line);
    if (!match) continue;
    matches.push({
      path: relative(root, resolve(match[1])) || match[1],
      line: Number(match[2]),
      text: match[3].trim()
    });
  }
  return matches;
}

async function fallbackSearch(root: string, dir: string, query: string, maxResults: number, matches: AgentActionSearchMatch[] = []): Promise<AgentActionSearchMatch[]> {
  if (matches.length >= maxResults) {
    return matches;
  }
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (matches.length >= maxResults) break;
    if (entry.name === "node_modules" || entry.name === ".git" || entry.name === "dist" || entry.name === "dist-electron") {
      continue;
    }
    const fullPath = resolve(dir, entry.name);
    if (entry.isDirectory()) {
      await fallbackSearch(root, fullPath, query, maxResults, matches);
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    try {
      const text = await readFile(fullPath, "utf8");
      const lines = text.split(/\r?\n/);
      for (let index = 0; index < lines.length && matches.length < maxResults; index += 1) {
        if (lines[index].includes(query)) {
          matches.push({
            path: relative(root, fullPath) || entry.name,
            line: index + 1,
            text: lines[index].trim()
          });
        }
      }
    } catch {
      // Binary or unreadable files are ignored in the bounded fallback.
    }
  }
  return matches;
}

async function searchAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const query = request.query?.trim() ?? "";
  if (!query) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Search query is required.", request)
    });
  }
  const resolved = resolveWorkspacePath(config, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const maxResults = clampMaxResults(request.maxResults, 100);
  const rg = spawnSync("rg", ["--line-number", "--fixed-strings", "--max-count", String(maxResults), "--", query, resolved], {
    encoding: "utf8",
    stdio: "pipe",
    timeout: 15_000,
    windowsHide: true
  });
  if (rg.error) {
    const matches = await fallbackSearch(config.workspaceRoot, resolved, query, maxResults);
    return result(config, request, {
      accepted: true,
      path: relative(config.workspaceRoot, resolved) || ".",
      matches,
      stderrPreview: `rg unavailable; used bounded node fallback: ${rg.error.message}`
    });
  }
  if (rg.status !== 0 && !rg.stdout.trim()) {
    return result(config, request, {
      accepted: true,
      path: relative(config.workspaceRoot, resolved) || ".",
      matches: [],
      exitCode: rg.status
    });
  }
  return result(config, request, {
    accepted: true,
    path: relative(config.workspaceRoot, resolved) || ".",
    matches: parseRgMatches(config.workspaceRoot, rg.stdout, maxResults),
    exitCode: rg.status
  });
}

async function createDirectoryAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const resolved = resolveWorkspacePath(config, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  await mkdir(resolved, { recursive: false });
  return result(config, request, { accepted: true, path: relative(config.workspaceRoot, resolved) || "." });
}

async function renameOrMoveAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const from = resolveWorkspacePath(config, request.path);
  const to = resolveWorkspacePath(config, request.toPath);
  if (typeof from !== "string") {
    return result(config, request, { accepted: false, error: from });
  }
  if (typeof to !== "string") {
    return result(config, request, { accepted: false, error: to });
  }
  await rename(from, to);
  return result(config, request, {
    accepted: true,
    path: relative(config.workspaceRoot, from) || ".",
    toPath: relative(config.workspaceRoot, to) || "."
  });
}

async function deleteEmptyDirectoryAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Deleting a directory requires confirmed:true.", request)
    });
  }
  const resolved = resolveWorkspacePath(config, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  if (canonicalPath(resolved) === canonicalPath(config.workspaceRoot)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Deleting the workspace root is blocked.", request)
    });
  }
  const info = await stat(resolved);
  if (!info.isDirectory()) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "delete_empty_directory only accepts directories.", request)
    });
  }
  const entries = await readdir(resolved);
  if (entries.length > 0) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Directory is not empty; recursive delete is blocked.", { path: request.path, entries: entries.length })
    });
  }
  await rmdir(resolved);
  return result(config, request, { accepted: true, path: relative(config.workspaceRoot, resolved) || "." });
}

function readonlyCommandAllowed(command: string, args: string[] = []): boolean {
  const normalized = command.toLowerCase();
  if (normalized === "rg" || normalized === "rg.exe") {
    return true;
  }
  if (normalized !== "git" && normalized !== "git.exe") {
    return false;
  }
  const subcommand = args[0] ?? "";
  return ["status", "diff", "branch", "rev-parse", "log", "show"].includes(subcommand);
}

function runReadonlyCommandAction(config: AgentActionHostConfig, request: AgentActionRequest): AgentActionResult {
  const command = request.command?.trim() ?? "";
  const args = request.args ?? [];
  if (!command || !readonlyCommandAllowed(command, args)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Command is not in the read-only allowlist.", { command, args })
    });
  }
  const child = spawnSync(command, args, {
    cwd: config.cwd,
    encoding: "utf8",
    stdio: "pipe",
    timeout: 15_000,
    windowsHide: true
  });
  if (child.error) {
    return result(config, request, {
      accepted: false,
      commandLine: [command, ...args].join(" "),
      error: actionError("rust_unavailable", child.error.message, { command, args })
    });
  }
  return result(config, request, {
    accepted: child.status === 0,
    commandLine: [command, ...args].join(" "),
    exitCode: child.status,
    stdoutPreview: (child.stdout ?? "").slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: (child.stderr ?? "").slice(0, MAX_PREVIEW_CHARS),
    error:
      child.status === 0
        ? undefined
        : actionError("rust_unavailable", `Command exited with status ${child.status ?? "unknown"}.`, { command, args, stderr: child.stderr })
  });
}

export async function executeAgentActionRequest(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  try {
    switch (request.action) {
      case "list":
        return await listAction(config, request);
      case "search":
        return await searchAction(config, request);
      case "create_directory":
        return await createDirectoryAction(config, request);
      case "rename_path":
      case "move_path":
        return await renameOrMoveAction(config, request);
      case "delete_empty_directory":
        return await deleteEmptyDirectoryAction(config, request);
      case "run_readonly_command":
        return runReadonlyCommandAction(config, request);
      default:
        return result(config, request, {
          accepted: false,
          error: actionError("bad_payload", "Unknown agent action.", request)
        });
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return result(config, request, {
      accepted: false,
      error: actionError("rust_unavailable", message, request)
    });
  }
}
