import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { getExtensionId, launchExtensionContext } from './helpers';

/**
 * Popup UI flow (build-env-deploy §3.3).
 *
 * The popup runs a full `VautrClient` in a popup-scoped Web Worker. This suite
 * drives the real popup page (`chrome-extension://<id>/src/popup/popup.html`)
 * through unlock -> vault list -> reveal, then proves the client worker is
 * gracefully shut down when the popup closes (`pagehide` -> `disposePopupClient`
 * -> `worker.terminate`).
 */

const DEMO_PASSWORD = 'password123';

test.describe
  .serial('popup UI flow', () => {
    let context: BrowserContext;
    let popup: Page;
    let profileDir: string;
    let extId: string;

    test.beforeAll(async () => {
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-popup-'));
      context = await launchExtensionContext({ profileDir });
      extId = await getExtensionId(context);
      popup = await context.newPage();
      await popup.goto(`chrome-extension://${extId}/src/popup/popup.html`);
    });

    test.afterAll(async () => {
      await context?.close();
      if (profileDir) {
        rmSync(profileDir, { recursive: true, force: true });
      }
    });

    test('popup opens locked and has no client worker yet', async () => {
      await popup.waitForSelector('#vault-password');
      await expect(popup.locator('button:has-text("Unlock")')).toBeVisible();
      // The VautrClient worker is lazy: it is only spawned on unlock.
      expect(popup.workers()).toHaveLength(0);
    });

    test('unlock renders the demo vault list', async () => {
      await popup.fill('#vault-password', DEMO_PASSWORD);
      await popup.click('button:has-text("Unlock")');
      await popup.waitForSelector('text=Acme Bank', { timeout: 15_000 });
      await expect(popup.getByText('Github', { exact: true })).toBeVisible();
      await expect(popup.getByText('Work Email', { exact: true })).toBeVisible();
      // Unlock spawned the popup-scoped VautrClient worker.
      expect(popup.workers().length).toBeGreaterThan(0);
    });

    test('reveal (Copy) decrypts a secret and reports success', async () => {
      await popup.click('button:has-text("Copy")');
      await popup.waitForSelector('text=Copied', { timeout: 10_000 });
      await expect(popup.locator('text=Copied')).toContainText('Acme Bank');
      // Reveal succeeded, so the client worker is still alive.
      expect(popup.workers().length).toBeGreaterThan(0);
    });

    test('popup gracefully shuts down its client worker on close', async () => {
      const workerCountBefore = popup.workers().length;
      expect(workerCountBefore).toBeGreaterThan(0);
      // The popup listens for `pagehide`/`beforeunload` to dispose the client.
      await popup.evaluate(() => window.dispatchEvent(new Event('pagehide')));
      await expect
        .poll(() => popup.workers().length, { timeout: 10_000 })
        .toBeLessThan(workerCountBefore);
      expect(popup.workers()).toHaveLength(0);
    });
  });
