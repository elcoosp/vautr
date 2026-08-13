//! Event bus: `VaultStateUpdate` broadcast stream. data.md §6.3.
//!
//! The UI does not poll the core for lists — the core pushes diffs
//! (`OverviewUpserted`, `OverviewDeleted`, conflict/key-update signals) and the
//! UI reactively re-renders (client.md §5).

use tokio::sync::broadcast;
use uuid::Uuid;
use vautr_domain::{DecryptedOverview, DomainModel};
use vautr_import::ImportReport;

/// Reactive event pushed from Core to UI. data.md §6.3 (full surface).
#[derive(Clone, Debug)]
pub enum VaultStateUpdate {
    SyncStarted,
    /// Integer 0–100 progress percentage (data.md §6.3 / §1 int-only rule).
    SyncProgress(u8),
    SyncCompleted,
    SyncFailed(String),
    OverviewUpserted(DecryptedOverview),
    OverviewDeleted(Uuid),
    ConflictDetected(ConflictEvent),
    KeyUpdateRequired,
    VaultLocked,
    /// A newer server version of a ValidIgnored item is available (core.md §3 UI indicator).
    NewerVersionAvailable {
        uuid: Uuid,
    },
    /// Mutation committed successfully. `TaskReceipt` is a monotonic u64.
    MutationSucceeded(u64),
    /// Mutation failed; UI re-inserts `original_state` (list only — never the secret).
    MutationFailed {
        receipt: u64,
        error: String,
        original_state: RevertibleState,
    },
    // --- Wave C: import / files / sharing / recovery / offline (VTR-027/051/026/043) ---
    /// Bulk import finished. Carries the strongly-typed [`ImportReport`]
    /// (data-import-seeding.md §5.1). Emitted once per import, after seeding.
    ImportCompleted(ImportReport),
    /// Throttled file transfer progress (file-storage.md §5.2), max 4/s.
    FileTransferProgress {
        file_uuid: Uuid,
        bytes_transferred: u64,
        total_bytes: u64,
    },
    /// A share was posted to the relay (share_id).
    ShareSent(Uuid),
    /// An incoming share was accepted/decrypted (share_id).
    ShareReceived(Uuid),
    /// A share was revoked (share_id).
    ShareRevoked(Uuid),
    /// The vault was unlocked via the Recovery Key (RK) and now requires a
    /// forced MP + RK rotation (emergency-recovery-account.md §2.3).
    RecoveryModeEntered,
    /// The forced post-recovery rotation completed; vault is healthy again.
    RecoveryCompleted,
    /// An offline mutation was queued for later push (queued receipt).
    OfflineMutationQueued(u64),
}

/// Payload for 412 Resolution UI (data.md §7.2).
#[derive(Clone, Debug)]
pub struct ConflictEvent {
    pub uuid: Uuid,
    pub local_version: u64,
    pub server_version: u64,
    pub is_toxic: bool,
}

/// Used for surgical UI reverts on `MutationFailed` (data.md §5.3 / §3 Delete flow).
/// SECURITY: only ever carries an `Overview`, never the `DecryptedSecret`.
#[derive(Clone, Debug)]
pub enum RevertibleState {
    Saved(DomainModel),
    Deleted(DecryptedOverview),
}

/// A monotonic task identifier (data.md §6.1). Passed to JS as a string.
pub type TaskReceipt = u64;

/// Internal broadcast bus. Cheap to clone; late subscribers get only new events.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<VaultStateUpdate>,
}

impl EventBus {
    /// Capacity tuned for a burst of overview diffs during a sync.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(16));
        Self { tx }
    }

    /// Subscribe to the event stream (data.md §6.3 `watch_state`).
    pub fn subscribe(&self) -> broadcast::Receiver<VaultStateUpdate> {
        self.tx.subscribe()
    }

    /// Publish an event to all subscribers. Drops are ignored (no live subscriber).
    pub fn publish(&self, event: VaultStateUpdate) {
        let _ = self.tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_delivers_to_subscriber() {
        let bus = EventBus::new(8);
        let mut rx = bus.subscribe();
        bus.publish(VaultStateUpdate::VaultLocked);
        bus.publish(VaultStateUpdate::SyncProgress(50));
        assert!(matches!(rx.try_recv(), Ok(VaultStateUpdate::VaultLocked)));
        assert!(matches!(
            rx.try_recv(),
            Ok(VaultStateUpdate::SyncProgress(50))
        ));
    }

    #[test]
    fn conflict_event_carries_versions() {
        let ev = ConflictEvent {
            uuid: Uuid::nil(),
            local_version: 5,
            server_version: 6,
            is_toxic: true,
        };
        assert!(ev.is_toxic);
        assert_eq!(ev.local_version, 5);
    }
}
