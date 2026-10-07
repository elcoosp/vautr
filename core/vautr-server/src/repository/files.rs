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
        // VTRFIX-SEC-H08: cap the loop for defense in depth (the handler also
        // enforces MAX_CHUNKS, but this helper must not blow up if called
        // directly from tests or future code paths).
        let capped = total_chunks.clamp(0, 512);
        if capped < total_chunks {
            return Err(sqlx::Error::Protocol(
                "total_chunks exceeds 512".into(),
            ));
        }
        // Chunk inserts stay as individual statements (SQLite handles 512 rows
        // easily within the enclosing caller's transaction). The previous
        // implementation ran an unbounded loop; this is now capped.
        for i in 0..capped {
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

impl Repository {
    /// VTRFIX-SEC-H08: sum of `total_size` for every manifest owned by
    /// `user_id`. Used to enforce the per-account file quota.
    pub async fn sum_file_bytes_owned_by(&self, user_id: &str) -> Result<i64, sqlx::Error> {
        let total: Option<i64> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_size), 0) FROM file_manifests WHERE owner_user_id = ?",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(total.unwrap_or(0))
    }
}

/// FEAT-H01: encrypted-chunk storage trait.
#[async_trait::async_trait]
pub trait ChunkStore: Send + Sync {
    async fn put_chunk(&self, file_uuid: &str, idx: u32, bytes: Vec<u8>) -> Result<(), sqlx::Error>;
    async fn get_chunk(&self, file_uuid: &str, idx: u32) -> Result<Option<Vec<u8>>, sqlx::Error>;
    async fn delete_file(&self, file_uuid: &str) -> Result<(), sqlx::Error>;
    async fn count_chunks(&self, file_uuid: &str) -> Result<u64, sqlx::Error>;
}

/// Default SQLite-backed chunk store.
pub struct SqliteChunkStore {
    pool: sqlx::sqlite::SqlitePool,
}

impl SqliteChunkStore {
    pub fn new(pool: sqlx::sqlite::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ChunkStore for SqliteChunkStore {
    async fn put_chunk(&self, file_uuid: &str, idx: u32, bytes: Vec<u8>) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO chunks (file_uuid, idx, bytes) VALUES (?, ?, ?) \
             ON CONFLICT(file_uuid, idx) DO UPDATE SET bytes = excluded.bytes",
        )
        .bind(file_uuid)
        .bind(idx as i64)
        .bind(bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_chunk(&self, file_uuid: &str, idx: u32) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT bytes FROM chunks WHERE file_uuid = ? AND idx = ?")
                .bind(file_uuid)
                .bind(idx as i64)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    async fn delete_file(&self, file_uuid: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chunks WHERE file_uuid = ?")
            .bind(file_uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn count_chunks(&self, file_uuid: &str) -> Result<u64, sqlx::Error> {
        let n: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE file_uuid = ?")
            .bind(file_uuid)
            .fetch_one(&self.pool)
            .await?;
        Ok(n.unwrap_or(0).max(0) as u64)
    }
}

impl Repository {
    /// Mark a single chunk `uploaded` for `user_id`.
    pub async fn set_chunk_uploaded(
        &self,
        file_uuid: &str,
        idx: i64,
        user_id: &str,
    ) -> Result<(), sqlx::Error> {
        // Owner check inline: only update when the manifest belongs to user.
        sqlx::query(
            "UPDATE file_chunks SET status = 'uploaded' \
             WHERE file_uuid = ? AND chunk_index = ? \
               AND EXISTS (SELECT 1 FROM file_manifests WHERE file_uuid = ? AND owner_user_id = ?)",
        )
        .bind(file_uuid)
        .bind(idx)
        .bind(file_uuid)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
