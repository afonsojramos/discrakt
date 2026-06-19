import path from "path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The built assets are embedded in the Discrakt binary and served from the
// setup server's root, so use relative asset paths.
export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    // For local UI development, point the API + logo routes at a running
    // Discrakt setup server: VITE_PROXY_TARGET=http://127.0.0.1:<port> pnpm dev
    proxy: process.env.VITE_PROXY_TARGET
      ? Object.fromEntries(
          ["/submit", "/submit-plex", "/plex-login", "/status", "/logo.svg", "/favicon.png"].map(
            (p) => [p, { target: process.env.VITE_PROXY_TARGET, changeOrigin: true }],
          ),
        )
      : undefined,
  },
});
