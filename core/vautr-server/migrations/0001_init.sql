-- Vautr server schema — initial migration (defined in docs/architecture/server-db.md).
-- Canonical source: docs/architecture/server-db.md
-- PRAGMAs are applied at pool-connect time (db::connect), not in migrations.

CREATE TABLE IF NOT EXISTS users (
    id                    TEXT PRIMARY KEY,
    email                 TEXT NOT NULL UNIQUE,
    kdf_salt              BLOB NOT NULL,
    opaque_record        BLOB NOT NULL,
    svk_ciphertext_blob  BLOB NOT NULL,
    svk_ciphertext_blob_rk BLOB NOT NULL,  -- KEK_RK-wrapped SVK (crypto.md §7, REQ-RECOVERY-02)
    min_enc_key_gen      INTEGER NOT NULL DEFAULT 1,
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS items (
    uuid         TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 1,
    enc_key_gen  INTEGER NOT NULL DEFAULT 1,
    deleted_date INTEGER,
    payload      BLOB,
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (uuid, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_items_user_version ON items(user_id, version);
CREATE INDEX IF NOT EXISTS idx_items_user_encgen  ON items(user_id, enc_key_gen);

CREATE TABLE IF NOT EXISTS sessions (
    token       TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL,
    expires_at  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS shares (
    item_uuid            TEXT NOT NULL,
    owner_user_id        TEXT NOT NULL,
    recipient_user_id    TEXT NOT NULL,
    wrapped_sik          BLOB NOT NULL,
    ephemeral_public_key BLOB NOT NULL,
    created_at           INTEGER NOT NULL,
    PRIMARY KEY (item_uuid, recipient_user_id),
    FOREIGN KEY (item_uuid) REFERENCES items(uuid) ON DELETE CASCADE,
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS server_config (
    id                        INTEGER PRIMARY KEY CHECK (id = 1),
    opaque_server_public_key  BLOB,
    created_at                INTEGER NOT NULL
) STRICT;
