//! Recovery repository — RK credential storage, one-time recovery sessions,
//! and account reclaim/suspension lifecycle.
//! Spec: docs/architecture/emergency-recovery-account.md §2-4.
//! Owned by the Wave B recovery agent; touches the `recovery_sessions` table
//! and the `users` columns added in migrations/0002_recovery.sql.

use crate::repository::Repository;

impl Repository {
    /// Store/replace the RK Ed25519 public key used to authenticate a recovery
    /// (emergency-recovery-account.md §2.2, `users.rk_public_key`).
    pub async fn set_rk_public_key(
        &self,
        user_id: &str,
        rk_public_key: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET rk_public_key = ? WHERE id = ?")
            .bind(rk_public_key)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Read the stored RK Ed25519 public key, if any.
    pub async fn get_rk_public_key(&self, user_id: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Option<Vec<u8>>,)> =
            sqlx::query_as("SELECT rk_public_key FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|r| r.0))
    }

    /// Atomically replace a user's recovery credentials after a successful RK
    /// recovery: a fresh OPAQUE record, new wrapped SVK blobs (MP + RK), and a
    /// new RK public key (emergency-recovery-account.md §2.3, step 7).
    pub async fn complete_recovery(
        &self,
        user_id: &str,
        opaque_record: &[u8],
        kdf_salt: &[u8],
        svk_ciphertext_blob: &[u8],
        svk_ciphertext_blob_rk: &[u8],
        rk_public_key: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET opaque_record = ?, kdf_salt = ?, \
             svk_ciphertext_blob = ?, svk_ciphertext_blob_rk = ?, rk_public_key = ?, \
             updated_at = ? WHERE id = ?",
        )
        .bind(opaque_record)
        .bind(kdf_salt)
        .bind(svk_ciphertext_blob)
        .bind(svk_ciphertext_blob_rk)
        .bind(rk_public_key)
        .bind(now)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Invalidate every active bearer session for a user (forced re-auth after
    /// recovery or deletion).
    pub async fn revoke_user_sessions(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Store a one-time, short-lived recovery session token
    /// (emergency-recovery-account.md §2.3, `recovery_sessions`).
    pub async fn create_recovery_session(
        &self,
        token: &str,
        user_id: &str,
        expires_at: i64,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO recovery_sessions (token, user_id, expires_at, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(token)
        .bind(user_id)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Consume a recovery session token atomically (one-time use). Returns the
    /// bound `(user_id, expires_at)` and deletes the row on read, so a token can
    /// never be used twice even under concurrent requests.
    pub async fn consume_recovery_session(
        &self,
        token: &str,
    ) -> Result<Option<(String, i64)>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT user_id, expires_at FROM recovery_sessions WHERE token = ?")
                .bind(token)
                .fetch_optional(&mut *tx)
                .await?;
        if row.is_some() {
            sqlx::query("DELETE FROM recovery_sessions WHERE token = ?")
                .bind(token)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(row)
    }

    /// Initiate an account reclaim: bind a single-use reclaim token to the user's
    /// email and suspend (hide) the vault for the 30-day grace period
    /// (emergency-recovery-account.md §4.2).
    pub async fn store_reclaim(
        &self,
        user_id: &str,
        reclaim_token: &str,
        suspended_until: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET reclaim_token = ?, suspended_until = ? WHERE id = ?")
            .bind(reclaim_token)
            .bind(suspended_until)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Resolve a reclaim token back to its user `(id, email)`, if valid.
    pub async fn get_user_by_reclaim_token(
        &self,
        token: &str,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT id, email FROM users WHERE reclaim_token = ?")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Finalize a reclaim: clear the reclaim token and lift the suspension so the
    /// user can re-register a fresh vault with the same email.
    pub async fn clear_reclaim(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET reclaim_token = NULL, suspended_until = NULL WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Permanently delete a user and all cascaded rows (sessions, recovery
    /// sessions, items, shares). emergency-recovery-account.md §4.1.
    pub async fn delete_account(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
