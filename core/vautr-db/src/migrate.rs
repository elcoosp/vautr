//! Schema bootstrap + versioned migrations for the client local DB.
//! Spec: docs/architecture/db-contract.md §2 (PRAGMAs), §3 (tables), §4 (FTS5).
//! Opens SQLite WAL, sets `synchronous=NORMAL`, creates STRICT tables, the
//! `items_fts` virtual table with insert/update/delete triggers, and the
//! `CHECK(state IN (...))` constraint on `local_blacklist`.
//!
//! Migrations are versioned: a `migrations` bookkeeping table records the highest
//! applied version, and `run_migrations` applies any pending `up` statements in
//! order (each with an optional `down` for rollback). The initial client schema
//! is migration `1`; future schema changes append a new `Migration` entry rather
//! than editing the `SCHEMA` constant.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr};

/// Full client schema as one batch (idempotent via `IF NOT EXISTS`).
/// This is migration version 1 and is applied by `run_migrations`.
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

/// A single, ordered schema migration. `up` is applied once when the current
/// recorded version is below `version`; `down` (if present) is the inverse used
/// by `rollback_to` for disaster recovery / downgrade testing.
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up: &'static str,
    pub down: Option<&'static str>,
}

/// Ordered list of all migrations. The first entry is the baseline client
/// schema (migration 1). Append new migrations here with strictly increasing
/// `version` values — never edit an already-shipped `up`/`down`.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "baseline_client_schema",
    up: SCHEMA,
    down: Some(
        "DROP TRIGGER IF EXISTS overviews_au; \
         DROP TRIGGER IF EXISTS overviews_ad; \
         DROP TRIGGER IF EXISTS overviews_ai; \
         DROP TABLE IF EXISTS items_fts; \
         DROP TABLE IF EXISTS quarantine; \
         DROP TABLE IF EXISTS local_blacklist; \
         DROP TABLE IF EXISTS sync_meta; \
         DROP TABLE IF EXISTS item_payloads; \
         DROP TABLE IF EXISTS item_overviews;",
    ),
}];

/// Highest migration version defined in this build.
pub fn latest_version() -> u32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

/// Apply every migration in `MIGRATIONS`, in ascending order, recording each in
/// the `migrations` bookkeeping table. Each `up` script is written with
/// `IF NOT EXISTS`/idempotent DDL, and the bookkeeping row uses `INSERT OR
/// IGNORE`, so re-running is always a safe no-op once applied.
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS migrations (\
            version INTEGER PRIMARY KEY,\
            name TEXT NOT NULL,\
            applied_at INTEGER NOT NULL\
        ) STRICT;",
    )
    .await?;
    for m in MIGRATIONS.iter() {
        db.execute_unprepared(m.up).await?;
        db.execute_unprepared(&format!(
            "INSERT OR IGNORE INTO migrations (version, name, applied_at) \
             VALUES ({}, '{}', strftime('%s','now'))",
            m.version, m.name
        ))
        .await?;
    }
    Ok(())
}

/// Roll back every migration with `version > target` (in descending order),
/// applying each migration's `down` script. Used for downgrade testing / disaster
/// recovery. `down` must be `Some` for every migration above `target`.
pub async fn rollback_to(db: &DatabaseConnection, target: u32) -> Result<(), DbErr> {
    for m in MIGRATIONS.iter().filter(|m| m.version > target).rev() {
        let down = m
            .down
            .ok_or_else(|| DbErr::Custom(format!("migration {} has no down script", m.version)))?;
        db.execute_unprepared(down).await?;
        db.execute_unprepared(&format!(
            "DELETE FROM migrations WHERE version = {}",
            m.version
        ))
        .await?;
    }
    Ok(())
}

/// Open and initialize the client SQLite database per db-contract §2–4.
/// Applies the versioned migration set (baseline is migration 1).
pub async fn init(db: &DatabaseConnection) -> Result<(), DbErr> {
    run_migrations(db).await
}
