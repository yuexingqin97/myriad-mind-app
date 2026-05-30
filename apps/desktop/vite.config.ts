import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Tauri expects a fixed port; fail if it's not available
  server: {
    port: 1420,
    strictPort: true,
    // Allow Tauri to reach the dev server
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  // Tauri production build
  build: {
    target: "esnext",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
