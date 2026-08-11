import { VautrWebClient } from '@vautr/client-sdk/real';
import type { CoreAction, OpaqueHandle } from '@vautr/client-sdk';
import type { DecryptedOverview } from '@vautr/ui-logic';
import { attachStoreToEventBus, vaultStore, vaultEventBus } from '@vautr/ui-logic';

let client: VautrWebClient | null = null;
let storeAttached = false;

/**
 * Lazy singleton for the real Vautr client, wired into the shared ui-logic
 * event bus + store (client.md §5: the client pushes reactive diffs). Crypto
 * goes through the wasm crypto-only module (ADR-005).
 */
export function getClient(): VautrWebClient {
  if (!client) {
    const instance = new VautrWebClient();
    instance.setClipboardHandler((action, secret) => {
      if (action.type === 'CopyToClipboard') {
        // The secret is a local variable; write it and drop the reference.
        void navigator.clipboard.writeText(secret);
      }
    });
    instance.subscribe((update) => vaultEventBus.emit(update));
    if (!storeAttached) {
      attachStoreToEventBus(vaultEventBus);
      storeAttached = true;
    }
    client = instance;
  }
  return client;
}

export function disposeClient(): void {
  client = null;
}

/** Register a brand-new account on the live server (OPAQUE, api.md §3.1). */
export async function register(username: string, password: string): Promise<void> {
  await getClient().register(username, password);
}

/** Log in (OPAQUE, api.md §3.2) → recover SVK → unlock the store → sync. */
export async function login(username: string, password: string): Promise<void> {
  const instance = getClient();
  await instance.login(username, password);
  localStorage.setItem('vautr:username', username);
  vaultStore.getState().unlock();
  await instance.sync();
}

/** Lock the client (clear in-memory keys) and the store. */
export async function lock(): Promise<void> {
  await getClient().lock();
  vaultStore.getState().lock();
}

/** Trigger a metadata-first sync. */
export async function sync(): Promise<void> {
  await getClient().sync();
}

/** A new-item input for the add flow. */
export interface NewItemInput {
  title: string;
  username: string;
  password: string;
  url: string;
}

/** Create + encrypt a new item locally (pushed on the next sync). */
export async function addItem(input: NewItemInput): Promise<DecryptedOverview> {
  return getClient().addItem(input);
}

/** Reveal a secret behind an opaque handle. */
export function reveal(uuid: string): Promise<OpaqueHandle> {
  return getClient().reveal(uuid);
}

/** Explicitly dispose an opaque secret handle. */
export function release(handle: OpaqueHandle): Promise<void> {
  return getClient().release(handle);
}

/** Delegate copy/autofill to the platform clipboard handler. */
export function performAction(action: CoreAction): Promise<void> {
  return getClient().performAction(action);
}
