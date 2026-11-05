/**
 * VTRFIX-BUG-C12: an OCC conflict on push must not loop forever.
 *
 * The server applies a push when `version = target_version` and then increments
 * (core/vautr-server/src/repository/items.rs `upsert_item_occ`), so after a
 * `conflict` result the SDK must re-base the pending item at the server's
 * current version (echoed in `current_server_state`, sync.rs `PushResult`).
 * The next push then targets the version the server actually holds and the
 * local edit lands at `serverVersion + 1` (data.md §7.2) — instead of the old
 * behaviour of re-sending the same stale `target_version` every sync forever.
 */
import { describe, expect, it } from 'vitest';
import { AsyncCryptoAdapter } from '../src/crypto';
import { VautrWebClient } from '../src/realClient';
import type { VaultStateUpdate } from '../src/types';

interface StoredItem {
  uuid: string;
  version: number;
  encKeyGen: number;
  deletedDate: number | null;
  payload: string | null;
  pending: boolean;
}

/** Minimal IndexedDbStore spy with in-memory items (mirrors reaper-events.test). */
function fakeStore(initial: StoredItem[]) {
  const store = {
    items: initial,
    async getState() {
      return {
        username: 'me',
        kdfSalt: null,
        svk: null,
        sessionToken: 'tok',
        cursor: 0,
        localKeyGen: 1,
        minEncKeyGen: 1,
        sharingSecretKey: null,
        groupKeys: {},
      };
    },
    async setState() {},
    async getItems(): Promise<StoredItem[]> {
      return store.items;
    },
    async getItem(uuid: string): Promise<StoredItem | null> {
      return store.items.find((i) => i.uuid === uuid) ?? null;
    },
    async putItem(item: StoredItem): Promise<void> {
      const idx = store.items.findIndex((i) => i.uuid === item.uuid);
      if (idx >= 0) {
        store.items[idx] = item;
      } else {
        store.items.push(item);
      }
    },
    async deleteItem(uuid: string): Promise<void> {
      const idx = store.items.findIndex((i) => i.uuid === uuid);
      if (idx >= 0) store.items.splice(idx, 1);
    },
  };
  return store;
}

const ITEM_UUID = '44444444-4444-4444-8444-444444444444';
const PAYLOAD = 'Y2lwaGVydGV4dC1ibG9i'; // base64("ciphertext-blob")

interface PushCall {
  target_version: number;
}

/**
 * Stateful fetch shim: serves `/sync/pull` (empty) and a scripted sequence of
 * `/sync/push-batch` responses, recording each push's `target_version`.
 */
function scriptedFetch(pushResponses: unknown[]) {
  const pushes: PushCall[] = [];
  let pushIndex = 0;
  const impl = (async (url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.includes('/sync/pull')) {
      return new Response(
        JSON.stringify({ new_cursor: 0, has_more: false, min_enc_key_gen: 1, items: [] }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      );
    }
    if (u.includes('/sync/push-batch')) {
      const body = JSON.parse(String(init?.body ?? '{}')) as { items: PushCall[] };
      for (const item of body.items) pushes.push(item);
      const respond = pushResponses[Math.min(pushIndex, pushResponses.length - 1)];
      pushIndex += 1;
      return new Response(JSON.stringify(respond), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    }
    throw new Error(`unexpected fetch: ${u}`);
  }) as unknown as typeof fetch;
  return { impl, pushes };
}

async function makeClient(items: StoredItem[], fetcher: { impl: typeof fetch }) {
  // ApiClient binds `fetch` at construction, so the shim must be installed
  // BEFORE the client is created (mirrors reaper-events.test.ts).
  const origFetch = globalThis.fetch;
  globalThis.fetch = fetcher.impl;
  const store = fakeStore(items);
  const client = new VautrWebClient({
    baseUrl: 'http://test',
    crypto: new AsyncCryptoAdapter(),
    store: store as never,
  });
  client.getApi().setToken('tok');
  // Unlock just enough for sync(): the DEK is only used to decrypt pulled
  // payloads and this scenario pulls none.
  (client as unknown as { dek: Uint8Array }).dek = new Uint8Array(32);
  const updates: VaultStateUpdate[] = [];
  client.subscribe((u) => updates.push(u));
  return {
    client,
    store,
    updates,
    restoreFetch(): void {
      globalThis.fetch = origFetch;
    },
  };
}

describe('sync push conflict re-target (VTRFIX-BUG-C12)', () => {
  it('re-bases a conflicted pending item at the server version, then lands at serverVersion + 1', async () => {
    // Local edit created at target_version 2; the server holds version 3.
    const store = fakeStore([
      {
        uuid: ITEM_UUID,
        version: 2,
        encKeyGen: 1,
        deletedDate: null,
        payload: PAYLOAD,
        pending: true,
      },
    ]);
    const fetcher = scriptedFetch([
      // 1st push: stale target_version → conflict, server echoes its row state.
      {
        results: [
          {
            uuid: ITEM_UUID,
            status: 'conflict',
            version: null,
            enc_key_gen: null,
            updated_at: null,
            current_server_state: { version: 3, enc_key_gen: 1 },
          },
        ],
      },
      // 2nd push (re-based at 3): succeeds, server lands the edit at 4.
      {
        results: [
          {
            uuid: ITEM_UUID,
            status: 'success',
            version: 4,
            enc_key_gen: 1,
            updated_at: 1700000000,
          },
        ],
      },
    ]);
    const { client, updates, restoreFetch } = await makeClient(store.items, fetcher);
    try {
      await client.sync(); // conflict turn

      // The conflict is surfaced to the UI with the server's real version.
      const conflict = updates.find((u) => u.type === 'ConflictDetected') as
        | { event: { uuid: string; localVersion: string; serverVersion: string } }
        | undefined;
      expect(conflict?.event.uuid).toBe(ITEM_UUID);
      expect(conflict?.event.localVersion).toBe('2');
      expect(conflict?.event.serverVersion).toBe('3');

      // The pending item is kept AND re-based at the server version so the
      // next push no longer targets the stale version.
      let stored = store.items.find((i) => i.uuid === ITEM_UUID);
      expect(stored?.pending).toBe(true);
      expect(stored?.version).toBe(3);
      expect(fetcher.pushes[0]?.target_version).toBe(2);

      updates.length = 0;
      await client.sync(); // re-push turn (resolveConflict('pushLocal') calls sync() too)
      stored = store.items.find((i) => i.uuid === ITEM_UUID);
      expect(stored?.pending).toBe(false);
      expect(stored?.version).toBe(4);
      expect(fetcher.pushes[1]?.target_version).toBe(3);
      expect(updates.some((u) => u.type === 'ConflictDetected')).toBe(false);
    } finally {
      restoreFetch();
    }
  });

  it('keeps the item pending without a bogus re-base when the server sends no version', async () => {
    const store = fakeStore([
      {
        uuid: ITEM_UUID,
        version: 2,
        encKeyGen: 1,
        deletedDate: null,
        payload: PAYLOAD,
        pending: true,
      },
    ]);
    const fetcher = scriptedFetch([
      // Old server shape: conflict without current_server_state.
      {
        results: [{ uuid: ITEM_UUID, status: 'conflict', version: null }],
      },
    ]);
    const { client, restoreFetch } = await makeClient(store.items, fetcher);
    try {
      await client.sync();
      const stored = store.items.find((i) => i.uuid === ITEM_UUID);
      // Still pending, still at the local version (no invented bump), so the
      // next sync retries instead of silently claiming the server state.
      expect(stored?.pending).toBe(true);
      expect(stored?.version).toBe(2);
      expect(fetcher.pushes[0]?.target_version).toBe(2);
    } finally {
      restoreFetch();
    }
  });
});
