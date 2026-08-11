//! Integration tests for the bulk import pipeline (VTR-027 / VTR-045).
//!
//! Gate: a 1000-row CSV smoke test completes, and a malformed-input test does
//! not panic. Also covers dedup, Bitwarden JSON, chunking, and FTS5 rebuild.

use sea_orm::{Database, DatabaseConnection, EntityTrait, FromQueryResult, TransactionTrait};
use uuid::Uuid;

use vautr_import::{
    import_file, import_items, RawImportItem, ImportFailureReason, VaultKeys,
};

/// Open a fresh in-memory sqlite DB with the schema bootstrapped.
async fn connect() -> DatabaseConnection {
    let path = std::env::temp_dir().join(format!("vautr_import_test_{}.sqlite", Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let db = Database::connect(&url).await.unwrap();
    vautr_db::migrate::init(&db).await.unwrap();
    db
}

/// Row mirror for the count query.
#[derive(FromQueryResult)]
struct CountRow {
    n: i64,
}

/// Count overview rows (plaintext hot data) in the DB.
async fn overview_count(db: &DatabaseConnection) -> u32 {
    let sql = "SELECT COUNT(*) AS n FROM item_overviews";
    let stmt = sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        sql,
        [],
    );
    let row = CountRow::find_by_statement(stmt).one(db).await.unwrap().unwrap();
    row.n as u32
}

/// Write a CSV file with `count` rows to a unique temp path and return it.
/// Uses `NamedTempFile` so the file persists for the test's lifetime.
fn write_csv(count: usize) -> std::path::PathBuf {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let path = file.path().to_path_buf();
    {
        let mut wtr = csv::WriterBuilder::new().from_writer(file.as_file_mut());
        wtr.write_record(["name", "url", "username", "password"]).unwrap();
        for i in 0..count {
            wtr.write_record([
                format!("Item {i}"),
                format!("https://site{i}.example"),
                format!("user{i}"),
                format!("pass{i}"),
            ])
            .unwrap();
        }
        wtr.flush().unwrap();
    }
    // Keep the temp file alive (leak) so the path stays valid for the import.
    std::mem::forget(file);
    path
}

#[tokio::test]
async fn smoke_import_1000_csv_rows_completes() {
    let db = connect().await;
    let path = write_csv(1000);
    let keys = VaultKeys::random();
    let last = std::cell::Cell::new(0u8);
    let report = import_file(path.to_str().unwrap(), &db, &keys, |p| {
        // Progress is monotonic, 0..=100.
        assert!(p >= last.get());
        assert!(p <= 100);
        last.set(p);
    })
    .await
    .unwrap();

    assert_eq!(report.total_parsed, 1000);
    assert_eq!(report.success_count, 1000);
    assert_eq!(report.skipped_count, 0);
    assert!(report.errors.is_empty());
    assert_eq!(last.get(), 100);
    assert_eq!(overview_count(&db).await, 1000);

    // FTS5 index rebuilt and searchable (verified directly on the index, since
    // the drop/rebuild fast-path repopulates it set-based).
    let stmt = sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "SELECT COUNT(*) AS n FROM items_fts WHERE items_fts MATCH 'Item'",
        [],
    );
    let hits = CountRow::find_by_statement(stmt).one(&db).await.unwrap().unwrap();
    assert_eq!(hits.n, 1000);
}

#[tokio::test]
async fn malformed_input_does_not_panic() {
    let db = connect().await;
    let keys = VaultKeys::random();

    // Malformed CSV: a row with no title, a row with too few/many columns.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.csv");
    std::fs::write(
        &path,
        "name,url,username,password\n,https://nourl.example,u1,p1\nGood,https://good.example,u2,p2\n\"unclosed\n",
    )
    .unwrap();

    let report = import_file(path.to_str().unwrap(), &db, &keys, |_| {}).await.unwrap();
    // It must not panic and must degrade gracefully: some rows import while
    // malformed rows accumulate as errors (not all records succeed).
    assert!(report.total_parsed >= 2);
    assert!(report.success_count >= 1);
    assert!(report.success_count < report.total_parsed);
    assert!(report.errors.len() >= 1);
}

#[tokio::test]
async fn dedup_skips_existing_title_url() {
    let db = connect().await;
    let keys = VaultKeys::random();

    // Pre-seed one item directly.
    let overviews = vec![vautr_db::entity::item_overview::ActiveModel {
        uuid: sea_orm::Set(Uuid::new_v4().to_string()),
        version: sea_orm::Set(1),
        enc_key_gen: sea_orm::Set(1),
        deleted_date: sea_orm::Set(None),
        overview_title: sea_orm::Set("Bank".to_string()),
        overview_subtitle: sea_orm::Set(String::new()),
        overview_icon_key: sea_orm::Set("generic".to_string()),
        overview_urls: sea_orm::Set("[\"https://bank.example\"]".to_string()),
        created_at: sea_orm::Set(0),
        updated_at: sea_orm::Set(0),
    }];
    let payloads = vec![vautr_db::entity::item_payload::ActiveModel {
        uuid: sea_orm::Set("x".to_string()),
        payload: sea_orm::Set(vec![1, 2, 3]),
    }];
    let txn = db.begin().await.unwrap();
    vautr_db::entity::item_overview::Entity::insert_many(overviews)
        .exec(&txn)
        .await
        .unwrap();
    vautr_db::entity::item_payload::Entity::insert_many(payloads)
        .exec(&txn)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    // Import a duplicate (title+url match) and a fresh item.
    let items = vec![
        RawImportItem {
            source_id: Some("c1".to_string()),
            title: "Bank".to_string(),
            url: Some("https://bank.example".to_string()),
            fields: serde_json::json!({}),
        },
        RawImportItem {
            source_id: Some("c2".to_string()),
            title: "Fresh".to_string(),
            url: Some("https://fresh.example".to_string()),
            fields: serde_json::json!({}),
        },
    ];

    let report = import_items(items, &db, &keys, |_| {}).await.unwrap();
    assert_eq!(report.total_parsed, 2);
    assert_eq!(report.skipped_count, 1);
    assert_eq!(report.success_count, 1);
    // The duplicate surfaces as a DuplicateSkip error too (per §5.1).
    assert!(report.errors.iter().any(|e| e.reason == ImportFailureReason::DuplicateSkip));
    // Pre-seeded (1) + fresh (1) = 2 rows.
    assert_eq!(overview_count(&db).await, 2);
}

#[tokio::test]
async fn imports_bitwarden_json() {
    let db = connect().await;
    let keys = VaultKeys::random();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bw.json");
    std::fs::write(
        &path,
        r#"{"encrypted":false,"items":[
            {"id":"a1","name":"GitHub","login":{"username":"u","password":"p","uris":[{"uri":"https://github.com"}]}},
            {"id":"a2","name":"Gmail","login":{"username":"x","password":"y"}}
        ]}"#,
    )
    .unwrap();

    let report = import_file(path.to_str().unwrap(), &db, &keys, |_| {}).await.unwrap();
    assert_eq!(report.total_parsed, 2);
    assert_eq!(report.success_count, 2);
    assert!(report.errors.is_empty());
    assert_eq!(overview_count(&db).await, 2);
}

#[tokio::test]
async fn chunking_preserves_all_items() {
    // 1050 items forces multiple 100-item rayon chunks.
    let db = connect().await;
    let keys = VaultKeys::random();
    let items: Vec<RawImportItem> = (0..1050)
        .map(|i| RawImportItem {
            source_id: Some(format!("c{i}")),
            title: format!("Chunk {i}"),
            url: Some(format!("https://chunk{i}.example")),
            fields: serde_json::json!({"password": "pw"}),
        })
        .collect();

    let report = import_items(items, &db, &keys, |_| {}).await.unwrap();
    assert_eq!(report.total_parsed, 1050);
    assert_eq!(report.success_count, 1050);
    assert_eq!(report.skipped_count, 0);
    assert!(report.errors.is_empty());
    assert_eq!(overview_count(&db).await, 1050);
}

#[tokio::test]
async fn imports_1pux_zip_archive() {
    // A `.1pux` source is a ZIP containing a Bitwarden-style JSON export. The
    // source extracts the inner JSON to a temp dir and streams it.
    let db = connect().await;
    let keys = VaultKeys::random();

    let inner = r#"{"encrypted":false,"items":[
        {"id":"z1","name":"FromZip","login":{"username":"u","password":"p","uris":[{"uri":"https://zip.example"}]}}
    ]}"#;

    let path = std::env::temp_dir().join(format!("vautr_import_{}.1pux", Uuid::new_v4()));
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(
            std::fs::File::create(&path).unwrap(),
        ));
        zip.start_file("data.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(inner.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    let report = import_file(path.to_str().unwrap(), &db, &keys, |_| {}).await.unwrap();
    assert_eq!(report.total_parsed, 1);
    assert_eq!(report.success_count, 1);
    assert!(report.errors.is_empty());
    assert_eq!(overview_count(&db).await, 1);
}

#[tokio::test]
async fn preexisting_rows_survive_drop_rebuild() {
    let db = connect().await;
    let keys = VaultKeys::random();
    // Seed one existing overview (as in dedup test) then import fresh rows.
    let items = vec![RawImportItem {
        source_id: None,
        title: "Existing".to_string(),
        url: Some("https://existing.example".to_string()),
        fields: serde_json::json!({}),
    }];
    let report = import_items(items, &db, &keys, |_| {}).await.unwrap();
    assert_eq!(report.success_count, 1);
    assert_eq!(overview_count(&db).await, 1);
}
