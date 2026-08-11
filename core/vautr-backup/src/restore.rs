//! Restoring a live store from a backup archive (docs/architecture/mlp-scope.md §1).
//!
//! [`restore_to_db`] decrypts an archive and writes the contained SQLite
//! snapshot to a target database file. The **live store is never touched** by
//! this crate: callers pass an explicit destination path, and the one-click
//! restore test ([`crate::restore_test`]) points that at a throwaway scratch DB.

use std::path::{Path, PathBuf};

use sqlx::sqlite::SqlitePool;
use thiserror::Error;

use crate::archive::{self, ArchiveError, ArchivePayload, ArchiveMeta};

/// Outcome of a restore operation (schema validation, not a full data audit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOutcome {
    /// Number of rows in the `items` table after restore.
    pub restored_records: u64,
    /// The manifest recovered from the archive.
    pub manifest: ArchiveMeta,
}

/// Errors produced while restoring a vault from a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RestoreError {
    /// The archive could not be opened / decrypted.
    #[error("archive open failed: {0}")]
    Archive(#[from] ArchiveError),
    /// The payload could not be deserialized.
    #[error("payload deserialization failed")]
    Payload,
    /// Writing the snapshot file failed.
    #[error("snapshot write failed: {0}")]
    Io(String),
    /// The target database could not be opened or validated.
    #[error("database open/validate failed: {0}")]
    Database(String),
}

/// Result alias for restore operations.
pub type RestoreResult<T> = Result<T, RestoreError>;

/// Decrypt an archive and recover the plaintext [`ArchivePayload`].
pub fn decrypt_archive(key: &[u8; 32], archive: &[u8]) -> RestoreResult<ArchivePayload> {
    let plaintext = archive::open(key, archive)?;
    let payload: ArchivePayload =
        serde_json::from_slice(&plaintext).map_err(|_| RestoreError::Payload)?;
    payload.manifest.verify_format()?;
    Ok(payload)
}

/// Write raw SQLite snapshot bytes to a destination file.
pub fn write_snapshot(snapshot: &[u8], dest: &Path) -> RestoreResult<()> {
    std::fs::write(dest, snapshot).map_err(|e| RestoreError::Io(format!("{e}")))
}

/// Allocate a fresh, unique scratch file path under the system temp dir.
pub fn scratch_file() -> RestoreResult<PathBuf> {
    Ok(std::env::temp_dir().join(format!(
        "vautr-restore-test-{}.db",
        uuid::Uuid::new_v4()
    )))
}

/// Open a fresh SQLite pool against a single DB file (max 1 connection so the
/// file is not shadowed by WAL/shm secondary pools).
pub async fn open_scratch_pool(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    let url = format!("sqlite://{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)?.create_if_missing(true).read_only(false);
    SqlitePoolOptions::new().max_connections(1).connect_with(opts).await
}

/// Restore an archive to a target database file and validate it opens cleanly.
///
/// The destination path is caller-controlled; it is **never** derived from the
/// live store. Returns [`RestoreOutcome`] with the recovered manifest and item
/// count, or an error if the archive fails to decrypt / the DB fails to open.
pub async fn restore_to_db(
    key: &[u8; 32],
    archive: &[u8],
    dest: &Path,
) -> RestoreResult<RestoreOutcome> {
    let payload = decrypt_archive(key, archive)?;
    let snapshot = payload
        .snapshot_bytes()
        .map_err(|_| RestoreError::Io("snapshot base64 decode".into()))?;
    write_snapshot(&snapshot, dest)?;

    let pool = open_scratch_pool(dest)
        .await
        .map_err(|e| RestoreError::Database(format!("open: {e}")))?;

    // PRAGMA integrity_check returns one row; a clean DB reports "ok".
    let integrity: Option<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_optional(&pool)
        .await
        .map_err(|e| RestoreError::Database(format!("integrity check: {e}")))?;
    match integrity.as_deref() {
        Some("ok") => {}
        other => {
            return Err(RestoreError::Database(format!(
                "integrity check failed: {other:?}"
            )));
        }
    }

    let restored_records: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_optional(&pool)
        .await
        .map_err(|e| RestoreError::Database(format!("count items: {e}")))?;

    pool.close().await;

    Ok(RestoreOutcome {
        restored_records: restored_records.unwrap_or(0).max(0) as u64,
        manifest: payload.manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveMeta;
    use crate::backup::create_archive;

    async fn sample_archive(key: &[u8; 32]) -> Vec<u8> {
        // Build a real SQLite file first.
        let scratch = scratch_file().unwrap();
        {
            let pool = open_scratch_pool(&scratch).await.unwrap();
            sqlx::query("CREATE TABLE items (uuid TEXT PRIMARY KEY) STRICT")
                .execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO items (uuid) VALUES ('a'), ('b')")
                .execute(&pool).await.unwrap();
            pool.close().await;
        }
        let snapshot = std::fs::read(&scratch).unwrap();
        std::fs::remove_file(&scratch).ok();
        let meta = ArchiveMeta::new("2026-08-11T00:00:00Z", "vault-1", 2);
        create_archive(key, &snapshot, &meta).unwrap()
    }

    #[tokio::test]
    async fn restore_to_db_roundtrips() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let key = [4u8; 32];
        let archive = sample_archive(&key).await;
        let dest = scratch_file().unwrap();
        let outcome = restore_to_db(&key, &archive, &dest).await.unwrap();
        assert_eq!(outcome.restored_records, 2);
        assert_eq!(outcome.manifest.entry_count, 2);
        std::fs::remove_file(&dest).ok();
    }

    #[tokio::test]
    async fn restore_to_db_rejects_wrong_key() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let key = [4u8; 32];
        let archive = sample_archive(&key).await;
        let dest = scratch_file().unwrap();
        let err = restore_to_db(&[5u8; 32], &archive, &dest).await.unwrap_err();
        assert!(matches!(err, RestoreError::Archive(ArchiveError::Decrypt)));
        std::fs::remove_file(&dest).ok();
    }
}
