import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  agentActionEventCommandForRequest,
  executeAgentActionRequest,
  type AgentActionHostConfig
} from "../src/main/agent-action-host";
import {
  agentActionLiveVisibleText,
  extractAgentActionJsonRequest
} from "../src/main/agent-action-loop";
import type { AgentActionKind, AgentActionRequest, AgentActionResult } from "../src/shared/ipc-contract";

type ScenarioExpectation = "must_accept" | "must_block" | "may_accept_or_block";

interface ExhaustiveScenario {
  paragraph: string;
  expectation: ScenarioExpectation;
  request: AgentActionRequest;
  prepare?: (config: AgentActionHostConfig) => Promise<void>;
}

const DATA_URL = "data:text/html,<html><head><title>Agent Matrix</title></head><body><button>Run</button><input><a download='matrix.txt' href='data:text/plain,matrix'>Download</a></body></html>";

const PNG_1X1 = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/l6s9TAAAAABJRU5ErkJggg==",
  "base64"
);

function minimalPdf(text: string): string {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
    `<< /Length ${text.length + 32} >>\nstream\nBT /F1 18 Tf 40 100 Td (${text}) Tj ET\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
  ];
  let pdf = "%PDF-1.4\n";
  const offsets: number[] = [];
  for (let index = 0; index < objects.length; index += 1) {
    offsets.push(Buffer.byteLength(pdf, "utf8"));
    pdf += `${index + 1} 0 obj\n${objects[index]}\nendobj\n`;
  }
  const xrefOffset = Buffer.byteLength(pdf, "utf8");
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  pdf += offsets.map((offset) => `${offset.toString().padStart(10, "0")} 00000 n \n`).join("");
  pdf += `trailer\n<< /Root 1 0 R /Size ${objects.length + 1} >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return pdf;
}

async function writeFixture(config: AgentActionHostConfig, path: string, content: string | Buffer): Promise<void> {
  const fullPath = join(config.workspaceRoot, path);
  await mkdir(join(fullPath, ".."), { recursive: true });
  await writeFile(fullPath, content);
}

async function prepareGitRepo(config: AgentActionHostConfig): Promise<void> {
  spawnSync("git", ["init"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
  spawnSync("git", ["config", "user.email", "agent@example.test"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
  spawnSync("git", ["config", "user.name", "Agent Matrix"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
  await writeFixture(config, "tracked.txt", "one\n");
  spawnSync("git", ["add", "tracked.txt"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
  spawnSync("git", ["commit", "-m", "Initial"], { cwd: config.workspaceRoot, encoding: "utf8", stdio: "pipe" });
  await writeFixture(config, "tracked.txt", "one\ntwo\n");
  await writeFixture(config, "untracked.txt", "new\n");
}

async function prepareBaseWorkspace(config: AgentActionHostConfig): Promise<void> {
  await mkdir(join(config.workspaceRoot, "docs"), { recursive: true });
  await mkdir(join(config.workspaceRoot, "copied"), { recursive: true });
  await writeFixture(config, "docs/sample.md", "# Matrix\n\nneedle\n");
  await writeFixture(config, "docs/scan.png", PNG_1X1);
  await writeFixture(config, "docs/sample.pdf", minimalPdf("Matrix PDF"));
  await writeFixture(config, "copy-source.txt", "copy\n");
  await prepareGitRepo(config);
}

async function withTempWorkspace<T>(run: (config: AgentActionHostConfig) => Promise<T>): Promise<T> {
  const root = join(tmpdir(), `ingen-agent-action-matrix-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  await mkdir(root, { recursive: true });
  const config: AgentActionHostConfig = {
    workspaceRoot: root,
    workspaceActive: true,
    cwd: root,
    platform: process.platform
  };
  try {
    await prepareBaseWorkspace(config);
    return await run(config);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

const SCENARIOS: Record<AgentActionKind, ExhaustiveScenario> = {
  list: {
    paragraph: "Je liste le workspace temporaire pour obtenir un etat de depart borne.",
    expectation: "must_accept",
    request: { action: "list", path: "." }
  },
  search: {
    paragraph: "Je cherche une chaine connue dans les fixtures pour verifier la recherche locale.",
    expectation: "must_accept",
    request: { action: "search", path: ".", query: "needle" }
  },
  create_directory: {
    paragraph: "Je cree un dossier local dans le workspace temporaire.",
    expectation: "must_accept",
    request: { action: "create_directory", path: "created-dir" }
  },
  rename_path: {
    paragraph: "Je renomme un fichier fixture et je verifie la destination.",
    expectation: "must_accept",
    request: { action: "rename_path", path: "rename-source.txt", toPath: "renamed.txt" },
    prepare: (config) => writeFixture(config, "rename-source.txt", "rename\n")
  },
  move_path: {
    paragraph: "Je deplace un fichier fixture vers un dossier deja prepare.",
    expectation: "must_accept",
    request: { action: "move_path", path: "move-source.txt", toPath: "moved/move-source.txt" },
    prepare: async (config) => {
      await mkdir(join(config.workspaceRoot, "moved"), { recursive: true });
      await writeFixture(config, "move-source.txt", "move\n");
    }
  },
  copy_path: {
    paragraph: "Je copie un fichier fixture sans toucher aux fichiers utilisateur.",
    expectation: "must_accept",
    request: { action: "copy_path", path: "copy-source.txt", toPath: "copied/copy-source.txt" }
  },
  delete_empty_directory: {
    paragraph: "Je supprime un dossier vide cree pour ce test.",
    expectation: "must_accept",
    request: { action: "delete_empty_directory", path: "empty-dir", confirmed: true },
    prepare: async (config) => {
      await mkdir(join(config.workspaceRoot, "empty-dir"), { recursive: true });
    }
  },
  delete_tree: {
    paragraph: "Je supprime un petit arbre local avec confirmation explicite et garde de chemin.",
    expectation: "must_accept",
    request: { action: "delete_tree", path: "tree-dir", confirmed: true, recursive: true },
    prepare: (config) => writeFixture(config, "tree-dir/nested/file.txt", "delete tree\n")
  },
  run_readonly_command: {
    paragraph: "Je lance une commande shell read-only autorisee.",
    expectation: "must_accept",
    request: { action: "run_readonly_command", command: "rg", args: ["--files", "."] }
  },
  run_command: {
    paragraph: "Je lance une commande confirmee inoffensive dans le workspace temporaire.",
    expectation: "must_accept",
    request: { action: "run_command", command: process.execPath, args: ["-e", "console.log('shell-ok')"], confirmed: true }
  },
  computer_inspect: {
    paragraph: "J'inspecte l'etat GUI sans interaction utilisateur.",
    expectation: "must_accept",
    request: { action: "computer_inspect" }
  },
  computer_appshot: {
    paragraph: "Je tente une capture appshot sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "computer_appshot" }
  },
  computer_focus_window: {
    paragraph: "Je tente un focus de fenetre sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "computer_focus_window", windowTitle: "InGen" }
  },
  computer_clipboard_read: {
    paragraph: "Je tente une lecture clipboard sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "computer_clipboard_read" }
  },
  computer_clipboard_write: {
    paragraph: "Je tente une ecriture clipboard sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "computer_clipboard_write", text: "blocked" }
  },
  computer_ui_tree: {
    paragraph: "J'inspecte l'arbre UI Automation quand le backend Windows le permet.",
    expectation: "may_accept_or_block",
    request: { action: "computer_ui_tree" }
  },
  computer_ocr: {
    paragraph: "Je tente l'OCR ecran sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "computer_ocr" }
  },
  computer_click: {
    paragraph: "Je tente un clic sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "computer_click", x: 1, y: 1 }
  },
  computer_type_text: {
    paragraph: "Je tente une saisie clavier sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "computer_type_text", text: "blocked" }
  },
  computer_scroll: {
    paragraph: "Je tente un scroll sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "computer_scroll", deltaY: 1 }
  },
  computer_drag: {
    paragraph: "Je tente un drag-drop sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "computer_drag", x: 1, y: 1, toX: 2, toY: 2 }
  },
  browser_inspect_url: {
    paragraph: "J'inspecte une page data URL locale sans navigation externe.",
    expectation: "must_accept",
    request: { action: "browser_inspect_url", url: DATA_URL }
  },
  browser_download: {
    paragraph: "Je telecharge une data URL dans le workspace avec hash d'artefact.",
    expectation: "must_accept",
    request: { action: "browser_download", url: "data:text/plain,matrix-download", path: "downloads/data.txt", confirmed: true }
  },
  browser_open_url: {
    paragraph: "Je tente une navigation navigateur sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "browser_open_url", url: DATA_URL }
  },
  browser_playwright_inspect: {
    paragraph: "J'inspecte DOM/network via Playwright si le runtime navigateur est disponible.",
    expectation: "may_accept_or_block",
    request: { action: "browser_playwright_inspect", url: DATA_URL }
  },
  browser_screenshot: {
    paragraph: "Je tente un screenshot navigateur sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "browser_screenshot", url: DATA_URL, path: "shots/page.png" }
  },
  browser_click: {
    paragraph: "Je tente un clic navigateur sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "browser_click", url: DATA_URL, selector: "button" }
  },
  browser_type_text: {
    paragraph: "Je tente une saisie navigateur sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "browser_type_text", url: DATA_URL, selector: "input", text: "blocked" }
  },
  browser_playwright_download: {
    paragraph: "Je tente un download Playwright sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "browser_playwright_download", url: DATA_URL, selector: "a[download]", path: "downloads/playwright.txt" }
  },
  document_inspect: {
    paragraph: "J'inspecte un markdown fixture avec resume media.",
    expectation: "must_accept",
    request: { action: "document_inspect", path: "docs/sample.md" }
  },
  document_write_text: {
    paragraph: "J'ecris un fichier texte et je verifie son hash.",
    expectation: "must_accept",
    request: { action: "document_write_text", path: "docs/write-text.txt", content: "text\n" }
  },
  document_write_json: {
    paragraph: "J'ecris du JSON et je verifie le parsing.",
    expectation: "must_accept",
    request: { action: "document_write_json", path: "docs/write-json.json", content: "{\"ok\":true}" }
  },
  document_write_csv: {
    paragraph: "J'ecris du CSV et je verifie les lignes/colonnes.",
    expectation: "must_accept",
    request: { action: "document_write_csv", path: "docs/write-csv.csv", content: "name,value\nalpha,1\n" }
  },
  document_convert_text: {
    paragraph: "Je convertis un markdown en texte lisible.",
    expectation: "must_accept",
    request: { action: "document_convert_text", path: "docs/sample.md", toPath: "docs/sample.txt" }
  },
  document_pdf_extract_text: {
    paragraph: "J'extrais le texte d'un PDF fixture ou je bloque proprement selon le backend.",
    expectation: "may_accept_or_block",
    request: { action: "document_pdf_extract_text", path: "docs/sample.pdf", toPath: "docs/sample-pdf.txt" }
  },
  document_office_inspect: {
    paragraph: "Je tente Office COM sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "document_office_inspect", path: "docs/sample.docx" }
  },
  document_office_export_pdf: {
    paragraph: "Je tente un export Office PDF sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "document_office_export_pdf", path: "docs/sample.docx", toPath: "docs/sample.docx.pdf" }
  },
  document_image_ocr: {
    paragraph: "Je tente OCR image sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "document_image_ocr", path: "docs/scan.png" }
  },
  document_media_metadata: {
    paragraph: "J'inspecte les metadonnees media si ffprobe est disponible.",
    expectation: "may_accept_or_block",
    request: { action: "document_media_metadata", path: "docs/scan.png" }
  },
  document_toolchain_inspect: {
    paragraph: "J'inspecte la toolchain document/OCR/media/Office.",
    expectation: "must_accept",
    request: { action: "document_toolchain_inspect", query: "all" }
  },
  document_toolchain_install: {
    paragraph: "Je tente l'installation toolchain sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "document_toolchain_install", query: "ocr" }
  },
  dev_repo_status: {
    paragraph: "J'inspecte le status Git du repo temporaire.",
    expectation: "must_accept",
    request: { action: "dev_repo_status" }
  },
  dev_git_diff: {
    paragraph: "J'inspecte le diff Git du repo temporaire.",
    expectation: "must_accept",
    request: { action: "dev_git_diff" }
  },
  dev_git_commit: {
    paragraph: "Je tente un commit sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "dev_git_commit", title: "Blocked commit", paths: ["tracked.txt"] }
  },
  dev_git_push: {
    paragraph: "Je tente un push sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "dev_git_push" }
  },
  dev_github_pr_create: {
    paragraph: "Je tente une creation PR sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "dev_github_pr_create", title: "Blocked PR", baseBranch: "master", headBranch: "agent-test" }
  },
  dev_github_pr_review_submit: {
    paragraph: "Je tente une review GitHub sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "dev_github_pr_review_submit", query: "1", command: "comment", content: "Blocked review." }
  },
  dev_run_check: {
    paragraph: "Je lance un check developpeur confirme et inoffensif.",
    expectation: "must_accept",
    request: { action: "dev_run_check", command: process.execPath, args: ["-e", "console.log('check-ok')"], confirmed: true }
  },
  cloud_cli_inspect: {
    paragraph: "J'inspecte les CLIs cloud sans extraire de credentials.",
    expectation: "must_accept",
    request: { action: "cloud_cli_inspect", cloudProvider: "all" }
  },
  cloud_cli_run_readonly: {
    paragraph: "Je lance une commande cloud read-only si le CLI GitHub est disponible.",
    expectation: "may_accept_or_block",
    request: { action: "cloud_cli_run_readonly", cloudProvider: "github", args: ["--version"] }
  },
  cloud_cli_run_write: {
    paragraph: "Je tente une commande cloud write sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "cloud_cli_run_write", cloudProvider: "github", args: ["repo", "edit"] }
  },
  windows_setting_inspect: {
    paragraph: "J'inspecte un etat Windows read-only.",
    expectation: "must_accept",
    request: { action: "windows_setting_inspect", settingName: "os" }
  },
  windows_setting_apply: {
    paragraph: "Je tente une mutation setting Windows sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "windows_setting_apply", path: "HKCU:\\Software\\InGenAgentMatrix", settingName: "Blocked", content: "1" }
  },
  windows_sensitive_inspect: {
    paragraph: "J'inspecte une variable utilisateur en read-only.",
    expectation: "must_accept",
    request: { action: "windows_sensitive_inspect", settingName: "user_env", query: "PATH" }
  },
  windows_sensitive_apply: {
    paragraph: "Je tente une mutation Defender confirmee mais dangereuse, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "windows_sensitive_apply", settingName: "defender", content: "disable", confirmed: true }
  },
  process_service_inspect: {
    paragraph: "J'inspecte le processus courant via Windows.",
    expectation: "must_accept",
    request: { action: "process_service_inspect" }
  },
  process_service_control: {
    paragraph: "Je tente un controle service sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "process_service_control", serviceName: "Spooler", command: "restart" }
  },
  package_inspect: {
    paragraph: "J'inspecte le package manager sans installer.",
    expectation: "must_accept",
    request: { action: "package_inspect" }
  },
  package_install_update: {
    paragraph: "Je tente une installation package sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "package_install_update", packageId: "Git.Git", command: "upgrade" }
  },
  ci_checks_inspect: {
    paragraph: "J'inspecte les checks CI si gh et le contexte repo le permettent.",
    expectation: "may_accept_or_block",
    request: { action: "ci_checks_inspect", maxResults: 3 }
  },
  ci_run_inspect: {
    paragraph: "J'inspecte les runs CI si gh et le contexte repo le permettent.",
    expectation: "may_accept_or_block",
    request: { action: "ci_run_inspect", maxResults: 3 }
  },
  ci_rerun_failed: {
    paragraph: "Je tente un rerun CI sans confirmation, il doit rester bloque.",
    expectation: "must_block",
    request: { action: "ci_rerun_failed", query: "123" }
  },
  virtualization_inspect: {
    paragraph: "J'inspecte WSL, Docker et Hyper-V sans mutation.",
    expectation: "must_accept",
    request: { action: "virtualization_inspect", provider: "all" }
  },
  virtualization_run_command: {
    paragraph: "Je lance une commande virtualisation via fallback natif confirme et verifie.",
    expectation: "must_accept",
    request: {
      action: "virtualization_run_command",
      provider: "docker",
      container: "ingen-missing-container-for-fallback",
      command: process.execPath,
      args: ["-e", "console.log('fallback-ok')"],
      nativeFallback: true,
      confirmed: true
    }
  },
  automation_schedule: {
    paragraph: "Je tente une creation Task Scheduler sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: {
      action: "automation_schedule",
      title: "Blocked scheduler",
      command: "cmd.exe",
      args: ["/d", "/s", "/c", "echo blocked"],
      taskName: "InGenAgent_MatrixBlocked",
      scheduleType: "ONLOGON"
    }
  },
  automation_list: {
    paragraph: "J'inspecte les automations persistantes InGen.",
    expectation: "may_accept_or_block",
    request: { action: "automation_list", maxResults: 10 }
  },
  automation_cancel: {
    paragraph: "Je tente une annulation automation sans confirmation, elle doit rester bloquee.",
    expectation: "must_block",
    request: { action: "automation_cancel", taskName: "InGenAgent_MatrixBlocked" }
  },
  automation_record: {
    paragraph: "J'enregistre une intention d'automation dans le ledger local.",
    expectation: "must_accept",
    request: { action: "automation_record", title: "Matrix reminder", content: "Record only.", confirmed: true }
  }
};

function assertResultMatchesExpectation(result: AgentActionResult, expectation: ScenarioExpectation): void {
  expect(result.schema).toBe("ingen.agent_action_host.result.v1");
  expect(result.proofHash).toMatch(/^[a-f0-9]{64}$/);
  expect(result.audit?.schema).toBe("ingen.agent_runtime_audit.summary.v1");
  expect(result.audit?.logSha256).toMatch(/^[a-f0-9]{64}$/);
  if (expectation === "must_accept") {
    expect(result.accepted, result.error?.message).toBe(true);
    expect(result.error).toBeUndefined();
    expect(result.verification?.passed).not.toBe(false);
    return;
  }
  if (expectation === "must_block") {
    expect(result.accepted).toBe(false);
    expect(result.error?.message || result.failureCategory || result.userPresenceRequired).toBeTruthy();
    return;
  }
  if (result.accepted) {
    expect(result.error).toBeUndefined();
    expect(result.verification?.passed).not.toBe(false);
  } else {
    expect(result.error?.message || result.failureCategory || result.userPresenceRequired).toBeTruthy();
  }
}

describe("agent action exhaustive capability matrix", () => {
  it("covers every AgentActionKind through the loop marker, event command and host result path", async () => {
    await withTempWorkspace(async (config) => {
      const scenarios = Object.values(SCENARIOS);
      const uniqueActions = new Set(scenarios.map((scenario) => scenario.request.action));
      expect(uniqueActions.size).toBe(scenarios.length);
      expect(scenarios.length).toBe(68);

      const runtimeEvents: Array<{ kind: "tool_call_started" | "tool_result" | "tool_call_completed"; command: string; accepted?: boolean }> = [];
      const results: AgentActionResult[] = [];

      for (const scenario of scenarios) {
        await scenario.prepare?.(config);
        const streamed = `${scenario.paragraph}\nAGENT_ACTION_JSON ${JSON.stringify(scenario.request)}`;
        expect(agentActionLiveVisibleText(streamed)).toBe(scenario.paragraph);
        const extracted = extractAgentActionJsonRequest(streamed);
        expect(extracted?.request).toEqual(scenario.request);
        const command = agentActionEventCommandForRequest(scenario.request);
        expect(command).toMatch(/^\/agent_/);

        runtimeEvents.push({ kind: "tool_call_started", command });
        const result = await executeAgentActionRequest(config, extracted!.request);
        runtimeEvents.push({ kind: "tool_result", command, accepted: result.accepted });
        runtimeEvents.push({ kind: "tool_call_completed", command, accepted: result.accepted });
        results.push(result);

        expect(result.action).toBe(scenario.request.action);
        assertResultMatchesExpectation(result, scenario.expectation);
      }

      expect(runtimeEvents).toHaveLength(scenarios.length * 3);
      expect(results.some((result) => result.accepted)).toBe(true);
      expect(results.some((result) => !result.accepted)).toBe(true);
      const auditLog = await readFile(join(config.workspaceRoot, ".ingen-agent-artifacts", "agent-action-runtime.jsonl"), "utf8");
      expect(auditLog).toContain('"kind":"started"');
      expect(auditLog).toContain('"kind":"result"');
      expect(auditLog).toContain('"kind":"blocked"');
      expect(auditLog).toContain('"kind":"verification"');
      expect(auditLog).toContain('"kind":"summary"');
    });
  }, 90_000);
});
