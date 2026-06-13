import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

describe("Claude LLM provider runtime contract", () => {
  it("uses the official Claude Code installers per host OS", () => {
    expect(mainSource).toContain("irm https://claude.ai/install.ps1 | iex");
    expect(mainSource).toContain("curl -fsSL https://claude.ai/install.sh | bash");
  });

  it("verifies the installed runtime before reporting readiness", () => {
    expect(mainSource).toContain('captureCommand(command, ["--version"]');
    expect(mainSource).toContain('captureCommand(command, ["auth", "status"]');
    expect(mainSource).toContain("runtimeVerified && status.exitCode === 0");
    expect(mainSource).toContain("runtime version verified:");
  });

  it("surfaces whether installation ran or was skipped because the runtime already exists", () => {
    expect(mainSource).toContain("installer command:");
    expect(mainSource).toContain("Claude Code installer completed with exit code");
    expect(mainSource).toContain("installer skipped: Claude Code runtime already executable");
    expect(mainSource).toContain("Claude Code install finished but runtime verification failed");
  });
});
