import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const launcherSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.cmd"), "utf8");
const viteServerSource = readFileSync(join(process.cwd(), "scripts", "ensure-vite-dev-server.ps1"), "utf8");
const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

describe("desktop launcher Vite dev server fast path", () => {
  it("defaults the desktop shortcut to a renderer dev server while keeping stable mode opt-out", () => {
    expect(launcherSource).toContain("setlocal EnableExtensions EnableDelayedExpansion");
    expect(launcherSource).toContain("set FORGE_ELECTRON_VITE_SERVER_SCRIPT=%~dp0scripts\\ensure-vite-dev-server.ps1");
    expect(launcherSource).toContain("set VITE_DEV_SERVER_URL_FILE=C:\\tmp\\ingen-vite-%WORKSPACE_BUILD_ID%.url");
    expect(launcherSource).toContain("set DESKTOP_DEV_SERVER=1");
    expect(launcherSource).toContain('if "%FORGE_ELECTRON_DESKTOP_STABLE%"=="1" set DESKTOP_DEV_SERVER=0');
    expect(launcherSource).toContain('if "%FORGE_ELECTRON_DESKTOP_DEV_SERVER%"=="0" set DESKTOP_DEV_SERVER=0');
    expect(launcherSource).toContain('if "%FORGE_ELECTRON_DESKTOP_DEV_SERVER%"=="1" set DESKTOP_DEV_SERVER=1');
  });

  it("separates Electron main rebuilds from renderer rebuilds", () => {
    expect(launcherSource).toContain("set NEED_RENDERER_REBUILD=0");
    expect(launcherSource).toContain("src\\main");
    expect(launcherSource).toContain("src\\preload");
    expect(launcherSource).toContain("tsconfig.electron.json");
    expect(launcherSource).toContain("src\\renderer");
    expect(launcherSource).toContain("vite.config.ts");
    expect(launcherSource).toContain("if errorlevel 1 set NEED_RENDERER_REBUILD=1");
    expect(launcherSource).toContain('if "%NEED_RENDERER_REBUILD%"=="1" (');
  });

  it("starts or reuses Vite and lets Electron load it through VITE_DEV_SERVER_URL", () => {
    expect(launcherSource).toContain("Ensuring Vite renderer dev server");
    expect(launcherSource).toContain("-File \"%FORGE_ELECTRON_VITE_SERVER_SCRIPT%\"");
    expect(launcherSource).toContain("-WorkspaceBuildId \"%WORKSPACE_BUILD_ID%\"");
    expect(launcherSource).toContain("-UrlPath \"%VITE_DEV_SERVER_URL_FILE%\"");
    expect(launcherSource).not.toContain("for /f \"usebackq delims=\" %%U");
    expect(launcherSource).toContain('set /p VITE_DEV_SERVER_URL=<"%VITE_DEV_SERVER_URL_FILE%"');
    expect(launcherSource).toContain("Vite renderer dev server ready: !VITE_DEV_SERVER_URL!");
    expect(launcherSource).toContain("Using Vite renderer dev server");
    expect(mainSource).toContain("process.env.VITE_DEV_SERVER_URL");
    expect(mainSource).toContain("await window.loadURL(labWindow ? `${process.env.VITE_DEV_SERVER_URL}/event-text-lab.html` : process.env.VITE_DEV_SERVER_URL)");
  });

  it("never restarts the active desktop app while preparing fresh build output", () => {
    expect(launcherSource).toContain("Any build work will prepare the next launch; the active app will not be restarted.");
    expect(launcherSource).toContain("Active InGen window preserved. Focusing the existing app instead of restarting it.");
    expect(launcherSource).toContain("[void]$shell.AppActivate([int]$running.ProcessId)");
    expect(launcherSource).not.toContain("Restarting existing InGen Electron process");
    expect(launcherSource).not.toContain("Stop-Process -Id $_.ProcessId -Force");
  });

  it("continues through Vite freshness checks after waiting for another launcher lock", () => {
    expect(launcherSource).toContain("Waiting briefly, then continuing freshness checks");
    expect(launcherSource).toContain(":build_lock_ready");
    expect(launcherSource).toContain(":own_build_lock");
    expect(launcherSource).toContain("owner.txt");
    expect(launcherSource).toContain("Removed stale InGen launcher build lock.");
    expect(launcherSource).toContain("if errorlevel 1 goto fail");
    expect(launcherSource).not.toContain("Waiting briefly, then starting the fresh build output");
  });

  it("keeps the Vite helper local, deterministic, and reusable by workspace", () => {
    expect(viteServerSource).toContain("node_modules\\vite\\bin\\vite.js");
    expect(viteServerSource).toContain("Invoke-WebRequest -UseBasicParsing -Uri \"http://127.0.0.1:$Port/\"");
    expect(viteServerSource).toContain("5173 + ($seed % 300)");
    expect(viteServerSource).toContain("--strictPort");
    expect(viteServerSource).toContain("System.Diagnostics.ProcessStartInfo");
    expect(viteServerSource).toContain("where.exe node");
    expect(viteServerSource).toContain("$startInfo.CreateNoWindow = $true");
    expect(viteServerSource).toContain("function Emit-DevServerUrl");
    expect(viteServerSource).toContain("Set-Content -LiteralPath $UrlPath -Value $url -Encoding ASCII");
    expect(viteServerSource).toContain("[Console]::Out.WriteLine($url)");
    expect(viteServerSource).toContain("Vite renderer dev server ready: $url.");
    expect(viteServerSource).toContain("Vite dev server did not become ready");
  });
});
