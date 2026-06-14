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
  AgentCapabilityAtlasEntry,
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
const MAX_PREVIEW_CHARS = 12_000;
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
    "summary=Use local actions when the user asks to inspect, search, create, copy, move, rename, delete files/folders, run commands, control Windows settings/tools, install/update software, download assets, or operate the workspace/computer.",
    "families=fs.list fs.search fs.create_directory fs.rename fs.move fs.copy fs.delete_empty_directory fs.delete_tree shell.readonly shell.full",
    "windows_reach=shell.full can invoke PowerShell, cmd.exe, winget, reg.exe, schtasks, netsh, DISM, rundll32, Start-Process, ms-settings URIs, installers, CLIs, and other native Windows tools when confirmed:true is appropriate.",
    "format=Emit AGENT_ACTION_JSON only when real execution is needed, then wait for AGENT_ACTION_RESULT. The AGENT_ACTION_JSON marker must start its own line, with no prose before it. Never fake tool events.",
    "retry=If a tool fails, read AGENT_ACTION_RESULT and try another safe available route before concluding blocked.",
    "loop_style=Write natural progress notes with varied openings. Avoid repeating 'Je vais'; prefer present-tense observation, decision, then action."
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

function charDeltaValue(added: number, removed: number): string {
  return `chars +${Math.max(0, Math.trunc(added))} -${Math.max(0, Math.trunc(removed))}`;
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

function atlasEntry(entry: AgentCapabilityAtlasEntry): AgentCapabilityAtlasEntry {
  return entry;
}

function actionCapability(entry: AgentActionCapability): AgentActionCapability {
  return entry;
}

function createExecutableActionCapabilities(): AgentActionCapability[] {
  return [
    actionCapability({
      id: "fs.list",
      family: "filesystem.discovery",
      surface: "filesystem",
      title: "List files",
      status: "available",
      risk: "read",
      operations: ["enumerate directories", "inspect path metadata"],
      underlyingTools: ["node:fs/readdir", "PowerShell Get-ChildItem", "rg --files"],
      fallbacks: ["shell.readonly Get-ChildItem", "shell.readonly rg --files"],
      verification: ["filesystem"],
      approval: "none",
      executableActionIds: ["fs.list"],
      requiresApproval: false,
      writes: false,
      description: "Enumerate files and directories inside the workspace or, with scope:\"computer\", anywhere except protected roots.",
      notes: "Prefer this before shell when the user asks what is in a folder."
    }),
    actionCapability({
      id: "fs.search",
      family: "filesystem.search",
      surface: "filesystem",
      title: "Search content",
      status: "available",
      risk: "read",
      operations: ["search file contents", "search bounded paths"],
      underlyingTools: ["rg", "node fallback"],
      fallbacks: ["shell.readonly Select-String", "shell.readonly findstr"],
      verification: ["filesystem"],
      approval: "none",
      executableActionIds: ["fs.search"],
      requiresApproval: false,
      writes: false,
      description: "Search file contents inside the workspace or, with scope:\"computer\", bounded areas of the computer.",
      notes: "Use compact result previews; keep broad computer searches bounded."
    }),
    actionCapability({
      id: "fs.create_directory",
      family: "filesystem.mutation",
      surface: "filesystem",
      title: "Create directory",
      status: "available",
      risk: "workspace_write",
      operations: ["create directory"],
      underlyingTools: ["node:fs/mkdir", "PowerShell New-Item"],
      fallbacks: ["shell.full New-Item", "cmd mkdir"],
      verification: ["filesystem"],
      approval: "confirmed",
      executableActionIds: ["fs.create_directory"],
      requiresApproval: true,
      writes: true,
      description: "Create one directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set.",
      notes: "Computer scope writes require explicit confirmation."
    }),
    actionCapability({
      id: "fs.rename",
      family: "filesystem.mutation",
      surface: "filesystem",
      title: "Rename path",
      status: "available",
      risk: "workspace_write",
      operations: ["rename file", "rename directory"],
      underlyingTools: ["node:fs/rename", "PowerShell Rename-Item"],
      fallbacks: ["shell.full Rename-Item", "cmd ren"],
      verification: ["filesystem"],
      approval: "confirmed",
      executableActionIds: ["fs.rename"],
      requiresApproval: true,
      writes: true,
      description: "Rename one file or directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set.",
      notes: "Verify old path disappeared and new path exists."
    }),
    actionCapability({
      id: "fs.move",
      family: "filesystem.mutation",
      surface: "filesystem",
      title: "Move path",
      status: "available",
      risk: "workspace_write",
      operations: ["move file", "move directory"],
      underlyingTools: ["node:fs/rename", "PowerShell Move-Item"],
      fallbacks: ["shell.full Move-Item", "robocopy then remove source"],
      verification: ["filesystem"],
      approval: "confirmed",
      executableActionIds: ["fs.move"],
      requiresApproval: true,
      writes: true,
      description: "Move one file or directory inside the workspace, or anywhere on the computer when scope:\"computer\" and confirmed:true are set.",
      notes: "Use path guards before any whole-computer move."
    }),
    actionCapability({
      id: "fs.copy",
      family: "filesystem.mutation",
      surface: "filesystem",
      title: "Copy path",
      status: "available",
      risk: "workspace_write",
      operations: ["copy file", "copy directory"],
      underlyingTools: ["node:fs/cp", "PowerShell Copy-Item"],
      fallbacks: ["shell.full Copy-Item", "robocopy"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["fs.copy"],
      requiresApproval: true,
      writes: true,
      description: "Copy files or directories. Directory copy requires recursive:true; computer scope requires confirmed:true.",
      notes: "For important copies, verify size or hash when practical."
    }),
    actionCapability({
      id: "fs.delete_empty_directory",
      family: "filesystem.destructive",
      surface: "filesystem",
      title: "Delete empty directory",
      status: "available",
      risk: "destructive",
      operations: ["delete empty directory"],
      underlyingTools: ["node:fs/rmdir", "PowerShell Remove-Item"],
      fallbacks: ["shell.full Remove-Item"],
      verification: ["filesystem"],
      approval: "prompt",
      executableActionIds: ["fs.delete_empty_directory"],
      requiresApproval: true,
      writes: true,
      description: "Delete only an empty directory; works in workspace or confirmed computer scope.",
      notes: "Deletion requires confirmation and protected roots remain blocked."
    }),
    actionCapability({
      id: "fs.delete_tree",
      family: "filesystem.destructive",
      surface: "filesystem",
      title: "Delete tree",
      status: "available",
      risk: "destructive",
      operations: ["recursive delete"],
      underlyingTools: ["node:fs/rm recursive", "PowerShell Remove-Item -Recurse"],
      fallbacks: ["shell.full Remove-Item -Recurse"],
      verification: ["filesystem"],
      approval: "prompt",
      executableActionIds: ["fs.delete_tree"],
      requiresApproval: true,
      writes: true,
      description: "Recursively delete a file or directory after confirmed:true and absolute root guards. System/protected roots are blocked.",
      notes: "Never run against unresolved paths or protected/system roots."
    }),
    actionCapability({
      id: "shell.readonly",
      family: "shell.readonly",
      surface: "shell",
      title: "Run read-only command",
      status: "available",
      risk: "read",
      operations: ["inspect workspace", "read command output"],
      underlyingTools: ["rg", "git status", "git diff", "git branch", "git rev-parse"],
      fallbacks: ["filesystem.discovery", "filesystem.search"],
      verification: ["command_exit"],
      approval: "none",
      executableActionIds: ["shell.readonly"],
      requiresApproval: false,
      writes: false,
      description: "Execute a small allowlist of read-only workspace inspection commands.",
      notes: "Readonly shell is for inspection, not local mutation."
    }),
    actionCapability({
      id: "shell.full",
      family: "shell.full",
      surface: "shell",
      title: "Run confirmed command",
      status: "available",
      risk: "computer_write",
      operations: ["run arbitrary local command", "invoke Windows native tools", "download/install/update when confirmed"],
      underlyingTools: ["PowerShell", "cmd", "winget", "reg.exe", "schtasks", "netsh", "DISM", "rundll32", "Start-Process", "ms-settings", "native shell"],
      fallbacks: ["PowerShell", "cmd.exe", "Windows native CLI", "GUI/computer_use when available"],
      verification: ["command_exit", "filesystem", "process_state", "registry_state", "service_state", "package_state"],
      approval: "prompt",
      executableActionIds: ["shell.full"],
      requiresApproval: true,
      writes: true,
      description: "Execute an arbitrary local command only when confirmed:true is set. This is the universal Windows escape hatch for settings, installers, updates, downloads, CLIs, system tools and app automation.",
      notes: "Prefer structured APIs first; use shell.full as the confirmed universal Windows escape hatch."
    })
  ];
}

export function createAgentCapabilityAtlas(config: AgentActionHostConfig): AgentCapabilityAtlasEntry[] {
  const executable = createExecutableActionCapabilities();
  const atlas: AgentCapabilityAtlasEntry[] = [
    ...executable,
    atlasEntry({
      id: "package.winget",
      family: "package.manager",
      surface: "windows.package",
      title: "Install, update and inspect packages",
      status: "planned",
      risk: "computer_write",
      operations: ["list packages", "install software", "upgrade software", "uninstall software"],
      underlyingTools: ["winget", "PowerShell", "installer executables"],
      fallbacks: ["direct vendor installer", "browser download", "manual confirmation"],
      verification: ["package_state", "command_exit", "process_state"],
      approval: "prompt",
      writes: true,
      notes: "Not a direct action in v1; route through confirmed shell.full."
    }),
    atlasEntry({
      id: "windows.registry",
      family: "windows.registry",
      surface: "windows.system",
      title: "Inspect and modify registry keys",
      status: "planned",
      risk: "computer_write",
      operations: ["query keys", "set values", "delete values", "export keys"],
      underlyingTools: ["reg.exe", "PowerShell registry provider"],
      fallbacks: ["ms-settings URI", "GUI/computer_use"],
      verification: ["registry_state", "command_exit"],
      approval: "prompt",
      writes: true,
      notes: "System hive writes require explicit user approval; use shell.full only."
    }),
    atlasEntry({
      id: "windows.services",
      family: "windows.services",
      surface: "windows.system",
      title: "Inspect and control services",
      status: "planned",
      risk: "computer_write",
      operations: ["query services", "start service", "stop service", "change startup mode"],
      underlyingTools: ["Get-Service", "sc.exe", "PowerShell service cmdlets"],
      fallbacks: ["Services MMC via GUI/computer_use"],
      verification: ["service_state", "event_log"],
      approval: "prompt",
      writes: true,
      notes: "Service mutation can affect the machine and remains approval-gated."
    }),
    atlasEntry({
      id: "windows.processes",
      family: "windows.processes",
      surface: "windows.system",
      title: "Inspect and control processes",
      status: "planned",
      risk: "computer_write",
      operations: ["list processes", "start process", "stop process", "inspect handles"],
      underlyingTools: ["Get-Process", "Start-Process", "Stop-Process", "tasklist", "taskkill"],
      fallbacks: ["Task Manager via GUI/computer_use"],
      verification: ["process_state", "command_exit"],
      approval: "prompt",
      writes: true,
      notes: "Killing processes requires confirmation unless app owns the process."
    }),
    atlasEntry({
      id: "windows.scheduler",
      family: "windows.scheduler",
      surface: "windows.system",
      title: "Create and inspect scheduled tasks",
      status: "planned",
      risk: "computer_write",
      operations: ["query tasks", "create task", "enable/disable task", "delete task"],
      underlyingTools: ["schtasks", "ScheduledTasks PowerShell module", "Task Scheduler COM"],
      fallbacks: ["Task Scheduler GUI/computer_use"],
      verification: ["command_exit", "event_log"],
      approval: "prompt",
      writes: true,
      notes: "Recurring/background tasks must be visible in the audit trail."
    }),
    atlasEntry({
      id: "windows.wmi",
      family: "windows.wmi",
      surface: "windows.system",
      title: "Query Windows management state",
      status: "planned",
      risk: "read",
      operations: ["query OS state", "query devices", "query hardware", "query installed software"],
      underlyingTools: ["Get-CimInstance", "Get-WmiObject", "wmic"],
      fallbacks: ["PowerShell native cmdlets", "Event Logs"],
      verification: ["command_exit"],
      approval: "none",
      writes: false,
      notes: `Runtime platform=${config.platform}; mutation through WMI must use a separate prompt-gated capability.`
    }),
    atlasEntry({
      id: "windows.events",
      family: "windows.event_logs",
      surface: "windows.diagnostics",
      title: "Read Event Viewer and diagnostic logs",
      status: "planned",
      risk: "read",
      operations: ["query event logs", "filter errors", "export diagnostic snippets"],
      underlyingTools: ["Get-WinEvent", "wevtutil", "Event Viewer"],
      fallbacks: ["application logs", "PowerShell transcript"],
      verification: ["event_log", "command_exit"],
      approval: "none",
      writes: false,
      notes: "Use for independent verification after system changes."
    }),
    atlasEntry({
      id: "windows.network",
      family: "windows.network",
      surface: "windows.network",
      title: "Inspect and adjust network state",
      status: "planned",
      risk: "computer_write",
      operations: ["inspect adapters", "query routes", "test connectivity", "change firewall rules"],
      underlyingTools: ["netsh", "Get-NetAdapter", "Test-NetConnection", "New-NetFirewallRule"],
      fallbacks: ["Windows Settings URI", "Control Panel GUI"],
      verification: ["command_exit", "registry_state", "event_log"],
      approval: "prompt",
      writes: true,
      notes: "Firewall/VPN/proxy mutation is sensitive and must be user-approved."
    }),
    atlasEntry({
      id: "windows.settings",
      family: "windows.settings",
      surface: "windows.gui",
      title: "Open and control Windows Settings",
      status: "planned",
      risk: "external_ui",
      operations: ["open settings pages", "inspect visible settings", "guide or automate toggles"],
      underlyingTools: ["ms-settings URI", "Start-Process", "UI Automation"],
      fallbacks: ["Control Panel applets", "registry", "PowerShell"],
      verification: ["ui_state", "registry_state"],
      approval: "prompt",
      writes: true,
      notes: "Use API/CLI when available, then shell, then GUI/computer-use for settings that only exist visually."
    }),
    atlasEntry({
      id: "windows.credentials",
      family: "windows.credentials",
      surface: "windows.security",
      title: "Credential and secret boundaries",
      status: "blocked",
      risk: "blocked",
      operations: ["detect credential requirement", "request user presence", "refuse silent secret access"],
      underlyingTools: ["Credential Manager", "SecretManagement", "browser profile prompts"],
      fallbacks: ["manual user entry", "external authenticated connector"],
      verification: ["manual_confirmation"],
      approval: "blocked",
      writes: false,
      notes: "Never read or exfiltrate secrets silently; authenticated actions require explicit user presence."
    }),
    atlasEntry({
      id: "windows.certificates",
      family: "windows.certificates",
      surface: "windows.security",
      title: "Certificate store inspection",
      status: "planned",
      risk: "computer_write",
      operations: ["inspect certificate stores", "import certificate", "export public metadata"],
      underlyingTools: ["certutil", "PowerShell Cert: provider"],
      fallbacks: ["certmgr.msc GUI"],
      verification: ["command_exit", "registry_state"],
      approval: "prompt",
      writes: true,
      notes: "Private key export or trust changes are high-risk and prompt-gated."
    }),
    atlasEntry({
      id: "browser.cdp",
      family: "browser.cdp",
      surface: "browser",
      title: "Control browsers through DevTools Protocol",
      status: "planned",
      risk: "external_ui",
      operations: ["inspect page", "click/type", "capture network", "download files", "run browser tests"],
      underlyingTools: ["Chrome DevTools Protocol", "Playwright", "Electron WebContents"],
      fallbacks: ["GUI/computer_use", "MCP browser tools"],
      verification: ["browser_state", "ui_state", "artifact_hash"],
      approval: "prompt",
      writes: true,
      notes: "External submissions, purchases and account changes require confirmation."
    }),
    atlasEntry({
      id: "computer.ui_automation",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Control desktop apps through accessibility and input",
      status: "planned",
      risk: "external_ui",
      operations: ["screenshot", "OCR", "click", "type", "drag", "inspect accessibility tree"],
      underlyingTools: ["UI Automation", "Win32 SendInput", "screenshot/OCR"],
      fallbacks: ["PowerShell/CLI", "app-specific COM/API"],
      verification: ["ui_state", "manual_confirmation"],
      approval: "prompt",
      writes: true,
      notes: "Prefer structured API/CLI first, shell second, GUI/computer-use only when needed."
    }),
    atlasEntry({
      id: "automation.rpa",
      family: "automation.rpa",
      surface: "desktop_gui",
      title: "Run repeatable desktop RPA flows",
      status: "planned",
      risk: "external_ui",
      operations: ["record or invoke RPA flow", "automate legacy GUI workflow"],
      underlyingTools: ["Power Automate Desktop", "UI Automation"],
      fallbacks: ["computer.ui_automation", "shell.full"],
      verification: ["ui_state", "event_log"],
      approval: "prompt",
      writes: true,
      notes: "RPA flows must be explicit, audited and cancellable."
    }),
    atlasEntry({
      id: "office.com",
      family: "office.com",
      surface: "office",
      title: "Control Office apps through COM/VBA",
      status: "planned",
      risk: "computer_write",
      operations: ["read documents", "edit workbooks", "export PDFs", "run macros only with consent"],
      underlyingTools: ["COM Automation", "VBA object models", "PowerShell"],
      fallbacks: ["document parsers", "GUI/computer_use"],
      verification: ["filesystem", "artifact_hash", "ui_state"],
      approval: "prompt",
      writes: true,
      notes: "Macros and Outlook sending are sensitive and require explicit confirmation."
    }),
    atlasEntry({
      id: "documents.media",
      family: "documents.media",
      surface: "documents",
      title: "Read, transform and create documents/media",
      status: "planned",
      risk: "workspace_write",
      operations: ["parse PDF/Office", "convert files", "extract text", "write reports", "process images/audio/video"],
      underlyingTools: ["bundled document libraries", "Office COM", "ffmpeg if installed", "OCR"],
      fallbacks: ["shell.full external CLI", "manual export"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      writes: true,
      notes: "Large artifacts should be referenced by path, hash and compact manifest."
    }),
    atlasEntry({
      id: "dev.git",
      family: "dev.git",
      surface: "developer",
      title: "Develop, test and collaborate in repositories",
      status: "planned",
      risk: "workspace_write",
      operations: ["inspect repo", "edit code", "run tests", "commit", "push", "open PR"],
      underlyingTools: ["git", "npm", "cargo", "gh", "language toolchains"],
      fallbacks: ["MCP GitHub", "shell.full"],
      verification: ["command_exit", "filesystem", "mcp_result"],
      approval: "confirmed",
      writes: true,
      notes: "Use repo wrappers where required; preserve unrelated user changes."
    }),
    atlasEntry({
      id: "virtualization.wsl",
      family: "virtualization.wsl",
      surface: "virtualization",
      title: "Use WSL distributions",
      status: "planned",
      risk: "computer_write",
      operations: ["list distros", "run Linux commands", "import/export distros", "install WSL"],
      underlyingTools: ["wsl.exe", "PowerShell"],
      fallbacks: ["native Windows toolchain", "Docker"],
      verification: ["command_exit", "filesystem", "process_state"],
      approval: "prompt",
      writes: true,
      notes: "Distro install/import/export and cross-filesystem writes require confirmation."
    }),
    atlasEntry({
      id: "virtualization.hyperv_docker",
      family: "virtualization.hyperv_docker",
      surface: "virtualization",
      title: "Use containers and virtual machines",
      status: "planned",
      risk: "computer_write",
      operations: ["inspect containers", "run containers", "manage Hyper-V VMs"],
      underlyingTools: ["docker", "PowerShell Hyper-V module", "wsl.exe"],
      fallbacks: ["local toolchain", "cloud runner"],
      verification: ["process_state", "command_exit"],
      approval: "prompt",
      writes: true,
      notes: "VM/container lifecycle changes are prompt-gated."
    }),
    atlasEntry({
      id: "cloud.clis",
      family: "cloud.clis",
      surface: "cloud",
      title: "Use authenticated cloud CLIs",
      status: "planned",
      risk: "external_ui",
      operations: ["inspect cloud resources", "deploy", "download artifacts"],
      underlyingTools: ["az", "aws", "gcloud", "gh"],
      fallbacks: ["MCP connectors", "browser.cdp"],
      verification: ["command_exit", "mcp_result"],
      approval: "prompt",
      writes: true,
      notes: "Cloud writes and credential prompts require explicit user confirmation."
    }),
    atlasEntry({
      id: "mcp.plugins",
      family: "mcp.plugins",
      surface: "external_tools",
      title: "Use MCP, plugins, skills, hooks and subagents",
      status: "planned",
      risk: "external_ui",
      operations: ["list tools", "call tools", "delegate subtask", "run hook"],
      underlyingTools: ["MCP tools/list", "MCP tools/call", "plugin manifests", "skills"],
      fallbacks: ["shell.full", "browser.cdp"],
      verification: ["mcp_result", "artifact_hash"],
      approval: "prompt",
      writes: true,
      notes: "External tool calls inherit the risk of the underlying connector."
    }),
    atlasEntry({
      id: "automations.goals",
      family: "automations.goals",
      surface: "agent_runtime",
      title: "Run persistent goals and automations",
      status: "planned",
      risk: "computer_write",
      operations: ["schedule follow-up", "poll status", "resume work", "monitor command"],
      underlyingTools: ["Task Scheduler", "app automation ledger", "thread wakeups"],
      fallbacks: ["manual reminder", "shell.full schtasks"],
      verification: ["event_log", "mcp_result", "manual_confirmation"],
      approval: "prompt",
      writes: true,
      notes: "Background work must be visible, cancellable and summarized."
    }),
    atlasEntry({
      id: "security.admin_boundary",
      family: "security.admin_boundary",
      surface: "windows.security",
      title: "Respect admin and security boundaries",
      status: "blocked",
      risk: "blocked",
      operations: ["detect UAC/admin boundary", "stop before bypass", "ask user for approval"],
      underlyingTools: ["UAC", "Windows Security"],
      fallbacks: ["manual user action", "reduced-permission route"],
      verification: ["manual_confirmation"],
      approval: "blocked",
      writes: false,
      notes: "Never bypass UAC, approve security prompts, or weaken security silently."
    })
  ];
  return atlas;
}

function compactCapabilityAtlasLine(atlas: AgentCapabilityAtlasEntry[]): string {
  const familySummary = atlas
    .map((entry) => `${entry.family}:${entry.status}/${entry.approval}`)
    .join(" ");
  const plannedFamilies = atlas
    .filter((entry) => entry.status !== "available")
    .map((entry) => entry.family)
    .join("|");
  return `capability_atlas=count:${atlas.length} families=${familySummary} planned_or_blocked=${plannedFamilies}`;
}

export function createAgentActionHostManifest(config: AgentActionHostConfig): AgentActionHostManifest {
  const capabilities = createExecutableActionCapabilities();
  const capabilityAtlas = createAgentCapabilityAtlas(config);
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
    capabilityAtlas,
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
    compactCapabilityAtlasLine(manifest.capabilityAtlas),
    "capability_policy=Use the atlas for reasoning, not as fake execution. Prefer structured app/API/CLI routes first, then confirmed shell.full, then GUI/computer-use only when the task cannot be completed through a safer route.",
    "capability_limits=Planned or blocked atlas entries are not direct AGENT_ACTION_JSON actions. Use available executable actions only, or explain the missing backend/approval boundary.",
    "windows_reach=shell.full can use PowerShell/cmd plus Windows-native tools such as winget, reg.exe, schtasks, netsh, DISM, rundll32, Start-Process and ms-settings URIs when confirmed:true is warranted.",
    `events=${AGENT_ACTION_EVENT_HINTS.join(" ")}`,
    "loop_stream=When local action is needed, write one short progress paragraph first, then emit exactly one AGENT_ACTION_JSON line that starts with AGENT_ACTION_JSON at column 1. After the app returns AGENT_ACTION_RESULT, continue with another short paragraph plus another AGENT_ACTION_JSON if more work remains, or finish with a compact summary.",
    "retry=If AGENT_ACTION_RESULT reports failure, inspect the error and try a different safe route before declaring the task blocked.",
    "loop_style=Use varied, concrete progress notes. Do not start every step with 'Je vais'. Prefer forms like 'Le bureau contient...', 'Je regroupe maintenant...', 'Prochaine action logique...', 'Ce fichier va dans...'.",
    "action_request_format=AGENT_ACTION_JSON {\"action\":\"copy_path\",\"scope\":\"computer\",\"path\":\"C:\\\\from.txt\",\"toPath\":\"C:\\\\to.txt\",\"confirmed\":true}",
    "tool_truth=Never claim an action was executed unless you emitted AGENT_ACTION_JSON and received AGENT_ACTION_RESULT from the app. The app renders the matching event icon; do not fake event lines by themselves.",
    "planned=browser.playwright computer_use mcp",
    "rule=Default to scope:\"workspace\". Use scope:\"computer\" only for explicit whole-computer requests; writes, recursive deletion and arbitrary shell require confirmed:true. Prefer structured filesystem/search actions before shell. Protected roots, external submissions and full computer-use require explicit human confirmation.",
    `proof=${manifest.proofHash}`
  ].join("\n");
}

export function agentActionEventCommandForRequest(request: AgentActionRequest): string {
  const command = AGENT_ACTION_EVENT_BY_ACTION[request.action];
  const attributes = [
    request.path ? `path=${JSON.stringify(request.path)}` : "",
    request.toPath ? `toPath=${JSON.stringify(request.toPath)}` : ""
  ].filter(Boolean);
  return [command, ...attributes].join(" ");
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
  try {
    await mkdir(resolved, { recursive: false });
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? String((error as { code?: unknown }).code) : "";
    if (code !== "EEXIST") {
      throw error;
    }
    const existing = await stat(resolved);
    if (!existing.isDirectory()) {
      throw error;
    }
    return result(config, request, { accepted: true, path: pathLabel(config, request, resolved), value: "directory already exists" });
  }
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved), value: charDeltaValue(0, 0) });
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
    toPath: pathLabel(config, request, to),
    value: charDeltaValue(0, 0)
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
  const fromInfo = await stat(from);
  const addedChars = fromInfo.isFile() ? fromInfo.size : 0;
  await cp(from, to, { recursive: request.recursive === true, force: false, errorOnExist: true });
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: pathLabel(config, request, to),
    value: charDeltaValue(addedChars, 0)
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
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved), value: charDeltaValue(0, 0) });
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
  return result(config, request, { accepted: true, path: pathLabel(config, request, resolved), value: charDeltaValue(0, 0) });
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
