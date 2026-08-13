//! Top-level `VautrClient` orchestrator. client.md §2 (Core API surface),
//! core.md §4 (modular crate wiring). Owns the event bus, the epoch state, the
//! `PersistenceWorker`, the in-memory `LocalBlacklist` DashMap, the live secret
//! handle table (Safety Reaper), and the sync `Engine`.
//!
//! Implemented surfaces (Phases A/B/C):
//! - Lifecycle: `new`, `watch_state`, `is_locked`, `unlock_with_password`,
//!   `unlock_with_raw_key`, `lock`.
//! - Queries: `search`, `get_overview`.
//! - Secret access (client.md §2-3): `reveal_secret`, `release_secret`,
//!   `perform_action`, `read_secret` (desktop-gated).
//! - Mutations (epoch-aware): `save_item`, `delete_item`.
//! - Sync (api.md §4): `connect_sync`, `sync`, `disconnect_sync`.
//! - Rotation (core.md §4 / ADR-006): `rotate_key`.

use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_auth::error::AuthError;
use vautr_crypto::sharing::{SharingKeyPair, SharingPublicKey};
use vautr_crypto::{aead, key_tree};
use vautr_db::entity::{item_overview, item_payload};
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};
use vautr_import::{ImportReport, RawImportItem, VaultKeys};
use vautr_sharing::{
    accept_share as kem_accept, add_group_member as kem_add_member,
    create_group as kem_create_group, remove_group_member as kem_remove_member,
    rotate_group_sik as kem_rotate_group, share_item as kem_share,
    share_to_group as kem_share_group, IncomingShare, ShareBundle, ShareGroupKey, WrappedGroupKey,
};
use vautr_sync::dashmap::{DashMapEntryState, LocalBlacklist};
use vautr_sync::engine::{Engine, PulledOverview, Transport};
use vautr_sync::quarantine::{ItemStatus, Quarantine, ReaperEvent};

use base64::Engine as _;

use crate::epoch::EpochState;
use crate::event_bus::{EventBus, VaultStateUpdate};
use crate::file_transfer::{FileTransferWorker, FileTransportHandle};
use crate::handles::{CoreAction, PlatformAdapter, SecretStore};
use crate::offline::{OfflineQueue, QueuedMutation};
use crate::project_transport::{ProjectSecretSummary, ProjectSummary, ProjectTransportHandle};
use crate::sharing::{ShareGroupStore, ShareTransportHandle};
use crate::worker::{DeleteCommand, PersistenceWorker, SaveCommand, TaskOutcome};

/// Type alias to avoid `>>>>` in a struct field (edition parse limit).
pub type TransportHandle = Arc<dyn Transport>;

/// A decrypted secret handle (opaque u64) for the Safety Reaper (client.md §3).
pub type SecretHandle = u64;

/// The kind of second factor an account may require before Master-Password
/// unlock completes (VTR-052 WebAuthn + mandatory-MFA TOTP). The server
/// enforces the policy; the client models it here so `unlock_with_password`
/// can refuse to complete until the factor is satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondFactorMethod {
    /// FIDO2 / WebAuthn assertion (VTR-052).
    WebAuthn,
    /// TOTP (RFC 6238) code, verified via `/mfa/totp/verify` (mandatory-MFA).
    Totp,
}

/// The unified client. Thin orchestrator over the Rust core crates.
pub struct VautrClient {
    db: DatabaseConnection,
    bus: EventBus,
    epoch: EpochState,
    worker: PersistenceWorker,
    /// Active SVK after unlock (held in `Zeroizing`). None while locked.
    svk: Arc<tokio::sync::Mutex<Option<Zeroizing<[u8; 32]>>>>,
    /// Active DEK (derived from SVK) for secret decryption on reveal.
    dek: Arc<tokio::sync::Mutex<Option<Zeroizing<[u8; 32]>>>>,
    local_gen: Arc<AtomicU64>,
    locked: Arc<AtomicBool>,

    /// Live secret handle table + Safety Reaper (Phase A).
    handles: SecretStore,
    /// In-memory DashMap (core.md §3 Stateful DashMap).
    blacklist: Arc<LocalBlacklist>,
    /// `true` when the DashMap changed during a sync and must be persisted.
    blacklist_dirty: Arc<AtomicBool>,
    /// Sync cursor persisted across sessions.
    cursor: Arc<AtomicU64>,

    /// Platform adapter (clipboard/autofill). Set by the FFI layer (Phase 4).
    platform: Arc<tokio::sync::RwLock<Option<Arc<dyn PlatformAdapter>>>>,
    /// Sync transport (HTTP by default). Set via `connect_sync`.
    transport: Arc<tokio::sync::RwLock<Option<TransportHandle>>>,
    /// Server user id (for AD-bound crypto + RK recovery).
    server_user_id: Arc<tokio::sync::RwLock<Option<Uuid>>>,
    // --- Wave C: sharing / files / recovery / offline (VTR-026/051/043/047) ---
    /// Sharing PKI relay. Set via `connect_sharing`.
    share_transport: Arc<tokio::sync::RwLock<Option<ShareTransportHandle>>>,
    /// This client's sharing keypair (generated at vault creation, VTR-057).
    sharing_keypair: Arc<tokio::sync::RwLock<Option<SharingKeyPair>>>,
    /// Admin-held group keys for this session (sharing-pki.md §6).
    groups: ShareGroupStore,
    /// File blob-store relay. Set via `connect_files`.
    file_transport: Arc<tokio::sync::RwLock<Option<FileTransportHandle>>>,
    /// Projects transport (project list + project-scoped secret metadata).
    /// Set via `connect_projects`. The server enforces membership + reveal
    /// scopes; the client only ever sees server-returned metadata.
    project_transport: Arc<tokio::sync::RwLock<Option<ProjectTransportHandle>>>,
    /// Set when the vault was unlocked via the RK; forces MP+RK rotation.
    recovery_pending: Arc<AtomicBool>,
    /// Derived RK auth credential held while a recovery is pending (§2.3).
    recovery_creds: Arc<tokio::sync::Mutex<Option<crate::recovery::RecoveryCredentials>>>,
    /// Offline mutation queue (VTR-047).
    offline: OfflineQueue,
    /// Quarantine of toxic/unreadable items (VTR-047). The reaper evaluates
    /// these against server metadata and emits `ItemRecovered` / `ItemPermanentlyDeleted`.
    quarantine: Arc<tokio::sync::Mutex<Quarantine>>,
    /// Count of sync pulls triggered by the quarantine reaper (VTR-047 TDD 5).
    /// Incremented by `trigger_recovery_sync`; read by tests.
    reaper_sync_requests: Arc<AtomicU64>,
    // --- WebAuthn / TOTP second factor (VTR-052 + mandatory MFA) ----------
    /// The required second factor for this login, if any. `None` means no
    /// second factor is required; `Some(method)` means the account must
    /// complete that method's challenge before MP unlock (server withholds the
    /// wrapped SVK until the factor is satisfied). Set from the server's
    /// `/account/status` (`second_factor_method`) or `/mfa/status`.
    second_factor: Arc<tokio::sync::Mutex<Option<SecondFactorMethod>>>,
    /// Whether the current login has satisfied the required second factor (a
    /// successful `/webauthn/assert/verify` or `/mfa/totp/verify` round-trip).
    second_factor_verified: Arc<AtomicBool>,
}

impl VautrClient {
    /// Construct a client over an open SeaORM connection (db-contract §2–4
    /// already applied via `vautr_db::migrate::init`).
    pub fn new(db: DatabaseConnection) -> Self {
        let bus = EventBus::new(256);
        let epoch = EpochState::new();
        let worker = PersistenceWorker::new(db.clone(), epoch.clone(), bus.clone());
        Self {
            db,
            bus,
            epoch,
            worker,
            svk: Arc::new(tokio::sync::Mutex::new(None)),
            dek: Arc::new(tokio::sync::Mutex::new(None)),
            local_gen: Arc::new(AtomicU64::new(1)),
            locked: Arc::new(AtomicBool::new(true)),
            handles: SecretStore::new(),
            blacklist: Arc::new(LocalBlacklist::new()),
            blacklist_dirty: Arc::new(AtomicBool::new(false)),
            cursor: Arc::new(AtomicU64::new(0)),
            platform: Arc::new(tokio::sync::RwLock::new(None)),
            transport: Arc::new(tokio::sync::RwLock::new(None)),
            server_user_id: Arc::new(tokio::sync::RwLock::new(None)),
            share_transport: Arc::new(tokio::sync::RwLock::new(None)),
            sharing_keypair: Arc::new(tokio::sync::RwLock::new(None)),
            groups: ShareGroupStore::new(),
            file_transport: Arc::new(tokio::sync::RwLock::new(None)),
            project_transport: Arc::new(tokio::sync::RwLock::new(None)),
            recovery_pending: Arc::new(AtomicBool::new(false)),
            recovery_creds: Arc::new(tokio::sync::Mutex::new(None)),
            offline: OfflineQueue::new(),
            quarantine: Arc::new(tokio::sync::Mutex::new(Quarantine::new())),
            reaper_sync_requests: Arc::new(AtomicU64::new(0)),
            second_factor: Arc::new(tokio::sync::Mutex::new(None)),
            second_factor_verified: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Install the platform adapter that services `CoreAction`s (clipboard /
    /// autofill) natively (client.md §2, ADR-003/005).
    pub async fn set_platform_adapter(&self, adapter: Arc<dyn PlatformAdapter>) {
        *self.platform.write().await = Some(adapter);
    }

    /// Connect the sync transport. `base_url` is the server origin, `token` the
    /// bearer session, `user_id` the server user id (for AD binding).
    pub async fn connect_sync(&self, base_url: &str, token: &str, user_id: Uuid) {
        let t = crate::sync_transport::HttpTransport::new(base_url, token);
        *self.transport.write().await = Some(Arc::new(t));
        *self.server_user_id.write().await = Some(user_id);
    }

    /// Connect an arbitrary transport implementation (dependency injection;
    /// used by tests and alternative sync backends).
    pub async fn connect_sync_with_transport(&self, transport: Arc<dyn Transport>, user_id: Uuid) {
        *self.transport.write().await = Some(transport);
        *self.server_user_id.write().await = Some(user_id);
    }

    /// Set the server user id used for AD-bound crypto and RK recovery.
    pub async fn set_server_user_id(&self, user_id: Uuid) {
        *self.server_user_id.write().await = Some(user_id);
    }

    /// Disconnect the sync transport.
    pub async fn disconnect_sync(&self) {
        *self.transport.write().await = None;
    }

    // --- Quarantine Reaper (VTR-047) -------------------------------------
    /// Park an item as toxic/unreadable so the reaper can later recover or
    /// tombstone it. Called by the sync engine when it cannot decrypt an item
    /// (e.g. a lagged device, or a conflict needing human resolution).
    pub async fn mark_item_toxic(&self, uuid: Uuid) {
        self.quarantine.lock().await.mark_toxic(uuid);
    }

    /// Run one quarantine reaper pass against `server_metadata` (the current
    /// server disposition of known items). Emits `ItemRecovered` /
    /// `ItemPermanentlyDeleted` onto the event bus for each resolved item.
    ///
    /// On recovery (`ItemRecovered`) the vault must re-sync immediately to fetch
    /// the now-readable payload — `trigger_recovery_sync` performs that pull.
    pub async fn run_quarantine_reap(&self, server_metadata: &HashMap<Uuid, ItemStatus>) {
        let events = {
            let mut guard = self.quarantine.lock().await;
            vautr_sync::quarantine::evaluate(&mut guard, server_metadata)
        };
        for ev in events {
            match ev {
                ReaperEvent::ItemRecovered(uuid) => {
                    self.bus.publish(VaultStateUpdate::ItemRecovered(uuid));
                    // TDD 5: recovery triggers an immediate sync pull to fetch
                    // the now-valid payload. Best-effort; logged, never fatal.
                    self.trigger_recovery_sync();
                }
                ReaperEvent::ItemPermanentlyDeleted(uuid) => {
                    self.bus
                        .publish(VaultStateUpdate::ItemPermanentlyDeleted(uuid));
                }
            }
        }
    }

    /// Re-sync after a recovery (VTR-047 TDD 5). Increments the request counter
    /// (observable by tests) and spawns the real `sync` in the background.
    fn trigger_recovery_sync(&self) {
        self.reaper_sync_requests.fetch_add(1, Ordering::SeqCst);
        let client = self.clone_for_sync();
        tokio::spawn(async move {
            let _ = client.sync().await;
        });
    }

    /// Cheap clone of the client handle for background sync tasks.
    fn clone_for_sync(&self) -> VautrClient {
        VautrClient {
            db: self.db.clone(),
            bus: self.bus.clone(),
            epoch: self.epoch.clone(),
            worker: self.worker.clone(),
            svk: self.svk.clone(),
            dek: self.dek.clone(),
            local_gen: self.local_gen.clone(),
            locked: self.locked.clone(),
            handles: self.handles.clone(),
            blacklist: self.blacklist.clone(),
            blacklist_dirty: self.blacklist_dirty.clone(),
            cursor: self.cursor.clone(),
            platform: self.platform.clone(),
            transport: self.transport.clone(),
            server_user_id: self.server_user_id.clone(),
            share_transport: self.share_transport.clone(),
            sharing_keypair: self.sharing_keypair.clone(),
            groups: self.groups.clone(),
            file_transport: self.file_transport.clone(),
            project_transport: self.project_transport.clone(),
            recovery_pending: self.recovery_pending.clone(),
            recovery_creds: self.recovery_creds.clone(),
            offline: self.offline.clone(),
            quarantine: self.quarantine.clone(),
            reaper_sync_requests: self.reaper_sync_requests.clone(),
            second_factor: self.second_factor.clone(),
            second_factor_verified: self.second_factor_verified.clone(),
        }
    }

    /// Number of sync pulls triggered by the quarantine reaper (test hook).
    pub fn reaper_sync_request_count(&self) -> u64 {
        self.reaper_sync_requests.load(Ordering::SeqCst)
    }

    /// Subscribe to the reactive event stream (data.md §6.3 `watch_state`).
    pub fn watch_state(&self) -> broadcast::Receiver<VaultStateUpdate> {
        self.bus.subscribe()
    }

    /// True while the vault is locked.
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    /// Current local vault key generation (epoch gate source). Exposed for
    /// testing/introspection; the in-memory value is never persisted in clear.
    pub fn current_key_gen(&self) -> u64 {
        self.local_gen.load(Ordering::SeqCst)
    }

    /// Current active SVK (in-memory only). Exposed for testing/introspection.
    pub async fn current_svk(&self) -> Option<Zeroizing<[u8; 32]>> {
        self.svk.lock().await.clone()
    }

    // --- Lifecycle (client.md §2) -------------------------------------------

    /// Unlock with the Master Password. Derives MK → KEK → unwraps SVK from the
    /// wrapped blob fetched from the server (crypto.md §2.3–2.4).
    /// `wrapped_svk` is the MP-derived `svk_ciphertext_blob` (server-side).
    pub async fn unlock_with_password(
        &self,
        mp: Zeroizing<String>,
        kdf_salt: &[u8; 32],
        wrapped_svk: &[u8],
        server_user_id: Uuid,
        local_gen: u64,
    ) -> Result<(), String> {
        // A4 (mlp-wave-plan §3): discover the server-declared required second
        // factor before evaluating the unlock gate. We only override the in-memory
        // state when the transport returns an authoritative `Some(...)`; a `None`
        // (or an unsupported transport) leaves any existing state untouched so
        // test/CLI flows that set it manually are not clobbered. The gate below
        // then refuses MP unlock until the factor is verified, mirroring the
        // server's independent enforcement via `/account/status`.
        if let Some(transport) = self.transport.read().await.clone() {
            if let Ok(Some(method)) = transport.second_factor_method().await {
                let parsed = match method.as_str() {
                    "webauthn" => Some(SecondFactorMethod::WebAuthn),
                    "totp" => Some(SecondFactorMethod::Totp),
                    _ => None,
                };
                self.set_second_factor_method(parsed).await;
            }
        }
        // VTR-052 + mandatory-MFA: when the account requires a second factor
        // (WebAuthn or TOTP), refuse to complete MP unlock until the client has
        // verified it. This surfaces `AuthError::SecondFactorRequired` to the
        // caller; the server independently withholds the wrapped SVK via
        // `/account/status` (`second_factor_method`).
        let sf_required = self.second_factor.lock().await.is_some();
        if sf_required && !self.second_factor_verified.load(Ordering::SeqCst) {
            return Err(AuthError::SecondFactorRequired.to_string());
        }
        let mk = vautr_crypto::kdf::derive_master_key(&mp, kdf_salt)
            .map_err(|e| format!("mk derive: {e}"))?;
        let kek = key_tree::derive_kek(&mk).map_err(|e| format!("kek derive: {e}"))?;
        let svk = {
            let pt = aead::decrypt(&kek, &server_user_id, 0, wrapped_svk)
                .map_err(|_| "svk unwrap failed (wrong password?)".to_string())?;
            if pt.len() != 32 {
                return Err("malformed svk blob".into());
            }
            let mut s = Zeroizing::new([0u8; 32]);
            s.copy_from_slice(&pt);
            s
        };
        // Derive the contextual DEK used to decrypt secrets on reveal.
        let dek = key_tree::derive_dek(&svk).map_err(|e| format!("dek derive: {e}"))?;
        *self.svk.lock().await = Some(svk);
        *self.dek.lock().await = Some(dek);
        *self.server_user_id.write().await = Some(server_user_id);
        self.local_gen.store(local_gen, Ordering::SeqCst);
        self.epoch.set_min_enc_key_gen(local_gen);
        self.load_blacklist().await;
        self.locked.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Unlock directly with a raw SVK (e.g. biometric unwrap, crypto.md §6).
    pub async fn unlock_with_raw_key(&self, raw_key: Zeroizing<[u8; 32]>, local_gen: u64) {
        let dek = key_tree::derive_dek(&raw_key).expect("dek derive");
        *self.svk.lock().await = Some(raw_key);
        *self.dek.lock().await = Some(dek);
        self.local_gen.store(local_gen, Ordering::SeqCst);
        self.epoch.set_min_enc_key_gen(local_gen);
        self.load_blacklist().await;
        self.locked.store(false, Ordering::SeqCst);
    }

    // --- WebAuthn / TOTP second factor (VTR-052 + mandatory MFA) ----------

    /// The kind of second factor the account requires before MP unlock.
    pub async fn set_second_factor_method(&self, method: Option<SecondFactorMethod>) {
        *self.second_factor.lock().await = method;
        if method.is_none() {
            // A cleared requirement also resets any stale "verified" marker.
            self.second_factor_verified.store(false, Ordering::SeqCst);
        }
    }

    /// Backwards-compatible setter: `true` maps to a required WebAuthn factor,
    /// `false` clears any required factor. Prefer [`set_second_factor_method`].
    pub async fn set_second_factor_required(&self, required: bool) {
        self.set_second_factor_method(if required {
            Some(SecondFactorMethod::WebAuthn)
        } else {
            None
        })
        .await;
    }

    /// Whether the account requires a second factor this login.
    pub async fn second_factor_required(&self) -> bool {
        self.second_factor.lock().await.is_some()
    }

    /// The required second-factor method, if any.
    pub async fn second_factor_method(&self) -> Option<SecondFactorMethod> {
        self.second_factor.lock().await.clone()
    }

    /// Whether the current login has already satisfied the second factor (a
    /// successful `/webauthn/assert/verify` or `/mfa/totp/verify`).
    pub fn second_factor_verified(&self) -> bool {
        self.second_factor_verified.load(Ordering::SeqCst)
    }

    /// Mark the second factor as verified after a successful
    /// `/webauthn/assert/verify` round-trip. This un-gates `unlock_with_password`.
    pub fn verify_second_factor(&self) {
        self.second_factor_verified.store(true, Ordering::SeqCst);
    }

    /// Complete a TOTP second-factor challenge (mandatory-MFA seam). Calls the
    /// transport's `/mfa/totp/verify`; on success the second factor is marked
    /// verified and MP unlock is un-gated. Errors if the transport rejects the
    /// code or no sync transport is connected.
    pub async fn verify_totp(&self, code: &str) -> Result<(), String> {
        let transport = self.transport.read().await.clone();
        let transport = transport.ok_or_else(|| "sync not connected".to_string())?;
        transport
            .verify_totp(code)
            .await
            .map_err(|e| format!("totp verify: {e}"))?;
        self.second_factor_verified.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Reset the second-factor state (e.g. on lock or a fresh login).
    pub async fn reset_second_factor(&self) {
        *self.second_factor.lock().await = None;
        self.second_factor_verified.store(false, Ordering::SeqCst);
    }

    /// Lock the vault, wipe in-memory keys, and release all secret handles.
    pub async fn lock(&self) {
        *self.svk.lock().await = None;
        *self.dek.lock().await = None;
        self.locked.store(true, Ordering::SeqCst);
        self.reset_second_factor().await;
        self.bus.publish(VaultStateUpdate::VaultLocked);
    }

    // --- Data queries (client.md §2) ----------------------------------------

    /// FTS5 search. Empty query returns the 50 most-recently-used items
    /// (db-contract §4 / client.md §2 `search`).
    pub async fn search(&self, query: &str) -> Result<Vec<DecryptedOverview>, String> {
        let rows = if query.trim().is_empty() {
            vautr_db::query::recent_overviews(&self.db, 50)
                .await
                .map_err(|e| format!("search: {e}"))?
        } else {
            vautr_db::query::search_overviews(&self.db, query)
                .await
                .map_err(|e| format!("search: {e}"))?
        };
        Ok(rows)
    }

    /// Fetch a single overview by uuid.
    pub async fn get_overview(&self, uuid: Uuid) -> Result<DecryptedOverview, String> {
        vautr_db::query::get_overview(&self.db, &uuid.to_string())
            .await
            .map_err(|e| format!("get_overview: {e}"))
    }

    // --- Secret access (client.md §2-3) -------------------------------------

    /// Decrypt and hold an item's secret behind an opaque handle (client.md §3).
    /// The Safety Reaper zeroizes it after 60s idle; call `release_secret` to
    /// dispose explicitly. Returns the `SecretHandle`.
    pub async fn reveal_secret(&self, uuid: Uuid) -> Result<SecretHandle, String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let Some((enc_key_gen, payload)) =
            vautr_db::query::get_secret_material(&self.db, &uuid.to_string()).await?
        else {
            return Err("item has no local secret".into());
        };
        let dek = self.dek.lock().await;
        let dek = dek.as_ref().ok_or_else(|| "vault locked".to_string())?;
        let pt = aead::decrypt(dek, &uuid, enc_key_gen as u64, &payload)
            .map_err(|_| "secret decrypt failed".to_string())?;
        let secret: DecryptedSecret =
            serde_json::from_slice(&pt).map_err(|e| format!("secret parse: {e}"))?;
        // Reveal only the primary password (the secret string the UI copies).
        let bytes = Zeroizing::new(secret.password.as_bytes().to_vec());
        Ok(self.handles.reveal(bytes))
    }

    /// Explicitly release a secret handle, zeroizing its memory (client.md §3).
    pub fn release_secret(&self, handle: SecretHandle) {
        self.handles.release(handle);
    }

    /// Delegate a copy/autofill action to the Platform Adapter. The secret never
    /// enters the JS bridge (client.md §2, ADR-003/005).
    pub async fn perform_action(&self, action: CoreAction) -> Result<(), String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let secret = self
            .handles
            .access(action.handle())
            .ok_or_else(|| "secret handle expired or invalid".to_string())?;
        let adapter = self.platform.read().await;
        let adapter = adapter
            .as_ref()
            .ok_or_else(|| "no platform adapter installed".to_string())?;
        adapter.service_action(action, &secret)
    }

    /// Read the secret string. ONLY available on Desktop via feature flag
    /// (client.md §2 / data.md §4 Restricted API Rule). GPUI renders directly.
    #[cfg(feature = "desktop-api")]
    pub fn read_secret(&self, handle: SecretHandle) -> Result<Zeroizing<String>, String> {
        let secret = self
            .handles
            .access(handle)
            .ok_or_else(|| "secret handle expired or invalid".to_string())?;
        let s = String::from_utf8((*secret).clone())
            .map_err(|_| "secret is not valid utf-8".to_string())?;
        Ok(Zeroizing::new(s))
    }

    // --- Mutations (epoch-aware, client.md §2) ------------------------------

    /// Save an item. `payload` is the pre-encrypted ciphertext blob (OEK/DEK).
    /// Captures the live `sync_epoch` and enqueues via the `PersistenceWorker`.
    pub async fn save_item(&self, item: DomainModel, payload: Vec<u8>) -> TaskOutcome {
        if self.is_locked() {
            return TaskOutcome::Rejected {
                receipt: 0,
                original_state: crate::event_bus::RevertibleState::Saved(item),
            };
        }
        let cmd = SaveCommand {
            item,
            payload,
            sync_epoch: self.epoch.capture(),
        };
        self.worker.save(cmd).await
    }

    /// Delete an item.
    pub async fn delete_item(&self, uuid: Uuid) -> TaskOutcome {
        if self.is_locked() {
            return TaskOutcome::Rejected {
                receipt: 0,
                original_state: crate::event_bus::RevertibleState::Deleted(DecryptedOverview {
                    uuid,
                    title: String::new(),
                    subtitle: String::new(),
                    icon_key: String::new(),
                    urls: Vec::new(),
                    updated_at: 0,
                }),
            };
        }
        let cmd = DeleteCommand {
            uuid,
            sync_epoch: self.epoch.capture(),
        };
        self.worker.delete(cmd).await
    }

    // --- Sync (api.md §4, core.md §3) ---------------------------------------

    /// Run a metadata-first sync: pull metadata, download payloads for
    /// non-ignored items, upsert locally, and update the epoch gate. At the end
    /// of the session the DashMap is persisted if dirty (core.md §3).
    pub async fn sync(&self) -> Result<(), String> {
        let transport = self.transport.read().await.clone();
        let transport = transport.ok_or_else(|| "sync not connected".to_string())?;
        let engine = Engine::new(transport.clone(), self.blacklist.clone());

        self.bus.publish(VaultStateUpdate::SyncStarted);
        let mut progress = 0u8;

        let (next_cursor, overviews) =
            engine.pull().await.map_err(|e| format!("sync pull: {e}"))?;
        self.cursor.store(next_cursor, Ordering::SeqCst);

        // Epoch gate: if the server's min_gen is ahead of our local key gen,
        // enter Read-Only (core.md §1.3, REQ-AUTH-05).
        let local_gen = self.local_gen.load(Ordering::SeqCst);

        let total = overviews.len().max(1) as u8;
        let mut idx = 0u8;
        for ov in &overviews {
            // Skip toxic/ignored items (ADR-002): never download their payload.
            if self.blacklist.is_ignored(&ov.uuid) {
                idx += 1;
                continue;
            }
            if !ov.deleted {
                if let Ok(Some(payload)) =
                    engine.fetch_payload_if_allowed(&ov.uuid, ov.version).await
                {
                    self.persist_synced_item(&ov, payload).await?;
                }
            }
            idx += 1;
            let p = ((idx as u32 * 100) / total as u32) as u8;
            if p != progress {
                progress = p;
                self.bus.publish(VaultStateUpdate::SyncProgress(p));
            }
        }

        // Update epoch gate from the server's min_enc_key_gen.
        if let Ok((min_gen, _svk_blob)) = transport.account_status().await {
            self.epoch.set_min_enc_key_gen(min_gen);
            if local_gen < min_gen {
                self.bus.publish(VaultStateUpdate::KeyUpdateRequired);
            }
        }

        // Persist the DashMap at end of session if it changed (core.md §3).
        self.persist_blacklist_if_dirty().await?;

        self.bus.publish(VaultStateUpdate::SyncCompleted);
        Ok(())
    }

    /// Upsert a synced item's overview + payload at the server's version.
    async fn persist_synced_item(
        &self,
        ov: &PulledOverview,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        let overview_am = item_overview::ActiveModel {
            uuid: Set(ov.uuid.to_string()),
            version: Set(ov.version as i64),
            enc_key_gen: Set(ov.enc_key_gen as i64),
            deleted_date: Set(if ov.deleted {
                Some(ov.version as i64)
            } else {
                None
            }),
            overview_title: Set(String::new()),
            overview_subtitle: Set(String::new()),
            overview_icon_key: Set(String::new()),
            overview_urls: Set("[]".into()),
            created_at: Set(0),
            updated_at: Set(0),
        };
        let payload_am = item_payload::ActiveModel {
            uuid: Set(ov.uuid.to_string()),
            payload: Set(payload),
        };
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| format!("begin txn: {e}"))?;
        vautr_db::txn::apply_sync_batch_txn(
            &txn,
            vec![overview_am],
            vec![payload_am],
            vec![],
            self.cursor.load(Ordering::SeqCst) as i64,
            self.epoch.current_min_gen() as i64,
        )
        .await
        .map_err(|e| format!("apply_sync_batch: {e}"))?;
        txn.commit().await.map_err(|e| format!("commit: {e}"))?;
        self.bus
            .publish(VaultStateUpdate::OverviewUpserted(DecryptedOverview {
                uuid: ov.uuid,
                title: String::new(),
                subtitle: String::new(),
                icon_key: String::new(),
                urls: Vec::new(),
                updated_at: 0,
            }));
        Ok(())
    }

    // --- Rotation (core.md §4 / ADR-006) ------------------------------------

    /// Crash-safe SVK rotation. Derives a new SVK, advances the server epoch
    /// gate, re-encrypts local items whose `enc_key_gen < new_gen` in batches of
    /// 100, pushes them, and persists the new MP-wrapped SVK blob.
    pub async fn rotate_key(&self, new_gen: u64) -> Result<(), String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let transport = self.transport.read().await.clone();
        let transport = transport.ok_or_else(|| "sync not connected".to_string())?;
        let user_id = *self.server_user_id.read().await;
        let _user_id = user_id.ok_or_else(|| "no server user id".to_string())?;

        // 1. Derive the new SVK. Wrap it under a KEK derived from the new SVK
        //    (the server stores the opaque blob; the client re-derives the KEK
        //    from the MP on next unlock via `unlock_with_password`).
        let old_svk = {
            let g = self.svk.lock().await;
            g.as_ref()
                .ok_or_else(|| "vault locked".to_string())?
                .clone()
        };
        let new_svk = vautr_keyring::svk::new_vault_keys().0;
        let kek = key_tree::derive_kek(&new_svk).map_err(|e| format!("kek: {e}"))?;
        let wrapped_new = vautr_keyring::wrap::wrap_svk(&kek, &new_svk);

        // 2. Advance the server epoch gate (idempotent, REQ-ROTATE-02).
        let confirmed_gen = transport
            .rotate_key(new_gen, wrapped_new.clone())
            .await
            .map_err(|e| format!("rotate_key: {e}"))?;
        self.epoch.set_min_enc_key_gen(confirmed_gen);
        self.bus.publish(VaultStateUpdate::KeyUpdateRequired);

        // 3. Crash-safe batch re-encryption (ADR-006). Re-encrypt every item
        //    whose `enc_key_gen < confirmed_gen`, push, and persist locally.
        let rows = vautr_db::query::list_enc_key_gens(&self.db)
            .await
            .map_err(|e| format!("list gens: {e}"))?;
        let batch_size = 100u64;
        let mut cursor_gen: i64 = 0;
        loop {
            let batch: Vec<(Uuid, i64, Vec<u8>)> = rows
                .iter()
                .filter(|(_, gen, _)| *gen < confirmed_gen as i64 && *gen >= cursor_gen)
                .take(batch_size as usize)
                .map(|(u, g, p)| (*u, *g, p.clone()))
                .collect();
            if batch.is_empty() {
                break;
            }
            cursor_gen = batch.last().map(|(_, g, _)| *g + 1).unwrap_or(cursor_gen);
            let mut push_items = Vec::with_capacity(batch.len());
            for (uuid, gen, payload) in &batch {
                // Decrypt under the OLD DEK, re-encrypt under the NEW SVK.
                let dek_old = key_tree::derive_dek(&old_svk).map_err(|e| format!("dek: {e}"))?;
                let pt = aead::decrypt(&dek_old, uuid, *gen as u64, payload)
                    .map_err(|_| "re-encrypt decrypt failed".to_string())?;
                let rot = vautr_keyring::rotate::RotationBatch {
                    old_gen: *gen as u64,
                    new_gen: confirmed_gen,
                };
                let new_payload = rot.reencrypt(uuid, &new_svk, &pt);
                // Persist the re-encrypted payload locally (atomic).
                let overview_am = item_overview::ActiveModel {
                    uuid: Set(uuid.to_string()),
                    version: Set(*gen as i64),
                    enc_key_gen: Set(confirmed_gen as i64),
                    deleted_date: Set(None),
                    overview_title: Set(String::new()),
                    overview_subtitle: Set(String::new()),
                    overview_icon_key: Set(String::new()),
                    overview_urls: Set("[]".into()),
                    created_at: Set(0),
                    updated_at: Set(0),
                };
                let payload_am = item_payload::ActiveModel {
                    uuid: Set(uuid.to_string()),
                    payload: Set(new_payload.clone()),
                };
                let txn = self.db.begin().await.map_err(|e| format!("begin: {e}"))?;
                vautr_db::txn::save_item_txn(&txn, overview_am, payload_am, &uuid.to_string())
                    .await
                    .map_err(|e| format!("persist re-encrypt: {e}"))?;
                txn.commit().await.map_err(|e| format!("commit: {e}"))?;
                push_items.push((*uuid, *gen as u64, confirmed_gen, Some(new_payload)));
            }
            let _outcomes = transport
                .push_batch(push_items)
                .await
                .map_err(|e| format!("push_batch: {e}"))?;
        }

        // 4. Persist the new wrapped SVK blob into `sync_meta`.
        vautr_db::txn::store_svk_blob(&self.db, &wrapped_new)
            .await
            .map_err(|e| format!("store svk: {e}"))?;

        // 5. Promote the new SVK locally and clear the Read-Only gate.
        self.local_gen.store(confirmed_gen, Ordering::SeqCst);
        self.epoch.set_min_enc_key_gen(confirmed_gen);
        *self.svk.lock().await = Some(new_svk.clone());
        *self.dek.lock().await =
            Some(key_tree::derive_dek(&new_svk).map_err(|e| format!("dek: {e}"))?);
        self.bus.publish(VaultStateUpdate::SyncCompleted);
        Ok(())
    }

    /// Bump the sync epoch (e.g. on a vault-mode transition, core.md §1.1).
    pub fn notify_epoch_transition(&self) -> u64 {
        self.epoch.bump()
    }

    // --- Internal DashMap persistence (core.md §3) --------------------------

    /// Load the persisted DashMap from SQLite at unlock (crash recovery).
    async fn load_blacklist(&self) {
        match vautr_db::query::list_blacklist(&self.db).await {
            Ok(rows) => {
                let mapped: Vec<(Uuid, i64, DashMapEntryState)> = rows
                    .into_iter()
                    .filter_map(|(uuid, v, state)| {
                        let uuid = Uuid::parse_str(&uuid).ok()?;
                        let state = DashMapEntryState::from_db_str(&state)?;
                        Some((uuid, v, state))
                    })
                    .collect();
                self.blacklist.load(mapped);
            }
            Err(_) => { /* fresh vault: empty blacklist */ }
        }
    }

    /// Persist the DashMap if it was modified during the sync session.
    async fn persist_blacklist_if_dirty(&self) -> Result<(), String> {
        if !self.blacklist_dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        let snapshot = self.blacklist.snapshot();
        let entries: Vec<vautr_db::entity::local_blacklist::ActiveModel> = snapshot
            .into_iter()
            .map(|(uuid, v, state)| {
                vautr_db::txn::blacklist_entry(uuid.to_string(), v, state.into_db_state())
            })
            .collect();
        let txn = self.db.begin().await.map_err(|e| format!("begin: {e}"))?;
        vautr_db::txn::persist_dashmap_txn(&txn, entries)
            .await
            .map_err(|e| format!("persist dashmap: {e}"))?;
        txn.commit().await.map_err(|e| format!("commit: {e}"))?;
        self.blacklist_dirty.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Mark an item ignored in the DashMap (core.md §3 context-sensitive 412).
    pub fn mark_ignored(&self, uuid: Uuid, ignored_version: i64, toxic: bool) {
        let state = if toxic {
            DashMapEntryState::ToxicIgnored
        } else {
            DashMapEntryState::ValidIgnored
        };
        self.blacklist.insert(uuid, ignored_version, state);
        self.blacklist_dirty.store(true, Ordering::SeqCst);
    }

    // === Wave C: transport wiring -----------------------------------------

    /// Install the sharing PKI relay (HTTP by default, in-memory in tests).
    pub async fn connect_sharing(&self, transport: ShareTransportHandle) {
        *self.share_transport.write().await = Some(transport);
    }

    /// Install the file blob-store relay for the `FileTransferWorker`.
    pub async fn connect_files(&self, transport: FileTransportHandle) {
        *self.file_transport.write().await = Some(transport);
    }

    /// Install the projects transport (project list + project-scoped secret
    /// metadata; the server enforces membership + reveal scopes, mlp-wave-plan
    /// §3 A1). The client never decrypts server-stored ciphertext.
    pub async fn connect_projects(&self, transport: ProjectTransportHandle) {
        *self.project_transport.write().await = Some(transport);
    }

    /// `GET /projects` — list the projects visible to the caller.
    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, String> {
        let t = self.project_transport.read().await.clone();
        let t = t.ok_or_else(|| "projects transport not connected".to_string())?;
        t.list_projects().await
    }

    /// `GET /projects/{uuid}/secrets` — list secret *metadata* within a project.
    /// The server enforces project access (CanView) and never returns the
    /// `value_ciphertext` here (reveal is a separate gated call).
    pub async fn list_project_secrets(
        &self,
        project_uuid: Uuid,
    ) -> Result<Vec<ProjectSecretSummary>, String> {
        let t = self.project_transport.read().await.clone();
        let t = t.ok_or_else(|| "projects transport not connected".to_string())?;
        t.list_project_secrets(project_uuid).await
    }

    /// True while the vault was unlocked via the RK and a forced MP+RK rotation
    /// is still pending (emergency-recovery-account.md §2.3).
    pub fn recovery_pending(&self) -> bool {
        self.recovery_pending.load(Ordering::SeqCst)
    }

    // === Vault creation (VTR-057) -----------------------------------------

    /// Create a fresh vault: generate the SVK, the sharing keypair (VTR-057)
    /// and a new 24-word Recovery Key. Unlocks the client in-place and returns
    /// the material the caller must persist (wrapped SVK, RK mnemonic).
    pub async fn create_vault(
        &self,
    ) -> Result<(Zeroizing<[u8; 32]>, SharingKeyPair, String), String> {
        let (svk, _oek, dek) = vautr_keyring::svk::new_vault_keys();
        let kp = crate::sharing::generate_vault_sharing_keypair();
        let rk = crate::recovery::generate_recovery_key();
        *self.svk.lock().await = Some(svk.clone());
        *self.dek.lock().await = Some(dek);
        *self.sharing_keypair.write().await = Some(kp.clone());
        self.locked.store(false, Ordering::SeqCst);
        Ok((svk, kp, rk))
    }

    /// Install a sharing keypair (used by tests / device restore).
    pub async fn set_sharing_keypair(&self, kp: SharingKeyPair) {
        *self.sharing_keypair.write().await = Some(kp);
    }

    /// This client's sharing public key (VTR-057), if a keypair is installed.
    pub async fn sharing_public_key(&self) -> Option<SharingPublicKey> {
        self.sharing_keypair.read().await.as_ref().map(|k| k.public)
    }

    // === Import → seed (VTR-027/045, data-import-seeding.md) ---------------

    /// Import a competitor export file (SVK → OEK/DEK), ingest into the local
    /// DB via the bulk fast-path, seed the server, and emit a single
    /// `ImportCompleted(report)` event.
    pub async fn import_file(
        &self,
        path: &str,
        progress: impl Fn(u8),
    ) -> Result<ImportReport, String> {
        let svk = self.require_svk().await?;
        let keys = VaultKeys::from_svk(&svk).map_err(|e| e.to_string())?;
        let report = vautr_import::import_file(path, &self.db, &keys, progress)
            .await
            .map_err(|e| e.to_string())?;
        self.seed().await?;
        self.bus
            .publish(VaultStateUpdate::ImportCompleted(report.clone()));
        Ok(report)
    }

    /// Import already-parsed raw items (no disk), ingest + seed, emit
    /// `ImportCompleted(report)`.
    pub async fn import_items(
        &self,
        items: Vec<RawImportItem>,
        progress: impl Fn(u8),
    ) -> Result<ImportReport, String> {
        let svk = self.require_svk().await?;
        let keys = VaultKeys::from_svk(&svk).map_err(|e| e.to_string())?;
        let report = vautr_import::import_items(items, &self.db, &keys, progress)
            .await
            .map_err(|e| e.to_string())?;
        self.seed().await?;
        self.bus
            .publish(VaultStateUpdate::ImportCompleted(report.clone()));
        Ok(report)
    }

    /// Offline, streaming plaintext export to CSV or JSON (VTR-058).
    ///
    /// Fully offline: no network calls. For each local item it decrypts the
    /// secret in-place (revealing + releasing immediately, per client.md §3),
    /// builds an [`vautr_export::ExportRow`], and hands the row to the streaming
    /// writer. The export is **blocked** when the vault is locked or in a
    /// Read-Only gate (requires re-authentication) — reported as an
    /// `EpochMismatch` failure.
    pub async fn export_vault(
        &self,
        format: vautr_export::ExportFormat,
        path: &str,
        cancel: &std::sync::atomic::AtomicBool,
        progress: impl Fn(u64, u64),
    ) -> Result<vautr_export::ExportReport, String> {
        // Gate: locked or Read-Only epoch ⇒ block (TDD #4: EpochMismatch).
        if self.is_locked() {
            return Err("export blocked: vault locked or in read-only gate (EpochMismatch)".into());
        }
        let local_gen = self.current_key_gen();
        if self.epoch.is_read_only(local_gen) {
            return Err("export blocked: vault locked or in read-only gate (EpochMismatch)".into());
        }

        let dek = self.dek.lock().await;
        let dek = dek.as_ref().ok_or_else(|| "vault locked".to_string())?;

        let rows_raw = self.list_local_items().await?;
        let mut rows = Vec::with_capacity(rows_raw.len());
        for (uuid, _version, gen, payload) in &rows_raw {
            let overview = self
                .get_overview(*uuid)
                .await
                .map_err(|e| format!("export overview {uuid}: {e}"))?;
            let pt = aead::decrypt(dek, uuid, *gen, payload)
                .map_err(|_| format!("export decrypt {uuid}: secret decrypt failed"))?;
            let secret: DecryptedSecret = serde_json::from_slice(&pt)
                .map_err(|e| format!("export secret parse {uuid}: {e}"))?;

            let totp = secret.totp.as_ref().map(|t| vautr_export::TotpExport {
                algorithm: format!("{:?}", t.algorithm),
                digits: t.digits,
                period: t.period,
                secret_base32: t.secret_base32.clone(),
            });
            rows.push(vautr_export::ExportRow {
                uuid: *uuid,
                title: overview.title.clone(),
                username: overview.subtitle.clone(),
                password: secret.password.clone(),
                urls: overview.urls.clone(),
                notes: secret.notes.clone(),
                totp,
            });
            // `pt`/`secret` drop here, zeroizing the plaintext.
        }
        drop(dek);

        vautr_export::export_rows(rows, format, path, cancel, progress)
            .map_err(|e| format!("export write: {e}"))
    }

    /// Seed the server with all locally-persisted items via `push_batch`
    /// (data-import-seeding.md §4). Reads the local vault and pushes every item
    /// whose payload is present, in batches of 100.
    pub async fn seed(&self) -> Result<(), String> {
        self.push_local_changes().await
    }

    /// Push every locally-persisted item to the server (save→sync→push path).
    /// Used by import seeding, offline flush, and the Phase 4 save→sync→pull
    /// flow (Client A pushes, Client B pulls).
    pub async fn push_local_changes(&self) -> Result<(), String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let transport = self
            .transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sync not connected".to_string())?;
        let rows = self.list_local_items().await?;
        for chunk in rows.chunks(100) {
            let items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)> = chunk
                .iter()
                .map(|(u, v, g, p)| (*u, *v as u64, *g, Some(p.clone())))
                .collect();
            let _outcomes = transport
                .push_batch(items)
                .await
                .map_err(|e| format!("push batch: {e}"))?;
        }
        Ok(())
    }

    /// Read `(uuid, version, enc_key_gen, payload)` for every local item.
    async fn list_local_items(&self) -> Result<Vec<(Uuid, i64, u64, Vec<u8>)>, String> {
        use sea_orm::FromQueryResult;
        #[derive(FromQueryResult)]
        struct Row {
            uuid: String,
            version: i64,
            enc_key_gen: i64,
            payload: Vec<u8>,
        }
        let sql = "SELECT o.uuid, o.version, o.enc_key_gen, p.payload \
                   FROM item_overviews o LEFT JOIN item_payloads p ON p.uuid = o.uuid";
        let stmt =
            sea_orm::Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, []);
        let rows = Row::find_by_statement(stmt)
            .all(&self.db)
            .await
            .map_err(|e| format!("list local items: {e}"))?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                Uuid::parse_str(&r.uuid)
                    .ok()
                    .map(|u| (u, r.version, r.enc_key_gen as u64, r.payload))
            })
            .collect())
    }

    // === Sharing (VTR-026/041, sharing-pki.md §3-6) ------------------------

    /// Share `item_uuid`'s plaintext with a recipient (1:1, §3). Looks up the
    /// recipient's public key, KEM-wraps a fresh SIK, DEM-encrypts the payload,
    /// relays both through the share transport, and emits `ShareSent`.
    pub async fn share_item(
        &self,
        recipient_uuid: Uuid,
        item_uuid: Uuid,
        plaintext: &[u8],
    ) -> Result<ShareBundle, String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        let sender = self
            .server_user_id
            .read()
            .await
            .ok_or_else(|| "no server user id".to_string())?;
        let pk = share_t.fetch_public_key(recipient_uuid).await?;
        let bundle = kem_share(sender, recipient_uuid, item_uuid, &pk, plaintext)
            .map_err(|e| e.to_string())?;
        share_t.post_share(&bundle).await?;
        // Deliver the DEM ciphertext separately (§5).
        let payload = base64::engine::general_purpose::STANDARD
            .decode(&bundle.encrypted_payload)
            .map_err(|e| format!("decode payload: {e}"))?;
        share_t.post_share_payload(bundle.share_id, payload).await?;
        self.bus
            .publish(VaultStateUpdate::ShareSent(bundle.share_id));
        Ok(bundle)
    }

    /// Fetch this client's inbox of pending shares.
    pub async fn fetch_shares(&self) -> Result<Vec<IncomingShare>, String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        share_t.fetch_inbox().await
    }

    /// Accept an incoming share: decapsulate the SIK with our sharing keypair,
    /// decrypt the DEM payload, and emit `ShareReceived`. Returns the plaintext.
    pub async fn accept_share(&self, incoming: &IncomingShare) -> Result<Vec<u8>, String> {
        let kp = self
            .sharing_keypair
            .read()
            .await
            .clone()
            .ok_or_else(|| "no sharing keypair installed".to_string())?;
        let pt = kem_accept(&kp, incoming).map_err(|e| e.to_string())?;
        self.bus
            .publish(VaultStateUpdate::ShareReceived(incoming.share_id));
        Ok(pt)
    }

    /// Revoke a share and its payload (1:1, §5).
    pub async fn revoke_share(&self, share_id: Uuid) -> Result<(), String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        share_t.revoke_share(share_id).await?;
        self.bus.publish(VaultStateUpdate::ShareRevoked(share_id));
        Ok(())
    }

    /// Create a sharing group with a fresh unified Group SIK (§6.1).
    pub async fn create_group(&self, name: String) -> Result<Arc<ShareGroupKey>, String> {
        let admin = self
            .server_user_id
            .read()
            .await
            .ok_or_else(|| "no server user id".to_string())?;
        let key = kem_create_group(name, admin).map_err(|e| e.to_string())?;
        let id = key.group.group_id;
        self.groups.put(key);
        self.groups
            .get(&id)
            .ok_or_else(|| "group store insert failed".to_string())
    }

    /// Add a member: wrap the Group SIK for their public key and relay it (§6.2).
    pub async fn add_group_member(
        &self,
        group_id: Uuid,
        member_uuid: Uuid,
    ) -> Result<WrappedGroupKey, String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        let key = self
            .groups
            .get(&group_id)
            .ok_or_else(|| "no such group".to_string())?;
        let pk = share_t.fetch_public_key(member_uuid).await?;
        let wrapped = kem_add_member(key.as_ref(), member_uuid, &pk).map_err(|e| e.to_string())?;
        share_t.store_group_wrapped_key(&wrapped).await?;
        Ok(wrapped)
    }

    /// Remove a member: rotate the Group SIK, re-wrap for remaining members
    /// (forward secrecy, §6.3), and relay.
    pub async fn remove_group_member(
        &self,
        group_id: Uuid,
        member_uuid: Uuid,
    ) -> Result<(), String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        let key = self
            .groups
            .get(&group_id)
            .ok_or_else(|| "no such group".to_string())?;
        let admin_uuid = key.group.admin_uuid;
        let pk = share_t.fetch_public_key(admin_uuid).await?;
        let rotation = kem_remove_member(key.as_ref(), member_uuid, &[(admin_uuid, pk)])
            .map_err(|e| e.to_string())?;
        share_t
            .replace_group_wrapped_keys(rotation.rewrapped)
            .await?;
        self.groups.replace(rotation.new_key);
        Ok(())
    }

    /// Rotate a group's SIK and re-wrap for remaining members (§6.3).
    pub async fn rotate_group_sik(&self, group_id: Uuid) -> Result<(), String> {
        let share_t = self
            .share_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "sharing not connected".to_string())?;
        let key = self
            .groups
            .get(&group_id)
            .ok_or_else(|| "no such group".to_string())?;
        let admin_uuid = key.group.admin_uuid;
        let pk = share_t.fetch_public_key(admin_uuid).await?;
        let rotation =
            kem_rotate_group(&key.group, &[(admin_uuid, pk)]).map_err(|e| e.to_string())?;
        share_t
            .replace_group_wrapped_keys(rotation.rewrapped)
            .await?;
        self.groups.replace(rotation.new_key);
        Ok(())
    }

    /// Encrypt a payload under a group's SIK for every member (§6).
    pub async fn share_to_group(
        &self,
        group_id: Uuid,
        item_uuid: Uuid,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let key = self
            .groups
            .get(&group_id)
            .ok_or_else(|| "no such group".to_string())?;
        kem_share_group(key.as_ref(), &item_uuid, plaintext).map_err(|e| e.to_string())
    }

    // === Files (VTR-051, file-storage.md §4-5) -----------------------------

    /// Build a `FileTransferWorker` over the installed file transport.
    async fn file_worker(&self) -> Result<FileTransferWorker, String> {
        let t = self
            .file_transport
            .read()
            .await
            .clone()
            .ok_or_else(|| "file transport not connected".to_string())?;
        Ok(FileTransferWorker::new(t, self.bus.clone()))
    }

    /// Upload a binary attachment through the multipart protocol. Returns the
    /// finalized (Available) manifest. Progress events are throttled to 4/s.
    pub async fn upload_file(
        &self,
        plaintext: &[u8],
        content_type: &str,
        last_modified: i64,
    ) -> Result<vautr_files::manifest::FileManifest, String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let svk = self.require_svk().await?;
        let worker = self.file_worker().await?;
        worker
            .upload_bytes(&svk, plaintext, content_type, last_modified)
            .await
    }

    /// Download + decrypt an attachment by manifest.
    pub async fn download_file(
        &self,
        manifest: &vautr_files::manifest::FileManifest,
    ) -> Result<Vec<u8>, String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let svk = self.require_svk().await?;
        let worker = self.file_worker().await?;
        worker.download(&svk, manifest).await
    }

    // === Recovery (VTR-043, emergency-recovery-account.md §2-4) ------------

    /// Generate a fresh 24-word Recovery Key (Emergency Kit mnemonic).
    pub fn generate_recovery_key(&self) -> String {
        crate::recovery::generate_recovery_key()
    }

    /// Onboarding proof-of-possession gate (§3.2): verify the user retyped the
    /// correct words at positions 4, 12 and 20.
    pub fn verify_recovery_key_possession(&self, mnemonic: &str, supplied: &[&str]) -> bool {
        crate::recovery::verify_recovery_key_possession(mnemonic, supplied)
    }

    /// Render the Emergency Kit PDF (BR-7: readable words, no QR code).
    pub fn render_emergency_kit_pdf(&self, email: &str, mnemonic: &str) -> Result<Vec<u8>, String> {
        vautr_files::pdf::render_emergency_kit_pdf(email, mnemonic).map_err(|e| e.to_string())
    }

    /// Unlock a vault via the RK (§2.3): derive KEK_RK, unwrap the SVK, derive
    /// the Ed25519 recovery-auth credential, and enter the forced-rotation gate.
    pub async fn recover_with_key(
        &self,
        mnemonic: &str,
        wrapped_svk_rk: &[u8],
        server_user_id: Uuid,
    ) -> Result<(), String> {
        let creds = crate::recovery::derive_recovery_credentials(mnemonic)
            .ok_or_else(|| "invalid recovery key".to_string())?;
        let svk = vautr_keyring::recover::recover_svk(mnemonic, wrapped_svk_rk, &server_user_id)
            .ok_or_else(|| "recovery unwrap failed (bad recovery key?)".to_string())?;
        let dek = key_tree::derive_dek(&svk).map_err(|e| format!("dek: {e}"))?;
        *self.svk.lock().await = Some(svk);
        *self.dek.lock().await = Some(dek);
        *self.server_user_id.write().await = Some(server_user_id);
        self.locked.store(false, Ordering::SeqCst);
        *self.recovery_creds.lock().await = Some(creds);
        self.recovery_pending.store(true, Ordering::SeqCst);
        self.bus.publish(VaultStateUpdate::RecoveryModeEntered);
        Ok(())
    }

    /// Sign the server recovery challenge nonce with the RK Ed25519 key
    /// (proof of possession, §2.3 step 2). Requires a pending recovery.
    pub async fn recovery_sign_challenge(&self, nonce: &[u8]) -> Result<String, String> {
        let creds = self
            .recovery_creds
            .lock()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| "no pending recovery".to_string())?;
        Ok(crate::recovery::sign_nonce(&creds, nonce))
    }

    /// Complete the forced post-recovery rotation (§2.3 step 4-7): derive a new
    /// KEK_MP, re-wrap the SVK for the new MP, generate a new RK, re-wrap the
    /// SVK for the new RK, and clear the gate. The new MP-wrapped blob is
    /// persisted locally; the server round-trip is driven by the caller through
    /// the recovery endpoints using `recovery_sign_challenge`.
    pub async fn complete_recovery(
        &self,
        new_mp: Zeroizing<String>,
        kdf_salt: &[u8; 32],
    ) -> Result<String, String> {
        if !self.recovery_pending() {
            return Err("no pending recovery".into());
        }
        let svk = self.require_svk().await?;
        let user_id = self
            .server_user_id
            .read()
            .await
            .ok_or_else(|| "no server user id".to_string())?;

        // New KEK_MP from the fresh Master Password; re-wrap the SVK.
        let mk = vautr_crypto::kdf::derive_master_key(&new_mp, kdf_salt)
            .map_err(|e| format!("mk derive: {e}"))?;
        let kek_mp = key_tree::derive_kek(&mk).map_err(|e| format!("kek: {e}"))?;
        let wrapped_mp = vautr_keyring::wrap::wrap_svk(&kek_mp, &svk);

        // New RK: re-wrap the SVK under the new KEK_RK and derive the new
        // Ed25519 recovery-auth keypair.
        let new_rk = crate::recovery::generate_recovery_key();
        let mnemonic = vautr_crypto::recovery::decode_recovery_mnemonic(&new_rk)
            .map_err(|e| format!("decode rk: {e}"))?;
        let kek_rk =
            vautr_crypto::recovery::derive_kek_rk(&mnemonic).map_err(|e| format!("kek_rk: {e}"))?;
        let _wrapped_rk = vautr_crypto::recovery::wrap_svk_with_rk(&svk, &kek_rk, &user_id)
            .map_err(|e| format!("wrap rk: {e}"))?;
        let creds = crate::recovery::derive_recovery_credentials(&new_rk)
            .ok_or_else(|| "derive creds".to_string())?;

        // Persist the new MP-wrapped SVK locally and clear the gate.
        vautr_db::txn::store_svk_blob(&self.db, &wrapped_mp)
            .await
            .map_err(|e| format!("store svk: {e}"))?;
        *self.recovery_creds.lock().await = Some(creds);
        self.recovery_pending.store(false, Ordering::SeqCst);
        self.bus.publish(VaultStateUpdate::RecoveryCompleted);
        Ok(new_rk)
    }

    // === Export (VTR-058) --------------------------------------------------

    /// Export the whole vault as encrypted-safe JSON mirroring the import path:
    /// every item's plaintext secret is decrypted in memory and serialized as a
    /// `DomainModel` list. The caller is responsible for writing/zeroizing.
    pub async fn export_to_json(&self) -> Result<Vec<u8>, String> {
        if self.is_locked() {
            return Err("vault locked".into());
        }
        let dek = self
            .dek
            .lock()
            .await
            .clone()
            .ok_or_else(|| "vault locked".to_string())?;
        let rows = vautr_db::query::list_enc_key_gens(&self.db).await?;
        let mut items = Vec::with_capacity(rows.len());
        for (uuid, gen, payload) in rows {
            let pt = aead::decrypt(&dek, &uuid, gen as u64, &payload)
                .map_err(|_| format!("export decrypt {uuid}"))?;
            let secret: DecryptedSecret =
                serde_json::from_slice(&pt).map_err(|e| format!("export parse: {e}"))?;
            let overview = self.get_overview(uuid).await?;
            let metadata = ItemMetadata {
                created_at: 0,
                updated_at: overview.updated_at,
                trashed: false,
            };
            items.push(DomainModel {
                uuid,
                enc_key_gen: gen as u64,
                overview,
                secret,
                metadata,
            });
        }
        serde_json::to_vec(&items).map_err(|e| format!("export serialize: {e}"))
    }

    /// Write the encrypted-safe JSON export to `path`.
    pub async fn export_file(&self, path: &str) -> Result<(), String> {
        let bytes = self.export_to_json().await?;
        std::fs::write(path, bytes).map_err(|e| format!("write export: {e}"))
    }

    // === Offline sync (VTR-047) -------------------------------------------

    /// Number of offline-queued mutations awaiting push.
    pub fn offline_queue_len(&self) -> usize {
        self.offline.len()
    }

    /// Queue a save for later push when offline. Returns the queue length.
    pub async fn offline_save(&self, item: DomainModel, payload: Vec<u8>) -> u64 {
        let n = self.offline.push(QueuedMutation::Save { item, payload });
        self.bus
            .publish(VaultStateUpdate::OfflineMutationQueued(n as u64));
        n as u64
    }

    /// Queue a delete for later push when offline. Returns the queue length.
    pub async fn offline_delete(&self, uuid: Uuid) -> u64 {
        let n = self.offline.push(QueuedMutation::Delete { uuid });
        self.bus
            .publish(VaultStateUpdate::OfflineMutationQueued(n as u64));
        n as u64
    }

    /// Re-apply queued offline mutations locally, then push them to the server.
    pub async fn flush_offline_queue(&self) -> Result<usize, String> {
        let mutations = self.offline.drain();
        let count = mutations.len();
        for m in mutations {
            match m {
                QueuedMutation::Save { item, payload } => {
                    let _ = self.save_item(item, payload).await;
                }
                QueuedMutation::Delete { uuid } => {
                    let _ = self.delete_item(uuid).await;
                }
            }
        }
        if count > 0 {
            self.push_local_changes().await?;
        }
        Ok(count)
    }

    // === Safety Reaper hooks -----------------------------------------------

    /// Quarantine an item on a toxic conflict (Reaper + DashMap integration,
    /// core.md §3). Marks it ignored so the sync layer never re-downloads its
    /// payload; the persisted DashMap is flushed at the next sync boundary.
    pub fn reaper_quarantine(&self, uuid: Uuid, server_version: u64, toxic: bool) {
        self.mark_ignored(uuid, server_version as i64, toxic);
        self.bus
            .publish(VaultStateUpdate::NewerVersionAvailable { uuid });
    }

    /// Persist the DashMap now (crash safety) rather than waiting for the next
    /// sync boundary. Public hook for flows that mutate the blacklist outside a
    /// sync (e.g. import/sharing reaper integration).
    pub async fn persist_blacklist_now(&self) -> Result<(), String> {
        self.persist_blacklist_if_dirty().await
    }

    // --- Internal helpers --------------------------------------------------

    /// Active SVK (error when locked).
    async fn require_svk(&self) -> Result<Zeroizing<[u8; 32]>, String> {
        self.svk
            .lock()
            .await
            .clone()
            .ok_or_else(|| "vault locked".to_string())
    }
}

pub use tokio::sync::broadcast;

// Helpers to bridge `CoreAction` (which stores the handle inline) to the
// handle-store access. `CoreAction` already carries the handle.
impl CoreAction {
    /// The handle this action targets.
    pub fn handle(&self) -> u64 {
        match self {
            CoreAction::CopyToClipboard { handle } | CoreAction::Autofill { handle } => *handle,
        }
    }
}
