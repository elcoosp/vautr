/**
 * The real browser client (api.md, data.md, client.md).
 *
 * Drives the live Vautr server over OPAQUE auth, keeps an encrypted mirror in
 * IndexedDB, and syncs item metadata + payloads. All crypto goes through the
 * `AsyncCryptoAdapter` (the wasm crypto-only module, ADR-005). Secrets are only
 * ever held in a module-local handle table keyed by an opaque u64-as-string
 * handle and are zeroized on release/reap; they never enter React state.
 *
 * This is exported from the `@vautr/client-sdk/real` entry, NOT the SDK root,
 * so browser-extension consumers that resolve the SDK against their own
 * `vautr-wasm` shim are unaffected.
 */

import { ApiClient, fromBase64, toBase64 } from './api';
import { AsyncCryptoAdapter } from './crypto';
import type { VautrMlpClient } from './mlp';
import { IndexedDbStore } from './storage';
import type {
  ClipboardHandler,
  ConflictChoice,
  ConflictEvent,
  CoreAction,
  DecryptedOverview,
  OpaqueHandle,
  VaultStateUpdate,
} from './types';

/** Server item metadata row from `GET /sync/pull` (api.md §4). */
interface SyncPullItem {
  uuid: string;
  version: number;
  enc_key_gen: number;
  deleted_date: number | null;
}

interface SyncPullResponse {
  new_cursor: number;
  has_more: boolean;
  min_enc_key_gen: number;
  items: SyncPullItem[];
}

interface PullPayloadResult {
  uuid: string;
  status: string;
  version?: number;
  enc_key_gen?: number;
  deleted_date?: number | null;
  payload?: string | null;
}

interface PushBatchItem {
  uuid: string;
  target_version: number;
  enc_key_gen: number;
  payload: string | null;
  deleted_date: number | null;
}

interface PushBatchResult {
  uuid: string;
  status: string;
  version?: number;
  enc_key_gen?: number;
  updated_at?: number;
}

interface AccountStatus {
  min_enc_key_gen: number;
  svk_ciphertext_blob: string;
}

/** Plaintext content of an item payload (overview + secret, data.md §3.6). */
interface ItemPlaintext {
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];
  password: string;
}

/** Input to create a new vault item. */
export interface AddItemInput {
  title: string;
  username: string;
  password: string;
  url: string;
}

/** Options for the browser client. */
export interface VautrWebClientOptions {
  baseUrl?: string;
  crypto?: AsyncCryptoAdapter;
  store?: IndexedDbStore;
}

/** A revealed secret handle with its idle timer (opaque-handle pattern). */
interface ActiveSecret {
  secret: string;
  lastAccess: number;
}

const HANDLE_TTL_MS = 60_000;

export class VautrWebClient {
  private readonly api: ApiClient;
  private readonly crypto: AsyncCryptoAdapter;
  private readonly store: IndexedDbStore;

  private listeners = new Set<(update: VaultStateUpdate) => void>();
  private clipboardHandler: ClipboardHandler | null = null;

  // In-memory session material (cleared on lock).
  private svk: Uint8Array | null = null;
  private dek: Uint8Array | null = null;
  private localKeyGen = 1;

  // Opaque handle table (plaintext never in React state).
  private handles = new Map<OpaqueHandle, ActiveSecret>();
  private nextHandle = 1;

  constructor(options: VautrWebClientOptions = {}) {
    this.api = new ApiClient({ baseUrl: options.baseUrl });
    this.crypto = options.crypto ?? new AsyncCryptoAdapter();
    this.store = options.store ?? new IndexedDbStore();
  }

  // -------------------------------------------------------------------------
  // Event stream / platform adapter
  // -------------------------------------------------------------------------

  subscribe(listener: (update: VaultStateUpdate) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  setClipboardHandler(handler: ClipboardHandler): void {
    this.clipboardHandler = handler;
  }

  private emit(update: VaultStateUpdate): void {
    for (const listener of this.listeners) {
      listener(update);
    }
  }

  /**
   * Subscribe to the server-backed quarantine reaper event stream (VTR-069):
   * `GET /events` SSE. The server pushes `item_permanently_deleted` /
   * `item_recovered` (tombstone / recovery) events so web/extension clients
   * drop stale items and re-sync without waiting for the next pull.
   *
   * Returns an unsubscribe function that closes the stream + stops reconnects.
   */
  subscribeVaultEvents(): () => void {
    let closed = false;
    let abort: AbortController | null = null;

    const connect = async (): Promise<void> => {
      if (closed) return;
      const token = this.api.getToken();
      if (!token) {
        // Not logged in yet; retry shortly.
        if (!closed) setTimeout(() => void connect(), 2000);
        return;
      }
      abort = new AbortController();
      try {
        const res = await fetch(`${this.api.getBaseUrl()}/events`, {
          headers: { Authorization: `Bearer ${token}` },
          signal: abort.signal,
        });
        if (!res.ok || !res.body) {
          if (!closed) setTimeout(() => void connect(), 2000);
          return;
        }
        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const parts = buffer.split('\n\n');
          const rest = parts.pop() ?? '';
          const frames = parts;
          buffer = rest;
          for (const frame of frames) {
            for (const line of frame.split('\n')) {
              const trimmed = line.trim();
              if (!trimmed.startsWith('data:')) continue;
              const payload = trimmed.slice('data:'.length).trim();
              if (!payload || payload === '[DONE]') continue;
              try {
                const ev = JSON.parse(payload) as {
                  type: string;
                  uuid?: string;
                };
                this.onServerEvent(ev);
              } catch {
                // Ignore malformed frames (e.g. comment lines, keep-alive).
              }
            }
          }
        }
      } catch (err) {
        if (closed || (err instanceof DOMException && err.name === 'AbortError')) return;
      } finally {
        abort = null;
        if (!closed) setTimeout(() => void connect(), 2000);
      }
    };

    void connect();
    return () => {
      closed = true;
      abort?.abort();
    };
  }

  /** Handle a single server reaper event, mapping it to a local state update. */
  private async onServerEvent(ev: { type: string; uuid?: string }): Promise<void> {
    if (ev.type === 'item_permanently_deleted' && ev.uuid) {
      await this.store.deleteItem(ev.uuid);
      this.emit({ type: 'OverviewDeleted', uuid: ev.uuid });
    } else if (ev.type === 'item_recovered' && ev.uuid && this.isUnlocked()) {
      // A tombstoned item was un-tombstoned; re-sync to fetch the new payload.
      try {
        this.emit({ type: 'SyncStarted' });
        await this.sync();
      } catch {
        // sync() emits its own SyncFailed on error.
      }
    }
  }

  isUnlocked(): boolean {
    return this.svk !== null && this.dek !== null;
  }

  /**
   * The underlying HTTP transport (shares the session bearer token). Used by
   * the MLP client (`VautrMlpClient`) so one login drives both the vault sync
   * and the Projects/Secrets/MFA surfaces.
   */
  getApi(): ApiClient {
    return this.api;
  }

  // -------------------------------------------------------------------------
  // Auth: register + login (OPAQUE, api.md §3)
  // -------------------------------------------------------------------------

  /** Register a brand-new account. Persists the KDF salt; caller then logs in. */
  async register(username: string, password: string): Promise<void> {
    await this.crypto.ready();

    // Local key material (crypto.md §2).
    const kdfSalt = this.crypto.generateKdfSalt();
    const mk = this.crypto.deriveMasterKey(password, kdfSalt);
    const kek = this.crypto.deriveKek(mk);
    const svk = this.crypto.generateSvk();
    const svkWrapped = this.crypto.wrapSvk(svk, kek);
    const mnemonic = this.crypto.generateRecoveryMnemonic();
    const svkRkWrapped = this.crypto.wrapSvkWithRk(svk, mnemonic);

    // OPAQUE registration (api.md §3.1).
    const start = this.crypto.opaqueRegisterStart(password);
    const startResp = await this.api.request<{ registration_response: string }>(
      'POST',
      '/auth/register/start',
      { username, registration_start: toBase64(start.message) },
    );
    const upload = this.crypto.opaqueRegisterFinish(
      start.state,
      fromBase64(startResp.registration_response),
      password,
      username,
    );
    await this.api.request('POST', '/auth/register/finish', {
      username,
      registration_finish: toBase64(upload),
      server_public_key: toBase64(new Uint8Array(32)),
      kdf_salt: toBase64(kdfSalt),
      svk_ciphertext_blob: toBase64(svkWrapped),
      svk_ciphertext_blob_rk: toBase64(svkRkWrapped),
    });

    // Persist the account salt so a later login can re-derive the MK.
    await this.store.setState({
      username,
      kdfSalt: toBase64(kdfSalt),
      svk: null,
      sessionToken: null,
    });
  }

  /** OPAQUE login → bearer token → recover SVK → unlock. */
  async login(username: string, password: string): Promise<void> {
    await this.crypto.ready();

    const start = this.crypto.opaqueLoginStart(password);
    const startResp = await this.api.request<{ login_response: string }>(
      'POST',
      '/auth/login/start',
      { username, login_start: toBase64(start.message) },
    );
    const finish = this.crypto.opaqueLoginFinish(
      start.state,
      fromBase64(startResp.login_response),
      password,
      username,
    );
    const finishResp = await this.api.request<{ session_token: string }>(
      'POST',
      '/auth/login/finish',
      { username, login_finish: toBase64(finish.upload) },
    );

    const token = finishResp.session_token;
    this.api.setToken(token);

    // Recover the SVK (api.md §5) and derive the DEK.
    const status = await this.api.request<AccountStatus>('GET', '/account/status');
    const state = await this.store.getState();
    const kdfSalt = state.kdfSalt ? fromBase64(state.kdfSalt) : null;
    if (!kdfSalt) {
      throw new Error('no stored KDF salt for this account; register or recover first');
    }
    const mk = this.crypto.deriveMasterKey(password, kdfSalt);
    const kek = this.crypto.deriveKek(mk);
    const svk = this.crypto.unwrapSvk(fromBase64(status.svk_ciphertext_blob), kek);
    const dek = this.crypto.deriveDek(svk);

    this.svk = svk;
    this.dek = dek;
    this.localKeyGen = Math.max(1, state.localKeyGen);
    await this.store.setState({
      username,
      sessionToken: token,
      svk,
      localKeyGen: this.localKeyGen,
      minEncKeyGen: status.min_enc_key_gen,
    });

    this.emit({ type: 'SyncCompleted' });
  }

  /** Lock: clear in-memory key material + handles; keep the session token. */
  async lock(): Promise<void> {
    this.svk = null;
    this.dek = null;
    this.handles.clear();
    this.emit({ type: 'VaultLocked' });
  }

  /** Forget the session entirely (logout): drop the stored token. */
  async forget(): Promise<void> {
    await this.lock();
    this.api.setToken(null);
    await this.store.setState({ sessionToken: null, svk: null });
  }

  /**
   * Rotate the vault encryption key (server `POST /account/rotate-key`).
   *
   * Key rotation re-wraps the in-memory SVK under a freshly-derived KEK (from the
   * master password) at the next `min_enc_key_gen`. The server stores the new
   * MP-wrapped `svk` blob and advances the epoch gate; clients at a lower
   * generation are forced read-only until they re-derive (data.md §5). Mirrors
   * the desktop orchestrator's `rotate_key` but performed explicitly here because
   * the server-backed client does not keep the master password in memory.
   */
  async rotateKey(password: string): Promise<number> {
    if (!this.svk) {
      throw new Error('vault is locked');
    }
    await this.crypto.ready();

    const status = await this.api.request<AccountStatus>('GET', '/account/status');
    const state = await this.store.getState();
    const kdfSalt = state.kdfSalt ? fromBase64(state.kdfSalt) : null;
    if (!kdfSalt) {
      throw new Error('no stored KDF salt for this account; register or recover first');
    }

    const mk = this.crypto.deriveMasterKey(password, kdfSalt);
    const kek = this.crypto.deriveKek(mk);
    const newSvkWrapped = this.crypto.wrapSvk(this.svk, kek);

    const newGen = status.min_enc_key_gen + 1;
    const resp = await this.api.request<{ status: string; min_enc_key_gen: number }>(
      'POST',
      '/account/rotate-key',
      { new_min_enc_key_gen: newGen, new_svk_ciphertext_blob: toBase64(newSvkWrapped) },
    );

    this.localKeyGen = resp.min_enc_key_gen;
    await this.store.setState({
      localKeyGen: this.localKeyGen,
      minEncKeyGen: resp.min_enc_key_gen,
    });
    return resp.min_enc_key_gen;
  }

  /** Current local encryption-key generation (for the rotation UI). */
  getKeyGen(): number {
    return this.localKeyGen;
  }

  // -------------------------------------------------------------------------
  // Sharing (ADR-007) — zero-knowledge 1:1 item share relay
  // -------------------------------------------------------------------------

  /**
   * Ensure this client has a sharing keypair. Generates + publishes one on
   * first use, then persists the secret key locally for inbox unwrap. Returns
   * the persisted sharing secret key (base64).
   */
  async ensureSharingKey(mlp: VautrMlpClient): Promise<string> {
    const state = await this.store.getState();
    if (state.sharingSecretKey) {
      return state.sharingSecretKey;
    }
    const kpJson = this.crypto.generateSharingKeypair();
    const kp = JSON.parse(kpJson) as { public: string; secret: string };
    if (state.username) {
      await mlp.publishSharingPublicKey(state.username, kp.public);
    }
    await this.store.setState({ sharingSecretKey: kp.secret });
    return kp.secret;
  }

  /** Share an item's plaintext with a recipient (by user id). */
  async shareItem(
    mlp: VautrMlpClient,
    itemUuid: string,
    recipientUserId: string,
    plaintext: Uint8Array,
  ): Promise<void> {
    if (!this.svk) {
      throw new Error('vault is locked');
    }
    const state = await this.store.getState();
    const sender = state.username;
    if (!sender) {
      throw new Error('no logged-in user');
    }
    const secret = await this.ensureSharingKey(mlp);
    void secret;

    const recipient = await mlp.getSharingPublicKey(recipientUserId);
    const bundleJson = this.crypto.shareItem(
      sender,
      recipientUserId,
      itemUuid,
      recipient.public_key,
      plaintext,
    );
    const bundle = JSON.parse(bundleJson) as {
      item_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
      encrypted_payload: string;
    };
    await mlp.createShare({
      item_uuid: bundle.item_uuid,
      recipient_uuid: recipientUserId,
      wrapped_sik: bundle.wrapped_sik,
      ephemeral_public_key: bundle.ephemeral_public_key,
    });
    await mlp.uploadSharePayload(bundle.item_uuid, bundle.encrypted_payload);
  }

  /** List shares waiting in our inbox. */
  async getShareInbox(mlp: VautrMlpClient) {
    return mlp.listShareInbox();
  }

  /**
   * Decrypt a received share to its plaintext (Uint8Array). The sharing secret
   * key is loaded from local storage and never retained.
   */
  async acceptShare(_mlp: VautrMlpClient, incoming: unknown): Promise<Uint8Array> {
    const state = await this.store.getState();
    if (!state.sharingSecretKey) {
      throw new Error('no sharing key; cannot decrypt share');
    }
    return this.crypto.acceptShare(JSON.stringify(incoming), state.sharingSecretKey);
  }

  /** Revoke a share we own. */
  async revokeShare(mlp: VautrMlpClient, itemUuid: string): Promise<void> {
    await mlp.revokeShare(itemUuid);
  }

  // -------------------------------------------------------------------------
  // Group sharing (sharing-pki.md §6) — zero-knowledge 1:N item share relay
  // -------------------------------------------------------------------------

  /**
   * Create a sharing group and persist its Group SIK locally. Returns the
   * `{ group, secret }` JSON (the admin's ShareGroupKey) for immediate use.
   */
  async createGroup(mlp: VautrMlpClient, name: string): Promise<string> {
    const state = await this.store.getState();
    const sender = state.username;
    if (!sender) {
      throw new Error('no logged-in user');
    }
    const groupJson = this.crypto.createSharingGroup(name, sender);
    const group = JSON.parse(groupJson) as { group: { group_id: string }; secret: string };
    const created = await mlp.createGroup({ name });
    // Persist the Group SIK keyed by the server-assigned group_id.
    const groupKeys = { ...state.groupKeys, [created.group_id]: group.secret };
    await this.store.setState({ groupKeys });
    return JSON.stringify({
      group: { ...group.group, group_id: created.group_id },
      secret: group.secret,
    });
  }

  /**
   * Add a member to a group: wrap the Group SIK for them and upload the wrap.
   * `groupJson` is the admin's persisted `{ group, secret }` (from createGroup or
   * an admin's stored group key).
   */
  async addGroupMember(
    mlp: VautrMlpClient,
    groupJson: string,
    memberUserId: string,
  ): Promise<void> {
    const state = await this.store.getState();
    const sender = state.username;
    if (!sender) {
      throw new Error('no logged-in user');
    }
    const recipient = await mlp.getSharingPublicKey(memberUserId);
    const wrappedJson = this.crypto.addGroupMember(groupJson, memberUserId, recipient.public_key);
    const wrapped = JSON.parse(wrappedJson) as {
      group_id: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
    };
    await mlp.addGroupMember(wrapped.group_id, {
      member_uuid: memberUserId,
      wrapped_sik: wrapped.wrapped_sik,
      ephemeral_public_key: wrapped.ephemeral_public_key,
    });
  }

  /** List the groups the caller belongs to, with each member's wrapped Group SIK. */
  async getGroupInbox(mlp: VautrMlpClient) {
    return mlp.groupInbox();
  }

  /**
   * Decapsulate + persist the Group SIK for a group from an inbox entry, using
   * the caller's sharing secret. Returns the member's `{ group, secret }`.
   */
  async unwrapGroupKey(_mlp: VautrMlpClient, inbox: unknown): Promise<string> {
    const state = await this.store.getState();
    if (!state.sharingSecretKey) {
      throw new Error('no sharing key; cannot unwrap group key');
    }
    const memberKeyJson = this.crypto.unwrapGroupKey(JSON.stringify(inbox), state.sharingSecretKey);
    const memberKey = JSON.parse(memberKeyJson) as { group: { group_id: string }; secret: string };
    const groupKeys = { ...state.groupKeys, [memberKey.group.group_id]: memberKey.secret };
    await this.store.setState({ groupKeys });
    return memberKeyJson;
  }

  /** Share an item (plaintext) into a group: encrypt once under the Group SIK. */
  async shareItemToGroup(
    mlp: VautrMlpClient,
    groupJson: string,
    itemUuid: string,
    plaintext: Uint8Array,
  ): Promise<void> {
    const ctB64 = this.crypto.encryptGroupItem(groupJson, itemUuid, plaintext);
    await mlp.addGroupItem(JSON.parse(groupJson).group.group_id, {
      item_uuid: itemUuid,
      payload: ctB64,
    });
  }

  /** List a group's shared items (item_uuid + Group-SIK-encrypted payload). */
  async getGroupItems(mlp: VautrMlpClient, groupId: string) {
    return mlp.listGroupItems(groupId);
  }

  /** Decrypt a group item to plaintext. `groupJson` is the member's key. */
  async acceptGroupItem(
    groupJson: string,
    itemUuid: string,
    payloadB64: string,
  ): Promise<Uint8Array> {
    return this.crypto.decryptGroupItem(groupJson, itemUuid, payloadB64);
  }

  /** Remove an item from a group (admin). */
  async revokeGroupItem(mlp: VautrMlpClient, groupId: string, itemUuid: string): Promise<void> {
    await mlp.deleteGroupItem(groupId, itemUuid);
  }

  /** Rotate the Group SIK and re-wrap remaining members (admin). */
  async rotateGroup(
    mlp: VautrMlpClient,
    groupJson: string,
    members: Array<{ userId: string; publicKeyB64: string }>,
  ): Promise<void> {
    const wrapped = members.map((m) => {
      const w = JSON.parse(this.crypto.addGroupMember(groupJson, m.userId, m.publicKeyB64)) as {
        group_id: string;
        wrapped_sik: string;
        ephemeral_public_key: string;
      };
      return {
        recipient_user_id: m.userId,
        wrapped_sik: w.wrapped_sik,
        ephemeral_public_key: w.ephemeral_public_key,
      };
    });
    await mlp.rotateGroup(JSON.parse(groupJson).group.group_id, { wrapped_keys: wrapped });
  }

  /** Remove a member from a group (admin). */
  async removeGroupMember(mlp: VautrMlpClient, groupId: string, memberUuid: string): Promise<void> {
    await mlp.removeGroupMember(groupId, memberUuid);
  }

  /**
   * Return the persisted `{ group, secret }` JSON for a group this client is an
   * admin or member of (from local storage), or null if not available. Used to
   * reconstruct the ShareGroupKey for adding members / decrypting items. The
   * group's display metadata (name/admin_uuid) is pulled from the server inbox.
   */
  async getGroupKey(mlp: VautrMlpClient, groupId: string): Promise<string | null> {
    const state = await this.store.getState();
    const secret = state.groupKeys[groupId];
    if (!secret) {
      return null;
    }
    let name = '';
    let adminUuid = '';
    try {
      const inbox = await this.getGroupInbox(mlp);
      const entry = inbox.find((g) => g.group_id === groupId);
      if (entry) {
        name = entry.name;
        adminUuid = entry.admin_uuid;
      }
    } catch {
      // Fall back to empty metadata; the Group SIK still decrypts items.
    }
    return JSON.stringify({
      group: { group_id: groupId, name, admin_uuid: adminUuid },
      secret,
    });
  }

  // -------------------------------------------------------------------------
  // Sync (api.md §4)
  // -------------------------------------------------------------------------

  /** Full metadata-first sync: pull changes, fetch payloads, push local. */
  async sync(): Promise<void> {
    if (!this.dek) {
      throw new Error('vault is locked');
    }
    this.emit({ type: 'SyncStarted' });
    try {
      const state = await this.store.getState();
      const pull = await this.api.request<SyncPullResponse>(
        'GET',
        `/sync/pull?cursor=${state.cursor}`,
      );

      // Epoch gate: if our local generation is stale, go read-only (api.md §5).
      const minGen = pull.min_enc_key_gen;
      if (state.localKeyGen < minGen) {
        this.emit({ type: 'KeyUpdateRequired' });
      }
      await this.store.setState({ minEncKeyGen: minGen });

      // Determine which pulled items need payloads (metadata-first bounding).
      const localItems = await this.store.getItems();
      const localByUuid = new Map(localItems.map((i) => [i.uuid, i]));
      const needPayloads: Array<{ uuid: string; version: number }> = [];

      for (const item of pull.items) {
        if (item.deleted_date !== null) {
          await this.store.deleteItem(item.uuid);
          this.emit({ type: 'OverviewDeleted', uuid: item.uuid });
          continue;
        }
        const local = localByUuid.get(item.uuid);
        if (!local || local.version !== item.version) {
          needPayloads.push({ uuid: item.uuid, version: item.version });
        }
      }

      // Fetch + decrypt the needed payloads.
      for (let i = 0; i < needPayloads.length; i += 100) {
        const batch = needPayloads.slice(i, i + 100);
        const resp = await this.api.request<{ results: PullPayloadResult[] }>(
          'POST',
          '/sync/pull-payloads',
          { items: batch },
        );
        for (const r of resp.results) {
          if (r.status !== 'payload_delivered' || r.payload == null) {
            continue;
          }
          const plaintext = this.crypto.decryptItem(
            r.uuid,
            r.enc_key_gen ?? 0,
            this.dek,
            fromBase64(r.payload),
          );
          const parsed = JSON.parse(new TextDecoder().decode(plaintext)) as ItemPlaintext;
          const overview: DecryptedOverview = {
            uuid: r.uuid,
            title: parsed.title,
            subtitle: parsed.subtitle,
            iconKey: parsed.iconKey ?? 'key',
            urls: parsed.urls ?? [],
            updatedAt: Date.now(),
          };
          await this.store.putItem({
            uuid: r.uuid,
            version: r.version ?? 0,
            encKeyGen: r.enc_key_gen ?? 0,
            deletedDate: r.deleted_date ?? null,
            payload: r.payload,
          });
          this.emit({ type: 'OverviewUpserted', overview });
        }
      }

      // Push any locally-pending mutations (adds/updates) via push-batch.
      const pending = (await this.store.getItems()).filter((i) => i.pending);
      if (pending.length > 0) {
        const batchItems: PushBatchItem[] = pending.map((i) => ({
          uuid: i.uuid,
          target_version: i.version,
          enc_key_gen: this.localKeyGen,
          payload: i.payload,
          deleted_date: i.deletedDate,
        }));
        const resp = await this.api.request<{ results: PushBatchResult[] }>(
          'POST',
          '/sync/push-batch',
          { items: batchItems },
        );
        const updated = new Map<string, number>();
        const conflicts: ConflictEvent[] = [];
        for (const r of resp.results) {
          if (r.status === 'success' && r.version !== undefined) {
            updated.set(r.uuid, r.version);
          } else if (this.isConflictStatus(r.status)) {
            // 412 / version-mismatch from the server (data.md §7.2). Surface it
            // to the UI as a conflict for human resolution.
            const local = pending.find((i) => i.uuid === r.uuid);
            conflicts.push({
              uuid: r.uuid,
              localVersion: String(local?.version ?? 0),
              serverVersion: String(r.version ?? (local?.version ?? 0) + 1),
              isToxic: r.status === 'toxic' || r.status === 'unreadable',
            });
          }
        }
        for (const item of pending) {
          const v = updated.get(item.uuid);
          await this.store.putItem({ ...item, version: v ?? item.version, pending: false });
        }
        for (const conflict of conflicts) {
          this.emit({ type: 'ConflictDetected', event: conflict });
        }
      }

      await this.store.setState({ cursor: pull.new_cursor });
      this.emit({ type: 'SyncCompleted' });
    } catch (error) {
      this.emit({ type: 'SyncFailed', error: { type: 'NetworkError', message: String(error) } });
      throw error;
    }
  }

  // -------------------------------------------------------------------------
  // Items
  // -------------------------------------------------------------------------

  /** Create + encrypt a new item locally (pushed on the next sync). */
  async addItem(input: AddItemInput): Promise<DecryptedOverview> {
    if (!this.dek) {
      throw new Error('vault is locked');
    }
    const uuid = crypto.randomUUID();
    const plaintext: ItemPlaintext = {
      title: input.title,
      subtitle: input.username,
      iconKey: 'key',
      urls: input.url ? [input.url] : [],
      password: input.password,
    };
    const payload = this.crypto.encryptItem(
      uuid,
      this.localKeyGen,
      this.dek,
      new TextEncoder().encode(JSON.stringify(plaintext)),
    );
    const item = {
      uuid,
      version: 0, // create intent
      encKeyGen: this.localKeyGen,
      deletedDate: null,
      payload: toBase64(payload),
      pending: true,
    };
    await this.store.putItem(item);
    const overview: DecryptedOverview = {
      uuid,
      title: input.title,
      subtitle: input.username,
      iconKey: 'key',
      urls: input.url ? [input.url] : [],
      updatedAt: Date.now(),
    };
    this.emit({ type: 'OverviewUpserted', overview });
    return overview;
  }

  /** Reveal a secret behind an opaque handle (looks up the local item). */
  async reveal(uuid: string): Promise<OpaqueHandle> {
    if (!this.dek) {
      throw new Error('vault is locked');
    }
    const item = await this.store.getItem(uuid);
    if (!item?.payload) {
      throw new Error('item payload not available locally; sync first');
    }
    const plaintext = this.crypto.decryptItem(
      uuid,
      item.encKeyGen,
      this.dek,
      fromBase64(item.payload),
    );
    const parsed = JSON.parse(new TextDecoder().decode(plaintext)) as ItemPlaintext;
    const handle = String(this.nextHandle++);
    this.handles.set(handle, { secret: parsed.password, lastAccess: Date.now() });
    return handle;
  }

  /**
   * Decrypt an item's full plaintext (the JSON item blob) as raw bytes for
   * sharing. The bytes are transient — handed straight to `shareItem` and
   * zeroized by the wasm module; they never enter React state.
   */
  async getItemPlaintext(uuid: string): Promise<Uint8Array> {
    if (!this.dek) {
      throw new Error('vault is locked');
    }
    const item = await this.store.getItem(uuid);
    if (!item?.payload) {
      throw new Error('item payload not available locally; sync first');
    }
    return this.crypto.decryptItem(uuid, item.encKeyGen, this.dek, fromBase64(item.payload));
  }

  /**
   * Decrypt and return an item's secret as a string for the authenticated UI
   * (popup/web). The plaintext is returned to the caller's JS momentarily and is
   * never persisted. Prefer the opaque-handle `reveal()`/`performAction()` path
   * where the platform can consume the secret directly without JS retention.
   */
  async revealSecret(uuid: string): Promise<string> {
    const handle = await this.reveal(uuid);
    const active = this.handles.get(handle);
    if (!active) {
      throw new Error('handle expired or unknown');
    }
    const secret = active.secret;
    this.handles.delete(handle);
    return secret;
  }

  /**
   * Encrypt a project secret value with the vault DEK (real AEAD, never
   * `btoa`/`atob`). The `projectUuid` is bound as associated data so a
   * ciphertext cannot be replayed into another project. Returns the base64
   * envelope to store as `value_ciphertext`. The secret UUID is assigned by the
   * server, so it is not available at encrypt time — the project scope is the
   * stable, known-at-both-ends binding.
   */
  async encryptSecretValue(projectUuid: string, plaintext: string): Promise<string> {
    const dek = this.dek;
    if (!dek) {
      throw new Error('vault is locked');
    }
    const payload = this.crypto.encryptItem(
      projectUuid,
      this.localKeyGen,
      dek,
      new TextEncoder().encode(plaintext),
    );
    return toBase64(payload);
  }

  /**
   * Decrypt a stored secret `value_ciphertext` (base64) with the vault DEK (real
   * AEAD). `projectUuid` must match the value used at encrypt time. Never
   * decodes with `atob` — the blob is genuine AEAD ciphertext.
   */
  async decryptSecretValue(projectUuid: string, valueCiphertext: string): Promise<string> {
    const dek = this.dek;
    if (!dek) {
      throw new Error('vault is locked');
    }
    const plaintext = this.crypto.decryptItem(
      projectUuid,
      this.localKeyGen,
      dek,
      fromBase64(valueCiphertext),
    );
    return new TextDecoder().decode(plaintext);
  }

  /** Delegate copy/autofill to the platform clipboard handler. */
  async performAction(action: CoreAction): Promise<void> {
    const active = this.handles.get(action.handle);
    if (!active) {
      throw new Error('handle expired or unknown');
    }
    active.lastAccess = Date.now();
    if (this.clipboardHandler) {
      this.clipboardHandler(action, active.secret);
    }
  }

  /** Explicitly dispose a handle (zeroizes the in-memory secret). */
  async release(handle: OpaqueHandle): Promise<void> {
    this.handles.delete(handle);
  }

  /** Reap idle handles (safety reaper, client.md §3). */
  reap(): void {
    const now = Date.now();
    for (const [handle, active] of this.handles) {
      if (now - active.lastAccess > HANDLE_TTL_MS) {
        this.handles.delete(handle);
      }
    }
  }

  /**
   * Whether a push-batch result status represents an OCC conflict (412 /
   * version-mismatch) that needs human resolution (data.md §7.2).
   */
  private isConflictStatus(status: string): boolean {
    return (
      status === 'conflict' ||
      status === 'version_mismatch' ||
      status === 'precondition_failed' ||
      status === 'toxic' ||
      status === 'unreadable'
    );
  }

  /**
   * Resolve an OCC conflict with the user's choice (VTR-056). Mirrors the sync
   * engine's `resolve_with_choice(uuid, choice)`:
   * - `acceptServer`: drop the local copy and re-pull to adopt the server
   *   version (valid "Keep Server Version"; toxic "Keep Local").
   * - `pushLocal`: re-push the local edit at `serverVersion + 1` (valid "Force
   *   Overwrite with Local"; toxic "Overwrite Server"), then re-sync.
   * The store dequeues the conflict after this returns; the UI refreshes.
   */
  async resolveConflict(uuid: string, choice: ConflictChoice): Promise<void> {
    if (!this.dek) {
      throw new Error('vault is locked');
    }
    const item = await this.store.getItem(uuid);
    if (choice === 'acceptServer') {
      await this.store.deleteItem(uuid);
      this.emit({ type: 'OverviewDeleted', uuid });
      await this.sync();
    } else {
      if (item) {
        await this.store.putItem({ ...item, pending: true });
      }
      await this.sync();
    }
  }
}
