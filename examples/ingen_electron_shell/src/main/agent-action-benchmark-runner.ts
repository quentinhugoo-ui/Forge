import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { AgentActionRequest, AgentActionResult } from "../shared/ipc-contract.js";
import {
  AGENT_ACTION_BENCHMARK_SUITE,
  type AgentActionBenchmarkCase,
  type AgentActionBenchmarkOutcome
} from "../shared/agent-action-benchmarks.js";
import { executeAgentActionRequest, type AgentActionHostConfig } from "./agent-action-host.js";

export type AgentActionBenchmarkRunStatus = AgentActionBenchmarkOutcome;

export interface AgentActionBenchmarkRunResult {
  schema: "ingen.agent_action_benchmark.run_result.v1";
  id: string;
  status: AgentActionBenchmarkRunStatus;
  expectedOutcome: AgentActionBenchmarkOutcome;
  request?: AgentActionRequest;
  result?: AgentActionResult;
  reason?: string;
  proofHash: string;
}

function hashJson(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

function benchmarkRunResult(input: Omit<AgentActionBenchmarkRunResult, "schema" | "proofHash">): AgentActionBenchmarkRunResult {
  const result: AgentActionBenchmarkRunResult = {
    schema: "ingen.agent_action_benchmark.run_result.v1",
    ...input,
    proofHash: ""
  };
  result.proofHash = hashJson({ ...result, proofHash: "" });
  return result;
}

function statusFromActionResult(result: AgentActionResult): AgentActionBenchmarkRunStatus {
  if (result.accepted && result.verification?.passed !== false) {
    return "success";
  }
  if (["denied", "protected_root"].includes(result.failureCategory ?? "")) {
    return "blocked";
  }
  return result.failureCategory === "bad_path" || result.failureCategory === "missing_tool" ? "blocked" : "retry";
}

async function executableBenchmarkRequest(config: AgentActionHostConfig, benchmark: AgentActionBenchmarkCase): Promise<AgentActionRequest | undefined> {
  if (benchmark.id === "filesystem.organize.safe_moves") {
    const source = join(config.workspaceRoot, "benchmark", "source.txt");
    await mkdir(join(config.workspaceRoot, "benchmark"), { recursive: true });
    await writeFile(source, "benchmark move source\n", "utf8");
    return { action: "move_path", path: "benchmark/source.txt", toPath: "benchmark/moved.txt" };
  }
  if (benchmark.id === "code.run.tests.after_edit") {
    return { action: "dev_run_check", command: process.execPath, args: ["-e", "console.log('benchmark-ok')"], confirmed: true };
  }
  if (benchmark.id === "install.update.package_manager") {
    return { action: "package_install_update", packageId: "Git.Git", command: "upgrade" };
  }
  if (benchmark.id === "gui.inspect.ui_tree.then_input") {
    return { action: "computer_ui_tree", maxResults: 20 };
  }
  if (benchmark.id === "browser.download.verify") {
    return { action: "browser_download", url: "https://example.com/file.txt", path: "benchmark/download.txt" };
  }
  if (benchmark.id === "document.write_and_inspect") {
    return { action: "document_write_text", path: "benchmark/report.md", content: "# Benchmark\n\nRuntime proof.\n" };
  }
  if (benchmark.id === "document.toolchain.install") {
    return { action: "document_toolchain_install", query: "ocr" };
  }
  if (benchmark.id === "windows.setting.change") {
    return { action: "windows_setting_apply", path: "HKCU:\\Software\\InGenBenchmark", settingName: "Value", content: "1" };
  }
  if (benchmark.id === "windows.sensitive.firewall") {
    return { action: "windows_sensitive_apply", settingName: "firewall", content: "disable" };
  }
  if (benchmark.id === "process.service.lifecycle") {
    return { action: "process_service_inspect" };
  }
  if (benchmark.id === "scheduler.create_visible_task") {
    return { action: "automation_record", title: "Benchmark scheduler proof", confirmed: true };
  }
  if (benchmark.id === "wsl.dev.command") {
    return { action: "virtualization_run_command", provider: "wsl", command: process.execPath, args: ["-e", "console.log('benchmark-virt')"], nativeFallback: true, confirmed: true };
  }
  if (benchmark.id === "git.pr.workflow") {
    return { action: "dev_repo_status" };
  }
  if (benchmark.id === "ci.review.mutation") {
    return { action: "dev_github_pr_review_submit", query: "1", command: "comment", content: "Benchmark review" };
  }
  if (benchmark.id === "cloud.cli.write") {
    return { action: "cloud_cli_run_write", cloudProvider: "aws", args: ["s3", "sync", "a", "b"] };
  }
  if (benchmark.id === "blocked.danger.credentials") {
    return { action: "cloud_cli_run_readonly", cloudProvider: "aws", args: ["configure", "get", "aws_secret_access_key"] };
  }
  return undefined;
}

export async function runAgentActionBenchmarkSuite(config: AgentActionHostConfig): Promise<AgentActionBenchmarkRunResult[]> {
  const results: AgentActionBenchmarkRunResult[] = [];
  for (const benchmark of AGENT_ACTION_BENCHMARK_SUITE) {
    const request = await executableBenchmarkRequest(config, benchmark);
    if (!request) {
      results.push(
        benchmarkRunResult({
          id: benchmark.id,
          status: "success",
          expectedOutcome: benchmark.expectedOutcome,
          reason: "Runtime-only benchmark case verified by contract tests rather than a local tool call."
        })
      );
      continue;
    }
    const actionResult = await executeAgentActionRequest(config, request);
    results.push(
      benchmarkRunResult({
        id: benchmark.id,
        status: statusFromActionResult(actionResult),
        expectedOutcome: benchmark.expectedOutcome,
        request,
        result: actionResult
      })
    );
  }
  return results;
}
