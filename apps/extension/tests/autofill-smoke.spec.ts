import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { BrowserContext, Page, Worker } from '@playwright/test';
import { chromium, expect, test } from '@playwright/test';
import { buildSecretEnvelope } from './helpers';

/**
 * Stateless autofill Service Worker smoke test (build-env-deploy §6.2).
 *
 * Proves the MV3 SW termination/restart cycle during autofill:
 *  1. An autofill request wakes the SW, which decrypts and fills the field.
 *  2. The SW is force-terminated (CDP `ServiceWorker.stopAllWorkers`).
 *  3. A second autofill restarts a FRESH, stateless SW that decrypts and fills
 *     again from `chrome.storage` alone (no in-memory state carried over).
 *
 * Autofill is driven through the real path: the page main world dispatches a DOM
 * `CustomEvent`, the content script relays it to the SW, the SW decrypts via the
 * nodejs-target WASM and asks the content script to fill the focused field.
 */

const DEMO_SECRET = 'vautr-demo-password-0x3f9a';
const UUID = 'b2e7b6d0-8c1a-4b2e-9f0a-6c2d3e4f5a6b';
const SVK = Array.from(new Uint8Array(32).fill(7));

const extensionRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const distPath = join(extensionRoot, 'dist');

test.describe
  .serial('stateless autofill service worker', () => {
    let context: BrowserContext;
    let page: Page;
    let profileDir: string;

    test.beforeAll(async () => {
      // The extension is built by `globalSetup.ts` into `dist/`.
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-pw-'));
      context = await chromium.launchPersistentContext(profileDir, {
        headless: true,
        channel: 'chromium',
        args: [`--disable-extensions-except=${distPath}`, `--load-extension=${distPath}`],
      });
      page = await context.newPage();
      await page.goto('http://localhost:4174/');
    });

    test.afterAll(async () => {
      await context?.close();
      if (profileDir) {
        rmSync(profileDir, { recursive: true, force: true });
      }
    });

    function getExtensionWorker(exclude?: Worker): Worker | undefined {
      return context
        .serviceWorkers()
        .find((w) => w.url().startsWith('chrome-extension://') && w !== exclude);
    }

    async function waitForExtensionWorker(exclude?: Worker, timeoutMs = 20_000) {
      const started = Date.now();
      while (Date.now() - started < timeoutMs) {
        const running = getExtensionWorker(exclude);
        if (running) {
          return running;
        }
        // Wake the SW by firing the relay; safe when storage is empty (returns an error).
        await triggerAutofill(page);
        await page.waitForTimeout(250);
      }
      throw new Error('extension service worker never started');
    }

    async function seedStorage(worker: Worker) {
      const envelope = buildSecretEnvelope(DEMO_SECRET, UUID, SVK);
      await worker.evaluate(
        ({ svk, uuid, payload, encKeyGen }) => {
          const chromeApi = globalThis as unknown as {
            chrome: {
              storage: {
                session: { set: (v: Record<string, unknown>) => Promise<void> };
                local: { set: (v: Record<string, unknown>) => Promise<void> };
              };
            };
          };
          return Promise.all([
            chromeApi.chrome.storage.session.set({ vautrSvk: svk }),
            chromeApi.chrome.storage.local.set({
              vautrCiphertext: { [uuid]: { uuid, encKeyGen, payload } },
            }),
          ]);
        },
        { svk: SVK, uuid: UUID, payload: envelope.payload, encKeyGen: envelope.encKeyGen },
      );
    }

    async function fillActiveFieldAndExpectSecret(secret: string) {
      await page.locator('#password').focus();
      await triggerAutofill(page);
      await page.waitForFunction(
        (expected) => {
          const el = document.getElementById('password') as HTMLInputElement | null;
          return !!el && el.value === expected;
        },
        secret,
        { timeout: 20_000 },
      );
      expect(await page.locator('#password').inputValue()).toBe(secret);
    }

    async function terminateExtensionServiceWorkers(): Promise<void> {
      // `Target.getTargets`/`Target.closeTarget` live on a browser-level session.
      const browser = context.browser();
      if (!browser) {
        throw new Error('no browser attached');
      }
      const cdp = (await (
        browser as unknown as {
          newBrowserCDPSession(): Promise<{
            send(
              method: string,
              params?: Record<string, unknown>,
            ): Promise<{
              targetInfos?: Array<{ type: string; url: string; targetId: string }>;
            }>;
            detach(): Promise<void>;
          }>;
        }
      ).newBrowserCDPSession()) as unknown as {
        send(
          method: string,
          params?: Record<string, unknown>,
        ): Promise<{
          targetInfos?: Array<{ type: string; url: string; targetId: string }>;
        }>;
        detach(): Promise<void>;
      };
      try {
        const { targetInfos = [] } = await cdp.send('Target.getTargets');
        const swTarget = targetInfos.find(
          (t) => t.type === 'service_worker' && t.url.startsWith('chrome-extension://'),
        );
        if (swTarget) {
          await cdp.send('Target.closeTarget', { targetId: swTarget.targetId });
        }
      } finally {
        await cdp.detach();
      }
    }

    test('autofill wakes, fills, terminates, and restarts a fresh stateless SW', async () => {
      const firstWorker = await waitForExtensionWorker();
      await seedStorage(firstWorker);

      await fillActiveFieldAndExpectSecret(DEMO_SECRET);

      // Force-terminate the SW (proving it holds no long-lived state).
      await terminateExtensionServiceWorkers();

      // Prove SW #1 actually terminated: its JS context is gone (evaluate rejects).
      await expect
        .poll(
          () =>
            firstWorker
              .evaluate(() => 1)
              .then(
                () => true,
                () => false,
              ),
          { timeout: 15_000 },
        )
        .toBe(false);

      // A second autofill must be served by a FRESH stateless SW that re-reads the
      // SVK + ciphertext from `chrome.storage` (no in-memory state carried over).
      await page.locator('#password').fill('');
      await fillActiveFieldAndExpectSecret(DEMO_SECRET);
    });
  });

async function triggerAutofill(page: Page) {
  await page.evaluate((uuid) => {
    window.dispatchEvent(new CustomEvent('vautr-autofill-request', { detail: { uuid } }));
  }, UUID);
}
