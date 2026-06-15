import { once } from "node:events";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { parseScrapersCodeAct } from "../src/main/scrapers-codeact";
import { runScrapersMcpBridge } from "../src/main/scrapers-mcp-bridge";

const fakeScraplingMcpPath = fileURLToPath(new URL("./fixtures/fake-scrapling-mcp.mjs", import.meta.url));
const previousEnv = {
  scraplingCommand: process.env.INGEN_SCRAPLING_MCP_COMMAND,
  scraplingArgs: process.env.INGEN_SCRAPLING_MCP_ARGS,
  crawl4aiBaseUrl: process.env.INGEN_CRAWL4AI_BASE_URL
};

afterEach(() => {
  restoreEnv("INGEN_SCRAPLING_MCP_COMMAND", previousEnv.scraplingCommand);
  restoreEnv("INGEN_SCRAPLING_MCP_ARGS", previousEnv.scraplingArgs);
  restoreEnv("INGEN_CRAWL4AI_BASE_URL", previousEnv.crawl4aiBaseUrl);
});

describe("Scrapers MCP bridge", () => {
  it("fans out to Scrapling MCP stdio and Crawl4AI HTTP, then merges a compact manifest", async () => {
    const server = createServer(crawl4AiHandler);
    try {
      server.listen(0, "127.0.0.1");
      await once(server, "listening");
      const address = server.address();
      if (!address || typeof address === "string") {
        throw new Error("Missing local HTTP address");
      }

      process.env.INGEN_SCRAPLING_MCP_COMMAND = process.execPath;
      process.env.INGEN_SCRAPLING_MCP_ARGS = `"${fakeScraplingMcpPath}"`;
      process.env.INGEN_CRAWL4AI_BASE_URL = `http://127.0.0.1:${address.port}`;

      const request = parseScrapersCodeAct([
        '/scrapers_ urls="https://example.com"',
        'goal="extract title markdown links image urls and screenshot"',
        'selectors="h1::text"',
        'artifacts="full_page_screenshot"',
        'limits="pages=1,links=10,images=10,timeout_ms=5000,concurrency=2"'
      ].join(" "));

      expect(request).toBeDefined();
      const result = await runScrapersMcpBridge(request!);

      expect(result.status).toBe("ok");
      expect(result.providers.map((provider) => provider.backend).sort()).toEqual(["crawl4ai", "scrapling"]);
      expect(result.providers.every((provider) => provider.status === "ok")).toBe(true);
      expect(result.merged.markdown.join("\n")).toContain("Example Domain");
      expect(JSON.stringify(result.merged.media)).toContain("https://example.com/hero.png");
      expect(JSON.stringify(result.merged.links)).toContain("https://example.com/about");
      expect(result.merged.artifacts.some((artifact) => artifact.sha256?.match(/^[a-f0-9]{64}$/))).toBe(true);
      expect(result.proofHash).toMatch(/^[a-f0-9]{64}$/);
    } finally {
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });
});

function crawl4AiHandler(request: IncomingMessage, response: ServerResponse): void {
  if (request.method !== "POST") {
    writeJson(response, 405, { error: "method not allowed" });
    return;
  }
  if (request.url === "/crawl") {
    writeJson(response, 200, {
      results: [
        {
          url: "https://example.com/",
          success: true,
          markdown: {
            fit_markdown: "# Example Domain\n\nUseful crawl markdown for RAG."
          },
          links: {
            internal: [{ href: "https://example.com/about", text: "About" }],
            external: []
          },
          media: {
            images: [{ src: "https://example.com/hero.png", alt: "Hero" }]
          }
        }
      ]
    });
    return;
  }
  if (request.url === "/screenshot") {
    writeJson(response, 200, {
      screenshot: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII="
    });
    return;
  }
  writeJson(response, 404, { error: "not found" });
}

function writeJson(response: ServerResponse, statusCode: number, body: unknown): void {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function restoreEnv(key: string, value: string | undefined): void {
  if (value === undefined) {
    delete process.env[key];
  } else {
    process.env[key] = value;
  }
}
