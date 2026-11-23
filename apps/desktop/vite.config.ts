import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vitest/config";

// In docker compose the devserver is reachable as http://back:7878; locally on :7878.
const apiTarget = process.env.VITE_PROXY_TARGET ?? "http://localhost:7878";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    proxy: { "/api": { target: apiTarget, changeOrigin: true } },
    watch: { ignored: ["**/src-tauri/**"], usePolling: process.env.VITE_POLLING === "true" },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "safari16",
    outDir: "dist",
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    // Node 25+ defines its own `localStorage` global, which shadows jsdom's; turn it off
    // where it exists (older Nodes don't know the flag).
    poolOptions: {
      forks: { execArgv: "localStorage" in globalThis ? ["--no-experimental-webstorage"] : [] },
    },
  },
});
