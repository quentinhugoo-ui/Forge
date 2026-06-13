import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const shellRoot = join(here, "..");
const outDir = join(shellRoot, "reference_screenshots", "electron", "panels_chat_bottom");
const generatedDir = join(shellRoot, "src", "shared", "generated");
const manifestPath = join(generatedDir, "panels_chat_bottom.visual-golden-manifest.generated.json");

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
      port: 5177,
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
      await page.goto("http://127.0.0.1:5177", { waitUntil: "networkidle" });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.evaluate(() => document.fonts.ready);

      const screenshot = await page.locator(".panelsChatBottom").screenshot({
        type: "png",
        animations: "disabled"
      });
      const screenshotPath = join(outDir, `panels-chat-bottom-${viewport.id}.png`);
      writeFileSync(screenshotPath, screenshot);

      const ariaSnapshot = await page.locator(".panelsChatBottom").ariaSnapshot();
      const ariaPath = join(outDir, `panels-chat-bottom-${viewport.id}.aria.yml`);
      writeFileSync(ariaPath, `${ariaSnapshot}\n`);

      const metrics = await page.evaluate(() => {
        const composer = document.querySelector(".composer");
        const transcript = document.querySelector(".transcriptRail");
        const statusDock = document.querySelector(".statusDock");
        const bottomControls = document.querySelector(".bottomControls");
        const all = [composer, transcript, statusDock, bottomControls].filter(Boolean);
        const boxes = Object.fromEntries(
          all.map((node) => {
            const rect = node.getBoundingClientRect();
            return [
              node.className,
              {
                left: Math.round(rect.left),
                top: Math.round(rect.top),
                width: Math.round(rect.width),
                height: Math.round(rect.height),
                overflowX: node.scrollWidth > node.clientWidth,
                overflowY: node.scrollHeight > node.clientHeight
              }
            ];
          })
        );
        const buttons = [...document.querySelectorAll(".panelsChatBottom button")].map((button) => {
          const rect = button.getBoundingClientRect();
          return {
            label: button.getAttribute("aria-label") ?? button.textContent?.trim() ?? "",
            width: Math.round(rect.width),
            height: Math.round(rect.height)
          };
        });
        return {
          boxes,
          buttonCount: buttons.length,
          buttons,
          composerInputPresent: Boolean(document.querySelector(".composer input")),
          horizontalOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
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
    schema: "ingen.electron.panels_chat_bottom_visual_golden_manifest.v1",
    slice_id: "panels_chat_bottom",
    source: "examples/ingen_electron_shell/scripts/capture-panels-chat-bottom-goldens.mjs",
    status: "captured",
    generated_by: "vite+playwright-buffer-screenshot",
    captures,
    gates: {
      expected_button_count_min: 8,
      composer_input_present: true,
      no_horizontal_overflow: true,
      min_button_width: 24,
      min_button_height: 24
    },
    proof_hash: ""
  };
  manifest.proof_hash = hashJson({ ...manifest, proof_hash: "" });
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(JSON.stringify(manifest, null, 2));
}

await main();
