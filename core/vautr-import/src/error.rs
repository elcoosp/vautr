//! Error types for the Vautr import pipeline.
//!
//! Maps directly to [`docs/architecture/data-import-seeding.md`] §5. The per-item
//! [`ImportError`] struct is collected into [`ImportReport`]. The top-level
//! [`ImportFailure`] enum is what a pipeline call returns on a non-recoverable
//! (or, in the scaffold, unimplemented) failure.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Why a single imported record failed. (§5.1)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportFailureReason {
    /// The record did not match the expected schema for the source format.
    SchemaMismatch(String),
    /// The record parsed but failed semantic validation (e.g. bad TOTP).
    ValidationFailed(String),
    /// A mandatory field (e.g. title) was absent.
    MissingRequiredField(String),
    /// Skipped because an exact Title/URL match already exists in the vault.
    DuplicateSkip,
    /// The competitor master password provided was wrong (decryption failed).
    DecryptionFailed,
}

/// A single record that could not be imported. (§5.1)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportError {
    /// 1-based line/record number in the source stream.
    pub line_number: u32,
    /// Human-readable identifier, e.g. `"Title: Bank of America"`.
    pub item_identifier: String,
    /// Why the record failed.
    pub reason: ImportFailureReason,
}

/// Top-level error returned by the import pipeline.
///
/// This is deliberately a *separate* type from the per-item [`ImportError`]
/// struct above to avoid a name collision while still exposing a single
/// recoverable-`Err` surface for the caller.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ImportFailure {
    /// The import pipeline is not implemented yet. Safe, non-panicking stub.
    #[error("import pipeline not implemented yet")]
    NotImplemented,

    /// A fatal, non-per-item failure aborted the whole import.
    #[error("import aborted: {0}")]
    Pipeline(String),
}

/// Result alias for `vautr-import`.
pub type Result<T> = core::result::Result<T, ImportFailure>;
