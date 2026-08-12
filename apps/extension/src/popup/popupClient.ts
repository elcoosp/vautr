import { VautrWebClient, type VautrWebClientOptions } from '@vautr/client-sdk/real';
import { VautrMlpClient } from '@vautr/client-sdk';
import { IndexedDbStore } from '../../../../packages/vautr-client-sdk/src/storage';
import { getApiUrl } from '../lib/apiUrl';

/**
 * Popup-scoped client lifecycle (build-env-deploy §3.3).
 *
 * The popup instantiates a full `VautrWebClient` backed by IndexedDB and the
 * real `vautr-wasm` module, plus a `VautrMlpClient` sharing the same session
 * bearer token for the Projects/Secrets/MFA surface. When the popup closes the
 * client locks gracefully.
 *
 * A shared `IndexedDbStore` is exposed so the popup can read the SVK after
 * login (to cache in `chrome.storage.session` for the stateless SW).
 */
let client: VautrWebClient | null = null;
let mlp: VautrMlpClient | null = null;
let sharedStore: IndexedDbStore | null = null;

/** Resolve the server URL and create the popup client lazily. */
export async function getPopupClient(): Promise<VautrWebClient> {
  if (!client) {
    const baseUrl = await getApiUrl();
    sharedStore = new IndexedDbStore();
    const options: VautrWebClientOptions = { baseUrl, store: sharedStore };
    client = new VautrWebClient(options);
  }
  return client;
}

/** The MLP client sharing the session token. Safe to call only while unlocked. */
export async function getPopupMlpClient(): Promise<VautrMlpClient> {
  const c = await getPopupClient();
  if (!mlp) {
    mlp = new VautrMlpClient(c.getApi());
  }
  return mlp;
}

/** Read the SVK from the shared IndexedDB store (set during login). */
export async function getCachedSvk(): Promise<Uint8Array | null> {
  if (!sharedStore) return null;
  const state = await sharedStore.getState();
  return state.svk;
}

/** Graceful shutdown: lock the vault and drop the client. */
export async function disposePopupClient(): Promise<void> {
  if (client) {
    await client.lock();
    client = null;
    mlp = null;
    sharedStore = null;
  }
}
