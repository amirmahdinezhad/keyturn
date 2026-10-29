import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

// Separate production frontend: never included in the normal app build.
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react(), tailwindcss()],
  resolve: { alias: Object.fromEntries([
    "@tauri-apps/api/core", "@tauri-apps/api/event", "@tauri-apps/api/app",
    "@tauri-apps/plugin-updater", "@tauri-apps/plugin-process", "@tauri-apps/plugin-opener",
  ].map(name => [name, fileURLToPath(new URL("./bridge.ts", import.meta.url))])) },
  build: { outDir: "/tmp/keyturn-motion/dist", emptyOutDir: true },
});
