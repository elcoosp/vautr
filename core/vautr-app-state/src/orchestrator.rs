//! Top-level `VautrClient` orchestrator. client.md §2 (Core API surface),
//! core.md §4 (modular crate wiring). Owns the event bus, the epoch state, the
//! `PersistenceWorker`, and the in-memory `LocalBlacklist` DashMap.

use sea_orm::DatabaseConnection;
use sea_orm::TransactionTrait;
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_crypto::{aead, kdf, key_tree};
use vautr_domain::{DecryptedOverview, DomainModel};

use crate::epoch::EpochState;
use crate::event_bus::{EventBus, VaultStateUpdate};
use crate::worker::{DeleteCommand, PersistenceWorker, SaveCommand};

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
    local_gen: Arc<std::sync::atomic::AtomicU64>,
    locked: Arc<std::sync::atomic::AtomicBool>,
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
            local_gen: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            locked: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Subscribe to the reactive event stream (data.md §6.3 `watch_state`).
    pub fn watch_state(&self) -> broadcast::Receiver<VaultStateUpdate> {
        self.bus.subscribe()
    }

    /// True while the vault is locked.
    pub fn is_locked(&self) -> bool {
        self.locked.load(std::sync::atomic::Ordering::SeqCst)
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
        let mk = kdf::derive_master_key(&mp, kdf_salt)
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
        *self.svk.lock().await = Some(svk);
        self.local_gen
            .store(local_gen, std::sync::atomic::Ordering::SeqCst);
        self.epoch.set_min_enc_key_gen(local_gen);
        self.locked
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Unlock directly with a raw SVK (e.g. biometric unwrap, crypto.md §6).
    pub async fn unlock_with_raw_key(&self, raw_key: Zeroizing<[u8; 32]>, local_gen: u64) {
        *self.svk.lock().await = Some(raw_key);
        self.local_gen
            .store(local_gen, std::sync::atomic::Ordering::SeqCst);
        self.epoch.set_min_enc_key_gen(local_gen);
        self.locked
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Lock the vault and wipe the in-memory SVK.
    pub async fn lock(&self) {
        *self.svk.lock().await = None;
        self.locked
            .store(true, std::sync::atomic::Ordering::SeqCst);
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

    // --- Mutations (epoch-aware, client.md §2) ------------------------------

    /// Save an item. `payload` is the pre-encrypted ciphertext blob (OEK/DEK).
    /// Captures the live `sync_epoch` and enqueues via the `PersistenceWorker`.
    pub async fn save_item(
        &self,
        item: DomainModel,
        payload: Vec<u8>,
    ) -> worker::TaskOutcome {
        if self.is_locked() {
            return worker::TaskOutcome::Rejected {
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
    pub async fn delete_item(&self, uuid: Uuid) -> worker::TaskOutcome {
        if self.is_locked() {
            return worker::TaskOutcome::Rejected {
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

    /// Bump the sync epoch (e.g. on a vault-mode transition, core.md §1.1).
    pub fn notify_epoch_transition(&self) -> u64 {
        self.epoch.bump()
    }
}

pub use tokio::sync::broadcast;
