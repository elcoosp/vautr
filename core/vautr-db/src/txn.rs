//! Transaction boundaries for the client local DB.
//! Spec: docs/architecture/db-contract.md §5 — save_item_txn,
//! apply_sync_batch_txn, persist_dashmap_txn, reaper_reset_ttl_txn.

use crate::entity::{item_overview, item_payload, local_blacklist, quarantine, sync_meta};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, ExprTrait, OnConflict};
use sea_orm::{DatabaseTransaction, DbErr, EntityTrait, Set};

/// Atomic upsert of hot/cold item data (db-contract §5.1).
/// The FTS5 trigger over `item_overviews` fires automatically on upsert.
pub async fn save_item_txn(
    txn: &DatabaseTransaction,
    overview_model: item_overview::ActiveModel,
    payload_model: item_payload::ActiveModel,
    item_uuid: &str,
) -> Result<(), DbErr> {
    // 1. Upsert Overview (FTS trigger fires automatically)
    item_overview::Entity::insert(overview_model)
        .on_conflict(
            OnConflict::column(item_overview::Column::Uuid)
                .update_columns([
                    item_overview::Column::Version,
                    item_overview::Column::EncKeyGen,
                    item_overview::Column::DeletedDate,
                    item_overview::Column::OverviewTitle,
                    item_overview::Column::OverviewSubtitle,
                    item_overview::Column::OverviewIconKey,
                    item_overview::Column::OverviewUrls,
                    item_overview::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(txn)
        .await?;

    // 2. Upsert Payload
    item_payload::Entity::insert(payload_model)
        .on_conflict(
            OnConflict::column(item_payload::Column::Uuid)
                .update_column(item_payload::Column::Payload)
                .to_owned(),
        )
        .exec(txn)
        .await?;

    // 3. Cascading State Cleanup: Remove from Blacklist and Quarantine
    local_blacklist::Entity::delete_many()
        .filter(local_blacklist::Column::Uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    quarantine::Entity::delete_many()
        .filter(quarantine::Column::Uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    Ok(())
}

/// Apply a server sync delta atomically (db-contract §5.2).
pub async fn apply_sync_batch_txn(
    txn: &DatabaseTransaction,
    overviews_to_upsert: Vec<item_overview::ActiveModel>,
    payloads_to_upsert: Vec<item_payload::ActiveModel>,
    uuids_to_delete: Vec<String>,
    new_cursor: i64,
    new_min_gen: i64,
) -> Result<(), DbErr> {
    // 1. Delete removed items (FTS trigger handles search cleanup)
    if !uuids_to_delete.is_empty() {
        item_payload::Entity::delete_many()
            .filter(item_payload::Column::Uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;

        item_overview::Entity::delete_many()
            .filter(item_overview::Column::Uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;

        // 2. Cascading State Cleanup
        local_blacklist::Entity::delete_many()
            .filter(local_blacklist::Column::Uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;

        quarantine::Entity::delete_many()
            .filter(quarantine::Column::Uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;
    }

    // 3. Upsert active overviews and payloads
    if !overviews_to_upsert.is_empty() {
        item_overview::Entity::insert_many(overviews_to_upsert)
            .on_conflict(
                OnConflict::column(item_overview::Column::Uuid)
                    .update_columns([
                        item_overview::Column::Version,
                        item_overview::Column::EncKeyGen,
                        item_overview::Column::DeletedDate,
                        item_overview::Column::OverviewTitle,
                        item_overview::Column::OverviewSubtitle,
                        item_overview::Column::OverviewIconKey,
                        item_overview::Column::OverviewUrls,
                        item_overview::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(txn)
            .await?;
    }
    if !payloads_to_upsert.is_empty() {
        item_payload::Entity::insert_many(payloads_to_upsert)
            .on_conflict(
                OnConflict::column(item_payload::Column::Uuid)
                    .update_column(item_payload::Column::Payload)
                    .to_owned(),
            )
            .exec(txn)
            .await?;
    }

    // 4. Update sync cursor + min_enc_key_gen
    sync_meta::Entity::update_many()
        .col_expr(sync_meta::Column::SyncCursor, Expr::value(new_cursor))
        .col_expr(sync_meta::Column::MinEncKeyGen, Expr::value(new_min_gen))
        .filter(sync_meta::Column::Id.eq(1))
        .exec(txn)
        .await?;

    Ok(())
}

/// Atomic wipe+rewrite of the DashMap snapshot (db-contract §5.3).
pub async fn persist_dashmap_txn(
    txn: &DatabaseTransaction,
    current_dashmap_entries: Vec<local_blacklist::ActiveModel>,
) -> Result<(), DbErr> {
    // 1. Atomic Wipe
    local_blacklist::Entity::delete_many().exec(txn).await?;

    // 2. Atomic Rewrite
    if !current_dashmap_entries.is_empty() {
        local_blacklist::Entity::insert_many(current_dashmap_entries)
            .exec(txn)
            .await?;
    }

    Ok(())
}

/// Reset Reaper TTL on 412 (db-contract §5.4).
pub async fn reaper_reset_ttl_txn(
    txn: &DatabaseTransaction,
    item_uuid: &str,
    new_target_version: i64,
    new_quarantine_until: i64,
) -> Result<(), DbErr> {
    quarantine::Entity::update_many()
        .col_expr(
            quarantine::Column::TargetVersion,
            Expr::value(new_target_version),
        )
        .col_expr(
            quarantine::Column::QuarantineUntil,
            Expr::value(new_quarantine_until),
        )
        .col_expr(
            quarantine::Column::Retries,
            Expr::col(quarantine::Column::Retries).add(1),
        )
        .filter(quarantine::Column::Uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    Ok(())
}

/// Persist the (re)wrapped SVK blob into `sync_meta` (rotation / recovery).
pub async fn store_svk_blob(db: &DatabaseConnection, blob: &[u8]) -> Result<(), DbErr> {
    sync_meta::Entity::update_many()
        .col_expr(
            sync_meta::Column::SvkCiphertextBlob,
            sea_orm::sea_query::Expr::value(sea_orm::Value::Bytes(Some(blob.to_vec()))),
        )
        .filter(sync_meta::Column::Id.eq(1))
        .exec(db)
        .await?;
    Ok(())
}
pub fn blacklist_entry(
    uuid: String,
    ignored_version: i64,
    state: crate::entity::DashMapState,
) -> local_blacklist::ActiveModel {
    local_blacklist::ActiveModel {
        uuid: Set(uuid),
        ignored_version: Set(ignored_version),
        state: Set(state.as_db_str().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{item_overview, item_payload};
    use sea_orm::{ActiveValue::Set, Database, DatabaseConnection, TransactionTrait};

    async fn connect() -> DatabaseConnection {
        let path =
            std::env::temp_dir().join(format!("vautr_db_test_{}.sqlite", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let db = Database::connect(&url).await.unwrap();
        crate::migrate::init(&db).await.unwrap();
        db
    }

    fn overview(uuid: &str, v: i64, gen: i64) -> item_overview::ActiveModel {
        item_overview::ActiveModel {
            uuid: Set(uuid.to_string()),
            version: Set(v),
            enc_key_gen: Set(gen),
            deleted_date: Set(None),
            overview_title: Set("Login".into()),
            overview_subtitle: Set("x".into()),
            overview_icon_key: Set("icon".into()),
            overview_urls: Set("[]".into()),
            created_at: Set(0),
            updated_at: Set(0),
        }
    }

    fn payload(uuid: &str) -> item_payload::ActiveModel {
        item_payload::ActiveModel {
            uuid: Set(uuid.to_string()),
            payload: Set(vec![1, 2, 3]),
        }
    }

    #[tokio::test]
    async fn save_and_sync_and_blacklist() {
        let db = connect().await;
        let txn = db.begin().await.unwrap();

        // Save flow
        save_item_txn(&txn, overview("u1", 1, 1), payload("u1"), "u1")
            .await
            .unwrap();

        // Sync batch
        let overviews = vec![overview("u2", 1, 1)];
        let payloads = vec![payload("u2")];
        apply_sync_batch_txn(&txn, overviews, payloads, vec!["u1".into()], 5, 2)
            .await
            .unwrap();

        // DashMap persist
        let entries = vec![blacklist_entry(
            "u3".into(),
            3,
            crate::entity::DashMapState::ToxicIgnored,
        )];
        persist_dashmap_txn(&txn, entries).await.unwrap();

        // Reaper reset
        reaper_reset_ttl_txn(&txn, "u3", 4, 99).await.unwrap();

        txn.commit().await.unwrap();
    }
}
