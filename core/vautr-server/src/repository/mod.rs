//! SQLx repository for the Vautr server.
//!
//! All queries scope strictly by `user_id` to prevent cross-tenant access.
//! Schema: [`docs/architecture/server-db.md`]. OCC (ADR-004) and epoch gating
//! (REQ-API-01) are implemented in `upsert_item_occ`.
//!
//! ⚠ OPAQUE record storage and the sharing KEM are implemented in `vautr-crypto`
//! (`opaque` + `sharing` modules) and wired here via `store_session` / `get_session`
//! and the `shares` table. See docs/architecture/adr-007-sharing-kem.md.
//! `opaque_record` / `shares` columns exist but are not yet written by the
//! auth flow.
//!
//! This module owns the `Repository` type and re-exports the per-domain row /
//! outcome types so the public surface stays `repository::Repository`,
//! `repository::UserRow`, `repository::ItemRow`, and `repository::UpsertOutcome`.
//! Methods are split into per-domain modules: `users`, `items`, `sessions`,
//! `config`.

use sqlx::sqlite::SqlitePool;

pub mod audit;
pub mod backup;
pub mod config;
pub mod files;
pub mod items;
pub mod machine_accounts;
pub mod mfa;
pub mod projects;
pub mod recovery;
pub mod secrets;
pub mod sessions;
pub mod sharing;
pub mod users;
/// WebAuthn (FIDO2) optional second factor (VTR-052). Feature-gated, off by default.
#[cfg(feature = "webauthn")]
pub mod webauthn;

pub use items::{ItemRow, UpsertOutcome};
pub use users::UserRow;
#[cfg(feature = "webauthn")]
pub use webauthn::WebauthnCredentialRow;

/// The server repository: a thin, tenant-scoped wrapper over the SQLite pool.
#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        // Unique temp file DB so tests don't collide and the schema is real.
        let path = std::env::temp_dir().join(format!("vautr_srv_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    #[tokio::test]
    async fn migrations_create_all_tables() {
        let repo = test_repo().await;
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name",
        )
        .fetch_all(repo.pool())
        .await
        .unwrap();
        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        for expected in [
            "users",
            "items",
            "sessions",
            "shares",
            "server_config",
            "webauthn_credentials",
        ] {
            assert!(
                names.contains(&expected),
                "missing table: {expected} (got {names:?})"
            );
        }
    }

    #[tokio::test]
    async fn user_and_item_occ_flow() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        repo.create_user(
            "u1",
            "alice@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();

        // First write: target_version 1, but the row doesn't exist yet, so OCC
        // reports Conflict (caller must insert on 404/initial). We model the
        // initial insert separately here to keep upsert_item_occ pure-OCC.
        sqlx::query(
            "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
             VALUES (?, ?, 1, 1, NULL, ?, ?)",
        )
        .bind("item-1")
        .bind("u1")
        .bind(&[9u8; 8][..])
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();

        // Correct OCC version → Updated.
        let out = repo
            .upsert_item_occ("item-1", "u1", 1, 1, Some(&[9u8; 8][..]), None, now + 1)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::Updated);

        // Stale OCC version → Conflict.
        let out = repo
            .upsert_item_occ("item-1", "u1", 1, 1, Some(&[9u8; 8][..]), None, now + 2)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::Conflict);

        // Raise epoch; old enc_key_gen rejected (REQ-API-01).
        sqlx::query("UPDATE users SET min_enc_key_gen = 3 WHERE id = 'u1'")
            .execute(repo.pool())
            .await
            .unwrap();
        let out = repo
            .upsert_item_occ("item-1", "u1", 2, 1, Some(&[9u8; 8][..]), None, now + 3)
            .await
            .unwrap();
        assert_eq!(out, UpsertOutcome::EpochTooOld);
    }
}
