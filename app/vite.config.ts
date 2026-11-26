import { defineConfig } from "vite";

// Tauri loads the page from this fixed port in development, so the port must
// not change silently when it is busy.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "safari15", outDir: "dist" },
});
