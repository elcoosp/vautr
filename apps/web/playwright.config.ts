import { defineConfig } from '@playwright/test';

/**
 * WebAuthn (FIDO2) second-factor e2e (VTR-052), driven with a CDP virtual
 * authenticator.
 *
 * `globalSetup` spawns the Vautr server (with the `webauthn` feature) against a
 * temp DB and seeds a user + session; `globalTeardown` stops it. The single
 * `webServer` here just hosts the Relying-Party origin the browser authenticates
 * against (http://localhost:5173).
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
  webServer: [
    {
      command: 'node e2e/serve.mjs',
      url: 'http://localhost:5173/',
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
    },
  ],
});
