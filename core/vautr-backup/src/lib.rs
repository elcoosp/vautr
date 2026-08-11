//! # vautr-backup
//!
//! Smart automatic backups and the **one-click restore test** for Vautr
//! (docs/architecture/mlp-scope.md §1).
//!
//! Vault backups are encrypted end-to-end, so "restore-test" means the operator
//! can prove, at any time, that an existing archive actually decrypts and yields
//! a consistent vault snapshot — without clobbering the live store.
//!
//! ## Module layout
//! - [`archive`] — the encrypted on-disk archive envelope, manifest, and integrity.
//! - [`backup`] — producing encrypted backup archives from a consistent snapshot.
//! - [`restore`] — restoring a target store from an archive (caller-chosen path).
//! - [`restore_test`] — the one-click restore test (restore to a sandbox, verify,
//!   discard) without touching the live store.

pub mod archive;
pub mod backup;
pub mod restore;
pub mod restore_test;

pub use archive::{ArchiveError, ArchiveMeta, ArchivePayload, ArchiveResult};
pub use backup::{BackupError, BackupResult};
pub use restore::{RestoreError, RestoreOutcome, RestoreResult};
pub use restore_test::{RestoreTestError, RestoreTestOutcome, RestoreTestResult};

/// Test-only support: a crate-wide lock that serializes the unit tests which
/// create/remove scratch SQLite files under the shared system temp dir. Without
/// it, a cleanup-assertion test can delete a scratch file a concurrent test is
/// still reading (the "leaves no scratch behind" test) or a test can observe a
/// transient file from a peer. `tokio::sync::Mutex` (not `std`) so the guard is
/// `Send` across `.await` points in async tests.
#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) static SCRATCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
}
