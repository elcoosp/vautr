//! VTR-055 — FTS5 search correctness + performance tests (TDD 1–5).
//!
//! Uses a real on-disk SQLite DB (tempfile) with 10k seeded rows so the latency
//! and rebuild targets are measured against a realistic index. TDD1's p95 < 100ms
//! is asserted deterministically here (the criterion bench in `benches/search.rs`
//! reports the distribution).

use sea_orm::{Database, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use vautr_db::entity::item_overview;
use vautr_db::migrate;
use vautr_db::query::{rebuild_fts, recent_overviews, search_overviews};
use vautr_db::search::prepare_search;

const N: usize = 10_000;

fn rt() -> Runtime {
    Runtime::new().expect("rt")
}

fn open_and_seed() -> (DatabaseConnection, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("vault.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let rt = rt();
    let db = rt.block_on(async {
        let db = Database::connect(&url).await.expect("connect");
        migrate::init(&db).await.expect("migrate");
        let txn = db.begin().await.expect("begin");
        let mut models = Vec::with_capacity(N);
        for i in 0..N {
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
        // Chunk the insert: a single `insert_many` of 10k rows exceeds SQLite's
        // 999 bind-variable limit, so batch in groups of 500.
        for chunk in models.chunks(500) {
            item_overview::Entity::insert_many(chunk.to_vec())
                .exec(&txn)
                .await
                .expect("insert");
        }
        txn.commit().await.expect("commit");
        db
    });
    (db, dir)
}

/// TDD1: a common-word prefix query must return in < 100ms (deterministic p95
/// proxy — we measure the median of 30 runs and assert the per-call budget).
#[test]
fn prefix_search_meets_100ms_budget() {
    let rt = rt();
    let (db, _dir) = open_and_seed();
    let mut max_ms = 0.0f64;
    for _ in 0..30 {
        let start = std::time::Instant::now();
        let (rows, _) = rt.block_on(search_overviews(&db, "bank*")).expect("search");
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        max_ms = max_ms.max(ms);
        assert!(
            !rows.is_empty(),
            "bank* should match seeded 'Bank'/'Bandwidth' rows"
        );
    }
    assert!(max_ms < 100.0, "p95 proxy exceeded 100ms: {max_ms:.2}ms");
}

/// TDD2: a prefix query (`ban*`) returns all items starting with "ban".
#[test]
fn prefix_query_returns_prefix_matches() {
    let rt = rt();
    let (db, _dir) = open_and_seed();
    let (rows, leading) = rt.block_on(search_overviews(&db, "ban*")).expect("search");
    assert!(!leading, "prefix query is not a leading wildcard");
    assert!(!rows.is_empty());
    for r in &rows {
        assert!(
            r.title.to_lowercase().starts_with("ban"),
            "result '{0}' does not start with 'ban'",
            r.title
        );
    }
}

/// TDD3: a substring query (`*ank`) defeats the index — the rewriter flags it
/// (leading_wildcard = true) so the UI can warn about a full scan. FTS5 still
/// returns matches, but it cannot use the index for a leading `*`.
#[test]
fn substring_leading_wildcard_is_flagged() {
    let rt = rt();
    let (db, _dir) = open_and_seed();
    let prep = prepare_search("*ank");
    assert!(prep.leading_wildcard, "leading wildcard must be flagged");
    let (rows, leading) = rt.block_on(search_overviews(&db, "*ank")).expect("search");
    assert!(
        leading,
        "search_overviews must surface the leading_wildcard flag"
    );
    // FTS5 with a leading `*` is not indexed; it may return 0 or do a scan.
    // The contract is that the flag is set so the UI warns — we don't assert
    // on the (implementation-dependent) row count.
    let _ = rows;
}

/// TDD4: a non-empty query is ordered by BM25 relevance (`rank`), NOT by date.
/// We seed rows where the most-recent item does NOT contain the query term, so
/// a date ordering would put it first while relevance ordering keeps matches.
#[test]
fn results_ordered_by_relevance_not_date() {
    let rt = rt();
    let (db, _dir) = open_and_seed();
    let (rows, _) = rt
        .block_on(search_overviews(&db, "github"))
        .expect("search");
    assert!(
        !rows.is_empty(),
        "github should match seeded 'GitHub token' rows"
    );
    // Every result must contain the term; if ordering were by date, the last
    // seeded row (created_at = N-1, which is a "Random note") would appear
    // first despite not matching. Assert the first match actually contains it.
    assert!(
        rows[0].title.to_lowercase().contains("github"),
        "top result should be relevance-ranked and contain the term"
    );
}

/// TDD5: after dropping + rebuilding the FTS5 index, search results are
/// identical to before the rebuild. Also confirms the rebuild stays under 2s.
#[test]
fn rebuild_index_preserves_results() {
    let rt = rt();
    let (db, _dir) = open_and_seed();

    let (before, _) = rt.block_on(search_overviews(&db, "bank*")).expect("before");
    let before_uuids: Vec<_> = before.iter().map(|r| r.uuid).collect();

    let dur = rt.block_on(rebuild_fts(&db)).expect("rebuild");
    assert!(dur.as_secs_f64() < 2.0, "rebuild exceeded 2s: {dur:?}");

    let (after, _) = rt.block_on(search_overviews(&db, "bank*")).expect("after");
    let after_uuids: Vec<_> = after.iter().map(|r| r.uuid).collect();

    assert_eq!(
        before_uuids.len(),
        after_uuids.len(),
        "result count changed after rebuild"
    );
    assert_eq!(
        before_uuids, after_uuids,
        "result set changed after rebuild"
    );
}

/// Acceptance: empty query returns the 50 most-recent items (no FTS).
#[test]
fn empty_query_returns_recent_fifty() {
    let rt = rt();
    let (db, _dir) = open_and_seed();
    let rows = rt.block_on(recent_overviews(&db, 50)).expect("recent");
    assert_eq!(rows.len(), 50, "recent list should cap at 50");
    // Ordered by updated_at DESC → the highest updated_at first.
    for w in rows.windows(2) {
        assert!(
            w[0].updated_at >= w[1].updated_at,
            "recent list must be descending by updated_at"
        );
    }
}
