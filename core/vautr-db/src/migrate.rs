//! Schema bootstrap for the client local DB.
//! Spec: docs/architecture/db-contract.md §2 (PRAGMAs), §3 (tables), §4 (FTS5).
//! Opens SQLite WAL, sets `synchronous=NORMAL`, creates STRICT tables, the
//! `items_fts` virtual table with insert/update/delete triggers, and the
//! `CHECK(state IN (...))` constraint on `local_blacklist`.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr};

/// Full client schema as one batch (idempotent via `IF NOT EXISTS`).
/// SQLite executes multiple statements in a single unprepared call.
const SCHEMA: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA temp_store=MEMORY;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS item_overviews (
    uuid TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    enc_key_gen INTEGER NOT NULL,
    deleted_date INTEGER,
    overview_title TEXT NOT NULL,
    overview_subtitle TEXT NOT NULL,
    overview_icon_key TEXT NOT NULL,
    overview_urls TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_overview_enc_key_gen ON item_overviews(enc_key_gen);
CREATE INDEX IF NOT EXISTS idx_overview_updated_at ON item_overviews(updated_at);

CREATE TABLE IF NOT EXISTS item_payloads (
    uuid TEXT PRIMARY KEY,
    payload BLOB NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS sync_meta (
    id INTEGER PRIMARY KEY,
    sync_cursor INTEGER NOT NULL,
    min_enc_key_gen INTEGER NOT NULL,
    svk_ciphertext_blob BLOB NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS local_blacklist (
    uuid TEXT PRIMARY KEY,
    ignored_version INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('ToxicIgnored','ValidIgnored'))
) STRICT;

CREATE TABLE IF NOT EXISTS quarantine (
    uuid TEXT PRIMARY KEY,
    target_version INTEGER NOT NULL,
    quarantine_until INTEGER NOT NULL,
    retries INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_quarantine_until ON quarantine(quarantine_until);

CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    uuid UNINDEXED, overview_title, overview_subtitle, overview_urls,
    content='item_overviews', content_rowid='rowid', tokenize="unicode61"
);

CREATE TRIGGER IF NOT EXISTS overviews_ai AFTER INSERT ON item_overviews BEGIN
    INSERT INTO items_fts(rowid, uuid, overview_title, overview_subtitle, overview_urls)
    VALUES (new.rowid, new.uuid, new.overview_title, new.overview_subtitle, new.overview_urls);
END;

CREATE TRIGGER IF NOT EXISTS overviews_ad AFTER DELETE ON item_overviews BEGIN
    INSERT INTO items_fts(items_fts, rowid, uuid, overview_title, overview_subtitle, overview_urls)
    VALUES ('delete', old.rowid, old.uuid, old.overview_title, old.overview_subtitle, old.overview_urls);
END;

CREATE TRIGGER IF NOT EXISTS overviews_au AFTER UPDATE ON item_overviews BEGIN
    INSERT INTO items_fts(items_fts, rowid, uuid, overview_title, overview_subtitle, overview_urls)
    VALUES ('delete', old.rowid, old.uuid, old.overview_title, old.overview_subtitle, old.overview_urls);
    INSERT INTO items_fts(rowid, uuid, overview_title, overview_subtitle, overview_urls)
    VALUES (new.rowid, new.uuid, new.overview_title, new.overview_subtitle, new.overview_urls);
END;
"#;

/// Open and initialize the client SQLite database per db-contract §2–4.
pub async fn init(db: &DatabaseConnection) -> Result<(), DbErr> {
    db.execute_unprepared(SCHEMA).await?;
    Ok(())
}
