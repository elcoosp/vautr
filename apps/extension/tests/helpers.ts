import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import type { BrowserContext, Page, Worker } from '@playwright/test';
import { chromium, expect } from '@playwright/test';

/**
 * Shared helpers for the extension Playwright E2E suites.
 *
 * These mirror the helper patterns established in `autofill-smoke.spec.ts`
 * (build-env-deploy §6.2): launch Chromium with `--load-extension`, wake the
 * stateless Service Worker, seed `chrome.storage`, drive autofill through the
 * real content-script relay, and force-terminate the SW via CDP.
 */

export const DEMO_SECRET = 'vautr-demo-password-0x3f9a';
export const DEFAULT_UUID = 'b2e7b6d0-8c1a-4b2e-9f0a-6c2d3e4f5a6b';
/** Stand-in raw SVK (browser-encrypted session cache). */
export const SVK = Array.from(new Uint8Array(32).fill(7));

const extensionRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
export const distPath = join(extensionRoot, 'dist');

// Real WASM crypto (nodejs build) used to produce genuine AEAD envelopes that the
// stateless SW can decrypt. The SW's `decrypt_secret_with_svk` expects the same
// DEK-derived ciphertext format that `encrypt_item_js` produces.
const require = createRequire(import.meta.url);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const wasm: any = require(join(extensionRoot, 'wasm-pkg-nodejs/vautr_wasm.js'));

export interface SecretEnvelope {
  encKeyGen: number;
  payload: number[];
}

/**
 * Build a real AEAD-encrypted item envelope for `secret` under the given raw SVK
 * and uuid. Matches what the app stores server-side and what the stateless SW
 * decrypts on autofill.
 */
export function buildSecretEnvelope(
  secret: string,
  uuid: string,
  svk: number[] = SVK,
  encKeyGen = 1,
): SecretEnvelope {
  const dek = wasm.derive_dek_js(Uint8Array.from(svk));
  const plaintext = Buffer.from(
    JSON.stringify({
      title: 'Demo',
      subtitle: 'demo@example.com',
      iconKey: 'key',
      urls: ['https://example.com'],
      password: secret,
    }),
    'utf8',
  );
  const payload = wasm.encrypt_item_js(uuid, BigInt(encKeyGen), dek, plaintext);
  return { encKeyGen, payload: Array.from(payload) };
}

export interface LaunchOptions {
  profileDir: string;
}

/**
 * Launch a persistent Chromium context with the built extension loaded
 * (`--disable-extensions-except` + `--load-extension`). A fresh `profileDir`
 * must be supplied by the caller (and cleaned up afterwards).
 */
export async function launchExtensionContext(options: LaunchOptions): Promise<BrowserContext> {
  return chromium.launchPersistentContext(options.profileDir, {
    headless: true,
    channel: 'chromium',
    args: [`--disable-extensions-except=${distPath}`, `--load-extension=${distPath}`],
  });
}

/** The running extension service worker, if any. */
export function getExtensionWorker(context: BrowserContext, exclude?: Worker): Worker | undefined {
  return context
    .serviceWorkers()
    .find((w) => w.url().startsWith('chrome-extension://') && w !== exclude);
}

/**
 * Wait for the extension SW to be running, waking it via the autofill relay when
 * needed (safe when storage is empty — the SW replies with an error).
 */
export async function waitForExtensionWorker(
  context: BrowserContext,
  exclude?: Worker,
  timeoutMs = 20_000,
): Promise<Worker> {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const running = getExtensionWorker(context, exclude);
    if (running) {
      return running;
    }
    const page = context.pages()[0];
    if (page) {
      try {
        await triggerAutofill(page);
      } catch {
        // ignore
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error('extension service worker never started');
}

/** Resolve the extension id (e.g. `chrome-extension://<id>/...`) by waking the SW. */
export async function getExtensionId(context: BrowserContext): Promise<string> {
  const worker = await waitForExtensionWorker(context);
  const id = worker.url().split('/')[2];
  if (!id) {
    throw new Error(`could not derive extension id from ${worker.url()}`);
  }
  return id;
}

/** Seed `chrome.storage.session` SVK + `chrome.storage.local` ciphertext. */
export async function seedStorage(
  worker: Worker,
  options: { svk?: number[]; uuid?: string } = {},
): Promise<void> {
  const { svk = SVK, uuid = DEFAULT_UUID } = options;
  const envelope = buildSecretEnvelope(DEMO_SECRET, uuid, svk);
  await worker.evaluate(
    ({ svkValue, uuidValue, payloadValue, encKeyGen }) => {
      const chromeApi = globalThis as unknown as {
        chrome: {
          storage: {
            session: { set: (v: Record<string, unknown>) => Promise<void> };
            local: { set: (v: Record<string, unknown>) => Promise<void> };
          };
        };
      };
      return Promise.all([
        chromeApi.chrome.storage.session.set({ vautrSvk: svkValue }),
        chromeApi.chrome.storage.local.set({
          vautrCiphertext: {
            [uuidValue]: { uuid: uuidValue, encKeyGen, payload: payloadValue },
          },
        }),
      ]);
    },
    {
      svkValue: svk,
      uuidValue: uuid,
      payloadValue: envelope.payload,
      encKeyGen: envelope.encKeyGen,
    },
  );
}

/** Dispatch the autofill request via the page's main world (content-script relay). */
export async function triggerAutofill(page: Page, uuid: string = DEFAULT_UUID): Promise<void> {
  await page.evaluate((u) => {
    window.dispatchEvent(new CustomEvent('vautr-autofill-request', { detail: { uuid: u } }));
  }, uuid);
}

/** The last autofill response the content script echoed onto the document. */
export function autofillResponseAttribute(page: Page): Promise<string | null> {
  return page.evaluate(() => document.documentElement.getAttribute('data-vautr-autofill'));
}

interface ChromeStorageWorker {
  chrome: {
    storage: {
      session: { get(keys: string | string[] | null): Promise<Record<string, unknown>> };
      local: { get(keys: string | string[] | null): Promise<Record<string, unknown>> };
    };
  };
}

/** Read from `chrome.storage.session` through the SW (typed). */
export async function readSessionFromWorker(
  worker: Worker,
  keys: string | string[] | null,
): Promise<Record<string, unknown>> {
  return worker.evaluate(
    (ks) => (globalThis as unknown as ChromeStorageWorker).chrome.storage.session.get(ks),
    keys,
  );
}

/** Whether the raw plaintext secret appears anywhere in session+local storage. */
export async function scanStoresForPlaintext(worker: Worker, secret: string): Promise<boolean> {
  return worker.evaluate((plaintext) => {
    const chromeApi = globalThis as unknown as ChromeStorageWorker;
    return Promise.all([
      chromeApi.chrome.storage.session.get(null),
      chromeApi.chrome.storage.local.get(null),
    ]).then(([session, local]) => JSON.stringify({ session, local }).includes(plaintext));
  }, secret);
}

/**
 * Focus `selector`, dispatch an autofill request, and wait until the focused
 * field holds `expected`. Returns after the fill is observed.
 */
export async function fillActiveFieldAndExpectSecret(
  page: Page,
  selector: string,
  expected: string = DEMO_SECRET,
): Promise<void> {
  await page.locator(selector).focus();
  await triggerAutofill(page);
  await page.waitForFunction(
    ({ selector: sel, expected: exp }) => {
      const el = document.querySelector(sel) as HTMLInputElement | null;
      return !!el && el.value === exp;
    },
    { selector, expected },
    { timeout: 20_000 },
  );
}

/** Force-terminate the extension SW via a browser-level CDP session. */
export async function terminateExtensionServiceWorkers(context: BrowserContext): Promise<void> {
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

/**
 * Poll until the given SW's JS context is gone (i.e. it was terminated).
 * Mirrors the proven pattern in `autofill-smoke.spec.ts`: the callback always
 * resolves to a boolean so the poll can never hang on a lingering evaluate.
 */
export async function expectWorkerTerminated(worker: Worker): Promise<void> {
  await expect
    .poll(
      () =>
        worker
          .evaluate(() => 1)
          .then(
            () => true,
            () => false,
          ),
      { timeout: 15_000 },
    )
    .toBe(false);
}
