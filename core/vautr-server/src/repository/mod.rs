//! SQLx repository for the Vautr server.
//!
//! All queries scope strictly by `user_id` to prevent cross-tenant access.
//! Schema: [`docs/architecture/server-db.md`]. OCC (ADR-004) and epoch gating
//! (REQ-API-01) are implemented in `upsert_item_occ`.
//!
//! ⚠ OPAQUE record storage and the sharing KEM are implemented in `vautr-crypto`
//! (`opaque` + `sharing` modules) and wired here via `store_session` / `get_session`
//! and the `shares` table. See docs/architecture/adr-007-sharing-kem.md.
//! `opaque_record` / `shares` columns exist but are not yet written by the
//! auth flow.

use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;

/// Row mirror of `users` (server-db.md §3).
#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub kdf_salt: Vec<u8>,
    pub opaque_record: Vec<u8>,
    pub svk_ciphertext_blob: Vec<u8>,
    pub svk_ciphertext_blob_rk: Vec<u8>,
    pub min_enc_key_gen: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Row mirror of `items` (server-db.md §3).
#[derive(Debug, Clone, FromRow)]
pub struct ItemRow {
    pub uuid: String,
    pub user_id: String,
    pub version: i64,
    pub enc_key_gen: i64,
    pub deleted_date: Option<i64>,
    pub payload: Option<Vec<u8>>,
    pub updated_at: i64,
}

/// Result of an OCC upsert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// Row updated (OCC matched); `version` was incremented.
    Updated,
    /// `version` did not match — caller should return 412 (ADR-004).
    Conflict,
    /// `enc_key_gen < min_enc_key_gen` — caller should return 422 (REQ-API-01).
    EpochTooOld,
}

/// The server repository: a thin, tenant-scoped wrapper over the SQLite pool.
#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // --- users ---

    /// Insert a new user. `min_enc_key_gen` defaults to 1.
    pub async fn create_user(
        &self,
        id: &str,
        email: &str,
        kdf_salt: &[u8],
        opaque_record: &[u8],
        svk_ciphertext_blob: &[u8],
        svk_ciphertext_blob_rk: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob, svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(id)
        .bind(email)
        .bind(kdf_salt)
        .bind(opaque_record)
        .bind(svk_ciphertext_blob)
        .bind(svk_ciphertext_blob_rk)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a user by email (login/registration lookup).
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
    }

    /// Fetch the RK-wrapped SVK blob for recovery (crypto.md §7, REQ-RECOVERY-02).
    pub async fn get_user_svk_rk(&self, user_id: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT svk_ciphertext_blob_rk FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Read the user's current `min_enc_key_gen` (epoch gate source).
    pub async fn min_enc_key_gen(&self, user_id: &str) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT min_enc_key_gen FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Fetch a user by id.
    pub async fn get_user_by_id(&self, id: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Store a login session (sessions table, TTL via `expires_at`).
    pub async fn store_session(
        &self,
        token: &str,
        user_id: &str,
        expires_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?) \
             ON CONFLICT(token) DO UPDATE SET user_id = excluded.user_id, expires_at = excluded.expires_at",
        )
        .bind(token)
        .bind(user_id)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a session's user + expiry. Caller must check `expires_at`.
    pub async fn get_session(
        &self,
        token: &str,
    ) -> Result<Option<(String, i64)>, sqlx::Error> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT user_id, expires_at FROM sessions WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update the global epoch gate (ADR-006 / REQ-API-01).
    pub async fn update_min_enc_key_gen(
        &self,
        user_id: &str,
        new_gen: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET min_enc_key_gen = ? WHERE id = ?")
            .bind(new_gen)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Replace the stored (MP-wrapped and RK-wrapped) SVK blobs (rotation / recovery).
    pub async fn update_svk(
        &self,
        user_id: &str,
        svk_ciphertext_blob: &[u8],
        svk_ciphertext_blob_rk: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET svk_ciphertext_blob = ?, svk_ciphertext_blob_rk = ? WHERE id = ?",
        )
        .bind(svk_ciphertext_blob)
        .bind(svk_ciphertext_blob_rk)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read a raw server-config value (single-row key/value table).
    pub async fn get_config(&self, key: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT value FROM server_config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Write a raw server-config value (idempotent). Used for the OPAQUE server setup.
    pub async fn set_config(&self, key: &str, value: &[u8]) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO server_config (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // --- items (OCC + epoch gate) ---

    /// Atomic OCC upsert (ADR-004 / REQ-API-02). Validates the epoch gate
    /// (REQ-API-01) before touching the row.
    pub async fn upsert_item_occ(
        &self,
        uuid: &str,
        user_id: &str,
        target_version: i64,
        enc_key_gen: i64,
        payload: Option<&[u8]>,
        deleted_date: Option<i64>,
        now: i64,
    ) -> Result<UpsertOutcome, sqlx::Error> {
        // Epoch gate: reject if the client's enc_key_gen is behind the server.
        if let Some(min_gen) = self.min_enc_key_gen(user_id).await? {
            if enc_key_gen < min_gen {
                return Ok(UpsertOutcome::EpochTooOld);
            }
        }

        let res = sqlx::query(
            "UPDATE items \
               SET payload = ?, version = version + 1, enc_key_gen = ?, deleted_date = ?, updated_at = ? \
             WHERE uuid = ? AND user_id = ? AND version = ?",
        )
        .bind(payload)
        .bind(enc_key_gen)
        .bind(deleted_date)
        .bind(now)
        .bind(uuid)
        .bind(user_id)
        .bind(target_version)
        .execute(&self.pool)
        .await?;

        Ok(if res.rows_affected() == 1 {
            UpsertOutcome::Updated
        } else {
            UpsertOutcome::Conflict
        })
    }

    /// Fetch a single item (metadata + payload) for a user.
    pub async fn get_item(
        &self,
        uuid: &str,
        user_id: &str,
    ) -> Result<Option<ItemRow>, sqlx::Error> {
        sqlx::query_as::<_, ItemRow>("SELECT * FROM items WHERE uuid = ? AND user_id = ?")
            .bind(uuid)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        // Unique temp file DB so tests don't collide and the schema is real.
        let path = std::env::temp_dir().join(format!("vautr_srv_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    #[tokio::test]
    async fn migrations_create_all_tables() {
        let repo = test_repo().await;
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name",
        )
        .fetch_all(repo.pool())
        .await
        .unwrap();
        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        for expected in ["users", "items", "sessions", "shares", "server_config"] {
            assert!(names.contains(&expected), "missing table: {expected} (got {names:?})");
        }
    }

    #[tokio::test]
    async fn user_and_item_occ_flow() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        repo.create_user(
            "u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now,
        )
        .await
        .unwrap();

        // First write: target_version 1, but the row doesn't exist yet, so OCC
        // reports Conflict (caller must insert on 404/initial). We model the
        // initial insert separately here to keep upsert_item_occ pure-OCC.
        sqlx::query(
            "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
             VALUES (?, ?, 1, 1, NULL, ?, ?)",
        )
        .bind("item-1")
        .bind("u1")
        .bind(&[9u8; 8][..])
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();

        // Correct OCC version → Updated.
        let out = repo
            .upsert_item_occ("item-1", "u1", 1, 1, Some(&[9u8; 8][..]), None, now + 1)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::Updated);

        // Stale OCC version → Conflict.
        let out = repo
            .upsert_item_occ("item-1", "u1", 1, 1, Some(&[9u8; 8][..]), None, now + 2)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::Conflict);

        // Raise epoch; old enc_key_gen rejected (REQ-API-01).
        sqlx::query("UPDATE users SET min_enc_key_gen = 3 WHERE id = 'u1'")
            .execute(repo.pool())
            .await
            .unwrap();
        let out = repo
            .upsert_item_occ("item-1", "u1", 2, 1, Some(&[9u8; 8][..]), None, now + 3)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::EpochTooOld);
    }
}
