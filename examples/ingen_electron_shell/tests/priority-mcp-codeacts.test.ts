import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  extractPriorityMcpCodeAct,
  parsePriorityMcpCodeAct,
  renderPriorityMcpCodeActResult,
  type PriorityMcpBridgeResult
} from "../src/main/priority-mcp-codeacts";
import { runPriorityMcpBridge } from "../src/main/priority-mcp-bridge";
import {
  BRAIN_CODEDOCS_COMMAND,
  BRAIN_CODEDOCS_RESULT_SCHEMA,
  BRAIN_GITHUB_MCP_COMMAND,
  BRAIN_GITHUB_MCP_RESULT_SCHEMA,
  BRAIN_SECURITYSCAN_COMMAND,
  BRAIN_WEBACT_COMMAND
} from "../src/shared/ipc-contract";

const savedEnv = {
  GITHUB_PERSONAL_ACCESS_TOKEN: process.env.GITHUB_PERSONAL_ACCESS_TOKEN,
  GITHUB_PAT: process.env.GITHUB_PAT,
  INGEN_GITHUB_TOKEN: process.env.INGEN_GITHUB_TOKEN,
  INGEN_GITHUB_MCP_COMMAND: process.env.INGEN_GITHUB_MCP_COMMAND
};

afterEach(() => {
  for (const [key, value] of Object.entries(savedEnv)) {
    if (value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }
});

describe("priority MCP CodeActs", () => {
  it("parses the four priority MCP commands with production slots", () => {
    expect(parsePriorityMcpCodeAct('/codedocs_ library="react" query="useEffect cleanup behavior" output="examples"')).toMatchObject({
      command: BRAIN_CODEDOCS_COMMAND,
      kind: "codedocs",
      provider: "context7",
      library: "react",
      query: "useEffect cleanup behavior",
      operation: "query_docs"
    });

    expect(parsePriorityMcpCodeAct('/github_ operation="ci_status" repo="owner/repo" number="42"')).toMatchObject({
      command: BRAIN_GITHUB_MCP_COMMAND,
      kind: "github",
      provider: "github",
      operation: "ci_status",
      repo: "owner/repo",
      number: "42"
    });

    expect(parsePriorityMcpCodeAct('/webact_ action="snapshot" url="http://localhost:3000"')).toMatchObject({
      command: BRAIN_WEBACT_COMMAND,
      kind: "webact",
      provider: "playwright",
      operation: "snapshot",
      url: "http://localhost:3000"
    });

    expect(parsePriorityMcpCodeAct('/securityscan_ target="src/main.ts" mode="security_check" severity_threshold="error"')).toMatchObject({
      command: BRAIN_SECURITYSCAN_COMMAND,
      kind: "securityscan",
      provider: "semgrep",
      operation: "security_check",
      target: "src/main.ts",
      severityThreshold: "error"
    });
  });

  it("extracts a priority MCP CodeAct from a loop-stream assistant message", () => {
    const request = extractPriorityMcpCodeAct([
      "Je verifie la doc exacte avant de coder.",
      "/codedocs_",
      'library="playwright"',
      'query="MCP server snapshot tool names"',
      "",
      "Je reprends apres le resultat."
    ].join("\n"));

    expect(request).toMatchObject({
      command: BRAIN_CODEDOCS_COMMAND,
      library: "playwright",
      query: "MCP server snapshot tool names"
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("renders compact result manifests with provider and tool trace", () => {
    const result: PriorityMcpBridgeResult = {
      schema: BRAIN_CODEDOCS_RESULT_SCHEMA,
      command: BRAIN_CODEDOCS_COMMAND,
      status: "ok",
      requestHash: "a".repeat(64),
      startedAt: "2026-06-15T00:00:00.000Z",
      finishedAt: "2026-06-15T00:00:01.000Z",
      durationMs: 1000,
      provider: "context7",
      operation: "query_docs",
      tool: "resolve-library-id+get-library-docs",
      availableTools: ["resolve-library-id", "get-library-docs"],
      content: { resolvedLibraryId: "/react/react", docs: "cleanup example" },
      toolResults: [{
        tool: "get-library-docs",
        arguments: { topic: "cleanup" },
        content: { text: "cleanup example" }
      }],
      warnings: [],
      proofHash: "b".repeat(64)
    };

    const rendered = renderPriorityMcpCodeActResult(result);

    expect(rendered).toContain("CODEDOCS_RESULT");
    expect(rendered).toContain(`schema=${BRAIN_CODEDOCS_RESULT_SCHEMA}`);
    expect(rendered).toContain("provider=context7");
    expect(rendered).toContain("tool=resolve-library-id+get-library-docs");
  });

  it("returns an explicit GitHub setup error when no token or custom command exists", async () => {
    delete process.env.GITHUB_PERSONAL_ACCESS_TOKEN;
    delete process.env.GITHUB_PAT;
    delete process.env.INGEN_GITHUB_TOKEN;
    delete process.env.INGEN_GITHUB_MCP_COMMAND;

    const request = parsePriorityMcpCodeAct('/github_ operation="repo_context" repo="owner/repo"')!;
    const result = await runPriorityMcpBridge(request);

    expect(result.schema).toBe(BRAIN_GITHUB_MCP_RESULT_SCHEMA);
    expect(result.status).toBe("error");
    expect(result.error).toContain("GITHUB_PERSONAL_ACCESS_TOKEN");
    expect(result.availableTools).toEqual([]);
  });

  it("is wired into the existing module CodeAct loop without replacing loopstream architecture", () => {
    const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

    expect(mainSource).toContain("executeAssistantPriorityMcpCodeAct");
    expect(mainSource).toContain("runPriorityMcpBridge");
    expect(mainSource).toContain("renderPriorityMcpCodeActResult");
    expect(mainSource).toContain("command === BRAIN_CODEDOCS_COMMAND");
    expect(mainSource).toContain("command === BRAIN_SECURITYSCAN_COMMAND");
    expect(mainSource).toContain("next = await executeAssistantPriorityMcpCodeAct(next)");
  });
});
