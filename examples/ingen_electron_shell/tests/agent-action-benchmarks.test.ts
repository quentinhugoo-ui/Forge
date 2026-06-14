import { mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { runAgentActionBenchmarkSuite } from "../src/main/agent-action-benchmark-runner";
import {
  AGENT_ACTION_BENCHMARK_SUITE,
  REQUIRED_AGENT_ACTION_BENCHMARK_SURFACES,
  agentActionBenchmarkSurfaceCoverage,
  missingAgentActionBenchmarkSurfaces
} from "../src/shared/agent-action-benchmarks";
import type { AgentActionHostConfig } from "../src/main/agent-action-host";

async function withTempWorkspace<T>(run: (config: AgentActionHostConfig) => Promise<T>): Promise<T> {
  const root = join(tmpdir(), `ingen-agent-action-benchmark-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  await mkdir(root, { recursive: true });
  try {
    return await run({
      workspaceRoot: root,
      workspaceActive: true,
      cwd: root,
      platform: process.platform
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

describe("agent action benchmark suite", () => {
  it("covers every required local-agent surface", () => {
    expect(missingAgentActionBenchmarkSurfaces()).toEqual([]);
    const coverage = agentActionBenchmarkSurfaceCoverage();
    for (const surface of REQUIRED_AGENT_ACTION_BENCHMARK_SURFACES) {
      expect(coverage[surface]).toBeGreaterThanOrEqual(1);
    }
  });

  it("requires runtime evidence instead of fake event prose", () => {
    for (const benchmark of AGENT_ACTION_BENCHMARK_SUITE) {
      expect(benchmark.id).toMatch(/^[a-z0-9_.]+$/);
      expect(benchmark.eventLabel).toMatch(/^[A-Z_a-z0-9 /-]+$/);
      expect(benchmark.evidence.length).toBeGreaterThan(0);
      expect(`${benchmark.proofRule} ${benchmark.evidence.join(" ")} ${benchmark.primaryActionId}`).toMatch(
        /\b(?:AGENT_ACTION_RESULT|runtime|runtime_event|verification|command|command_exit|probe|block|filesystem|artifact_hash|policy_block|summary|Compaction)\b/i
      );
      expect(benchmark.observedState).not.toMatch(/^\s*$/);
    }
  });

  it("models success, retry and blocked outcomes", () => {
    const outcomes = new Set(AGENT_ACTION_BENCHMARK_SUITE.map((benchmark) => benchmark.expectedOutcome));
    expect(outcomes).toEqual(new Set(["success", "retry", "blocked"]));
  });

  it("keeps risky and unavailable work behind approval or policy blocks", () => {
    const sensitive = AGENT_ACTION_BENCHMARK_SUITE.filter((benchmark) =>
      /install|setting|service|scheduler|wsl|cloud|blocked/.test(benchmark.surface)
    );
    expect(sensitive.length).toBeGreaterThanOrEqual(6);
    for (const benchmark of sensitive) {
      expect(["prompt", "blocked"]).toContain(benchmark.approval);
    }
    expect(AGENT_ACTION_BENCHMARK_SUITE.find((benchmark) => benchmark.id === "blocked.danger.credentials")).toMatchObject({
      expectedOutcome: "blocked",
      approval: "blocked",
      evidence: ["policy_block"],
      observedState: "no command is executed"
    });
  });

  it("keeps UX guarantees in the benchmark contract", () => {
    expect(AGENT_ACTION_BENCHMARK_SUITE.find((benchmark) => benchmark.surface === "context_compaction")).toMatchObject({
      eventCommand: "/context_compaction_",
      eventLabel: "Context compressed"
    });
    expect(AGENT_ACTION_BENCHMARK_SUITE.find((benchmark) => benchmark.surface === "final_summary")).toMatchObject({
      primaryActionId: "runtime:final_summary",
      eventLabel: "agent loop summarized"
    });
    expect(AGENT_ACTION_BENCHMARK_SUITE.find((benchmark) => benchmark.id === "document.write_and_inspect")?.proofRule).toContain("result.value");
  });

  it("runs available benchmark cases and fails blocked/runtime-only cases cleanly", async () => {
    await withTempWorkspace(async (config) => {
      const results = await runAgentActionBenchmarkSuite(config);
      expect(results.length).toBe(AGENT_ACTION_BENCHMARK_SUITE.length);
      const byId = new Map(results.map((result) => [result.id, result]));
      expect(byId.get("filesystem.organize.safe_moves")).toMatchObject({ status: "success" });
      expect(byId.get("code.run.tests.after_edit")).toMatchObject({ status: "success" });
      expect(byId.get("document.write_and_inspect")).toMatchObject({ status: "success" });
      expect(byId.get("install.update.package_manager")).toMatchObject({ status: "blocked" });
      expect(byId.get("windows.setting.change")).toMatchObject({ status: "blocked" });
      expect(byId.get("cloud.cli.write")).toMatchObject({ status: "blocked" });
      expect(byId.get("blocked.danger.credentials")).toMatchObject({ status: "blocked" });
      expect((results as Array<{ status: string }>).some((result) => result.status === "planned")).toBe(false);
      for (const result of results) {
        if (result.status === "blocked") {
          expect(result.result?.accepted ?? false).toBe(false);
        }
        if (result.status === "success" && result.result) {
          expect(result.result?.verification?.passed).not.toBe(false);
          expect(result.result?.audit?.logSha256).toMatch(/^[a-f0-9]{64}$/);
        }
      }
    });
  });
});
