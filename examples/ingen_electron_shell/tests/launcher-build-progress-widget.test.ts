import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const launcherSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.cmd"), "utf8");
const widgetSource = readFileSync(join(process.cwd(), "scripts", "launcher-build-progress-widget.ps1"), "utf8");

describe("desktop launcher build progress widget", () => {
  it("starts a non-blocking widget with the launch log and workspace process identity", () => {
    expect(launcherSource).toContain("FORGE_ELECTRON_BUILD_WIDGET_SCRIPT");
    expect(launcherSource).toContain("launcher-build-progress-widget.ps1");
    expect(launcherSource).toContain('if not "%FORGE_ELECTRON_BUILD_WIDGET%"=="0"');
    expect(launcherSource).toContain("-Sta -File");
    expect(launcherSource).toContain("-LogPath \"%LOG%\"");
    expect(launcherSource).toContain("-ElectronPath \"%FORGE_ELECTRON_EXE%\"");
    expect(launcherSource).toContain("-ShellRoot \"%~dp0\"");
    expect(launcherSource).toContain("-BuildLockPath \"%BUILD_LOCK%\"");
  });

  it("renders as a frameless transparent top-right desktop overlay", () => {
    expect(widgetSource).toContain('WindowStyle="None"');
    expect(widgetSource).toContain('AllowsTransparency="True"');
    expect(widgetSource).toContain('Background="Transparent"');
    expect(widgetSource).toContain('ShowInTaskbar="False"');
    expect(widgetSource).toContain('ShowActivated="False"');
    expect(widgetSource).toContain('Topmost="True"');
    expect(widgetSource).toContain("[System.Windows.SystemParameters]::WorkArea");
    expect(widgetSource).toContain("$workArea.Right - $window.Width - 24");
    expect(widgetSource).toContain("$workArea.Top + 24");
  });

  it("closes as soon as the matching Electron desktop shell is running", () => {
    expect(widgetSource).toContain("function Test-InGenElectronRunning");
    expect(widgetSource).toContain("Get-CimInstance Win32_Process -Filter \"Name = 'electron.exe'\"");
    expect(widgetSource).toContain("$path -eq $normalizedElectronPath");
    expect(widgetSource).toContain("$commandLine.Contains($normalizedShellRoot)");
    expect(widgetSource).toContain("if (Test-InGenElectronRunning)");
    expect(widgetSource).toContain("$window.Close()");
  });

  it("tracks build phases from launcher log lines", () => {
    expect(widgetSource).toContain("Building Rust backend bridge");
    expect(widgetSource).toContain("Building Windows taskbar helper");
    expect(widgetSource).toContain("Building Electron main process");
    expect(widgetSource).toContain("Building Electron renderer");
    expect(widgetSource).toContain("Another InGen launcher");
    expect(widgetSource).toContain("Starting Electron");
  });
});
