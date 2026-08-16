/**
 * Vautr mobile bridge (React Native + Expo, build-env-deploy.md §3.1).
 *
 * Owned by the mobile client (deconflict: this module is additive and does not
 * touch the shared web/extension worker surface in `./client.ts` / `./types.ts`).
 *
 * The Rust core (vautr-ffi, uniffi 0.31) exposes `initialize`, `unlock`,
 * `list_overviews`, `reveal_secret`, `lock`, `sync` plus a `SecureEnclaveBridge`
 * for OS-keystore biometric storage of the SVK. On React Native this native
 * surface is reached through a TurboModule (the `VautrNativeBridge` contract
 * below) that the app supplies. u64 values (opaque handles) are surfaced to JS
 * as strings, matching the web/extension WASM worker bridge (data.md §1 rule 1).
 *
 * Plaintext secrets never cross into JS: `reveal` returns an opaque handle and
 * the secret is only delegated back to the OS clipboard/autofill via the native
 * platform handler.
 */

import type { DecryptedOverview, OpaqueHandle } from './types';

/** Opaque u64 surfaced as a string. Re-exported for mobile consumers. */
/** Re-exported list-item type for mobile consumers. */
export type { DecryptedOverview, OpaqueHandle } from './types';

/**
 * OS-keystore biometrics storage of the 32-byte SVK. The app implements this
 * over `expo-secure-store` (Keychain / Android Keystore) + `expo-local-authentication`
 * and registers it so a biometric unlock never re-prompts for a master password.
 */
export interface SecureEnclaveBridge {
  /** Persist the 32-byte SVK under biometric (or device-passcode) protection. */
  saveSvk(svk: Uint8Array): Promise<void>;
  /** Load the stored SVK. Returns `null` if absent or biometric auth cancelled. */
  loadSvk(): Promise<Uint8Array | null>;
  /** Delete the stored SVK (e.g. on explicit lock or vault removal). */
  deleteSvk(): Promise<void>;
  /** Whether an SVK is currently stored and available for a biometric unlock. */
  hasSvk(): Promise<boolean>;
}

/** Result of a native OPAQUE register/login (VTR-104). */
export interface NativeAuthResult {
  /** Recovery mnemonic (Emergency Kit). Display once on register. */
  recoveryMnemonic: string;
  /** Session token (null after register; present after login). */
  sessionToken: string | null;
}

/**
 * The native (TurboModule) contract the app implements to link the uniffi core
 * into the JSI instance. Every method is async so JSI calls can be run under
 * `useTransition` and never block the UI thread; errors are `Result` surfaced
 * as `.message` via try/catch.
 */
export interface VautrNativeBridge {
  /** Link the Rust core into the JSI instance and run migrations. */
  initialize(dbPath: string): Promise<void>;
  /** Unlock with a raw 32-byte SVK + local key generation. */
  unlock(rawKey: Uint8Array, localGen: number): Promise<void>;
  /** List overviews, most-recently-used first. */
  listOverviews(): Promise<DecryptedOverview[]>;
  /** Reveal a secret behind an opaque handle (u64 as string). */
  revealSecret(uuid: string): Promise<OpaqueHandle>;
  /** Explicitly dispose a handle (zeroizes the in-memory secret). */
  releaseSecret(handle: OpaqueHandle): Promise<void>;
  /**
   * Render a revealed secret in the native overlay view (VTR-048, ADR-003).
   * The native TurboModule delivers the plaintext only to the native overlay
   * component (Kotlin/Swift) via the registered `PlatformActionHandler` — the
   * JS side never receives the secret string, only the opaque handle.
   */
  renderInOverlay(handle: OpaqueHandle): Promise<void>;
  /** Lock the vault (zeroizes keys + in-memory secrets). */
  lock(): Promise<void>;
  /** Run a metadata-first sync. */
  sync(): Promise<void>;
  /** Register the OS-keystore SVK adapter (biometric unlock). */
  setSecureEnclaveBridge(bridge: SecureEnclaveBridge): Promise<void>;

  // ── Native OPAQUE account creation / first unlock (VTR-104) ──────────
  // On RN/Hermes the wasm crypto cannot run, so OPAQUE must execute in the
  // Rust core. These run the full register/login + unlock + sync locally and
  // return the recovery mnemonic (Emergency Kit) and the session token.
  /**
   * Register a brand-new account via the native Rust OPAQUE client. Returns the
   * recovery mnemonic (display once) and `null` for the token (registration
   * does not mint a session — follow with `login`).
   */
  register(serverUrl: string, username: string, password: string): Promise<NativeAuthResult>;
  /**
   * Log in via the native Rust OPAQUE client. Returns the recovery mnemonic and
   * the session token (the core is unlocked + synced on success).
   */
  login(serverUrl: string, username: string, password: string): Promise<NativeAuthResult>;

  // ── Sharing PKI (ADR-007 / sharing-pki.md §6) ────────────────────────
  // These run the zero-knowledge crypto in Rust; plaintext secret bytes are
  // returned only to native callers and never enter the JS heap.
  /** Ensure a sharing keypair exists; returns the public key (base64) to publish. */
  ensureSharingKey(): Promise<string>;
  /** Persist the sharing secret key (base64) loaded from the OS secure store. */
  setSharingSecret(secretB64: string | null): Promise<void>;
  /** Build a 1:1 share bundle for a recipient (returns the JSON bundle). */
  shareItem(
    senderUuid: string,
    recipientUuid: string,
    itemUuid: string,
    recipientPubkeyB64: string,
    plaintext: Uint8Array,
  ): Promise<string>;
  /** Decrypt an incoming 1:1 share (JSON IncomingShare) → plaintext bytes. */
  acceptShare(incomingJson: string): Promise<Uint8Array>;
  /** Create a sharing group; returns the admin's `{ group, secret }` JSON. */
  createGroup(name: string, adminUuid: string): Promise<string>;
  /** Wrap the Group SIK for a new member; returns the wrapped key JSON. */
  addGroupMember(groupJson: string, memberUuid: string, memberPubkeyB64: string): Promise<string>;
  /** Member-side: decapsulate the Group SIK from an inbox entry JSON. */
  unwrapGroupKey(inboxJson: string): Promise<string>;
  /** Encrypt a vault item's payload for a group; returns base64 ciphertext. */
  encryptGroupItem(groupJson: string, itemUuid: string, plaintext: Uint8Array): Promise<string>;
  /** Decrypt a group item's payload (base64) → plaintext bytes. */
  decryptGroupItem(groupJson: string, itemUuid: string, ctB64: string): Promise<Uint8Array>;
}

/** Options to {@link initializeVautrCore}. */
export interface InitializeCoreOptions {
  /** The native TurboModule bridge linked into the JSI instance. */
  native: VautrNativeBridge;
  /** OS-keystore SVK adapter (biometric unlock). */
  secureEnclave: SecureEnclaveBridge;
  /** SQLite vault DB path on device. */
  dbPath: string;
}

/**
 * Promise-based mobile client over the native bridge. Mirrors the web
 * `VautrClient` surface for the Lock/List/Detail screens.
 */
export class MobileVautrClient {
  private readonly native: VautrNativeBridge;

  constructor(native: VautrNativeBridge) {
    this.native = native;
  }

  /** Unlock the vault with a raw 32-byte SVK (biometric path). */
  unlock(rawKey: Uint8Array, localGen: number): Promise<void> {
    return this.native.unlock(rawKey, localGen);
  }

  /** List all overviews, most-recently-used first. */
  listOverviews(): Promise<DecryptedOverview[]> {
    return this.native.listOverviews();
  }

  /** Reveal a secret behind an opaque handle (u64 as string). */
  reveal(uuid: string): Promise<OpaqueHandle> {
    return this.native.revealSecret(uuid);
  }

  /** Explicitly dispose a handle (zeroizes the in-memory secret). */
  release(handle: OpaqueHandle): Promise<void> {
    return this.native.releaseSecret(handle);
  }

  /**
   * Render the revealed secret in the native overlay view (VTR-048, ADR-003).
   * The plaintext is delivered only to the native overlay component via the
   * registered `PlatformActionHandler`; JS keeps only the opaque handle.
   */
  renderInOverlay(handle: OpaqueHandle): Promise<void> {
    return this.native.renderInOverlay(handle);
  }

  /** Lock the vault. */
  lock(): Promise<void> {
    return this.native.lock();
  }

  /** Run a metadata-first sync. */
  sync(): Promise<void> {
    return this.native.sync();
  }

  /**
   * Register a new account via the native Rust OPAQUE client (VTR-104).
   * The Rust core performs registration, persists the KDF salt + wrapped SVK
   * locally, and returns the recovery mnemonic. Follow with {@link login} to
   * mint a session.
   */
  register(serverUrl: string, username: string, password: string): Promise<NativeAuthResult> {
    return this.native.register(serverUrl, username, password);
  }

  /**
   * Log in via the native Rust OPAQUE client (VTR-104). The Rust core performs
   * the OPAQUE login, unlocks the vault locally, connects the sync transport,
   * and returns the recovery mnemonic + session token.
   */
  login(serverUrl: string, username: string, password: string): Promise<NativeAuthResult> {
    return this.native.login(serverUrl, username, password);
  }

  /** The underlying uniffi native bridge (for building a `MobileSharingClient`). */
  getNativeBridge(): VautrNativeBridge {
    return this.native;
  }
}

/**
 * Mobile sharing client (ADR-007 / sharing-pki.md §6). Zero-knowledge: the
 * crypto runs in Rust via the uniffi `MobileClient` (the native bridge), and
 * plaintext secret bytes are only ever returned to native code — never held in
 * the JS heap. This class layers the HTTP relay (publish/fetch public key,
 * upload/download share + group bundles) on top of those primitives.
 *
 * Native-gated: construct only when `getMobileClient()` is non-null (the uniffi
 * core is linked). On an HTTP-only build, sharing lives in the web/extension
 * `VautrMlpClient` instead.
 *
 * `SharingRelay` is the subset of the mobile API client the sharing flow needs;
 * the app's `MobileApiClient` structurally satisfies it.
 */
export interface SharingRelay {
  publishSharingPublicKey(userId: string, publicKeyB64: string): Promise<void>;
  getSharingPublicKey(userId: string): Promise<string | null>;
  createShare(input: {
    item_uuid: string;
    recipient_uuid: string;
    wrapped_sik: string;
    ephemeral_public_key: string;
  }): Promise<void>;
  uploadSharePayload(itemUuid: string, payloadB64: string): Promise<void>;
  listShareInbox(): Promise<
    Array<{
      share_id: string;
      sender_uuid: string;
      item_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
      encrypted_payload: string;
    }>
  >;
  createGroup(input: {
    name: string;
  }): Promise<{ group_id: string; name: string; admin_uuid: string }>;
  addGroupMember(
    groupId: string,
    input: { member_uuid: string; wrapped_sik: string; ephemeral_public_key: string },
  ): Promise<void>;
  listGroupItems(groupId: string): Promise<Array<{ item_uuid: string; payload: string }>>;
  addGroupItem(groupId: string, input: { item_uuid: string; payload: string }): Promise<void>;
  /** List the groups the caller belongs to, with each member's wrapped Group SIK. */
  groupInbox(): Promise<
    Array<{
      group_id: string;
      name: string;
      admin_uuid: string;
      member_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
    }>
  >;
}

export class MobileSharingClient {
  private readonly native: VautrNativeBridge;
  private readonly api: SharingRelay;
  /**
   * Local cache of `{ group, secret }` JSON keyed by group_id (the Group SIK).
   * Mirrors the web client's `store.groupKeys`. Populated by `createGroup`
   * (admin) and `acceptGroup` (member). The Group SIK is legitimately held in
   * JS for sharing; this is not a `read_secret`/reveal path.
   */
  private readonly groupKeys = new Map<string, string>();

  constructor(native: VautrNativeBridge, api: SharingRelay) {
    this.native = native;
    this.api = api;
  }

  /** Ensure our sharing keypair exists and publish its public key to the PKI. */
  async publishMyPublicKey(userId: string): Promise<void> {
    const pub = await this.native.ensureSharingKey();
    await this.api.publishSharingPublicKey(userId, pub);
  }

  /** 1:1 share: encrypt for `recipientUserId`, then relay the bundle. */
  async shareItem(
    senderUuid: string,
    recipientUserId: string,
    itemUuid: string,
    plaintext: Uint8Array,
  ): Promise<void> {
    const pub = await this.api.getSharingPublicKey(recipientUserId);
    if (!pub) throw new Error(`recipient ${recipientUserId} has no sharing public key`);
    const bundleJson = await this.native.shareItem(
      senderUuid,
      recipientUserId,
      itemUuid,
      pub,
      plaintext,
    );
    const b = JSON.parse(bundleJson) as {
      item_uuid: string;
      recipient_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
      encrypted_payload: string;
    };
    await this.api.createShare({
      item_uuid: b.item_uuid,
      recipient_uuid: b.recipient_uuid,
      wrapped_sik: b.wrapped_sik,
      ephemeral_public_key: b.ephemeral_public_key,
    });
    await this.api.uploadSharePayload(b.item_uuid, b.encrypted_payload);
  }

  /** Pull the inbox and decrypt each waiting 1:1 share to plaintext. */
  async collectShares(): Promise<Array<{ itemUuid: string; plaintext: Uint8Array }>> {
    const inbox = await this.api.listShareInbox();
    const out: Array<{ itemUuid: string; plaintext: Uint8Array }> = [];
    for (const s of inbox) {
      const incoming = JSON.stringify({
        share_id: s.share_id,
        sender_uuid: s.sender_uuid,
        item_uuid: s.item_uuid,
        wrapped_sik: s.wrapped_sik,
        ephemeral_public_key: s.ephemeral_public_key,
        encrypted_payload: s.encrypted_payload,
      });
      const plaintext = await this.native.acceptShare(incoming);
      out.push({ itemUuid: s.item_uuid, plaintext });
    }
    return out;
  }

  /** Create a group (admin) and persist its Group SIK locally. */
  async createGroup(name: string, adminUuid: string): Promise<string> {
    const groupJson = await this.native.createGroup(name, adminUuid);
    const g = JSON.parse(groupJson) as { group_id: string };
    this.groupKeys.set(g.group_id, groupJson);
    await this.api.createGroup({ name });
    return groupJson;
  }

  /** List the groups the caller belongs to (admin + member invites). */
  async getGroupInbox(): Promise<
    Array<{
      group_id: string;
      name: string;
      admin_uuid: string;
      member_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
    }>
  > {
    return this.api.groupInbox();
  }

  /**
   * Return the persisted `{ group, secret }` JSON for a group this client is an
   * admin or member of, or null if not cached locally. Used to reconstruct the
   * group key for adding members / sharing / decrypting items.
   */
  getGroupKey(groupId: string): string | null {
    return this.groupKeys.get(groupId) ?? null;
  }

  /** Add a member to a group: wrap the Group SIK for them and upload it. */
  async addGroupMember(groupJson: string, memberUserId: string): Promise<void> {
    const pub = await this.api.getSharingPublicKey(memberUserId);
    if (!pub) throw new Error(`member ${memberUserId} has no sharing public key`);
    const wrappedJson = await this.native.addGroupMember(groupJson, memberUserId, pub);
    const w = JSON.parse(wrappedJson) as {
      group_id: string;
      member_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
    };
    await this.api.addGroupMember(w.group_id, {
      member_uuid: w.member_uuid,
      wrapped_sik: w.wrapped_sik,
      ephemeral_public_key: w.ephemeral_public_key,
    });
  }

  /** Decapsulate + persist the Group SIK from an inbox entry (member side). */
  async acceptGroup(groupInboxEntryJson: string): Promise<string> {
    const memberKeyJson = await this.native.unwrapGroupKey(groupInboxEntryJson);
    const g = JSON.parse(memberKeyJson) as { group: { group_id: string } };
    this.groupKeys.set(g.group.group_id, memberKeyJson);
    return memberKeyJson;
  }

  /** Encrypt a vault item's plaintext for a group and upload it. */
  async shareToGroup(
    groupJson: string,
    groupId: string,
    itemUuid: string,
    plaintext: Uint8Array,
  ): Promise<void> {
    const ctB64 = await this.native.encryptGroupItem(groupJson, itemUuid, plaintext);
    await this.api.addGroupItem(groupId, { item_uuid: itemUuid, payload: ctB64 });
  }

  /** List + decrypt a group's items (member/admin). */
  async listGroupItems(
    groupJson: string,
    groupId: string,
  ): Promise<Array<{ itemUuid: string; plaintext: Uint8Array }>> {
    const items = await this.api.listGroupItems(groupId);
    const out: Array<{ itemUuid: string; plaintext: Uint8Array }> = [];
    for (const it of items) {
      const plaintext = await this.native.decryptGroupItem(groupJson, it.item_uuid, it.payload);
      out.push({ itemUuid: it.item_uuid, plaintext });
    }
    return out;
  }
}

let activeClient: MobileVautrClient | null = null;

/**
 * Boot the mobile core (skill matrix boot pattern): links the Rust structures
 * into the JSI instance via `native.initialize`, registers the `SecureEnclaveBridge`,
 * and returns a ready `MobileVautrClient`. Await this on mount; keep it wrapped in
 * a `useTransition` so the JSI allocation never blocks the UI thread.
 */
export async function initializeVautrCore(
  options: InitializeCoreOptions,
): Promise<MobileVautrClient> {
  const { native, secureEnclave, dbPath } = options;
  await native.initialize(dbPath);
  await native.setSecureEnclaveBridge(secureEnclave);
  activeClient = new MobileVautrClient(native);
  return activeClient;
}

/** The active mobile client, or `null` before {@link initializeVautrCore}. */
export function getMobileClient(): MobileVautrClient | null {
  return activeClient;
}
