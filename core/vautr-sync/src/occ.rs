//! OCC conflict resolution. REQ-SYNC-03, REQ-API-02.
//! Maps 412 `precondition_failed` / `version_mismatch` into DashMap updates and
//! the user-facing `ConflictEvent` (data.md §7.2).

use uuid::Uuid;

/// Resolution decision surfaced to the UI for a 412 conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    KeepServer,
    ForceOverwrite,
}

/// A conflict event delivered to the UI (data.md §7.2).
#[derive(Clone, Debug)]
pub struct ConflictEvent {
    pub uuid: Uuid,
    pub local_version: u64,
    pub server_version: u64,
    pub is_toxic: bool,
}

/// Resolve a 412 conflict given the local and server versions.
///
/// Returns the decision plus an event for the UI. `force_overwrite` means the
/// local edit wins (re-push at `server_version + 1`); otherwise the server copy
/// is kept and the local item is reconciled to `server_version`.
pub fn resolve(
    uuid: Uuid,
    local_version: u64,
    server_version: u64,
    is_toxic: bool,
    force_overwrite: bool,
) -> (ConflictResolution, ConflictEvent) {
    let resolution = if force_overwrite {
        ConflictResolution::ForceOverwrite
    } else {
        ConflictResolution::KeepServer
    };
    let event = ConflictEvent {
        uuid,
        local_version,
        server_version,
        is_toxic,
    };
    (resolution, event)
}

/// Decide the next OCC target version for a re-push after a conflict.
/// The client must push at `server_version` (the version the server currently
/// holds) so its update is applied as the new `server_version + 1`.
pub fn next_push_version(server_version: u64) -> u64 {
    server_version
}
