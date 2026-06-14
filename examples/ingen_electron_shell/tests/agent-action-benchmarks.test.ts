import { describe, expect, it } from "vitest";
import {
  AGENT_ACTION_BENCHMARK_SUITE,
  REQUIRED_AGENT_ACTION_BENCHMARK_SURFACES,
  agentActionBenchmarkSurfaceCoverage,
  missingAgentActionBenchmarkSurfaces
} from "../src/shared/agent-action-benchmarks";

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
});
