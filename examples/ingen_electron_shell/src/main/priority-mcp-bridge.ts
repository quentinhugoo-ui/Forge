import { Buffer } from "node:buffer";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import {
  priorityMcpSchemaForCommand,
  stablePriorityMcpHash,
  type PriorityMcpBridgeResult,
  type PriorityMcpCodeActRequest,
  type PriorityMcpProvider,
  type PriorityMcpStatus,
  type PriorityMcpToolResult
} from "./priority-mcp-codeacts.js";

const DEFAULT_MCP_PROTOCOL_VERSION = "2024-11-05";

interface JsonRpcMessage {
  jsonrpc?: "2.0";
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code?: number; message?: string; data?: unknown };
}

interface McpTool {
  name: string;
  description?: string;
  inputSchema?: {
    type?: string;
    properties?: Record<string, unknown>;
    required?: string[];
  };
}

interface McpToolCallPlan {
  name: string;
  arguments: Record<string, unknown>;
}

interface McpCommandSpec {
  command: string;
  args: string[];
  env?: Record<string, string | undefined>;
  warnings: string[];
  setupError?: string;
}

export async function runPriorityMcpBridge(request: PriorityMcpCodeActRequest): Promise<PriorityMcpBridgeResult> {
  if (request.kind === "codedocs") {
    return runContext7Bridge(request);
  }
  return runGenericPriorityMcpBridge(request);
}

async function runContext7Bridge(request: PriorityMcpCodeActRequest): Promise<PriorityMcpBridgeResult> {
  const startedAt = new Date().toISOString();
  const started = Date.now();
  const spec = commandSpecForRequest(request);
  if (spec.setupError) {
    return finishResult(request, startedAt, started, "error", [], [], spec.warnings, spec.setupError, {});
  }
  const client = new StdioMcpClient(spec.command, spec.args, request.timeoutMs, spec.env);
  try {
    await initializeClient(client);
    const tools = readMcpTools(await client.request("tools/list", {}));
    const resolveTool = findToolByCandidates(tools, ["resolve-library-id", "resolve_library_id", "resolve"]);
    const docsTool = findToolByCandidates(tools, [
      "query-docs",
      "query_docs",
      "get-library-docs",
      "get_library_docs",
      "library-docs",
      "docs"
    ]);
    const toolResults: PriorityMcpToolResult[] = [];
    let resolvedLibraryId = request.libraryId;
    if (!resolvedLibraryId && resolveTool) {
      const args = filterToolArguments(resolveTool, {
        libraryName: request.library,
        library: request.library,
        query: request.query
      });
      const content = await client.request("tools/call", { name: resolveTool.name, arguments: args });
      toolResults.push({ tool: resolveTool.name, arguments: args, content });
      resolvedLibraryId = inferContext7LibraryId(content);
    }
    if (docsTool && (resolvedLibraryId || request.library)) {
      const args = filterToolArguments(docsTool, {
        context7CompatibleLibraryID: resolvedLibraryId,
        libraryId: resolvedLibraryId,
        library_id: resolvedLibraryId,
        library: request.library,
        topic: request.query,
        query: request.query,
        tokens: request.maxTokens,
        maxTokens: request.maxTokens,
        max_tokens: request.maxTokens
      });
      const content = await client.request("tools/call", { name: docsTool.name, arguments: args });
      toolResults.push({ tool: docsTool.name, arguments: args, content });
    }
    const warnings = [
      ...spec.warnings,
      !resolveTool && !request.libraryId ? "Context7 MCP did not list a library resolution tool; queried docs directly when possible." : "",
      !docsTool ? "Context7 MCP did not list a docs query tool." : "",
      !resolvedLibraryId && !request.libraryId && resolveTool ? "Context7 library resolution did not expose a clear library ID in the response." : ""
    ].filter(Boolean);
    const status: PriorityMcpStatus = docsTool && toolResults.length > 0 ? "ok" : toolResults.length > 0 ? "partial" : "error";
    const error = status === "error" ? "Context7 MCP is running but no compatible documentation tool was available." : undefined;
    return finishResult(
      request,
      startedAt,
      started,
      status,
      tools,
      toolResults,
      warnings,
      error,
      {
        resolvedLibraryId,
        library: request.library,
        query: request.query,
        responses: toolResults.map((item) => item.content)
      }
    );
  } catch (error) {
    return finishResult(request, startedAt, started, "error", [], [], spec.warnings, friendlyError(error), {
      command: `${spec.command} ${spec.args.join(" ")}`.trim()
    });
  } finally {
    client.close();
  }
}

async function runGenericPriorityMcpBridge(request: PriorityMcpCodeActRequest): Promise<PriorityMcpBridgeResult> {
  const startedAt = new Date().toISOString();
  const started = Date.now();
  if (request.kind === "github" && isWriteGithubOperation(request.operation)) {
    return finishResult(
      request,
      startedAt,
      started,
      "error",
      [],
      [],
      ["GitHub write-class operations are blocked by the generic /github_ bridge until an explicit confirmation flow is attached."],
      "Write operation requires explicit user confirmation and a scoped GitHub token.",
      { operation: request.operation, mode: request.mode }
    );
  }
  const spec = commandSpecForRequest(request);
  if (spec.setupError) {
    return finishResult(request, startedAt, started, "error", [], [], spec.warnings, spec.setupError, {});
  }
  const client = new StdioMcpClient(spec.command, spec.args, request.timeoutMs, spec.env);
  try {
    await initializeClient(client);
    const tools = readMcpTools(await client.request("tools/list", {}));
    const plans = buildToolPlans(request, tools);
    if (plans.length === 0) {
      return finishResult(
        request,
        startedAt,
        started,
        "error",
        tools,
        [],
        spec.warnings,
        `${request.provider} MCP is running but no compatible tool was listed for operation=${request.operation}.`,
        { availableTools: tools.map((tool) => tool.name) }
      );
    }
    const toolResults: PriorityMcpToolResult[] = [];
    for (const plan of plans) {
      const content = await client.request("tools/call", { name: plan.name, arguments: plan.arguments });
      toolResults.push({ tool: plan.name, arguments: plan.arguments, content });
    }
    return finishResult(request, startedAt, started, "ok", tools, toolResults, spec.warnings, undefined, {
      responses: toolResults.map((item) => item.content)
    });
  } catch (error) {
    return finishResult(request, startedAt, started, "error", [], [], spec.warnings, friendlyError(error), {
      command: `${spec.command} ${spec.args.join(" ")}`.trim()
    });
  } finally {
    client.close();
  }
}

function commandSpecForRequest(request: PriorityMcpCodeActRequest): McpCommandSpec {
  switch (request.provider) {
    case "context7":
      return {
        command: process.env.INGEN_CONTEXT7_MCP_COMMAND || npxCommand(),
        args: splitCommandArgs(process.env.INGEN_CONTEXT7_MCP_ARGS || "-y @upstash/context7-mcp"),
        env: envWithOptional({
          CONTEXT7_API_KEY: process.env.CONTEXT7_API_KEY || process.env.INGEN_CONTEXT7_API_KEY
        }),
        warnings: process.env.CONTEXT7_API_KEY || process.env.INGEN_CONTEXT7_API_KEY
          ? []
          : ["CONTEXT7_API_KEY is not configured; Context7 may run with lower public rate limits."]
      };
    case "github": {
      const token = process.env.GITHUB_PERSONAL_ACCESS_TOKEN || process.env.GITHUB_PAT || process.env.INGEN_GITHUB_TOKEN || "";
      const hasCustomCommand = Boolean(process.env.INGEN_GITHUB_MCP_COMMAND);
      if (!token && !hasCustomCommand) {
        return {
          command: "",
          args: [],
          warnings: [],
          setupError: "GITHUB_PERSONAL_ACCESS_TOKEN, GITHUB_PAT or INGEN_GITHUB_TOKEN is not configured for GitHub MCP."
        };
      }
      return {
        command: process.env.INGEN_GITHUB_MCP_COMMAND || "docker",
        args: splitCommandArgs(process.env.INGEN_GITHUB_MCP_ARGS || "run -i --rm -e GITHUB_PERSONAL_ACCESS_TOKEN ghcr.io/github/github-mcp-server"),
        env: envWithOptional({ GITHUB_PERSONAL_ACCESS_TOKEN: token }),
        warnings: token ? [] : ["Using custom GitHub MCP command without a detected token in the app environment."]
      };
    }
    case "playwright":
      return {
        command: process.env.INGEN_PLAYWRIGHT_MCP_COMMAND || npxCommand(),
        args: splitCommandArgs(process.env.INGEN_PLAYWRIGHT_MCP_ARGS || "-y @playwright/mcp@latest"),
        warnings: []
      };
    case "semgrep":
      return {
        command: process.env.INGEN_SEMGREP_MCP_COMMAND || uvxCommand(),
        args: splitCommandArgs(process.env.INGEN_SEMGREP_MCP_ARGS || "semgrep-mcp"),
        warnings: []
      };
  }
}

async function initializeClient(client: StdioMcpClient): Promise<void> {
  await client.start();
  await client.request("initialize", {
    protocolVersion: process.env.INGEN_MCP_PROTOCOL_VERSION || DEFAULT_MCP_PROTOCOL_VERSION,
    capabilities: {},
    clientInfo: {
      name: "InGen Priority MCP Bridge",
      version: "0.1.0"
    }
  });
  client.notify("notifications/initialized", {});
}

function buildToolPlans(request: PriorityMcpCodeActRequest, tools: McpTool[]): McpToolCallPlan[] {
  switch (request.kind) {
    case "github":
      return buildGithubPlans(request, tools);
    case "webact":
      return buildPlaywrightPlans(request, tools);
    case "securityscan":
      return buildSemgrepPlans(request, tools);
    case "codedocs":
      return [];
  }
}

function buildGithubPlans(request: PriorityMcpCodeActRequest, tools: McpTool[]): McpToolCallPlan[] {
  const tool = githubToolForOperation(request.operation, tools);
  if (!tool) {
    return [];
  }
  const { owner, repo } = splitRepo(request.repo);
  return [{
    name: tool.name,
    arguments: filterToolArguments(tool, {
      owner,
      repo,
      repository: request.repo,
      repository_full_name: request.repo,
      query: request.query,
      q: request.query,
      issue_number: numericString(request.number),
      pull_number: numericString(request.number),
      pr_number: numericString(request.number),
      number: numericString(request.number),
      ref: request.ref,
      branch: request.ref,
      sha: request.ref,
      per_page: 20,
      limit: 20
    })
  }];
}

function githubToolForOperation(operation: string, tools: McpTool[]): McpTool | undefined {
  switch (operation) {
    case "code_search":
      return findToolByCandidates(tools, ["search_code", "search-code", "code_search", "search"]);
    case "issue_pr_triage":
      return findToolByCandidates(tools, ["get_pull_request", "get_issue", "list_pull_requests", "list_issues", "search_issues"]);
    case "ci_status":
      return findToolByCandidates(tools, ["list_workflow_runs", "get_workflow_run", "list_workflow_jobs", "get_commit_status"]);
    case "release_notes":
      return findToolByCandidates(tools, ["get_latest_release", "list_releases", "get_release_by_tag"]);
    case "repo_context":
    default:
      return findToolByCandidates(tools, ["get_repository", "get_file_contents", "search_repositories", "list_commits"]);
  }
}

function buildPlaywrightPlans(request: PriorityMcpCodeActRequest, tools: McpTool[]): McpToolCallPlan[] {
  const plans: McpToolCallPlan[] = [];
  const pushPlan = (tool: McpTool | undefined, args: Record<string, unknown>) => {
    if (tool) {
      plans.push({ name: tool.name, arguments: filterToolArguments(tool, args) });
    }
  };
  if (request.operation === "test_flow" && request.url) {
    pushPlan(findToolByCandidates(tools, ["browser_navigate", "navigate"]), { url: request.url });
    pushPlan(findToolByCandidates(tools, ["browser_snapshot", "snapshot"]), {});
    pushPlan(findToolByCandidates(tools, ["browser_take_screenshot", "screenshot"]), {
      fullPage: true,
      filename: "ingen-webact-screenshot.png"
    });
    return plans;
  }
  switch (request.operation) {
    case "navigate":
      pushPlan(findToolByCandidates(tools, ["browser_navigate", "navigate"]), { url: request.url });
      break;
    case "click":
      pushPlan(findToolByCandidates(tools, ["browser_click", "click"]), {
        element: request.instruction || request.selector,
        ref: request.selector,
        selector: request.selector
      });
      break;
    case "type":
      pushPlan(findToolByCandidates(tools, ["browser_type", "type"]), {
        element: request.instruction || request.selector,
        ref: request.selector,
        selector: request.selector,
        text: request.text
      });
      break;
    case "screenshot":
      pushPlan(findToolByCandidates(tools, ["browser_take_screenshot", "screenshot"]), {
        fullPage: true,
        filename: "ingen-webact-screenshot.png"
      });
      break;
    case "evaluate":
      pushPlan(findToolByCandidates(tools, ["browser_evaluate", "evaluate"]), {
        function: request.instruction || request.text,
        expression: request.instruction || request.text
      });
      break;
    case "snapshot":
    default:
      pushPlan(findToolByCandidates(tools, ["browser_snapshot", "snapshot"]), {});
      break;
  }
  return plans;
}

function buildSemgrepPlans(request: PriorityMcpCodeActRequest, tools: McpTool[]): McpToolCallPlan[] {
  const tool = semgrepToolForOperation(request.operation, tools);
  if (!tool) {
    return [];
  }
  return [{
    name: tool.name,
    arguments: filterToolArguments(tool, {
      path: request.target,
      target: request.target,
      paths: [request.target],
      code_files: [request.target],
      config: request.config,
      rule: request.rule,
      custom_rule: request.rule,
      language: request.language,
      severity: request.severityThreshold
    })
  }];
}

function semgrepToolForOperation(operation: string, tools: McpTool[]): McpTool | undefined {
  switch (operation) {
    case "semgrep_scan":
      return findToolByCandidates(tools, ["semgrep_scan", "semgrep-scan", "scan"]);
    case "custom_rule":
      return findToolByCandidates(tools, ["semgrep_scan_with_custom_rule", "custom_rule", "scan_with_custom_rule"]);
    case "ast":
      return findToolByCandidates(tools, ["get_abstract_syntax_tree", "abstract_syntax_tree", "ast"]);
    case "security_check":
    default:
      return findToolByCandidates(tools, ["security_check", "security-check", "semgrep_scan", "scan"]);
  }
}

function finishResult(
  request: PriorityMcpCodeActRequest,
  startedAt: string,
  started: number,
  status: PriorityMcpStatus,
  tools: McpTool[],
  toolResults: PriorityMcpToolResult[],
  warnings: string[],
  error: string | undefined,
  content: unknown
): PriorityMcpBridgeResult {
  const finishedAt = new Date().toISOString();
  const result: PriorityMcpBridgeResult = {
    schema: priorityMcpSchemaForCommand(request.command),
    command: request.command,
    status,
    requestHash: request.proofHash,
    startedAt,
    finishedAt,
    durationMs: Date.now() - started,
    provider: request.provider,
    operation: request.operation,
    tool: toolResults.map((item) => item.tool).join("+") || undefined,
    availableTools: tools.map((tool) => tool.name),
    content,
    toolResults,
    warnings,
    error,
    proofHash: ""
  };
  result.proofHash = stablePriorityMcpHash({ ...result, proofHash: "" });
  return result;
}

class StdioMcpClient {
  private child?: ChildProcessWithoutNullStreams;
  private nextId = 1;
  private stdoutBuffer = Buffer.alloc(0);
  private stderrTail = "";
  private pending = new Map<number, {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }>();

  constructor(
    private readonly command: string,
    private readonly args: string[],
    private readonly timeoutMs: number,
    private readonly extraEnv: Record<string, string | undefined> = {}
  ) {}

  start(): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const child = spawn(this.command, this.args, {
        stdio: "pipe",
        windowsHide: true,
        env: { ...process.env, ...this.extraEnv }
      });
      this.child = child;
      child.stdout.on("data", (chunk: Buffer) => this.handleStdout(chunk));
      child.stderr.on("data", (chunk: Buffer) => {
        this.stderrTail = compactTail(`${this.stderrTail}${chunk.toString("utf8")}`, 2_000);
      });
      child.on("error", (error) => {
        this.rejectAll(error instanceof Error ? error : new Error(String(error)));
        if (!settled) {
          settled = true;
          reject(error);
        }
      });
      child.on("exit", (code, signal) => {
        const detail = this.stderrTail ? ` stderr=${this.stderrTail}` : "";
        this.rejectAll(new Error(`MCP process exited code=${code ?? "null"} signal=${signal ?? "null"}.${detail}`));
      });
      setTimeout(() => {
        if (!settled) {
          settled = true;
          resolve();
        }
      }, 0);
    });
  }

  request(method: string, params?: unknown): Promise<unknown> {
    const id = this.nextId++;
    const message: JsonRpcMessage = { jsonrpc: "2.0", id, method, params };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`MCP request timed out: ${method}`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.send(message);
    });
  }

  notify(method: string, params?: unknown): void {
    this.send({ jsonrpc: "2.0", method, params });
  }

  close(): void {
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(new Error(`MCP client closed before response id=${id}`));
    }
    this.pending.clear();
    const child = this.child;
    if (!child) return;
    try {
      child.stdin.end();
    } catch {
      // Best effort cleanup.
    }
    if (!child.killed) {
      setTimeout(() => {
        if (!child.killed) {
          child.kill();
        }
      }, 250);
    }
  }

  private send(message: JsonRpcMessage): void {
    if (!this.child || !this.child.stdin.writable) {
      throw new Error("MCP process is not writable.");
    }
    const body = Buffer.from(JSON.stringify(message), "utf8");
    const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
    this.child.stdin.write(Buffer.concat([header, body]));
  }

  private handleStdout(chunk: Buffer): void {
    this.stdoutBuffer = Buffer.concat([this.stdoutBuffer, chunk]);
    while (true) {
      const headerEnd = this.stdoutBuffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        return;
      }
      const header = this.stdoutBuffer.slice(0, headerEnd).toString("utf8");
      const lengthMatch = /content-length\s*:\s*(\d+)/iu.exec(header);
      if (!lengthMatch) {
        this.stdoutBuffer = this.stdoutBuffer.slice(headerEnd + 4);
        continue;
      }
      const length = Number(lengthMatch[1]);
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + length;
      if (this.stdoutBuffer.length < bodyEnd) {
        return;
      }
      const body = this.stdoutBuffer.slice(bodyStart, bodyEnd).toString("utf8");
      this.stdoutBuffer = this.stdoutBuffer.slice(bodyEnd);
      try {
        this.handleMessage(JSON.parse(body) as JsonRpcMessage);
      } catch (error) {
        this.rejectAll(new Error(`Invalid MCP JSON response: ${friendlyError(error)}`));
      }
    }
  }

  private handleMessage(message: JsonRpcMessage): void {
    if (typeof message.id !== "number") {
      return;
    }
    const pending = this.pending.get(message.id);
    if (!pending) {
      return;
    }
    this.pending.delete(message.id);
    clearTimeout(pending.timer);
    if (message.error) {
      pending.reject(new Error(message.error.message || `MCP error ${message.error.code ?? ""}`.trim()));
    } else {
      pending.resolve(message.result);
    }
  }

  private rejectAll(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

function readMcpTools(value: unknown): McpTool[] {
  const tools = readRecord(value)?.tools;
  if (!Array.isArray(tools)) {
    return [];
  }
  return tools
    .map((tool) => readRecord(tool))
    .filter((tool): tool is Record<string, unknown> => {
      if (!tool) return false;
      return typeof tool.name === "string";
    })
    .map((tool) => ({
      name: String(tool.name),
      description: typeof tool.description === "string" ? tool.description : undefined,
      inputSchema: readRecord(tool.inputSchema) as McpTool["inputSchema"]
    }));
}

function findToolByCandidates(tools: McpTool[], candidates: string[]): McpTool | undefined {
  for (const candidate of candidates) {
    const exact = tools.find((tool) => normalizeToolName(tool.name) === normalizeToolName(candidate));
    if (exact) {
      return exact;
    }
  }
  for (const candidate of candidates) {
    const normalized = normalizeToolName(candidate);
    const fuzzy = tools.find((tool) => normalizeToolName(tool.name).includes(normalized));
    if (fuzzy) {
      return fuzzy;
    }
  }
  return undefined;
}

function normalizeToolName(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/gu, "_").replace(/^_+|_+$/gu, "");
}

function filterToolArguments(tool: McpTool | undefined, args: Record<string, unknown>): Record<string, unknown> {
  const clean = dropUndefined(args);
  const properties = tool?.inputSchema?.properties;
  if (!properties || Object.keys(properties).length === 0) {
    return clean;
  }
  const filtered: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(clean)) {
    if (Object.prototype.hasOwnProperty.call(properties, key)) {
      filtered[key] = value;
    }
  }
  if (Object.keys(filtered).length > 0) {
    return filtered;
  }
  for (const required of tool?.inputSchema?.required ?? []) {
    if (required in clean) {
      filtered[required] = clean[required];
    }
  }
  return Object.keys(filtered).length > 0 ? filtered : clean;
}

function inferContext7LibraryId(value: unknown): string {
  const text = JSON.stringify(value);
  const exact = /(?:context7CompatibleLibraryID|libraryId|library_id)["']?\s*[:=]\s*["']([^"']+)["']/iu.exec(text);
  if (exact?.[1]) {
    return exact[1];
  }
  const slashId = /\/[a-z0-9_.-]+\/[a-z0-9_.-]+(?:\/[a-z0-9_.-]+)?/iu.exec(text);
  return slashId?.[0] ?? "";
}

function splitRepo(value: string): { owner: string; repo: string } {
  const [owner = "", repo = ""] = value.split("/");
  return { owner, repo };
}

function numericString(value: string): number | string {
  if (!value) {
    return "";
  }
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : value;
}

function isWriteGithubOperation(operation: string): boolean {
  return operation === "create_issue" || operation === "comment_pr";
}

function npxCommand(): string {
  return process.platform === "win32" ? "npx.cmd" : "npx";
}

function uvxCommand(): string {
  return process.platform === "win32" ? "uvx.exe" : "uvx";
}

function envWithOptional(values: Record<string, string | undefined>): Record<string, string | undefined> {
  return Object.fromEntries(Object.entries(values).filter(([, value]) => Boolean(value)));
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

function dropUndefined(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined && item !== ""));
}

function splitCommandArgs(value: string): string[] {
  const args: string[] = [];
  const regex = /"([^"]*)"|'([^']*)'|(\S+)/gu;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(value)) !== null) {
    args.push(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return args.filter(Boolean);
}

function compactTail(value: string, maxChars: number): string {
  return value.length <= maxChars ? value : value.slice(value.length - maxChars);
}

function friendlyError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
