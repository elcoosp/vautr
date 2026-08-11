//! `config` repository domain: raw server-config key/value reads/writes
//! (e.g. the persisted OPAQUE server setup).

use crate::repository::Repository;

impl Repository {
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
}
