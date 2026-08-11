//! Restoring a live store from a backup archive.
//!
//! Safe stub (Wave 0.3): the real decryption + apply pipeline lands in Wave A5.

use thiserror::Error;

/// Errors produced while restoring a vault from a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RestoreError {
    /// Restore is not wired yet.
    #[error("restore is not yet implemented (Wave A5)")]
    NotYetImplemented,
}

/// Result alias for restore operations.
pub type RestoreResult<T> = Result<T, RestoreError>;

/// Restore a live vault store from a backup archive.
///
/// Safe stub (Wave 0.3): always returns [`RestoreError::NotYetImplemented`].
pub fn restore_from_archive() -> RestoreResult<()> {
    Err(RestoreError::NotYetImplemented)
}
