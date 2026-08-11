//! The **one-click restore test** for Vautr (docs/architecture/mlp-scope.md §1).
//!
//! The operator proves an archive still decrypts and yields a consistent vault
//! snapshot by restoring it into a throwaway sandbox and verifying the result,
//! without touching the live store.
//!
//! Safe stub (Wave 0.3): the real sandbox/verify/discard pipeline lands in Wave A5.

use thiserror::Error;

/// Errors produced while running the one-click restore test.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RestoreTestError {
    /// The restore test is not wired yet.
    #[error("restore test is not yet implemented (Wave A5)")]
    NotYetImplemented,
}

/// Result alias for restore-test operations.
pub type RestoreTestResult<T> = Result<T, RestoreTestError>;

/// Run the one-click restore test against an archive.
///
/// Safe stub (Wave 0.3): always returns [`RestoreTestError::NotYetImplemented`].
pub fn run_restore_test() -> RestoreTestResult<()> {
    Err(RestoreTestError::NotYetImplemented)
}
