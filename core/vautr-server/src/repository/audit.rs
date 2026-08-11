//! Audit-log repository — metadata-only security event recording.
//! Table: `audit_log` (0005_audit.sql). Spec: server-scaling.md §8, VTR-053.
//!
//! Entries store a `user_id` (never the raw email), an `action`, an optional
//! `actor`, optional `detail`, and a timestamp. They NEVER store request
//! payloads, encrypted blobs, item UUIDs, or sync metrics — those are metadata
//! exfiltration vectors (server-scaling.md §8).

use sqlx::FromRow;

use crate::repository::Repository;

/// Row mirror of `audit_log` (0005_audit.sql).
#[derive(Debug, Clone, FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub user_id: Option<String>,
    pub action: String,
    pub actor: Option<String>,
    pub detail: Option<String>,
    pub created_at: i64,
}

impl Repository {
    /// Append a metadata-only audit entry.
    ///
    /// `user_id` is the affected account (not the email); `actor` is who
    /// triggered the event (usually the same account). `detail` is reserved for
    /// non-sensitive context and MUST NOT contain PII or payload data.
    pub async fn audit_log(
        &self,
        user_id: Option<&str>,
        action: &str,
        actor: Option<&str>,
        detail: Option<&str>,
        created_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO audit_log (user_id, action, actor, detail, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(action)
        .bind(actor)
        .bind(detail)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Query audit entries, newest first.
    ///
    /// `user_id` optionally scopes to one account; when `None` all accounts are
    /// returned (admin view). Bounded by `limit`/`offset` for pagination.
    pub async fn list_audit_logs(
        &self,
        user_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditRow>, sqlx::Error> {
        sqlx::query_as::<_, AuditRow>(
            "SELECT id, user_id, action, actor, detail, created_at \
             FROM audit_log \
             WHERE (? IS NULL OR user_id = ?) \
             ORDER BY id DESC \
             LIMIT ? OFFSET ?",
        )
        .bind(user_id)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        let path =
            std::env::temp_dir().join(format!("vautr_audit_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    #[tokio::test]
    async fn audit_log_write_and_query() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        // user_id is FK to users(id); create the account first.
        repo.create_user("u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();

        repo.audit_log(Some("u1"), "rotate_key", Some("u1"), None, now)
            .await
            .unwrap();
        repo.audit_log(Some("u1"), "account_deleted", Some("u1"), None, now + 1)
            .await
            .unwrap();
        repo.audit_log(None, "reclaim_initiated", Some("admin"), None, now + 2)
            .await
            .unwrap();

        // Scoped by user.
        let u1 = repo.list_audit_logs(Some("u1"), 100, 0).await.unwrap();
        assert_eq!(u1.len(), 2);
        assert_eq!(u1[0].action, "account_deleted"); // newest first
        assert_eq!(u1[0].user_id.as_deref(), Some("u1"));

        // Unscoped admin view includes the reclaim row (user_id NULL).
        let all = repo.list_audit_logs(None, 100, 0).await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].action, "reclaim_initiated");
        assert_eq!(all[0].user_id, None);

        // Metadata-only: no payloads / item UUIDs / raw emails stored.
        for row in &all {
            assert!(row.detail.is_none());
            assert!(!row.action.contains("item"));
            assert_ne!(row.user_id.as_deref(), Some("alice@example.com"));
        }
    }

    #[tokio::test]
    async fn audit_log_limit_offset() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        repo.create_user("u2", "bob@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        for i in 0..5 {
            repo.audit_log(Some("u2"), "login", Some("u2"), None, now + i)
                .await
                .unwrap();
        }
        let page1 = repo.list_audit_logs(Some("u2"), 2, 0).await.unwrap();
        assert_eq!(page1.len(), 2);
        let page2 = repo.list_audit_logs(Some("u2"), 2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);
        let page3 = repo.list_audit_logs(Some("u2"), 2, 4).await.unwrap();
        assert_eq!(page3.len(), 1);
        // Newest first ordering across pages.
        assert!(page1[0].created_at > page1[1].created_at);
    }
}
