import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFile, cp, mkdir, readdir, readFile, rename, rm, rmdir, stat, writeFile } from "node:fs/promises";
import { basename, dirname, isAbsolute, parse, relative, resolve } from "node:path";
import type {
  AgentAppshotArtifact,
  AgentActionCapability,
  AgentActionHostManifest,
  AgentActionPathEntry,
  AgentActionRequest,
  AgentActionResult,
  AgentActionSearchMatch,
  AgentActionInstalledTool,
  AgentActionRuntimeManifestSummary,
  AgentCapabilityAtlasEntry,
  AgentCapabilityVerification,
  AgentBrowserDownloadArtifact,
  AgentBrowserPageSummary,
  AgentBrowserScreenshotArtifact,
  AgentBrowserWebPolicy,
  AgentComputerDisplaySummary,
  AgentComputerUsePolicy,
  AgentComputerUseSnapshot,
  AgentComputerWindowSummary,
  AgentUiAutomationNodeSummary,
  AgentDocumentMediaKind,
  AgentDocumentMediaPolicy,
  AgentDocumentMediaSummary,
  AgentAutomationLedgerEntry,
  AgentDeveloperAutomationPolicy,
  AgentDeveloperRepoSummary,
  AgentFailureCategory,
  AgentRetryStrategy,
  AgentRetryStrategyId,
  AgentVerificationPolicy,
  AgentVerificationProbe,
  AgentVerificationResult,
  AgentVirtualizationProvider,
  AgentVirtualizationSummary,
  AgentWindowsExecutionAdapterId,
  AgentWindowsExecutionPolicy,
  AgentWindowsRouteCatalogEntry,
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
const MAX_COMMAND_TIMEOUT_MS = 600_000;
const AGENT_ACTION_TOOL_DETECTION_TIMEOUT_MS = 450;
const AGENT_ACTION_TOOL_DETECTION_COMMANDS: readonly { id: string; command: string }[] = [
  { id: "powershell", command: "powershell.exe" },
  { id: "pwsh", command: "pwsh.exe" },
  { id: "cmd", command: "cmd.exe" },
  { id: "winget", command: "winget.exe" },
  { id: "reg", command: "reg.exe" },
  { id: "schtasks", command: "schtasks.exe" },
  { id: "netsh", command: "netsh.exe" },
  { id: "dism", command: "dism.exe" },
  { id: "rundll32", command: "rundll32.exe" },
  { id: "wsl", command: "wsl.exe" },
  { id: "docker", command: "docker.exe" },
  { id: "git", command: "git.exe" },
  { id: "gh", command: "gh.exe" },
  { id: "node", command: "node.exe" },
  { id: "npm", command: "npm.cmd" },
  { id: "cargo", command: "cargo.exe" },
  { id: "python", command: "python.exe" },
  { id: "code", command: "code.cmd" }
];
let agentActionToolDetectionCache: { platform: NodeJS.Platform; tools: AgentActionInstalledTool[] } | undefined;
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
  "shell.full:/agent_shell_",
  "computer.inspect:/agent_computer_inspect_",
  "computer.appshot:/agent_appshot_",
  "computer.focus_window:/agent_focus_window_",
  "computer.clipboard_read:/agent_clipboard_read_",
  "computer.clipboard_write:/agent_clipboard_write_",
  "computer.ui_tree:/agent_ui_tree_",
  "computer.ocr:/agent_ocr_",
  "computer.click:/agent_click_",
  "computer.type_text:/agent_type_text_",
  "computer.scroll:/agent_scroll_",
  "computer.drag:/agent_drag_",
  "browser.inspect_url:/agent_browser_inspect_",
  "browser.download:/agent_browser_download_",
  "browser.open_url:/agent_browser_open_",
  "browser.playwright_inspect:/agent_browser_playwright_inspect_",
  "browser.screenshot:/agent_browser_screenshot_",
  "browser.click:/agent_browser_click_",
  "browser.type_text:/agent_browser_type_text_",
  "browser.playwright_download:/agent_browser_playwright_download_",
  "document.inspect:/agent_document_inspect_",
  "document.write_text:/agent_document_write_",
  "document.write_json:/agent_document_write_",
  "document.write_csv:/agent_document_write_",
  "document.convert_text:/agent_document_convert_",
  "dev.repo_status:/agent_dev_status_",
  "dev.git_diff:/agent_dev_diff_",
  "dev.git_commit:/agent_dev_commit_",
  "dev.git_push:/agent_dev_push_",
  "dev.github_pr_create:/agent_github_pr_create_",
  "dev.run_check:/agent_dev_check_",
  "virtualization.inspect:/agent_virtualization_inspect_",
  "virtualization.run_command:/agent_virtualization_run_",
  "automation.schedule:/agent_automation_schedule_",
  "automation.list:/agent_automation_list_",
  "automation.cancel:/agent_automation_cancel_",
  "automation.record:/agent_automation_record_"
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
  run_command: "/agent_shell_",
  computer_inspect: "/agent_computer_inspect_",
  computer_appshot: "/agent_appshot_",
  computer_focus_window: "/agent_focus_window_",
  computer_clipboard_read: "/agent_clipboard_read_",
  computer_clipboard_write: "/agent_clipboard_write_",
  computer_ui_tree: "/agent_ui_tree_",
  computer_ocr: "/agent_ocr_",
  computer_click: "/agent_click_",
  computer_type_text: "/agent_type_text_",
  computer_scroll: "/agent_scroll_",
  computer_drag: "/agent_drag_",
  browser_inspect_url: "/agent_browser_inspect_",
  browser_download: "/agent_browser_download_",
  browser_open_url: "/agent_browser_open_",
  browser_playwright_inspect: "/agent_browser_playwright_inspect_",
  browser_screenshot: "/agent_browser_screenshot_",
  browser_click: "/agent_browser_click_",
  browser_type_text: "/agent_browser_type_text_",
  browser_playwright_download: "/agent_browser_playwright_download_",
  document_inspect: "/agent_document_inspect_",
  document_write_text: "/agent_document_write_",
  document_write_json: "/agent_document_write_",
  document_write_csv: "/agent_document_write_",
  document_convert_text: "/agent_document_convert_",
  dev_repo_status: "/agent_dev_status_",
  dev_git_diff: "/agent_dev_diff_",
  dev_git_commit: "/agent_dev_commit_",
  dev_git_push: "/agent_dev_push_",
  dev_github_pr_create: "/agent_github_pr_create_",
  dev_run_check: "/agent_dev_check_",
  virtualization_inspect: "/agent_virtualization_inspect_",
  virtualization_run_command: "/agent_virtualization_run_",
  automation_schedule: "/agent_automation_schedule_",
  automation_list: "/agent_automation_list_",
  automation_cancel: "/agent_automation_cancel_",
  automation_record: "/agent_automation_record_"
};

const WINDOWS_EXECUTION_ADAPTERS: AgentWindowsExecutionAdapterId[] = ["powershell", "cmd", "windows_command", "shell_full"];

const WINDOWS_ROUTE_CATALOG: AgentWindowsRouteCatalogEntry[] = [
  {
    id: "powershell.inline",
    adapter: "powershell",
    commands: ["powershell.exe", "pwsh.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "Get-ChildItem, Get-Process, Get-Service, Get-CimInstance, Get-ExecutionPolicy",
    gatedWriteScenario: "New-Item, Move-Item, Set-ItemProperty, Start-Process after confirmed:true",
    verification: ["command_exit", "filesystem", "process_state", "registry_state", "service_state"],
    notes: "Preferred Windows structured shell route for native cmdlets and CIM/WMI."
  },
  {
    id: "cmd.inline",
    adapter: "cmd",
    commands: ["cmd.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "dir, where, ver, assoc, ftype",
    gatedWriteScenario: "mkdir, ren, move, copy after confirmed:true",
    verification: ["command_exit", "filesystem"],
    notes: "Use for batch, cmd built-ins, and legacy Windows command behavior."
  },
  {
    id: "winget.package",
    adapter: "windows_command",
    commands: ["winget.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "winget list or winget search",
    gatedWriteScenario: "winget install, upgrade or uninstall after confirmed:true",
    verification: ["command_exit", "package_state"],
    notes: "Package-management route; installs and upgrades require explicit confirmation."
  },
  {
    id: "registry.reg",
    adapter: "windows_command",
    commands: ["reg.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "reg query",
    gatedWriteScenario: "reg add, delete or import after confirmed:true",
    verification: ["command_exit", "registry_state"],
    notes: "Registry writes are sensitive and must stay explicit."
  },
  {
    id: "scheduler.schtasks",
    adapter: "windows_command",
    commands: ["schtasks.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "schtasks /Query",
    gatedWriteScenario: "schtasks /Create, /Change or /Delete after confirmed:true",
    verification: ["command_exit", "process_state"],
    notes: "Task Scheduler route; persisted automations need confirmation."
  },
  {
    id: "network.netsh",
    adapter: "windows_command",
    commands: ["netsh.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "netsh interface show interface",
    gatedWriteScenario: "firewall or interface changes after confirmed:true",
    verification: ["command_exit"],
    notes: "Network and firewall route; avoid silent connectivity changes."
  },
  {
    id: "deployment.dism",
    adapter: "windows_command",
    commands: ["dism.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "dism /Online /Get-Features",
    gatedWriteScenario: "feature enable/disable after confirmed:true",
    verification: ["command_exit"],
    notes: "Windows component route; may require admin/UAC."
  },
  {
    id: "services.sc",
    adapter: "windows_command",
    commands: ["sc.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "sc.exe query",
    gatedWriteScenario: "sc.exe start, stop, config or delete after confirmed:true",
    verification: ["command_exit", "service_state"],
    notes: "Windows service route; service mutation is approval-gated."
  },
  {
    id: "processes.tasklist",
    adapter: "windows_command",
    commands: ["tasklist.exe"],
    risk: "read",
    approval: "none",
    readScenario: "tasklist /FO CSV",
    gatedWriteScenario: "none; use taskkill route for process termination",
    verification: ["command_exit", "process_state"],
    notes: "Read-only process inventory route."
  },
  {
    id: "processes.taskkill",
    adapter: "windows_command",
    commands: ["taskkill.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "tasklist before taskkill",
    gatedWriteScenario: "taskkill /PID or /IM after confirmed:true",
    verification: ["command_exit", "process_state"],
    notes: "Process termination route; do not kill system/security processes silently."
  },
  {
    id: "files.robocopy",
    adapter: "windows_command",
    commands: ["robocopy.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "robocopy /L dry-run",
    gatedWriteScenario: "robocopy copy/mirror after confirmed:true; mirror/delete needs extra care",
    verification: ["command_exit", "filesystem"],
    notes: "Large file copy/sync route; dry-run first for destructive switches."
  },
  {
    id: "security.icacls",
    adapter: "windows_command",
    commands: ["icacls.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "icacls path",
    gatedWriteScenario: "icacls grant/remove/reset after confirmed:true",
    verification: ["command_exit", "filesystem"],
    notes: "ACL mutation is security-sensitive and may require admin."
  },
  {
    id: "certificates.certutil",
    adapter: "windows_command",
    commands: ["certutil.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "certutil -hashfile or -store",
    gatedWriteScenario: "certificate import/delete after confirmed:true",
    verification: ["command_exit", "artifact_hash"],
    notes: "Certificate route; trust-store writes are sensitive."
  },
  {
    id: "events.wevtutil",
    adapter: "windows_command",
    commands: ["wevtutil.exe"],
    risk: "read",
    approval: "none",
    readScenario: "wevtutil qe System /c:20",
    gatedWriteScenario: "log clear/export after confirmed:true",
    verification: ["command_exit", "event_log"],
    notes: "Event Log route for diagnostics."
  },
  {
    id: "virtualization.wsl",
    adapter: "windows_command",
    commands: ["wsl.exe"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "wsl.exe --status or --list --verbose",
    gatedWriteScenario: "install/import/unregister after confirmed:true",
    verification: ["command_exit", "process_state"],
    notes: "WSL route; distribution changes need explicit confirmation."
  },
  {
    id: "shell.start_process",
    adapter: "powershell",
    commands: ["Start-Process"],
    risk: "external_ui",
    approval: "confirmed",
    readScenario: "Start-Process with -PassThru for an approved local target",
    gatedWriteScenario: "launch installers, Settings pages, elevated tools after confirmed:true",
    verification: ["command_exit", "process_state", "manual_confirmation"],
    notes: "GUI/process launch route through PowerShell."
  },
  {
    id: "settings.ms_settings",
    adapter: "windows_command",
    commands: ["ms-settings:"],
    risk: "external_ui",
    approval: "prompt",
    readScenario: "open a Settings URI for user-visible inspection",
    gatedWriteScenario: "user confirms changes in Settings UI",
    verification: ["manual_confirmation", "ui_state"],
    notes: "Settings URI opens UI; the app must not claim the user changed settings without verification."
  },
  {
    id: "shell.full",
    adapter: "shell_full",
    commands: ["*"],
    risk: "computer_write",
    approval: "confirmed",
    readScenario: "arbitrary CLI read after safer structured routes are unsuitable",
    gatedWriteScenario: "arbitrary shell mutation after confirmed:true",
    verification: ["command_exit", "manual_confirmation"],
    notes: "Universal fallback route; structured Windows routes are preferred."
  }
];

const AGENT_FAILURE_CATEGORIES: AgentFailureCategory[] = [
  "denied",
  "missing_tool",
  "bad_path",
  "timeout",
  "permission",
  "protected_root",
  "command_error",
  "unverifiable",
  "partial_success"
];

const AGENT_RETRY_STRATEGIES: AgentRetryStrategy[] = [
  {
    id: "api_cli",
    label: "Structured app/API/CLI route",
    appliesTo: ["missing_tool", "command_error", "unverifiable", "partial_success"],
    requiresApproval: "none",
    notes: "Prefer a typed local API or CLI before falling back to raw shell."
  },
  {
    id: "powershell",
    label: "PowerShell route",
    appliesTo: ["bad_path", "permission", "command_error", "timeout", "unverifiable", "partial_success"],
    requiresApproval: "confirmed",
    notes: "Use PowerShell cmdlets for Windows-native filesystem, registry, services and CIM/WMI operations."
  },
  {
    id: "cmd",
    label: "CMD route",
    appliesTo: ["bad_path", "command_error", "timeout"],
    requiresApproval: "confirmed",
    notes: "Use CMD for legacy built-ins, batch files and simple Windows shell fallbacks."
  },
  {
    id: "windows_command",
    label: "Native Windows command route",
    appliesTo: ["missing_tool", "command_error", "timeout", "partial_success"],
    requiresApproval: "confirmed",
    notes: "Use reg, schtasks, netsh, sc, robocopy, icacls, certutil, wevtutil, wsl or other cataloged commands."
  },
  {
    id: "wmi_cim",
    label: "WMI/CIM route",
    appliesTo: ["unverifiable", "command_error", "partial_success"],
    requiresApproval: "prompt",
    notes: "Use CIM/WMI when process, service, hardware or OS state needs structured verification."
  },
  {
    id: "registry",
    label: "Registry route",
    appliesTo: ["unverifiable", "command_error", "permission"],
    requiresApproval: "confirmed",
    notes: "Use only for registry-backed settings after explicit confirmation."
  },
  {
    id: "settings_uri",
    label: "Windows Settings URI route",
    appliesTo: ["permission", "unverifiable"],
    requiresApproval: "prompt",
    notes: "Open Settings UI when no safe CLI/API can mutate the target."
  },
  {
    id: "browser_cdp",
    label: "Browser CDP route",
    appliesTo: ["unverifiable", "partial_success"],
    requiresApproval: "prompt",
    notes: "Use for browser state verification once CDP is wired."
  },
  {
    id: "gui_computer_use",
    label: "GUI/computer-use route",
    appliesTo: ["permission", "unverifiable", "partial_success"],
    requiresApproval: "prompt",
    notes: "Last resort for UI-only tasks."
  },
  {
    id: "manual_approval",
    label: "Manual approval route",
    appliesTo: ["denied", "permission", "protected_root"],
    requiresApproval: "prompt",
    notes: "Stop and ask the user when policy, UAC, credentials or protected roots are involved."
  }
];

export function agentActionRoutingHint(): string {
  return [
    "LOCAL_ACTION_TOOLS v1",
    "summary=Use local actions when the user asks to inspect, search, create, copy, move, rename, delete files/folders, write/inspect documents, run commands, control Windows settings/tools, install/update software, download assets, or operate the workspace/computer.",
    "families=fs.list fs.search fs.create_directory fs.rename fs.move fs.copy fs.delete_empty_directory fs.delete_tree shell.readonly shell.full computer.inspect computer.appshot computer.focus_window computer.clipboard_read computer.clipboard_write computer.ui_tree computer.ocr computer.click computer.type_text computer.scroll computer.drag browser.inspect_url browser.download browser.open_url browser.playwright_inspect browser.screenshot browser.click browser.type_text browser.playwright_download document.inspect document.write_text document.write_json document.write_csv document.convert_text dev.repo_status dev.git_diff dev.git_commit dev.git_push dev.github_pr_create",
    "windows_reach=shell.full can invoke PowerShell, cmd.exe, winget, reg.exe, schtasks, netsh, DISM, rundll32, Start-Process, ms-settings URIs, installers, CLIs, and other native Windows tools when confirmed:true is appropriate.",
    "computer_use=Inspect GUI/UIA first, then act once with confirmed:true, then verify by foreground/UI state. Never approve security, payment, credential, destructive, or UAC prompts for the user.",
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

function estimatePromptTokens(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return Math.max(Math.ceil(trimmed.length / 4), Math.ceil(trimmed.split(/\s+/).length * 1.35));
}

function actionError(code: IpcError["code"], message: string, input: unknown): IpcError {
  return {
    code,
    message,
    proofHash: hashJson({ code, message, input })
  };
}

function categorizeFailure(params: { error?: IpcError; timedOut?: boolean; verification?: AgentVerificationResult }): AgentFailureCategory | undefined {
  if (params.timedOut) {
    return "timeout";
  }
  if (params.verification && !params.verification.passed) {
    return "unverifiable";
  }
  const message = params.error?.message ?? "";
  if (!message) {
    return undefined;
  }
  if (/protected root|system root|drive root|workspace root/i.test(message)) {
    return "protected_root";
  }
  if (/confirmed:true|confirmation|approval|denied/i.test(message)) {
    return "denied";
  }
  if (/outside the active workspace|bad path|not found|ENOENT|path/i.test(message)) {
    return "bad_path";
  }
  if (/permission|EACCES|EPERM|UAC|administrator/i.test(message)) {
    return "permission";
  }
  if (/not recognized|not found|ENOENT|spawn/i.test(message)) {
    return "missing_tool";
  }
  return "command_error";
}

function retryRoutesForFailure(category: AgentFailureCategory | undefined): AgentRetryStrategyId[] {
  if (!category) {
    return [];
  }
  if (category === "protected_root") {
    return [];
  }
  return AGENT_RETRY_STRATEGIES.filter((strategy) => strategy.appliesTo.includes(category)).map((strategy) => strategy.id);
}

function result(
  config: AgentActionHostConfig,
  request: AgentActionRequest,
  patch: Omit<Partial<AgentActionResult>, "schema" | "action" | "cwd" | "proofHash">
): AgentActionResult {
  const verificationFailed = patch.verification !== undefined && !patch.verification.passed;
  const accepted = (patch.accepted ?? false) && !verificationFailed;
  const error = patch.error ?? (verificationFailed ? actionError("rust_unavailable", "Independent verification failed.", patch.verification) : undefined);
  const failureCategory = patch.failureCategory ?? categorizeFailure({ error, timedOut: patch.timedOut, verification: patch.verification });
  const envelope: AgentActionResult = {
    schema: "ingen.agent_action_host.result.v1",
    accepted,
    action: request.action,
    cwd: config.cwd,
    path: patch.path,
    toPath: patch.toPath,
    items: patch.items,
    matches: patch.matches,
    commandLine: patch.commandLine,
    executionAdapter: patch.executionAdapter,
    routeId: patch.routeId,
    exitCode: patch.exitCode,
    durationMs: patch.durationMs,
    timeoutMs: patch.timeoutMs,
    timedOut: patch.timedOut,
    stdoutPreview: patch.stdoutPreview,
    stderrPreview: patch.stderrPreview,
    artifacts: patch.artifacts,
    observedChanges: patch.observedChanges,
    verification: patch.verification,
    computerUse: patch.computerUse,
    appshot: patch.appshot,
    browserPage: patch.browserPage,
    browserScreenshot: patch.browserScreenshot,
    download: patch.download,
    documentMedia: patch.documentMedia,
    developer: patch.developer,
    virtualization: patch.virtualization,
    automation: patch.automation,
    userPresenceRequired: patch.userPresenceRequired,
    failureCategory,
    retryRoutes: patch.retryRoutes ?? retryRoutesForFailure(failureCategory),
    value: patch.value,
    proofHash: "",
    error
  };
  envelope.proofHash = hashJson({ ...envelope, proofHash: "" });
  return envelope;
}

function charDeltaValue(added: number, removed: number): string {
  return `chars +${Math.max(0, Math.trunc(added))} -${Math.max(0, Math.trunc(removed))}`;
}

function verificationProbe(params: {
  id: string;
  kind: AgentCapabilityVerification;
  target?: string;
  expectation: string;
  actual: string;
  passed: boolean;
}): AgentVerificationProbe {
  const probe: AgentVerificationProbe = {
    ...params,
    proofHash: ""
  };
  probe.proofHash = hashJson({ ...probe, proofHash: "" });
  return probe;
}

function verificationResult(probes: AgentVerificationProbe[]): AgentVerificationResult {
  const verification: AgentVerificationResult = {
    schema: "ingen.agent_verification.result.v1",
    passed: probes.every((probe) => probe.passed),
    probes,
    proofHash: ""
  };
  verification.proofHash = hashJson({ ...verification, proofHash: "" });
  return verification;
}

async function filesystemProbe(id: string, target: string, expectation: "exists" | "missing" | "directory" | "file" | "empty_directory"): Promise<AgentVerificationProbe> {
  try {
    const info = await stat(target);
    const entries = expectation === "empty_directory" && info.isDirectory() ? await readdir(target) : [];
    const passed =
      expectation === "exists" ||
      (expectation === "directory" && info.isDirectory()) ||
      (expectation === "file" && info.isFile()) ||
      (expectation === "empty_directory" && info.isDirectory() && entries.length === 0);
    const actual = info.isDirectory()
      ? `directory entries=${entries.length}`
      : info.isFile()
        ? `file bytes=${info.size}`
        : "other";
    return verificationProbe({
      id,
      kind: "filesystem",
      target,
      expectation,
      actual,
      passed
    });
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? String((error as { code?: unknown }).code) : "";
    const missing = code === "ENOENT";
    return verificationProbe({
      id,
      kind: "filesystem",
      target,
      expectation,
      actual: missing ? "missing" : error instanceof Error ? error.message : String(error),
      passed: expectation === "missing" && missing
    });
  }
}

function commandExitVerification(execution: { commandLine: string; accepted: boolean; exitCode: number | null; timedOut: boolean }): AgentVerificationResult {
  return verificationResult([
    verificationProbe({
      id: "command.exit",
      kind: "command_exit",
      target: execution.commandLine,
      expectation: "exit_code=0 and timed_out=false",
      actual: `exit_code=${execution.exitCode ?? "unknown"} timed_out=${execution.timedOut}`,
      passed: execution.accepted && execution.exitCode === 0 && !execution.timedOut
    })
  ]);
}

function commandExitProbe(execution: { commandLine: string; accepted: boolean; exitCode: number | null; timedOut: boolean }): AgentVerificationProbe {
  return commandExitVerification(execution).probes[0]!;
}

type GitExecution = {
  accepted: boolean;
  commandLine: string;
  exitCode: number | null;
  durationMs: number;
  stdout: string;
  stderr: string;
  timedOut: boolean;
  error?: IpcError;
};

function executeGit(config: AgentActionHostConfig, args: string[], timeoutMs = 15_000): GitExecution {
  const startedAt = Date.now();
  const child = spawnSync("git", args, {
    cwd: config.workspaceRoot,
    encoding: "utf8",
    stdio: "pipe",
    timeout: timeoutMs,
    windowsHide: true
  });
  const durationMs = Math.max(0, Date.now() - startedAt);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const timedOut = commandTimedOut(child.error);
  const accepted = !child.error && child.status === 0;
  const commandLine = renderCommandLine("git", args);
  return {
    accepted,
    commandLine,
    exitCode,
    durationMs,
    stdout,
    stderr,
    timedOut,
    error: accepted
      ? undefined
      : actionError(
          "rust_unavailable",
          timedOut ? `Git command timed out after ${timeoutMs}ms.` : child.error?.message ?? `Git exited with status ${exitCode ?? "unknown"}.`,
          { args, stderr, exitCode, timedOut }
        )
  };
}

function executeGh(config: AgentActionHostConfig, args: string[], timeoutMs = 30_000): GitExecution {
  const startedAt = Date.now();
  const child = spawnSync("gh", args, {
    cwd: config.workspaceRoot,
    encoding: "utf8",
    stdio: "pipe",
    timeout: timeoutMs,
    windowsHide: true
  });
  const durationMs = Math.max(0, Date.now() - startedAt);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const timedOut = commandTimedOut(child.error);
  const accepted = !child.error && child.status === 0;
  const commandLine = renderCommandLine("gh", args);
  return {
    accepted,
    commandLine,
    exitCode,
    durationMs,
    stdout,
    stderr,
    timedOut,
    error: accepted
      ? undefined
      : actionError(
          "rust_unavailable",
          timedOut ? `GitHub CLI command timed out after ${timeoutMs}ms.` : child.error?.message ?? `GitHub CLI exited with status ${exitCode ?? "unknown"}.`,
          { args, stderr, exitCode, timedOut }
        )
  };
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

function detectToolPath(config: AgentActionHostConfig, command: string): string | undefined {
  const detector = config.platform === "win32" ? "where.exe" : "sh";
  const args = config.platform === "win32" ? [command] : ["-lc", `command -v ${command.replace(/'/g, "'\\''")}`];
  const detected = spawnSync(detector, args, {
    encoding: "utf8",
    timeout: AGENT_ACTION_TOOL_DETECTION_TIMEOUT_MS,
    windowsHide: true
  });
  if (detected.status !== 0 || detected.error) {
    return undefined;
  }
  const firstLine = (detected.stdout ?? "").split(/\r?\n/).map((line) => line.trim()).find(Boolean);
  return firstLine;
}

export function detectAgentActionInstalledTools(config: AgentActionHostConfig): AgentActionInstalledTool[] {
  if (agentActionToolDetectionCache?.platform === config.platform) {
    return agentActionToolDetectionCache.tools;
  }
  const tools = AGENT_ACTION_TOOL_DETECTION_COMMANDS.map(({ id, command }) => {
    const detectedPath = detectToolPath(config, command);
    return {
      id,
      command,
      available: Boolean(detectedPath),
      detectedPath
    };
  });
  agentActionToolDetectionCache = { platform: config.platform, tools };
  return tools;
}

export function createWindowsExecutionPolicy(_config: AgentActionHostConfig): AgentWindowsExecutionPolicy {
  const policy: AgentWindowsExecutionPolicy = {
    schema: "ingen.windows_execution.policy.v1",
    adapters: WINDOWS_EXECUTION_ADAPTERS,
    routeCatalog: WINDOWS_ROUTE_CATALOG,
    defaultTimeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
    maxTimeoutMs: MAX_COMMAND_TIMEOUT_MS,
    stdoutPreviewBytes: MAX_PREVIEW_CHARS,
    stderrPreviewBytes: MAX_PREVIEW_CHARS,
    confirmationPolicy: "computer_writes_and_shell_full_require_confirmed_true",
    cancellationPolicy: "timeout_kills_child_and_reports_timed_out",
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
}

export function createAgentVerificationPolicy(_config: AgentActionHostConfig): AgentVerificationPolicy {
  const policy: AgentVerificationPolicy = {
    schema: "ingen.agent_verification.policy.v1",
    probeKinds: [
      "filesystem",
      "command_exit",
      "process_state",
      "service_state",
      "registry_state",
      "package_state",
      "browser_state",
      "ui_state",
      "event_log",
      "artifact_hash",
      "mcp_result",
      "manual_confirmation"
    ],
    retryStrategies: AGENT_RETRY_STRATEGIES,
    failureCategories: AGENT_FAILURE_CATEGORIES,
    mutationCompletionRule: "verified_or_blocked",
    protectedBoundaryRule: "block_without_retry",
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
}

export function createComputerUsePolicy(_config: AgentActionHostConfig): AgentComputerUsePolicy {
  const policy: AgentComputerUsePolicy = {
    schema: "ingen.computer_use.policy.v1",
    executableActions: [
      "computer_inspect",
      "computer_appshot",
      "computer_focus_window",
      "computer_clipboard_read",
      "computer_clipboard_write",
      "computer_ui_tree",
      "computer_ocr",
      "computer_click",
      "computer_type_text",
      "computer_scroll",
      "computer_drag"
    ],
    inspectionRequiresConfirmation: false,
    interactionRequiresConfirmation: true,
    userPresenceMode: "foreground_required_for_risky_gui_actions",
    pacingPolicy: "single_action_then_verify",
    forbiddenPrompts: ["security", "payment", "credential", "destructive", "uac", "password", "pin", "passkey", "credit card", "checkout"],
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
}

export function createBrowserWebPolicy(_config: AgentActionHostConfig): AgentBrowserWebPolicy {
  const policy: AgentBrowserWebPolicy = {
    schema: "ingen.browser_web.policy.v1",
    executableActions: [
      "browser_inspect_url",
      "browser_download",
      "browser_open_url",
      "browser_playwright_inspect",
      "browser_screenshot",
      "browser_click",
      "browser_type_text",
      "browser_playwright_download"
    ],
    inspectionRequiresConfirmation: false,
    navigationRequiresConfirmation: true,
    downloadRequiresConfirmation: true,
    submissionRequiresConfirmation: true,
    credentialPromptPolicy: "never_fill_or_submit_without_user",
    artifactPolicy: "persist_downloads_with_size_and_sha256",
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
}

export function createDocumentMediaPolicy(_config: AgentActionHostConfig): AgentDocumentMediaPolicy {
  const policy: AgentDocumentMediaPolicy = {
    schema: "ingen.document_media.policy.v1",
    executableActions: ["document_inspect", "document_write_text", "document_write_json", "document_write_csv", "document_convert_text"],
    workspaceWritesRequireConfirmation: false,
    computerScopeWritesRequireConfirmation: true,
    officeComRequiresConfirmation: true,
    macroPolicy: "blocked_without_explicit_user_approval",
    artifactPolicy: "verify_readback_size_hash_and_parser_status",
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
}

export function createDeveloperAutomationPolicy(_config: AgentActionHostConfig): AgentDeveloperAutomationPolicy {
  const policy: AgentDeveloperAutomationPolicy = {
    schema: "ingen.developer_automation.policy.v1",
    executableActions: [
      "dev_repo_status",
      "dev_git_diff",
      "dev_git_commit",
      "dev_git_push",
      "dev_github_pr_create",
      "dev_run_check",
      "virtualization_inspect",
      "virtualization_run_command",
      "automation_schedule",
      "automation_list",
      "automation_cancel",
      "automation_record"
    ],
    repoInspectionRequiresConfirmation: false,
    commandChecksRequireConfirmation: true,
    gitMutationRequiresConfirmation: true,
    cloudWritesRequireConfirmation: true,
    mcpToolCallingStatus: "planned_connector_required",
    automationPersistenceRequiresConfirmation: true,
    artifactPolicy: "verify_command_exit_git_state_or_ledger_hash",
    proofHash: ""
  };
  policy.proofHash = hashJson({ ...policy, proofHash: "" });
  return policy;
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
    }),
    actionCapability({
      id: "computer.inspect",
      family: "computer.gui",
      surface: "computer.gui",
      title: "Inspect GUI state",
      status: "available",
      risk: "read",
      operations: ["list displays", "list visible windows", "summarize GUI affordances"],
      underlyingTools: ["PowerShell Get-Process", "System.Windows.Forms.Screen", "UI Automation planned"],
      fallbacks: ["shell.full Get-Process", "manual screenshot"],
      verification: ["process_state", "ui_state"],
      approval: "none",
      executableActionIds: ["computer.inspect"],
      requiresApproval: false,
      writes: false,
      description: "Inspect foreground computer state with bounded display and window summaries.",
      notes: "Does not click or type; use before any GUI interaction."
    }),
    actionCapability({
      id: "computer.appshot",
      family: "computer.gui",
      surface: "computer.gui",
      title: "Capture appshot",
      status: "available",
      risk: "external_ui",
      operations: ["capture primary screen image", "write screenshot artifact", "hash screenshot artifact"],
      underlyingTools: ["PowerShell System.Drawing.CopyFromScreen", "Electron desktopCapturer planned"],
      fallbacks: ["manual screenshot", "contained app screenshot"],
      verification: ["artifact_hash", "ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.appshot"],
      requiresApproval: true,
      writes: true,
      description: "Capture a confirmed screenshot artifact for visual verification.",
      notes: "Screen capture is privacy-sensitive and requires confirmed:true."
    }),
    actionCapability({
      id: "computer.focus_window",
      family: "computer.gui",
      surface: "computer.gui",
      title: "Focus window",
      status: "available",
      risk: "external_ui",
      operations: ["activate visible window", "bring app foreground"],
      underlyingTools: ["Microsoft.VisualBasic Interaction.AppActivate"],
      fallbacks: ["Alt-Tab via future computer-use", "manual focus"],
      verification: ["process_state", "ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.focus_window"],
      requiresApproval: true,
      writes: true,
      description: "Bring a matching visible window to the foreground after confirmation.",
      notes: "One foreground action at a time; verify after focus."
    }),
    actionCapability({
      id: "computer.clipboard",
      family: "computer.clipboard",
      surface: "computer.gui",
      title: "Read or write clipboard",
      status: "available",
      risk: "external_ui",
      operations: ["read clipboard text", "write clipboard text"],
      underlyingTools: ["PowerShell Get-Clipboard", "PowerShell Set-Clipboard"],
      fallbacks: ["manual clipboard action"],
      verification: ["ui_state", "manual_confirmation"],
      approval: "prompt",
      executableActionIds: ["computer.clipboard_read", "computer.clipboard_write"],
      requiresApproval: true,
      writes: true,
      description: "Read or replace clipboard text after explicit confirmation.",
      notes: "Clipboard may contain secrets; reads and writes require confirmed:true."
    }),
    actionCapability({
      id: "computer.ui_tree",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Inspect UI Automation tree",
      status: "available",
      risk: "read",
      operations: ["inspect bounded accessibility tree", "summarize controls", "capture foreground UI structure"],
      underlyingTools: ["Windows UIAutomationClient", "PowerShell UIAutomationTypes"],
      fallbacks: ["computer.inspect", "app-specific API", "manual screenshot"],
      verification: ["ui_state"],
      approval: "none",
      executableActionIds: ["computer.ui_tree"],
      requiresApproval: false,
      writes: false,
      description: "Read a bounded Windows UI Automation tree without clicking or typing.",
      notes: "Limited to a small depth/node budget so desktop inspection cannot become an unbounded scrape."
    }),
    actionCapability({
      id: "computer.ocr",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Run confirmed OCR",
      status: "available",
      risk: "external_ui",
      operations: ["OCR confirmed screenshot or image artifact", "return extracted text with proof"],
      underlyingTools: ["tesseract.exe when installed", "PowerShell screenshot capture"],
      fallbacks: ["computer.ui_tree", "manual user readout"],
      verification: ["command_exit", "artifact_hash", "ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.ocr"],
      requiresApproval: true,
      writes: true,
      description: "Run OCR only with confirmation and only when a local OCR engine is actually available.",
      notes: "If no OCR engine is detected, the action returns missing_tool instead of a fake success."
    }),
    actionCapability({
      id: "computer.click",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Click foreground UI",
      status: "available",
      risk: "external_ui",
      operations: ["single mouse click", "before and after foreground snapshots", "forbidden prompt detection"],
      underlyingTools: ["user32 mouse_event", "Windows Forms cursor"],
      fallbacks: ["UI Automation InvokePattern", "manual user click"],
      verification: ["ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.click"],
      requiresApproval: true,
      writes: true,
      description: "Perform one confirmed foreground click with strict pacing and post-action verification.",
      notes: "Blocks UAC, payment, credential and security prompts before injecting input."
    }),
    actionCapability({
      id: "computer.type_text",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Type foreground text",
      status: "available",
      risk: "external_ui",
      operations: ["single text typing action", "before and after foreground snapshots", "forbidden prompt detection"],
      underlyingTools: ["System.Windows.Forms.SendKeys"],
      fallbacks: ["clipboard paste with confirmation", "manual user typing"],
      verification: ["ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.type_text"],
      requiresApproval: true,
      writes: true,
      description: "Type confirmed text into the foreground app after blocking sensitive prompts.",
      notes: "Never type into password, payment, UAC or credential prompts."
    }),
    actionCapability({
      id: "computer.scroll",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Scroll foreground UI",
      status: "available",
      risk: "external_ui",
      operations: ["single mouse wheel scroll", "cursor and foreground verification"],
      underlyingTools: ["user32 mouse_event wheel"],
      fallbacks: ["UI Automation ScrollPattern", "manual user scroll"],
      verification: ["ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.scroll"],
      requiresApproval: true,
      writes: true,
      description: "Perform one confirmed foreground scroll and report the verified UI state.",
      notes: "Pacing stays one action per tool call."
    }),
    actionCapability({
      id: "computer.drag",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Drag foreground UI",
      status: "available",
      risk: "external_ui",
      operations: ["single bounded drag", "cursor movement verification", "before and after foreground snapshots"],
      underlyingTools: ["user32 mouse_event", "Windows Forms cursor"],
      fallbacks: ["UI Automation TransformPattern", "manual user drag"],
      verification: ["ui_state"],
      approval: "prompt",
      executableActionIds: ["computer.drag"],
      requiresApproval: true,
      writes: true,
      description: "Perform one confirmed drag-drop gesture with strict foreground safety checks.",
      notes: "Blocks sensitive prompts and refuses missing coordinates."
    }),
    actionCapability({
      id: "browser.inspect_url",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Inspect URL",
      status: "available",
      risk: "read",
      operations: ["fetch page metadata", "summarize title", "count links and forms", "detect download candidates"],
      underlyingTools: ["fetch", "HTTP headers", "HTML summary parser", "CDP planned"],
      fallbacks: ["browser.open_url", "shell.full curl", "GUI/computer_use"],
      verification: ["browser_state", "command_exit"],
      approval: "none",
      executableActionIds: ["browser.inspect_url"],
      requiresApproval: false,
      writes: false,
      description: "Inspect a URL through a bounded HTTP read and return compact page state.",
      notes: "Does not submit forms, fill credentials or click external pages."
    }),
    actionCapability({
      id: "browser.download",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Download URL artifact",
      status: "available",
      risk: "computer_write",
      operations: ["download file", "persist artifact", "compute size", "compute sha256"],
      underlyingTools: ["fetch", "node:fs/writeFile", "artifact hash", "Electron DownloadItem planned"],
      fallbacks: ["Playwright download", "browser.open_url", "shell.full curl"],
      verification: ["filesystem", "artifact_hash", "browser_state"],
      approval: "prompt",
      executableActionIds: ["browser.download"],
      requiresApproval: true,
      writes: true,
      description: "Download a URL to a workspace or confirmed computer path and verify the artifact hash.",
      notes: "Requires confirmed:true; executing downloaded files is a separate approval-gated task."
    }),
    actionCapability({
      id: "browser.open_url",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Open URL",
      status: "available",
      risk: "external_ui",
      operations: ["open URL with OS/browser", "hand off to foreground browser"],
      underlyingTools: ["PowerShell Start-Process", "msedge/chrome default browser", "contained WebExplorer planned"],
      fallbacks: ["manual open", "computer.focus_window", "CDP target activation planned"],
      verification: ["command_exit", "manual_confirmation"],
      approval: "prompt",
      executableActionIds: ["browser.open_url"],
      requiresApproval: true,
      writes: true,
      description: "Open a URL in the default browser after explicit confirmation.",
      notes: "External navigation can expose data or trigger account state; never submit forms without user approval."
    }),
    actionCapability({
      id: "browser.playwright_inspect",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Inspect page with Playwright",
      status: "available",
      risk: "read",
      operations: ["navigate isolated browser context", "inspect DOM counts", "capture ARIA snapshot", "record network request/response summaries"],
      underlyingTools: ["@playwright/test chromium", "Playwright page events", "Playwright ariaSnapshot"],
      fallbacks: ["browser.inspect_url", "shell.full curl", "computer.ui_tree"],
      verification: ["browser_state", "command_exit"],
      approval: "none",
      executableActionIds: ["browser.playwright_inspect"],
      requiresApproval: false,
      writes: false,
      description: "Open a URL in an isolated headless browser and return DOM plus bounded network evidence.",
      notes: "Uses a fresh browser context without profile credentials."
    }),
    actionCapability({
      id: "browser.screenshot",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Capture browser screenshot",
      status: "available",
      risk: "external_ui",
      operations: ["navigate isolated browser context", "capture full-page screenshot", "persist PNG artifact", "hash screenshot"],
      underlyingTools: ["@playwright/test chromium", "page.screenshot"],
      fallbacks: ["computer.appshot", "manual screenshot"],
      verification: ["browser_state", "artifact_hash", "filesystem"],
      approval: "prompt",
      executableActionIds: ["browser.screenshot"],
      requiresApproval: true,
      writes: true,
      description: "Capture a confirmed Playwright screenshot artifact and verify its hash.",
      notes: "Writes a local artifact, so confirmed:true is required."
    }),
    actionCapability({
      id: "browser.click",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Click page element",
      status: "available",
      risk: "external_ui",
      operations: ["navigate isolated browser context", "click locator", "block form submission without explicit confirmation", "verify post-click page state"],
      underlyingTools: ["@playwright/test chromium", "locator.click"],
      fallbacks: ["computer.click", "manual browser action"],
      verification: ["browser_state"],
      approval: "prompt",
      executableActionIds: ["browser.click"],
      requiresApproval: true,
      writes: true,
      description: "Perform one confirmed locator click in an isolated Playwright page and verify the resulting state.",
      notes: "Submit buttons and form-associated clicks require formSubmissionConfirmed:true."
    }),
    actionCapability({
      id: "browser.type_text",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Type into page element",
      status: "available",
      risk: "external_ui",
      operations: ["navigate isolated browser context", "fill locator text", "block credential and payment fields", "verify selector value"],
      underlyingTools: ["@playwright/test chromium", "locator.fill"],
      fallbacks: ["computer.type_text", "manual browser action"],
      verification: ["browser_state"],
      approval: "prompt",
      executableActionIds: ["browser.type_text"],
      requiresApproval: true,
      writes: true,
      description: "Type confirmed text into a non-sensitive browser field in an isolated context.",
      notes: "Password, one-time-code and payment fields are blocked."
    }),
    actionCapability({
      id: "browser.playwright_download",
      family: "browser.cdp",
      surface: "browser.web",
      title: "Download via page click",
      status: "available",
      risk: "computer_write",
      operations: ["navigate isolated browser context", "wait for download event", "click locator", "save artifact", "hash downloaded file"],
      underlyingTools: ["@playwright/test chromium", "page.waitForEvent('download')", "download.saveAs"],
      fallbacks: ["browser.download", "shell.full curl"],
      verification: ["browser_state", "filesystem", "artifact_hash"],
      approval: "prompt",
      executableActionIds: ["browser.playwright_download"],
      requiresApproval: true,
      writes: true,
      description: "Trigger a confirmed page download through Playwright and verify the persisted artifact.",
      notes: "If no download event is observed, this returns blocked/unverifiable rather than success."
    }),
    actionCapability({
      id: "document.inspect",
      family: "documents.media",
      surface: "documents.media",
      title: "Inspect document or media artifact",
      status: "available",
      risk: "read",
      operations: ["read file metadata", "detect common document/media type", "compute sha256", "parse text/json/csv/markdown summaries"],
      underlyingTools: ["node:fs/readFile", "node:crypto/createHash", "JSON parser", "RFC 4180 style CSV summary"],
      fallbacks: ["shell.readonly file inspection", "shell.full external metadata tool when confirmed"],
      verification: ["filesystem", "artifact_hash"],
      approval: "none",
      executableActionIds: ["document.inspect"],
      requiresApproval: false,
      writes: false,
      description: "Inspect a document, data file or media artifact and return a compact verified summary.",
      notes: "Rich Office/PDF/media parsing is planned; v1 always returns size and sha256."
    }),
    actionCapability({
      id: "document.write_text",
      family: "documents.media",
      surface: "documents.media",
      title: "Write text or Markdown",
      status: "available",
      risk: "workspace_write",
      operations: ["write text", "write markdown", "read back content", "hash artifact"],
      underlyingTools: ["node:fs/writeFile", "node:fs/readFile", "node:crypto/createHash"],
      fallbacks: ["fs.copy from generated artifact", "shell.full Set-Content when confirmed"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["document.write_text"],
      requiresApproval: false,
      writes: true,
      description: "Create or replace a UTF-8 text/Markdown file and verify by readback, size and hash.",
      notes: "Computer-scope writes require confirmed:true."
    }),
    actionCapability({
      id: "document.write_json",
      family: "documents.media",
      surface: "documents.media",
      title: "Write JSON",
      status: "available",
      risk: "workspace_write",
      operations: ["validate JSON", "pretty-print JSON", "write file", "verify parser readback"],
      underlyingTools: ["JSON.parse", "JSON.stringify", "node:fs/writeFile", "node:crypto/createHash"],
      fallbacks: ["document.write_text after user approval", "shell.full node script when confirmed"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["document.write_json"],
      requiresApproval: false,
      writes: true,
      description: "Validate, pretty-print and write JSON, then verify by parser readback and hash.",
      notes: "Invalid JSON is rejected rather than written."
    }),
    actionCapability({
      id: "document.write_csv",
      family: "documents.media",
      surface: "documents.media",
      title: "Write CSV",
      status: "available",
      risk: "workspace_write",
      operations: ["validate CSV shape", "write CSV text", "summarize rows and columns", "hash artifact"],
      underlyingTools: ["RFC 4180 style CSV parser", "node:fs/writeFile", "node:crypto/createHash"],
      fallbacks: ["document.write_text with explicit user intent", "shell.full converter when confirmed"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["document.write_csv"],
      requiresApproval: false,
      writes: true,
      description: "Write CSV content and verify row/column shape plus hash.",
      notes: "This is a bounded CSV validator, not a full spreadsheet engine."
    }),
    actionCapability({
      id: "document.convert_text",
      family: "documents.media",
      surface: "documents.media",
      title: "Convert text and Markdown",
      status: "available",
      risk: "workspace_write",
      operations: ["convert Markdown to plain text", "copy plain text to Markdown-safe artifact", "verify output hash"],
      underlyingTools: ["bounded Markdown stripper", "node:fs/readFile", "node:fs/writeFile"],
      fallbacks: ["external document converter through confirmed shell.full"],
      verification: ["filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["document.convert_text"],
      requiresApproval: false,
      writes: true,
      description: "Perform a safe local text/Markdown conversion between two paths and verify the output artifact.",
      notes: "PDF, Office, image, audio and video conversion remain planned backends."
    }),
    actionCapability({
      id: "dev.repo_status",
      family: "dev.git",
      surface: "developer.git",
      title: "Inspect repository status",
      status: "available",
      risk: "read",
      operations: ["git rev-parse", "git branch", "git status porcelain", "summarize dirty state"],
      underlyingTools: ["git status --porcelain", "git rev-parse", "git branch"],
      fallbacks: ["shell.readonly git status", "filesystem.discovery"],
      verification: ["command_exit", "filesystem"],
      approval: "none",
      executableActionIds: ["dev.repo_status"],
      requiresApproval: false,
      writes: false,
      description: "Inspect Git repository state and return compact dirty/staged/untracked counts.",
      notes: "Use before code edits, commits or pushes to preserve unrelated changes."
    }),
    actionCapability({
      id: "dev.git_diff",
      family: "dev.git",
      surface: "developer.git",
      title: "Inspect repository diff",
      status: "available",
      risk: "read",
      operations: ["git diff --stat", "git diff --name-status", "summarize changed files"],
      underlyingTools: ["git diff", "git diff --stat", "git diff --name-status"],
      fallbacks: ["shell.readonly git diff"],
      verification: ["command_exit"],
      approval: "none",
      executableActionIds: ["dev.git_diff"],
      requiresApproval: false,
      writes: false,
      description: "Inspect current Git diff without staging or changing files.",
      notes: "Use this to summarize work before commit or review."
    }),
    actionCapability({
      id: "dev.git_commit",
      family: "dev.git",
      surface: "developer.git",
      title: "Create confirmed Git commit",
      status: "available",
      risk: "workspace_write",
      operations: ["inspect status before commit", "stage explicit paths when provided", "create commit", "verify HEAD changed"],
      underlyingTools: ["git status --porcelain=v1", "git add -- <paths>", "git commit -m", "git rev-parse HEAD"],
      fallbacks: ["manual staging", "shell.full confirmed git commit"],
      verification: ["command_exit", "filesystem", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["dev.git_commit"],
      requiresApproval: true,
      writes: true,
      description: "Create a confirmed Git commit from explicit paths or already-staged changes, then verify the new HEAD hash.",
      notes: "This never stages the whole repository implicitly; pass paths for newly selected files or stage beforehand."
    }),
    actionCapability({
      id: "dev.git_push",
      family: "dev.git",
      surface: "developer.git",
      title: "Push confirmed Git branch",
      status: "available",
      risk: "external_ui",
      operations: ["inspect local branch", "push branch to remote", "verify remote head"],
      underlyingTools: ["git status --porcelain=v1 -b", "git push", "git ls-remote --heads"],
      fallbacks: ["GitHub connector push", "shell.full confirmed git push"],
      verification: ["command_exit", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["dev.git_push"],
      requiresApproval: true,
      writes: true,
      description: "Push the current or requested branch to a remote and verify the remote head matches local HEAD.",
      notes: "Network write; requires confirmed:true and never extracts credentials."
    }),
    actionCapability({
      id: "dev.github_pr_create",
      family: "dev.git",
      surface: "developer.github",
      title: "Create confirmed GitHub pull request",
      status: "available",
      risk: "external_ui",
      operations: ["inspect GitHub CLI auth", "create PR non-interactively", "verify PR URL with gh pr view"],
      underlyingTools: ["gh auth status --active", "gh pr create", "gh pr view"],
      fallbacks: ["GitHub connector create_pull_request", "manual PR URL"],
      verification: ["command_exit", "artifact_hash"],
      approval: "confirmed",
      executableActionIds: ["dev.github_pr_create"],
      requiresApproval: true,
      writes: true,
      description: "Create a GitHub PR with explicit title/body/head/base inputs and verify the created URL.",
      notes: "MCP stays out of scope; this uses the GitHub CLI when available and authenticated."
    }),
    actionCapability({
      id: "dev.run_check",
      family: "dev.git",
      surface: "developer.command",
      title: "Run confirmed developer check",
      status: "available",
      risk: "workspace_write",
      operations: ["run tests", "run build", "run lint", "capture stdout/stderr", "verify exit code"],
      underlyingTools: ["npm", "node", "cargo wrapper", "git", "project scripts"],
      fallbacks: ["shell.full confirmed command", "CI logs", "manual approval"],
      verification: ["command_exit", "filesystem"],
      approval: "prompt",
      executableActionIds: ["dev.run_check"],
      requiresApproval: true,
      writes: true,
      description: "Run a developer verification command in the workspace after confirmed:true.",
      notes: "Checks may write caches or build outputs, so confirmation is required."
    }),
    actionCapability({
      id: "virtualization.inspect",
      family: "virtualization.wsl",
      surface: "virtualization",
      title: "Inspect WSL, Docker and Hyper-V",
      status: "available",
      risk: "read",
      operations: ["inspect WSL status and distributions", "inspect Docker version and containers", "inspect Hyper-V VMs"],
      underlyingTools: ["wsl.exe --status", "wsl.exe --list --verbose", "docker version", "docker ps --format json", "PowerShell Get-VM"],
      fallbacks: ["native Windows toolchain", "dev.run_check", "shell.readonly"],
      verification: ["command_exit", "process_state"],
      approval: "none",
      executableActionIds: ["virtualization.inspect"],
      requiresApproval: false,
      writes: false,
      description: "Inspect local virtualization backends and return compact runtime evidence without mutating distributions, containers or VMs.",
      notes: "Missing WSL/Docker/Hyper-V is reported as a verified backend state, not as success."
    }),
    actionCapability({
      id: "virtualization.run_command",
      family: "virtualization.wsl",
      surface: "virtualization",
      title: "Run confirmed WSL or Docker command",
      status: "available",
      risk: "computer_write",
      operations: ["run command through WSL", "run command in an existing Docker container", "capture exit code", "fallback to native dev command when backend is missing"],
      underlyingTools: ["wsl.exe --exec", "docker exec", "native command fallback"],
      fallbacks: ["dev.run_check", "shell.full confirmed native command"],
      verification: ["command_exit"],
      approval: "prompt",
      executableActionIds: ["virtualization.run_command"],
      requiresApproval: true,
      writes: true,
      description: "Execute a confirmed development command through WSL or Docker and verify by exit code; Hyper-V command execution remains blocked.",
      notes: "Distro/container lifecycle changes, Docker image pulls and Hyper-V VM commands are not direct actions in this step."
    }),
    actionCapability({
      id: "automation.schedule",
      family: "windows.scheduler",
      surface: "automation.task_scheduler",
      title: "Schedule confirmed Windows automation",
      status: "available",
      risk: "computer_write",
      operations: ["create Task Scheduler task", "verify scheduled task", "mirror audit ledger"],
      underlyingTools: ["schtasks /Create", "schtasks /Query", "workspace JSONL audit ledger"],
      fallbacks: ["automation.record", "shell.full confirmed schtasks"],
      verification: ["command_exit", "event_log", "artifact_hash"],
      approval: "prompt",
      executableActionIds: ["automation.schedule"],
      requiresApproval: true,
      writes: true,
      description: "Create an InGen-owned visible Windows scheduled task with the InGenAgent_ prefix and mirror the proof to the audit ledger.",
      notes: "Only InGenAgent_ root tasks are directly managed; arbitrary system task mutation remains outside this direct action."
    }),
    actionCapability({
      id: "automation.list",
      family: "windows.scheduler",
      surface: "automation.task_scheduler",
      title: "List InGen scheduled automations",
      status: "available",
      risk: "read",
      operations: ["query Task Scheduler", "summarize InGen-owned tasks", "read audit ledger"],
      underlyingTools: ["schtasks /Query /FO CSV /V", "workspace JSONL audit ledger"],
      fallbacks: ["automation.record ledger", "shell.readonly schtasks query"],
      verification: ["command_exit", "artifact_hash"],
      approval: "none",
      executableActionIds: ["automation.list"],
      requiresApproval: false,
      writes: false,
      description: "List visible InGen-owned scheduled tasks and include the audit ledger path/hash.",
      notes: "The action filters to InGenAgent_ root tasks instead of exposing unrelated system tasks."
    }),
    actionCapability({
      id: "automation.cancel",
      family: "windows.scheduler",
      surface: "automation.task_scheduler",
      title: "Cancel confirmed scheduled automation",
      status: "available",
      risk: "computer_write",
      operations: ["delete InGen Task Scheduler task", "verify deletion", "append audit ledger cancellation"],
      underlyingTools: ["schtasks /Delete", "schtasks /Query", "workspace JSONL audit ledger"],
      fallbacks: ["schtasks /Change /Disable through confirmed shell.full"],
      verification: ["command_exit", "event_log", "artifact_hash"],
      approval: "prompt",
      executableActionIds: ["automation.cancel"],
      requiresApproval: true,
      writes: true,
      description: "Delete an InGen-owned scheduled task after confirmation and append a cancellation audit record.",
      notes: "This refuses task names outside InGenAgent_ so the agent cannot silently alter system tasks."
    }),
    actionCapability({
      id: "automation.record",
      family: "automations.goals",
      surface: "automation.ledger",
      title: "Record automation goal",
      status: "available",
      risk: "workspace_write",
      operations: ["record resumable goal", "append automation ledger", "hash ledger entry"],
      underlyingTools: ["workspace JSONL ledger", "Task Scheduler audit mirror"],
      fallbacks: ["automation.schedule", "document.write_json", "manual reminder"],
      verification: ["filesystem", "artifact_hash"],
      approval: "prompt",
      executableActionIds: ["automation.record"],
      requiresApproval: true,
      writes: true,
      description: "Record a confirmed automation/background goal in a workspace ledger for visibility and cancellation handoff.",
      notes: "Use automation.schedule when a real OS-visible scheduled task is required."
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
      status: "available",
      risk: "computer_write",
      operations: ["query InGen tasks", "create InGen task", "delete InGen task", "mirror audit ledger"],
      underlyingTools: ["schtasks /Query", "schtasks /Create", "schtasks /Delete", "workspace JSONL audit ledger"],
      fallbacks: ["ScheduledTasks PowerShell module", "Task Scheduler GUI/computer_use", "confirmed shell.full"],
      verification: ["command_exit", "event_log"],
      approval: "prompt",
      writes: true,
      executableActionIds: ["automation.schedule", "automation.list", "automation.cancel"],
      notes: "Direct actions are limited to InGenAgent_ root tasks; arbitrary task changes still require a separate confirmed shell route."
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
      status: "available",
      risk: "external_ui",
      operations: ["inspect page", "click/type", "capture network", "download files", "run browser tests"],
      underlyingTools: ["Playwright", "Chrome DevTools Protocol", "Electron WebContents"],
      fallbacks: ["GUI/computer_use", "MCP browser tools"],
      verification: ["browser_state", "ui_state", "artifact_hash"],
      approval: "prompt",
      writes: true,
      executableActionIds: ["browser.playwright_inspect", "browser.screenshot", "browser.click", "browser.type_text", "browser.playwright_download"],
      notes: "Playwright direct actions run in an isolated context. External submissions, purchases and account changes require confirmation; MCP browser tools remain out of scope."
    }),
    atlasEntry({
      id: "computer.ui_automation",
      family: "computer.ui_automation",
      surface: "desktop_gui",
      title: "Control desktop apps through accessibility and input",
      status: "available",
      risk: "external_ui",
      operations: ["screenshot", "OCR", "click", "type", "drag", "inspect accessibility tree"],
      underlyingTools: ["UI Automation", "user32 mouse_event", "System.Windows.Forms.SendKeys", "tesseract.exe when installed"],
      fallbacks: ["PowerShell/CLI", "app-specific COM/API"],
      verification: ["ui_state", "manual_confirmation"],
      approval: "prompt",
      writes: true,
      executableActionIds: ["computer.ui_tree", "computer.ocr", "computer.click", "computer.type_text", "computer.scroll", "computer.drag"],
      notes: "UI tree inspection is read-only; OCR and input gestures are confirmation-gated and never pass UAC, payment or credential prompts."
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
      fallbacks: ["GitHub connector", "shell.full"],
      verification: ["command_exit", "filesystem", "mcp_result"],
      approval: "confirmed",
      writes: true,
      notes: "Git status, diff, commit, push and PR creation now have direct actions; broader CI/review flows remain planned."
    }),
    atlasEntry({
      id: "virtualization.wsl",
      family: "virtualization.wsl",
      surface: "virtualization",
      title: "Use WSL distributions",
      status: "available",
      risk: "computer_write",
      operations: ["list distros", "run Linux commands", "import/export distros", "install WSL"],
      underlyingTools: ["wsl.exe", "PowerShell"],
      fallbacks: ["native Windows toolchain", "Docker"],
      verification: ["command_exit", "filesystem", "process_state"],
      approval: "prompt",
      writes: true,
      executableActionIds: ["virtualization.inspect", "virtualization.run_command"],
      notes: "Inspection and confirmed command execution are direct actions; distro install/import/export and destructive lifecycle changes remain blocked or shell-confirmed."
    }),
    atlasEntry({
      id: "virtualization.hyperv_docker",
      family: "virtualization.hyperv_docker",
      surface: "virtualization",
      title: "Use containers and virtual machines",
      status: "available",
      risk: "computer_write",
      operations: ["inspect containers", "run containers", "manage Hyper-V VMs"],
      underlyingTools: ["docker", "PowerShell Hyper-V module", "wsl.exe"],
      fallbacks: ["local toolchain", "cloud runner"],
      verification: ["process_state", "command_exit"],
      approval: "prompt",
      writes: true,
      executableActionIds: ["virtualization.inspect", "virtualization.run_command"],
      notes: "Docker inspection and confirmed docker exec are direct actions. Hyper-V VM inventory is direct; VM lifecycle and in-guest command execution remain prompt-gated/planned."
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

function capabilityDetailLines(entry: AgentCapabilityAtlasEntry): string[] {
  return [
    `id=${entry.id}`,
    `family=${entry.family}`,
    `surface=${entry.surface}`,
    `status=${entry.status}`,
    `risk=${entry.risk}`,
    `approval=${entry.approval}`,
    `writes=${entry.writes}`,
    `operations=${entry.operations.join("|")}`,
    `tools=${entry.underlyingTools.join("|")}`,
    `fallbacks=${entry.fallbacks.join("|")}`,
    `verification=${entry.verification.join("|")}`,
    entry.executableActionIds?.length ? `executable_actions=${entry.executableActionIds.join("|")}` : "executable_actions=none",
    `notes=${entry.notes}`
  ];
}

export function agentActionCapabilityDetailManifest(config: AgentActionHostConfig, selector: string): string {
  const manifest = createAgentActionHostManifest(config);
  const normalizedSelector = selector.trim().toLowerCase();
  const capability = manifest.capabilityAtlas.find((entry) =>
    entry.id.toLowerCase() === normalizedSelector ||
    entry.family.toLowerCase() === normalizedSelector ||
    entry.executableActionIds?.some((id) => id.toLowerCase() === normalizedSelector)
  );
  return [
    "AGENT_ACTION_CAPABILITY_DETAIL v1",
    `manifest_hash=${manifest.runtime.manifestHash}`,
    `atlas_hash=${manifest.runtime.atlasHash}`,
    capability ? capabilityDetailLines(capability).join("\n") : `missing_selector=${selector}`,
    "rule=Use this detail as context only. Execute only real available AGENT_ACTION_JSON actions and wait for AGENT_ACTION_RESULT."
  ].join("\n");
}

function uniqueSortedFamilies(entries: AgentCapabilityAtlasEntry[]): string[] {
  return [...new Set(entries.map((entry) => entry.family))].sort();
}

export function createAgentActionRuntimeManifestSummary(config: AgentActionHostConfig): AgentActionRuntimeManifestSummary {
  const capabilities = createExecutableActionCapabilities();
  const capabilityAtlas = createAgentCapabilityAtlas(config);
  const installedTools = detectAgentActionInstalledTools(config);
  const windowsExecution = createWindowsExecutionPolicy(config);
  const verification = createAgentVerificationPolicy(config);
  const summary: AgentActionRuntimeManifestSummary = {
    schema: "ingen.agent_action_runtime_manifest.summary.v1",
    manifestHash: "",
    atlasHash: hashJson(capabilityAtlas),
    installedToolsHash: hashJson(installedTools),
    windowsExecutionHash: windowsExecution.proofHash,
    verificationHash: verification.proofHash,
    executableActionIds: capabilities.map((capability) => capability.id),
    availableFamilies: uniqueSortedFamilies(capabilityAtlas.filter((entry) => entry.status === "available")),
    plannedFamilies: uniqueSortedFamilies(capabilityAtlas.filter((entry) => entry.status === "planned")),
    blockedFamilies: uniqueSortedFamilies(capabilityAtlas.filter((entry) => entry.status === "blocked")),
    approvalGatedFamilies: uniqueSortedFamilies(capabilityAtlas.filter((entry) => entry.approval === "prompt" || entry.approval === "confirmed")),
    installedToolIds: installedTools.filter((tool) => tool.available).map((tool) => tool.id).sort(),
    missingToolIds: installedTools.filter((tool) => !tool.available).map((tool) => tool.id).sort(),
    windowsRouteIds: windowsExecution.routeCatalog.map((route) => route.id),
    promptTokenEstimate: {
      fullManifest: estimatePromptTokens(JSON.stringify(capabilityAtlas)),
      compactContinuation: estimatePromptTokens([
        "AGENT_ACTION_HOST_CONTINUATION v1",
        capabilities.map((capability) => capability.id).join(" "),
        uniqueSortedFamilies(capabilityAtlas.filter((entry) => entry.status !== "available")).join("|")
      ].join("\n")),
      selectedCapabilityDetail: Math.max(...capabilityAtlas.map((entry) => estimatePromptTokens(capabilityDetailLines(entry).join("\n"))))
    },
    injectionPolicy: "full_on_local_intent_compact_delta_on_continuation",
    promptBudget: "compact_by_default_detail_on_selected_capability",
    resultReinjectionPolicy: "compact_tool_result_is_ground_truth_each_round"
  };
  summary.manifestHash = hashJson({ ...summary, manifestHash: "" });
  return summary;
}

export function createAgentActionHostManifest(config: AgentActionHostConfig): AgentActionHostManifest {
  const capabilities = createExecutableActionCapabilities();
  const capabilityAtlas = createAgentCapabilityAtlas(config);
  const installedTools = detectAgentActionInstalledTools(config);
  const windowsExecution = createWindowsExecutionPolicy(config);
  const verification = createAgentVerificationPolicy(config);
  const computerUse = createComputerUsePolicy(config);
  const browserWeb = createBrowserWebPolicy(config);
  const documentMedia = createDocumentMediaPolicy(config);
  const developerAutomation = createDeveloperAutomationPolicy(config);
  const runtime = createAgentActionRuntimeManifestSummary(config);
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
    installedTools,
    windowsExecution,
    verification,
    computerUse,
    browserWeb,
    documentMedia,
    developerAutomation,
    runtime,
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
    `manifest_hash=${manifest.runtime.manifestHash}`,
    `atlas_hash=${manifest.runtime.atlasHash}`,
    `windows_execution_hash=${manifest.runtime.windowsExecutionHash}`,
    `verification_hash=${manifest.runtime.verificationHash}`,
    `injection_policy=${manifest.runtime.injectionPolicy}`,
    `prompt_budget=${manifest.runtime.promptBudget}`,
    `result_reinjection=${manifest.runtime.resultReinjectionPolicy}`,
    `token_estimate_full=${manifest.runtime.promptTokenEstimate.fullManifest}`,
    `token_estimate_compact=${manifest.runtime.promptTokenEstimate.compactContinuation}`,
    `token_estimate_selected_capability=${manifest.runtime.promptTokenEstimate.selectedCapabilityDetail}`,
    `installed_tools=${manifest.runtime.installedToolIds.join("|")}`,
    `missing_tools=${manifest.runtime.missingToolIds.join("|")}`,
    `windows_adapters=${manifest.windowsExecution.adapters.join("|")}`,
    `windows_routes=${manifest.runtime.windowsRouteIds.join("|")}`,
    `windows_timeout=default:${manifest.windowsExecution.defaultTimeoutMs} max:${manifest.windowsExecution.maxTimeoutMs} cancellation:${manifest.windowsExecution.cancellationPolicy}`,
    `verification_policy=${manifest.verification.mutationCompletionRule} protected_boundary=${manifest.verification.protectedBoundaryRule}`,
    `failure_categories=${manifest.verification.failureCategories.join("|")}`,
    `retry_strategies=${manifest.verification.retryStrategies.map((strategy) => strategy.id).join("|")}`,
    `computer_use=${manifest.computerUse.userPresenceMode} pacing=${manifest.computerUse.pacingPolicy} forbidden=${manifest.computerUse.forbiddenPrompts.join("|")}`,
    `browser_web=download:${manifest.browserWeb.downloadRequiresConfirmation ? "confirmed" : "open"} navigation:${manifest.browserWeb.navigationRequiresConfirmation ? "confirmed" : "open"} submission:${manifest.browserWeb.submissionRequiresConfirmation ? "confirmed" : "open"} artifact:${manifest.browserWeb.artifactPolicy}`,
    `document_media=workspace_writes:${manifest.documentMedia.workspaceWritesRequireConfirmation ? "confirmed" : "open"} computer_writes:${manifest.documentMedia.computerScopeWritesRequireConfirmation ? "confirmed" : "open"} office_com:${manifest.documentMedia.officeComRequiresConfirmation ? "confirmed" : "open"} macros:${manifest.documentMedia.macroPolicy} artifact:${manifest.documentMedia.artifactPolicy}`,
    `developer_automation=repo_inspect:${manifest.developerAutomation.repoInspectionRequiresConfirmation ? "confirmed" : "open"} checks:${manifest.developerAutomation.commandChecksRequireConfirmation ? "confirmed" : "open"} git_mutation:${manifest.developerAutomation.gitMutationRequiresConfirmation ? "confirmed" : "open"} cloud_writes:${manifest.developerAutomation.cloudWritesRequireConfirmation ? "confirmed" : "open"} mcp:${manifest.developerAutomation.mcpToolCallingStatus} automation:${manifest.developerAutomation.automationPersistenceRequiresConfirmation ? "confirmed" : "open"}`,
    "available=fs.list fs.search fs.create_directory fs.rename fs.move fs.copy fs.delete_empty_directory fs.delete_tree shell.readonly shell.full computer.inspect computer.appshot computer.focus_window computer.clipboard_read computer.clipboard_write computer.ui_tree computer.ocr computer.click computer.type_text computer.scroll computer.drag browser.inspect_url browser.download browser.open_url browser.playwright_inspect browser.screenshot browser.click browser.type_text browser.playwright_download document.inspect document.write_text document.write_json document.write_csv document.convert_text dev.repo_status dev.git_diff dev.git_commit dev.git_push dev.github_pr_create dev.run_check virtualization.inspect virtualization.run_command automation.schedule automation.list automation.cancel automation.record",
    compactCapabilityAtlasLine(manifest.capabilityAtlas),
    `planned_families=${manifest.runtime.plannedFamilies.join("|")}`,
    `blocked_families=${manifest.runtime.blockedFamilies.join("|")}`,
    `approval_gated_families=${manifest.runtime.approvalGatedFamilies.join("|")}`,
    "capability_policy=Use the atlas for reasoning, not as fake execution. Prefer structured app/API/CLI routes first, then confirmed shell.full, then GUI/computer-use only when the task cannot be completed through a safer route.",
    "capability_limits=Planned or blocked atlas entries are not direct AGENT_ACTION_JSON actions. Use available executable actions only, or explain the missing backend/approval boundary.",
    "windows_reach=Prefer typed adapters powershell|cmd|windows_command before shell_full. Route IDs include winget.package, registry.reg, scheduler.schtasks, network.netsh, deployment.dism, services.sc, processes.tasklist, processes.taskkill, files.robocopy, security.icacls, certificates.certutil, events.wevtutil, virtualization.wsl, shell.start_process, settings.ms_settings.",
    `events=${AGENT_ACTION_EVENT_HINTS.join(" ")}`,
    "loop_stream=When local action is needed, write one short progress paragraph first, then emit exactly one AGENT_ACTION_JSON line that starts with AGENT_ACTION_JSON at column 1. After the app returns AGENT_ACTION_RESULT, continue with another short paragraph plus another AGENT_ACTION_JSON if more work remains, or finish with a compact summary.",
    "retry=If AGENT_ACTION_RESULT reports failure, inspect the error and try a different safe route before declaring the task blocked.",
    "loop_style=Use varied, concrete progress notes. Do not start every step with 'Je vais'. Prefer forms like 'Le bureau contient...', 'Je regroupe maintenant...', 'Prochaine action logique...', 'Ce fichier va dans...'.",
    "action_request_format=AGENT_ACTION_JSON {\"action\":\"copy_path\",\"scope\":\"computer\",\"path\":\"C:\\\\from.txt\",\"toPath\":\"C:\\\\to.txt\",\"confirmed\":true}",
    "tool_truth=Never claim an action was executed unless you emitted AGENT_ACTION_JSON and received AGENT_ACTION_RESULT from the app. The app renders the matching event icon; do not fake event lines by themselves.",
    "planned=contained_webexplorer_dom persistent_browser_sessions browser_account_state_changes office.com pdf_rich_parse media_codecs mcp.tools_call cloud_cli_writes thread_wakeups hyperv_guest_command_execution semantic_screen_targeting bundled_ocr_engine",
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
    const verification = verificationResult([await filesystemProbe("directory.exists", resolved, "directory")]);
    return result(config, request, {
      accepted: true,
      path: pathLabel(config, request, resolved),
      value: "directory already exists",
      verification,
      observedChanges: ["filesystem:directory_exists"]
    });
  }
  const verification = verificationResult([await filesystemProbe("directory.created", resolved, "directory")]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, resolved),
    value: charDeltaValue(0, 0),
    verification,
    observedChanges: ["filesystem:directory_created"]
  });
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
  const observedChanges: string[] = [];
  try {
    await rename(from, to);
    observedChanges.push("route:filesystem.rename");
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? String((error as { code?: unknown }).code) : "";
    if (request.action !== "move_path" || code !== "EXDEV") {
      throw error;
    }
    await cp(from, to, { recursive: true, force: false, errorOnExist: true });
    await rm(from, { recursive: true, force: false });
    observedChanges.push("route:filesystem.copy_rm_fallback");
  }
  const verification = verificationResult([
    await filesystemProbe("move.destination_exists", to, "exists"),
    await filesystemProbe("move.source_missing", from, "missing")
  ]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: pathLabel(config, request, to),
    verification,
    observedChanges,
    retryRoutes: observedChanges.includes("route:filesystem.copy_rm_fallback") ? ["api_cli", "powershell", "cmd"] : undefined,
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
  const verification = verificationResult([
    await filesystemProbe("copy.source_exists", from, "exists"),
    await filesystemProbe("copy.destination_exists", to, fromInfo.isDirectory() ? "directory" : fromInfo.isFile() ? "file" : "exists")
  ]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: pathLabel(config, request, to),
    verification,
    observedChanges: ["filesystem:copy_completed"],
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
  const verification = verificationResult([await filesystemProbe("directory.deleted", resolved, "missing")]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, resolved),
    value: charDeltaValue(0, 0),
    verification,
    observedChanges: ["filesystem:empty_directory_deleted"]
  });
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
  const verification = verificationResult([await filesystemProbe("tree.deleted", resolved, "missing")]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, resolved),
    value: charDeltaValue(0, 0),
    verification,
    observedChanges: ["filesystem:tree_deleted"]
  });
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

function commandTimeout(request: AgentActionRequest): number {
  return Math.max(100, Math.min(MAX_COMMAND_TIMEOUT_MS, request.timeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS));
}

function commandBase(command: string): string {
  const trimmed = command.trim().replace(/^"+|"+$/g, "");
  if (trimmed.toLowerCase().startsWith("ms-settings:")) {
    return "ms-settings:";
  }
  return basename(trimmed).toLowerCase();
}

function quoteCommandArg(value: string): string {
  return /^[A-Za-z0-9_./:=+-]+$/.test(value) ? value : JSON.stringify(value);
}

function renderCommandLine(command: string, args: string[]): string {
  return [command, ...args].map(quoteCommandArg).join(" ");
}

function quoteSchedulerTaskRunPart(value: string): string {
  const trimmed = value.trim();
  if (/^[A-Za-z0-9_./:=+-]+$/.test(trimmed)) {
    return trimmed;
  }
  return `"${trimmed.replace(/"/g, '""')}"`;
}

function renderSchedulerTaskRun(command: string, args: string[]): string {
  const trimmedCommand = command.trim();
  const commandPart = args.length > 0 ? `"${trimmedCommand.replace(/"/g, '""')}"` : quoteSchedulerTaskRunPart(trimmedCommand);
  return [commandPart, ...args.map(quoteSchedulerTaskRunPart)].join(" ");
}

function routeForWindowsCommand(command: string): AgentWindowsRouteCatalogEntry {
  const base = commandBase(command);
  const route = WINDOWS_ROUTE_CATALOG.find((entry) =>
    entry.commands.some((candidate) => candidate === "*" || candidate.toLowerCase() === base || candidate.toLowerCase() === command.trim().toLowerCase())
  );
  return route ?? WINDOWS_ROUTE_CATALOG.find((entry) => entry.id === "shell.full")!;
}

function inferWindowsExecutionAdapter(request: AgentActionRequest, command: string): AgentWindowsExecutionAdapterId {
  if (request.executionAdapter) {
    return request.executionAdapter;
  }
  const base = commandBase(command);
  if (base === "powershell.exe" || base === "pwsh.exe") {
    return "powershell";
  }
  if (base === "cmd.exe") {
    return "cmd";
  }
  const route = routeForWindowsCommand(command);
  return route.id === "shell.full" ? "shell_full" : route.adapter;
}

function commandTimedOut(error: unknown): boolean {
  return Boolean(
    error &&
      typeof error === "object" &&
      "code" in error &&
      String((error as { code?: unknown }).code).toUpperCase() === "ETIMEDOUT"
  );
}

function commandObservedChanges(params: {
  accepted: boolean;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  durationMs: number;
  timedOut: boolean;
}): string[] {
  const changes = [
    `exit_code:${params.exitCode ?? "unknown"}`,
    `stdout_bytes:${Buffer.byteLength(params.stdout, "utf8")}`,
    `stderr_bytes:${Buffer.byteLength(params.stderr, "utf8")}`,
    `duration_ms:${params.durationMs}`
  ];
  if (params.timedOut) {
    changes.push("timed_out:true");
  }
  changes.push(params.accepted ? "command_status:completed" : "command_status:failed");
  return changes;
}

type WindowsCommandExecution = {
  accepted: boolean;
  commandLine: string;
  executionAdapter: AgentWindowsExecutionAdapterId;
  routeId: string;
  exitCode: number | null;
  durationMs: number;
  timeoutMs: number;
  timedOut: boolean;
  stdoutPreview: string;
  stderrPreview: string;
  artifacts: string[];
  observedChanges: string[];
  verification: AgentVerificationResult;
  error?: IpcError;
};

function powerShellEncodedCommand(script: string): string {
  return Buffer.from(`$ProgressPreference = 'SilentlyContinue'\n${script}`, "utf16le").toString("base64");
}

function powerShellString(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

function executePowerShellJson(script: string, timeoutMs = 10_000): { accepted: boolean; stdout: string; stderr: string; exitCode: number | null; timedOut: boolean; error?: IpcError } {
  const child = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", powerShellEncodedCommand(script)],
    {
      encoding: "utf8",
      stdio: "pipe",
      timeout: timeoutMs,
      windowsHide: true
    }
  );
  const timedOut = commandTimedOut(child.error);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const accepted = !child.error && child.status === 0;
  return {
    accepted,
    stdout,
    stderr,
    exitCode,
    timedOut,
    error: accepted
      ? undefined
      : actionError(
          "rust_unavailable",
          timedOut ? `PowerShell command timed out after ${timeoutMs}ms.` : child.error?.message ?? `PowerShell exited with status ${exitCode ?? "unknown"}.`,
          { stderr, exitCode, timedOut }
        )
  };
}

function parseJsonObject<T>(text: string): T {
  const trimmed = text.trim();
  const withoutClixml = trimmed.replace(/\r?\n#< CLIXML[\s\S]*$/u, "").trim();
  return JSON.parse(withoutClixml) as T;
}

function computerUseSnapshot(input: Omit<AgentComputerUseSnapshot, "schema" | "proofHash">): AgentComputerUseSnapshot {
  const snapshot: AgentComputerUseSnapshot = {
    schema: "ingen.computer_use.snapshot.v1",
    ...input,
    proofHash: ""
  };
  snapshot.proofHash = hashJson({ ...snapshot, proofHash: "" });
  return snapshot;
}

type ForegroundUiSnapshot = {
  pid?: number;
  processName?: string;
  title?: string;
  cursor?: { x: number; y: number };
  forbiddenPromptDetected?: boolean;
};

const FORBIDDEN_UI_PROMPT_PATTERN =
  /\b(uac|user account control|administrator|windows security|security warning|credential|credentials|password|pin|passkey|payment|credit card|checkout|purchase|buy now|delete permanently)\b/i;

function forbiddenPromptDetected(snapshot: ForegroundUiSnapshot): boolean {
  const haystack = `${snapshot.processName ?? ""} ${snapshot.title ?? ""}`;
  return FORBIDDEN_UI_PROMPT_PATTERN.test(haystack) || /^(consent|credentialui|secure system)$/i.test(snapshot.processName ?? "");
}

function foregroundUiProbe(snapshot: ForegroundUiSnapshot, id: string): AgentVerificationProbe {
  return verificationProbe({
    id,
    kind: "ui_state",
    target: snapshot.title || snapshot.processName || "foreground",
    expectation: "foreground window and cursor state captured",
    actual: `pid=${snapshot.pid ?? "unknown"} process=${snapshot.processName ?? "unknown"} title=${snapshot.title ?? ""} cursor=${snapshot.cursor ? `${snapshot.cursor.x},${snapshot.cursor.y}` : "unknown"} forbidden=${snapshot.forbiddenPromptDetected === true}`,
    passed: true
  });
}

function foregroundUiSnapshot(config: AgentActionHostConfig, timeoutMs = 5_000): { accepted: boolean; snapshot?: ForegroundUiSnapshot; error?: IpcError; exitCode?: number | null; timedOut?: boolean; stderr?: string } {
  if (config.platform !== "win32") {
    return {
      accepted: false,
      error: actionError("rust_unavailable", "Foreground UI inspection is available only on win32.", { platform: config.platform })
    };
  }
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class InGenForeground {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", SetLastError=true)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll", SetLastError=true)] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@
$hwnd = [InGenForeground]::GetForegroundWindow()
$pidValue = [uint32]0
[void][InGenForeground]::GetWindowThreadProcessId($hwnd, [ref]$pidValue)
$builder = [System.Text.StringBuilder]::new(512)
[void][InGenForeground]::GetWindowText($hwnd, $builder, $builder.Capacity)
$process = if ($pidValue -gt 0) { Get-Process -Id $pidValue -ErrorAction SilentlyContinue } else { $null }
$cursor = [System.Windows.Forms.Cursor]::Position
[pscustomobject]@{
  pid = [int]$pidValue
  processName = if ($process) { $process.ProcessName } else { "" }
  title = $builder.ToString()
  cursor = [pscustomobject]@{ x = $cursor.X; y = $cursor.Y }
} | ConvertTo-Json -Depth 5 -Compress
`;
  const execution = executePowerShellJson(script, timeoutMs);
  if (!execution.accepted) {
    return {
      accepted: false,
      error: execution.error,
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderr: execution.stderr
    };
  }
  const parsed = parseJsonObject<ForegroundUiSnapshot>(execution.stdout);
  parsed.forbiddenPromptDetected = forbiddenPromptDetected(parsed);
  return { accepted: true, snapshot: parsed, exitCode: execution.exitCode, timedOut: execution.timedOut };
}

function requireConfirmedForegroundInput(config: AgentActionHostConfig, request: AgentActionRequest): AgentActionResult | undefined {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Foreground UI interaction requires confirmed:true and user presence.", request)
    });
  }
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Foreground UI interaction is available only on win32.", request)
    });
  }
  return undefined;
}

function appshotArtifact(input: Omit<AgentAppshotArtifact, "schema" | "proofHash">): AgentAppshotArtifact {
  const artifact: AgentAppshotArtifact = {
    schema: "ingen.computer_use.appshot.v1",
    ...input,
    proofHash: ""
  };
  artifact.proofHash = hashJson({ ...artifact, proofHash: "" });
  return artifact;
}

function browserPageSummary(input: Omit<AgentBrowserPageSummary, "schema" | "proofHash">): AgentBrowserPageSummary {
  const summary: AgentBrowserPageSummary = {
    schema: "ingen.browser.page_summary.v1",
    ...input,
    proofHash: ""
  };
  summary.proofHash = hashJson({ ...summary, proofHash: "" });
  return summary;
}

function browserDownloadArtifact(input: Omit<AgentBrowserDownloadArtifact, "schema" | "proofHash">): AgentBrowserDownloadArtifact {
  const artifact: AgentBrowserDownloadArtifact = {
    schema: "ingen.browser.download_artifact.v1",
    ...input,
    proofHash: ""
  };
  artifact.proofHash = hashJson({ ...artifact, proofHash: "" });
  return artifact;
}

function browserScreenshotArtifact(input: Omit<AgentBrowserScreenshotArtifact, "schema" | "proofHash">): AgentBrowserScreenshotArtifact {
  const artifact: AgentBrowserScreenshotArtifact = {
    schema: "ingen.browser.screenshot_artifact.v1",
    ...input,
    proofHash: ""
  };
  artifact.proofHash = hashJson({ ...artifact, proofHash: "" });
  return artifact;
}

function executeWindowsCommand(config: AgentActionHostConfig, request: AgentActionRequest, readonly: boolean): WindowsCommandExecution {
  const command = request.command?.trim() ?? "";
  const args = request.args ?? [];
  const timeoutMs = readonly ? Math.min(commandTimeout(request), 15_000) : commandTimeout(request);
  const startedAt = Date.now();
  const child = spawnSync(command, args, {
    cwd: readonly || request.scope === "computer" ? config.cwd : config.workspaceRoot,
    encoding: "utf8",
    stdio: "pipe",
    timeout: timeoutMs,
    windowsHide: true
  });
  const durationMs = Math.max(0, Date.now() - startedAt);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const timedOut = commandTimedOut(child.error);
  const accepted = !child.error && child.status === 0;
  const executionAdapter = inferWindowsExecutionAdapter(request, command);
  const route = routeForWindowsCommand(command);
  const commandLine = renderCommandLine(command, args);
  const error = accepted
    ? undefined
    : actionError(
        "rust_unavailable",
        timedOut ? `Command timed out after ${timeoutMs}ms.` : child.error?.message ?? `Command exited with status ${exitCode ?? "unknown"}.`,
        { command, args, stderr, exitCode, timedOut, routeId: route.id, executionAdapter }
      );
  return {
    accepted,
    commandLine,
    executionAdapter,
    routeId: route.id === "shell.full" && executionAdapter !== "shell_full" ? `${executionAdapter}.inline` : route.id,
    exitCode,
    durationMs,
    timeoutMs,
    timedOut,
    stdoutPreview: stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: stderr.slice(0, MAX_PREVIEW_CHARS),
    artifacts: [],
    observedChanges: commandObservedChanges({ accepted, stdout, stderr, exitCode, durationMs, timedOut }),
    verification: commandExitVerification({ commandLine, accepted, exitCode, timedOut }),
    error
  };
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
  return result(config, request, executeWindowsCommand(config, request, true));
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
  return result(config, request, executeWindowsCommand(config, request, false));
}

async function computerInspectAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const maxResults = clampMaxResults(request.maxResults, 40);
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
  [pscustomobject]@{
    id = $_.DeviceName
    primary = $_.Primary
    x = $_.Bounds.X
    y = $_.Bounds.Y
    width = $_.Bounds.Width
    height = $_.Bounds.Height
    scaleFactor = 1
  }
}
$windows = Get-Process | Where-Object { $_.MainWindowTitle } | Select-Object -First ${maxResults} | ForEach-Object {
  [pscustomobject]@{
    pid = $_.Id
    processName = $_.ProcessName
    title = $_.MainWindowTitle
    focused = $false
  }
}
[pscustomobject]@{
  displays = @($screens)
  windows = @($windows)
} | ConvertTo-Json -Depth 5 -Compress
`;
  const execution = executePowerShellJson(script, 8_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.inspect>",
      executionAdapter: "powershell",
      routeId: "computer.inspect",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "command_error",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{ displays?: AgentComputerDisplaySummary[]; windows?: AgentComputerWindowSummary[] }>(execution.stdout);
  const snapshot = computerUseSnapshot({
    action: "inspect",
    displays: Array.isArray(parsed.displays) ? parsed.displays : [],
    windows: Array.isArray(parsed.windows) ? parsed.windows : [],
    accessibilityTreeStatus: "planned",
    ocrStatus: "planned"
  });
  const verification = verificationResult([
    verificationProbe({
      id: "computer.inspect.window_inventory",
      kind: "ui_state",
      expectation: "display and window inventory returned",
      actual: `displays=${snapshot.displays.length} windows=${snapshot.windows.length}`,
      passed: snapshot.displays.length > 0
    })
  ]);
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <computer.inspect>",
    executionAdapter: "powershell",
    routeId: "computer.inspect",
    exitCode: execution.exitCode,
    stdoutPreview: execution.stdout.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`displays:${snapshot.displays.length}`, `windows:${snapshot.windows.length}`],
    verification,
    computerUse: snapshot
  });
}

async function computerAppshotAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Screen capture requires confirmed:true because it may expose private information.", request)
    });
  }
  const artifactPath = resolveActionPath(
    config,
    { ...request, scope: request.scope ?? "workspace" },
    request.path ?? `.ingen-agent-artifacts/appshot-${Date.now()}.png`
  );
  if (typeof artifactPath !== "string") {
    return result(config, request, { accepted: false, error: artifactPath });
  }
  await mkdir(dirname(artifactPath), { recursive: true });
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$path = ${powerShellString(artifactPath)}
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
try {
  $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
  $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
} finally {
  $graphics.Dispose()
  $bitmap.Dispose()
}
$item = Get-Item -LiteralPath $path
[pscustomobject]@{ path = $item.FullName; width = $bounds.Width; height = $bounds.Height; bytes = $item.Length } | ConvertTo-Json -Compress
`;
  const execution = executePowerShellJson(script, 12_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.appshot>",
      executionAdapter: "powershell",
      routeId: "computer.appshot",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "command_error",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{ path: string; width: number; height: number; bytes: number }>(execution.stdout);
  const bytes = await readFile(parsed.path);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  const artifact = appshotArtifact({
    path: pathLabel(config, request, parsed.path),
    width: parsed.width,
    height: parsed.height,
    bytes: parsed.bytes,
    sha256
  });
  const verification = verificationResult([
    await filesystemProbe("appshot.artifact_exists", parsed.path, "file"),
    verificationProbe({
      id: "appshot.artifact_hash",
      kind: "artifact_hash",
      target: parsed.path,
      expectation: "sha256 computed",
      actual: sha256,
      passed: /^[a-f0-9]{64}$/.test(sha256)
    })
  ]);
  return result(config, request, {
    accepted: true,
    path: artifact.path,
    commandLine: "powershell.exe -EncodedCommand <computer.appshot>",
    executionAdapter: "powershell",
    routeId: "computer.appshot",
    exitCode: execution.exitCode,
    artifacts: [artifact.path],
    observedChanges: [`appshot:${artifact.width}x${artifact.height}`, `bytes:${artifact.bytes}`],
    verification,
    appshot: artifact,
    userPresenceRequired: true
  });
}

async function computerFocusWindowAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Window focus requires confirmed:true and foreground user presence.", request)
    });
  }
  const title = request.windowTitle?.trim() || request.query?.trim() || "";
  if (!title) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "windowTitle or query is required to focus a window.", request)
    });
  }
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Add-Type -AssemblyName Microsoft.VisualBasic
$needle = ${powerShellString(title)}
$process = Get-Process | Where-Object { $_.MainWindowTitle -and $_.MainWindowTitle -like "*$needle*" } | Select-Object -First 1
if (-not $process) { throw "No visible window matched '$needle'." }
$ok = [Microsoft.VisualBasic.Interaction]::AppActivate([int]$process.Id)
[pscustomobject]@{ pid = $process.Id; processName = $process.ProcessName; title = $process.MainWindowTitle; focused = [bool]$ok } | ConvertTo-Json -Compress
`;
  const execution = executePowerShellJson(script, 8_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.focus_window>",
      executionAdapter: "powershell",
      routeId: "computer.focus_window",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "unverifiable",
      error: execution.error
    });
  }
  const focused = parseJsonObject<AgentComputerWindowSummary>(execution.stdout);
  const verification = verificationResult([
    verificationProbe({
      id: "computer.focus_window.appactivate",
      kind: "ui_state",
      target: focused.title,
      expectation: "AppActivate returned true",
      actual: `focused=${focused.focused === true}`,
      passed: focused.focused === true
    })
  ]);
  const snapshot = computerUseSnapshot({
    action: "focus_window",
    displays: [],
    windows: [focused],
    accessibilityTreeStatus: "planned",
    ocrStatus: "planned"
  });
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <computer.focus_window>",
    executionAdapter: "powershell",
    routeId: "computer.focus_window",
    exitCode: execution.exitCode,
    observedChanges: [`focused_window:${focused.processName}:${focused.pid}`],
    verification,
    computerUse: snapshot,
    userPresenceRequired: true
  });
}

async function computerClipboardReadAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Clipboard read requires confirmed:true because clipboard may contain secrets.", request)
    });
  }
  const execution = executePowerShellJson("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); [pscustomobject]@{ text = (Get-Clipboard -Raw) } | ConvertTo-Json -Compress", 5_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.clipboard_read>",
      executionAdapter: "powershell",
      routeId: "computer.clipboard_read",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "unverifiable",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{ text?: string }>(execution.stdout);
  const text = parsed.text ?? "";
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <computer.clipboard_read>",
    executionAdapter: "powershell",
    routeId: "computer.clipboard_read",
    exitCode: execution.exitCode,
    stdoutPreview: text.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`clipboard_chars:${text.length}`],
    verification: verificationResult([
      verificationProbe({
        id: "computer.clipboard_read.text",
        kind: "ui_state",
        expectation: "clipboard text returned",
        actual: `chars=${text.length}`,
        passed: true
      })
    ]),
    userPresenceRequired: true
  });
}

async function computerClipboardWriteAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Clipboard write requires confirmed:true.", request)
    });
  }
  const text = request.text ?? "";
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$value = ${powerShellString(text)}
Set-Clipboard -Value $value
$actual = Get-Clipboard -Raw
[pscustomobject]@{ chars = $actual.Length; verified = ($actual -eq $value) } | ConvertTo-Json -Compress
`;
  const execution = executePowerShellJson(script, 5_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.clipboard_write>",
      executionAdapter: "powershell",
      routeId: "computer.clipboard_write",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "unverifiable",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{ chars: number; verified: boolean }>(execution.stdout);
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <computer.clipboard_write>",
    executionAdapter: "powershell",
    routeId: "computer.clipboard_write",
    exitCode: execution.exitCode,
    observedChanges: [`clipboard_written_chars:${parsed.chars}`],
    verification: verificationResult([
      verificationProbe({
        id: "computer.clipboard_write.verify",
        kind: "ui_state",
        expectation: "clipboard equals requested text",
        actual: `verified=${parsed.verified}`,
        passed: parsed.verified === true
      })
    ]),
    value: charDeltaValue(text.length, 0),
    userPresenceRequired: true
  });
}

async function computerUiTreeAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Windows UI Automation tree inspection is available only on win32.", request)
    });
  }
  const maxNodes = clampMaxResults(request.maxResults, 80);
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class InGenUiTreeForeground {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", SetLastError=true)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll", SetLastError=true)] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@
$maxNodes = ${maxNodes}
$maxDepth = 3
$script:count = 0
function SafeValue([scriptblock]$block, $fallback = $null) {
  try { & $block } catch { $fallback }
}
function NodeFor($element, [int]$depth) {
  if ($null -eq $element -or $script:count -ge $maxNodes) { return $null }
  $script:count += 1
  $rect = SafeValue { $element.Current.BoundingRectangle } $null
  $children = @()
  if ($depth -lt $maxDepth) {
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $child = SafeValue { $walker.GetFirstChild($element) } $null
    while ($null -ne $child -and $script:count -lt $maxNodes) {
      $node = NodeFor $child ($depth + 1)
      if ($null -ne $node) { $children += $node }
      $child = SafeValue { $walker.GetNextSibling($child) } $null
    }
  }
  $node = [ordered]@{
    name = [string](SafeValue { $element.Current.Name } "")
    automationId = [string](SafeValue { $element.Current.AutomationId } "")
    controlType = [string]((SafeValue { $element.Current.ControlType.ProgrammaticName } "") -replace '^ControlType\\.', '')
    className = [string](SafeValue { $element.Current.ClassName } "")
    enabled = [bool](SafeValue { $element.Current.IsEnabled } $false)
    focused = [bool](SafeValue { $element.Current.HasKeyboardFocus } $false)
  }
  if ($null -ne $rect) {
    $node.boundingRectangle = [pscustomobject]@{ x = [int]$rect.X; y = [int]$rect.Y; width = [int]$rect.Width; height = [int]$rect.Height }
  }
  if ($children.Count -gt 0) { $node.children = @($children) }
  [pscustomobject]$node
}
$hwnd = [InGenUiTreeForeground]::GetForegroundWindow()
$pidValue = [uint32]0
[void][InGenUiTreeForeground]::GetWindowThreadProcessId($hwnd, [ref]$pidValue)
$builder = [System.Text.StringBuilder]::new(512)
[void][InGenUiTreeForeground]::GetWindowText($hwnd, $builder, $builder.Capacity)
$process = if ($pidValue -gt 0) { Get-Process -Id $pidValue -ErrorAction SilentlyContinue } else { $null }
$root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
if ($null -eq $root) { $root = [System.Windows.Automation.AutomationElement]::RootElement }
$tree = NodeFor $root 0
[pscustomobject]@{
  window = [pscustomobject]@{
    pid = [int]$pidValue
    processName = if ($process) { $process.ProcessName } else { "" }
    title = $builder.ToString()
    focused = $true
  }
  nodeCount = $script:count
  tree = @($tree)
} | ConvertTo-Json -Depth 12 -Compress
`;
  const execution = executePowerShellJson(script, 10_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.ui_tree>",
      executionAdapter: "powershell",
      routeId: "computer.ui_tree",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "missing_tool",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{
    window?: AgentComputerWindowSummary;
    nodeCount?: number;
    tree?: AgentUiAutomationNodeSummary[];
  }>(execution.stdout);
  const tree = Array.isArray(parsed.tree) ? parsed.tree : [];
  const snapshot = computerUseSnapshot({
    action: "ui_tree",
    displays: [],
    windows: parsed.window ? [parsed.window] : [],
    accessibilityTreeStatus: "available",
    ocrStatus: "planned",
    accessibilityTree: tree
  });
  const nodeCount = parsed.nodeCount ?? tree.length;
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <computer.ui_tree>",
    executionAdapter: "powershell",
    routeId: "computer.ui_tree",
    exitCode: execution.exitCode,
    stdoutPreview: execution.stdout.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`ui_tree_nodes:${nodeCount}`, `foreground:${parsed.window?.processName ?? "unknown"}`],
    verification: verificationResult([
      verificationProbe({
        id: "computer.ui_tree.nodes",
        kind: "ui_state",
        target: parsed.window?.title ?? "foreground",
        expectation: "bounded UI Automation tree returned",
        actual: `nodes=${nodeCount}`,
        passed: nodeCount > 0
      })
    ]),
    computerUse: snapshot
  });
}

async function computerOcrAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "OCR requires confirmed:true because screenshots/images may expose private information.", request)
    });
  }
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "OCR screenshot capture is available only on win32.", request)
    });
  }
  const tesseract = detectToolPath(config, "tesseract.exe");
  if (!tesseract) {
    return result(config, request, {
      accepted: false,
      commandLine: "where.exe tesseract.exe",
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "No local OCR engine was detected. Install tesseract.exe or provide a supported OCR backend.", request),
      verification: verificationResult([
        verificationProbe({
          id: "computer.ocr.engine",
          kind: "command_exit",
          target: "tesseract.exe",
          expectation: "OCR engine detected before claiming OCR success",
          actual: "missing",
          passed: false
        })
      ])
    });
  }
  const imagePath = resolveActionPath(
    config,
    { ...request, scope: request.scope ?? "workspace" },
    request.path ?? `.ingen-agent-artifacts/ocr-${Date.now()}.png`
  );
  if (typeof imagePath !== "string") {
    return result(config, request, { accepted: false, error: imagePath });
  }
  if (!request.path) {
    await mkdir(dirname(imagePath), { recursive: true });
    const captureScript = `
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$path = ${powerShellString(imagePath)}
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
try {
  $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
  $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
} finally {
  $graphics.Dispose()
  $bitmap.Dispose()
}
`;
    const captured = executePowerShellJson(captureScript, 12_000);
    if (!captured.accepted) {
      return result(config, request, {
        accepted: false,
        commandLine: "powershell.exe -EncodedCommand <computer.ocr.capture>",
        executionAdapter: "powershell",
        routeId: "computer.ocr",
        exitCode: captured.exitCode,
        timedOut: captured.timedOut,
        stderrPreview: captured.stderr.slice(0, MAX_PREVIEW_CHARS),
        failureCategory: captured.timedOut ? "timeout" : "command_error",
        error: captured.error
      });
    }
  }
  const imageBytes = await readFile(imagePath);
  const imageHash = createHash("sha256").update(imageBytes).digest("hex");
  const startedAt = Date.now();
  const ocr = spawnSync(tesseract, [imagePath, "stdout"], {
    cwd: config.cwd,
    encoding: "utf8",
    stdio: "pipe",
    timeout: Math.min(commandTimeout(request), 60_000),
    windowsHide: true
  });
  const timedOut = commandTimedOut(ocr.error);
  const accepted = !ocr.error && ocr.status === 0;
  const text = ocr.stdout ?? "";
  const commandLine = renderCommandLine(tesseract, [imagePath, "stdout"]);
  const snapshot = computerUseSnapshot({
    action: "ocr",
    displays: [],
    windows: [],
    accessibilityTreeStatus: "planned",
    ocrStatus: accepted ? "available" : "blocked",
    ocrText: text.slice(0, MAX_PREVIEW_CHARS)
  });
  return result(config, request, {
    accepted,
    path: pathLabel(config, request, imagePath),
    commandLine,
    routeId: "computer.ocr",
    exitCode: ocr.status ?? null,
    durationMs: Math.max(0, Date.now() - startedAt),
    timedOut,
    stdoutPreview: text.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: (ocr.stderr ?? "").slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`ocr_chars:${text.length}`, `image_sha256:${imageHash}`],
    verification: verificationResult([
      await filesystemProbe("computer.ocr.image", imagePath, "file"),
      verificationProbe({
        id: "computer.ocr.engine_exit",
        kind: "command_exit",
        target: commandLine,
        expectation: "OCR engine exit_code=0 and timed_out=false",
        actual: `exit_code=${ocr.status ?? "unknown"} timed_out=${timedOut}`,
        passed: accepted && !timedOut
      }),
      verificationProbe({
        id: "computer.ocr.image_hash",
        kind: "artifact_hash",
        target: imagePath,
        expectation: "image sha256 computed",
        actual: imageHash,
        passed: /^[a-f0-9]{64}$/.test(imageHash)
      })
    ]),
    computerUse: snapshot,
    userPresenceRequired: true,
    error: accepted ? undefined : actionError("rust_unavailable", ocr.error?.message ?? `OCR exited with status ${ocr.status ?? "unknown"}.`, { stderr: ocr.stderr, timedOut })
  });
}

function sendKeysLiteral(value: string): string {
  return value.replace(/[+^%~()[\]{}]/g, (char) => `{${char}}`);
}

async function computerInputAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const confirmationError = requireConfirmedForegroundInput(config, request);
  if (confirmationError) {
    return confirmationError;
  }
  const before = foregroundUiSnapshot(config);
  if (!before.accepted || !before.snapshot) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <computer.input.before>",
      executionAdapter: "powershell",
      routeId: request.action.replace("_", "."),
      exitCode: before.exitCode,
      timedOut: before.timedOut,
      stderrPreview: before.stderr?.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: before.timedOut ? "timeout" : "unverifiable",
      error: before.error
    });
  }
  if (before.snapshot.forbiddenPromptDetected) {
    const snapshot = computerUseSnapshot({
      action: request.action.replace("computer_", "") as AgentComputerUseSnapshot["action"],
      displays: [],
      windows: before.snapshot.pid
        ? [{ pid: before.snapshot.pid, processName: before.snapshot.processName ?? "", title: before.snapshot.title ?? "", focused: true }]
        : [],
      accessibilityTreeStatus: "blocked",
      ocrStatus: "blocked",
      cursor: before.snapshot.cursor,
      forbiddenPromptDetected: true
    });
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      computerUse: snapshot,
      verification: verificationResult([foregroundUiProbe(before.snapshot, "computer.input.forbidden_prompt")]),
      error: actionError("bad_payload", "Foreground UI interaction is blocked on security, credential, payment or UAC prompts.", before.snapshot)
    });
  }
  const button = request.button ?? "left";
  const x = request.x;
  const y = request.y;
  const toX = request.toX;
  const toY = request.toY;
  const deltaY = request.deltaY ?? -480;
  if ((request.action === "computer_click" || request.action === "computer_drag") && (!Number.isInteger(x) || !Number.isInteger(y))) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "x and y integer coordinates are required for click and drag.", request)
    });
  }
  if (request.action === "computer_drag" && (!Number.isInteger(toX) || !Number.isInteger(toY))) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "toX and toY integer coordinates are required for drag.", request)
    });
  }
  if (request.action === "computer_type_text" && !request.text) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "text is required for computer_type_text.", request)
    });
  }
  const routeId = request.action.replace("computer_", "computer.");
  const actionScript =
    request.action === "computer_type_text"
      ? `
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait(${powerShellString(sendKeysLiteral(request.text ?? ""))})
`
      : `
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class InGenMouseInput {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(int flags, int dx, int dy, int data, UIntPtr extraInfo);
}
"@
$leftDown = 0x0002; $leftUp = 0x0004; $rightDown = 0x0008; $rightUp = 0x0010; $middleDown = 0x0020; $middleUp = 0x0040; $wheel = 0x0800
$down = if (${powerShellString(button)} -eq 'right') { $rightDown } elseif (${powerShellString(button)} -eq 'middle') { $middleDown } else { $leftDown }
$up = if (${powerShellString(button)} -eq 'right') { $rightUp } elseif (${powerShellString(button)} -eq 'middle') { $middleUp } else { $leftUp }
${Number.isInteger(x) && Number.isInteger(y) ? `[void][InGenMouseInput]::SetCursorPos(${x}, ${y})` : ""}
Start-Sleep -Milliseconds 120
${request.action === "computer_scroll" ? `[InGenMouseInput]::mouse_event($wheel, 0, 0, ${deltaY}, [UIntPtr]::Zero)` : ""}
${request.action === "computer_click" ? "[InGenMouseInput]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80; [InGenMouseInput]::mouse_event($up, 0, 0, 0, [UIntPtr]::Zero)" : ""}
${request.action === "computer_drag" ? `[InGenMouseInput]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 120; [void][InGenMouseInput]::SetCursorPos(${toX}, ${toY}); Start-Sleep -Milliseconds 120; [InGenMouseInput]::mouse_event($up, 0, 0, 0, [UIntPtr]::Zero)` : ""}
`;
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
${actionScript}
Start-Sleep -Milliseconds 180
[pscustomobject]@{ ok = $true } | ConvertTo-Json -Compress
`;
  const execution = executePowerShellJson(script, 8_000);
  const after = foregroundUiSnapshot(config);
  const actionName = request.action.replace("computer_", "") as AgentComputerUseSnapshot["action"];
  const snapshot = computerUseSnapshot({
    action: actionName,
    displays: [],
    windows: after.snapshot?.pid
      ? [{ pid: after.snapshot.pid, processName: after.snapshot.processName ?? "", title: after.snapshot.title ?? "", focused: true }]
      : [],
    accessibilityTreeStatus: after.accepted ? "available" : "blocked",
    ocrStatus: "planned",
    cursor: after.snapshot?.cursor ?? before.snapshot.cursor,
    inputSummary: request.action === "computer_type_text" ? `typed_chars:${request.text?.length ?? 0}` : request.action,
    forbiddenPromptDetected: after.snapshot?.forbiddenPromptDetected
  });
  const verification = verificationResult([
    foregroundUiProbe(before.snapshot, "computer.input.before"),
    verificationProbe({
      id: "computer.input.action_exit",
      kind: "command_exit",
      target: `powershell.exe -EncodedCommand <${routeId}>`,
      expectation: "input command exit_code=0 and timed_out=false",
      actual: `exit_code=${execution.exitCode ?? "unknown"} timed_out=${execution.timedOut}`,
      passed: execution.accepted && execution.exitCode === 0 && !execution.timedOut
    }),
    after.snapshot
      ? foregroundUiProbe(after.snapshot, "computer.input.after")
      : verificationProbe({
          id: "computer.input.after",
          kind: "ui_state",
          expectation: "post-action foreground state captured",
          actual: after.error?.message ?? "missing",
          passed: false
        })
  ]);
  return result(config, request, {
    accepted: execution.accepted && after.accepted,
    commandLine: `powershell.exe -EncodedCommand <${routeId}>`,
    executionAdapter: "powershell",
    routeId,
    exitCode: execution.exitCode,
    timedOut: execution.timedOut,
    stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [
      `foreground_before:${before.snapshot.processName ?? "unknown"}`,
      `foreground_after:${after.snapshot?.processName ?? "unknown"}`,
      request.action === "computer_type_text" ? `typed_chars:${request.text?.length ?? 0}` : actionName
    ],
    verification,
    computerUse: snapshot,
    userPresenceRequired: true,
    failureCategory: execution.accepted && after.accepted ? undefined : "unverifiable",
    error: execution.accepted && after.accepted ? undefined : execution.error ?? after.error ?? actionError("rust_unavailable", "Foreground UI input could not be independently verified.", request)
  });
}

function parseAgentUrl(request: AgentActionRequest, requireHttp = true): URL | IpcError {
  const rawUrl = request.url?.trim() || request.query?.trim() || "";
  if (!rawUrl) {
    return actionError("bad_payload", "URL is required.", request);
  }
  try {
    const url = new URL(rawUrl);
    const allowed = requireHttp ? ["http:", "https:", "data:"] : ["http:", "https:"];
    if (!allowed.includes(url.protocol)) {
      return actionError("bad_payload", `Unsupported URL protocol: ${url.protocol}`, { url: rawUrl });
    }
    return url;
  } catch {
    return actionError("bad_payload", "URL is invalid.", { url: rawUrl });
  }
}

function isIpcError(value: URL | IpcError): value is IpcError {
  return "code" in value && "proofHash" in value;
}

function browserFetchTimeout(request: AgentActionRequest): number {
  return Math.max(500, Math.min(60_000, request.timeoutMs ?? 20_000));
}

async function fetchWithTimeout(url: URL, timeoutMs: number): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, {
      redirect: "follow",
      signal: controller.signal,
      headers: {
        "User-Agent": "InGen-AgentActionHost/1.0"
      }
    });
  } finally {
    clearTimeout(timer);
  }
}

function htmlTitle(text: string): string | undefined {
  const match = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(text);
  if (!match) {
    return undefined;
  }
  return match[1].replace(/\s+/g, " ").trim().slice(0, 240);
}

function countPattern(text: string, pattern: RegExp): number {
  return Array.from(text.matchAll(pattern)).length;
}

function suggestedDownloadFilename(url: URL, contentDisposition: string | null): string {
  const dispositionName = /filename\*?=(?:UTF-8''|")?([^";\r\n]+)/i.exec(contentDisposition ?? "")?.[1];
  const decodedDisposition = dispositionName ? decodeURIComponent(dispositionName.replace(/^"|"$/g, "")) : "";
  const urlName = basename(decodeURIComponent(url.pathname || "")).trim();
  const candidate = decodedDisposition || urlName || "download.bin";
  const sanitized = candidate.replace(/[<>:"/\\|?*\x00-\x1F]/g, "_").slice(0, 180);
  return sanitized || "download.bin";
}

async function pathExists(candidate: string): Promise<boolean> {
  try {
    await stat(candidate);
    return true;
  } catch {
    return false;
  }
}

type PlaywrightNetworkRecord = {
  url: string;
  method?: string;
  resourceType?: string;
  status?: number;
  ok?: boolean;
  failureText?: string;
};

type PlaywrightPageState = {
  finalUrl: string;
  statusCode?: number;
  ok?: boolean;
  title?: string;
  linkCount: number;
  formCount: number;
  downloadCandidateCount: number;
  domNodeCount: number;
  ariaSnapshot?: string;
  network: PlaywrightNetworkRecord[];
  width: number;
  height: number;
};

async function loadPlaywrightChromium(): Promise<{ chromium: { launch(options: Record<string, unknown>): Promise<unknown> } } | IpcError> {
  try {
    const playwright = await import("@playwright/test");
    return { chromium: playwright.chromium as unknown as { launch(options: Record<string, unknown>): Promise<unknown> } };
  } catch (error) {
    return actionError("rust_unavailable", error instanceof Error ? error.message : String(error), { module: "@playwright/test" });
  }
}

function selectorRequired(config: AgentActionHostConfig, request: AgentActionRequest): AgentActionResult | undefined {
  if (!request.selector?.trim()) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "selector is required for this browser action.", request)
    });
  }
  return undefined;
}

async function playwrightPageState(page: any, response: any, network: PlaywrightNetworkRecord[]): Promise<PlaywrightPageState> {
  const [title, counts, ariaSnapshot, viewport] = await Promise.all([
    page.title().catch(() => undefined),
    page.evaluate(() => ({
      linkCount: document.querySelectorAll("a").length,
      formCount: document.querySelectorAll("form").length,
      downloadCandidateCount: document.querySelectorAll("[download], a[href*='download'], button, input[type='submit']").length,
      domNodeCount: document.querySelectorAll("*").length,
      width: Math.max(document.documentElement.scrollWidth, document.body?.scrollWidth ?? 0, window.innerWidth),
      height: Math.max(document.documentElement.scrollHeight, document.body?.scrollHeight ?? 0, window.innerHeight)
    })),
    typeof page.ariaSnapshot === "function" ? page.ariaSnapshot({ depth: 3, mode: "ai", boxes: true, timeout: 2_000 }).catch(() => undefined) : undefined,
    page.viewportSize?.() ?? undefined
  ]);
  return {
    finalUrl: page.url(),
    statusCode: typeof response?.status === "function" ? response.status() : undefined,
    ok: typeof response?.ok === "function" ? response.ok() : undefined,
    title,
    linkCount: counts.linkCount,
    formCount: counts.formCount,
    downloadCandidateCount: counts.downloadCandidateCount,
    domNodeCount: counts.domNodeCount,
    ariaSnapshot: typeof ariaSnapshot === "string" ? ariaSnapshot.slice(0, 8_000) : undefined,
    network: network.slice(-80),
    width: counts.width || viewport?.width || 0,
    height: counts.height || viewport?.height || 0
  };
}

async function withPlaywrightPage<T>(
  config: AgentActionHostConfig,
  request: AgentActionRequest,
  run: (params: { page: any; response: any; network: PlaywrightNetworkRecord[]; timeoutMs: number; url: URL }) => Promise<T>
): Promise<T | AgentActionResult> {
  const url = parseAgentUrl(request);
  if (isIpcError(url)) {
    return result(config, request, { accepted: false, error: url });
  }
  const loaded = await loadPlaywrightChromium();
  if ("code" in loaded) {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Playwright is not available in the Electron shell runtime.", loaded)
    });
  }
  const timeoutMs = browserFetchTimeout(request);
  let browser: any;
  let context: any;
  try {
    browser = await loaded.chromium.launch({ headless: true, timeout: Math.min(timeoutMs, 30_000) });
    context = await browser.newContext({ acceptDownloads: true, viewport: { width: 1280, height: 900 } });
    const page = await context.newPage();
    page.setDefaultTimeout(Math.min(timeoutMs, 30_000));
    const network = new Map<string, PlaywrightNetworkRecord>();
    page.on("request", (requestInfo: any) => {
      const key = `${Date.now()}:${requestInfo.url()}`;
      network.set(key, {
        url: requestInfo.url(),
        method: requestInfo.method(),
        resourceType: requestInfo.resourceType()
      });
      if (network.size > 100) {
        const first = network.keys().next().value;
        if (first) network.delete(first);
      }
    });
    page.on("response", (responseInfo: any) => {
      const urlValue = responseInfo.url();
      const existingKey = [...network.keys()].reverse().find((key) => network.get(key)?.url === urlValue);
      const entry: PlaywrightNetworkRecord = existingKey ? network.get(existingKey)! : { url: urlValue };
      entry.status = responseInfo.status();
      entry.ok = responseInfo.ok();
      if (existingKey) {
        network.set(existingKey, entry);
      }
    });
    page.on("requestfailed", (requestInfo: any) => {
      const urlValue = requestInfo.url();
      const existingKey = [...network.keys()].reverse().find((key) => network.get(key)?.url === urlValue);
      const entry: PlaywrightNetworkRecord = existingKey ? network.get(existingKey)! : { url: urlValue };
      entry.failureText = requestInfo.failure()?.errorText ?? "failed";
      if (existingKey) {
        network.set(existingKey, entry);
      }
    });
    const response = await page.goto(url.toString(), { waitUntil: "domcontentloaded", timeout: timeoutMs });
    return await run({ page, response, network: [...network.values()], timeoutMs, url });
  } catch (error) {
    const timedOut = error instanceof Error && /timeout/i.test(error.message);
    return result(config, request, {
      accepted: false,
      commandLine: `playwright chromium ${url.toString()}`,
      routeId: "browser.playwright",
      timeoutMs,
      timedOut,
      failureCategory: timedOut ? "timeout" : "missing_tool",
      error: actionError("rust_unavailable", timedOut ? `Playwright action timed out after ${timeoutMs}ms.` : error instanceof Error ? error.message : String(error), {
        url: url.toString()
      })
    });
  } finally {
    try {
      await context?.close();
    } catch {
      // Best-effort cleanup only.
    }
    try {
      await browser?.close();
    } catch {
      // Best-effort cleanup only.
    }
  }
}

async function browserInspectUrlAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const url = parseAgentUrl(request);
  if (isIpcError(url)) {
    return result(config, request, { accepted: false, error: url });
  }
  const timeoutMs = browserFetchTimeout(request);
  const startedAt = Date.now();
  try {
    const response = await fetchWithTimeout(url, timeoutMs);
    const contentType = response.headers.get("content-type") ?? undefined;
    const contentLength = Number(response.headers.get("content-length") ?? "0");
    const shouldReadBody = url.protocol === "data:" || !contentLength || contentLength <= 2_000_000;
    const body = shouldReadBody ? Buffer.from(await response.arrayBuffer()) : Buffer.alloc(0);
    const text = contentType?.includes("text/html") || contentType?.includes("text/") || url.protocol === "data:" ? body.toString("utf8") : "";
    const page = browserPageSummary({
      action: "inspect_url",
      url: url.toString(),
      finalUrl: response.url || url.toString(),
      statusCode: response.status,
      ok: response.ok,
      contentType,
      title: htmlTitle(text),
      byteLength: shouldReadBody ? body.byteLength : contentLength,
      linkCount: text ? countPattern(text, /<a\b/gi) : undefined,
      formCount: text ? countPattern(text, /<form\b/gi) : undefined,
      downloadCandidateCount: text ? countPattern(text, /\bdownload\b/gi) : undefined,
      screenshotStatus: "planned",
      domStatus: text ? "available" : "planned",
      networkLogStatus: "planned"
    });
    const verification = verificationResult([
      verificationProbe({
        id: "browser.inspect.response",
        kind: "browser_state",
        target: page.finalUrl,
        expectation: "HTTP response state returned",
        actual: `status=${page.statusCode ?? "unknown"} bytes=${page.byteLength ?? 0}`,
        passed: typeof page.statusCode === "number" || url.protocol === "data:"
      })
    ]);
    return result(config, request, {
      accepted: true,
      commandLine: `fetch ${url.toString()}`,
      routeId: "browser.inspect_url",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      stdoutPreview: text.slice(0, MAX_PREVIEW_CHARS),
      observedChanges: [`browser_status:${page.statusCode ?? "data"}`, `browser_bytes:${page.byteLength ?? 0}`],
      verification,
      browserPage: page
    });
  } catch (error) {
    const timedOut = error instanceof Error && error.name === "AbortError";
    return result(config, request, {
      accepted: false,
      commandLine: `fetch ${url.toString()}`,
      routeId: "browser.inspect_url",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      timedOut,
      failureCategory: timedOut ? "timeout" : "command_error",
      error: actionError("rust_unavailable", timedOut ? `URL inspection timed out after ${timeoutMs}ms.` : error instanceof Error ? error.message : String(error), {
        url: url.toString()
      })
    });
  }
}

async function browserDownloadAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Download requires confirmed:true and will be persisted with size and sha256.", request)
    });
  }
  const url = parseAgentUrl(request);
  if (isIpcError(url)) {
    return result(config, request, { accepted: false, error: url });
  }
  const timeoutMs = browserFetchTimeout(request);
  const startedAt = Date.now();
  try {
    const response = await fetchWithTimeout(url, timeoutMs);
    if (!response.ok && url.protocol !== "data:") {
      return result(config, request, {
        accepted: false,
        commandLine: `fetch ${url.toString()} > <download>`,
        routeId: "browser.download",
        durationMs: Math.max(0, Date.now() - startedAt),
        timeoutMs,
        failureCategory: "command_error",
        error: actionError("rust_unavailable", `Download response was HTTP ${response.status}.`, { url: url.toString(), status: response.status })
      });
    }
    const suggestedFilename = suggestedDownloadFilename(url, response.headers.get("content-disposition"));
    const targetPath = resolveActionPath(
      config,
      { ...request, scope: request.scope ?? "workspace" },
      request.path ?? `.ingen-agent-artifacts/downloads/${suggestedFilename}`
    );
    if (typeof targetPath !== "string") {
      return result(config, request, { accepted: false, error: targetPath });
    }
    await mkdir(dirname(targetPath), { recursive: true });
    const bytes = Buffer.from(await response.arrayBuffer());
    await writeFile(targetPath, bytes, { flag: "wx" });
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    const artifact = browserDownloadArtifact({
      url: url.toString(),
      path: pathLabel(config, request, targetPath),
      bytes: bytes.length,
      sha256,
      contentType: response.headers.get("content-type") ?? undefined,
      suggestedFilename
    });
    const verification = verificationResult([
      await filesystemProbe("browser.download.exists", targetPath, "file"),
      verificationProbe({
        id: "browser.download.hash",
        kind: "artifact_hash",
        target: targetPath,
        expectation: "sha256 computed for persisted download",
        actual: sha256,
        passed: /^[a-f0-9]{64}$/.test(sha256)
      })
    ]);
    return result(config, request, {
      accepted: true,
      path: artifact.path,
      commandLine: `fetch ${url.toString()} > ${artifact.path}`,
      routeId: "browser.download",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      artifacts: [artifact.path],
      observedChanges: [`download:${artifact.bytes}`, `sha256:${artifact.sha256}`],
      verification,
      download: artifact,
      value: charDeltaValue(bytes.length, 0),
      userPresenceRequired: true
    });
  } catch (error) {
    const timedOut = error instanceof Error && error.name === "AbortError";
    return result(config, request, {
      accepted: false,
      commandLine: `fetch ${url.toString()} > <download>`,
      routeId: "browser.download",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      timedOut,
      failureCategory: timedOut ? "timeout" : "command_error",
      error: actionError("rust_unavailable", timedOut ? `Download timed out after ${timeoutMs}ms.` : error instanceof Error ? error.message : String(error), {
        url: url.toString()
      })
    });
  }
}

async function browserOpenUrlAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Opening an external browser URL requires confirmed:true.", request)
    });
  }
  const url = parseAgentUrl(request, false);
  if (isIpcError(url)) {
    return result(config, request, { accepted: false, error: url });
  }
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$url = ${powerShellString(url.toString())}
Start-Process $url
[pscustomobject]@{ opened = $true; url = $url } | ConvertTo-Json -Compress
`;
  const execution = executePowerShellJson(script, 8_000);
  if (!execution.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: "powershell.exe -EncodedCommand <browser.open_url>",
      executionAdapter: "powershell",
      routeId: "browser.open_url",
      exitCode: execution.exitCode,
      timedOut: execution.timedOut,
      stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: execution.timedOut ? "timeout" : "command_error",
      error: execution.error
    });
  }
  const parsed = parseJsonObject<{ opened: boolean; url: string }>(execution.stdout);
  const page = browserPageSummary({
    action: "open_url",
    url: parsed.url,
    finalUrl: parsed.url,
    screenshotStatus: "planned",
    domStatus: "planned",
    networkLogStatus: "planned"
  });
  return result(config, request, {
    accepted: true,
    commandLine: "powershell.exe -EncodedCommand <browser.open_url>",
    executionAdapter: "powershell",
    routeId: "browser.open_url",
    exitCode: execution.exitCode,
    observedChanges: [`browser_opened:${parsed.url}`],
    verification: verificationResult([
      verificationProbe({
        id: "browser.open_url.start_process",
        kind: "browser_state",
        target: parsed.url,
        expectation: "Start-Process accepted URL handoff",
        actual: `opened=${parsed.opened === true}`,
        passed: parsed.opened === true
      })
    ]),
    browserPage: page,
    userPresenceRequired: true
  });
}

async function browserPlaywrightInspectAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const startedAt = Date.now();
  const outcome = await withPlaywrightPage(config, request, async ({ page, response, network, timeoutMs, url }) => {
    const state = await playwrightPageState(page, response, network);
    return result(config, request, {
      accepted: true,
      commandLine: `playwright chromium inspect ${url.toString()}`,
      routeId: "browser.playwright_inspect",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      observedChanges: [`dom_nodes:${state.domNodeCount}`, `network_events:${state.network.length}`, `forms:${state.formCount}`],
      verification: verificationResult([
        verificationProbe({
          id: "browser.playwright.dom",
          kind: "browser_state",
          target: state.finalUrl,
          expectation: "DOM and network state returned from isolated Playwright page",
          actual: `dom_nodes=${state.domNodeCount} network=${state.network.length} status=${state.statusCode ?? "data"}`,
          passed: state.domNodeCount > 0 && (typeof state.statusCode === "number" || url.protocol === "data:")
        })
      ]),
      browserPage: browserPageSummary({
        action: "playwright_inspect",
        url: url.toString(),
        finalUrl: state.finalUrl,
        statusCode: state.statusCode,
        ok: state.ok,
        title: state.title,
        linkCount: state.linkCount,
        formCount: state.formCount,
        downloadCandidateCount: state.downloadCandidateCount,
        domNodeCount: state.domNodeCount,
        ariaSnapshot: state.ariaSnapshot,
        network: state.network,
        screenshotStatus: "planned",
        domStatus: "available",
        networkLogStatus: "available"
      })
    });
  });
  return outcome as AgentActionResult;
}

async function browserScreenshotAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Browser screenshot requires confirmed:true because it persists a visual artifact.", request)
    });
  }
  const startedAt = Date.now();
  const outcome = await withPlaywrightPage(config, request, async ({ page, response, network, timeoutMs, url }) => {
    const targetPath = resolveActionPath(config, { ...request, scope: request.scope ?? "workspace" }, request.path ?? `.ingen-agent-artifacts/browser-screenshot-${Date.now()}.png`);
    if (typeof targetPath !== "string") {
      return result(config, request, { accepted: false, error: targetPath });
    }
    if (await pathExists(targetPath)) {
      return result(config, request, {
        accepted: false,
        failureCategory: "bad_path",
        error: actionError("bad_payload", "Browser screenshot target already exists; refusing to overwrite.", { path: targetPath })
      });
    }
    await mkdir(dirname(targetPath), { recursive: true });
    await page.screenshot({ path: targetPath, fullPage: true });
    const bytes = await readFile(targetPath);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    const state = await playwrightPageState(page, response, network);
    const artifact = browserScreenshotArtifact({
      url: url.toString(),
      path: pathLabel(config, request, targetPath),
      width: state.width,
      height: state.height,
      bytes: bytes.length,
      sha256
    });
    return result(config, request, {
      accepted: true,
      path: artifact.path,
      commandLine: `playwright chromium screenshot ${url.toString()} > ${artifact.path}`,
      routeId: "browser.screenshot",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      artifacts: [artifact.path],
      observedChanges: [`browser_screenshot:${artifact.width}x${artifact.height}`, `sha256:${artifact.sha256}`],
      verification: verificationResult([
        await filesystemProbe("browser.screenshot.exists", targetPath, "file"),
        verificationProbe({
          id: "browser.screenshot.hash",
          kind: "artifact_hash",
          target: targetPath,
          expectation: "screenshot sha256 computed",
          actual: sha256,
          passed: /^[a-f0-9]{64}$/.test(sha256)
        })
      ]),
      browserPage: browserPageSummary({
        action: "screenshot",
        url: url.toString(),
        finalUrl: state.finalUrl,
        statusCode: state.statusCode,
        ok: state.ok,
        title: state.title,
        linkCount: state.linkCount,
        formCount: state.formCount,
        downloadCandidateCount: state.downloadCandidateCount,
        domNodeCount: state.domNodeCount,
        network: state.network,
        screenshotStatus: "available",
        domStatus: "available",
        networkLogStatus: "available"
      }),
      browserScreenshot: artifact,
      userPresenceRequired: true
    });
  });
  return outcome as AgentActionResult;
}

async function browserSelectorInfo(page: any, selector: string): Promise<{
  exists: boolean;
  tagName?: string;
  type?: string;
  text?: string;
  role?: string;
  autocomplete?: string;
  inForm?: boolean;
  submitLike?: boolean;
  sensitiveLike?: boolean;
}> {
  const locator = page.locator(selector).first();
  const count = await locator.count().catch(() => 0);
  if (count < 1) {
    return { exists: false };
  }
  return await locator.evaluate((element: Element) => {
    const input = element as HTMLInputElement;
    const tagName = element.tagName.toLowerCase();
    const type = (input.getAttribute?.("type") ?? "").toLowerCase();
    const autocomplete = (input.getAttribute?.("autocomplete") ?? "").toLowerCase();
    const text = (element.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 240);
    const role = (element.getAttribute("role") ?? "").toLowerCase();
    const inForm = Boolean(element.closest("form"));
    const submitLike = tagName === "button" || role === "button" || type === "submit" || /submit|pay|purchase|checkout|confirm|buy/i.test(text);
    const sensitiveLike = type === "password" || /one-time-code|current-password|new-password|cc-|card|credential|password|pin/i.test(`${autocomplete} ${type} ${text}`);
    return { exists: true, tagName, type, text, role, autocomplete, inForm, submitLike, sensitiveLike };
  });
}

function browserInteractionConfirmationError(config: AgentActionHostConfig, request: AgentActionRequest): AgentActionResult | undefined {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      userPresenceRequired: true,
      failureCategory: "denied",
      error: actionError("bad_payload", "Browser interaction requires confirmed:true.", request)
    });
  }
  return selectorRequired(config, request);
}

async function browserClickAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const preflight = browserInteractionConfirmationError(config, request);
  if (preflight) return preflight;
  const selector = request.selector!.trim();
  const startedAt = Date.now();
  const outcome = await withPlaywrightPage(config, request, async ({ page, response, network, timeoutMs, url }) => {
    const info = await browserSelectorInfo(page, selector);
    if (!info.exists) {
      return result(config, request, { accepted: false, failureCategory: "bad_path", error: actionError("bad_payload", "Browser selector did not match any element.", { selector }) });
    }
    if ((info.inForm || info.submitLike) && request.formSubmissionConfirmed !== true) {
      const state = await playwrightPageState(page, response, network);
      return result(config, request, {
        accepted: false,
        userPresenceRequired: true,
        failureCategory: "denied",
        browserPage: browserPageSummary({
          action: "click",
          url: url.toString(),
          finalUrl: state.finalUrl,
          selector,
          selectorMatched: true,
          formCount: state.formCount,
          domNodeCount: state.domNodeCount,
          network: state.network,
          screenshotStatus: "planned",
          domStatus: "available",
          networkLogStatus: "available"
        }),
        error: actionError("bad_payload", "Click may submit a form or trigger an account/payment action; set formSubmissionConfirmed:true to proceed.", info)
      });
    }
    await page.locator(selector).first().click();
    await page.waitForLoadState("domcontentloaded", { timeout: 2_000 }).catch(() => undefined);
    const state = await playwrightPageState(page, response, network);
    return result(config, request, {
      accepted: true,
      commandLine: `playwright chromium click ${url.toString()} ${selector}`,
      routeId: "browser.click",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      observedChanges: [`selector_clicked:${selector}`, `final_url:${state.finalUrl}`, `network_events:${state.network.length}`],
      verification: verificationResult([
        verificationProbe({
          id: "browser.click.selector",
          kind: "browser_state",
          target: selector,
          expectation: "selector matched and click completed without a blocked form submission",
          actual: `matched=${info.exists} final_url=${state.finalUrl}`,
          passed: true
        })
      ]),
      browserPage: browserPageSummary({
        action: "click",
        url: url.toString(),
        finalUrl: state.finalUrl,
        statusCode: state.statusCode,
        ok: state.ok,
        title: state.title,
        selector,
        selectorMatched: true,
        linkCount: state.linkCount,
        formCount: state.formCount,
        downloadCandidateCount: state.downloadCandidateCount,
        domNodeCount: state.domNodeCount,
        ariaSnapshot: state.ariaSnapshot,
        network: state.network,
        screenshotStatus: "planned",
        domStatus: "available",
        networkLogStatus: "available"
      }),
      userPresenceRequired: true
    });
  });
  return outcome as AgentActionResult;
}

async function browserTypeTextAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const preflight = browserInteractionConfirmationError(config, request);
  if (preflight) return preflight;
  if (request.text === undefined) {
    return result(config, request, { accepted: false, error: actionError("bad_payload", "text is required for browser_type_text.", request) });
  }
  const selector = request.selector!.trim();
  const startedAt = Date.now();
  const outcome = await withPlaywrightPage(config, request, async ({ page, response, network, timeoutMs, url }) => {
    const info = await browserSelectorInfo(page, selector);
    if (!info.exists) {
      return result(config, request, { accepted: false, failureCategory: "bad_path", error: actionError("bad_payload", "Browser selector did not match any element.", { selector }) });
    }
    if (info.sensitiveLike) {
      return result(config, request, {
        accepted: false,
        userPresenceRequired: true,
        failureCategory: "denied",
        error: actionError("bad_payload", "Typing into password, credential, one-time-code or payment fields is blocked.", info)
      });
    }
    await page.locator(selector).first().fill(request.text ?? "");
    const actualValue = await page.locator(selector).first().evaluate((element: Element) => (element as HTMLInputElement).value ?? element.textContent ?? "");
    const state = await playwrightPageState(page, response, network);
    return result(config, request, {
      accepted: true,
      commandLine: `playwright chromium fill ${url.toString()} ${selector}`,
      routeId: "browser.type_text",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      observedChanges: [`selector_filled:${selector}`, `typed_chars:${request.text?.length ?? 0}`],
      verification: verificationResult([
        verificationProbe({
          id: "browser.type_text.value",
          kind: "browser_state",
          target: selector,
          expectation: "locator value equals requested text",
          actual: `chars=${String(actualValue).length}`,
          passed: actualValue === request.text
        })
      ]),
      browserPage: browserPageSummary({
        action: "type_text",
        url: url.toString(),
        finalUrl: state.finalUrl,
        statusCode: state.statusCode,
        ok: state.ok,
        title: state.title,
        selector,
        selectorMatched: true,
        linkCount: state.linkCount,
        formCount: state.formCount,
        downloadCandidateCount: state.downloadCandidateCount,
        domNodeCount: state.domNodeCount,
        ariaSnapshot: state.ariaSnapshot,
        network: state.network,
        screenshotStatus: "planned",
        domStatus: "available",
        networkLogStatus: "available"
      }),
      value: charDeltaValue(request.text?.length ?? 0, 0),
      userPresenceRequired: true
    });
  });
  return outcome as AgentActionResult;
}

async function browserPlaywrightDownloadAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const preflight = browserInteractionConfirmationError(config, request);
  if (preflight) return preflight;
  const selector = request.selector!.trim();
  const startedAt = Date.now();
  const outcome = await withPlaywrightPage(config, request, async ({ page, response, network, timeoutMs, url }) => {
    const info = await browserSelectorInfo(page, selector);
    if (!info.exists) {
      return result(config, request, { accepted: false, failureCategory: "bad_path", error: actionError("bad_payload", "Browser selector did not match any element.", { selector }) });
    }
    if ((info.inForm || info.submitLike) && request.formSubmissionConfirmed !== true && !/download/i.test(`${info.text ?? ""} ${selector}`)) {
      return result(config, request, {
        accepted: false,
        userPresenceRequired: true,
        failureCategory: "denied",
        error: actionError("bad_payload", "Download click is form-associated; set formSubmissionConfirmed:true to proceed.", info)
      });
    }
    const downloadPromise = page.waitForEvent("download", { timeout: timeoutMs });
    await page.locator(selector).first().click();
    const download = await downloadPromise;
    const suggestedFilename = suggestedDownloadFilename(url, download.suggestedFilename?.() ?? null);
    const targetPath = resolveActionPath(config, { ...request, scope: request.scope ?? "workspace" }, request.path ?? `.ingen-agent-artifacts/downloads/${suggestedFilename}`);
    if (typeof targetPath !== "string") {
      return result(config, request, { accepted: false, error: targetPath });
    }
    if (await pathExists(targetPath)) {
      return result(config, request, { accepted: false, failureCategory: "bad_path", error: actionError("bad_payload", "Download target already exists; refusing to overwrite.", { path: targetPath }) });
    }
    await mkdir(dirname(targetPath), { recursive: true });
    await download.saveAs(targetPath);
    const bytes = await readFile(targetPath);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    const state = await playwrightPageState(page, response, network);
    const artifact = browserDownloadArtifact({
      url: download.url?.() ?? url.toString(),
      path: pathLabel(config, request, targetPath),
      bytes: bytes.length,
      sha256,
      suggestedFilename
    });
    return result(config, request, {
      accepted: true,
      path: artifact.path,
      commandLine: `playwright chromium download ${url.toString()} ${selector} > ${artifact.path}`,
      routeId: "browser.playwright_download",
      durationMs: Math.max(0, Date.now() - startedAt),
      timeoutMs,
      artifacts: [artifact.path],
      observedChanges: [`download:${artifact.bytes}`, `sha256:${artifact.sha256}`, `network_events:${state.network.length}`],
      verification: verificationResult([
        await filesystemProbe("browser.playwright_download.exists", targetPath, "file"),
        verificationProbe({
          id: "browser.playwright_download.hash",
          kind: "artifact_hash",
          target: targetPath,
          expectation: "sha256 computed for Playwright download",
          actual: sha256,
          passed: /^[a-f0-9]{64}$/.test(sha256)
        })
      ]),
      browserPage: browserPageSummary({
        action: "playwright_download",
        url: url.toString(),
        finalUrl: state.finalUrl,
        statusCode: state.statusCode,
        ok: state.ok,
        title: state.title,
        selector,
        selectorMatched: true,
        linkCount: state.linkCount,
        formCount: state.formCount,
        downloadCandidateCount: state.downloadCandidateCount,
        domNodeCount: state.domNodeCount,
        network: state.network,
        screenshotStatus: "planned",
        domStatus: "available",
        networkLogStatus: "available"
      }),
      download: artifact,
      value: charDeltaValue(bytes.length, 0),
      userPresenceRequired: true
    });
  });
  return outcome as AgentActionResult;
}

function documentKindForPath(path: string): AgentDocumentMediaKind {
  const extension = parse(path).ext.toLowerCase();
  if ([".txt", ".log", ".ini", ".yaml", ".yml", ".xml", ".html", ".css", ".ts", ".tsx", ".js", ".jsx", ".rs", ".py"].includes(extension)) {
    return "text";
  }
  if ([".md", ".markdown"].includes(extension)) return "markdown";
  if (extension === ".json") return "json";
  if ([".csv", ".tsv"].includes(extension)) return "csv";
  if ([".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".odt", ".ods", ".odp"].includes(extension)) return "office";
  if (extension === ".pdf") return "pdf";
  if ([".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg", ".ico"].includes(extension)) return "image";
  if ([".mp3", ".wav", ".ogg", ".flac", ".m4a"].includes(extension)) return "audio";
  if ([".mp4", ".mov", ".mkv", ".avi", ".webm"].includes(extension)) return "video";
  if ([".zip", ".7z", ".rar", ".tar", ".gz"].includes(extension)) return "archive";
  return extension ? "binary" : "unknown";
}

function documentParserStatus(kind: AgentDocumentMediaKind): "available" | "planned" | "blocked" {
  return ["text", "markdown", "json", "csv"].includes(kind) ? "available" : "planned";
}

function textLineCount(text: string): number {
  if (!text) return 0;
  return text.split(/\r\n|\r|\n/).length;
}

function parseCsvRows(text: string): { rows: string[][]; valid: boolean; error?: string } {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    const next = text[index + 1];
    if (quoted) {
      if (char === '"' && next === '"') {
        field += '"';
        index += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
      continue;
    }
    if (char === '"') {
      if (field.length > 0) {
        return { rows, valid: false, error: "quote appeared inside an unquoted field" };
      }
      quoted = true;
      continue;
    }
    if (char === ",") {
      row.push(field);
      field = "";
      continue;
    }
    if (char === "\r" || char === "\n") {
      if (char === "\r" && next === "\n") {
        index += 1;
      }
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      continue;
    }
    field += char;
  }
  if (quoted) {
    return { rows, valid: false, error: "unterminated quoted field" };
  }
  if (field.length > 0 || row.length > 0 || text.endsWith(",")) {
    row.push(field);
    rows.push(row);
  }
  const width = rows[0]?.length ?? 0;
  const valid = rows.every((candidate) => candidate.length === width);
  return { rows, valid, error: valid ? undefined : "rows do not have a consistent number of fields" };
}

function stripMarkdownToText(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, (block) => block.replace(/```[a-zA-Z0-9_-]*\n?|\n?```/g, ""))
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^[>\-*+]\s+/gm, "")
    .replace(/[*_`~]/g, "")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd();
}

function documentMediaSummaryFromBytes(params: {
  action: AgentDocumentMediaSummary["action"];
  path: string;
  label: string;
  bytes: Buffer;
}): AgentDocumentMediaSummary {
  const kind = documentKindForPath(params.path);
  const extension = parse(params.path).ext.toLowerCase();
  const sha256 = createHash("sha256").update(params.bytes).digest("hex");
  const parserStatus = documentParserStatus(kind);
  const summary: AgentDocumentMediaSummary = {
    schema: "ingen.document_media.summary.v1",
    action: params.action,
    path: params.label,
    kind,
    extension,
    bytes: params.bytes.length,
    sha256,
    parserStatus,
    conversionStatus: ["text", "markdown"].includes(kind) ? "available" : "planned",
    proofHash: ""
  };
  if (parserStatus === "available") {
    const text = params.bytes.toString("utf8");
    summary.lineCount = textLineCount(text);
    summary.charCount = text.length;
    if (kind === "json") {
      try {
        JSON.parse(text);
        summary.jsonValid = true;
      } catch {
        summary.jsonValid = false;
      }
    }
    if (kind === "csv") {
      const parsed = parseCsvRows(text);
      summary.csvRows = parsed.rows.length;
      summary.csvColumns = parsed.rows[0]?.length ?? 0;
    }
    if (kind === "markdown") {
      summary.markdownHeadingCount = Array.from(text.matchAll(/^#{1,6}\s+/gm)).length;
    }
  }
  summary.proofHash = hashJson({ ...summary, proofHash: "" });
  return summary;
}

async function documentMediaSummaryForPath(
  config: AgentActionHostConfig,
  request: AgentActionRequest,
  action: AgentDocumentMediaSummary["action"],
  absolutePath: string
): Promise<AgentDocumentMediaSummary> {
  const bytes = await readFile(absolutePath);
  return documentMediaSummaryFromBytes({
    action,
    path: absolutePath,
    label: pathLabel(config, request, absolutePath),
    bytes
  });
}

async function documentInspectAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const summary = await documentMediaSummaryForPath(config, request, "inspect", resolved);
  return result(config, request, {
    accepted: true,
    path: summary.path,
    observedChanges: [`document_kind:${summary.kind}`, `bytes:${summary.bytes}`, `sha256:${summary.sha256}`],
    verification: verificationResult([
      await filesystemProbe("document.inspect.exists", resolved, "file"),
      verificationProbe({
        id: "document.inspect.hash",
        kind: "artifact_hash",
        target: resolved,
        expectation: "sha256 computed",
        actual: summary.sha256,
        passed: /^[a-f0-9]{64}$/.test(summary.sha256)
      })
    ]),
    documentMedia: summary
  });
}

async function readPreviousText(path: string): Promise<string> {
  try {
    return await readFile(path, "utf8");
  } catch {
    return "";
  }
}

async function writeDocumentContent(
  config: AgentActionHostConfig,
  request: AgentActionRequest,
  action: AgentDocumentMediaSummary["action"],
  content: string
): Promise<AgentActionResult> {
  if (requiresComputerWriteConfirmation(request)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Computer-scope document write requires confirmed:true.", request)
    });
  }
  const resolved = resolveActionPath(config, request, request.path);
  if (typeof resolved !== "string") {
    return result(config, request, { accepted: false, error: resolved });
  }
  const previous = await readPreviousText(resolved);
  await mkdir(dirname(resolved), { recursive: true });
  await writeFile(resolved, content, "utf8");
  const summary = await documentMediaSummaryForPath(config, request, action, resolved);
  const readback = await readFile(resolved, "utf8");
  const verification = verificationResult([
    await filesystemProbe("document.write.exists", resolved, "file"),
    verificationProbe({
      id: "document.write.readback",
      kind: "artifact_hash",
      target: resolved,
      expectation: "readback matches requested content",
      actual: `matches=${readback === content} sha256=${summary.sha256}`,
      passed: readback === content
    })
  ]);
  return result(config, request, {
    accepted: true,
    path: summary.path,
    artifacts: [summary.path],
    observedChanges: [`document_kind:${summary.kind}`, `bytes:${summary.bytes}`, `sha256:${summary.sha256}`],
    verification,
    documentMedia: summary,
    value: charDeltaValue(content.length, previous.length)
  });
}

async function documentWriteTextAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.content === undefined) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "content is required for document_write_text.", request)
    });
  }
  return writeDocumentContent(config, request, "write_text", request.content);
}

async function documentWriteJsonAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.content === undefined) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "content is required for document_write_json.", request)
    });
  }
  let rendered = "";
  try {
    rendered = `${JSON.stringify(JSON.parse(request.content), null, 2)}\n`;
  } catch (error) {
    return result(config, request, {
      accepted: false,
      failureCategory: "bad_path",
      error: actionError("bad_payload", error instanceof Error ? `Invalid JSON: ${error.message}` : "Invalid JSON.", request)
    });
  }
  return writeDocumentContent(config, request, "write_json", rendered);
}

async function documentWriteCsvAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.content === undefined) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "content is required for document_write_csv.", request)
    });
  }
  const parsed = parseCsvRows(request.content);
  if (!parsed.valid) {
    return result(config, request, {
      accepted: false,
      failureCategory: "bad_path",
      error: actionError("bad_payload", `Invalid CSV: ${parsed.error ?? "parse failed"}.`, request)
    });
  }
  return writeDocumentContent(config, request, "write_csv", request.content.endsWith("\n") ? request.content : `${request.content}\n`);
}

async function documentConvertTextAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (requiresComputerWriteConfirmation(request)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "Computer-scope document conversion requires confirmed:true.", request)
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
  const source = await readFile(from, "utf8");
  const fromKind = documentKindForPath(from);
  const toKind = documentKindForPath(to);
  if (!["text", "markdown"].includes(fromKind) || !["text", "markdown"].includes(toKind)) {
    return result(config, request, {
      accepted: false,
      failureCategory: "unverifiable",
      error: actionError("bad_payload", "document_convert_text currently supports only text and Markdown paths.", { fromKind, toKind })
    });
  }
  const converted = fromKind === "markdown" && toKind === "text" ? stripMarkdownToText(source) : source;
  await mkdir(dirname(to), { recursive: true });
  await writeFile(to, converted, "utf8");
  const summary = await documentMediaSummaryForPath(config, request, "convert_text", to);
  const verification = verificationResult([
    await filesystemProbe("document.convert.output_exists", to, "file"),
    verificationProbe({
      id: "document.convert.output_hash",
      kind: "artifact_hash",
      target: to,
      expectation: "converted output hash computed",
      actual: summary.sha256,
      passed: /^[a-f0-9]{64}$/.test(summary.sha256)
    })
  ]);
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, request, from),
    toPath: summary.path,
    artifacts: [summary.path],
    observedChanges: [`document_convert:${fromKind}->${toKind}`, `sha256:${summary.sha256}`],
    verification,
    documentMedia: summary,
    value: charDeltaValue(converted.length, 0)
  });
}

function parseGitStatusPorcelain(output: string): {
  branch?: string;
  ahead?: number;
  behind?: number;
  changedFiles: number;
  stagedFiles: number;
  unstagedFiles: number;
  untrackedFiles: number;
} {
  let branch: string | undefined;
  let ahead: number | undefined;
  let behind: number | undefined;
  let changedFiles = 0;
  let stagedFiles = 0;
  let unstagedFiles = 0;
  let untrackedFiles = 0;
  for (const line of output.split(/\r?\n/)) {
    if (!line) continue;
    if (line.startsWith("## ")) {
      const branchPart = line.slice(3).split("...", 1)[0]?.split(/\s+/, 1)[0];
      branch = branchPart && branchPart !== "HEAD" && branchPart !== "No" ? branchPart : undefined;
      const aheadMatch = /\[ahead (\d+)/.exec(line);
      const behindMatch = /behind (\d+)/.exec(line);
      ahead = aheadMatch ? Number(aheadMatch[1]) : undefined;
      behind = behindMatch ? Number(behindMatch[1]) : undefined;
      continue;
    }
    changedFiles += 1;
    if (line.startsWith("??")) {
      untrackedFiles += 1;
      continue;
    }
    const indexState = line[0] ?? " ";
    const worktreeState = line[1] ?? " ";
    if (indexState !== " " && indexState !== "?") stagedFiles += 1;
    if (worktreeState !== " " && worktreeState !== "?") unstagedFiles += 1;
  }
  return { branch, ahead, behind, changedFiles, stagedFiles, unstagedFiles, untrackedFiles };
}

function developerRepoSummary(input: Omit<AgentDeveloperRepoSummary, "schema" | "proofHash">): AgentDeveloperRepoSummary {
  const summary: AgentDeveloperRepoSummary = {
    schema: "ingen.developer.repo_summary.v1",
    ...input,
    proofHash: ""
  };
  summary.proofHash = hashJson({ ...summary, proofHash: "" });
  return summary;
}

function automationLedgerEntry(input: Omit<AgentAutomationLedgerEntry, "schema" | "proofHash">): AgentAutomationLedgerEntry {
  const entry: AgentAutomationLedgerEntry = {
    schema: "ingen.automation.ledger_entry.v1",
    ...input,
    proofHash: ""
  };
  entry.proofHash = hashJson({ ...entry, proofHash: "" });
  return entry;
}

const INGEN_TASK_SCHEDULER_ROOT = "InGenAgent_";

function automationLedgerPath(config: AgentActionHostConfig): string {
  return resolve(config.workspaceRoot, ".ingen-agent-artifacts", "automation-ledger.jsonl");
}

function normalizeInGenTaskPath(input: string): string | IpcError {
  const trimmed = input.trim();
  if (!trimmed) {
    return actionError("bad_payload", "taskName is required.", input);
  }
  const withoutLeadingSlash = trimmed.replace(/^\\+/, "");
  const raw = withoutLeadingSlash.startsWith(INGEN_TASK_SCHEDULER_ROOT) ? withoutLeadingSlash : `${INGEN_TASK_SCHEDULER_ROOT}${withoutLeadingSlash}`;
  const normalized = raw.replace(/\//g, "\\").replace(/\\{2,}/g, "\\");
  if (!normalized.startsWith(INGEN_TASK_SCHEDULER_ROOT)) {
    return actionError("bad_payload", "Only InGenAgent_ scheduled tasks can be managed by direct automation actions.", input);
  }
  if (normalized.includes("\\") || normalized.includes("..") || /[<>:"|?*]/.test(normalized)) {
    return actionError("bad_payload", "Task name must be a root Task Scheduler name with no folders or forbidden characters.", input);
  }
  return normalized.slice(0, 238);
}

function defaultTaskName(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48) || "automation";
  const suffix = createHash("sha256").update(`${Date.now()}\n${title}`).digest("hex").slice(0, 10);
  return `${slug}-${suffix}`;
}

function defaultSchedulerStartTime(): string {
  const date = new Date(Date.now() + 5 * 60_000);
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${hours}:${minutes}`;
}

function executeSchtasks(args: string[], timeoutMs = 30_000): GitExecution {
  const startedAt = Date.now();
  const child = spawnSync("schtasks.exe", args, {
    encoding: "utf8",
    stdio: "pipe",
    timeout: timeoutMs,
    windowsHide: true
  });
  const durationMs = Math.max(0, Date.now() - startedAt);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const timedOut = commandTimedOut(child.error);
  const accepted = !child.error && child.status === 0;
  const commandLine = renderCommandLine("schtasks.exe", args);
  return {
    accepted,
    commandLine,
    exitCode,
    durationMs,
    stdout,
    stderr,
    timedOut,
    error: accepted
      ? undefined
      : actionError(
          "rust_unavailable",
          timedOut ? `schtasks command timed out after ${timeoutMs}ms.` : child.error?.message ?? `schtasks exited with status ${exitCode ?? "unknown"}.`,
          { args, stderr, exitCode, timedOut }
        )
  };
}

function schedulerCommandFailure(config: AgentActionHostConfig, request: AgentActionRequest, routeId: string, execution: GitExecution): AgentActionResult {
  const schedulerOutput = `${execution.stderr}\n${execution.stdout}\n${execution.error?.message ?? ""}`;
  const permissionDenied = /access denied|denied|acces refuse|accès refusé|refus|permission|privilege|0x80070005/i.test(schedulerOutput);
  const unavailable = /path.*not.*found|system cannot find|chemin.*introuvable|introuvable/i.test(schedulerOutput);
  return result(config, request, {
    accepted: false,
    commandLine: execution.commandLine,
    routeId,
    exitCode: execution.exitCode,
    durationMs: execution.durationMs,
    timedOut: execution.timedOut,
    stdoutPreview: execution.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
    failureCategory: execution.timedOut ? "timeout" : permissionDenied ? "permission" : unavailable ? "missing_tool" : "command_error",
    verification: commandExitVerification({
      commandLine: execution.commandLine,
      accepted: false,
      exitCode: execution.exitCode,
      timedOut: execution.timedOut
    }),
    error: execution.error
  });
}

function schedulerTaskRows(csvText: string): Record<string, string>[] {
  const parsed = parseCsvRows(csvText);
  if (!parsed.valid || parsed.rows.length < 2) {
    return [];
  }
  const headers = parsed.rows[0] ?? [];
  return parsed.rows.slice(1).map((row) => {
    const record: Record<string, string> = {};
    headers.forEach((header, index) => {
      record[header] = row[index] ?? "";
    });
    return record;
  });
}

function schedulerTaskSummaryFromQuery(taskPath: string, query: GitExecution): Partial<AgentAutomationLedgerEntry> {
  const row = schedulerTaskRows(query.stdout).find((candidate) => (candidate["TaskName"] ?? "").replace(/^\\+/, "").toLowerCase() === taskPath.toLowerCase());
  return {
    taskPath,
    nextRunTime: row?.["Next Run Time"],
    schedulerStatus: row?.["Status"] || row?.["Scheduled Task State"]
  };
}

async function appendAutomationLedger(config: AgentActionHostConfig, entry: AgentAutomationLedgerEntry): Promise<{ ledgerPath: string; ledgerHash: string }> {
  const ledgerPath = automationLedgerPath(config);
  await mkdir(dirname(ledgerPath), { recursive: true });
  await appendFile(ledgerPath, `${JSON.stringify(entry)}\n`, "utf8");
  const ledgerBytes = await readFile(ledgerPath);
  return {
    ledgerPath,
    ledgerHash: createHash("sha256").update(ledgerBytes).digest("hex")
  };
}

function executeNativeTool(command: string, args: string[], cwd: string, timeoutMs = 15_000): GitExecution {
  const startedAt = Date.now();
  const child = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: "pipe",
    timeout: timeoutMs,
    windowsHide: true
  });
  const durationMs = Math.max(0, Date.now() - startedAt);
  const stdout = child.stdout ?? "";
  const stderr = child.stderr ?? "";
  const exitCode = child.status ?? null;
  const timedOut = commandTimedOut(child.error);
  const accepted = !child.error && child.status === 0;
  const commandLine = renderCommandLine(command, args);
  return {
    accepted,
    commandLine,
    exitCode,
    durationMs,
    stdout,
    stderr,
    timedOut,
    error: accepted
      ? undefined
      : actionError(
          "rust_unavailable",
          timedOut ? `${command} timed out after ${timeoutMs}ms.` : child.error?.message ?? `${command} exited with status ${exitCode ?? "unknown"}.`,
          { command, args, stderr, exitCode, timedOut }
        )
  };
}

function virtualizationSummary(input: Omit<AgentVirtualizationSummary, "schema" | "proofHash">): AgentVirtualizationSummary {
  const summary: AgentVirtualizationSummary = {
    schema: "ingen.virtualization.summary.v1",
    ...input,
    proofHash: ""
  };
  summary.proofHash = hashJson({ ...summary, proofHash: "" });
  return summary;
}

function selectedVirtualizationProviders(provider: AgentVirtualizationProvider | undefined): Exclude<AgentVirtualizationProvider, "all">[] {
  if (!provider || provider === "all") {
    return ["wsl", "docker", "hyperv"];
  }
  return [provider];
}

function virtualizationToolName(provider: Exclude<AgentVirtualizationProvider, "all">): string {
  return provider === "wsl" ? "wsl.exe" : provider === "docker" ? "docker.exe" : "powershell.exe";
}

function inspectWsl(config: AgentActionHostConfig): Record<string, unknown> {
  const status = executeNativeTool("wsl.exe", ["--status"], config.cwd, 8_000);
  const version = executeNativeTool("wsl.exe", ["--version"], config.cwd, 8_000);
  const distros = executeNativeTool("wsl.exe", ["--list", "--verbose"], config.cwd, 8_000);
  return {
    provider: "wsl",
    available: status.accepted || distros.accepted,
    commands: [
      { commandLine: status.commandLine, accepted: status.accepted, exitCode: status.exitCode },
      { commandLine: version.commandLine, accepted: version.accepted, exitCode: version.exitCode },
      { commandLine: distros.commandLine, accepted: distros.accepted, exitCode: distros.exitCode }
    ],
    statusPreview: status.stdout.slice(0, 2_000),
    versionPreview: version.stdout.slice(0, 2_000),
    distroPreview: distros.stdout.slice(0, 2_000),
    errorPreview: [status.stderr, version.stderr, distros.stderr].filter(Boolean).join("\n").slice(0, 2_000)
  };
}

function inspectDocker(config: AgentActionHostConfig): Record<string, unknown> {
  const version = executeNativeTool("docker.exe", ["version", "--format", "json"], config.cwd, 10_000);
  const containers = executeNativeTool("docker.exe", ["ps", "--all", "--format", "json"], config.cwd, 10_000);
  return {
    provider: "docker",
    available: version.accepted || containers.accepted,
    commands: [
      { commandLine: version.commandLine, accepted: version.accepted, exitCode: version.exitCode },
      { commandLine: containers.commandLine, accepted: containers.accepted, exitCode: containers.exitCode }
    ],
    versionPreview: version.stdout.slice(0, 2_000),
    containersPreview: containers.stdout.slice(0, 4_000),
    errorPreview: [version.stderr, containers.stderr].filter(Boolean).join("\n").slice(0, 2_000)
  };
}

function inspectHyperV(): Record<string, unknown> {
  const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
if (-not (Get-Command Get-VM -ErrorAction SilentlyContinue)) {
  [pscustomobject]@{ available = $false; reason = 'Hyper-V PowerShell module not available'; vms = @() } | ConvertTo-Json -Depth 5 -Compress
  exit 0
}
$vms = Get-VM | Select-Object Name, State, Status, Version, Uptime
[pscustomobject]@{ available = $true; vms = @($vms) } | ConvertTo-Json -Depth 5 -Compress
`;
  const execution = executePowerShellJson(script, 10_000);
  let parsed: Record<string, unknown> = {};
  if (execution.stdout.trim()) {
    try {
      parsed = parseJsonObject<Record<string, unknown>>(execution.stdout);
    } catch {
      parsed = { parseError: true };
    }
  }
  return {
    provider: "hyperv",
    available: execution.accepted && parsed.available === true,
    commandLine: "powershell.exe -EncodedCommand <virtualization.hyperv.inspect>",
    exitCode: execution.exitCode,
    vms: Array.isArray(parsed.vms) ? parsed.vms : [],
    reason: parsed.reason,
    errorPreview: execution.stderr.slice(0, 2_000)
  };
}

async function virtualizationInspectAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const provider = request.provider ?? "all";
  const resources = selectedVirtualizationProviders(provider).map((candidate) => {
    if (candidate === "wsl") {
      return inspectWsl(config);
    }
    if (candidate === "docker") {
      return inspectDocker(config);
    }
    return inspectHyperV();
  });
  const available = resources.some((resource) => resource.available === true);
  const summary = virtualizationSummary({
    provider,
    action: "inspect",
    available,
    resources,
    fallback: available ? undefined : "native Windows command or dev.run_check"
  });
  const verification = verificationResult([
    verificationProbe({
      id: "virtualization.inspect.completed",
      kind: "process_state",
      target: provider,
      expectation: "virtualization backends inspected without mutation",
      actual: `providers=${resources.length} available=${available}`,
      passed: resources.length > 0
    })
  ]);
  return result(config, request, {
    accepted: true,
    commandLine: `virtualization.inspect ${provider}`,
    routeId: "virtualization.inspect",
    stdoutPreview: JSON.stringify(resources, null, 2).slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`virtualization_providers:${resources.length}`, `virtualization_available:${available}`],
    verification,
    virtualization: summary,
    value: `providers ${resources.length} available ${available ? "yes" : "no"}`
  });
}

async function virtualizationRunCommandAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Virtualization command execution requires confirmed:true.", request)
    });
  }
  const provider = request.provider;
  const command = request.command?.trim() ?? "";
  if (provider !== "wsl" && provider !== "docker") {
    return result(config, request, {
      accepted: false,
      failureCategory: provider === "hyperv" ? "unverifiable" : "bad_path",
      error: actionError("bad_payload", "virtualization_run_command supports provider:\"wsl\" or provider:\"docker\" only; Hyper-V guest command execution remains planned.", request)
    });
  }
  if (!command) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "command is required for virtualization_run_command.", request)
    });
  }
  const timeoutMs = commandTimeout(request);
  const tool = virtualizationToolName(provider);
  let args: string[] = [];
  if (provider === "wsl") {
    args = request.distro ? ["--distribution", request.distro, "--exec", command, ...(request.args ?? [])] : ["--exec", command, ...(request.args ?? [])];
  } else {
    const container = request.container?.trim() ?? "";
    if (!container) {
      return result(config, request, {
        accepted: false,
        error: actionError("bad_payload", "container is required for provider:\"docker\" virtualization_run_command.", request)
      });
    }
    args = ["exec", container, command, ...(request.args ?? [])];
  }
  let execution = executeNativeTool(tool, args, config.workspaceRoot, timeoutMs);
  let routeId = `virtualization.${provider}.run_command`;
  let fallbackUsed: string | undefined;
  if (!execution.accepted && request.nativeFallback === true && request.scope !== "computer") {
    fallbackUsed = "native workspace command";
    execution = executeNativeTool(command, request.args ?? [], config.workspaceRoot, timeoutMs);
    routeId = "virtualization.native_fallback";
  }
  const summary = virtualizationSummary({
    provider,
    action: "run_command",
    available: execution.accepted,
    resources: [
      {
        provider,
        commandLine: execution.commandLine,
        exitCode: execution.exitCode,
        timedOut: execution.timedOut,
        fallbackUsed
      }
    ],
    fallback: fallbackUsed
  });
  return result(config, request, {
    accepted: execution.accepted,
    commandLine: execution.commandLine,
    executionAdapter: "windows_command",
    routeId,
    exitCode: execution.exitCode,
    durationMs: execution.durationMs,
    timeoutMs,
    timedOut: execution.timedOut,
    stdoutPreview: execution.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`virtualization_provider:${provider}`, `exit_code:${execution.exitCode ?? "unknown"}`],
    verification: commandExitVerification({
      commandLine: execution.commandLine,
      accepted: execution.accepted,
      exitCode: execution.exitCode,
      timedOut: execution.timedOut
    }),
    virtualization: summary,
    failureCategory: execution.accepted ? undefined : execution.timedOut ? "timeout" : "missing_tool",
    error: execution.error
  });
}

async function devRepoStatusAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const status = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  if (!status.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: status.commandLine,
      routeId: "dev.repo_status",
      exitCode: status.exitCode,
      durationMs: status.durationMs,
      timedOut: status.timedOut,
      stderrPreview: status.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: status.timedOut ? "timeout" : "command_error",
      error: status.error
    });
  }
  const root = executeGit(config, ["rev-parse", "--show-toplevel"]);
  const parsed = parseGitStatusPorcelain(status.stdout);
  const summary = developerRepoSummary({
    action: "repo_status",
    root: (root.accepted ? root.stdout.trim() : config.workspaceRoot) || config.workspaceRoot,
    branch: parsed.branch,
    ahead: parsed.ahead,
    behind: parsed.behind,
    changedFiles: parsed.changedFiles,
    stagedFiles: parsed.stagedFiles,
    unstagedFiles: parsed.unstagedFiles,
    untrackedFiles: parsed.untrackedFiles,
    commandLine: status.commandLine,
    exitCode: status.exitCode,
    durationMs: status.durationMs
  });
  return result(config, request, {
    accepted: true,
    commandLine: status.commandLine,
    routeId: "dev.repo_status",
    exitCode: status.exitCode,
    durationMs: status.durationMs,
    stdoutPreview: status.stdout.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`git_changed:${summary.changedFiles}`, `git_staged:${summary.stagedFiles}`, `git_untracked:${summary.untrackedFiles}`],
    verification: commandExitVerification({ commandLine: status.commandLine, accepted: true, exitCode: status.exitCode, timedOut: status.timedOut }),
    developer: summary
  });
}

async function devGitDiffAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  const statDiff = executeGit(config, ["diff", "--stat"]);
  if (!statDiff.accepted) {
    return result(config, request, {
      accepted: false,
      commandLine: statDiff.commandLine,
      routeId: "dev.git_diff",
      exitCode: statDiff.exitCode,
      durationMs: statDiff.durationMs,
      timedOut: statDiff.timedOut,
      stderrPreview: statDiff.stderr.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: statDiff.timedOut ? "timeout" : "command_error",
      error: statDiff.error
    });
  }
  const status = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  const parsed = status.accepted ? parseGitStatusPorcelain(status.stdout) : { changedFiles: 0, stagedFiles: 0, unstagedFiles: 0, untrackedFiles: 0 };
  const summary = developerRepoSummary({
    action: "git_diff",
    root: config.workspaceRoot,
    branch: parsed.branch,
    ahead: parsed.ahead,
    behind: parsed.behind,
    changedFiles: parsed.changedFiles,
    stagedFiles: parsed.stagedFiles,
    unstagedFiles: parsed.unstagedFiles,
    untrackedFiles: parsed.untrackedFiles,
    diffStat: statDiff.stdout.slice(0, MAX_PREVIEW_CHARS),
    commandLine: statDiff.commandLine,
    exitCode: statDiff.exitCode,
    durationMs: statDiff.durationMs
  });
  return result(config, request, {
    accepted: true,
    commandLine: statDiff.commandLine,
    routeId: "dev.git_diff",
    exitCode: statDiff.exitCode,
    durationMs: statDiff.durationMs,
    stdoutPreview: statDiff.stdout.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`git_diff_bytes:${statDiff.stdout.length}`],
    verification: commandExitVerification({ commandLine: statDiff.commandLine, accepted: true, exitCode: statDiff.exitCode, timedOut: statDiff.timedOut }),
    developer: summary
  });
}

function gitStatusSummary(config: AgentActionHostConfig, action: AgentDeveloperRepoSummary["action"], status: GitExecution): AgentDeveloperRepoSummary {
  const parsed = status.accepted
    ? parseGitStatusPorcelain(status.stdout)
    : { changedFiles: 0, stagedFiles: 0, unstagedFiles: 0, untrackedFiles: 0 };
  return developerRepoSummary({
    action,
    root: config.workspaceRoot,
    branch: parsed.branch,
    ahead: parsed.ahead,
    behind: parsed.behind,
    changedFiles: parsed.changedFiles,
    stagedFiles: parsed.stagedFiles,
    unstagedFiles: parsed.unstagedFiles,
    untrackedFiles: parsed.untrackedFiles,
    commandLine: status.commandLine,
    exitCode: status.exitCode,
    durationMs: status.durationMs
  });
}

function gitCommandFailure(config: AgentActionHostConfig, request: AgentActionRequest, routeId: string, execution: GitExecution): AgentActionResult {
  return result(config, request, {
    accepted: false,
    commandLine: execution.commandLine,
    routeId,
    exitCode: execution.exitCode,
    durationMs: execution.durationMs,
    timedOut: execution.timedOut,
    stdoutPreview: execution.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: execution.stderr.slice(0, MAX_PREVIEW_CHARS),
    failureCategory: execution.timedOut ? "timeout" : "command_error",
    verification: commandExitVerification({
      commandLine: execution.commandLine,
      accepted: false,
      exitCode: execution.exitCode,
      timedOut: execution.timedOut
    }),
    error: execution.error
  });
}

function trimmedGitHead(config: AgentActionHostConfig): string {
  const head = executeGit(config, ["rev-parse", "HEAD"]);
  return head.accepted ? head.stdout.trim() : "";
}

async function devGitCommitAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Git commit requires confirmed:true.", request)
    });
  }
  const message = (request.title ?? request.text ?? request.content ?? "").trim();
  if (!message) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "title, text or content is required as the Git commit message.", request)
    });
  }
  const requestedPaths = (request.paths ?? (request.path ? [request.path] : [])).map((path) => path.trim()).filter(Boolean);
  const beforeStatus = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  if (!beforeStatus.accepted) {
    return gitCommandFailure(config, request, "dev.git_commit.status_before", beforeStatus);
  }
  if (requestedPaths.length > 0) {
    const add = executeGit(config, ["add", "--", ...requestedPaths], 60_000);
    if (!add.accepted) {
      return gitCommandFailure(config, request, "dev.git_commit.add", add);
    }
  }
  const stagedStatus = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  const staged = stagedStatus.accepted ? parseGitStatusPorcelain(stagedStatus.stdout).stagedFiles : 0;
  if (staged <= 0) {
    return result(config, request, {
      accepted: false,
      routeId: "dev.git_commit",
      commandLine: stagedStatus.commandLine,
      exitCode: stagedStatus.exitCode,
      durationMs: stagedStatus.durationMs,
      stdoutPreview: stagedStatus.stdout.slice(0, MAX_PREVIEW_CHARS),
      failureCategory: "unverifiable",
      error: actionError("bad_payload", "No staged changes are available for commit; pass explicit paths or stage files first.", request),
      developer: gitStatusSummary(config, "git_commit", stagedStatus)
    });
  }
  const beforeHead = trimmedGitHead(config);
  const commit = executeGit(config, ["commit", "-m", message], 120_000);
  if (!commit.accepted) {
    return gitCommandFailure(config, request, "dev.git_commit", commit);
  }
  const afterHead = trimmedGitHead(config);
  const afterStatus = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  const summary = {
    ...gitStatusSummary(config, "git_commit", afterStatus),
    commitHash: afterHead,
    commandLine: commit.commandLine,
    exitCode: commit.exitCode,
    durationMs: commit.durationMs
  };
  const verification = verificationResult([
    commandExitProbe({ commandLine: commit.commandLine, accepted: commit.accepted, exitCode: commit.exitCode, timedOut: commit.timedOut }),
    verificationProbe({
      id: "git.commit.head_changed",
      kind: "artifact_hash",
      target: config.workspaceRoot,
      expectation: "HEAD changes after commit",
      actual: `before=${beforeHead || "none"} after=${afterHead || "none"}`,
      passed: afterHead.length > 0 && afterHead !== beforeHead
    }),
    verificationProbe({
      id: "git.commit.status_after",
      kind: "command_exit",
      target: afterStatus.commandLine,
      expectation: "post-commit status query exits with 0",
      actual: `exit_code=${afterStatus.exitCode ?? "unknown"}`,
      passed: afterStatus.accepted
    })
  ]);
  return result(config, request, {
    accepted: true,
    commandLine: commit.commandLine,
    routeId: "dev.git_commit",
    exitCode: commit.exitCode,
    durationMs: commit.durationMs,
    stdoutPreview: commit.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: commit.stderr.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`git_commit:${afterHead}`, `git_staged_after:${summary.stagedFiles}`, `git_changed_after:${summary.changedFiles}`],
    verification,
    developer: summary
  });
}

async function devGitPushAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Git push requires confirmed:true.", request)
    });
  }
  const status = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  if (!status.accepted) {
    return gitCommandFailure(config, request, "dev.git_push.status_before", status);
  }
  const parsed = parseGitStatusPorcelain(status.stdout);
  const branch = (request.headBranch ?? parsed.branch ?? "").trim();
  if (!branch) {
    return result(config, request, {
      accepted: false,
      failureCategory: "unverifiable",
      error: actionError("bad_payload", "headBranch is required when the current Git branch cannot be detected.", request),
      developer: gitStatusSummary(config, "git_push", status)
    });
  }
  const remote = (request.remote ?? "origin").trim();
  const head = trimmedGitHead(config);
  const push = executeGit(config, ["push", remote, branch], 120_000);
  if (!push.accepted) {
    return gitCommandFailure(config, request, "dev.git_push", push);
  }
  const lsRemote = executeGit(config, ["ls-remote", "--heads", remote, branch], 30_000);
  const remoteMatches = Boolean(head && lsRemote.accepted && lsRemote.stdout.includes(head));
  const afterStatus = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  const summary = {
    ...gitStatusSummary(config, "git_push", afterStatus),
    commitHash: head,
    remote,
    commandLine: push.commandLine,
    exitCode: push.exitCode,
    durationMs: push.durationMs
  };
  const verification = verificationResult([
    commandExitProbe({ commandLine: push.commandLine, accepted: push.accepted, exitCode: push.exitCode, timedOut: push.timedOut }),
    verificationProbe({
      id: "git.push.remote_head",
      kind: "artifact_hash",
      target: `${remote}/${branch}`,
      expectation: "remote branch head matches local HEAD",
      actual: `local=${head || "none"} remote_output_bytes=${Buffer.byteLength(lsRemote.stdout, "utf8")}`,
      passed: remoteMatches
    })
  ]);
  return result(config, request, {
    accepted: true,
    commandLine: push.commandLine,
    routeId: "dev.git_push",
    exitCode: push.exitCode,
    durationMs: push.durationMs,
    stdoutPreview: push.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: push.stderr.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`git_pushed:${remote}/${branch}`, `git_head:${head}`],
    verification,
    developer: summary
  });
}

function extractGithubPrUrl(output: string): string {
  return output.split(/\s+/).find((token) => /^https:\/\/github\.com\/[^/\s]+\/[^/\s]+\/pull\/\d+\/?$/.test(token.trim()))?.trim() ?? "";
}

async function devGithubPrCreateAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "GitHub PR creation requires confirmed:true.", request)
    });
  }
  const title = (request.title ?? "").trim();
  if (!title) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "title is required for GitHub PR creation.", request)
    });
  }
  const status = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  if (!status.accepted) {
    return gitCommandFailure(config, request, "dev.github_pr_create.status_before", status);
  }
  const parsed = parseGitStatusPorcelain(status.stdout);
  const headBranch = (request.headBranch ?? parsed.branch ?? "").trim();
  if (!headBranch) {
    return result(config, request, {
      accepted: false,
      failureCategory: "unverifiable",
      error: actionError("bad_payload", "headBranch is required when the current Git branch cannot be detected.", request),
      developer: gitStatusSummary(config, "github_pr_create", status)
    });
  }
  if ((parsed.ahead ?? 0) > 0) {
    return result(config, request, {
      accepted: false,
      failureCategory: "unverifiable",
      error: actionError("bad_payload", "Branch has unpushed commits; run confirmed dev_git_push before creating the PR.", { headBranch, ahead: parsed.ahead }),
      developer: gitStatusSummary(config, "github_pr_create", status)
    });
  }
  const auth = executeGh(config, ["auth", "status", "--active"], 15_000);
  if (!auth.accepted) {
    return gitCommandFailure(config, request, "dev.github_pr_create.auth", auth);
  }
  const args = ["pr", "create", "--title", title, "--body", request.content ?? request.text ?? "", "--head", headBranch];
  if (request.baseBranch?.trim()) {
    args.push("--base", request.baseBranch.trim());
  }
  if (request.draft === true) {
    args.push("--draft");
  }
  const created = executeGh(config, args, 120_000);
  if (!created.accepted) {
    return gitCommandFailure(config, request, "dev.github_pr_create", created);
  }
  const prUrl = extractGithubPrUrl(created.stdout);
  const viewed = prUrl ? executeGh(config, ["pr", "view", prUrl, "--json", "url,state,headRefName,baseRefName"], 30_000) : undefined;
  const viewMatches = Boolean(viewed?.accepted && prUrl && viewed.stdout.includes(prUrl) && viewed.stdout.includes(headBranch));
  const summary = {
    ...gitStatusSummary(config, "github_pr_create", status),
    prUrl,
    commandLine: created.commandLine,
    exitCode: created.exitCode,
    durationMs: created.durationMs
  };
  const verification = verificationResult([
    commandExitProbe({ commandLine: created.commandLine, accepted: created.accepted, exitCode: created.exitCode, timedOut: created.timedOut }),
    verificationProbe({
      id: "github.pr.url",
      kind: "artifact_hash",
      target: prUrl || "gh pr create stdout",
      expectation: "created PR URL is printed by gh",
      actual: prUrl || "missing",
      passed: Boolean(prUrl)
    }),
    verificationProbe({
      id: "github.pr.view",
      kind: "command_exit",
      target: viewed?.commandLine ?? "gh pr view",
      expectation: "gh pr view confirms URL and head branch",
      actual: `accepted=${viewed?.accepted ?? false} stdout_bytes=${Buffer.byteLength(viewed?.stdout ?? "", "utf8")}`,
      passed: viewMatches
    })
  ]);
  return result(config, request, {
    accepted: true,
    commandLine: created.commandLine,
    routeId: "dev.github_pr_create",
    exitCode: created.exitCode,
    durationMs: created.durationMs,
    stdoutPreview: created.stdout.slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: created.stderr.slice(0, MAX_PREVIEW_CHARS),
    artifacts: prUrl ? [prUrl] : [],
    observedChanges: prUrl ? [`github_pr:${prUrl}`, `github_head:${headBranch}`] : [`github_head:${headBranch}`],
    verification,
    developer: summary
  });
}

async function devRunCheckAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Developer checks require confirmed:true because they may write caches or build outputs.", request)
    });
  }
  const command = request.command?.trim() ?? "";
  if (!command) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "command is required for dev_run_check.", request)
    });
  }
  const execution = executeWindowsCommand(config, request, false);
  const status = executeGit(config, ["status", "--porcelain=v1", "-b"]);
  const parsed = status.accepted ? parseGitStatusPorcelain(status.stdout) : { changedFiles: 0, stagedFiles: 0, unstagedFiles: 0, untrackedFiles: 0 };
  const summary = developerRepoSummary({
    action: "run_check",
    root: config.workspaceRoot,
    branch: parsed.branch,
    ahead: parsed.ahead,
    behind: parsed.behind,
    changedFiles: parsed.changedFiles,
    stagedFiles: parsed.stagedFiles,
    unstagedFiles: parsed.unstagedFiles,
    untrackedFiles: parsed.untrackedFiles,
    commandLine: execution.commandLine,
    exitCode: execution.exitCode,
    durationMs: execution.durationMs
  });
  return result(config, request, {
    ...execution,
    routeId: "dev.run_check",
    developer: summary
  });
}

async function automationScheduleAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Creating a Windows scheduled automation requires confirmed:true.", request)
    });
  }
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Windows Task Scheduler is available only on win32.", request)
    });
  }
  const title = (request.title ?? request.query ?? request.text ?? "").trim();
  const command = request.command?.trim() ?? "";
  if (!title || !command) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "title and command are required for automation_schedule.", request)
    });
  }
  const taskPath = normalizeInGenTaskPath(request.taskName ?? defaultTaskName(title));
  if (typeof taskPath !== "string") {
    return result(config, request, { accepted: false, error: taskPath });
  }
  const scheduleType = (request.scheduleType ?? "ONCE").trim().toUpperCase();
  if (!["ONCE", "DAILY", "WEEKLY", "MONTHLY", "HOURLY", "MINUTE", "ONLOGON", "ONSTART"].includes(scheduleType)) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "scheduleType must be one of ONCE, DAILY, WEEKLY, MONTHLY, HOURLY, MINUTE, ONLOGON or ONSTART.", request)
    });
  }
  const taskRun = renderSchedulerTaskRun(command, request.args ?? []);
  const args = ["/Create", "/TN", taskPath, "/TR", taskRun, "/SC", scheduleType, "/F"];
  const startTime = request.startTime?.trim() || (["ONCE", "DAILY", "WEEKLY", "MONTHLY", "HOURLY", "MINUTE"].includes(scheduleType) ? defaultSchedulerStartTime() : "");
  if (startTime) {
    args.push("/ST", startTime);
  }
  if (request.startDate?.trim()) {
    args.push("/SD", request.startDate.trim());
  }
  const created = executeSchtasks(args, 60_000);
  if (!created.accepted) {
    return schedulerCommandFailure(config, request, "automation.schedule.create", created);
  }
  const queried = executeSchtasks(["/Query", "/TN", taskPath, "/FO", "CSV", "/V"], 30_000);
  const createdAt = new Date().toISOString();
  const id = createHash("sha256").update(`scheduled\n${taskPath}\n${createdAt}\n${taskRun}`).digest("hex").slice(0, 16);
  const schedulerSummary = schedulerTaskSummaryFromQuery(taskPath, queried);
  const entry = automationLedgerEntry({
    id,
    title,
    status: "scheduled",
    ledgerPath: pathLabel(config, { ...request, scope: "workspace" }, automationLedgerPath(config)),
    createdAt,
    backend: "windows_task_scheduler",
    taskName: basename(taskPath),
    taskPath,
    taskRun,
    scheduleType,
    startTime: startTime || undefined,
    nextRunTime: schedulerSummary.nextRunTime,
    schedulerStatus: schedulerSummary.schedulerStatus
  });
  const ledger = await appendAutomationLedger(config, entry);
  const verification = verificationResult([
    commandExitProbe({ commandLine: created.commandLine, accepted: created.accepted, exitCode: created.exitCode, timedOut: created.timedOut }),
    verificationProbe({
      id: "automation.scheduler.query",
      kind: "event_log",
      target: taskPath,
      expectation: "schtasks query returns the created task",
      actual: `accepted=${queried.accepted} stdout_bytes=${Buffer.byteLength(queried.stdout, "utf8")}`,
      passed: queried.accepted && queried.stdout.toLowerCase().includes(taskPath.toLowerCase())
    }),
    verificationProbe({
      id: "automation.ledger.hash",
      kind: "artifact_hash",
      target: ledger.ledgerPath,
      expectation: "ledger sha256 computed after scheduler mirror append",
      actual: ledger.ledgerHash,
      passed: /^[a-f0-9]{64}$/.test(ledger.ledgerHash)
    })
  ]);
  return result(config, request, {
    accepted: true,
    path: entry.taskPath,
    artifacts: [entry.ledgerPath],
    commandLine: created.commandLine,
    routeId: "automation.schedule",
    exitCode: created.exitCode,
    durationMs: created.durationMs,
    stdoutPreview: [created.stdout, queried.stdout].join("\n").slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: [created.stderr, queried.stderr].join("\n").slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`scheduler_task:${taskPath}`, `automation_status:scheduled`, `ledger_sha256:${ledger.ledgerHash}`],
    verification,
    automation: entry
  });
}

async function automationListAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Windows Task Scheduler is available only on win32.", request)
    });
  }
  const queried = executeSchtasks(["/Query", "/FO", "CSV", "/V"], 30_000);
  if (!queried.accepted) {
    return schedulerCommandFailure(config, request, "automation.list", queried);
  }
  const rows = schedulerTaskRows(queried.stdout).filter((row) =>
    (row["TaskName"] ?? "").replace(/^\\+/, "").toLowerCase().startsWith(INGEN_TASK_SCHEDULER_ROOT.toLowerCase())
  );
  const ledgerPath = automationLedgerPath(config);
  let ledgerHash = "missing";
  try {
    const ledgerBytes = await readFile(ledgerPath);
    ledgerHash = createHash("sha256").update(ledgerBytes).digest("hex");
  } catch {
    ledgerHash = "missing";
  }
  return result(config, request, {
    accepted: true,
    path: pathLabel(config, { ...request, scope: "workspace" }, ledgerPath),
    commandLine: queried.commandLine,
    routeId: "automation.list",
    exitCode: queried.exitCode,
    durationMs: queried.durationMs,
    stdoutPreview: rows.slice(0, request.maxResults ?? 25).map((row) => JSON.stringify(row)).join("\n").slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: queried.stderr.slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`scheduler_tasks:${rows.length}`, `ledger_sha256:${ledgerHash}`],
    verification: verificationResult([
      commandExitProbe({ commandLine: queried.commandLine, accepted: queried.accepted, exitCode: queried.exitCode, timedOut: queried.timedOut }),
      verificationProbe({
        id: "automation.list.filter",
        kind: "event_log",
        target: INGEN_TASK_SCHEDULER_ROOT,
        expectation: "query completed and InGen task filter was applied",
        actual: `rows=${rows.length}`,
        passed: true
      })
    ]),
    value: `scheduler tasks ${rows.length}`
  });
}

async function automationCancelAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Cancelling a Windows scheduled automation requires confirmed:true.", request)
    });
  }
  if (config.platform !== "win32") {
    return result(config, request, {
      accepted: false,
      failureCategory: "missing_tool",
      error: actionError("rust_unavailable", "Windows Task Scheduler is available only on win32.", request)
    });
  }
  const taskPath = normalizeInGenTaskPath(request.taskName ?? request.path ?? "");
  if (typeof taskPath !== "string") {
    return result(config, request, { accepted: false, error: taskPath });
  }
  const deleted = executeSchtasks(["/Delete", "/TN", taskPath, "/F"], 30_000);
  if (!deleted.accepted) {
    return schedulerCommandFailure(config, request, "automation.cancel.delete", deleted);
  }
  const queried = executeSchtasks(["/Query", "/TN", taskPath, "/FO", "CSV", "/V"], 15_000);
  const cancelledAt = new Date().toISOString();
  const entry = automationLedgerEntry({
    id: createHash("sha256").update(`cancelled\n${taskPath}\n${cancelledAt}`).digest("hex").slice(0, 16),
    title: request.title?.trim() || taskPath,
    status: "cancelled",
    ledgerPath: pathLabel(config, { ...request, scope: "workspace" }, automationLedgerPath(config)),
    createdAt: cancelledAt,
    cancelledAt,
    backend: "windows_task_scheduler",
    taskName: basename(taskPath),
    taskPath
  });
  const ledger = await appendAutomationLedger(config, entry);
  return result(config, request, {
    accepted: true,
    path: taskPath,
    artifacts: [entry.ledgerPath],
    commandLine: deleted.commandLine,
    routeId: "automation.cancel",
    exitCode: deleted.exitCode,
    durationMs: deleted.durationMs,
    stdoutPreview: [deleted.stdout, queried.stdout].join("\n").slice(0, MAX_PREVIEW_CHARS),
    stderrPreview: [deleted.stderr, queried.stderr].join("\n").slice(0, MAX_PREVIEW_CHARS),
    observedChanges: [`scheduler_task_deleted:${taskPath}`, `automation_status:cancelled`, `ledger_sha256:${ledger.ledgerHash}`],
    verification: verificationResult([
      commandExitProbe({ commandLine: deleted.commandLine, accepted: deleted.accepted, exitCode: deleted.exitCode, timedOut: deleted.timedOut }),
      verificationProbe({
        id: "automation.scheduler.deleted",
        kind: "event_log",
        target: taskPath,
        expectation: "post-delete query no longer returns the task",
        actual: `accepted=${queried.accepted} exit_code=${queried.exitCode ?? "unknown"}`,
        passed: !queried.accepted
      }),
      verificationProbe({
        id: "automation.ledger.cancel_hash",
        kind: "artifact_hash",
        target: ledger.ledgerPath,
        expectation: "ledger sha256 computed after cancellation append",
        actual: ledger.ledgerHash,
        passed: /^[a-f0-9]{64}$/.test(ledger.ledgerHash)
      })
    ]),
    automation: entry
  });
}

async function automationRecordAction(config: AgentActionHostConfig, request: AgentActionRequest): Promise<AgentActionResult> {
  if (request.confirmed !== true) {
    return result(config, request, {
      accepted: false,
      failureCategory: "denied",
      error: actionError("bad_payload", "Recording a persistent automation goal requires confirmed:true.", request)
    });
  }
  const title = (request.title ?? request.query ?? request.text ?? request.content ?? "").trim();
  if (!title) {
    return result(config, request, {
      accepted: false,
      error: actionError("bad_payload", "title, query, text or content is required for automation_record.", request)
    });
  }
  const ledgerPath = automationLedgerPath(config);
  const createdAt = new Date().toISOString();
  const id = createHash("sha256").update(`automation\n${createdAt}\n${title}`).digest("hex").slice(0, 16);
  const entry = automationLedgerEntry({
    id,
    title,
    status: "recorded",
    ledgerPath: pathLabel(config, { ...request, scope: "workspace" }, ledgerPath),
    createdAt,
    backend: "ledger"
  });
  const ledger = await appendAutomationLedger(config, entry);
  return result(config, request, {
    accepted: true,
    path: entry.ledgerPath,
    artifacts: [entry.ledgerPath],
    observedChanges: [`automation_record:${entry.id}`, `ledger_sha256:${ledger.ledgerHash}`],
    verification: verificationResult([
      await filesystemProbe("automation.ledger.exists", ledgerPath, "file"),
      verificationProbe({
        id: "automation.ledger.hash",
        kind: "artifact_hash",
        target: ledgerPath,
        expectation: "ledger sha256 computed after append",
        actual: ledger.ledgerHash,
        passed: /^[a-f0-9]{64}$/.test(ledger.ledgerHash)
      })
    ]),
    automation: entry,
    value: charDeltaValue(JSON.stringify(entry).length + 1, 0)
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
      case "computer_inspect":
        return await computerInspectAction(config, request);
      case "computer_appshot":
        return await computerAppshotAction(config, request);
      case "computer_focus_window":
        return await computerFocusWindowAction(config, request);
      case "computer_clipboard_read":
        return await computerClipboardReadAction(config, request);
      case "computer_clipboard_write":
        return await computerClipboardWriteAction(config, request);
      case "computer_ui_tree":
        return await computerUiTreeAction(config, request);
      case "computer_ocr":
        return await computerOcrAction(config, request);
      case "computer_click":
      case "computer_type_text":
      case "computer_scroll":
      case "computer_drag":
        return await computerInputAction(config, request);
      case "browser_inspect_url":
        return await browserInspectUrlAction(config, request);
      case "browser_download":
        return await browserDownloadAction(config, request);
      case "browser_open_url":
        return await browserOpenUrlAction(config, request);
      case "browser_playwright_inspect":
        return await browserPlaywrightInspectAction(config, request);
      case "browser_screenshot":
        return await browserScreenshotAction(config, request);
      case "browser_click":
        return await browserClickAction(config, request);
      case "browser_type_text":
        return await browserTypeTextAction(config, request);
      case "browser_playwright_download":
        return await browserPlaywrightDownloadAction(config, request);
      case "document_inspect":
        return await documentInspectAction(config, request);
      case "document_write_text":
        return await documentWriteTextAction(config, request);
      case "document_write_json":
        return await documentWriteJsonAction(config, request);
      case "document_write_csv":
        return await documentWriteCsvAction(config, request);
      case "document_convert_text":
        return await documentConvertTextAction(config, request);
      case "dev_repo_status":
        return await devRepoStatusAction(config, request);
      case "dev_git_diff":
        return await devGitDiffAction(config, request);
      case "dev_git_commit":
        return await devGitCommitAction(config, request);
      case "dev_git_push":
        return await devGitPushAction(config, request);
      case "dev_github_pr_create":
        return await devGithubPrCreateAction(config, request);
      case "dev_run_check":
        return await devRunCheckAction(config, request);
      case "virtualization_inspect":
        return await virtualizationInspectAction(config, request);
      case "virtualization_run_command":
        return await virtualizationRunCommandAction(config, request);
      case "automation_schedule":
        return await automationScheduleAction(config, request);
      case "automation_list":
        return await automationListAction(config, request);
      case "automation_cancel":
        return await automationCancelAction(config, request);
      case "automation_record":
        return await automationRecordAction(config, request);
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
