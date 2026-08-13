/// <reference lib="webworker" />

import {
  type AutofillResponse,
  createItemCiphertextStore,
  createStatelessCrypto,
  createSvkSessionStore,
  type StorageArea,
  statelessAutofill,
} from '@vautr/client-sdk/extension';
import * as browser from 'webextension-polyfill';

/**
 * Stateless Autofill Service Worker (build-env-deploy §3.3).
 *
 * MV3 service workers are ephemeral: the browser terminates them after ~30s of
 * inactivity. This worker therefore holds NO long-lived state — no `VautrClient`,
 * no DashMap, no SQLite handle. On every wake it:
 *   1. reads the raw SVK from `chrome.storage.session` (browser-encrypted),
 *   2. reads the item ciphertext from `chrome.storage.local`,
 *   3. decrypts it with a fresh `--target nodejs` stateless crypto instance,
 *   4. asks the content script to fill the focused field,
 *   5. returns and lets the browser terminate it.
 */

const AUTOFILL_REQUEST = 'VAUTR_AUTOFILL';
const AUTOFILL_FILL = 'VAUTR_FILL';

/** Storage areas injected through the polyfill. */
const sessionArea = (browser.storage as unknown as { session: StorageArea }).session;
const localArea = browser.storage.local as unknown as StorageArea;

/** Wire protocol message from the popup. */
interface AutofillRequest {
  type: 'VAUTR_AUTOFILL';
  uuid: string;
}

function isAutofillRequest(value: unknown): value is AutofillRequest {
  return (
    !!value &&
    typeof value === 'object' &&
    (value as { type?: string }).type === AUTOFILL_REQUEST &&
    typeof (value as { uuid?: string }).uuid === 'string'
  );
}

async function handleAutofill(
  request: AutofillRequest,
  sender: browser.Runtime.MessageSender,
): Promise<AutofillResponse> {
  try {
    const svkStore = createSvkSessionStore(sessionArea);
    const ciphertextStore = createItemCiphertextStore(localArea);
    const crypto = createStatelessCrypto();

    // One stateless decrypt, then release all references. The browser may kill
    // this worker immediately after the response is delivered.
    const secret = await statelessAutofill(svkStore, ciphertextStore, crypto, request.uuid);

    const tabId = sender.tab?.id;
    if (tabId === undefined) {
      return { ok: true, filled: false, reason: 'no-active-tab' };
    }
    await browser.tabs.sendMessage(tabId, { type: AUTOFILL_FILL, secret });
    return { ok: true, filled: true };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { ok: false, error: message };
  }
}

browser.runtime.onMessage.addListener(((message, sender, sendResponse) => {
  if (isAutofillRequest(message)) {
    void handleAutofill(message, sender).then(sendResponse);
    return true; // keep the message channel open for the async response
  }
  return;
}) as Parameters<typeof browser.runtime.onMessage.addListener>[0]);
