import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, readdir, readFile, rename, rm, rmdir, stat } from "node:fs/promises";
import { isAbsolute, parse, relative, resolve } from "node:path";
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
const WINDOWS_DESTRUCTIVE_BLOCK_ROOTS = [
  "C:\\",
  "C:\\Windows",
  "C:\\Program Files",
  "C:\\Program Files (x86)",
  "C:\\ProgramData"
];
const MAX_PREVIEW_CHARS = 24_000;
const DEFAULT_COMMAND_TIMEOUT_MS = 30_000;
const AGENT_ACTION_EVENT_HINTS = [
  "fs.list:/agent_list_",
  "fs.search:/agent_search_",
  "fs.create_directory:/agent_create_directory_",
  "fs.rename:/agent_rename_path_",
  "fs.move:/agent_move_path_",
  "fs.copy:/agent_copy_path_",
  "fs.delete_empty_directory:/agent_delete_empty_directory_",
  "fs.delete_tree:/agent_delete_tree_",
  "shell.readonly:/agent_readonly_shell_",
  "shell.full:/agent_shell_"
];

const AGENT_ACTION_EVENT_BY_ACTION: Record<AgentActionRequest["action"], string> = {
  list: "/agent_list_",
  search: "/agent_search_",
  create_directory: "/agent_create_directory_",
  rename_path: "/agent_rename_path_",
  move_path: "/agent_move_path_",
  copy_path: "/agent_copy_path_",
  delete_empty_directory: "/agent_delete_empty_directory_",
  delete_tree: "/agent_delete_tree_",
  run_readonly_command: "/agent_readonly_shell_",
  run_command: "/agent_shell_"
};

export function agentActionRoutingHint(): string {
  return [
    "LOCAL_ACTION_TOOLS v1",
    "summary=Use local actions when the user asks to inspect, search, create, copy, move, rename, delete files/folders, or run local commands on the workspace/computer.",
    "families=fs.list fs.search fs.create_directory fs.rename fs.move fs.copy fs.delete_empty_directory fs.delete_tree shell.readonly shell.full",
    "format=Emit AGENT_ACTION_JSON only when real execution is needed, then wait for AGENT_ACTION_RESULT. Never fake tool events."
  ].join("\n");
}

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

function pathLabel(config: AgentActionHostConfig, request: AgentActionRequest, resolvedPath: string): string {
  return request.scope === "computer" ? resolvedPath : relative(config.workspaceRoot, resolvedPath) || ".";
}

function requiresComputerWriteConfirmation(request: AgentActionRequest): boolean {
  return request.scope === "computer" && request.confirmed !== true;
}

function dangerousDeleteTarget(config: AgentActionHostConfig, candidate: string): IpcError | undefined {
  const canonical = canonicalPath(candidate);
  const root = canonicalPath(parse(candidate).root || candidate);
  if (canonical === root || canonical === canonicalPath(config.workspaceRoot)) {
    return actionError("bad_payload", "Deleting a drive root or workspace root is blocked.", { candidate });
  }
  const blockRoots = process.platform === "win32" ? WINDOWS_DESTRUCTIVE_BLOCK_ROOTS : ["/", "/bin", "/etc", "/usr", "/var", "/System", "/Library"];
  for (const blocked of blockRoots) {
    const blockedRoot = canonicalPath(parse(blocked).root || blocked);
    const blocksByEqualityOnly = canonicalPath(blocked) === blockedRoot;
    const blockedMatch = blocksByEqualityOnly ? canonical === canonicalPath(blocked) : sameOrInside(blocked, candidate);
    if (blockedMatch) {
      return actionError("bad_payload", "Recursive deletion inside a protected system root is blocked.", { candidate, blocked });
    }
  }
  for (const protectedRoot of PROTECTED_ROOTS) {
    if (sameOrInside(protectedRoot, candidate) || sameOrInside(candidate, protectedRoot)) {
      return actionError("bad_payload", "Agent action rejected because it targets a protected root.", { candidate, protectedRoot });
    }
  }
  return undefined;
}

function protectedPathError(input: string, candidate: string): IpcError | undefined {
  for (const protectedRoot of PROTECTED_ROOTS) {
    if (sameOrInside(protectedRoot, candidate) || sameOrInside(candidate, protectedRoot)) {
      return actionError("bad_payload", "Agent action rejected because it targets a protected root.", { input, protectedRoot });
    }
  }
  return undefined;
}

function resolveActionPath(config: AgentActionHostConfig, request: AgentActionRequest, input = "."): string | IpcError {
  const scope = request.scope ?? "workspace";
  const candidate = scope === "computer" ? resolve(config.cwd, input) : resolve(config.workspaceRoot, input);
  if (scope === "workspace" && !sameOrInside(config.workspaceRoot, candidate)) {
    return actionError("bad_payload", "Agent action path is outside the active workspace. Use scope:\"computer\" with confirmation for whole-computer actions.", { input, workspace: config.workspaceRoot });
  }
  const protectedError = protectedPathError(input, candidate);
  if (protectedError) {
    return protectedError;
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
      title: "List files",
      status: "available",
      risk: "read",
      underlyingTools: ["node:fs/readdir", "PowerShell Get-ChildItem", "rg --files"],
      requiresApproval: false,
      writes: false,
      description: "Enumerate files and directories inside the workspace or, with scope:\"computer\", anywhere except protected roots."
    },
    {
      id: "fs.search",
      title: "Search content",
      status: "available",
      risk: "read",
      underlyingTools: ["rg", "node fallback"],
      requiresApproval: false,
      writes: false,
      description: "Search file contents inside the workspace or, with scope:\"computer\", bounded areas of the computer."
    },
    {
      id: "fs.create_directory",
      title: "Create directory",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/mkdir", "PowerShell New-Item"],
      requiresApproval: true,
      writes: true,
      description: "Create one directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set."
    },
    {
      id: "fs.rename",
      title: "Rename path",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/rename", "PowerShell Rename-Item"],
      requiresApproval: true,
      writes: true,
      description: "Rename one file or directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set."
    },
    {
      id: "fs.move",
      title: "Move path",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/rename", "PowerShell Move-Item"],
      requiresApproval: true,
      writes: true,
      description: "Move one file or directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set."
    },
    {
      id: "fs.copy",
      title: "Copy path",
      status: "available",
      risk: "workspace_write",
      underlyingTools: ["node:fs/cp", "PowerShell Copy-Item"],
      requiresApproval: true,
      writes: true,
      description: "Copy files or directories. Directory copy requires recursive:true; computer scope requires confirmed:true."
    },
    {
      id: "fs.delete_empty_directory",
      title: "Delete empty directory",
      status: "available",
      risk: "destructive",
      underlyingTools: ["node:fs/rmdir", "PowerShell Remove-Item"],
      requiresApproval: true,
      writes: true,
      description: "Delete only an empty directory; works in workspace or confirmed computer scope."
    },
    {
      id: "fs.delete_tree",
      title: "Delete tree",
      status: "available",
      risk: "destructive",
      underlyingTools: ["node:fs/rm recursive", "PowerShell Remove-Item -Recurse"],
      requiresApproval: true,
      writes: true,
      description: "Recursively delete a file or directory after confirmed:true and absolute root guards. System/protected roots are blocked."
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
      id: "shell.full",
      title: "Run confirmed command",
      status: "available",
      risk: "computer_write",
      underlyingTools: ["PowerShell", "cmd", "bash", "native shell"],
      requiresApproval: true,
      writes: true,
      description: "Execute an arbitrary local command only when confirmed:true is set. This is the escape hatch for settings and system tools."
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
      sandbox: "workspace_or_confirmed_computer",
      recursiveDelete: "confirmed_with_absolute_path_guard",
      shell: "readonly_allowlist_or_confirmed_full",
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
    "available=fs.list fs.search fs.create_directory fs.rename fs.move fs.copy fs.delete_empty_directory fs.delete_tree shell.readonly shell.full",
    `events=${AGENT_ACTION_EVENT_HINTS.join(" ")}`,
    "loop_stream=When local action is needed, write one short progress paragraph first, then emit exactly one AGENT_ACTION_JSON line. After the app returns AGENT_ACTION_RESULT, continue with another short paragraph plus another AGENT_ACTION_JSON if more work remains, or finish with a compact summary.",
    "action_request_format=AGENT_ACTION_JSON {\"action\":\"copy_path\",\"scope\":\"computer\",\"path\":\"C:\\\\from.txt\",\"toPath\":\"C:\\\\to.txt\",\"confirmed\":true}",
    "tool_truth=Never claim an action was executed unless you emitted AGENT_ACTION_JSON and received AGENT_ACTION_RESULT from the app. The app renders the matching event icon; do not fake event lines by themselves.",
    "planned=browser.playwright computer_use mcp",
    "rule=Default to scope:\"workspace\". Use scope:\"computer\" only for explicit whole-computer requests; writes, recursive deletion and arbitrary shell require confirmed:true. Prefer structured filesystem/search actions before shell. Protected roots, external submissions and full computer-use require explicit human confirmation.",
    `proof=${manifest.proofHash}`
  ].join("\n");
}

export function agentActionEventCommandForRequest(request: AgentActionRequest): string {
  return AGENT_ACTION_EVENT_BY_ACTION[request.action];
}

async function listAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const entries = await readdir(resolved, { withFileTypes: true });
  const maxResults = clampMaxResults(request.maxResults, 100);
  const items = entries
    .slice(0, maxResults)
    .map((entry) => ({
      name: entry.name,
      path: pathLabel(config, request, resolve(resolved, entry.name)),
      kind: pathKind(entry)
    }));
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved), items });
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
  const resolved = resolveActionPath(config, request, request.path);
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
    const matches = await fallbackSearch(request.scope === "computer" ? resolved : config.workspaceRoot, resolved, query, maxResults);
    return result(config, request, {
      accepted: true,
      path: pathLabel(config, request, resolved),
      matches,
      stderrPreview: `rg unavailable; used bounded node fallback: ${rg.error.message}`
    });
  }
  if (rg.status !== 0 && !rg.stdout.trim()) {
    return result(config, request, {
      accepted: true,
      path: pathLabel(config, request, resolved),
      matches: [],
      exitCode: rg.status
    });
  }
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, resolved),
    matches: parseRgMatches(request.scope === "computer" ? resolved : config.workspaceRoot, rg.stdout, maxResults),
    exitCode: rg.status
  });
}

async function createDirectoryAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (requiresComputerWriteConfirmation(request)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Computer-scope directory creation requires confirmed:true.", request)
    });
  }
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  await mkdir(resolved, { recursive: false });
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved) });
}

async function renameOrMoveAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (requiresComputerWriteConfirmation(request)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Computer-scope rename/move requires confirmed:true.", request)
    });
  }
  const from = resolveActionPath(config, request, request.path);
  const to = resolveActionPath(config, request, request.toPath);
  if (typeof from !== "string") {
    return result(config, request, { accepted: false, error: from });
  }
  if (typeof to !== "string") {
    return result(config, request, { accepted: false, error: to });
  }
  await rename(from, to);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: pathLabel(config, request, to)
  });
}

async function copyAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (requiresComputerWriteConfirmation(request)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Computer-scope copy requires confirmed:true.", request)
    });
  }
  const from = resolveActionPath(config, request, request.path);
  const to = resolveActionPath(config, request, request.toPath);
  if (typeof from !== "string") {
    return result(config, request, { accepted: false, error: from });
  }
  if (typeof to !== "string") {
    return result(config, request, { accepted: false, error: to });
  }
  await cp(from, to, { recursive: request.recursive === true, force: false, errorOnExist: true });
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: pathLabel(config, request, to)
  });
}

async function deleteEmptyDirectoryAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Deleting a directory requires confirmed:true.", request)
    });
  }
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const deleteGuard = dangerousDeleteTarget(config, resolved);
  if (deleteGuard) {
    return result(config, request, {
      accepted: false,
      error: deleteGuard
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
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved) });
}

async function deleteTreeAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true || request.recursive !== true) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Recursive deletion requires confirmed:true and recursive:true.", request)
    });
  }
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const deleteGuard = dangerousDeleteTarget(config, resolved);
  if (deleteGuard) {
    return result(config, request, {
      accepted: false,
      error: deleteGuard
    });
  }
  await rm(resolved, { recursive: true, force: false });
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved) });
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

function commandTimeout(request: AgentActionRequest): number {
  return Math.max(100, Math.min(600_000, request.timeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS));
}

function runCommandAction(config: AgentActionHostConfig, request: AgentActionRequest): AgentActionResult {
  const command = request.command?.trim() ?? "";
  const args = request.args ?? [];
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Arbitrary command execution requires confirmed:true.", { command, args })
    });
  }
  if (!command) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Command is required.", request)
    });
  }
  const child = spawnSync(command, args, {
    cwd: request.scope === "computer" ? config.cwd : config.workspaceRoot,
    encoding: "utf8",
    stdio: "pipe",
    timeout: commandTimeout(request),
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
      case "copy_path":
        return await copyAction(config, request);
      case "delete_empty_directory":
        return await deleteEmptyDirectoryAction(config, request);
      case "delete_tree":
        return await deleteTreeAction(config, request);
      case "run_readonly_command":
        return runReadonlyCommandAction(config, request);
      case "run_command":
        return runCommandAction(config, request);
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
