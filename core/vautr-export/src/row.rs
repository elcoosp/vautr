//! A single decrypted, export-ready vault item (VTR-058).
//!
//! The orchestrator builds these one at a time by revealing + immediately
//! releasing each secret, so the plaintext never lives longer than necessary
//! and is never retained by the export crate.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

/// TOTP parameters as exported in plaintext (the secret is included so the
/// user can re-import elsewhere). Not PII beyond what the user already owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpExport {
    pub algorithm: String,
    pub digits: u8,
    pub period: u8,
    /// Base32-encoded shared secret.
    pub secret_base32: Zeroizing<String>,
}

/// One vault item, fully decrypted and ready to write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRow {
    pub uuid: Uuid,
    pub title: String,
    /// Username / subtitle (login identifier).
    pub username: String,
    pub password: Zeroizing<String>,
    /// One or more associated URLs.
    pub urls: Vec<String>,
    pub notes: Zeroizing<String>,
    pub totp: Option<TotpExport>,
}

impl ExportRow {
    /// A stable, human-readable identifier for progress/diagnostics.
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "untitled"
        } else {
            &self.title
        }
    }
}
