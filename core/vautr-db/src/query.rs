//! Read queries: FTS5 search, single overview fetch, and recent-items list.
//! db-contract §4 (FTS5 operates on `item_overviews`), client.md §2 `search`.
//!
//! All plaintext `DecryptedOverview` reads happen locally (the server only ever
//! sees ciphertext — crypto.md §4).

use sea_orm::FromQueryResult;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
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
///
/// The raw query is rewritten by [`crate::search::prepare_search`] (stopword
/// stripping, safe quoting, prefix wildcards) and results are ordered by FTS5
/// BM25 `rank` (relevance, not date) and bounded to `LIMIT 100` (VTR-055).
/// Returns `(matches, leading_wildcard)` so the UI can warn when the query
/// defeats the index with a leading `*` (TDD3).
pub async fn search_overviews(
    db: &DatabaseConnection,
    query: &str,
) -> Result<(Vec<DecryptedOverview>, bool), String> {
    use crate::search::prepare_search;

    let prepared = prepare_search(query);
    if prepared.is_empty {
        // Nothing searchable: fall back to the recent list (no FTS).
        let recent = recent_overviews(db, 50).await?;
        return Ok((recent, false));
    }

    let sql = r#"
        SELECT o.uuid, o.overview_title, o.overview_subtitle,
               o.overview_icon_key, o.overview_urls, o.updated_at
        FROM items_fts f
        JOIN item_overviews o ON o.uuid = f.uuid
        WHERE items_fts MATCH ?
        ORDER BY rank
        LIMIT 100
    "#;
    let stmt = Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        sql,
        [prepared.match_expr.into()],
    );
    let rows = OverviewRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("fts search: {e}"))?;
    Ok((
        rows.into_iter().map(row_to_overview).collect(),
        prepared.leading_wildcard,
    ))
}

/// Drop, recreate, and rebuild the FTS5 index from `item_overviews` (VTR-055,
/// TDD5). Used after bulk imports and to defragment/recover the index. Returns
/// the wall-clock rebuild duration so callers can assert the < 2s target.
pub async fn rebuild_fts(db: &DatabaseConnection) -> Result<std::time::Duration, String> {
    let start = std::time::Instant::now();
    // 1. Drop the FTS5 virtual table and its triggers (db-contract §4).
    db.execute_unprepared(
        "DROP TABLE IF EXISTS items_fts; \
         DROP TRIGGER IF EXISTS overviews_ai; \
         DROP TRIGGER IF EXISTS overviews_ad; \
         DROP TRIGGER IF EXISTS overviews_au;",
    )
    .await
    .map_err(|e| format!("drop fts: {e}"))?;
    // 2. Recreate the FTS objects (idempotent). VTRFIX: this used to call
    // `migrate::init`, which re-ran every migration on every boot; migrations
    // are now version-consulted, so the rebuild path recreates the FTS
    // objects explicitly.
    crate::migrate::recreate_fts_schema(db)
        .await
        .map_err(|e| format!("recreate fts: {e}"))?;
    // 3. Bulk re-index every row atomically (data-import-seeding §3.2 fast-path).
    db.execute_unprepared(
        "INSERT INTO items_fts(rowid, uuid, overview_title, overview_subtitle, overview_urls) \
         SELECT rowid, uuid, overview_title, overview_subtitle, overview_urls \
         FROM item_overviews;",
    )
    .await
    .map_err(|e| format!("rebuild fts: {e}"))?;
    Ok(start.elapsed())
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
        ORDER BY updated_at DESC, created_at DESC /* VTRFIX-BUG-L01 */
        LIMIT ?
    "#;
    let stmt =
        Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, [limit.into()]);
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
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, [uuid.into()]);
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
pub async fn list_blacklist(db: &DatabaseConnection) -> Result<Vec<(String, i64, String)>, String> {
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
    // VTRFIX: a metadata-only item (overview without a payload row) used to
    // fail the whole query — sea-orm cannot decode SQL NULL into a non-Option
    // `Vec<u8>`. The contract says "returns None if the payload does not
    // exist", so decode the payload as Option and map it through.
    Ok(row.and_then(|r| r.payload.map(|p| (r.enc_key_gen, p))))
}

/// Row mirror of the joined overview/payload query above.
#[derive(Debug, FromQueryResult)]
struct SecretMaterialRow {
    enc_key_gen: i64,
    payload: Option<Vec<u8>>,
}

/// List `(uuid, enc_key_gen, payload)` for every item. Used by key rotation to
/// find items needing re-encryption (enc_key_gen < new_gen, ADR-006).
pub async fn list_enc_key_gens(
    db: &DatabaseConnection,
) -> Result<Vec<(Uuid, i64, Vec<u8>)>, String> {
    // VTRFIX-BUG-C06: ORDER BY is mandatory — without it the rotation loop's
    // "advance the cursor to last+1" heuristic skipped items with lower gens
    // behind the cursor, permanently losing them.
    // VTRFIX-BUG-M05: an overview row without a payload (metadata-only item)
    // would decode NULL into a non-Option Vec<u8> and fail the whole query.
    // Filter those rows out at SQL level — the caller only needs payloads.
    let sql = "SELECT o.uuid, o.enc_key_gen, p.payload \
               FROM item_overviews o INNER JOIN item_payloads p ON p.uuid = o.uuid \
               WHERE p.payload IS NOT NULL \
               ORDER BY o.enc_key_gen ASC";
    let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, []);
    let rows = EncKeyGenRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("list_enc_key_gens: {e}"))?;
    Ok(rows
        .into_iter()
        // VTRFIX-BUG-M06: never invent a nil UUID for a corrupt row. Skip it
        // with a warning so a bad row does not create a phantom item.
        .filter_map(|r| match Uuid::parse_str(&r.uuid) {
            Ok(uuid) => Some((uuid, r.enc_key_gen, r.payload)),
            Err(e) => {
                tracing::warn!(uuid = %r.uuid, error = %e, "list_enc_key_gens: skipping corrupt uuid");
                None
            }
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

/// List `(uuid, version, enc_key_gen, payload)` for every live item whose
/// `enc_key_gen < max_gen` — the full rotation input (ADR-006). Unlike
/// [`list_enc_key_gens`], this also carries each item's server `version`,
/// which the rotation push needs as its OCC target (VTRFIX: pushing
/// `target_version = enc_key_gen` matched no previously-edited item and the
/// discarded outcomes silently stranded old-generation ciphertext on the
/// server). Deleted items are excluded: tombstones carry no payload to
/// re-encrypt.
pub async fn list_rotation_items(
    db: &DatabaseConnection,
    max_gen: i64,
) -> Result<Vec<(Uuid, i64, i64, Vec<u8>)>, String> {
    let sql = "SELECT o.uuid, o.version, o.enc_key_gen, p.payload \
               FROM item_overviews o INNER JOIN item_payloads p ON p.uuid = o.uuid \
               WHERE p.payload IS NOT NULL \
                 AND o.deleted_date IS NULL \
                 AND o.enc_key_gen < ? \
               ORDER BY o.enc_key_gen ASC";
    let stmt =
        Statement::from_sql_and_values(sea_orm::DatabaseBackend::Sqlite, sql, [max_gen.into()]);
    let rows = RotationRow::find_by_statement(stmt)
        .all(db)
        .await
        .map_err(|e| format!("list_rotation_items: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| match Uuid::parse_str(&r.uuid) {
            Ok(uuid) => Some((uuid, r.version, r.enc_key_gen, r.payload)),
            Err(e) => {
                tracing::warn!(uuid = %r.uuid, error = %e, "list_rotation_items: skipping corrupt uuid");
                None
            }
        })
        .collect())
}

/// Row mirror of the rotation query above.
#[derive(Debug, FromQueryResult)]
struct RotationRow {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    payload: Vec<u8>,
}

/// Read the persisted sync cursor (`sync_meta.sync_cursor`, row id = 1).
/// Returns 0 when the singleton is missing (fresh DB before the first sync).
/// VTRFIX (cursor persistence): the cursor was written per page but never
/// read back, so every restart re-pulled the whole vault.
pub async fn get_sync_cursor(db: &DatabaseConnection) -> Result<u64, String> {
    let sql = "SELECT sync_cursor FROM sync_meta WHERE id = 1";
    let stmt = Statement::from_string(sea_orm::DatabaseBackend::Sqlite, sql.to_owned());
    let row = CursorRow::find_by_statement(stmt)
        .one(db)
        .await
        .map_err(|e| format!("get_sync_cursor: {e}"))?;
    Ok(row.map(|r| r.sync_cursor).unwrap_or(0).max(0) as u64)
}

#[derive(Debug, FromQueryResult)]
struct CursorRow {
    sync_cursor: i64,
}
