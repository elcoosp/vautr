/**
 * Demo (mock) native bridge + demo SVK derivation for running the mobile app
 * in Expo Go without a compiled uniffi TurboModule. Mirrors `apps/web`'s
 * `mockWasm.ts`: identical contract, in-memory data. When the real native
 * module (`NativeModules.VautrCore`) is present it is used instead (native.ts).
 */

import type {
  DecryptedOverview,
  OpaqueHandle,
  SecureEnclaveBridge,
  VautrNativeBridge,
} from '@vautr/client-sdk/mobile';

const DEMO_ITEMS: DecryptedOverview[] = [
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

/**
 * Derive a fixed 32-byte key for the demo unlock. This is NOT real KDF
 * (Argon2id lives in the Rust core); it only satisfies the 32-byte contract so
 * the app can be exercised without a compiled uniffi module.
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

/** A 32-byte demo SVK used when no real keystore entry exists yet. */
export function demoSvk(): Uint8Array {
  const svk = new Uint8Array(32);
  for (let i = 0; i < 32; i += 1) {
    svk[i] = (i * 0x7d) & 0xff;
  }
  return svk;
}

/**
 * In-memory `VautrNativeBridge` matching the uniffi surface. Reads throw when
 * locked, mirroring the Rust `Result` errors surfaced as `.message`.
 */
export function createDemoNativeBridge(): VautrNativeBridge {
  let unlocked = false;
  let enclave: SecureEnclaveBridge | null = null;

  return {
    async initialize() {
      // No-op: the real module links Rust into the JSI instance + runs migrations.
    },
    async unlock(rawKey, localGen) {
      if (rawKey.length !== 32) {
        throw new Error('raw key must be 32 bytes');
      }
      if (localGen < 1) {
        throw new Error('invalid local key generation');
      }
      unlocked = true;
    },
    async listOverviews() {
      if (!unlocked) throw new Error('vault locked');
      return DEMO_ITEMS;
    },
    async revealSecret() {
      if (!unlocked) throw new Error('vault locked');
      return '1' satisfies OpaqueHandle;
    },
    async releaseSecret() {
      // zeroizes the in-memory secret; no-op in the demo.
    },
    async lock() {
      unlocked = false;
    },
    async sync() {
      if (!unlocked) throw new Error('vault locked');
    },
    async setSecureEnclaveBridge(bridge) {
      enclave = bridge;
    },
  };
}
