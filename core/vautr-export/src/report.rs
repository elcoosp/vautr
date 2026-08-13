//! Outcome of an export run (VTR-058).

/// Summary returned after an export completes (or is cancelled mid-stream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    /// Number of items actually written to the file.
    pub items_exported: u64,
    /// Total bytes written to disk.
    pub bytes_written: u64,
    /// The format that was written.
    pub format: crate::format::ExportFormat,
    /// The destination path.
    pub path: std::path::PathBuf,
    /// True when the user cancelled before all items were written.
    pub cancelled: bool,
}
