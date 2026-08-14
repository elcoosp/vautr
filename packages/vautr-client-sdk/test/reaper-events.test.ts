import { describe, expect, it, vi } from 'vitest';
import { AsyncCryptoAdapter } from '../src/crypto';
import { VautrWebClient } from '../src/realClient';
import type { VaultStateUpdate } from '../src/types';

/**
 * Tests for the server-backed quarantine reaper SSE consumer (VTR-069).
 * Verifies that `GET /events` frames are parsed and mapped to the correct
 * local state update: `item_permanently_deleted` -> store.deleteItem +
 * OverviewDeleted; `item_recovered` -> re-sync trigger.
 */

/** A minimal store spy that records deleteItem calls. */
function fakeStore() {
  const deletes: string[] = [];
  return {
    deletes,
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
    async getItems() {
      return [];
    },
    async deleteItem(uuid: string) {
      deletes.push(uuid);
    },
  };
}

/** Build an SSE `fetch` shim that replays the given frames once, then closes. */
function sseFetch(frames: string[]): typeof fetch {
  return (async (_url: string, _init: unknown) => {
    const encoder = new TextEncoder();
    // Real SSE frames are terminated by a blank line (`\n\n`); the trailing
    // separator is what marks a frame as complete on the client.
    const body = frames.map((f) => `${f}\n\n`).join('');
    const bytes = encoder.encode(body);
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(bytes);
        controller.close();
      },
    });
    return new Response(stream, {
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
    });
  }) as unknown as typeof fetch;
}

describe('subscribeVaultEvents (reaper SSE)', () => {
  it('deletes a tombstoned item and emits OverviewDeleted', async () => {
    const store = fakeStore();
    const updates: VaultStateUpdate[] = [];
    const client = new VautrWebClient({
      baseUrl: 'http://test',
      crypto: new AsyncCryptoAdapter(),
      store: store as never,
    });
    client.getApi().setToken('tok');
    (client as { sync: () => Promise<void> }).sync = vi.fn().mockResolvedValue(undefined);
    client.subscribe((u) => updates.push(u));

    // Override fetch with the SSE shim.
    const origFetch = globalThis.fetch;
    globalThis.fetch = sseFetch(['data: {"type":"item_permanently_deleted","uuid":"deadbeef"}']);
    const stop = client.subscribeVaultEvents();
    // Give the stream a tick to be read + processed.
    await new Promise((r) => setTimeout(r, 50));
    stop();
    globalThis.fetch = origFetch;

    expect(store.deletes).toContain('deadbeef');
    expect(updates.some((u) => u.type === 'OverviewDeleted' && u.uuid === 'deadbeef')).toBe(true);
  });

  it('re-syncs on item_recovered when unlocked', async () => {
    const store = fakeStore();
    const syncSpy = vi.fn().mockResolvedValue(undefined);
    const client = new VautrWebClient({
      baseUrl: 'http://test',
      crypto: new AsyncCryptoAdapter(),
      store: store as never,
    });
    client.getApi().setToken('tok');
    (client as { sync: () => Promise<void> }).sync = syncSpy;
    // Mark unlocked so item_recovered triggers a sync.
    (client as { isUnlocked: () => boolean }).isUnlocked = () => true;

    const origFetch = globalThis.fetch;
    globalThis.fetch = sseFetch(['data: {"type":"item_recovered","uuid":"abc"}']);
    const stop = client.subscribeVaultEvents();
    await new Promise((r) => setTimeout(r, 50));
    stop();
    globalThis.fetch = origFetch;

    expect(syncSpy).toHaveBeenCalled();
  });
});
