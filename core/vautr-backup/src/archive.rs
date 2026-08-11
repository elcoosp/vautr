//! On-disk backup archive format, manifest, and integrity checks.
//!
//! Wave A5 defines the concrete binary/encrypted format. Wave 0.3 provides the
//! versioned [`ArchiveMeta`] manifest shape and a safe integrity gate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while reading or validating a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArchiveError {
    /// The archive's declared format version is not understood.
    #[error("archive format version {0} is not supported")]
    UnsupportedVersion(u32),
    /// An archive integrity check failed.
    #[error("archive integrity check failed: {0}")]
    Integrity(&'static str),
}

/// Result alias for archive operations.
pub type ArchiveResult<T> = Result<T, ArchiveError>;

/// The highest archive format version this crate understands.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// Lightweight, serializable archive manifest (PII-free; entry payloads are
/// encrypted separately).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMeta {
    /// Archive format version (see [`CURRENT_FORMAT_VERSION`]).
    pub format_version: u32,
    /// RFC3339-ish timestamp of when the archive was produced.
    pub created_at: String,
    /// Logical id of the vault this archive is a snapshot of.
    pub vault_id: String,
    /// Number of (encrypted) entries recorded in the archive.
    pub entry_count: u64,
}

impl ArchiveMeta {
    /// Construct a manifest from its fields.
    pub fn new(created_at: impl Into<String>, vault_id: impl Into<String>, entry_count: u64) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            created_at: created_at.into(),
            vault_id: vault_id.into(),
            entry_count,
        }
    }

    /// Reject archives from a future/incompatible format version.
    ///
    /// Safe stub: returns [`ArchiveError::Integrity`] when the format version is
    /// beyond what this build understands.
    pub fn verify_format(&self) -> ArchiveResult<()> {
        if self.format_version > CURRENT_FORMAT_VERSION {
            Err(ArchiveError::UnsupportedVersion(self.format_version))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_with_version(v: u32) -> ArchiveMeta {
        ArchiveMeta {
            format_version: v,
            created_at: "2026-08-11T00:00:00Z".to_string(),
            vault_id: "vault-1".to_string(),
            entry_count: 0,
        }
    }

    #[test]
    fn current_version_is_accepted() {
        assert_eq!(meta_with_version(CURRENT_FORMAT_VERSION).verify_format(), Ok(()));
    }

    #[test]
    fn future_version_is_rejected() {
        assert_eq!(
            meta_with_version(CURRENT_FORMAT_VERSION + 1).verify_format(),
            Err(ArchiveError::UnsupportedVersion(CURRENT_FORMAT_VERSION + 1))
        );
    }

    #[test]
    fn manifest_roundtrips_through_json() {
        let meta = meta_with_version(CURRENT_FORMAT_VERSION);
        let json = serde_json::to_string(&meta).expect("serialize");
        let back: ArchiveMeta = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, meta);
    }
}
