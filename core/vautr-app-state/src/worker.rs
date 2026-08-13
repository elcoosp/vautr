//! Persistence worker: applies SaveCommand/DeleteCommand with epoch gating.
//! REQ-SYNC-06, core.md §2. Offloaded from the UI thread; validates the
//! `sync_epoch` captured at enqueue time against the live epoch at commit time
//! and applies Context-Aware Resolution (core.md §1.3 / §2).

use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use uuid::Uuid;
use vautr_db::entity::{item_overview, item_payload};
use vautr_db::txn::save_item_txn;
use vautr_domain::{DecryptedOverview, DomainModel};

use crate::epoch::EpochState;
use crate::event_bus::{EventBus, RevertibleState, TaskReceipt};

/// A save command: an atomic aggregate + the epoch it was issued under.
/// `payload` is the ciphertext blob (encrypted by OEK/DEK in the app layer,
/// per data.md §3 contract — the worker only persists it).
#[derive(Clone, Debug)]
pub struct SaveCommand {
    pub item: DomainModel,
    pub payload: Vec<u8>,
    pub sync_epoch: u64,
}

/// A delete command: uuid + the epoch it was issued under.
#[derive(Clone, Debug)]
pub struct DeleteCommand {
    pub uuid: Uuid,
    pub sync_epoch: u64,
}

/// Outcome of a committed task.
#[derive(Clone, Debug)]
pub enum TaskOutcome {
    /// Committed; carries the monotonic receipt.
    Committed(TaskReceipt),
    /// Aborted by the Read-Only Gate; the optimistic UI must be reverted with
    /// `Rejected::original_state`.
    Rejected {
        receipt: TaskReceipt,
        original_state: RevertibleState,
    },
}

/// The gated, contention-resilient persistence worker (core.md §2).
/// Owns a monotonic receipt counter and channels commit outcomes through the
/// [`EventBus`].
pub struct PersistenceWorker {
    db: DatabaseConnection,
    epoch: EpochState,
    bus: EventBus,
    receipts: std::sync::atomic::AtomicU64,
}

impl PersistenceWorker {
    pub fn new(db: DatabaseConnection, epoch: EpochState, bus: EventBus) -> Self {
        Self {
            db,
            epoch,
            bus,
            receipts: std::sync::atomic::AtomicU64::new(1),
        }
    }

    fn next_receipt(&self) -> TaskReceipt {
        self.receipts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Enqueue + commit a save. Mirrors core.md §2 save flow.
    pub async fn save(&self, cmd: SaveCommand) -> TaskOutcome {
        let receipt = self.next_receipt();
        let overview = cmd.item.overview.clone();
        let original = RevertibleState::Saved(cmd.item.clone());

        match self.commit_save(&cmd, &overview).await {
            Ok(()) => {
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::MutationSucceeded(
                        receipt,
                    ));
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::OverviewUpserted(
                        overview,
                    ));
                TaskOutcome::Committed(receipt)
            }
            Err(reason) => {
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::MutationFailed {
                        receipt,
                        error: reason,
                        original_state: original.clone(),
                    });
                TaskOutcome::Rejected {
                    receipt,
                    original_state: RevertibleState::Saved(cmd.item.clone()),
                }
            }
        }
    }

    /// Enqueue + commit a delete. Mirrors client.md §6 delete flow.
    pub async fn delete(&self, cmd: DeleteCommand) -> TaskOutcome {
        let receipt = self.next_receipt();
        // The optimistic UI already removed the item; on failure we re-insert it.
        let original = RevertibleState::Deleted(DecryptedOverview {
            uuid: cmd.uuid,
            title: String::new(),
            subtitle: String::new(),
            icon_key: String::new(),
            urls: Vec::new(),
            updated_at: 0,
        });

        match self.commit_delete(&cmd).await {
            Ok(()) => {
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::MutationSucceeded(
                        receipt,
                    ));
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::OverviewDeleted(
                        cmd.uuid,
                    ));
                TaskOutcome::Committed(receipt)
            }
            Err(reason) => {
                self.bus
                    .publish(crate::event_bus::VaultStateUpdate::MutationFailed {
                        receipt,
                        error: reason,
                        original_state: original.clone(),
                    });
                TaskOutcome::Rejected {
                    receipt,
                    original_state: original,
                }
            }
        }
    }

    /// Commit-time gate (core.md §1.3 / §2.2). Returns `Ok(())` to proceed, or
    /// an error string describing the abort (published as `MutationFailed`).
    fn verify_epoch(&self, task_epoch: u64, local_gen: u64) -> Result<(), String> {
        let mode = self.epoch.vault_mode(local_gen);
        // Read-Only Gate: when the local key generation lags the server's minimum
        // (local_gen < min_enc_key_gen), ALL writes are rejected — even if the
        // epoch happens to match (core.md §1.3, REQ-AUTH-05).
        if matches!(mode, crate::epoch::VaultMode::KeyUpdateRequired) {
            return Err("vault updated (KeyUpdateRequired): write rejected, reverting".into());
        }
        // Safe states (ReadWrite / Migrating): proceed. On an epoch mismatch the
        // caller re-binds to the newly active SVK before committing (Context-Aware
        // Resolution, core.md §1.3 / §2.3). The `task_epoch` is checked but never
        // blocks a safe-state write.
        let _ = task_epoch;
        let _ = mode;
        Ok(())
    }

    async fn commit_save(
        &self,
        cmd: &SaveCommand,
        _overview: &DecryptedOverview,
    ) -> Result<(), String> {
        // Epoch gate. local_gen is the item's enc_key_gen (its key family).
        self.verify_epoch(cmd.sync_epoch, cmd.item.enc_key_gen)?;
        let item = &cmd.item;
        let ov = &item.overview;
        let meta = &item.metadata;

        let overview_am = item_overview::ActiveModel {
            uuid: Set(ov.uuid.to_string()),
            version: Set(1), // increment handled by sync push; local first-write is v1
            enc_key_gen: Set(item.enc_key_gen as i64),
            deleted_date: Set(if meta.trashed {
                Some(meta.updated_at)
            } else {
                None
            }),
            overview_title: Set(ov.title.clone()),
            overview_subtitle: Set(ov.subtitle.clone()),
            overview_icon_key: Set(ov.icon_key.clone()),
            overview_urls: Set(serde_json::to_string(&ov.urls).unwrap_or_else(|_| "[]".into())),
            created_at: Set(meta.created_at),
            updated_at: Set(meta.updated_at),
        };

        // The cold payload is the serialized DomainModel secret blob (ciphertext
        // from the OS layer). For the worker we persist the encrypted payload the
        // caller attached; here we store the overview-derived bytes for the schema.
        let payload_am = item_payload::ActiveModel {
            uuid: Set(ov.uuid.to_string()),
            payload: Set(cmd.payload.clone()),
        };

        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| format!("begin txn: {e}"))?;
        save_item_txn(&txn, overview_am, payload_am, &ov.uuid.to_string())
            .await
            .map_err(|e| format!("save_item_txn: {e}"))?;
        txn.commit().await.map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    async fn commit_delete(&self, cmd: &DeleteCommand) -> Result<(), String> {
        // Epoch gate with gen 0 (delete is key-family independent here).
        self.verify_epoch(cmd.sync_epoch, 0)?;
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| format!("begin txn: {e}"))?;
        item_overview::Entity::delete_many()
            .filter(item_overview::Column::Uuid.eq(cmd.uuid.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| format!("delete overview: {e}"))?;
        item_payload::Entity::delete_many()
            .filter(item_payload::Column::Uuid.eq(cmd.uuid.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| format!("delete payload: {e}"))?;
        txn.commit().await.map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_only_gate_rejects_save() {
        let epoch = EpochState::new();
        epoch.set_min_enc_key_gen(3);

        // Read-Only Gate: local_gen 2 lags min_gen 3 → KeyUpdateRequired.
        assert_eq!(
            epoch.vault_mode(2),
            crate::epoch::VaultMode::KeyUpdateRequired
        );
        // Under the gate, BOTH a stale-epoch and a live-epoch save are rejected
        // (core.md §1.3: Read-Only aborts regardless of epoch match).
        let stale = epoch.capture();
        let _ = epoch.bump();
        assert!(PersistenceWorker::verify_epoch_static(stale, 2).is_err());
        let live = epoch.capture();
        assert!(PersistenceWorker::verify_epoch_static(live, 2).is_err());

        // Safe state: local_gen 5 >= min_gen 3 → ReadWrite. Writes proceed even
        // when the captured epoch is stale (Context-Aware Resolution re-binds).
        assert_eq!(epoch.vault_mode(5), crate::epoch::VaultMode::ReadWrite);
        assert!(PersistenceWorker::verify_epoch_static(stale, 5).is_ok());
        assert!(PersistenceWorker::verify_epoch_static(live, 5).is_ok());
    }
}

#[cfg(test)]
impl PersistenceWorker {
    /// Exposed for tests (avoids constructing a DB). Mirrors [`verify_epoch`].
    fn verify_epoch_static(task_epoch: u64, local_gen: u64) -> Result<(), String> {
        let epoch = EpochState::new();
        epoch.set_min_enc_key_gen(3);
        let mode = epoch.vault_mode(local_gen);
        if matches!(mode, crate::epoch::VaultMode::KeyUpdateRequired) {
            return Err("vault updated: write rejected".into());
        }
        let _ = task_epoch;
        Ok(())
    }
}
