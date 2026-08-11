import { describe, expect, it } from 'vitest';
import {
  createItemCiphertextStore,
  createStatelessCrypto,
  createSvkSessionStore,
  statelessAutofill,
  type StorageArea,
} from '@vautr/client-sdk/extension';
import { buildSecretEnvelope } from './helpers';

/**
 * Unit tests for the extension-autofill entrypoints (SDK `src/extension.ts`).
 *
 * These exercise the stateless single-wake flow against a fake storage area and
 * the dev `vautr-wasm-nodejs` mock: cache SVK -> cache ciphertext -> stateless
 * decrypt -> report. Plaintext never touches storage; only the ciphertext
 * envelope is persisted.
 */

const DEMO_SECRET = 'vautr-demo-password-0x3f9a';
const DEFAULT_UUID = 'b2e7b6d0-8c1a-4b2e-9f0a-6c2d3e4f5a6b';

function realEnvelope(): { encKeyGen: number; payload: number[] } {
  return buildSecretEnvelope(DEMO_SECRET, DEFAULT_UUID);
}

function fakeStorage(): StorageArea & { data: Record<string, unknown> } {
  const data: Record<string, unknown> = {};
  return {
    data,
    async get(keys) {
      if (keys === null) {
        return { ...data };
      }
      const list = Array.isArray(keys) ? keys : [keys];
      const out: Record<string, unknown> = {};
      for (const key of list) {
        if (key in data) {
          out[key] = data[key];
        }
      }
      return out;
    },
    async set(items) {
      Object.assign(data, items);
    },
    async remove(keys) {
      const list = Array.isArray(keys) ? keys : [keys];
      for (const key of list) {
        delete data[key];
      }
    },
  };
}

describe('vautr-client-sdk extension autofill entrypoints', () => {
  it('statelessly decrypts a cached item given a cached SVK', async () => {
    const session = fakeStorage();
    const local = fakeStorage();
    const svkStore = createSvkSessionStore(session);
    const ciphertextStore = createItemCiphertextStore(local);
    const crypto = createStatelessCrypto();

    await svkStore.cache(new Uint8Array(32).fill(7));
    const env = realEnvelope();
    await ciphertextStore.cache({ uuid: DEFAULT_UUID, encKeyGen: env.encKeyGen, payload: env.payload });

    const secret = await statelessAutofill(svkStore, ciphertextStore, crypto, DEFAULT_UUID);
    expect(secret).toBe(DEMO_SECRET);
  });

  it('refuses to autofill when the vault is locked (no cached SVK)', async () => {
    const session = fakeStorage();
    const local = fakeStorage();
    const svkStore = createSvkSessionStore(session);
    const ciphertextStore = createItemCiphertextStore(local);
    await ciphertextStore.cache({ uuid: 'u1', encKeyGen: 1, payload: [1, 2, 3] });

    await expect(
      statelessAutofill(svkStore, ciphertextStore, createStatelessCrypto(), 'u1'),
    ).rejects.toThrow(/locked/);
  });

  it('refuses to autofill when the item ciphertext is not cached', async () => {
    const session = fakeStorage();
    const local = fakeStorage();
    const svkStore = createSvkSessionStore(session);
    await svkStore.cache(new Uint8Array(32).fill(9));

    await expect(
      statelessAutofill(svkStore, createItemCiphertextStore(local), createStatelessCrypto(), 'u1'),
    ).rejects.toThrow(/no cached ciphertext/);
  });

  it('round-trips the SVK through the session store and zeroizes on clear', async () => {
    const session = fakeStorage();
    const svkStore = createSvkSessionStore(session);
    const svk = new Uint8Array([1, 2, 3, 4, 5]);
    await svkStore.cache(svk);
    await expect(svkStore.read()).resolves.toEqual(svk);
    await svkStore.clear();
    await expect(svkStore.read()).resolves.toBeNull();
  });

  it('rejects when a cached item is removed from the ciphertext store', async () => {
    const session = fakeStorage();
    const local = fakeStorage();
    const svkStore = createSvkSessionStore(session);
    const ciphertextStore = createItemCiphertextStore(local);
    await svkStore.cache(new Uint8Array(32).fill(3));
    await ciphertextStore.cache({ uuid: 'u1', encKeyGen: 1, payload: [9] });
    await ciphertextStore.remove('u1');

    await expect(
      statelessAutofill(svkStore, ciphertextStore, createStatelessCrypto(), 'u1'),
    ).rejects.toThrow(/no cached ciphertext/);
  });
});
