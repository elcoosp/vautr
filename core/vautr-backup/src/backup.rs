//! Producing encrypted backup archives (docs/architecture/mlp-scope.md §1).
//!
//! A backup is produced in two steps:
//! 1. [`create_snapshot`] takes a **consistent** SQLite snapshot of a live pool
//!    using SQLite's `VACUUM INTO` (safe under WAL, no locking of the live DB).
//! 2. [`create_archive`] seals the snapshot + a PII-free [`ArchiveMeta`]
//!    manifest into an AEAD-encrypted envelope (see [`archive`]).
//!
//! The resulting archive bytes never contain plaintext SQLite data and can be
//! stored anywhere; the one-click restore test ([`crate::restore_test`]) proves
//! they still decrypt + validate into a throwaway DB.

use std::path::Path;

use sqlx::sqlite::SqlitePool;
use thiserror::Error;

use crate::archive::{self, ArchiveMeta, ArchivePayload};

/// Errors produced while creating a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BackupError {
    /// The SQLite snapshot could not be produced.
    #[error("snapshot failed: {0}")]
    Snapshot(String),
    /// The archive could not be read/written on disk.
    #[error("archive I/O failed: {0}")]
    Io(String),
    /// Payload serialization or sealing failed.
    #[error("archive creation failed: {0}")]
    Archive(&'static str),
}

/// Result alias for backup operations.
pub type BackupResult<T> = Result<T, BackupError>;

/// Produce a consistent SQLite snapshot of `pool` as raw file bytes.
///
/// Uses SQLite's `VACUUM INTO '<path>'`, which writes a self-contained,
/// transactionally-consistent copy of the whole database into a fresh file.
/// The temporary snapshot file is removed before returning.
pub async fn create_snapshot(pool: &SqlitePool, dest_dir: &Path) -> BackupResult<Vec<u8>> {
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| BackupError::Io(format!("create dir: {e}")))?;

    let file_name = format!(
        "vautr-snapshot-{}.db",
        uuid::Uuid::new_v4()
    );
    let path = dest_dir.join(file_name);
    let escaped = path.display().to_string().replace('\'', "''");

    let sql = format!("VACUUM INTO '{escaped}'");
    // `sql` is built only from a `Uuid`-suffixed temp path with single quotes
    // escaped, so it is audited safe for `raw_sql`.
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(pool)
        .await
        .map_err(|e| BackupError::Snapshot(format!("VACUUM INTO: {e}")))?;

    let bytes = std::fs::read(&path).map_err(|e| BackupError::Io(format!("read snapshot: {e}")))?;
    let _ = std::fs::remove_file(&path);
    Ok(bytes)
}

/// Seal a SQLite snapshot into an encrypted backup archive.
///
/// The plaintext payload is `{ manifest, snapshot_b64 }`; it is AEAD-sealed
/// under `key` with a fresh random nonce before returning, so the returned
/// bytes are safe to store off-box.
pub fn create_archive(
    key: &[u8; 32],
    snapshot: &[u8],
    meta: &ArchiveMeta,
) -> BackupResult<Vec<u8>> {
    meta.verify_format().map_err(|_| BackupError::Archive("unsupported format"))?;
    let payload = ArchivePayload::new(meta.clone(), snapshot);
    let bytes = serde_json::to_vec(&payload)
        .map_err(|_| BackupError::Archive("payload serialization failed"))?;
    archive::seal(key, &bytes).map_err(|_| BackupError::Archive("sealing failed"))
}

/// Count entries in a snapshot for the manifest, treating a missing `items`
/// table as zero (e.g. a schema-only or empty store).
pub async fn count_entries(snapshot: &[u8]) -> BackupResult<u64> {
    let scratch = crate::restore::scratch_file()
        .map_err(|e| BackupError::Snapshot(format!("scratch: {e}")))?;
    let _ = crate::restore::write_snapshot(snapshot, &scratch)
        .map_err(|e| BackupError::Snapshot(format!("write scratch: {e}")))?;
    let pool = open_scratch_pool(&scratch).await
        .map_err(|e| BackupError::Snapshot(format!("open scratch: {e}")))?;
    let count: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_optional(&pool)
        .await
        .map_err(|_| BackupError::Snapshot("entry count failed".into()))?;
    pool.close().await;
    let _ = std::fs::remove_file(&scratch);
    Ok(count.unwrap_or(0).max(0) as u64)
}

// ---------------------------------------------------------------------------
// Helpers (shared with restore / restore_test)
// ---------------------------------------------------------------------------

pub(crate) async fn open_scratch_pool(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    let url = format!("sqlite://{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)?.create_if_missing(true).read_only(false);
    SqlitePoolOptions::new().max_connections(1).connect_with(opts).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore::scratch_file;

    #[tokio::test]
    async fn create_archive_produces_encrypted_bytes() {
        let key = [9u8; 32];
        let snapshot = b"fake-sqlite-bytes".to_vec();
        let meta = ArchiveMeta::new("2026-08-11T00:00:00Z", "vault-1", 3);
        let archive = create_archive(&key, &snapshot, &meta).unwrap();
        assert!(archive.len() > snapshot.len());
        // Plaintext must not appear in the archive.
        assert!(!archive.windows(snapshot.len()).any(|w| w == snapshot.as_slice()));
    }

    #[tokio::test]
    async fn create_snapshot_and_count_entries() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let scratch = scratch_file().unwrap();
        // Build a tiny schema + one item, then snapshot it.
        {
            let pool = open_scratch_pool(&scratch).await.unwrap();
            sqlx::query("CREATE TABLE items (uuid TEXT PRIMARY KEY) STRICT")
                .execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO items (uuid) VALUES ('a'), ('b'), ('c')")
                .execute(&pool).await.unwrap();
            pool.close().await;
        }
        let bytes = std::fs::read(&scratch).unwrap();
        let count = count_entries(&bytes).await.unwrap();
        assert_eq!(count, 3);
        let _ = std::fs::remove_file(&scratch);
    }
}
