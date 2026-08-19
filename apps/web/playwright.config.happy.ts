import { defineConfig } from '@playwright/test';

/**
 * Self-contained config for the web happy-path flow e2e (flow.spec.ts).
 *
 * It boots ONLY the Vite dev server for the Relying-Party origin
 * (http://localhost:5173). The Vautr API server is expected to already be
 * running on http://localhost:8080 (e.g. `./target/release/vautr-server` with
 * VAUTR_DB_URL set) — the spec drives the real OPAQUE register/login UI, so no
 * DB seeding or IndexedDB injection is required.
 */

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/flow.spec.ts',
  timeout: 120_000,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [['list']],
  use: {
    headless: true,
    channel: 'chromium',
  },
  webServer: [
    {
      command: 'node e2e/serve.mjs',
      url: 'http://localhost:5173/',
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
    },
  ],
});
