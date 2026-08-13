import type { DecryptedOverview } from '@vautr/client-sdk';
import { createItemCiphertextStore } from '@vautr/client-sdk/extension';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import * as browser from 'webextension-polyfill';
import { localArea, sessionArea } from '../lib/extensionStorage';

const AUTOFILL_REQUEST = 'VAUTR_AUTOFILL';

/**
 * Cache the SVK into `chrome.storage.session` so the stateless autofill Service
 * Worker can decrypt items on wake without holding long-lived state.
 */
export async function cacheSvkForSw(svk: Uint8Array): Promise<void> {
  const { createSvkSessionStore } = await import('@vautr/client-sdk/extension');
  const store = createSvkSessionStore(sessionArea);
  await store.cache(svk);
}

/**
 * Read every locally-synced item from the shared IndexedDB store and cache its
 * encrypted envelope into `chrome.storage.local` for the stateless SW.
 */
export async function cacheAllCiphertexts(): Promise<void> {
  try {
    const { IndexedDbStore } = await import('../../../../packages/vautr-client-sdk/src/storage');
    const store = new IndexedDbStore();
    const storedItems = await store.getItems();
    const ciphertextStore = createItemCiphertextStore(localArea);
    for (const item of storedItems) {
      if (item.payload) {
        const binaryStr = atob(item.payload);
        const payloadBytes: number[] = [];
        for (let i = 0; i < binaryStr.length; i += 1) {
          payloadBytes.push(binaryStr.charCodeAt(i));
        }
        await ciphertextStore.cache({
          uuid: item.uuid,
          encKeyGen: item.encKeyGen,
          payload: payloadBytes,
        });
      }
    }
  } catch {
    // Non-critical: autofill falls back gracefully.
  }
}

/**
 * Autofill an item into the active tab via the stateless Service Worker. The SW
 * decrypts from `chrome.storage` and asks the tab's content script to fill the
 * focused field.
 */
export async function autofillItem(
  _client: VautrWebClient,
  uuid: string,
): Promise<{ ok: boolean; message: string }> {
  await cacheAllCiphertexts();
  const response = (await browser.runtime.sendMessage({
    type: AUTOFILL_REQUEST,
    uuid,
  })) as { ok: boolean; filled?: boolean; error?: string } | undefined;
  if (response?.ok) {
    return {
      ok: true,
      message: response.filled ? 'Autofilled.' : 'Decrypted; no focused field.',
    };
  }
  return { ok: false, message: response?.error ?? 'Autofill failed.' };
}

/** Copy an item's secret to the clipboard via the opaque-handle pattern. */
export async function copySecret(client: VautrWebClient, item: DecryptedOverview): Promise<void> {
  const handle = await client.reveal(item.uuid);
  await client.performAction({ type: 'CopyToClipboard', handle });
  await client.release(handle);
}
