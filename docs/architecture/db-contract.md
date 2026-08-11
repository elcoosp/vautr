# Vautr Local Database Schema & Persistence Contract

This document defines the exact SQLite schema, SeaORM 2.0 entity definitions, indexing strategy, transaction boundaries, and persistence rules for the Vautr Client. The local database is the single source of offline truth, acting as the persistence layer for the Core Architecture's state machines, the Sync Engine, and the FTS5 search index.

Deviations from this specification will result in UI thread locks, orphaned DashMap states, or catastrophic loss of crash consistency.

---

## 1. Architectural Principles (The Persistence Rules)

1.  **WAL Mode Enforcement:** The database MUST be opened with `journal_mode=WAL`. This allows the UI thread to read concurrently while the `PersistenceWorker` writes, completely eliminating UI freezes.
2.  **Strict Tables:** All `CREATE TABLE` statements MUST use the `STRICT` keyword. SeaORM migrations must emit `STRICT` tables.
3.  **Hot/Cold Data Isolation:** To prevent UI stuttering, small, frequently accessed "Hot" data (Overviews, Metadata) MUST be physically separated from large, rarely accessed "Cold" data (Payload BLOBs). They are joined only when the user explicitly requests to view a secret.
4.  **The Security Boundary:** The `payload` column is strictly a `BLOB` (ciphertext). `DecryptedSecrets` are **NEVER** written to disk. `DecryptedOverviews` are written to disk as plaintext to power FTS5, protected solely by OS full-disk encryption.
5.  **Synchronous Normal:** With WAL mode enabled, `synchronous=NORMAL` is safe and provides a massive performance boost over `FULL`.
6.  **The `i64` Casting Contract:** SQLite lacks native `u64` support. All Rust `u64` values (Versions, Epochs, Cursors) MUST be cast to `i64` for persistence. The Core MUST validate on ingestion that server values do not exceed `i64::MAX` to prevent silent integer overflow.
7.  **SeaORM 2.0 Patterns:** Entities MUST use the `#[sea_orm::model]` macro with inline relation definitions. Queries MUST use strongly-typed columns (`Entity::COLUMN.field`).

---

## 2. Database Initialization (Connection Contract)

Upon establishing a `DatabaseConnection` via SeaORM, the Core MUST execute the following PRAGMAs before running any queries:

```rust
db.execute_unprepared("PRAGMA journal_mode=WAL;").await?;
db.execute_unprepared("PRAGMA synchronous=NORMAL;").await?;
db.execute_unprepared("PRAGMA temp_store=MEMORY;").await?;
db.execute_unprepared("PRAGMA foreign_keys=ON;").await?;
```

---

## 3. SeaORM 2.0 Entity Definitions

### Custom Types

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String")]
pub enum DashMapState {
    #[sea_orm(string_value = "ToxicIgnored")]
    ToxicIgnored,
    #[sea_orm(string_value = "ValidIgnored")]
    ValidIgnored,
}
```

### `ItemOverview` Entity (Hot Data)
Stores server metadata and denormalized `DecryptedOverview` fields for fast list rendering. Searched by FTS5.

```rust
mod item_overview {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "item_overviews")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub version: i64,
        pub enc_key_gen: i64,
        pub deleted_date: Option<i64>,
        
        // DecryptedOverview (Plaintext for UI/FTS)
        pub overview_title: String,
        pub overview_subtitle: String,
        pub overview_icon_key: String,
        #[sea_orm(column_type = "Text")]
        pub overview_urls: String, // JSON Array
        
        pub created_at: i64,
        pub updated_at: i64,

        #[sea_orm(has_one)]
        pub payload: HasOne<super::item_payload::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```
**Required Migration Indexes:**
```rust
manager.create_index(Index::create().name("idx_overview_enc_key_gen").table(item_overview::Entity).col(item_overview::Column::EncKeyGen)).await?;
manager.create_index(Index::create().name("idx_overview_updated_at").table(item_overview::Entity).col(item_overview::Column::UpdatedAt)).await?;
```

### `ItemPayload` Entity (Cold Data)
Stores the opaque ciphertext BLOB. Only accessed when the user explicitly reveals a secret or during background rotation.

```rust
mod item_payload {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "item_payloads")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        #[sea_orm(column_type = "Blob")]
        pub payload: Vec<u8>,

        #[sea_orm(belongs_to, from = "uuid", to = "uuid")]
        pub overview: HasOne<super::item_overview::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```

### `SyncMeta` Entity
Singleton table tracking the client's view of the server state.

```rust
mod sync_meta {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "sync_meta")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i32, // Always 1
        pub sync_cursor: i64,
        pub min_enc_key_gen: i64,
        #[sea_orm(column_type = "Blob")]
        pub svk_ciphertext_blob: Vec<u8>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```

### `LocalBlacklist` Entity (The Batch-Persisted DashMap)
Tracks ignored/toxic items. Loaded into the in-memory `DashMap` at vault unlock. No `is_dirty` flag; the entire table is treated as an atomic snapshot.

```rust
mod local_blacklist {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "local_blacklist")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub ignored_version: i64,
        pub state: super::DashMapState,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```
**Migration Constraint:** Add `CHECK(state IN ('ToxicIgnored', 'ValidIgnored'))` via raw SQL.

### `Quarantine` Entity (The Reaper's Domain)
Tracks locally deleted items awaiting server tombstoning.

```rust
mod quarantine {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "quarantine")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub target_version: i64,
        pub quarantine_until: i64,
        pub retries: i32,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```
**Required Migration Index:** `idx_quarantine_until` on `quarantine_until`.

---

## 4. Search Schema (FTS5 & Raw SQL Migrations)

FTS5 operates exclusively on the `item_overviews` hot table, ensuring that search indexing never touches or pages the cold payload BLOBs. SeaORM does not natively support FTS5, so these are executed via raw statements in migrations.

```sql
CREATE VIRTUAL TABLE items_fts USING fts5(
    uuid UNINDEXED, 
    title, 
    subtitle, 
    urls,
    content='item_overviews',
    content_rowid='rowid',
    tokenize="unicode61"
);

-- Triggers updated to reference item_overviews
CREATE TRIGGER overviews_ai AFTER INSERT ON item_overviews BEGIN
    INSERT INTO items_fts(rowid, uuid, title, subtitle, urls) 
    VALUES (new.rowid, new.uuid, new.overview_title, new.overview_subtitle, new.overview_urls);
END;

CREATE TRIGGER overviews_ad AFTER DELETE ON item_overviews BEGIN
    INSERT INTO items_fts(items_fts, rowid, uuid, title, subtitle, urls) 
    VALUES ('delete', old.rowid, old.uuid, old.overview_title, old.overview_subtitle, old.overview_urls);
END;

CREATE TRIGGER overviews_au AFTER UPDATE ON item_overviews BEGIN
    INSERT INTO items_fts(items_fts, rowid, uuid, title, subtitle, urls) 
    VALUES ('delete', old.rowid, old.uuid, old.overview_title, old.overview_subtitle, old.overview_urls);
    INSERT INTO items_fts(rowid, uuid, title, subtitle, urls) 
    VALUES (new.rowid, new.uuid, new.overview_title, new.overview_subtitle, new.overview_urls);
END;
```

---

## 5. Transaction Boundaries (SeaORM 2.0 Implementation)

These define the strict ACID boundaries using SeaORM 2.0 syntax and strongly-typed columns.

### 5.1 The Save Flow (Optimistic UI Commit)
Executes an atomic upsert of the Hot/Cold data, explicitly cascading state deletions to resolve conflicts/cancellations.

```rust
use entity::{item_overview, item_payload, local_blacklist, quarantine};
use sea_orm::*;

pub async fn save_item_txn(
    txn: &DatabaseTransaction, 
    overview_model: item_overview::ActiveModel,
    payload_model: item_payload::ActiveModel,
    item_uuid: &str
) -> Result<(), DbErr> {
    // 1. Upsert Overview (FTS trigger fires automatically)
    item_overview::Entity::insert(overview_model)
        .on_conflict(
            Conflict::column(item_overview::Column::Uuid)
                .update_column([
                    item_overview::Column::Version,
                    item_overview::Column::EncKeyGen,
                    item_overview::Column::DeletedDate,
                    item_overview::Column::OverviewTitle,
                    item_overview::Column::OverviewSubtitle,
                    item_overview::Column::OverviewIconKey,
                    item_overview::Column::OverviewUrls,
                    item_overview::Column::UpdatedAt,
                ])
                .to_owned()
        )
        .exec(txn)
        .await?;

    // 2. Upsert Payload
    item_payload::Entity::insert(payload_model)
        .on_conflict(
            Conflict::column(item_payload::Column::Uuid)
                .update_column([item_payload::Column::Payload])
                .to_owned()
        )
        .exec(txn)
        .await?;

    // 3. Cascading State Cleanup: Remove from Blacklist and Quarantine
    local_blacklist::Entity::delete_many()
        .filter(local_blacklist::COLUMN.uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    quarantine::Entity::delete_many()
        .filter(quarantine::COLUMN.uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    Ok(())
}
```

### 5.2 The Sync Pull Flow (Batch Application)
Applies delta changes from the server atomically, cleaning up local state machines when items are remotely deleted.

```rust
pub async fn apply_sync_batch_txn(
    txn: &DatabaseTransaction,
    overviews_to_upsert: Vec<item_overview::ActiveModel>,
    payloads_to_upsert: Vec<item_payload::ActiveModel>,
    uuids_to_delete: Vec<String>,
    new_cursor: i64,
    new_min_gen: i64
) -> Result<(), DbErr> {
    // 1. Delete removed items (FTS trigger handles search cleanup)
    if !uuids_to_delete.is_empty() {
        item_payload::Entity::delete_many()
            .filter(item_payload::COLUMN.uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;
            
        item_overview::Entity::delete_many()
            .filter(item_overview::COLUMN.uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;

        // 2. Cascading State Cleanup
        local_blacklist::Entity::delete_many()
            .filter(local_blacklist::COLUMN.uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;

        quarantine::Entity::delete_many()
            .filter(quarantine::COLUMN.uuid.is_in(&uuids_to_delete))
            .exec(txn)
            .await?;
    }

    // 3. Upsert active overviews and payloads
    if !overviews_to_upsert.is_empty() {
        item_overview::Entity::insert_many(overviews_to_upsert)
            .on_conflict(
                Conflict::column(item_overview::Column::Uuid)
                    .update_column([/* ... all overview columns ... */])
                    .to_owned()
            )
            .exec(txn).await?;
    }
    if !payloads_to_upsert.is_empty() {
        item_payload::Entity::insert_many(payloads_to_upsert)
            .on_conflict(
                Conflict::column(item_payload::Column::Uuid)
                    .update_column([item_payload::Column::Payload])
                    .to_owned()
            )
            .exec(txn).await?;
    }

    // 4. Update sync cursor
    sync_meta::Entity::update_many()
        .col_expr(sync_meta::Column::SyncCursor, Expr::value(new_cursor))
        .col_expr(sync_meta::Column::MinEncKeyGen, Expr::value(new_min_gen))
        .filter(sync_meta::COLUMN.id.eq(1))
        .exec(txn)
        .await?;

    Ok(())
}
```

### 5.3 The DashMap Batch-Persist Flow (Crash Consistency)
The `DashMap` is a bounded in-memory cache. The most crash-safe and performant way to persist it is an atomic Wipe and Rewrite. The absence of a `is_dirty` flag prevents logic bugs and constraint violations.

```rust
pub async fn persist_dashmap_txn(
    txn: &DatabaseTransaction,
    current_dashmap_entries: Vec<local_blacklist::ActiveModel>
) -> Result<(), DbErr> {
    // 1. Atomic Wipe: Delete the entire table
    local_blacklist::Entity::delete_many().exec(txn).await?;

    // 2. Atomic Rewrite: Insert the current memory state
    if !current_dashmap_entries.is_empty() {
        local_blacklist::Entity::insert_many(current_dashmap_entries)
            .exec(txn)
            .await?;
    }

    Ok(())
}
```

### 5.4 The Reaper TTL Reset Flow (412 Handling)
Updates the Reaper's state on a `412 Precondition Failed`, ensuring the TTL is reset and the target version is bumped.

```rust
pub async fn reaper_reset_ttl_txn(
    txn: &DatabaseTransaction,
    item_uuid: &str,
    new_target_version: i64,
    new_quarantine_until: i64
) -> Result<(), DbErr> {
    quarantine::Entity::update_many()
        .col_expr(quarantine::Column::TargetVersion, Expr::value(new_target_version))
        .col_expr(quarantine::Column::QuarantineUntil, Expr::value(new_quarantine_until))
        .col_expr(quarantine::Column::Retries, Expr::col(quarantine::COLUMN.retries).add(1))
        .filter(quarantine::COLUMN.uuid.eq(item_uuid))
        .exec(txn)
        .await?;

    Ok(())
}
```
