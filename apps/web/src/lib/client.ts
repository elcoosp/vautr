import { createClipboardHandler, VautrClient } from '@vautr/client-sdk';
import type { DecryptedOverview } from '@vautr/ui-logic';
import { attachClientToEventBus, vaultEventBus } from '@vautr/ui-logic';

let client: VautrClient | null = null;

/**
 * Lazily create the single WASM worker client and wire its event stream into
 * the shared ui-logic event bus + store (client.md §5: core pushes diffs).
 */
export function getClient(): VautrClient {
  if (!client) {
    const instance = new VautrClient();
    instance.setClipboardHandler(createClipboardHandler());
    attachClientToEventBus((listener) => instance.subscribe(listener), vaultEventBus);
    client = instance;
  }
  return client;
}

export function disposeClient(): void {
  if (client) {
    client.close();
    client = null;
  }
}

/** Demo vault contents, pushed as `OverviewUpserted` diffs (ui-state-charts §6). */
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
  {
    uuid: 'e5baa903-bf4d-4e51-c2d3-9f5a6b7c8d9e',
    title: 'Streaming',
    subtitle: 'family@stream.example',
    iconKey: 'play',
    urls: ['https://stream.example'],
    updatedAt: 1719700000000,
  },
];

/** Push the demo overviews into the store via the event bus. */
export function seedDemoData(): void {
  for (const item of DEMO_ITEMS) {
    vaultEventBus.emit({ type: 'OverviewUpserted', overview: item });
  }
}

/**
 * Derive a fixed 32-byte key for the demo unlock. This is NOT real KDF
 * (Argon2id lives in the Rust core); it only satisfies the mock's 32-byte
 * contract so the SPA can be exercised without a WASM build.
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
