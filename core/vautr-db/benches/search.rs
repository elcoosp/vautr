//! VTR-055 — FTS5 search benchmark with 10k items (criterion).
//!
//! Seeds 10,000 overview rows and measures search latency for three query
//! shapes: prefix (`bank*`), exact (`github`), and a leading-wildcard substring
//! (`*ank`). Also benchmarks the FTS5 index rebuild (drop + recreate + re-index)
//! to confirm the < 2s target for 10k items.
//!
//! The numeric p95 < 100ms / rebuild < 2s assertions are enforced by the
//! integration tests in `tests/search.rs` (deterministic); this bench reports
//! the distribution for human inspection via `cargo bench -p vautr-db`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use vautr_db::entity::item_overview;
use vautr_db::migrate;
use vautr_db::query::{rebuild_fts, search_overviews};

/// Seed `n` overview rows with a rotating title scheme so the benchmarked
/// queries have real matches. Runs inside a single transaction (FTS triggers
/// fire per row, exercising the realistic indexed path).
async fn seed(db: &DatabaseConnection, n: usize) {
    let txn = db.begin().await.expect("begin");
    let mut models = Vec::with_capacity(n);
    for i in 0..n {
        let title = match i % 5 {
            0 => format!("Bank login {i}"),
            1 => format!("Bandwidth key {i}"),
            2 => format!("Submarine ank password {i}"),
            3 => format!("GitHub token {i}"),
            _ => format!("Random note {i}"),
        };
        models.push(item_overview::ActiveModel {
            uuid: Set(format!("00000000-0000-0000-0000-{i:012x}")),
            version: Set(1),
            enc_key_gen: Set(1),
            deleted_date: Set(None),
            overview_title: Set(title),
            overview_subtitle: Set("login".into()),
            overview_icon_key: Set("web".into()),
            overview_urls: Set("[]".into()),
            created_at: Set(i as i64),
            updated_at: Set(i as i64),
        });
    }
    // Chunk: a single insert_many of 10k rows exceeds SQLite's 999 bind-variable
    // limit, so batch in groups of 500.
    for chunk in models.chunks(500) {
        item_overview::Entity::insert_many(chunk.to_vec())
            .exec(&txn)
            .await
            .expect("insert");
    }
    txn.commit().await.expect("commit");
}

fn open() -> (DatabaseConnection, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("vault.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let rt = Runtime::new().expect("rt");
    let db = rt.block_on(async {
        let db = Database::connect(&url).await.expect("connect");
        migrate::init(&db).await.expect("migrate");
        db
    });
    (db, dir)
}

fn search_bench(c: &mut Criterion) {
    let rt = Runtime::new().expect("rt");
    let (db, _dir) = open();
    rt.block_on(seed(&db, 10_000));

    let mut group = c.benchmark_group("fts5_search_10k");
    group.sample_size(50);
    group.measurement_time(std::time::Duration::from_secs(5));

    for (name, q) in [
        ("prefix_bank", "bank*"),
        ("exact_github", "github"),
        ("substring_ank_leading_wildcard", "*ank"),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), &q, |b, q| {
            b.iter(|| {
                let _ = rt.block_on(search_overviews(&db, q)).expect("search");
            });
        });
    }
    group.finish();

    // Index rebuild (drop + recreate + re-index) must stay under 2s for 10k.
    let mut rebuild = c.benchmark_group("fts5_rebuild_10k");
    rebuild.sample_size(20);
    rebuild.bench_function("rebuild", |b| {
        b.iter(|| {
            let d = rt.block_on(rebuild_fts(&db)).expect("rebuild");
            assert!(d.as_secs_f64() < 2.0, "rebuild exceeded 2s: {d:?}");
        });
    });
    rebuild.finish();
}

criterion_group!(benches, search_bench);
criterion_main!(benches);
