//! `backup` repository domain: backup state, encryption key, and run history
//! (schema 0011_backup.sql). Backed by the server pool; the live store is the
//! source of the snapshot but is never modified by backup bookkeeping.

use rand::RngCore;

use crate::repository::Repository;

/// Snapshot of backup configuration + last-run state (singleton row, id = 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupState {
    pub enabled: bool,
    pub location: Option<String>,
    pub schedule: String,
    pub last_backup_at: Option<i64>,
    pub last_backup_size_bytes: Option<i64>,
    pub last_restore_test_at: Option<i64>,
    pub last_restore_test_status: Option<String>,
}

/// A recorded backup archive (backup_runs row). `archive_path` points at the
/// sealed archive on disk, retrievable by `backup_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRun {
    pub id: String,
    pub created_at: i64,
    pub size_bytes: i64,
    pub checksum: String,
    pub archive_path: String,
    pub vault_id: String,
    pub entry_count: i64,
}

impl Repository {
    /// Return the persisted backup encryption key, generating + storing a fresh
    /// 32-byte random key on first use. The key seals/opens all archives.
    pub async fn get_or_create_backup_key(&self) -> Result<[u8; 32], sqlx::Error> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT key FROM backup_key WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        if let Some((bytes,)) = row {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Ok(key);
            }
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        sqlx::query("INSERT INTO backup_key (id, key) VALUES (1, ?)")
            .bind(&key[..])
            .execute(&self.pool)
            .await?;
        Ok(key)
    }

    /// Read the singleton backup state, creating the default row if missing.
    pub async fn backup_state(&self) -> Result<BackupState, sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO backup_state (id) VALUES (1)")
            .execute(&self.pool)
            .await?;
        let row: (
            i64,
            Option<String>,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT enabled, location, schedule, last_backup_at, last_backup_size_bytes, \
                 last_restore_test_at, last_restore_test_status FROM backup_state WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(BackupState {
            enabled: row.0 != 0,
            location: row.1,
            schedule: row.2,
            last_backup_at: row.3,
            last_backup_size_bytes: row.4,
            last_restore_test_at: row.5,
            last_restore_test_status: row.6,
        })
    }

    /// Persist the configured backup location (directory archives are written to).
    pub async fn set_backup_location(&self, location: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO backup_state (id, location) VALUES (1, ?) \
             ON CONFLICT(id) DO UPDATE SET location = excluded.location",
        )
        .bind(location)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a completed backup run (insert row + update last-run state).
    pub async fn record_backup_run(&self, run: &BackupRun, now_ms: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO backup_runs (id, created_at, size_bytes, checksum, archive_path, vault_id, entry_count) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(run.created_at)
        .bind(run.size_bytes)
        .bind(&run.checksum)
        .bind(&run.archive_path)
        .bind(&run.vault_id)
        .bind(run.entry_count)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE backup_state SET last_backup_at = ?, last_backup_size_bytes = ? WHERE id = 1",
        )
        .bind(now_ms)
        .bind(run.size_bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up a recorded backup run by id (to resolve an archive on disk).
    pub async fn get_backup_run(&self, id: &str) -> Result<Option<BackupRun>, sqlx::Error> {
        let row: Option<(String, i64, i64, String, String, String, i64)> = sqlx::query_as(
            "SELECT id, created_at, size_bytes, checksum, archive_path, vault_id, entry_count \
             FROM backup_runs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| BackupRun {
            id: r.0,
            created_at: r.1,
            size_bytes: r.2,
            checksum: r.3,
            archive_path: r.4,
            vault_id: r.5,
            entry_count: r.6,
        }))
    }

    /// Record the outcome of a one-click restore test.
    pub async fn record_restore_test(&self, status: &str, at_ms: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO backup_state (id, last_restore_test_at, last_restore_test_status) \
             VALUES (1, ?, ?) ON CONFLICT(id) DO UPDATE SET \
             last_restore_test_at = excluded.last_restore_test_at, \
             last_restore_test_status = excluded.last_restore_test_status",
        )
        .bind(at_ms)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        let path = std::env::temp_dir().join(format!("vautr_bk_repo_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    #[tokio::test]
    async fn backup_key_is_generated_and_stable() {
        let repo = test_repo().await;
        let k1 = repo.get_or_create_backup_key().await.unwrap();
        let k2 = repo.get_or_create_backup_key().await.unwrap();
        assert_eq!(k1, k2, "backup key must be stable across reads");
    }

    #[tokio::test]
    async fn backup_state_and_run_roundtrip() {
        let repo = test_repo().await;
        let initial = repo.backup_state().await.unwrap();
        assert!(initial.enabled);

        let run = BackupRun {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: 1_700_000_000_000,
            size_bytes: 42,
            checksum: "abc".to_string(),
            archive_path: "/tmp/bk.bin".to_string(),
            vault_id: "vault-1".to_string(),
            entry_count: 7,
        };
        repo.record_backup_run(&run, 1_700_000_000_500)
            .await
            .unwrap();
        let fetched = repo.get_backup_run(&run.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, run.id);
        assert_eq!(fetched.entry_count, 7);

        let state = repo.backup_state().await.unwrap();
        assert_eq!(state.last_backup_at, Some(1_700_000_000_500));
        assert_eq!(state.last_backup_size_bytes, Some(42));

        repo.record_restore_test("passed", 1_700_000_000_900)
            .await
            .unwrap();
        let state = repo.backup_state().await.unwrap();
        assert_eq!(state.last_restore_test_at, Some(1_700_000_000_900));
        assert_eq!(state.last_restore_test_status.as_deref(), Some("passed"));
    }
}
