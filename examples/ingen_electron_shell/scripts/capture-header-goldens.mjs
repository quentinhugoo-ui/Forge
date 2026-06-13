import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const shellRoot = join(here, "..");
const outDir = join(shellRoot, "reference_screenshots", "electron", "header");
const generatedDir = join(shellRoot, "src", "shared", "generated");
const manifestPath = join(generatedDir, "header.visual-golden-manifest.generated.json");

const viewports = [
  { id: "desktop", width: 1535, height: 786, deviceScaleFactor: 1 },
  { id: "narrow", width: 1180, height: 760, deviceScaleFactor: 1 }
];

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
      port: 5176,
      strictPort: true
    }
  });
  await server.listen();

  const browser = await chromium.launch();
  const captures = [];

  try {
    for (const viewport of viewports) {
      const page = await browser.newPage({
        viewport: { width: viewport.width, height: viewport.height },
        deviceScaleFactor: viewport.deviceScaleFactor
      });
      await page.goto("http://127.0.0.1:5176", { waitUntil: "networkidle" });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.evaluate(() => document.fonts.ready);

      const screenshot = await page.screenshot({
        fullPage: true,
        type: "png",
        animations: "disabled"
      });
      const screenshotPath = join(outDir, `header-${viewport.id}.png`);
      writeFileSync(screenshotPath, screenshot);

      const ariaSnapshot = await page.locator("main.shell").ariaSnapshot();
      const ariaPath = join(outDir, `header-${viewport.id}.aria.yml`);
      writeFileSync(ariaPath, `${ariaSnapshot}\n`);

      const metrics = await page.evaluate(() => {
        const titlebar = document.querySelector(".titlebar");
        const workspaceHeader = document.querySelector(".workspaceHeader");
        const shadowLedger = document.querySelector(".shadowLedger");
        const headerRoots = [titlebar, workspaceHeader, shadowLedger].filter(Boolean);
        const buttons = [
          ...new Set(headerRoots.flatMap((root) => [...root.querySelectorAll("button")]))
        ].map((button) => {
          const rect = button.getBoundingClientRect();
          return {
            label: button.getAttribute("aria-label") ?? "",
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            overflow: button.scrollWidth > button.clientWidth || button.scrollHeight > button.clientHeight
          };
        });
        const layoutShifts = performance
          .getEntriesByType("layout-shift")
          .filter((entry) => !entry.hadRecentInput)
          .map((entry) => entry.value);
        return {
          titlebar: titlebar
            ? { width: titlebar.clientWidth, height: titlebar.clientHeight }
            : null,
          workspaceHeader: workspaceHeader
            ? {
                width: workspaceHeader.clientWidth,
                height: workspaceHeader.clientHeight,
                left: Math.round(workspaceHeader.getBoundingClientRect().left)
              }
            : null,
          shadowLedger: shadowLedger
            ? {
                width: shadowLedger.clientWidth,
                height: shadowLedger.clientHeight,
                overflowX: shadowLedger.scrollWidth > shadowLedger.clientWidth,
                overflowY: shadowLedger.scrollHeight > shadowLedger.clientHeight
              }
            : null,
          buttonCount: buttons.length,
          buttons,
          cls: layoutShifts.reduce((sum, value) => sum + value, 0)
        };
      });

      captures.push({
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
    schema: "ingen.electron.header_visual_golden_manifest.v1",
    slice_id: "header",
    source: "examples/ingen_electron_shell/scripts/capture-header-goldens.mjs",
    status: "captured",
    generated_by: "vite+playwright-buffer-screenshot",
    captures,
    gates: {
      expected_button_count: 11,
      max_cls: 0,
      no_shadow_ledger_overflow: true,
      min_icon_button_width: 32,
      min_icon_button_height: 32
    },
    proof_hash: ""
  };
  manifest.proof_hash = hashJson({ ...manifest, proof_hash: "" });
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(JSON.stringify(manifest, null, 2));
}

await main();
