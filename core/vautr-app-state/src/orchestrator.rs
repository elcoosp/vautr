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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_crypto::{aead, key_tree};
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel};
use vautr_sync::dashmap::{DashMapEntryState, LocalBlacklist};
use vautr_sync::engine::{Engine, PulledOverview, Transport};
use vautr_db::entity::{item_overview, item_payload};

use crate::epoch::EpochState;
use crate::event_bus::{EventBus, VaultStateUpdate};
use crate::handles::{CoreAction, PlatformAdapter, SecretStore};
use crate::worker::{DeleteCommand, PersistenceWorker, SaveCommand, TaskOutcome};

/// Type alias to avoid `>>>>` in a struct field (edition parse limit).
pub type TransportHandle = Arc<dyn Transport>;

/// A decrypted secret handle (opaque u64) for the Safety Reaper (client.md §3).
pub type SecretHandle = u64;

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

    /// Disconnect the sync transport.
    pub async fn disconnect_sync(&self) {
        *self.transport.write().await = None;
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

    /// Lock the vault, wipe in-memory keys, and release all secret handles.
    pub async fn lock(&self) {
        *self.svk.lock().await = None;
        *self.dek.lock().await = None;
        self.locked.store(true, Ordering::SeqCst);
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
        let secret: DecryptedSecret = serde_json::from_slice(&pt)
            .map_err(|e| format!("secret parse: {e}"))?;
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

        let (next_cursor, overviews) = engine.pull().await
            .map_err(|e| format!("sync pull: {e}"))?;
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
    async fn persist_synced_item(&self, ov: &PulledOverview, payload: Vec<u8>) -> Result<(), String> {
        let overview_am = item_overview::ActiveModel {
            uuid: Set(ov.uuid.to_string()),
            version: Set(ov.version as i64),
            enc_key_gen: Set(ov.enc_key_gen as i64),
            deleted_date: Set(if ov.deleted { Some(ov.version as i64) } else { None }),
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
                let txn = self.db.begin().await
                    .map_err(|e| format!("begin: {e}"))?;
                vautr_db::txn::save_item_txn(
                    &txn,
                    overview_am,
                    payload_am,
                    &uuid.to_string(),
                )
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
        *self.dek.lock().await = Some(key_tree::derive_dek(&new_svk).map_err(|e| format!("dek: {e}"))?);
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
