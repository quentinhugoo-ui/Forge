import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const nativeRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const repoRoot = resolve(nativeRoot, "..", "..");
const tauriRoot = join(repoRoot, "examples", "forge_tauri_ui");
const uiRoot = join(tauriRoot, "ui");
const outSlint = join(nativeRoot, "ui", "app.slint");
const outReport = join(nativeRoot, "design", "tauri_front_slint_import_report.json");
const generatedAssets = join(nativeRoot, "ui", "generated_assets");
const requireFromTauri = createRequire(join(tauriRoot, "package.json"));
const { chromium } = requireFromTauri("@playwright/test");

const viewport = { width: 1535, height: 786 };
const outputViewport = viewport;
const cssScale = 1;
const renderMode = "native";
const mime = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".wasm", "application/wasm"],
  [".json", "application/json; charset=utf-8"],
  [".svg", "image/svg+xml"],
  [".png", "image/png"],
]);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stableJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;
}

function slintString(value) {
  return JSON.stringify(String(value).replace(/\s+/g, " ").trim());
}

function px(value, scale = 1) {
  const n = Number(value);
  return `${Math.round(n * scale * 100) / 100}px`;
}

function colorToSlint(value) {
  const match = String(value).match(/rgba?\(([^)]+)\)/);
  if (!match) return "#00000000";
  const parts = match[1].split(",").map((item) => item.trim());
  const r = Number(parts[0]);
  const g = Number(parts[1]);
  const b = Number(parts[2]);
  const a = parts.length > 3 ? Number(parts[3]) : 1;
  const hex = [r, g, b, Math.round(a * 255)]
    .map((n) => Math.max(0, Math.min(255, n)).toString(16).padStart(2, "0"))
    .join("");
  return a >= 0.999 ? `#${hex.slice(0, 6)}` : `#${hex}`;
}

function serveUi() {
  const server = createServer((req, res) => {
    try {
      const url = new URL(req.url || "/", "http://127.0.0.1");
      const rawPath = decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname);
      if (rawPath === "/dist/forge-front-gate.js") {
        res.writeHead(200, {
          "content-type": "text/javascript; charset=utf-8",
          "cache-control": "no-store",
        });
        res.end("// disabled by Slint visual importer\n");
        return;
      }
      const candidate = normalize(join(uiRoot, rawPath));
      if (!candidate.startsWith(uiRoot + sep)) {
        res.writeHead(403);
        res.end("forbidden");
        return;
      }
      if (!existsSync(candidate)) {
        res.writeHead(404);
        res.end("not found");
        return;
      }
      res.writeHead(200, {
        "content-type": mime.get(extname(candidate)) || "application/octet-stream",
        "cache-control": "no-store",
      });
      createReadStream(candidate).pipe(res);
    } catch (err) {
      res.writeHead(500);
      res.end(String(err?.message || err));
    }
  });
  return new Promise((resolveServer) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolveServer({ server, origin: `http://127.0.0.1:${address.port}` });
    });
  });
}

async function extractScene(page) {
  return await page.evaluate(() => {
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    const ignored = new Set(["SCRIPT", "STYLE", "LINK", "META", "TITLE", "TEMPLATE"]);
    const directText = (element) => Array.from(element.childNodes)
      .filter((node) => node.nodeType === Node.TEXT_NODE)
      .map((node) => node.textContent || "")
      .join(" ")
      .replace(/\s+/g, " ")
      .trim();
    const rectOf = (element) => {
      const r = element.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    };
    const isVisible = (element, style, rect) => {
      if (ignored.has(element.tagName)) return false;
      if (style.display === "none" || style.visibility === "hidden" || Number(style.opacity) === 0) return false;
      if (rect.width <= 0.5 || rect.height <= 0.5) return false;
      if (rect.x > viewport.width || rect.y > viewport.height || rect.x + rect.width < 0 || rect.y + rect.height < 0) return false;
      return true;
    };
    const entries = [];
    const buttons = [];
    let svgIndex = 0;
    const isInteractive = (element, style) => {
      const tag = element.tagName.toLowerCase();
      if (tag === "button" || tag === "a" || tag === "input" || tag === "select") return true;
      if (element.getAttribute("role") === "button" || element.getAttribute("tabindex") === "0") return true;
      if (style.cursor === "pointer") return true;
      const id = element.id || "";
      const classes = Array.from(element.classList || []).join(" ");
      return /btn|button|tab|nav|toggle|action|control/i.test(`${id} ${classes}`);
    };
    const interactiveLabel = (element) => (
      element.getAttribute("aria-label") ||
      element.getAttribute("title") ||
      element.getAttribute("data-section") ||
      element.getAttribute("data-action") ||
      element.textContent ||
      element.id ||
      Array.from(element.classList || []).join(" ")
    ).replace(/\s+/g, " ").trim();
    for (const element of document.querySelectorAll("body, body *")) {
      const style = getComputedStyle(element);
      const rect = rectOf(element);
      if (!isVisible(element, style, rect)) continue;
      const clipped = {
        x: Math.max(0, rect.x),
        y: Math.max(0, rect.y),
        width: Math.min(viewport.width, rect.x + rect.width) - Math.max(0, rect.x),
        height: Math.min(viewport.height, rect.y + rect.height) - Math.max(0, rect.y),
      };
      if (clipped.width <= 0.5 || clipped.height <= 0.5) continue;
      const bg = style.backgroundColor;
      const borderWidth = Math.max(
        parseFloat(style.borderTopWidth) || 0,
        parseFloat(style.borderRightWidth) || 0,
        parseFloat(style.borderBottomWidth) || 0,
        parseFloat(style.borderLeftWidth) || 0,
      );
      const hasBg = bg !== "rgba(0, 0, 0, 0)" && bg !== "transparent";
      if ((hasBg || borderWidth > 0.5) && clipped.width >= 2 && clipped.height >= 2) {
        entries.push({
          kind: "rect",
          tag: element.tagName.toLowerCase(),
          id: element.id || "",
          classes: Array.from(element.classList || []),
          rect: clipped,
          background: bg,
          borderColor: style.borderTopColor,
          borderWidth,
          radius: parseFloat(style.borderTopLeftRadius) || 0,
        });
      }
      if (element.tagName.toLowerCase() === "svg") {
        const clone = element.cloneNode(true);
        const iconColor = style.color || "rgba(238,238,232,0.72)";
        clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
        if (!clone.getAttribute("fill") || clone.getAttribute("fill") === "currentColor") clone.setAttribute("fill", "none");
        if (!clone.getAttribute("stroke") || clone.getAttribute("stroke") === "currentColor") clone.setAttribute("stroke", iconColor);
        clone.setAttribute("stroke-width", clone.getAttribute("stroke-width") || "1.8");
        clone.setAttribute("stroke-linecap", clone.getAttribute("stroke-linecap") || "round");
        clone.setAttribute("stroke-linejoin", clone.getAttribute("stroke-linejoin") || "round");
        for (const child of clone.querySelectorAll("path,circle,rect,ellipse,line,polyline,polygon")) {
          if (!child.getAttribute("fill") || child.getAttribute("fill") === "currentColor") child.setAttribute("fill", "none");
          if (!child.getAttribute("stroke") || child.getAttribute("stroke") === "currentColor") child.setAttribute("stroke", iconColor);
          if (!child.getAttribute("stroke-width")) child.setAttribute("stroke-width", "1.8");
          if (!child.getAttribute("stroke-linecap")) child.setAttribute("stroke-linecap", "round");
          if (!child.getAttribute("stroke-linejoin")) child.setAttribute("stroke-linejoin", "round");
        }
        entries.push({
          kind: "svg",
          rect: clipped,
          source: clone.outerHTML,
          color: style.color,
        });
        svgIndex += 1;
        continue;
      }
      if (element !== document.body && element.scrollHeight > element.clientHeight + 2 && clipped.height >= 24 && clipped.width >= 24) {
        const trackWidth = 3;
        const trackHeight = clipped.height;
        const thumbHeight = Math.max(32, trackHeight * (element.clientHeight / element.scrollHeight));
        const maxScroll = Math.max(1, element.scrollHeight - element.clientHeight);
        const thumbY = clipped.y + (trackHeight - thumbHeight) * (element.scrollTop / maxScroll);
        entries.push({
          kind: "rect",
          tag: "native-scrollbar-track",
          id: "",
          classes: ["generated-scrollbar-track"],
          rect: { x: clipped.x + clipped.width - trackWidth - 1, y: clipped.y, width: trackWidth, height: trackHeight },
          background: "rgba(255, 255, 255, 0.06)",
          borderColor: "rgba(0, 0, 0, 0)",
          borderWidth: 0,
          radius: 999,
        });
        entries.push({
          kind: "rect",
          tag: "native-scrollbar-thumb",
          id: "",
          classes: ["generated-scrollbar-thumb"],
          rect: { x: clipped.x + clipped.width - trackWidth - 1, y: thumbY, width: trackWidth, height: thumbHeight },
          background: "rgba(210, 210, 206, 0.5)",
          borderColor: "rgba(0, 0, 0, 0)",
          borderWidth: 0,
          radius: 999,
        });
      }
      if (isInteractive(element, style)) {
        buttons.push({
          id: element.id || "",
          classes: Array.from(element.classList || []),
          rect: clipped,
          label: interactiveLabel(element),
        });
      }
      const text = directText(element);
      if (text && clipped.width >= 4 && clipped.height >= 4) {
        entries.push({
          kind: "text",
          tag: element.tagName.toLowerCase(),
          id: element.id || "",
          classes: Array.from(element.classList || []),
          rect: clipped,
          text,
          color: style.color,
          fontSize: parseFloat(style.fontSize) || 14,
          fontWeight: Number(style.fontWeight) || (style.fontWeight === "bold" ? 700 : 400),
          textAlign: style.textAlign,
        });
      }
    }
    return { viewport, entries, buttons, svgCount: svgIndex };
  });
}

function emitButtonOverlays(buttons) {
  const routeFor = (button) => {
    if (button.id === "webexplorer") return 'root.navigate("webexplorer");';
    if (button.id === "bangerBoomBtn") return 'root.navigate("banger");';
    if (button.id === "tradingWorkspaceBtn") return 'root.navigate("trading");';
    if (button.id === "realEstateModeBtn" || button.id === "realEstateHomeSectionBtn") return 'root.navigate("real-estate");';
    if (button.id === "alphaNewSessionBtn" || button.id === "panelTabCompute") return 'root.navigate("forge");';
    if (button.id === "panelTabAtlas") return 'root.navigate("webexplorer");';
    if (button.id === "windowClose") return "root.window_close();";
    if (button.id === "windowMinimize") return "root.window_minimize();";
    if (button.id === "windowMaximize") return "root.window_toggle_maximize();";
    if (button.id === "navLibraryBtn" || button.id === "navProgramsBtn") return 'root.navigate("forge");';
    const label = button.label || button.id || button.classes.join(".");
    return `root.activate(${slintString(label || "unlabelled control")});`;
  };
  const rectContains = (outer, inner) => (
    inner.x >= outer.x &&
    inner.y >= outer.y &&
    inner.x + inner.width <= outer.x + outer.width &&
    inner.y + inner.height <= outer.y + outer.height
  );
  const area = (rect) => rect.width * rect.height;
  const overlays = buttons
    .map((button, index) => ({ button, index, action: routeFor(button) }))
    .filter((overlay) => overlay.action)
    .filter((overlay, _index, all) => {
      const rect = overlay.button.rect;
      return !all.some((candidate) => {
        if (candidate === overlay || !candidate.button.id) return false;
        if (!rectContains(candidate.button.rect, rect)) return false;
        return area(candidate.button.rect) >= area(rect) * 1.8;
      });
    })
    .sort((a, b) => area(a.button.rect) - area(b.button.rect) || a.index - b.index);
  return overlays
    .map((overlay) => {
      const rect = overlay.button.rect;
      return `    TouchArea { x: ${px(rect.x, cssScale)}; y: ${px(rect.y, cssScale)}; width: ${px(rect.width, cssScale)}; height: ${px(rect.height, cssScale)}; clicked => { ${overlay.action} } }`;
    })
    .filter(Boolean)
    .join("\n");
}

function emitLayerBody(scene, svgCounterRef) {
  const body = [];
  for (const entry of scene.entries) {
    if (entry.kind === "rect") {
      body.push(`    Rectangle { x: ${px(entry.rect.x, cssScale)}; y: ${px(entry.rect.y, cssScale)}; width: ${px(entry.rect.width, cssScale)}; height: ${px(entry.rect.height, cssScale)}; background: ${colorToSlint(entry.background)}; border-color: ${colorToSlint(entry.borderColor)}; border-width: ${px(entry.borderWidth, cssScale)}; border-radius: ${px(entry.radius, cssScale)}; }`);
    } else if (entry.kind === "svg") {
      const fileName = `svg_${String(svgCounterRef.value++).padStart(4, "0")}.svg`;
      writeFileSync(join(generatedAssets, fileName), entry.source);
      body.push(`    Image { x: ${px(entry.rect.x, cssScale)}; y: ${px(entry.rect.y, cssScale)}; width: ${px(entry.rect.width, cssScale)}; height: ${px(entry.rect.height, cssScale)}; source: @image-url("generated_assets/${fileName}"); }`);
    } else if (entry.kind === "text") {
      const align = entry.textAlign === "center" ? "center" : entry.textAlign === "right" ? "right" : "left";
      const textHeight = Math.max(entry.rect.height, entry.fontSize * 1.65);
      const textY = entry.rect.y - Math.max(0, (textHeight - entry.rect.height) / 2);
      const wrap = entry.text.length > 32 || entry.rect.height > entry.fontSize * 1.8 ? " wrap: word-wrap;" : "";
      body.push(`    Text { x: ${px(entry.rect.x, cssScale)}; y: ${px(textY, cssScale)}; width: ${px(entry.rect.width + 4, cssScale)}; height: ${px(textHeight, cssScale)}; text: ${slintString(entry.text)}; color: ${colorToSlint(entry.color)}; font-size: ${px(entry.fontSize, cssScale)}; font-weight: ${Math.round(entry.fontWeight)}; horizontal-alignment: ${align}; vertical-alignment: center;${wrap} }`);
    }
  }
  return body.join("\n");
}

function emitLayerComponent(scene, svgCounterRef) {
  const componentName = `GeneratedLayer${scene.name.replace(/(^|-)([a-z])/g, (_, _dash, letter) => letter.toUpperCase())}`;
  return `component ${componentName} inherits Rectangle {
    in property <bool> layer_visible: false;
    in-out property <string> chat_text;
    callback navigate(string);
    callback activate(string);
    callback window_close();
    callback window_minimize();
    callback window_toggle_maximize();
    visible: root.layer_visible;
    x: 0px;
    y: 0px;
    width: ${px(outputViewport.width)};
    height: ${px(outputViewport.height)};
    background: #0e0e0f;
    clip: true;
${renderMode === "bitmap" ? `    Image {
        x: 0px;
        y: 0px;
        width: parent.width;
        height: parent.height;
        source: @image-url("generated_assets/layer_${scene.name}.png");
    }` : emitLayerBody(scene, svgCounterRef)}
${emitButtonOverlays(scene.buttons || []).replaceAll("root.navigate", "root.navigate")}
    LineEdit {
        x: 624px;
        y: 669px;
        width: 552px;
        height: 37px;
        text <=> root.chat_text;
        placeholder-text: "Mine this 50 GB metagenome to invent a motif family that predicts";
    }
}
`;
}

function emitSlint(scenes) {
  const svgCounterRef = { value: 0 };
  const layerComponents = scenes.map((scene) => emitLayerComponent(scene, svgCounterRef)).join("\n");
  const layerInstances = scenes.map((scene) => {
    const componentName = `GeneratedLayer${scene.name.replace(/(^|-)([a-z])/g, (_, _dash, letter) => letter.toUpperCase())}`;
    const visibleExpr = scene.name === "forge"
      ? 'root.active_section == "shell" || root.active_section == "alpha" || root.active_section == "forge"'
      : `root.active_section == "${scene.name}"`;
    return `    ${componentName} { layer_visible: ${visibleExpr}; chat_text <=> root.chat_text; navigate(section) => { root.navigate(section); } activate(label) => { root.activate_imported_control(label); } window_close => { root.window_close(); } window_minimize => { root.window_minimize(); } window_toggle_maximize => { root.window_toggle_maximize(); } }`;
  }).join("\n");
  return `// Generated by examples/ingen_native_front/scripts/import_tauri_front_to_slint.mjs
// Source of truth: examples/forge_tauri_ui/ui/index.html + styles.css + dist runtime.
// Do not hand-edit generated geometry; change the importer or the source front.
import { LineEdit } from "std-widgets.slint";

${layerComponents}

component NativeModal inherits Rectangle {
    in property <bool> modal_visible;
    in property <string> modal_title;
    in property <string> modal_body;
    callback close_modal();
    visible: root.modal_visible;
    background: #00000088;
    TouchArea { clicked => { root.close_modal(); } }
    Rectangle {
        width: 420px;
        height: 168px;
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        background: #232322;
        border-color: #353534;
        border-width: 1px;
        border-radius: 6px;
        Text { x: 16px; y: 16px; text: root.modal_title; color: #e8ecee; font-size: 16px; font-weight: 700; }
        Text { x: 16px; y: 48px; width: 388px; text: root.modal_body; color: #9a9a94; font-size: 12px; wrap: word-wrap; }
        Rectangle {
            x: 306px;
            y: 118px;
            width: 96px;
            height: 34px;
            background: #343432;
            border-radius: 4px;
            Text { text: "Close"; color: #e8ecee; font-size: 13px; horizontal-alignment: center; vertical-alignment: center; }
            TouchArea { clicked => { root.close_modal(); } }
        }
    }
}

export component AppWindow inherits Window {
    in property <string> proof_status;
    in property <string> gpu_status;
    in property <string> webview_status;
    in property <string> active_section;
    in property <string> section_title;
    in property <string> canvas_title;
    in property <string> canvas_hint;
    in property <string> hardware_status;
    in property <string> provider_status;
    in property <string> job_status;
    in property <string> proof_badge_status;
    in property <string> brain_status;
    in property <string> monster_status;
    in property <string> panel_status;
    in property <string> native_webexplorer_status;
    in property <string> banger_status;
    in property <string> trading_status;
    in property <string> real_estate_status;
    in property <bool> modal_visible;
    in property <string> modal_title;
    in property <string> modal_body;
    in-out property <string> chat_text;
    callback refresh_probes();
    callback navigate(string);
    callback activate_imported_control(string);
    callback window_close();
    callback window_minimize();
    callback window_toggle_maximize();
    callback navigate_next();
    callback send_chat();
    callback close_modal();

    title: "InGen Native Front - Forge";
    preferred-width: ${px(outputViewport.width)};
    preferred-height: ${px(outputViewport.height)};
    min-width: 1180px;
    min-height: 760px;
    background: #0e0e0f;

    FocusScope {
        KeyBinding { keys: @keys(Control+Tab); activated => { root.navigate_next(); } }
        KeyBinding { keys: @keys(Escape); activated => { root.close_modal(); } }
    }

${layerInstances}

    NativeModal {
        x: 0px;
        y: 0px;
        width: root.width;
        height: root.height;
        modal_visible: root.modal_visible;
        modal_title: root.modal_title;
        modal_body: root.modal_body;
        close_modal => { root.close_modal(); }
    }
}
`;
}

async function main() {
  const { server, origin } = await serveUi();
  let browser;
  try {
    rmSync(generatedAssets, { recursive: true, force: true });
    mkdirSync(generatedAssets, { recursive: true });
    browser = await chromium.launch({ headless: true });
    async function capture(name, clickSelector = null) {
      const page = await browser.newPage({ viewport, deviceScaleFactor: 1 });
      await page.addInitScript(() => {
        window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
      });
      await page.goto(`${origin}/index.html`, { waitUntil: "networkidle", timeout: 45000 });
      await page.waitForSelector(".window-titlebar", { timeout: 45000 });
      await page.addStyleTag({
        content: `
          *, *::before, *::after {
            animation-duration: 0s !important;
            animation-delay: 0s !important;
            transition-duration: 0s !important;
            transition-delay: 0s !important;
            caret-color: transparent !important;
          }
          #alphaProofPanel {
            display: none !important;
            visibility: hidden !important;
          }
          .content.proof-open {
            grid-template-columns: var(--left-panel-width, 279px) minmax(0, 1fr) !important;
          }
        `,
      });
      await page.evaluate(() => {
        document.getElementById("alphaProofPanel")?.setAttribute("hidden", "");
        document.getElementById("alphaProofPanel")?.setAttribute("aria-hidden", "true");
        document.querySelector(".content")?.classList.remove("proof-open");
        document.body?.classList.remove("webexplorer-surface", "real-estate-mode", "banger-fullscreen-mode");
      });
      if (clickSelector) {
        await page.locator(clickSelector).click({ timeout: 10000 }).catch(() => {});
        await page.waitForTimeout(700);
        await page.evaluate(() => {
          document.getElementById("alphaProofPanel")?.setAttribute("hidden", "");
          document.getElementById("alphaProofPanel")?.setAttribute("aria-hidden", "true");
          document.querySelector(".content")?.classList.remove("proof-open");
        });
      }
      await page.waitForTimeout(300);
      const scene = await extractScene(page);
      const screenshot = await page.screenshot({
        path: join(generatedAssets, `layer_${name}.png`),
        fullPage: false,
        animations: "disabled",
      });
      scene.screenshot = {
        bytes: screenshot.length,
        sha256: sha256(screenshot),
      };
      await page.close();
      return { ...scene, name };
    }
    const scenes = [
      await capture("forge"),
      await capture("webexplorer", "#webexplorer"),
      await capture("banger", "#bangerBoomBtn"),
      await capture("trading", "#tradingWorkspaceBtn"),
      await capture("real-estate", "#realEstateModeBtn"),
    ];
    writeFileSync(outSlint, emitSlint(scenes));
    const sourceFiles = [
      "index.html",
      "styles.css",
      "dist/forge-app.js",
      "dist/forge-shell-runtime.js",
      "dist/forge-banger.js",
      "dist/forge-trading.js",
      "dist/forge-real-estate.js",
    ];
    const report = {
      schema: "ingen.native_front.tauri_to_slint_import.v1",
      cssViewport: viewport,
      outputViewport,
      cssScale,
      renderMode,
      sourceRoot: "examples/forge_tauri_ui/ui",
      output: "examples/ingen_native_front/ui/app.slint",
      layers: scenes.map((scene) => ({
        name: scene.name,
        entries: scene.entries.length,
        buttons: scene.buttons.length,
        svgAssets: scene.entries.filter((entry) => entry.kind === "svg").length,
        screenshot: scene.screenshot,
      })),
      entries: scenes.reduce((sum, scene) => sum + scene.entries.length, 0),
      buttons: scenes.reduce((sum, scene) => sum + scene.buttons.length, 0),
      svgAssets: scenes.reduce((sum, scene) => sum + scene.entries.filter((entry) => entry.kind === "svg").length, 0),
      sourceHashes: Object.fromEntries(sourceFiles.map((file) => {
        const path = join(uiRoot, file);
        return [file, existsSync(path) ? sha256(readFileSync(path)) : null];
      })),
    };
    report.proofHash = sha256(stableJson(report));
    writeFileSync(outReport, `${JSON.stringify(report, null, 2)}\n`);
    console.log(JSON.stringify(report, null, 2));
  } finally {
    if (browser) await browser.close();
    await new Promise((resolveClose) => server.close(resolveClose));
  }
}

await main();
