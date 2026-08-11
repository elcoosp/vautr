/**
 * IndexedDB persistence for the browser client (data.md).
 *
 * Stores the session token, account crypto salt, sync cursor, key generation,
 * and the encrypted item blobs. The server is the source of truth during sync;
 * this store is the offline/local mirror (metadata-first, ciphertext blobs).
 *
 * Plaintext secrets are never stored here — only encrypted `payload` bytes.
 */

import { fromBase64, toBase64 } from './api';

export { fromBase64, toBase64 };

/** A synced item's encrypted envelope + metadata (server `items` row). */
export interface StoredItem {
  uuid: string;
  version: number;
  encKeyGen: number;
  deletedDate: number | null;
  /** Opaque AEAD ciphertext (base64), mirror of the server payload. */
  payload: string | null;
  /** True while a local mutation is awaiting push to the server. */
  pending?: boolean;
}

/** Account + session + cursor slice. */
export interface StoredState {
  username: string | null;
  /** Per-user KDF salt (base64). Needed to re-derive MK on login. */
  kdfSalt: string | null;
  /** Local key generation (defaults to 1). */
  localKeyGen: number;
  /** Sync cursor (api.md §4). */
  cursor: number;
  /** `min_enc_key_gen` from the last /account/status. */
  minEncKeyGen: number;
  /** Bearer session token. */
  sessionToken: string | null;
  /** Recovered SVK kept for this session (decrypted, in-memory only). */
  svk: Uint8Array | null;
}

export const EMPTY_STATE: StoredState = {
  username: null,
  kdfSalt: null,
  localKeyGen: 1,
  cursor: 0,
  minEncKeyGen: 1,
  sessionToken: null,
  svk: null,
};

const DB_NAME = 'vautr-client';
const DB_VERSION = 1;
const STATE_STORE = 'state';
const ITEMS_STORE = 'items';
const ITEMS_KEY = 'uuid';

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STATE_STORE)) {
        db.createObjectStore(STATE_STORE);
      }
      if (!db.objectStoreNames.contains(ITEMS_STORE)) {
        db.createObjectStore(ITEMS_STORE, { keyPath: ITEMS_KEY });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error('indexedDB open failed'));
  });
}

function idbRequest<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error('indexedDB request failed'));
  });
}

/**
 * Minimal IndexedDB-backed store for the client. Safe to construct in a worker
 * or main thread; uses one DB with a `state` key-value store and an `items`
 * store keyed by uuid.
 */
export class IndexedDbStore {
  private dbPromise: Promise<IDBDatabase> | null = null;

  private db(): Promise<IDBDatabase> {
    if (!this.dbPromise) {
      this.dbPromise = openDb();
    }
    return this.dbPromise;
  }

  // --- state (single key) ---
  async getState(): Promise<StoredState> {
    const db = await this.db();
    const tx = db.transaction(STATE_STORE, 'readonly');
    const row = (await idbRequest(tx.objectStore(STATE_STORE).get('root'))) as
      | Partial<StoredState>
      | undefined;
    return { ...EMPTY_STATE, ...(row ?? {}) };
  }

  async setState(patch: Partial<StoredState>): Promise<void> {
    const db = await this.db();
    const current = await this.getState();
    const next: StoredState = { ...current, ...patch };
    const tx = db.transaction(STATE_STORE, 'readwrite');
    await idbRequest(tx.objectStore(STATE_STORE).put(next, 'root'));
    return new Promise((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error('state write failed'));
    });
  }

  async resetState(): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(STATE_STORE, 'readwrite');
    await idbRequest(tx.objectStore(STATE_STORE).put(EMPTY_STATE, 'root'));
    return new Promise((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error('state reset failed'));
    });
  }

  // --- items ---
  async getItems(): Promise<StoredItem[]> {
    const db = await this.db();
    const tx = db.transaction(ITEMS_STORE, 'readonly');
    const all = (await idbRequest(tx.objectStore(ITEMS_STORE).getAll())) as StoredItem[];
    return all ?? [];
  }

  async getItem(uuid: string): Promise<StoredItem | undefined> {
    const db = await this.db();
    const tx = db.transaction(ITEMS_STORE, 'readonly');
    return (await idbRequest(tx.objectStore(ITEMS_STORE).get(uuid))) as StoredItem | undefined;
  }

  async putItem(item: StoredItem): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(ITEMS_STORE, 'readwrite');
    await idbRequest(tx.objectStore(ITEMS_STORE).put(item));
    return new Promise((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error('item write failed'));
    });
  }

  async putItems(items: StoredItem[]): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(ITEMS_STORE, 'readwrite');
    const store = tx.objectStore(ITEMS_STORE);
    for (const item of items) {
      store.put(item);
    }
    return new Promise((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error('items write failed'));
    });
  }

  async deleteItem(uuid: string): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(ITEMS_STORE, 'readwrite');
    await idbRequest(tx.objectStore(ITEMS_STORE).delete(uuid));
    return new Promise((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error ?? new Error('item delete failed'));
    });
  }
}
