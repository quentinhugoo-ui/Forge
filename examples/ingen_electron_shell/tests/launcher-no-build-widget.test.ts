import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const launcherSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.cmd"), "utf8");

describe("desktop launcher without build progress widget", () => {
  it("does not start or reference the retired build widget", () => {
    expect(launcherSource).not.toContain("FORGE_ELECTRON_BUILD_WIDGET");
    expect(launcherSource).not.toContain("launcher-build-progress-widget.ps1");
    expect(launcherSource).not.toContain("-Sta -File");
    expect(existsSync(join(process.cwd(), "scripts", "launcher-build-progress-widget.ps1"))).toBe(false);
  });
});
