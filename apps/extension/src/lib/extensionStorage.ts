import type { StorageArea } from '@vautr/client-sdk/extension';
import * as browser from 'webextension-polyfill';

/**
 * Typed adapters over the browser extension storage areas.
 *
 * build-env-deploy §3.3:
 *  - session  -> raw SVK cache (browser-encrypted, wiped on browser close).
 *  - local    -> per-uuid item ciphertext + runtime API URL injection (§5).
 */
export const sessionArea: StorageArea = browser.storage.session as unknown as StorageArea;
export const localArea: StorageArea = browser.storage.local as unknown as StorageArea;
