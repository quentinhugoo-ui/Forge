import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

describe("Codex LLM provider subscription runtime", () => {
  it("uses local Codex OAuth subscription credentials instead of the paid OpenAI API", () => {
    expect(mainSource).toContain("readCodexLocalAuth");
    expect(mainSource).toContain("runCodexOAuthDirect");
    expect(mainSource).toContain(".codex");
    expect(mainSource).toContain("auth.json");
    expect(mainSource).toContain("models_cache.json");
    expect(mainSource).toContain("readCodexLocalModelCatalog");
    expect(mainSource).toContain('await applyCodexLocalAuthProfile(["refresh Codex local model catalog"])');
    expect(mainSource).toContain('String(entry.visibility ?? "list").toLowerCase() === "list"');
    expect(mainSource).toContain("GPT-5.6-Sol");
    expect(mainSource).toContain("GPT-5.6-Terra");
    expect(mainSource).toContain("GPT-5.6-Luna");
    expect(mainSource).toContain("https://chatgpt.com/backend-api/codex/responses");
    expect(mainSource).toContain("ChatGPT-Account-ID");
    expect(mainSource).toContain("responses=experimental");
    expect(mainSource).toContain("applyCodexLocalAuthProfile");
    expect(mainSource).not.toContain("api.openai.com");
    expect(mainSource).not.toContain('"exec",\n    "--skip-git-repo-check"');
    expect(mainSource).not.toContain("codex exec");
    expect(mainSource).not.toContain('source: "cookie"');
  });
});
