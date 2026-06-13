import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  root: ".",
  build: {
    outDir: "dist/renderer",
    emptyOutDir: true,
    sourcemap: true,
    target: "es2024",
    rolldownOptions: {
      input: {
        main: resolve(currentDir, "index.html"),
        eventTextLab: resolve(currentDir, "event-text-lab.html")
      }
    }
  },
  server: {
    host: "127.0.0.1",
    strictPort: false
  }
});
