-- Vautr server schema — file attachments (defined in docs/architecture/file-storage.md).
-- Canonical source: docs/architecture/file-storage.md
-- Adds the server-side manifest for large binary attachments and the optional
-- per-chunk tracking table used by the multipart gateway (§2.3, §4.1).

-- FileManifest mirror: assembled after the S3 multipart upload commits, so the
-- server only ever holds manifests for fully realized files (file-storage.md §4.1).
CREATE TABLE IF NOT EXISTS file_manifests (
    file_uuid    TEXT PRIMARY KEY,
    owner_user_id TEXT NOT NULL,
    total_size   INTEGER NOT NULL,
    chunk_size   INTEGER NOT NULL,
    total_chunks INTEGER NOT NULL,
    enc_key_gen  INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    status       TEXT NOT NULL,  -- 'PendingUpload' | 'Available' (file-storage.md §2.3)
    last_modified INTEGER NOT NULL,
    created_at   INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Optional per-chunk transfer state for resumable multipart uploads
-- (file-storage.md §4.1: GET /files/{file_uuid}/upload/status).
CREATE TABLE IF NOT EXISTS file_chunks (
    file_uuid   TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    status      TEXT NOT NULL,
    PRIMARY KEY (file_uuid, chunk_index),
    FOREIGN KEY (file_uuid) REFERENCES file_manifests(file_uuid) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_file_manifests_owner ON file_manifests(owner_user_id);
