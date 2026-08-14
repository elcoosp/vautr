//! `sessions` repository domain: bearer-session storage and lookup.

use crate::repository::Repository;

impl Repository {
    /// Store a login session (sessions table, TTL via `expires_at`).
    pub async fn store_session(
        &self,
        token: &str,
        user_id: &str,
        expires_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(token) DO UPDATE SET user_id = excluded.user_id, expires_at = excluded.expires_at",
        )
        .bind(token)
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

    /// Fetch a session's user + expiry. Caller must check `expires_at`.
    pub async fn get_session(&self, token: &str) -> Result<Option<(String, i64)>, sqlx::Error> {
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token = ?")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }
}
