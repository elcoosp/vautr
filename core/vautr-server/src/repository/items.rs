//! `items` repository domain: OCC upserts (ADR-004), epoch gating
//! (REQ-API-01), and item reads.

use sqlx::FromRow;

use crate::repository::Repository;

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

impl Repository {
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
        } else if target_version == 0 {
            // Fresh create (no row exists): the OCC UPDATE matched nothing and
            // the caller expressed intent to create a new item (target_version
            // 0). Insert it at version 1 so the sync engine can serve it. This
            // is the initial-insert path the pure-OCC update intentionally
            // leaves to the caller; the HTTP layer was not wiring it.
            sqlx::query(
                "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
                 VALUES (?, ?, 1, ?, ?, ?, ?)",
            )
            .bind(uuid)
            .bind(user_id)
            .bind(enc_key_gen)
            .bind(deleted_date)
            .bind(payload)
            .bind(now)
            .execute(&self.pool)
            .await?;
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
