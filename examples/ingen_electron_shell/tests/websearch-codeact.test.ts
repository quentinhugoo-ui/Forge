import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  extractWebSearchCodeAct,
  parseWebSearchCodeAct,
  readWebSearchCodeAct,
  renderWebSearchCodeActResult,
  renderWebSearchTemplateResult,
  WEBSEARCH_COMMAND,
  WEBSEARCH_RESULT_SCHEMA,
  type WebSearchBridgeResult
} from "../src/main/websearch-codeact";
import { runWebSearchBridge } from "../src/main/websearch-bridge";

const savedEnv = {
  OPENAI_API_KEY: process.env.OPENAI_API_KEY,
  INGEN_OPENAI_API_KEY: process.env.INGEN_OPENAI_API_KEY,
  ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
  CLAUDE_API_KEY: process.env.CLAUDE_API_KEY,
  INGEN_ANTHROPIC_API_KEY: process.env.INGEN_ANTHROPIC_API_KEY
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

describe("WebSearch CodeAct", () => {
  it("requires the two-phase template handoff before execution", () => {
    const templateStep = readWebSearchCodeAct("/websearch_");

    expect(templateStep?.kind).toBe("template");
    expect(templateStep?.kind === "template" ? renderWebSearchTemplateResult(templateStep.result) : "").toContain("WEBSEARCH_TEMPLATE_RESULT");

    const directFilled = readWebSearchCodeAct('/websearch_ query="latest OpenAI web search API docs"');
    expect(directFilled?.kind).toBe("template");
    expect(directFilled?.kind === "template" ? directFilled.result.reason : "").toBe("template_required");

    const rendered = templateStep?.kind === "template" ? renderWebSearchTemplateResult(templateStep.result) : "";
    expect(rendered).toContain("domains=[]");
    expect(rendered).toContain('media="none|images|video_audio"');
    expect(rendered).toContain('next="answer_only|scrape_urls"');
    expect(rendered).not.toContain("providers=");
    expect(rendered).not.toContain("tool_choice=");
    expect(rendered).not.toContain("search_context_size=");
    const proofHash = rendered.match(/template_proof_hash=sha256:([a-f0-9]{64})/)?.[1];
    expect(proofHash).toMatch(/^[a-f0-9]{64}$/);

    const request = readWebSearchCodeAct([
      `/websearch_ template_proof_hash="sha256:${proofHash}"`,
      'query="latest OpenAI web search API docs"',
      'goal="url_discovery_for_scrapers"',
      'providers="both_parallel"'
    ].join(" "));

    expect(request?.kind).toBe("request");
    expect(request?.kind === "request" ? request.request : undefined).toMatchObject({
      command: WEBSEARCH_COMMAND,
      templateProofHash: proofHash,
      query: "latest OpenAI web search API docs",
      goal: "url_discovery_for_scrapers",
      providers: "both_parallel"
    });
  });

  it("maps the compact template aliases to the full runtime request", () => {
    const templateStep = readWebSearchCodeAct("/websearch_");
    const rendered = templateStep?.kind === "template" ? renderWebSearchTemplateResult(templateStep.result) : "";
    const proofHash = rendered.match(/template_proof_hash=sha256:([a-f0-9]{64})/)?.[1];

    const request = readWebSearchCodeAct([
      `/websearch_ template_proof_hash="sha256:${proofHash}"`,
      'query="sources officielles The Witcher 3 Hearts of Stone"',
      'goal="url_discovery_for_scrapers"',
      'freshness="any_time"',
      'domains=["thewitcher.com"]',
      'media="images"',
      'next="scrape_urls"',
      'locale="fr"'
    ].join(" "));

    expect(request?.kind).toBe("request");
    expect(request?.kind === "request" ? request.request : undefined).toMatchObject({
      query: "sources officielles The Witcher 3 Hearts of Stone",
      allowedDomains: ["thewitcher.com"],
      mediaIntent: "image_enrichment",
      searchContentTypes: "text_image",
      extractIntent: "next_loop_scrapers",
      output: "media_manifest",
      locale: "fr"
    });
  });

  it("parses a production /websearch_ request with provider and domain controls", () => {
    const request = parseWebSearchCodeAct([
      '/websearch_ query="latest OpenAI web search API docs"',
      'goal="url_discovery_for_scrapers"',
      'providers="both_parallel"',
      'tool_choice="required"',
      'freshness="latest"',
      'allowed_domains=["platform.openai.com","docs.anthropic.com"]',
      'blocked_domains="reddit.com,quora.com"',
      'max_searches="50"',
      'top_k_urls="99"',
      'search_context_size="high"',
      'media_intent="image_enrichment"',
      'search_content_types="text_image"',
      'image_max_results="99"',
      'extract_intent="next_loop_scrapers"'
    ].join(" "));

    expect(request).toMatchObject({
      command: WEBSEARCH_COMMAND,
      query: "latest OpenAI web search API docs",
      goal: "url_discovery_for_scrapers",
      providers: "both_parallel",
      toolChoice: "required",
      freshness: "latest",
      allowedDomains: ["platform.openai.com", "docs.anthropic.com"],
      blockedDomains: ["reddit.com", "quora.com"],
      maxSearches: 20,
      topKUrls: 30,
      searchContextSize: "high",
      mediaIntent: "image_enrichment",
      searchContentTypes: "text_image",
      imageMaxResults: 30,
      extractIntent: "next_loop_scrapers"
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts the command only when the assistant emits it explicitly", () => {
    expect(extractWebSearchCodeAct("Cherche les dernieres docs OpenAI")).toBeUndefined();

    const request = extractWebSearchCodeAct([
      "Je lance une recherche sourcee.",
      "/websearch_",
      'query="best MCP web search providers with \\"citations\\""',
      'goal="comparative_research"',
      'extract_intent="next_loop_scrapers"',
      "",
      "Je reprendrai ensuite les URLs utiles."
    ].join("\n"));

    expect(request).toMatchObject({
      command: WEBSEARCH_COMMAND,
      query: 'best MCP web search providers with "citations"',
      goal: "comparative_research",
      extractIntent: "next_loop_scrapers"
    });
  });

  it("renders a compact result manifest for the loop stream", () => {
    const result: WebSearchBridgeResult = {
      schema: WEBSEARCH_RESULT_SCHEMA,
      command: WEBSEARCH_COMMAND,
      status: "ok",
      requestHash: "a".repeat(64),
      startedAt: "2026-06-15T00:00:00.000Z",
      finishedAt: "2026-06-15T00:00:01.000Z",
      durationMs: 1000,
      providers: [{
        provider: "openai",
        status: "ok",
        durationMs: 900,
        model: "gpt-5.5",
        searchedQueries: ["best MCP web search providers"],
        answer: "Use source-backed search first.",
        urls: [{ provider: "openai", url: "https://example.com/a", title: "Example" }],
        citations: [{ provider: "openai", url: "https://example.com/a", title: "Example" }],
        media: [{
          provider: "openai",
          kind: "image",
          url: "https://example.com/image.jpg",
          thumbnailUrl: "https://example.com/thumb.jpg",
          sourceUrl: "https://example.com/a",
          caption: "Example image"
        }],
        warnings: []
      }],
      answer: "Use source-backed search first.",
      urls: [{ provider: "openai", url: "https://example.com/a", title: "Example" }],
      citations: [{ provider: "openai", url: "https://example.com/a", title: "Example" }],
      media: [{
        provider: "openai",
        kind: "image",
        url: "https://example.com/image.jpg",
        thumbnailUrl: "https://example.com/thumb.jpg",
        sourceUrl: "https://example.com/a",
        caption: "Example image"
      }],
      suggestedScraperUrls: ["https://example.com/a"],
      warnings: [],
      proofHash: "b".repeat(64)
    };

    const rendered = renderWebSearchCodeActResult(result);

    expect(rendered).toContain("WEBSEARCH_RESULT");
    expect(rendered).toContain(`schema=${WEBSEARCH_RESULT_SCHEMA}`);
    expect(rendered).toContain("status=ok");
    expect(rendered).toContain("media=");
    expect(rendered).toContain("suggested_scraper_urls");
  });

  it("returns an explicit provider setup error instead of silently pretending to search", async () => {
    delete process.env.OPENAI_API_KEY;
    delete process.env.INGEN_OPENAI_API_KEY;
    delete process.env.ANTHROPIC_API_KEY;
    delete process.env.CLAUDE_API_KEY;
    delete process.env.INGEN_ANTHROPIC_API_KEY;

    const request = parseWebSearchCodeAct('/websearch_ query="latest docs" providers="auto"')!;
    const result = await runWebSearchBridge(request);

    expect(result.status).toBe("error");
    expect(result.providers).toHaveLength(2);
    expect(result.warnings.join(" ")).toContain("API_KEY");
    expect(result.suggestedScraperUrls).toEqual([]);
    expect(result.media).toEqual([]);
  });

  it("is wired into the existing module CodeAct loop without replacing the loopstream architecture", () => {
    const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

    expect(mainSource).toContain("executeAssistantWebSearchCodeAct");
    expect(mainSource).toContain("runWebSearchBridge");
    expect(mainSource).toContain("renderWebSearchCodeActResult");
    expect(mainSource).toContain("renderWebSearchTemplateResult");
    expect(mainSource).toContain("command === BRAIN_WEBSEARCH_COMMAND");
    expect(mainSource).toContain("next = await executeAssistantWebSearchCodeAct(next)");
  });
});
