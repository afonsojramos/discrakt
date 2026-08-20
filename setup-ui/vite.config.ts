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
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    proxy: process.env.VITE_PROXY_TARGET
      ? Object.fromEntries(
          ["/submit", "/submit-plex", "/plex-login", "/status"].map((p) => [
            p,
            { target: process.env.VITE_PROXY_TARGET, changeOrigin: true },
          ]),
        )
      : undefined,
  },
});
