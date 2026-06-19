import path from "path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, type PluginOption } from "vite-plus";

// `react()`/`tailwindcss()` are typed against the standalone `vite` package,
// while Vite+ bundles its own copy; the plugin instances are compatible at
// runtime, so cast to Vite+'s plugin type to satisfy the type checker.
const plugins = [react(), tailwindcss()] as PluginOption[];

// The built assets are embedded in the Discrakt binary and served from the
// setup server's root, so use relative asset paths.
export default defineConfig({
  base: "./",
  plugins,
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    // For local UI development, point the API routes at a running Discrakt
    // setup server: VITE_PROXY_TARGET=http://127.0.0.1:<port> pnpm dev
    proxy: process.env.VITE_PROXY_TARGET
      ? Object.fromEntries(
          ["/submit", "/submit-plex", "/plex-login", "/status"].map((p) => [
            p,
            { target: process.env.VITE_PROXY_TARGET, changeOrigin: true },
          ]),
        )
      : undefined,
  },
  // oxlint config lives here (Vite+ reads it from vite.config.ts, not .oxlintrc.json).
  // eslint-plugin-better-tailwindcss runs via oxlint's JS plugin support; line
  // wrapping is left to oxfmt, so only the class-validation rules are enabled.
  lint: {
    settings: {
      "better-tailwindcss": {
        entryPoint: "src/index.css",
      },
    },
    jsPlugins: [{ name: "better-tailwindcss", specifier: "eslint-plugin-better-tailwindcss" }],
    rules: {
      "better-tailwindcss/enforce-consistent-class-order": "warn",
      "better-tailwindcss/enforce-canonical-classes": "warn",
      "better-tailwindcss/no-duplicate-classes": "warn",
      "better-tailwindcss/no-deprecated-classes": "warn",
      "better-tailwindcss/no-unnecessary-whitespace": "warn",
      "better-tailwindcss/no-unknown-classes": "warn",
      "better-tailwindcss/no-conflicting-classes": "warn",
    },
  },
});
