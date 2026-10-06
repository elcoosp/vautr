//! Quarantine Reaper (VTR-047).
//!
//! Extends the Safety Reaper (VTR-022) with *toxic-item lifecycle* handling.
//! When the sync engine flags an item as toxic (e.g. a device that cannot yet
//! decrypt it, or a conflict that needs human resolution), the item is parked
//! in the `Quarantine`. A background reaper periodically re-checks each
//! quarantined item's server metadata:
//!
//! - If the item has become **valid** (the lagged device re-authed, or a newer
//!   server version is now decryptable), the reaper emits
//!   [`ReaperEvent::ItemRecovered`] so the UI can notify the user and re-sync.
//! - If the item was **permanently deleted** server-side (tombstoned), the
//!   reaper emits [`ReaperEvent::ItemPermanentlyDeleted`].
//!
//! The reaper emits events over a tokio `mpsc` channel; `vautr-app-state`
//! forwards them onto its [`crate::event_bus`]-style broadcast bus (see
//! `VaultStateUpdate::ItemRecovered` / `ItemPermanentlyDeleted`). This crate
//! stays free of any higher-level UI dependency.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

/// Server-side disposition of a quarantined item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemStatus {
    /// The item now exists on the server and is decryptable by this client.
    Valid,
    /// The item was deleted server-side (tombstone). Nothing to recover.
    Tombstoned,
}

/// An event emitted by the quarantine reaper for one tombstoned/toxic item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaperEvent {
    /// A previously unreadable (toxic) item became valid; the UI should notify
    /// and re-sync to fetch its now-readable payload.
    ItemRecovered(Uuid),
    /// A toxic item was permanently deleted server-side; the UI should drop any
    /// stale toxic indicator.
    ItemPermanentlyDeleted(Uuid),
}

/// Set of items currently quarantined because they were toxic/unreadable.
#[derive(Clone, Debug, Default)]
pub struct Quarantine {
    toxic: HashSet<Uuid>,
}

impl Quarantine {
    /// Create an empty quarantine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Park an item as toxic/unreadable.
    pub fn mark_toxic(&mut self, uuid: Uuid) {
        self.toxic.insert(uuid);
    }

    /// True if `uuid` is currently quarantined.
    pub fn is_quarantined(&self, uuid: &Uuid) -> bool {
        self.toxic.contains(uuid)
    }

    /// Number of quarantined items (used by tests / diagnostics).
    pub fn len(&self) -> usize {
        self.toxic.len()
    }

    /// True when nothing is quarantined.
    pub fn is_empty(&self) -> bool {
        self.toxic.is_empty()
    }

    /// Remove `uuid` from quarantine (e.g. after it is recovered or deleted).
    pub fn remove(&mut self, uuid: &Uuid) {
        self.toxic.remove(uuid);
    }
}

/// Re-evaluate the quarantine against current server metadata.
///
/// VTRFIX-BUG-H07: absence no longer means "tombstoned". An item the server
/// does not currently advertise stays quarantined (Unknown), and a `fetch`
/// failure skips the cycle entirely. Only an EXPLICIT `Tombstoned` disposition
/// produces `ItemPermanentlyDeleted`.
///
/// `Err` from `fetch_metadata` also means "skip the cycle": nothing is
/// removed, nothing is emitted, and the next tick retries.
pub fn evaluate(
    quarantine: &mut Quarantine,
    server_metadata: &HashMap<Uuid, ItemStatus>,
) -> Vec<ReaperEvent> {
    let mut events = Vec::new();
    // Snapshot to avoid borrowing `quarantine` while mutating it.
    let pending: Vec<Uuid> = quarantine.toxic.iter().copied().collect();
    for uuid in pending {
        match server_metadata.get(&uuid).copied() {
            Some(ItemStatus::Valid) => {
                events.push(ReaperEvent::ItemRecovered(uuid));
                quarantine.remove(&uuid);
            }
            Some(ItemStatus::Tombstoned) => {
                events.push(ReaperEvent::ItemPermanentlyDeleted(uuid));
                quarantine.remove(&uuid);
            }
            // Absent = Unknown: retain, retry next cycle.
            None => continue,
        }
    }
    events
}

/// Interval between quarantine reaper ticks.
pub const QUARANTINE_TICK: Duration = Duration::from_secs(10);

/// Shared quarantine handle + event sink, owned by `vautr-app-state`.
pub type QuarantineHandle = Arc<Mutex<Quarantine>>;

/// Spawn the quarantine reaper loop.
///
/// Every [`QUARANTINE_TICK`] the reaper asks `fetch_metadata` (provided by the
/// caller — typically the sync engine's server status probe) for the current
/// disposition of all known items, runs [`evaluate`], and forwards any
/// [`ReaperEvent`]s over `tx`. The reaper never blocks on the caller: failures
/// of `fetch_metadata` are logged and skipped for that tick.
pub fn spawn_quarantine_reaper(
    quarantine: QuarantineHandle,
    tx: mpsc::Sender<ReaperEvent>,
    fetch_metadata: Arc<dyn Fn() -> Result<HashMap<Uuid, ItemStatus>, String> + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(QUARANTINE_TICK);
        let mut consecutive_failures: u32 = 0;
        loop {
            interval.tick().await;
            // VTRFIX-BUG-H07: a fetch failure skips the whole cycle.
            let meta = match fetch_metadata() {
                Ok(m) => {
                    consecutive_failures = 0;
                    m
                }
                Err(e) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    if consecutive_failures == 5 {
                        tracing::warn!(
                            failures = consecutive_failures,
                            error = %e,
                            "quarantine reaper: 5 consecutive fetch failures"
                        );
                    }
                    continue;
                }
            };
            let events = {
                let mut guard = quarantine.lock().await;
                evaluate(&mut guard, &meta)
            };
            for ev in events {
                if tx.send(ev).await.is_err() {
                    // No live subscriber (UI dropped). Stop the reaper.
                    return;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_map(pairs: &[(Uuid, ItemStatus)]) -> HashMap<Uuid, ItemStatus> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn recovered_item_emits_item_recovered_and_clears_quarantine() {
        // TDD 1: a toxic item that becomes valid on the server triggers
        // ItemRecovered and reappears.
        let id = Uuid::new_v4();
        let mut q = Quarantine::new();
        q.mark_toxic(id);
        assert!(q.is_quarantined(&id));

        let mut meta = status_map(&[(id, ItemStatus::Valid)]);
        let events = evaluate(&mut q, &meta);

        assert_eq!(events, vec![ReaperEvent::ItemRecovered(id)]);
        assert!(
            !q.is_quarantined(&id),
            "resolved item must leave quarantine"
        );
        // Mutating the map afterwards must not re-emit.
        meta.insert(id, ItemStatus::Tombstoned);
        assert!(evaluate(&mut q, &meta).is_empty());
    }

    #[test]
    fn tombstoned_item_emits_item_permanently_deleted() {
        // TDD 2: after tombstoning, the reaper emits ItemPermanentlyDeleted.
        let id = Uuid::new_v4();
        let mut q = Quarantine::new();
        q.mark_toxic(id);

        let events = evaluate(&mut q, &status_map(&[(id, ItemStatus::Tombstoned)]));
        assert_eq!(events, vec![ReaperEvent::ItemPermanentlyDeleted(id)]);
        assert!(!q.is_quarantined(&id));
    }

    #[test]
    fn multiple_items_each_emit_their_own_event() {
        // TDD 3: no batching loss — every recovered/deleted item emits once.
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let mut q = Quarantine::new();
        q.mark_toxic(a);
        q.mark_toxic(b);
        q.mark_toxic(c);

        let meta = status_map(&[
            (a, ItemStatus::Valid),
            (b, ItemStatus::Tombstoned),
            (c, ItemStatus::Valid),
        ]);
        let mut events = evaluate(&mut q, &meta);
        events.sort_by_key(|e| match e {
            ReaperEvent::ItemRecovered(u) => (0u8, *u),
            ReaperEvent::ItemPermanentlyDeleted(u) => (1u8, *u),
        });

        // Order-independent checks: every item emitted exactly once, with the
        // correct variant (no batching loss).
        assert_eq!(events.len(), 3);
        let recovered: HashSet<Uuid> = events
            .iter()
            .filter_map(|e| match e {
                ReaperEvent::ItemRecovered(u) => Some(*u),
                _ => None,
            })
            .collect();
        let deleted: HashSet<Uuid> = events
            .iter()
            .filter_map(|e| match e {
                ReaperEvent::ItemPermanentlyDeleted(u) => Some(*u),
                _ => None,
            })
            .collect();
        assert_eq!(recovered, HashSet::from([a, c]));
        assert_eq!(deleted, HashSet::from([b]));
        assert!(q.is_empty());
    }

    #[test]
    fn no_event_for_already_removed_item() {
        // TDD 4: an item the user already removed locally is not in the
        // quarantine, so the reaper emits nothing for it.
        let id = Uuid::new_v4();
        // Item is NOT marked toxic (user deleted it locally before the reaper
        // ran). Server also tombstoned it.
        let mut q = Quarantine::new();
        assert!(!q.is_quarantined(&id));

        let events = evaluate(&mut q, &status_map(&[(id, ItemStatus::Tombstoned)]));
        assert!(
            events.is_empty(),
            "no event when item was never quarantined"
        );
    }

    #[test]
    fn missing_server_metadata_retains_quarantine() {
        // VTRFIX-BUG-H07: absence is UNKNOWN, not tombstoned. A partial server
        // response (or a page that didn't include the item) must NOT cause
        // mass false-positive deletions.
        let id = Uuid::new_v4();
        let mut q = Quarantine::new();
        q.mark_toxic(id);
        let events = evaluate(&mut q, &status_map(&[]));
        assert!(events.is_empty(), "absent must not emit any event");
        assert!(q.is_quarantined(&id), "absent must retain the quarantine");
    }

    #[test]
    fn explicit_tombstone_emits_delete() {
        let id = Uuid::new_v4();
        let mut q = Quarantine::new();
        q.mark_toxic(id);
        let events = evaluate(&mut q, &status_map(&[(id, ItemStatus::Tombstoned)]));
        assert_eq!(events, vec![ReaperEvent::ItemPermanentlyDeleted(id)]);
        assert!(!q.is_quarantined(&id));
    }
}
