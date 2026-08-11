/**
 * Vautr client SDK — extension-autofill entrypoints (build-env-deploy §3.3).
 *
 * These entrypoints are consumed exclusively by the Manifest V3 browser
 * extension (popup + stateless Service Worker). They are isolated in this file
 * so the shared `index.ts` surface (used by web + mobile) is untouched.
 *
 * Security contracts honoured here (build-env-deploy §3.3, data.md §1):
 *  - The autofill Service Worker is 100% STATELESS. It wakes on an OS event,
 *    runs ONE action, and terminates. It never opens SQLite.
 *  - The raw SVK is cached in `chrome.storage.session` (encrypted by the
 *    browser, wiped on close) and item ciphertext in `chrome.storage.local`.
 *  - Only the crypto-only nodejs-target WASM surface (`vautr-wasm-nodejs`) is
 *    imported. `read_secret` (desktop-api feature) is compiled out of that
 *    target and never reaches the extension bundle (build-env-deploy §2.5).
 *  - u64 epochs (encKeyGen) cross as JS numbers within the safe-integer range.
 */

// ---------------------------------------------------------------------------
// Stateless crypto (nodejs-target WASM)
// ---------------------------------------------------------------------------

// Static import (not dynamic): the Manifest V3 Service Worker runs in a module
// context with no `window`/`document`. Vite wraps dynamic `import()` calls in a
// preload helper that references `window`, which crashes in the SW. A static
// import keeps the module bundled into the same chunk and avoids that path.
import * as vautrWasmNodejs from 'vautr-wasm-nodejs';

/** Inputs needed to statelessly decrypt a single autofill item. */
export interface AutofillDecryptArgs {
  /** Raw 32-byte SVK read from `chrome.storage.session`. */
  svk: Uint8Array;
  uuid: string;
  /** u64 encryption-key generation, safe-integer range. */
  encKeyGen: number;
  /** Raw AEAD ciphertext envelope bytes. */
  payload: Uint8Array;
}

/**
 * Minimal stateless decryptor over the nodejs-target WASM build. The SW holds
 * no long-lived `WebClient`, no DashMap, and no SQLite handle: it constructs a
 * fresh crypto instance per wake, decrypts one item, and lets the browser
 * terminate the worker.
 */
export interface StatelessCrypto {
  decryptSecret(args: AutofillDecryptArgs): Promise<string>;
}

/**
 * Concrete stateless decryptor. Lazily loads + initialises the nodejs WASM
 * module once per worker wake (the instance is never persisted across worker
 * restarts because the browser evicts the worker).
 */
export class StatelessAutofillCrypto implements StatelessCrypto {
  private initPromise: Promise<void> | null = null;

  private ensureWasm(): Promise<void> {
    if (!this.initPromise) {
      this.initPromise = (async () => {
        if (typeof vautrWasmNodejs.init === 'function') {
          await vautrWasmNodejs.init();
        }
      })();
    }
    return this.initPromise;
  }

  async decryptSecret(args: AutofillDecryptArgs): Promise<string> {
    await this.ensureWasm();
    return vautrWasmNodejs.decrypt_secret_with_svk(
      new Uint8Array(args.svk),
      args.uuid,
      args.encKeyGen,
      new Uint8Array(args.payload),
    );
  }
}

/** Construct a fresh stateless decryptor for a single SW wake. */
export function createStatelessCrypto(): StatelessCrypto {
  return new StatelessAutofillCrypto();
}

// ---------------------------------------------------------------------------
// chrome.storage adapters (injected so the SDK stays chrome-free & testable)
// ---------------------------------------------------------------------------

/**
 * Minimal subset of `chrome.storage.StorageArea` (works with both
 * `chrome.storage.session` and `chrome.storage.local`).
 */
export interface StorageArea {
  get(keys: string | string[] | null): Promise<Record<string, unknown>>;
  set(items: Record<string, unknown>): Promise<void>;
  remove(keys: string | string[]): Promise<void>;
}

/** Storage key holding the raw SVK (browser-encrypted session storage). */
export const SVK_STORAGE_KEY = 'vautrSvk';

/** Storage key holding the per-uuid ciphertext map (local storage). */
export const CIPHERTEXT_STORAGE_KEY = 'vautrCiphertext';

/** Raw SVK session store backed by an injected `chrome.storage.session`. */
export interface SvkSessionStore {
  /** Cache the raw SVK. Overwrites any prior value. */
  cache(svk: Uint8Array): Promise<void>;
  /** Read the cached SVK, or `null` when none is cached (locked vault). */
  read(): Promise<Uint8Array | null>;
  /** Wipe the cached SVK (e.g. vault lock / popup close). */
  clear(): Promise<void>;
}

/** Build an SVK session store over an injected session storage area. */
export function createSvkSessionStore(session: StorageArea): SvkSessionStore {
  return {
    async cache(svk) {
      await session.set({ [SVK_STORAGE_KEY]: Array.from(svk) });
    },
    async read() {
      const data = await session.get([SVK_STORAGE_KEY]);
      const raw = data[SVK_STORAGE_KEY];
      if (!Array.isArray(raw) || raw.length === 0) {
        return null;
      }
      return Uint8Array.from(raw as number[]);
    },
    async clear() {
      await session.remove([SVK_STORAGE_KEY]);
    },
  };
}

/** A single item's ciphertext envelope cached for stateless autofill. */
export interface ItemCiphertext {
  uuid: string;
  /** u64 encryption-key generation, safe-integer range. */
  encKeyGen: number;
  /** Raw AEAD ciphertext envelope bytes. */
  payload: number[];
}

/**
 * Wire response for a stateless autofill request (popup <-> Service Worker).
 * `filled: true` means the SW decrypted and asked the content script to fill
 * the focused field.
 */
export type AutofillResponse =
  | { ok: true; filled: boolean; reason?: string }
  | { ok: false; error: string };

/**
 * Per-uuid ciphertext cache backed by `chrome.storage.local`. Plaintext never
 * touches this store; only the encrypted envelope is persisted.
 */
export interface ItemCiphertextStore {
  cache(item: ItemCiphertext): Promise<void>;
  read(uuid: string): Promise<ItemCiphertext | null>;
  remove(uuid: string): Promise<void>;
}

/** Build a ciphertext store over an injected local storage area. */
export function createItemCiphertextStore(local: StorageArea): ItemCiphertextStore {
  async function readAll(): Promise<Record<string, ItemCiphertext>> {
    const data = await local.get([CIPHERTEXT_STORAGE_KEY]);
    const raw = data[CIPHERTEXT_STORAGE_KEY];
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
      return {};
    }
    return raw as Record<string, ItemCiphertext>;
  }

  return {
    async cache(item) {
      const all = await readAll();
      const { uuid, encKeyGen, payload } = item;
      all[uuid] = { uuid, encKeyGen, payload };
      await local.set({ [CIPHERTEXT_STORAGE_KEY]: all });
    },
    async read(uuid) {
      const all = await readAll();
      return all[uuid] ?? null;
    },
    async remove(uuid) {
      const all = await readAll();
      delete all[uuid];
      await local.set({ [CIPHERTEXT_STORAGE_KEY]: all });
    },
  };
}

// ---------------------------------------------------------------------------
// Stateless autofill orchestration (single-wake, then terminate)
// ---------------------------------------------------------------------------

/**
 * Execute one stateless autofill action for `uuid`.
 *
 * Reads the SVK from the session store, the item ciphertext from the local
 * store, decrypts it with a fresh stateless crypto instance, and returns the
 * plaintext secret to the caller (the SW fills the focused field and returns).
 * Throws when the vault is locked (no cached SVK) or the item is missing.
 */
export async function statelessAutofill(
  svkStore: SvkSessionStore,
  ciphertextStore: ItemCiphertextStore,
  crypto: StatelessCrypto,
  uuid: string,
): Promise<string> {
  const svk = await svkStore.read();
  if (!svk) {
    throw new Error('vault is locked: no SVK cached in chrome.storage.session');
  }
  const item = await ciphertextStore.read(uuid);
  if (!item) {
    throw new Error(`no cached ciphertext for item ${uuid}`);
  }
  return crypto.decryptSecret({
    svk,
    uuid,
    encKeyGen: item.encKeyGen,
    payload: Uint8Array.from(item.payload),
  });
}
