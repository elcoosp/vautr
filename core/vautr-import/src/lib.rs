//! # vautr-import
//!
//! High-throughput, zero-trust data import for Vautr.
//!
//! Spec: [`docs/architecture/data-import-seeding.md`]. This crate implements the
//! streaming parser, the UUID-mandate translation layer, size-aware parallel
//! encryption (`rayon`), and the bulk SQLite drop/rebuild ingestion fast-path.
//!
//! The parser is zero-trust (§1): imported data is always treated as malformed
//! or hostile, and individual record failures are accumulated in an
//! [`ImportReport`] rather than aborting the whole import.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{DatabaseConnection, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub mod encrypt;
pub mod error;
pub mod ingest;
pub mod parser;
pub mod source;
pub mod translate;

pub use error::{ImportError, ImportFailure, ImportFailureReason, Result};

use crate::encrypt::{encrypt_chunk, plaintext_size, EncryptedItem, MAX_ITEMS_PER_CHUNK, MAX_PLAINTEXT_BYTES};
use crate::ingest::ingest;
use crate::parser::{parse_stream, ParseRecord};
use crate::source::{ImportSource, PathImportSource};
use crate::translate::{translate, TranslatedItem};

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

/// A single raw record yielded by a source parser (per-format). (§2.3)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawImportItem {
    /// The competitor's identifier, preserved only as opaque provenance.
    pub source_id: Option<String>,
    pub title: String,
    /// Primary URL, used (with title) for exact-match dedup.
    pub url: Option<String>,
    /// Format-specific fields (username / password / notes / totp ...).
    pub fields: serde_json::Value,
}

/// The OEK/DEK key pair used to encrypt imported items.
///
/// Derived from the SVK (via `vautr-crypto::key_tree` / `vautr-keyring`).
#[derive(Debug, Clone)]
pub struct VaultKeys {
    /// Overview Encryption Key — encrypts the `DecryptedOverview`.
    pub oek: Zeroizing<[u8; 32]>,
    /// Data Encryption Key — encrypts the `DecryptedSecret` payload.
    pub dek: Zeroizing<[u8; 32]>,
}

impl VaultKeys {
    /// Derive OEK/DEK from an SVK.
    pub fn from_svk(svk: &Zeroizing<[u8; 32]>) -> Result<Self> {
        let oek = vautr_crypto::key_tree::derive_oek(svk)
            .map_err(|e| ImportFailure::Pipeline(format!("oek derive: {e}")))?;
        let dek = vautr_crypto::key_tree::derive_dek(svk)
            .map_err(|e| ImportFailure::Pipeline(format!("dek derive: {e}")))?;
        Ok(Self { oek, dek })
    }

    /// Generate fresh OEK/DEK from a new random SVK (test / standalone use).
    pub fn random() -> Self {
        let (_, oek, dek) = vautr_keyring::svk::new_vault_keys();
        Self { oek, dek }
    }
}

/// Row mirror of the pre-flight dedup query.
#[derive(Debug, FromQueryResult)]
struct DedupRow {
    overview_title: String,
    overview_urls: String,
}

/// Build the exact-match dedup set `(title, url)` from the local DB (§2.3).
/// Only the primary (first) URL is used, mirroring the translation layer.
async fn preflight(db: &DatabaseConnection) -> Result<HashSet<(String, String)>> {
    let sql = "SELECT overview_title, overview_urls FROM item_overviews";
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, []);
    let rows = DedupRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("preflight: {e}")))?;
    let mut set = HashSet::new();
    for r in rows {
        let urls: Vec<String> = serde_json::from_str(&r.overview_urls).unwrap_or_default();
        let first = urls.into_iter().next().unwrap_or_default();
        set.insert((r.overview_title, first));
    }
    Ok(set)
}

/// Current Unix timestamp in seconds.
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Progress-reporting helper that never moves the bar backwards.
struct Progress {
    current: u8,
}
impl Progress {
    fn new() -> Self {
        Self { current: 0 }
    }
    fn report(&mut self, value: u8, cb: &dyn Fn(u8)) {
        if value > self.current {
            self.current = value;
            cb(self.current);
        }
    }
}

/// Run the shared pipeline over a stream of parsed records.
async fn run_pipeline(
    records: Box<dyn Iterator<Item = core::result::Result<ParseRecord, String>>>,
    existing: &mut HashSet<(String, String)>,
    db: &DatabaseConnection,
    keys: &VaultKeys,
    progress: impl Fn(u8),
) -> Result<ImportReport> {
    let mut prog = Progress::new();
    let mut total_parsed: u32 = 0;
    let mut skipped: u32 = 0;
    let mut errors: Vec<ImportError> = Vec::new();

    let mut chunk: Vec<vautr_domain::DomainModel> = Vec::new();
    let mut chunk_bytes: usize = 0;
    let mut encrypted: Vec<EncryptedItem> = Vec::new();

    for record in records {
        total_parsed += 1;

        let parse_result = match record {
            Ok(r) => r,
            Err(e) => {
                // Malformed record: accumulate, keep streaming.
                errors.push(ImportError {
                    line_number: total_parsed,
                    item_identifier: "Record".to_string(),
                    reason: ImportFailureReason::SchemaMismatch(e),
                });
                continue;
            }
        };

        match translate(parse_result.item, existing, parse_result.line_number) {
            Ok(TranslatedItem { domain, source_id: _ }) => {
                let size = plaintext_size(&domain);
                chunk_bytes += size;
                chunk.push(domain);
                if chunk.len() >= MAX_ITEMS_PER_CHUNK || chunk_bytes >= MAX_PLAINTEXT_BYTES {
                    encrypted.extend(
                        encrypt_chunk(std::mem::take(&mut chunk), keys)
                            .map_err(|e| ImportFailure::Pipeline(e.to_string()))?,
                    );
                    chunk_bytes = 0;
                    prog.report(50, &progress);
                }
            }
            Err(e) => {
                // Per-item failures accumulate (duplicates included, so the
                // "View Details" modal in §5.2 can show why items were skipped).
                errors.push(e);
                if errors.last().map(|x| x.reason == ImportFailureReason::DuplicateSkip)
                    == Some(true)
                {
                    skipped += 1;
                }
            }
        }
    }

    // Flush the final partial chunk.
    if !chunk.is_empty() {
        encrypted.extend(
            encrypt_chunk(std::mem::take(&mut chunk), keys)
                .map_err(|e| ImportFailure::Pipeline(e.to_string()))?,
        );
    }

    let success_count = encrypted.len() as u32;
    prog.report(80, &progress);

    ingest(db, encrypted).await?;
    prog.report(100, &progress);

    Ok(ImportReport {
        total_parsed,
        success_count,
        skipped_count: skipped,
        errors,
    })
}

/// Import a file from a supported competitor export format.
///
/// `progress` receives an integer 0–100 mapping the bulk pipeline stages
/// (parse → encrypt → ingest) and is guaranteed non-decreasing.
pub async fn import_file(
    path: &str,
    db: &DatabaseConnection,
    keys: &VaultKeys,
    progress: impl Fn(u8),
) -> Result<ImportReport> {
    let source = PathImportSource::new(path);
    let kind = source.kind();
    let reader = source.reader()?;
    let records = parse_stream(reader, kind)?;
    let mut existing = preflight(db).await?;
    run_pipeline(records, &mut existing, db, keys, progress).await
}

/// Import already-parsed raw items without touching disk.
pub async fn import_items(
    items: Vec<RawImportItem>,
    db: &DatabaseConnection,
    keys: &VaultKeys,
    progress: impl Fn(u8),
) -> Result<ImportReport> {
    let records: Vec<core::result::Result<ParseRecord, String>> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| Ok(ParseRecord {
            line_number: (i + 1) as u32,
            item,
        }))
        .collect();
    let mut existing = preflight(db).await?;
    run_pipeline(Box::new(records.into_iter()), &mut existing, db, keys, progress).await
}
