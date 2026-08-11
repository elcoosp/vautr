//! In-memory DashMap blacklist (LocalBlacklist). ADR-002.
//! Bounds payload downloads for toxic/ignored items. Persisted to SQLite at
//! end of every sync session (db-contract §5.3) via `vautr_db::txn::persist_dashmap_txn`.

use dashmap::DashMap;
use uuid::Uuid;

/// Per-entry blacklist state (data.md §7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashMapEntryState {
    ToxicIgnored,
    ValidIgnored,
}

impl DashMapEntryState {
    /// Persisted string form.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            DashMapEntryState::ToxicIgnored => "ToxicIgnored",
            DashMapEntryState::ValidIgnored => "ValidIgnored",
        }
    }

    /// Parse from the DB string form.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "ToxicIgnored" => Some(DashMapEntryState::ToxicIgnored),
            "ValidIgnored" => Some(DashMapEntryState::ValidIgnored),
            _ => None,
        }
    }
}

/// In-memory blacklist. O(1) lookups, lock-free sharding.
/// Value = `(ignored_version, state)`.
pub struct LocalBlacklist {
    pub entries: DashMap<Uuid, (i64, DashMapEntryState)>,
}

impl LocalBlacklist {
    /// Empty blacklist.
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// True if `uuid` is blacklisted (toxic or valid-ignored).
    pub fn is_ignored(&self, uuid: &Uuid) -> bool {
        self.entries.contains_key(uuid)
    }

    /// True only for toxic items (payload must never be downloaded).
    pub fn is_toxic(&self, uuid: &Uuid) -> bool {
        self.entries
            .get(uuid)
            .map(|e| e.1 == DashMapEntryState::ToxicIgnored)
            .unwrap_or(false)
    }

    /// Insert/replace an entry.
    pub fn insert(&self, uuid: Uuid, ignored_version: i64, state: DashMapEntryState) {
        self.entries.insert(uuid, (ignored_version, state));
    }

    /// Remove an entry.
    pub fn remove(&self, uuid: &Uuid) -> Option<(i64, DashMapEntryState)> {
        self.entries.remove(uuid).map(|(_, v)| v)
    }

    /// Snapshot to `(uuid, ignored_version, state)` tuples for persistence.
    pub fn snapshot(&self) -> Vec<(Uuid, i64, DashMapEntryState)> {
        self.entries
            .iter()
            .map(|e| (*e.key(), e.value().0, e.value().1))
            .collect()
    }

    /// Replace the whole in-memory set from a persisted snapshot (crash recovery).
    pub fn load(&self, rows: Vec<(Uuid, i64, DashMapEntryState)>) {
        self.entries.clear();
        for (uuid, v, s) in rows {
            self.entries.insert(uuid, (v, s));
        }
    }
}

impl Default for LocalBlacklist {
    fn default() -> Self {
        Self::new()
    }
}
