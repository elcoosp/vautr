//! `sessions` repository domain: bearer-session storage and lookup.
//!
//! VTRFIX-SEC-H03: session tokens are hashed at rest with SHA-256 (hex).
//! Callers still pass the raw token on write and on lookup; the hashing is
//! confined to this module so no handler has to know.

use crate::repository::Repository;
use sha2::{Digest, Sha256};

/// Hex-encoded SHA-256 of a raw session token. This is what we persist.
pub fn hash_session_token(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

impl Repository {
    /// Store a login session. `token` is the raw bearer token returned to the
    /// client; the DB only ever sees its SHA-256 hash.
    pub async fn store_session(
        &self,
        token: &str,
        user_id: &str,
        expires_at: i64,
    ) -> Result<(), sqlx::Error> {
        let token_hash = hash_session_token(token);
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(token_hash) DO UPDATE \
               SET user_id = excluded.user_id, expires_at = excluded.expires_at",
        )
        .bind(&token_hash)
        .bind(user_id)
        .bind(expires_at)
        .bind(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a session's user + expiry by the raw presented token.
    pub async fn get_session(&self, token: &str) -> Result<Option<(String, i64)>, sqlx::Error> {
        let token_hash = hash_session_token(token);
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token_hash = ?")
                .bind(&token_hash)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Delete a session by the raw presented token (logout).
    pub async fn delete_session(&self, token: &str) -> Result<u64, sqlx::Error> {
        let token_hash = hash_session_token(token);
        let res = sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Prune sessions whose `expires_at` is in the past.
    pub async fn purge_expired_sessions(&self, now_ms: i64) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
            .bind(now_ms)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
