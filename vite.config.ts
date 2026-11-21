import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Settings recommended for Tauri: a fixed port the Rust side points at,
// and no watching of the Rust sources.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/core/**", "**/target/**"] },
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
