//! The **one-click restore test** for Vautr (docs/architecture/mlp-scope.md §1).
//!
//! The operator proves an archive still decrypts and yields a consistent vault
//! snapshot by restoring it into a throwaway sandbox and verifying the result,
//! **without touching the live store**. This is exactly the flow exposed by the
//! server's `POST /backup/restore` endpoint.
//!
//! Isolation guarantees:
//! - The snapshot is written to a fresh scratch file under the temp dir, never
//!   to the live DB path.
//! - The scratch DB is opened with its own single-connection pool.
//! - The scratch file is removed after validation.
//! - The live store is never opened by this module.

use std::path::PathBuf;

use thiserror::Error;

use crate::archive::ArchiveMeta;
use crate::restore::{self, open_scratch_pool, RestoreError};

/// Result of running the one-click restore test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreTestOutcome {
    /// Whether the archive decrypted and the restored scratch DB validated.
    pub passed: bool,
    /// Number of rows restored into the scratch DB (`items` table).
    pub restored_records: u64,
    /// The recovered manifest (present when decryption succeeded).
    pub manifest: Option<ArchiveMeta>,
    /// Human-readable detail (why it passed / failed).
    pub detail: String,
}

/// Errors that indicate the restore test could not *run* (infrastructure), as
/// opposed to the archive failing verification (which yields a `passed: false`
/// outcome).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RestoreTestError {
    /// Could not set up or tear down the scratch sandbox.
    #[error("restore test setup failed: {0}")]
    Setup(String),
    /// The underlying restore step failed unexpectedly.
    #[error("restore failed: {0}")]
    Restore(#[from] RestoreError),
}

/// Result alias for restore-test operations.
pub type RestoreTestResult<T> = Result<T, RestoreTestError>;

/// Run the one-click restore test against an archive: decrypt, write to a
/// scratch DB, validate, discard. Returns a [`RestoreTestOutcome`] describing
/// pass/fail. The live store is never touched.
pub async fn run_restore_test(key: &[u8; 32], archive: &[u8]) -> RestoreTestResult<RestoreTestOutcome> {
    // Decrypt first. A decrypt failure is a *failed test*, not a crash.
    let payload = match restore::decrypt_archive(key, archive) {
        Ok(p) => p,
        Err(e) => {
            return Ok(RestoreTestOutcome {
                passed: false,
                restored_records: 0,
                manifest: None,
                detail: format!("archive could not be decrypted: {e}"),
            });
        }
    };

    let scratch: PathBuf = restore::scratch_file().map_err(RestoreTestError::from)?;
    let mut outcome = validate_in_scratch(&payload.snapshot_b64, &scratch).await?;
    let _ = std::fs::remove_file(&scratch);
    outcome.manifest = Some(payload.manifest.clone());
    Ok(outcome)
}

/// Write the snapshot to `scratch` and validate it opens as a consistent DB.
async fn validate_in_scratch(
    snapshot_b64: &str,
    scratch: &std::path::Path,
) -> RestoreTestResult<RestoreTestOutcome> {
    use base64::Engine;
    let snapshot = match base64::engine::general_purpose::STANDARD.decode(snapshot_b64) {
        Ok(b) => b,
        Err(_) => {
            return Ok(RestoreTestOutcome {
                passed: false,
                restored_records: 0,
                manifest: None,
                detail: "snapshot base64 is corrupt".to_string(),
            });
        }
    };

    restore::write_snapshot(&snapshot, scratch)
        .map_err(RestoreTestError::from)
        .map_err(|_| RestoreTestError::Setup("write scratch DB".into()))?;

    let pool = open_scratch_pool(scratch)
        .await
        .map_err(|e| RestoreTestError::Setup(format!("open scratch: {e}")))?;

    let integrity: Option<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_optional(&pool)
        .await
        .map_err(|e| RestoreTestError::Setup(format!("integrity check: {e}")))?;

    let restored_records: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_optional(&pool)
        .await
        .map_err(|e| RestoreTestError::Setup(format!("count items: {e}")))?;

    pool.close().await;

    let restored_records = restored_records.unwrap_or(0).max(0) as u64;

    if integrity.as_deref() != Some("ok") {
        return Ok(RestoreTestOutcome {
            passed: false,
            restored_records,
            manifest: None,
            detail: format!("integrity check reported: {integrity:?}"),
        });
    }

    Ok(RestoreTestOutcome {
        passed: true,
        restored_records,
        manifest: None,
        detail: "decrypted and validated in scratch DB".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveMeta;
    use crate::backup::create_archive;

    async fn sample_archive(key: &[u8; 32], entries: &[&str]) -> Vec<u8> {
        let scratch = restore::scratch_file().unwrap();
        {
            let pool = open_scratch_pool(&scratch).await.unwrap();
            sqlx::query("CREATE TABLE items (uuid TEXT PRIMARY KEY) STRICT")
                .execute(&pool).await.unwrap();
            for e in entries {
                sqlx::query("INSERT INTO items (uuid) VALUES (?)")
                    .bind(e).execute(&pool).await.unwrap();
            }
            pool.close().await;
        }
        let snapshot = std::fs::read(&scratch).unwrap();
        std::fs::remove_file(&scratch).ok();
        let meta = ArchiveMeta::new("2026-08-11T00:00:00Z", "vault-1", entries.len() as u64);
        create_archive(key, &snapshot, &meta).unwrap()
    }

    #[tokio::test]
    async fn restore_test_passes_on_valid_archive() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let key = [3u8; 32];
        let archive = sample_archive(&key, &["a", "b", "c"]).await;
        let outcome = run_restore_test(&key, &archive).await.unwrap();
        assert!(outcome.passed, "detail: {}", outcome.detail);
        assert_eq!(outcome.restored_records, 3);
    }

    #[tokio::test]
    async fn restore_test_fails_on_wrong_key() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let key = [3u8; 32];
        let archive = sample_archive(&key, &["a"]).await;
        let outcome = run_restore_test(&[9u8; 32], &archive).await.unwrap();
        assert!(!outcome.passed);
        assert!(outcome.detail.contains("decrypt"));
    }

    #[tokio::test]
    async fn restore_test_leaves_no_scratch_behind() {
        let _g = crate::test_support::SCRATCH_LOCK.lock().await;
        let key = [3u8; 32];
        let archive = sample_archive(&key, &["x"]).await;
        let temp = std::env::temp_dir();
        // Clear any leftovers from prior crashed runs so this assertion is about
        // the run we just performed, not the whole temp dir history.
        for entry in std::fs::read_dir(&temp).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with("vautr-restore-test-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let _ = run_restore_test(&key, &archive).await.unwrap();
        // The scratch files use a unique vautr-restore-test- prefix; assert the
        // temp dir holds none with our prefix right now.
        let leftovers: Vec<_> = std::fs::read_dir(&temp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().starts_with("vautr-restore-test-")
            })
            .collect();
        assert!(leftovers.is_empty(), "scratch DBs left behind: {leftovers:?}");
    }
}
