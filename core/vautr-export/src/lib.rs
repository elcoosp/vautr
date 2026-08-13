//! # vautr-export
//!
//! Offline, streaming plaintext export (CSV/JSON) for Vautr vaults (VTR-058).
//!
//! The pipeline is fully offline: it takes already-decrypted [`row::ExportRow`]s
//! (the orchestrator reveals + releases each secret to build them) and writes
//! them to disk one row at a time, so it never loads the whole vault into memory
//! and never makes a network call.
//!
//! See [`export::export_rows`] for the streaming writer and [`error::ExportError`]
//! for the `EpochMismatch` gate failure.

pub mod error;
pub mod export;
pub mod format;
pub mod report;
pub mod row;

pub use error::ExportError;
pub use export::{cancel_flag, count_total, export_rows};
pub use format::ExportFormat;
pub use report::ExportReport;
pub use row::{ExportRow, TotpExport};
