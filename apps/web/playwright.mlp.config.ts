import { defineConfig } from '@playwright/test';

/**
 * MLP Web vault e2e (Wave B1).
 *
 * Playwright manages BOTH the vautr-server (on :8080, fresh temp DB) and the
 * built web app (served on :5173 via `vite preview`), so neither is subject to
 * the background-task lifetime limit. No cargo build runs here (JS-only gate):
 * it reuses the already-built `../../target/debug/vautr-server` binary.
 */
export default defineConfig({
  testDir: './e2e-mlp',
  testMatch: '**/*.spec.ts',
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
      // Fresh DB per run; clear any competing server on :8080 first.
      command:
        "bash -c 'lsof -ti tcp:8080 | xargs -r kill; rm -f /tmp/vautr-web-e2e.db; VAUTR_DB_URL=sqlite:/tmp/vautr-web-e2e.db ../../target/debug/vautr-server'",
      url: 'http://localhost:8080/projects',
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: 'node node_modules/vite/bin/vite.js preview --port 5173 --strictPort',
      url: 'http://localhost:5173/',
      reuseExistingServer: false,
      timeout: 30_000,
    },
  ],
});
