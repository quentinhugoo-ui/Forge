import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const shellRoot = join(here, "..");
const outDir = join(shellRoot, "reference_screenshots", "electron", "sidebar");
const generatedDir = join(shellRoot, "src", "shared", "generated");
const manifestPath = join(generatedDir, "sidebar.visual-golden-manifest.generated.json");

const states = [
  { id: "sidebar", query: "" },
  { id: "sessions-recents", query: "?sidebar-state=sessions-recents" },
  { id: "sessions-archived", query: "?sidebar-state=sessions-archived" },
  { id: "profile", query: "?sidebar-state=profile" }
];
const viewport = { id: "desktop", width: 1535, height: 786, deviceScaleFactor: 1 };

function hashBytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function hashJson(value) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

async function main() {
  mkdirSync(outDir, { recursive: true });
  mkdirSync(generatedDir, { recursive: true });

  const server = await createServer({
    configFile: join(shellRoot, "vite.config.ts"),
    server: {
      host: "127.0.0.1",
      port: 5177,
      strictPort: true
    }
  });
  await server.listen();

  const browser = await chromium.launch();
  const captures = [];

  try {
    for (const state of states) {
      const page = await browser.newPage({
        viewport: { width: viewport.width, height: viewport.height },
        deviceScaleFactor: viewport.deviceScaleFactor
      });
      await page.goto(`http://127.0.0.1:5177/${state.query}`, { waitUntil: "networkidle" });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.evaluate(() => document.fonts.ready);

      const screenshot = await page.screenshot({
        fullPage: true,
        type: "png",
        animations: "disabled"
      });
      const screenshotPath = join(outDir, `sidebar-${state.id}.png`);
      writeFileSync(screenshotPath, screenshot);

      const ariaSnapshot = await page.locator("main.shell").ariaSnapshot();
      const ariaPath = join(outDir, `sidebar-${state.id}.aria.yml`);
      writeFileSync(ariaPath, `${ariaSnapshot}\n`);

      const metrics = await page.evaluate(() => {
        const leftPanel = document.querySelector(".leftPanel");
        const sessionsCanvas = document.querySelector(".sessionsCanvas");
        const profileCanvas = document.querySelector(".profileCanvas");
        const buttons = [...document.querySelectorAll(".leftPanel button, .sessionsCanvas button, .profileCanvas button")];
        return {
          leftPanel: leftPanel
            ? {
                width: leftPanel.clientWidth,
                height: leftPanel.clientHeight,
                overflowX: leftPanel.scrollWidth > leftPanel.clientWidth
              }
            : null,
          sessionsCanvasVisible: Boolean(sessionsCanvas),
          profileCanvasVisible: Boolean(profileCanvas),
          sidebarButtonCount: buttons.length,
          buttonOverflowCount: buttons.filter((button) => button.scrollWidth > button.clientWidth || button.scrollHeight > button.clientHeight).length
        };
      });

      captures.push({
        state,
        viewport,
        screenshot: relative(shellRoot, screenshotPath).replaceAll("\\", "/"),
        screenshot_sha256: hashBytes(screenshot),
        aria_snapshot: relative(shellRoot, ariaPath).replaceAll("\\", "/"),
        aria_sha256: hashBytes(readFileSync(ariaPath)),
        metrics
      });

      await page.close();
    }
  } finally {
    await browser.close();
    await server.close();
  }

  const manifest = {
    schema: "ingen.electron.sidebar_visual_golden_manifest.v1",
    slice_id: "sidebar_sessions",
    source: "examples/ingen_electron_shell/scripts/capture-sidebar-goldens.mjs",
    status: "captured",
    generated_by: "vite+playwright-buffer-screenshot",
    captures,
    gates: {
      expected_states: states.map((state) => state.id),
      max_button_overflow_count: 0,
      left_panel_width: 279
    },
    proof_hash: ""
  };
  manifest.proof_hash = hashJson({ ...manifest, proof_hash: "" });
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(JSON.stringify(manifest, null, 2));
}

await main();
