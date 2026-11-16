import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Served by the Rust API under /admin, so assets are built with that base path.
export default defineConfig({
  plugins: [react()],
  base: "/admin/",
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: process.env.VITE_API_PROXY ?? "http://127.0.0.1:8090",
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
    sourcemap: true,
  },
});
