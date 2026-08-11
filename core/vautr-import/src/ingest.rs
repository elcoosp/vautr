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

/// Rebuild the (already recreated) FTS5 index from the content table.
///
/// The FTS table uses external content (`content='item_overviews'`) whose column
/// names (`overview_title`, ...) differ from the FTS columns (`title`, ...), so
/// the `rebuild` command's automatic content scan is inapplicable. Instead we
/// bulk-populate the index with a set-based `INSERT ... SELECT`, which is the
/// drop/rebuild fast-path in §3.2 and re-indexes every row atomically.
const REBUILD_FTS_SQL: &str =
    "INSERT INTO items_fts(rowid, uuid, title, subtitle, urls) \
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

    // 1. Drop FTS5 index + triggers before the bulk insert.
    db.execute_unprepared(DROP_FTS_SQL)
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("drop fts: {e}")))?;

    let overviews: Vec<item_overview::ActiveModel> =
        items.iter().map(|e| e.overview.clone()).collect();
    let payloads: Vec<item_payload::ActiveModel> = items.iter().map(|e| e.payload.clone()).collect();

    // 2. Single transaction with bulk insert.
    let txn = db
        .begin()
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("begin txn: {e}")))?;

    let result = async {
        if !overviews.is_empty() {
            item_overview::Entity::insert_many(overviews).exec(&txn).await?;
        }
        if !payloads.is_empty() {
            item_payload::Entity::insert_many(payloads).exec(&txn).await?;
        }
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

    // 3. Recreate the FTS5 virtual table + triggers, then rebuild the index.
    migrate::init(db)
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("recreate fts: {e}")))?;
    db.execute_unprepared(REBUILD_FTS_SQL)
        .await
        .map_err(|e| ImportFailure::Pipeline(format!("rebuild fts: {e}")))?;

    Ok(())
}
