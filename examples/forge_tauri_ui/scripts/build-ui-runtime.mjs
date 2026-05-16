import { existsSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const entry = (path) => `./${path}`;

function resolveLocalImport(resolveDir, specifier) {
  const base = resolve(resolveDir || root, specifier);
  const ext = extname(base);
  const candidates = [base];
  if (ext === ".js") candidates.push(`${base.slice(0, -3)}.ts`);
  if (!ext) candidates.push(`${base}.ts`, `${base}.js`, resolve(base, "index.ts"), resolve(base, "index.js"));
  return candidates.find((candidate) => existsSync(candidate)) || null;
}

const localFileResolver = {
  name: "forge-local-file-resolver",
  setup(build) {
    build.onResolve({ filter: /^@tauri-apps\/api(?:\/.*)?$/ }, (args) => {
      const submodule = args.path.replace(/^@tauri-apps\/api\/?/, "") || "index";
      const resolved = resolveLocalImport(root, `node_modules/@tauri-apps/api/${submodule}.js`);
      return resolved ? { path: resolved } : null;
    });
    build.onResolve({ filter: /^\.{1,2}\// }, (args) => {
      const resolved = resolveLocalImport(args.resolveDir, args.path);
      return resolved ? { path: resolved } : null;
    });
  },
};

const shared = {
  absWorkingDir: root,
  bundle: true,
  format: "iife",
  target: "es2022",
  logLevel: "info",
  plugins: [localFileResolver],
};

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/legacy-section-registry.ts")],
  outfile: resolve(root, "ui/dist/forge-section-registry.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/tauri-bridge.ts")],
  outfile: resolve(root, "ui/dist/forge-tauri-bridge.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/main.ts")],
  outfile: resolve(root, "ui/dist/forge-shell-runtime.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/boot.ts")],
  outfile: resolve(root, "ui/dist/forge-boot.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/window-controls.ts")],
  outfile: resolve(root, "ui/dist/forge-window-controls.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/hardware.ts")],
  outfile: resolve(root, "ui/dist/forge-hardware.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/sidebar.ts")],
  outfile: resolve(root, "ui/dist/forge-sidebar.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/guardian.ts")],
  outfile: resolve(root, "ui/dist/forge-shell-guardian.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/search-palette.ts")],
  outfile: resolve(root, "ui/dist/forge-search-palette.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/sections/webexplorer/config.ts")],
  outfile: resolve(root, "ui/dist/forge-webexplorer-config.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/sections/real-estate/index.ts")],
  outfile: resolve(root, "ui/dist/forge-real-estate.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/sections/trading/surface.ts")],
  outfile: resolve(root, "ui/dist/forge-trading.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/sections/banger/surface.ts")],
  outfile: resolve(root, "ui/dist/forge-banger.js"),
});

await esbuild.build({
  ...shared,
  entryPoints: [entry("ui/src/shell/surface.ts")],
  outfile: resolve(root, "ui/dist/forge-app.js"),
});
