//! VTRFIX-TST-07: auth-state machine invariants.

use std::sync::Arc;
use vautr_server::db;
use vautr_server::handlers::auth::sha256_hex;
use vautr_server::repository::Repository;

async fn test_repo() -> Arc<Repository> {
    // `sqlite::memory:` via the server's own connect helper (runs migrations).
    let pool = db::connect("sqlite::memory:").await.expect("connect+migrate");
    Arc::new(Repository::new(pool))
}

#[tokio::test]
async fn session_token_is_hashed_not_stored_raw() {
    let repo = test_repo().await;
    let raw = "vtr-session-secret-abcdef";
    let now = 1_700_000_000_000i64;
    repo.create_user("u1", "a@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
        .await
        .unwrap();
    repo.store_session(raw, "u1", now + 86_400_000).await.unwrap();

    let raw_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE token_hash = ?")
            .bind(raw)
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(raw_count, 0, "raw token must NOT be present in the DB");

    let hashed = repo.get_session(raw).await.unwrap();
    assert!(hashed.is_some(), "hashed lookup must succeed");
}

#[tokio::test]
async fn pending_mfa_is_single_use() {
    let repo = test_repo().await;
    let now = 1_700_000_000_000i64;
    repo.create_user("u1", "a@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
        .await
        .unwrap();
    let hash = sha256_hex(b"vtr-mfa-test");
    repo.store_pending_mfa(&hash, "u1", now + 300_000, now).await.unwrap();
    assert!(repo.get_pending_mfa(&hash).await.unwrap().is_some());
    repo.delete_pending_mfa(&hash).await.unwrap();
    assert!(repo.get_pending_mfa(&hash).await.unwrap().is_none());
}
