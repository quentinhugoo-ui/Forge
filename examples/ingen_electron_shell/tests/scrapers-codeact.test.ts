import { describe, expect, it } from "vitest";
import {
  extractScrapersCodeAct,
  parseScrapersCodeAct,
  renderScrapersCodeActResult,
  SCRAPERS_COMMAND,
  SCRAPERS_RESULT_SCHEMA,
  type ScrapersBridgeResult
} from "../src/main/scrapers-codeact";

describe("Scrapers MCP CodeAct", () => {
  it("parses the production /scrapers_ template into bounded provider inputs", () => {
    const request = parseScrapersCodeAct([
      '/scrapers_ urls=["https://example.com/a","https://example.com/b"]',
      'goal="collect product cards for RAG"',
      'fields_schema=\'[{"name":"title","selector":"h1","attr":"text"},{"name":"image","selector":"img","attr":"src"}]\'',
      'selectors="h1::text, img::attr(src)"',
      'fetch_mode="dynamic"',
      'wait_until="images_loaded"',
      'media="all_media_urls"',
      'artifacts="full_page_screenshot"',
      'limits="pages=2,links=80,images=40,bytes=999999999,timeout_ms=45000,concurrency=9"'
    ].join(" "));

    expect(request).toMatchObject({
      command: SCRAPERS_COMMAND,
      urls: ["https://example.com/a", "https://example.com/b"],
      goal: "collect product cards for RAG",
      fetchMode: "dynamic",
      waitUntil: "images_loaded",
      media: "all_media_urls",
      artifacts: "full_page_screenshot",
      backends: ["scrapling", "crawl4ai"]
    });
    expect(request?.fieldsSchema).toEqual([
      { name: "title", selector: "h1", attr: "text" },
      { name: "image", selector: "img", attr: "src" }
    ]);
    expect(request?.limits).toMatchObject({
      pages: 2,
      links: 80,
      images: 40,
      bytes: 52_428_800,
      timeoutMs: 45_000,
      concurrency: 8
    });
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts /scrapers_ only when the assistant emits an explicit command", () => {
    expect(extractScrapersCodeAct("Recupere les images de cette page")).toBeUndefined();

    const request = extractScrapersCodeAct([
      "Je lance les deux scrapers en parallele.",
      '/scrapers_ urls="https://example.com" goal="extract clean markdown and image URLs"'
    ].join("\n"));

    expect(request).toMatchObject({
      command: SCRAPERS_COMMAND,
      urls: ["https://example.com/"],
      goal: "extract clean markdown and image URLs"
    });
  });

  it("renders one compact merged manifest for the continuation loop", () => {
    const result: ScrapersBridgeResult = {
      schema: SCRAPERS_RESULT_SCHEMA,
      command: SCRAPERS_COMMAND,
      status: "partial",
      requestHash: "a".repeat(64),
      startedAt: "2026-06-15T00:00:00.000Z",
      finishedAt: "2026-06-15T00:00:01.000Z",
      durationMs: 1000,
      providers: [
        {
          backend: "scrapling",
          status: "ok",
          transport: "mcp_stdio",
          durationMs: 500,
          tool: "fetch",
          warnings: [],
          counts: { urls: 1, fields: 1, markdownChars: 0, links: 0, media: 1, artifacts: 0 },
          data: {
            urls: ["https://example.com/"],
            fields: [{ title: "Example" }],
            markdown: [],
            links: [],
            media: [{ url: "https://example.com/image.png" }],
            artifacts: []
          }
        },
        {
          backend: "crawl4ai",
          status: "error",
          transport: "crawl4ai_http",
          durationMs: 500,
          error: "server offline",
          warnings: ["Expected Crawl4AI self-hosted server at http://localhost:11235."],
          counts: { urls: 0, fields: 0, markdownChars: 0, links: 0, media: 0, artifacts: 0 },
          data: { urls: [], fields: [], markdown: [], links: [], media: [], artifacts: [] }
        }
      ],
      merged: {
        urls: ["https://example.com/"],
        fields: [{ title: "Example" }],
        markdown: [],
        links: [],
        media: [{ url: "https://example.com/image.png" }],
        artifacts: [],
        warnings: ["Expected Crawl4AI self-hosted server at http://localhost:11235."],
        provenance: []
      },
      proofHash: "b".repeat(64)
    };

    const rendered = renderScrapersCodeActResult(result);

    expect(rendered).toContain("SCRAPERS_RESULT");
    expect(rendered).toContain(`schema=${SCRAPERS_RESULT_SCHEMA}`);
    expect(rendered).toContain("status=partial");
    expect(rendered).toContain("media_manifest=");
    expect(rendered).toContain('"backend":"scrapling"');
    expect(rendered).toContain('"backend":"crawl4ai"');
  });
});
