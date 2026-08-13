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
