//! Bulk SQLite ingestion (data-import-seeding.md §3.2): the drop/rebuild strategy.
//!
//! To maximise bulk I/O throughput:
//! 1. Drop the FTS5 index **and its triggers** (so inserts don't fire per-row
//!    index updates).
//! 2. Wrap the entire batch in a single transaction with `insert_many`.
//! 3. Recreate the FTS5 virtual table + triggers and run `rebuild` to rebuild
//!    the search index from the whole `item_overviews` content table atomically.
//!
//! Pre-existing rows are preserved: `rebuild` re-indexes the full content table.

use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, TransactionTrait};
use vautr_db::entity::{item_overview, item_payload};
use vautr_db::migrate;

use crate::encrypt::EncryptedItem;
use crate::error::ImportFailure;

/// Drop the FTS5 index and its sync triggers (part of the drop/rebuild strategy).
const DROP_FTS_SQL: &str = r#"
DROP TABLE IF EXISTS items_fts;
DROP TRIGGER IF EXISTS overviews_ai;
DROP TRIGGER IF EXISTS overviews_ad;
DROP TRIGGER IF EXISTS overviews_au;
"#;

/// VTRFIX-BUG-M03: recreate the FTS5 virtual table + triggers. This is the
/// same DDL as `migrate::SCHEMA` (lines 71-92). Kept inline so the ingest
/// transaction can run the whole sequence — drop, insert, recreate, rebuild —
/// atomically. A crash mid-sequence no longer leaves the FTS index dropped.
const RECREATE_FTS_SQL: &str = r#"
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

/// Rebuild the (already recreated) FTS5 index from the content table.
///
/// The FTS table uses external content (`content='item_overviews'`) whose column
/// names (`overview_title`, ...) differ from the FTS columns (`title`, ...), so
/// the `rebuild` command's automatic content scan is inapplicable. Instead we
/// bulk-populate the index with a set-based `INSERT ... SELECT`, which is the
/// drop/rebuild fast-path in §3.2 and re-indexes every row atomically.
const REBUILD_FTS_SQL: &str =
    "INSERT INTO items_fts(rowid, uuid, overview_title, overview_subtitle, overview_urls) \
     SELECT rowid, uuid, overview_title, overview_subtitle, overview_urls FROM item_overviews;";

/// Persist the encrypted batch: drop FTS5, bulk-insert in one transaction,
/// recreate + rebuild FTS5.
pub async fn ingest(
    db: &DatabaseConnection,
    items: Vec<EncryptedItem>,
) -> Result<(), ImportFailure> {
    migrate::init(db)
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("db init: {e}")))?;

    let overviews: Vec<item_overview::ActiveModel> =
        items.iter().map(|e| e.overview.clone()).collect();
    let payloads: Vec<item_payload::ActiveModel> = items.iter().map(|e| e.payload.clone()).collect();

    // VTRFIX-BUG-M03: the entire drop→insert→recreate→rebuild sequence runs
    // inside ONE transaction. Previously DROP and RECREATE were separate
    // statements on the connection; a crash between them left the search
    // index dropped until the next successful import.
    let txn = db
        .begin()
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("begin txn: {e}")))?;

    let result = async {
        // 1. Drop FTS5 index + triggers inside the transaction.
        txn.execute_unprepared(DROP_FTS_SQL)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(format!("drop fts: {e}")))?;

        // 2. Chunked bulk inserts (VTRFIX-BUG-C08: SQLite caps bound variables
        //    at ~999 on older builds; 80 rows * ~10 cols = 800 binds).
        const CHUNK: usize = 80;
        for chunk in overviews.chunks(CHUNK) {
            item_overview::Entity::insert_many(chunk.to_vec())
                .exec(&txn)
                .await?;
        }
        for chunk in payloads.chunks(CHUNK) {
            item_payload::Entity::insert_many(chunk.to_vec())
                .exec(&txn)
                .await?;
        }

        // 3. Recreate FTS + triggers and bulk-rebuild the index, still inside
        //    the same transaction.
        txn.execute_unprepared(RECREATE_FTS_SQL)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(format!("recreate fts: {e}")))?;
        txn.execute_unprepared(REBUILD_FTS_SQL)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(format!("rebuild fts: {e}")))?;
        Ok::<(), sea_orm::DbErr>(())
    }
    .await;

    if let Err(e) = result {
        txn.rollback()
            .await
            .map_err(|re| ImportFailure::Pipeline(format!("rollback: {re}")))?;
        return Err(ImportFailure::Pipeline(format!("bulk insert: {e}")));
    }
    txn.commit()
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("commit txn: {e}")))?;

    Ok(())
}
