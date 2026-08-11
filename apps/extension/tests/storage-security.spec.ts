import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Page, Worker } from '@playwright/test';
import { expect, test } from '@playwright/test';
import {
  autofillResponseAttribute,
  DEMO_SECRET,
  launchExtensionContext,
  scanStoresForPlaintext,
  seedStorage,
  triggerAutofill,
  waitForExtensionWorker,
} from './helpers';

/**
 * Storage & Zero-Knowledge boundary (build-env-deploy §3.3).
 *
 * The autofill secret decrypts in the stateless SW and is written into the
 * focused input by the content script (isolated world). It must never leak into
 * the page's main world (DOM text, other inputs, or the echoed response
 * attribute), and the plaintext must never be persisted to `chrome.storage`.
 */
test.describe
  .serial('storage & zero-knowledge boundary', () => {
    let context: BrowserContext;
    let page: Page;
    let worker: Worker;
    let profileDir: string;

    test.beforeAll(async () => {
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-zk-'));
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

    test('autofills into the focused input without exposing the secret to the page', async () => {
      await page.locator('#password').focus();
      await triggerAutofill(page);
      await page.waitForFunction(
        (expected) => {
          const el = document.getElementById('password') as HTMLInputElement | null;
          return !!el && el.value === expected;
        },
        DEMO_SECRET,
        { timeout: 20_000 },
      );

      // The secret may only live in the focused input's value, nowhere else in the
      // main world (other inputs, body text, or the echoed relay response).
      const leak = await page.evaluate((secret) => {
        const input = document.getElementById('password') as HTMLInputElement;
        const other = Array.from(
          document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('input, textarea'),
        )
          .filter((el) => el !== input && el.value.includes(secret))
          .map((el) => el.id);
        const attr = document.documentElement.getAttribute('data-vautr-autofill') ?? '';
        return {
          focusedValue: input.value,
          leakedInOtherInputs: other,
          leakedInBodyText: document.body.innerText.includes(secret),
          leakedInResponseAttr: attr.includes(secret),
        };
      }, DEMO_SECRET);

      expect(leak.focusedValue).toBe(DEMO_SECRET);
      expect(leak.leakedInOtherInputs).toEqual([]);
      expect(leak.leakedInBodyText).toBe(false);
      expect(leak.leakedInResponseAttr).toBe(false);

      // The relay response echoes success only, never the secret.
      const response = await autofillResponseAttribute(page);
      expect(response).toContain('"ok":true');
      expect(response).toContain('"filled":true');
    });

    test('never persists the plaintext secret to chrome.storage', async () => {
      const containsPlaintext = await scanStoresForPlaintext(worker, DEMO_SECRET);
      expect(containsPlaintext).toBe(false);
    });
  });
