import { defineConfig } from "@playwright/test";

// End-to-end tests: the real API (with PostgreSQL) and the Vite dev server, in Chromium.
//   FORGE_E2E_DATABASE_URL=postgres://… pnpm --filter @forge/web e2e
// The API is started with `cargo run -p forge-api`, so the first run compiles it.
const db = process.env.FORGE_E2E_DATABASE_URL;
if (!db) throw new Error("Set FORGE_E2E_DATABASE_URL to a PostgreSQL database for the end-to-end tests.");
const apiPort = 18090;
const webPort = 5199;

export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { baseURL: `http://127.0.0.1:${webPort}`, trace: "retain-on-failure", browserName: "chromium" },
  webServer: [
    {
      command: "cargo run -q -p forge-api",
      cwd: "../..",
      url: `http://127.0.0.1:${apiPort}/readyz`,
      timeout: 900_000,
      reuseExistingServer: false,
      env: {
        FORGE_API_ADDR: `127.0.0.1:${apiPort}`,
        FORGE_DATABASE_URL: db,
        // A throwaway key for this test run only.
        FORGE_TOKEN_KEY: Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString("base64"),
        // Plain HTTP on 127.0.0.1 in tests; production keeps Secure cookies.
        FORGE_COOKIE_SECURE: "false",
        // The engine worker the API spawns, and a scratch workspace root for this run.
        FORGE_WORKER_BIN: process.env.FORGE_WORKER_BIN ?? "forge-worker",
        FORGE_WORKSPACE_ROOT: process.env.FORGE_WORKSPACE_ROOT ?? "/tmp/forge-e2e-ws",
        RUST_LOG: "warn",
      },
    },
    {
      command: `pnpm exec vite --port ${webPort} --strictPort --host 127.0.0.1`,
      url: `http://127.0.0.1:${webPort}`,
      timeout: 120_000,
      reuseExistingServer: false,
      env: { FORGE_API_URL: `http://127.0.0.1:${apiPort}` },
    },
  ],
});
