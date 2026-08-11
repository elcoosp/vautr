//! Read queries: FTS5 search, single overview fetch, and recent-items list.
//! db-contract §4 (FTS5 operates on `item_overviews`), client.md §2 `search`.
//!
//! All plaintext `DecryptedOverview` reads happen locally (the server only ever
//! sees ciphertext — crypto.md §4).

use sea_orm::FromQueryResult;
use sea_orm::{DatabaseConnection, Statement};
use serde_json::from_str;
use uuid::Uuid;
use vautr_domain::DecryptedOverview;

/// Maps a `item_overviews` row to the API `DecryptedOverview` (data.md §3.1).
#[derive(Debug, FromQueryResult)]
struct OverviewRow {
    uuid: String,
    overview_title: String,
    overview_subtitle: String,
    overview_icon_key: String,
    overview_urls: String,
    updated_at: i64,
}

fn row_to_overview(r: OverviewRow) -> DecryptedOverview {
    let urls = from_str::<Vec<String>>(&r.overview_urls).unwrap_or_default();
    DecryptedOverview {
        uuid: Uuid::parse_str(&r.uuid).unwrap_or_default(),
        title: r.overview_title,
        subtitle: r.overview_subtitle,
        icon_key: r.overview_icon_key,
        urls,
        updated_at: r.updated_at,
    }
}

/// Full-text search over the local FTS5 index (db-contract §4).
/// Returns matches ordered by FTS5 rank (relevance).
pub async fn search_overviews(
    db: &DatabaseConnection,
    query: &str,
) -> Result<Vec<DecryptedOverview>, String> {
    // Match the FTS5 virtual table; join back to the hot table for columns.
    let sql = r#"
        SELECT o.uuid, o.overview_title, o.overview_subtitle,
               o.overview_icon_key, o.overview_urls, o.updated_at
        FROM items_fts f
        JOIN item_overviews o ON o.uuid = f.uuid
        WHERE items_fts MATCH ?
        ORDER BY rank
        LIMIT 200
    "#;
    let stmt = Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        sql,
        [query.into()],
    );
    let rows = OverviewRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("fts search: {e}"))?;
    Ok(rows.into_iter().map(row_to_overview).collect())
}

/// The 50 most-recently-used items (client.md §2 `search("")` contract).
/// Ordered by `updated_at DESC, created_at DESC` (db-contract §4 intent).
pub async fn recent_overviews(
    db: &DatabaseConnection,
    limit: u32,
) -> Result<Vec<DecryptedOverview>, String> {
    let sql = r#"
        SELECT uuid, overview_title, overview_subtitle,
               overview_icon_key, overview_urls, updated_at
        FROM item_overviews
        ORDER BY updated_at DESC
        LIMIT ?
    "#;
    let stmt = Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        sql,
        [limit.into()],
    );
    let rows = OverviewRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("recent: {e}"))?;
    Ok(rows.into_iter().map(row_to_overview).collect())
}

/// Fetch a single overview by uuid (client.md §2 `get_overview`).
pub async fn get_overview(
    db: &DatabaseConnection,
    uuid: &str,
) -> Result<DecryptedOverview, String> {
    let sql = r#"
        SELECT uuid, overview_title, overview_subtitle,
               overview_icon_key, overview_urls, updated_at
        FROM item_overviews
        WHERE uuid = ?
    "#;
    let stmt = Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        sql,
        [uuid.into()],
    );
    let row = OverviewRow::find_by_statement(stmt)
        .one(db)
        .await
        .map_err(|e| format!("get_overview: {e}"))?
        .ok_or_else(|| "overview not found".to_string())?;
    Ok(row_to_overview(row))
}

/// Read the persisted DashMap (`local_blacklist`) for crash recovery (core.md §3).
/// Returns `(uuid, ignored_version, state_str)` tuples where `state_str` is
/// `ToxicIgnored` / `ValidIgnored` (db-contract §3 `CHECK` constraint).
pub async fn list_blacklist(
    db: &DatabaseConnection,
) -> Result<Vec<(String, i64, String)>, String> {
    let sql = "SELECT uuid, ignored_version, state FROM local_blacklist";
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, []);
    let rows = BlacklistRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("list_blacklist: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|r| (r.uuid, r.ignored_version, r.state))
        .collect())
}

/// Row mirror of `local_blacklist` (db-contract §3).
#[derive(Debug, FromQueryResult)]
struct BlacklistRow {
    uuid: String,
    ignored_version: i64,
    state: String,
}

/// Fetch the persisted secret material for `uuid`: `(enc_key_gen, payload)`.
/// Used by `reveal_secret` to decrypt the `DecryptedSecret` blob (client.md §2).
/// Returns `None` if the item or its payload does not exist locally.
pub async fn get_secret_material(
    db: &DatabaseConnection,
    uuid: &str,
) -> Result<Option<(i64, Vec<u8>)>, String> {
    let sql = "SELECT o.enc_key_gen, p.payload FROM item_overviews o \
               LEFT JOIN item_payloads p ON p.uuid = o.uuid \
               WHERE o.uuid = ?";
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, [uuid.into()]);
    let row = SecretMaterialRow::find_by_statement(stmt)
        .one(db)
        .await
        .map_err(|e| format!("get_secret_material: {e}"))?;
    Ok(row.map(|r| (r.enc_key_gen, r.payload)))
}

/// Row mirror of the joined overview/payload query above.
#[derive(Debug, FromQueryResult)]
struct SecretMaterialRow {
    enc_key_gen: i64,
    payload: Vec<u8>,
}

/// List `(uuid, enc_key_gen, payload)` for every item. Used by key rotation to
/// find items needing re-encryption (enc_key_gen < new_gen, ADR-006).
pub async fn list_enc_key_gens(
    db: &DatabaseConnection,
) -> Result<Vec<(Uuid, i64, Vec<u8>)>, String> {
    let sql = "SELECT o.uuid, o.enc_key_gen, p.payload \
               FROM item_overviews o LEFT JOIN item_payloads p ON p.uuid = o.uuid";
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, []);
    let rows = EncKeyGenRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("list_enc_key_gens: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let uuid = Uuid::parse_str(&r.uuid).ok()?;
            Some((uuid, r.enc_key_gen, r.payload))
        })
        .collect())
}

/// Row mirror of the `(uuid, enc_key_gen, payload)` rotation query.
#[derive(Debug, FromQueryResult)]
struct EncKeyGenRow {
    uuid: String,
    enc_key_gen: i64,
    payload: Vec<u8>,
}
