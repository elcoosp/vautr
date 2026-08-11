//! # vautr-import
//!
//! High-throughput, zero-trust data import for Vautr.
//!
//! Spec: [`docs/architecture/data-import-seeding.md`]. This crate implements the
//! streaming parser, the UUID-mandate translation layer, size-aware parallel
//! encryption (`rayon`), and the bulk SQLite drop/rebuild ingestion fast-path.
//!
//! **Scaffold state:** all entry points are safe, non-panicking stubs that
//! return [`ImportFailure::NotImplemented`]. The `no todo!()` roadmap rule is
//! honored: stubs return `Err(...)`, never panic.
//!
//! Per §1 of the spec, the parser is zero-trust: imported data is always treated
//! as malformed or hostile, and individual record failures are accumulated in an
//! [`ImportReport`] rather than aborting the whole import.

use serde::{Deserialize, Serialize};

pub mod error;

pub use error::{ImportError, ImportFailure, ImportFailureReason, Result};

/// Strongly-typed result of a completed import. (§5.1)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    /// Total records read from the source stream.
    pub total_parsed: u32,
    /// Records successfully imported.
    pub success_count: u32,
    /// Records skipped (duplicates, or user-selected overwrite/skip).
    pub skipped_count: u32,
    /// Per-record failures, for the "View Details" modal.
    pub errors: Vec<ImportError>,
}

/// A single raw record yielded by a source parser (per-format).
///
/// Scaffold placeholder for the `RawImportItem` type described in §2.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawImportItem {
    /// The competitor's identifier, preserved only as opaque provenance.
    pub source_id: Option<String>,
    pub title: String,
    pub fields: serde_json::Value,
}

/// Import a file from a supported competitor export format.
///
/// # Stub
/// Returns [`ImportFailure::NotImplemented`] until Wave B1 lands. This is a
/// safe, non-panicking placeholder.
pub fn import_file(_path: &str) -> Result<ImportReport> {
    Err(ImportFailure::NotImplemented)
}

/// Import raw items (already extracted/decrypted) without touching disk.
///
/// # Stub
/// Returns [`ImportFailure::NotImplemented`] until Wave B1 lands.
pub fn import_items(_items: Vec<RawImportItem>) -> Result<ImportReport> {
    Err(ImportFailure::NotImplemented)
}
