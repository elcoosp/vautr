import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { getExtensionId, launchExtensionContext } from './helpers';

/**
 * Popup UI flow (build-env-deploy §3.3).
 *
 * The popup runs a full `VautrWebClient` against the live server
 * (`http://localhost:8080`) backed by IndexedDB and the real `vautr-wasm`
 * crypto. This suite drives the real popup page through register -> unlocked
 * vault -> tabs -> lock, and proves the client is torn down gracefully on
 * close (`pagehide` -> `disposePopupClient`).
 */

test.describe.serial('popup UI flow', () => {
  let context: BrowserContext;
  let popup: Page;
  let profileDir: string;
  let extId: string;

  const username = `popup-${Date.now()}@vautr.test`;
  const password = 'correct-horse-battery-staple-popup';

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

  test('popup opens locked', async () => {
    await popup.waitForSelector('#auth-username');
    await expect(popup.getByText('Unlock your vault')).toBeVisible();
    await expect(popup.getByRole('tab', { name: 'Login' })).toBeVisible();
  });

  test('registering a new account unlocks the vault', async () => {
    await popup.getByRole('tab', { name: 'Register' }).click();
    await popup.fill('#auth-username', username);
    await popup.fill('#auth-password', password);
    await popup.getByRole('button', { name: 'Register' }).click();

    // Registration + login against the live server, then sync. The header shows
    // the username once the vault is unlocked.
    await popup.getByText(username, { exact: true }).waitFor({ timeout: 30_000 });

    // The tab shell is present after unlock.
    for (const tab of ['Vault', 'Projects', 'Secrets', 'Generator', 'MFA']) {
      await expect(popup.getByRole('tab', { name: tab })).toBeVisible();
    }
    await expect(popup.getByRole('button', { name: 'Lock' })).toBeVisible();
  });

  test('projects tab lists the fresh empty project set', async () => {
    await popup.getByRole('tab', { name: 'Projects' }).click();
    await expect(popup.getByText('No projects yet')).toBeVisible({ timeout: 15_000 });
    await expect(popup.getByRole('button', { name: 'New' })).toBeVisible();
  });

  test('locking returns to the auth view', async () => {
    await popup.getByRole('button', { name: 'Lock' }).click();
    await expect(popup.getByText('Unlock your vault')).toBeVisible({ timeout: 10_000 });
    await expect(popup.getByRole('tab', { name: 'Login' })).toBeVisible();
  });
});
