import { defineConfig } from '@playwright/test';

/**
 * Playwright config for the extension E2E smoke test (build-env-deploy §6.2).
 *
 * The webServer builds the unpacked extension into `dist/` and serves a tiny
 * static fixture page the extension's content script can match against
 * (`<all_urls>`). The spec launches Chromium with `--load-extension` and proves
 * the stateless Service Worker survives a termination/restart cycle.
 */
export default defineConfig({
  testDir: './tests',
  testMatch: '**/*.spec.ts',
  globalSetup: './tests/globalSetup.ts',
  timeout: 90_000,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [['list']],
  use: {
    headless: true,
    channel: 'chromium',
  },
  webServer: {
    command: 'node tests/smoke/server.mjs',
    url: 'http://localhost:4174/',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
