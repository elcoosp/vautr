-- Vautr server schema — backups & one-click restore test (mlp-scope.md §1,
-- mlp-wave-plan.md §3 A5). Append-only numbered migration; never edit 0001..0010.
-- Canonical source: docs/architecture/mlp-scope.md §1, mlp-wave-plan.md (A5).

-- Backup encryption key (single row, id = 1). Backup archives are sealed with
-- XChaCha20-Poly1305 under this key (core/vautr-backup). The one-click restore
-- test decrypts + validates into a scratch DB without touching the live store.
CREATE TABLE IF NOT EXISTS backup_key (
    id  INTEGER PRIMARY KEY CHECK (id = 1),
    key BLOB NOT NULL
) STRICT;

-- Backup configuration + last-run state (single row, id = 1).
CREATE TABLE IF NOT EXISTS backup_state (
    id                        INTEGER PRIMARY KEY CHECK (id = 1),
    enabled                   INTEGER NOT NULL DEFAULT 1,
    location                  TEXT,
    schedule                  TEXT NOT NULL DEFAULT 'daily',
    last_backup_at            INTEGER,
    last_backup_size_bytes    INTEGER,
    last_restore_test_at      INTEGER,
    last_restore_test_status  TEXT
) STRICT;

-- History of produced backup archives: maps a backup_id (uuid) to the sealed
-- archive file on disk so POST /backup/restore can target an existing archive.
CREATE TABLE IF NOT EXISTS backup_runs (
    id            TEXT PRIMARY KEY,
    created_at    INTEGER NOT NULL,
    size_bytes    INTEGER NOT NULL,
    checksum      TEXT NOT NULL,
    archive_path  TEXT NOT NULL,
    vault_id      TEXT NOT NULL,
    entry_count   INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_backup_runs_created_at ON backup_runs(created_at);
