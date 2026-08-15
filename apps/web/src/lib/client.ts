import type { CoreAction, OpaqueHandle } from '@vautr/client-sdk';
import { VautrMlpClient } from '@vautr/client-sdk';
import { VautrWebClient } from '@vautr/client-sdk/real';
import type { DecryptedOverview } from '@vautr/ui-logic';
import { attachStoreToEventBus, vaultEventBus, vaultStore } from '@vautr/ui-logic';

let client: VautrWebClient | null = null;
let mlp: VautrMlpClient | null = null;
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
    // Server-backed quarantine reaper (VTR-069): proactive tombstone/recovery push.
    instance.subscribeVaultEvents();
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
export async function register(
  username: string,
  password: string,
): Promise<{ recoveryMnemonic: string }> {
  return getClient().register(username, password);
}

/** Log in (OPAQUE, api.md §3.2) → recover SVK → unlock the store → sync. */
export async function login(username: string, password: string): Promise<void> {
  const instance = getClient();
  await instance.login(username, password);
  localStorage.setItem('vautr:username', username);
  vaultStore.getState().unlock();
  await instance.sync();
  window.dispatchEvent(new CustomEvent('vautr:auth-change'));
}

/** Lock the client (clear in-memory keys) and the store. */
export async function lock(): Promise<void> {
  await getClient().lock();
  vaultStore.getState().lock();
}

/** Log out: forget the session token and lock the vault. */
export async function logout(): Promise<void> {
  await getClient().forget();
  vaultStore.getState().lock();
  localStorage.removeItem('vautr:username');
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

/** The MLP client (sharing PKI, projects/secrets) sharing this session. */
export function getMlp(): VautrMlpClient {
  if (!mlp) {
    mlp = new VautrMlpClient(getClient().getApi());
  }
  return mlp;
}

/** Emergency Kit (Recovery Key) for display/download. ZK: opened locally. */
export function getEmergencyKit(): { mnemonic: string; words: string[] } | null {
  return getClient().getEmergencyKit();
}

// --- sharing (ADR-007) ---
export const getItemPlaintext = (uuid: string): Promise<Uint8Array> =>
  getClient().getItemPlaintext(uuid);
export const ensureSharingKey = (): Promise<string> => getClient().ensureSharingKey(getMlp());
export const shareItem = (
  itemUuid: string,
  recipientUserId: string,
  plaintext: Uint8Array,
): Promise<void> => getClient().shareItem(getMlp(), itemUuid, recipientUserId, plaintext);
export const getShareInbox = () => getClient().getShareInbox(getMlp());
export const acceptShare = (incoming: unknown): Promise<Uint8Array> =>
  getClient().acceptShare(getMlp(), incoming);
export const revokeShare = (itemUuid: string): Promise<void> =>
  getClient().revokeShare(getMlp(), itemUuid);

// --- group sharing (sharing-pki.md §6) ---
export const createGroup = (name: string): Promise<string> =>
  getClient().createGroup(getMlp(), name);
export const addGroupMember = (groupJson: string, memberUserId: string): Promise<void> =>
  getClient().addGroupMember(getMlp(), groupJson, memberUserId);
export const getGroupInbox = () => getClient().getGroupInbox(getMlp());
export const unwrapGroupKey = (inbox: unknown): Promise<string> =>
  getClient().unwrapGroupKey(getMlp(), inbox);
export const shareItemToGroup = (
  groupJson: string,
  itemUuid: string,
  plaintext: Uint8Array,
): Promise<void> => getClient().shareItemToGroup(getMlp(), groupJson, itemUuid, plaintext);
export const getGroupItems = (groupId: string) => getClient().getGroupItems(getMlp(), groupId);
export const acceptGroupItem = (
  groupJson: string,
  itemUuid: string,
  payloadB64: string,
): Promise<Uint8Array> => getClient().acceptGroupItem(groupJson, itemUuid, payloadB64);
export const revokeGroupItem = (groupId: string, itemUuid: string): Promise<void> =>
  getClient().revokeGroupItem(getMlp(), groupId, itemUuid);
export const getGroupKey = (groupId: string): Promise<string | null> =>
  getClient().getGroupKey(getMlp(), groupId);
