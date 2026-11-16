import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// In development the API runs on its own port (`cargo run -p forge-api`); the dev server proxies
// /api, /healthz and /readyz to it so the app talks to one origin, as it does in production.
const api = process.env.FORGE_API_URL ?? "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: { "/api": api, "/healthz": api, "/readyz": api },
  },
  build: { sourcemap: true },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    css: false,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
