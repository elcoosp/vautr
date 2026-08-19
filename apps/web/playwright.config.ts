import { defineConfig } from '@playwright/test';

/**
 * WebAuthn (FIDO2) second-factor e2e (VTR-052) + conflict-resolution modal
 * (VTR-056), driven with a CDP virtual authenticator.
 *
 * Neither the vautr backend nor the vite dev server (the app origin +
 * /api -> :8080 proxy) are managed by Playwright here: both are started (and
 * torn down) by the run script (see /tmp/run_web_e2e.sh). Playwright's own
 * webServer lifecycle proved unreliable in this environment — its management
 * signalled the process group and reaped the backend 30s into the run.
 *
 * `globalSetup` seeds a user + session into the already-running backend's DB.
 */
export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  globalSetup: './e2e/globalSetup.ts',
  globalTeardown: './e2e/globalTeardown.ts',
  timeout: 120_000,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [['list']],
  use: {
    headless: true,
    channel: 'chromium',
  },
});
