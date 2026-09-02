import { chromium } from "playwright";
import { readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const shellRoot = new URL("..", import.meta.url).pathname.replace(/^\/(?=[A-Z]:)/, "");
const svgPath = join(shellRoot, "public", "shell-assets", "graphen-cube-icon.svg");
const icoPath = join(shellRoot, "public", "shell-assets", "graphen-cube-icon.ico");
const pngPath = join(tmpdir(), `graphen-cube-${process.pid}.png`);
const svg = await readFile(svgPath, "utf8");
const browser = await chromium.launch({
  headless: true,
  executablePath: "C:\\Users\\Quentin\\AppData\\Local\\BraveSoftware\\Brave-Browser\\Application\\brave.exe"
});

try {
  const page = await browser.newPage({ viewport: { width: 256, height: 256 }, deviceScaleFactor: 1 });
  await page.setContent(`<body style="margin:0;background:transparent"><img src="data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}" width="256" height="256" /></body>`);
  await page.screenshot({ path: pngPath, omitBackground: true });
} finally {
  await browser.close();
}

const png = await readFile(pngPath);
await rm(pngPath, { force: true });
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(1, 4);
const directory = Buffer.alloc(16);
directory.writeUInt8(0, 0);
directory.writeUInt8(0, 1);
directory.writeUInt8(0, 2);
directory.writeUInt8(0, 3);
directory.writeUInt16LE(1, 4);
directory.writeUInt16LE(32, 6);
directory.writeUInt32LE(png.length, 8);
directory.writeUInt32LE(22, 12);
await writeFile(icoPath, Buffer.concat([header, directory, png]));
console.log(`Generated ${icoPath}`);
