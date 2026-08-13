//! Error types for the Vautr offline export pipeline (VTR-058).

use thiserror::Error;

/// Why an export failed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The vault is locked or in a Read-Only gate. Export requires a live,
    /// writable vault (re-authentication), per VTR-058: the export is blocked
    /// and reported as an epoch mismatch.
    #[error("export blocked: vault locked or in read-only gate (EpochMismatch)")]
    EpochMismatch(String),

    /// An I/O error writing the export file.
    #[error("export io error: {0}")]
    Io(String),

    /// A row could not be serialized.
    #[error("export serialize error: {0}")]
    Serialize(String),

    /// The user cancelled the export mid-stream (progress/cancel flag).
    #[error("export cancelled by user")]
    Cancelled,
}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for ExportError {
    fn from(e: serde_json::Error) -> Self {
        ExportError::Serialize(e.to_string())
    }
}

impl From<csv::Error> for ExportError {
    fn from(e: csv::Error) -> Self {
        ExportError::Serialize(e.to_string())
    }
}
