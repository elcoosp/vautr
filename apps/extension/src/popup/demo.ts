import type { DecryptedOverview } from '@vautr/ui-logic';

/**
 * Demo vault contents for the popup (mirrors apps/web/src/lib/client.ts).
 *
 * The `payload` values are stand-in AEAD ciphertext envelopes for the dev mock;
 * real builds derive them from the server blob store. `encKeyGen` is a u64
 * epoch held within the JS safe-integer range (data.md §1 rule 1).
 */

export interface DemoItemCiphertext {
  encKeyGen: number;
  payload: number[];
}

export const DEMO_ITEMS: DecryptedOverview[] = [
  {
    uuid: 'b2e7b6d0-8c1a-4b2e-9f0a-6c2d3e4f5a6b',
    title: 'Acme Bank',
    subtitle: 'john.doe@acme.example',
    iconKey: 'bank',
    urls: ['https://acme.example'],
    updatedAt: 1720000000000,
  },
  {
    uuid: 'c3f8c7e1-9d2b-4c3f-a0b1-7d3e4f5a6b7c',
    title: 'Github',
    subtitle: 'jdoe@github.com',
    iconKey: 'code',
    urls: ['https://github.com'],
    updatedAt: 1719900000000,
  },
  {
    uuid: 'd4a9d8f2-ae3c-4d40-b1c2-8e4f5a6b7c8d',
    title: 'Work Email',
    subtitle: 'j.doe@work.example',
    iconKey: 'mail',
    urls: ['https://mail.work.example'],
    updatedAt: 1719800000000,
  },
];

/** Per-uuid ciphertext envelope for the stateless autofill path. */
export const DEMO_CIPHERTEXTS: Record<string, DemoItemCiphertext> = {
  'b2e7b6d0-8c1a-4b2e-9f0a-6c2d3e4f5a6b': {
    encKeyGen: 1,
    payload: Array.from({ length: 16 }, (_, i) => (i * 7 + 3) & 0xff),
  },
  'c3f8c7e1-9d2b-4c3f-a0b1-7d3e4f5a6b7c': {
    encKeyGen: 1,
    payload: Array.from({ length: 16 }, (_, i) => (i * 11 + 5) & 0xff),
  },
  'd4a9d8f2-ae3c-4d40-b1c2-8e4f5a6b7c8d': {
    encKeyGen: 1,
    payload: Array.from({ length: 16 }, (_, i) => (i * 13 + 9) & 0xff),
  },
};

/**
 * Derive a fixed 32-byte demo key. Not real KDF (Argon2id lives in the Rust
 * core); it only satisfies the 32-byte contract so the popup can be exercised
 * without a WASM build.
 */
export function makeDemoKey(password: string): Uint8Array {
  const key = new Uint8Array(32);
  const bytes = new TextEncoder().encode(password);
  for (let i = 0; i < bytes.length; i += 1) {
    const idx = i % 32;
    const k = key[idx] ?? 0;
    const b = bytes[i] ?? 0;
    key[idx] = (k + b) & 0xff;
  }
  for (let i = 0; i < 32; i += 1) {
    const k = key[i] ?? 0;
    key[i] = (k ^ (i * 0x2f)) & 0xff;
  }
  return key;
}
