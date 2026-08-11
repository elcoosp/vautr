import * as browser from 'webextension-polyfill';

/**
 * Runtime API URL injection (build-env-deploy §5).
 *
 * The API URL is read from `chrome.storage.local` at runtime so the WASM bundle
 * never needs to be rebuilt to steer environments. Falls back to a local dev
 * endpoint when unset.
 */
export const API_URL_STORAGE_KEY = 'vautrApiUrl';

const DEFAULT_API_URL = 'http://localhost:8080';

/** Resolve the active Vautr server API URL. */
export async function getApiUrl(): Promise<string> {
  const data = await browser.storage.local.get([API_URL_STORAGE_KEY]);
  const url = data[API_URL_STORAGE_KEY];
  return typeof url === 'string' && url.length > 0 ? url : DEFAULT_API_URL;
}

/** Persist a new runtime API URL override. */
export async function setApiUrl(url: string): Promise<void> {
  await browser.storage.local.set({ [API_URL_STORAGE_KEY]: url });
}
