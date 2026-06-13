import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(process.cwd(), "src", "preload", "preload.ts"), "utf8");
const appSource = readFileSync(join(process.cwd(), "src", "renderer", "App.tsx"), "utf8");
const canvasSource = readFileSync(join(process.cwd(), "src", "renderer", "CanvasSurfacesSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(process.cwd(), "src", "renderer", "styles.css"), "utf8");
const packageSource = readFileSync(join(process.cwd(), "package.json"), "utf8");

describe("embedded native Windows terminal host", () => {
  it("attaches the real PowerShell window into the Electron pane through Win32", () => {
    expect(mainSource).toContain("function showNativeTerminal(event: Electron.IpcMainInvokeEvent, bounds: NativeTerminalBounds): NativeTerminalResult");
    expect(mainSource).toContain("waitForProcessMainWindow(child.pid");
    expect(mainSource).toContain("function attachNativeTerminalWindow");
    expect(mainSource).toContain("SetParent");
    expect(mainSource).toContain("MoveWindow");
    expect(mainSource).toContain('command: "powershell.exe"');
    expect(mainSource).toContain("nativeTerminalHwnd");
  });

  it("drives terminal bounds from the renderer slot instead of opening an external launcher", () => {
    expect(preloadSource).toContain("showNativeTerminal(bounds: NativeTerminalBounds)");
    expect(preloadSource).toContain('ipcRenderer.invoke("forge:terminal-show-native", bounds)');
    expect(canvasSource).toContain("terminalSlotRef");
    expect(canvasSource).toContain("showNativeTerminal");
    expect(canvasSource).toContain("updateNativeTerminalBounds");
    expect(canvasSource).toContain("hideNativeTerminal");
    expect(canvasSource).toContain("canvasTerminalPane__nativeHost");
    expect(canvasSource).not.toContain("Open PowerShell");
  });

  it("lets the top-right sidebar icon close open Files and Terminal canvas panes", () => {
    expect(appSource).toContain('if (control.id === "right-panel")');
    expect(appSource).toContain("if (canvasFilesOpen || canvasTerminalOpen)");
    expect(appSource).toContain("setCanvasFilesOpen(false)");
    expect(appSource).toContain("setCanvasTerminalOpen(false)");
    expect(appSource).toContain('setCanvasActivePane("")');
    expect(appSource).toContain("setCanvasSplitOpen(false)");
    expect(appSource).toContain("? canvasSplitOpen || canvasFilesOpen || canvasTerminalOpen");
  });

  it("adds Files and Terminal as tabs instead of replacing the current pane", () => {
    expect(appSource).toContain("const [canvasActivePane, setCanvasActivePane]");
    expect(appSource).toContain('setCanvasActivePane("files")');
    expect(appSource).toContain('setCanvasActivePane("terminal")');
    expect(appSource).toContain("activePane={canvasActivePane}");
    expect(appSource).toContain("onActivePaneChange={setCanvasActivePane}");
    expect(canvasSource).toContain("openToolPanes");
    expect(canvasSource).toContain('activeToolPane === "files"');
    expect(canvasSource).toContain('activeToolPane === "terminal"');
    expect(canvasSource).toContain('role="tablist"');
    expect(canvasSource).toContain("onActivatePane");
    expect(canvasSource).toContain("Files from Another Session");
    expect(canvasSource).toContain('aria-label="Open files from another session"');
    expect(canvasSource).toContain('className="canvasPaneTabs__addWrap"');
    expect(stylesSource).toContain(".canvasPaneTabs__addWrap");
    expect(stylesSource).toContain(".canvasPaneTabs__menu--sessionFiles");
    expect(stylesSource).toContain("width: calc(100% - 20px);");
    expect(stylesSource).toContain("max-height: min(560px, calc(100vh - 210px));");
  });

  it("does not ship an HTML terminal emulator path", () => {
    expect(packageSource).not.toContain("node-pty");
    expect(packageSource).not.toContain("@xterm/xterm");
    expect(mainSource).not.toContain("pty.spawn");
    expect(canvasSource).not.toContain('import { Terminal } from "@xterm/xterm"');
    expect(canvasSource).not.toContain("terminal.write(event.data);");
  });
});
