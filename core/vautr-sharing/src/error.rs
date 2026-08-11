//! Error types for the Vautr sharing crate.
//!
//! Maps to [`docs/architecture/sharing-pki.md`]. All flow entry points return a
//! safe, non-panicking `Err(ShareError::NotImplemented)` stub until the sharing
//! pipeline is implemented.

use thiserror::Error;

/// Errors produced by `vautr-sharing`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShareError {
    /// The operation is not implemented yet. Safe, non-panicking stub.
    #[error("sharing operation not implemented yet")]
    NotImplemented,

    /// The recipient's public key could not be resolved/verified.
    #[error("recipient public key unavailable: {0}")]
    RecipientKeyUnavailable(String),

    /// A KEM/DEM step failed (wrong key, tampered blob, etc.).
    #[error("share crypto failure: {0}")]
    Crypto(String),

    /// The share does not exist or the caller is not authorized.
    #[error("share not found or forbidden")]
    NotFound,

    /// Group operation failed (member missing, admin-only action, etc.).
    #[error("group error: {0}")]
    Group(String),
}

/// Result alias for `vautr-sharing`.
pub type Result<T> = core::result::Result<T, ShareError>;
