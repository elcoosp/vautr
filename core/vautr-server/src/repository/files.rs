//! `files` repository domain: server-side file manifest + per-chunk transfer
//! tracking for the multipart upload gateway (file-storage.md §2.3, §4.1).
//!
//! All queries scope strictly by `owner_user_id` so a user can never read or
//! mutate another tenant's file session. The server only ever persists
//! manifests, never the FEK or plaintext (zero-knowledge blob store).

use sqlx::FromRow;

use crate::repository::Repository;

/// Row mirror of `file_manifests` (migrations/0004_files.sql).
#[derive(Debug, Clone, FromRow)]
pub struct FileManifestRow {
    pub file_uuid: String,
    pub owner_user_id: String,
    pub total_size: i64,
    pub chunk_size: i64,
    pub total_chunks: i64,
    pub enc_key_gen: i64,
    pub content_type: String,
    pub status: String, // 'PendingUpload' | 'Available'
    pub last_modified: i64,
    pub created_at: i64,
}

/// Row mirror of `file_chunks` (migrations/0004_files.sql).
#[derive(Debug, Clone, FromRow)]
pub struct FileChunkRow {
    pub file_uuid: String,
    pub chunk_index: i64,
    pub status: String,
}

impl Repository {
    /// Create (or reset) a pending file manifest plus its per-chunk rows.
    ///
    /// Re-initiation of a session re-arms every chunk to `pending`. Callers must
    /// reject re-initiation of an already-`Available` file before calling this
    /// (see `handlers/files.rs::upload_initiate`).
    pub async fn upsert_file_manifest(
        &self,
        file_uuid: &str,
        user_id: &str,
        total_size: i64,
        chunk_size: i64,
        total_chunks: i64,
        enc_key_gen: i64,
        content_type: &str,
        last_modified: i64,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file_manifests \
               (file_uuid, owner_user_id, total_size, chunk_size, total_chunks, \
                enc_key_gen, content_type, status, last_modified, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 'PendingUpload', ?, ?) \
             ON CONFLICT(file_uuid) DO UPDATE SET \
               owner_user_id = excluded.owner_user_id, \
               total_size = excluded.total_size, \
               chunk_size = excluded.chunk_size, \
               total_chunks = excluded.total_chunks, \
               enc_key_gen = excluded.enc_key_gen, \
               content_type = excluded.content_type, \
               status = 'PendingUpload', \
               last_modified = excluded.last_modified",
        )
        .bind(file_uuid)
        .bind(user_id)
        .bind(total_size)
        .bind(chunk_size)
        .bind(total_chunks)
        .bind(enc_key_gen)
        .bind(content_type)
        .bind(last_modified)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // Re-arm the per-chunk state for the new session.
        sqlx::query("DELETE FROM file_chunks WHERE file_uuid = ?")
            .bind(file_uuid)
            .execute(&self.pool)
            .await?;
        for i in 0..total_chunks {
            sqlx::query(
                "INSERT INTO file_chunks (file_uuid, chunk_index, status) VALUES (?, ?, 'pending')",
            )
            .bind(file_uuid)
            .bind(i)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Fetch a manifest owned by `user_id`. Returns `None` if it does not exist
    /// or belongs to another tenant (owner scoping, 404).
    pub async fn get_file_manifest(
        &self,
        file_uuid: &str,
        user_id: &str,
    ) -> Result<Option<FileManifestRow>, sqlx::Error> {
        sqlx::query_as::<_, FileManifestRow>(
            "SELECT * FROM file_manifests WHERE file_uuid = ? AND owner_user_id = ?",
        )
        .bind(file_uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Atomically transition a manifest's status from `from` to `to`, scoped by
    /// owner. Returns `true` if this call performed the transition; `false` if
    /// the row did not match (e.g. already in `to`, or owned elsewhere).
    ///
    /// This is the race-free commit primitive: only one `complete` can flip a
    /// `PendingUpload` -> `Available` manifest, preventing double commits.
    pub async fn set_file_status(
        &self,
        file_uuid: &str,
        user_id: &str,
        from: &str,
        to: &str,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE file_manifests SET status = ? \
             WHERE file_uuid = ? AND owner_user_id = ? AND status = ?",
        )
        .bind(to)
        .bind(file_uuid)
        .bind(user_id)
        .bind(from)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    /// List the per-chunk transfer state for a manifest owned by `user_id`.
    pub async fn get_chunk_statuses(
        &self,
        file_uuid: &str,
        user_id: &str,
    ) -> Result<Vec<FileChunkRow>, sqlx::Error> {
        sqlx::query_as::<_, FileChunkRow>(
            "SELECT c.file_uuid, c.chunk_index, c.status \
             FROM file_chunks c \
             JOIN file_manifests m ON m.file_uuid = c.file_uuid \
             WHERE c.file_uuid = ? AND m.owner_user_id = ? \
             ORDER BY c.chunk_index",
        )
        .bind(file_uuid)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Mark every chunk of a manifest owned by `user_id` as uploaded (invoked on
    /// commit). Mirrors the post-assembly state of the S3 multipart upload.
    pub async fn set_all_chunks_uploaded(
        &self,
        file_uuid: &str,
        user_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE file_chunks SET status = 'uploaded' \
             WHERE file_uuid = ? AND file_uuid IN \
               (SELECT file_uuid FROM file_manifests WHERE file_uuid = ? AND owner_user_id = ?)",
        )
        .bind(file_uuid)
        .bind(file_uuid)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
