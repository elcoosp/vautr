import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Worker } from '@playwright/test';
import { expect, test } from '@playwright/test';
import {
  autofillResponseAttribute,
  expectWorkerTerminated,
  fillActiveFieldAndExpectSecret,
  getExtensionWorker,
  launchExtensionContext,
  readSessionFromWorker,
  seedStorage,
  terminateExtensionServiceWorkers,
  triggerAutofill,
  waitForExtensionWorker,
} from './helpers';

/**
 * Service Worker lifecycle stress (build-env-deploy §6.2).
 *
 *  - Several force-termination / restart cycles in a row, each served by a fresh
 *    stateless SW that re-reads state from `chrome.storage`.
 *  - `chrome.storage.session` (the raw SVK cache) survives SW restarts but is
 *    wiped on browser close — verified by reopening the same persistent profile.
 */

test('stateless SW survives repeated termination/restart cycles', async () => {
  const profileDir = mkdtempSync(join(tmpdir(), 'vautr-lifecycle-'));
  const context: BrowserContext = await launchExtensionContext({ profileDir });
  try {
    const page = await context.newPage();
    await page.goto('http://localhost:4174/');
    const worker = await waitForExtensionWorker(context);
    await seedStorage(worker);

    // Fill, terminate the SW, then prove a fresh SW re-fills from storage.
    for (let cycle = 0; cycle < 3; cycle += 1) {
      await page.locator('#password').fill('');
      await fillActiveFieldAndExpectSecret(page, '#password');

      const running = getExtensionWorker(context);
      expect(running, 'expected a running SW before termination').toBeTruthy();
      await terminateExtensionServiceWorkers(context);
      await expectWorkerTerminated(running as Worker);
    }

    // A final restart still decrypts and fills from `chrome.storage` alone.
    await page.locator('#password').fill('');
    await fillActiveFieldAndExpectSecret(page, '#password');
  } finally {
    await context.close();
    rmSync(profileDir, { recursive: true, force: true });
  }
});

test('SVK survives SW restarts but is wiped on browser close', async () => {
  const profileDir = mkdtempSync(join(tmpdir(), 'vautr-reopen-'));
  let context: BrowserContext = await launchExtensionContext({ profileDir });
  try {
    let page = await context.newPage();
    await page.goto('http://localhost:4174/');
    let worker = await waitForExtensionWorker(context);
    await seedStorage(worker);

    // SVK cached in session storage; autofill works.
    await fillActiveFieldAndExpectSecret(page, '#password');

    // Terminate the SW: the SVK (in `chrome.storage.session`) survives and a
    // fresh SW re-reads it to fill again.
    await terminateExtensionServiceWorkers(context);
    await page.locator('#password').fill('');
    await fillActiveFieldAndExpectSecret(page, '#password');

    // Simulate browser close.
    await context.close();

    // Reopen the same persistent profile: `chrome.storage.session` is in-memory
    // and must be wiped, so the vault is locked again.
    context = await launchExtensionContext({ profileDir });
    page = await context.newPage();
    await page.goto('http://localhost:4174/');
    worker = await waitForExtensionWorker(context);

    const session = await readSessionFromWorker(worker, ['vautrSvk']);
    expect(session).not.toHaveProperty('vautrSvk');

    await page.locator('#password').focus();
    await triggerAutofill(page);
    await expect
      .poll(() => autofillResponseAttribute(page), { timeout: 15_000 })
      .toContain('vault is locked');
    expect(await page.locator('#password').inputValue()).toBe('');
  } finally {
    await context.close();
    rmSync(profileDir, { recursive: true, force: true });
  }
});
