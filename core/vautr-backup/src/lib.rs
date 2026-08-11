//! # vautr-backup
//!
//! Smart automatic backups and the **one-click restore test** for Vautr
//! (docs/architecture/mlp-scope.md §1).
//!
//! Vault backups are encrypted end-to-end, so "restore-test" means the operator
//! can prove, at any time, that an existing archive actually decrypts and yields
//! a consistent vault snapshot — without clobbering the live store.
//!
//! ## Module layout (Wave A5 will fill these in)
//! - [`archive`] — the on-disk archive format, manifest, and integrity checks.
//! - [`backup`] — producing encrypted backup archives on a schedule.
//! - [`restore`] — restoring a live store from an archive.
//! - [`restore_test`] — the one-click restore test (restore to a sandbox, verify,
//!   discard).
//!
//! **Scaffold note (Wave 0.3):** every entry point is a safe, non-panicking stub.
//! The `no todo!()` roadmap rule is honored throughout; real logic lands in Wave A5.

pub mod archive;
pub mod backup;
pub mod restore;
pub mod restore_test;

pub use archive::{ArchiveError, ArchiveMeta, ArchiveResult};
pub use backup::BackupError;
pub use restore::RestoreError;
pub use restore_test::RestoreTestError;
