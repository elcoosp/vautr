import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Page, Worker } from '@playwright/test';
import { expect, test } from '@playwright/test';
import {
  autofillResponseAttribute,
  DEMO_SECRET,
  fillActiveFieldAndExpectSecret,
  launchExtensionContext,
  seedStorage,
  triggerAutofill,
  waitForExtensionWorker,
} from './helpers';

/**
 * Autofill breadth (build-env-deploy §3.3).
 *
 * Beyond the single-field smoke test, this exercises the autofill path against a
 * richer fixture form: filling several fields in one form, targeting exactly the
 * focused field when multiple fields could match, and gracefully no-op'ing when
 * no cached entry exists for the requested item.
 */

test.describe
  .serial('autofill breadth', () => {
    let context: BrowserContext;
    let page: Page;
    let worker: Worker;
    let profileDir: string;

    test.beforeAll(async () => {
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-breadth-'));
      context = await launchExtensionContext({ profileDir });
      page = await context.newPage();
      await page.goto('http://localhost:4174/');
      worker = await waitForExtensionWorker(context);
      await seedStorage(worker);
    });

    test.afterAll(async () => {
      await context?.close();
      if (profileDir) {
        rmSync(profileDir, { recursive: true, force: true });
      }
    });

    async function clearAllFields(): Promise<void> {
      for (const sel of ['#username', '#password', '#pass-confirm', '#search', '#notes']) {
        await page.locator(sel).fill('');
      }
    }

    test('fills several fields in one form across separate requests', async () => {
      await clearAllFields();
      // Each request fills the focused field; earlier fills are preserved.
      await fillActiveFieldAndExpectSecret(page, '#username');
      await fillActiveFieldAndExpectSecret(page, '#password');
      await fillActiveFieldAndExpectSecret(page, '#notes');
      expect(await page.locator('#username').inputValue()).toBe(DEMO_SECRET);
      expect(await page.locator('#password').inputValue()).toBe(DEMO_SECRET);
      expect(await page.locator('#notes').inputValue()).toBe(DEMO_SECRET);
    });

    test('targets exactly the focused field when several fields could match', async () => {
      await clearAllFields();
      // Among several editable fields (password + pass-confirm + search + notes),
      // autofill must fill only the focused one and leave the rest untouched.
      await fillActiveFieldAndExpectSecret(page, '#pass-confirm');
      expect(await page.locator('#pass-confirm').inputValue()).toBe(DEMO_SECRET);
      expect(await page.locator('#password').inputValue()).toBe('');
      expect(await page.locator('#username').inputValue()).toBe('');
      expect(await page.locator('#search').inputValue()).toBe('');
      expect(await page.locator('#notes').inputValue()).toBe('');
    });

    test('gracefully no-ops when no cached entry matches the requested item', async () => {
      await clearAllFields();
      await page.locator('#password').focus();
      await triggerAutofill(page, '00000000-0000-4000-8000-000000000000');
      // The SW reports an error and the focused field is left empty.
      await expect
        .poll(() => autofillResponseAttribute(page), { timeout: 15_000 })
        .toContain('"ok":false');
      const response = await autofillResponseAttribute(page);
      expect(response).toContain('no cached ciphertext');
      expect(await page.locator('#password').inputValue()).toBe('');
    });
  });
