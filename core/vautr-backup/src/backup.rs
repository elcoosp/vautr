//! Producing encrypted backup archives on a schedule.
//!
//! Safe stub (Wave 0.3): the real snapshot/encryption pipeline lands in Wave A5.

use thiserror::Error;

/// Errors produced while creating a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BackupError {
    /// Backup creation is not wired yet.
    #[error("backup creation is not yet implemented (Wave A5)")]
    NotYetImplemented,
}

/// Result alias for backup operations.
pub type BackupResult<T> = Result<T, BackupError>;

/// Create an encrypted snapshot of the vault.
///
/// Safe stub (Wave 0.3): always returns [`BackupError::NotYetImplemented`].
pub fn create_snapshot() -> BackupResult<()> {
    Err(BackupError::NotYetImplemented)
}
