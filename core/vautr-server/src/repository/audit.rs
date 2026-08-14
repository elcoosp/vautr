//! Audit-log repository — metadata-only security event recording (Wave A7).
//! Table: `audit_log` (0005_audit.sql + 0012_audit_events.sql). Spec:
//! mlp-scope.md §4/§5, server-scaling.md §8, VTR-053.
//!
//! Entries store a `user_id` (never the raw email), an `action`, an optional
//! `actor`, optional `detail`, a timestamp, plus the Wave-A7 classification
//! columns: `event_type` (`org` / `secret_access` / ...), `resource_type`,
//! `resource_id`, and `ip_address`. They NEVER store request payloads,
//! encrypted blobs, item UUIDs, or sync metrics — those are metadata
//! exfiltration vectors (server-scaling.md §8).

use sqlx::{FromRow, QueryBuilder};

use crate::repository::Repository;

/// Row mirror of `audit_log` (0005_audit.sql + 0012_audit_events.sql).
#[derive(Debug, Clone, FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub user_id: Option<String>,
    pub action: String,
    pub actor: Option<String>,
    pub detail: Option<String>,
    pub created_at: i64,
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
}

/// Filter for [`Repository::query_audit_events`]. Every field is optional and
/// AND-ed together; `None` means "no constraint on that dimension".
#[derive(Debug, Clone, Default)]
pub struct AuditFilter<'a> {
    /// Affected account id.
    pub user_id: Option<&'a str>,
    /// Who triggered the event.
    pub actor: Option<&'a str>,
    /// Category: `org`, `secret_access`, ...
    pub event_type: Option<&'a str>,
    /// Kind of resource, e.g. `project`, `org_member`, `secret`.
    pub resource_type: Option<&'a str>,
    /// Specific resource UUID.
    pub resource_id: Option<&'a str>,
    /// Lower bound (inclusive) on `created_at` (ms).
    pub from: Option<i64>,
    /// Upper bound (inclusive) on `created_at` (ms).
    pub to: Option<i64>,
}

impl Repository {
    /// Append a metadata-only audit entry (legacy shape; 0005 columns only).
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
            "INSERT INTO audit_log (user_id, action, actor, detail, created_at, event_type) \
             VALUES (?, ?, ?, ?, ?, 'auth')",
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

    /// **Published recording API (Wave A7).** Record an org-level event.
    ///
    /// Call this from project/membership/offboarding/MFA/policy handlers when a
    /// security-relevant org action happens. `resource_type` is one of
    /// `project`, `org`, `org_member`, `policy`, `mfa`, `offboarding`, ... and
    /// `resource_id` is the UUID of the affected object (never PII/payload).
    pub async fn audit_org_event(
        &self,
        actor: Option<&str>,
        user_id: Option<&str>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        detail: Option<&str>,
        ip: Option<&str>,
        created_at: i64,
    ) -> Result<(), sqlx::Error> {
        self.insert_event(
            "org",
            actor,
            user_id,
            action,
            resource_type,
            resource_id,
            detail,
            ip,
            created_at,
        )
        .await
    }

    /// **Published recording API (Wave A7).** Record a secret-access event —
    /// who accessed which secret and when.
    ///
    /// The **A3 secrets handlers** call this after every read/write/delete of a
    /// secret. `secret_id` is the secret UUID; `project_id` is optional context
    /// stored in `detail` (still metadata, never the secret value). `action` is
    /// one of `read`, `list`, `create`, `update`, `delete`.
    ///
    /// The actor is stored in `actor` (who accessed). `user_id` is left NULL
    /// because the actor may be a human user *or* a machine account (A2) — the
    /// `audit_log.user_id` FK points at `users(id)`, so it cannot hold a machine
    /// identity. `actor` is the authoritative "who" for secret access.
    pub async fn audit_secret_access(
        &self,
        actor: &str,
        secret_id: &str,
        project_id: Option<&str>,
        action: &str,
        ip: Option<&str>,
        created_at: i64,
    ) -> Result<(), sqlx::Error> {
        let detail = project_id.map(|p| format!("project={p}"));
        self.insert_event(
            "secret_access",
            Some(actor),
            None, // actor may be a machine account; user_id is left NULL
            action,
            "secret",
            Some(secret_id),
            detail.as_deref(),
            ip,
            created_at,
        )
        .await
    }

    /// Shared INSERT for every event type (all columns, 0012).
    #[allow(clippy::too_many_arguments)]
    async fn insert_event(
        &self,
        event_type: &str,
        actor: Option<&str>,
        user_id: Option<&str>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        detail: Option<&str>,
        ip: Option<&str>,
        created_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO audit_log \
               (user_id, action, actor, detail, created_at, event_type, resource_type, resource_id, ip_address) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(action)
        .bind(actor)
        .bind(detail)
        .bind(created_at)
        .bind(event_type)
        .bind(resource_type)
        .bind(resource_id)
        .bind(ip)
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
        let filter = AuditFilter {
            user_id,
            ..AuditFilter::default()
        };
        self.query_audit_events(&filter, limit, offset).await
    }

    /// Query audit events with filters on actor / resource / time / category.
    ///
    /// Newest first, bounded by `limit`/`offset`. Filters are optional and
    /// AND-ed together. This backs the `GET /audit` admin endpoint.
    pub async fn query_audit_events(
        &self,
        f: &AuditFilter<'_>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditRow>, sqlx::Error> {
        let mut qb = QueryBuilder::new(
            "SELECT id, user_id, action, actor, detail, created_at, event_type, \
                    resource_type, resource_id, ip_address \
             FROM audit_log",
        );
        qb.push(" WHERE 1=1");
        if let Some(uid) = f.user_id {
            qb.push(" AND user_id = ").push_bind(uid);
        }
        if let Some(actor) = f.actor {
            qb.push(" AND actor = ").push_bind(actor);
        }
        if let Some(et) = f.event_type {
            qb.push(" AND event_type = ").push_bind(et);
        }
        if let Some(rt) = f.resource_type {
            qb.push(" AND resource_type = ").push_bind(rt);
        }
        if let Some(rid) = f.resource_id {
            qb.push(" AND resource_id = ").push_bind(rid);
        }
        if let Some(from) = f.from {
            qb.push(" AND created_at >= ").push_bind(from);
        }
        if let Some(to) = f.to {
            qb.push(" AND created_at <= ").push_bind(to);
        }
        qb.push(" ORDER BY id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        qb.build_query_as::<AuditRow>().fetch_all(&self.pool).await
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
        repo.create_user(
            "u2",
            "bob@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
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

    #[tokio::test]
    async fn org_and_secret_access_events_recorded_and_queryable() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        repo.create_user(
            "owner",
            "owner@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();
        repo.create_user(
            "dev",
            "dev@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();

        let proj = uuid::Uuid::new_v4().to_string();
        let secret = uuid::Uuid::new_v4().to_string();

        // Org events: membership role change, project create, offboarding.
        repo.audit_org_event(
            Some("owner"),
            Some("dev"),
            "role_change",
            "org_member",
            Some("dev"),
            Some("Member -> Admin"),
            Some("10.0.0.1"),
            now,
        )
        .await
        .unwrap();
        repo.audit_org_event(
            Some("owner"),
            Some("owner"),
            "project_create",
            "project",
            Some(&proj),
            None,
            None,
            now + 1,
        )
        .await
        .unwrap();
        repo.audit_org_event(
            Some("owner"),
            Some("dev"),
            "offboard",
            "offboarding",
            Some("dev"),
            None,
            Some("10.0.0.1"),
            now + 2,
        )
        .await
        .unwrap();

        // Secret access: who read which secret and when.
        repo.audit_secret_access(
            "dev",
            &secret,
            Some(&proj),
            "read",
            Some("10.0.0.9"),
            now + 3,
        )
        .await
        .unwrap();

        // Filter by event_type.
        let secret_events = repo
            .query_audit_events(
                &AuditFilter {
                    event_type: Some("secret_access"),
                    ..AuditFilter::default()
                },
                100,
                0,
            )
            .await
            .unwrap();
        assert_eq!(secret_events.len(), 1);
        assert_eq!(secret_events[0].action, "read");
        assert_eq!(secret_events[0].resource_type.as_deref(), Some("secret"));
        assert_eq!(
            secret_events[0].resource_id.as_deref(),
            Some(secret.as_str())
        );
        assert_eq!(secret_events[0].actor.as_deref(), Some("dev"));
        assert_eq!(secret_events[0].ip_address.as_deref(), Some("10.0.0.9"));

        // Filter by actor + resource together.
        let dev_org = repo
            .query_audit_events(
                &AuditFilter {
                    actor: Some("owner"),
                    resource_type: Some("org_member"),
                    ..AuditFilter::default()
                },
                100,
                0,
            )
            .await
            .unwrap();
        assert_eq!(dev_org.len(), 1);
        assert_eq!(dev_org[0].action, "role_change");
        assert_eq!(dev_org[0].event_type.as_deref(), Some("org"));

        // Filter by time window.
        let in_window = repo
            .query_audit_events(
                &AuditFilter {
                    from: Some(now),
                    to: Some(now + 2),
                    ..AuditFilter::default()
                },
                100,
                0,
            )
            .await
            .unwrap();
        assert_eq!(in_window.len(), 3); // offboard(now+2) + project_create(now+1) + role_change(now)
                                        // Newest first within the window.
        assert_eq!(in_window[0].action, "offboard");

        // Metadata-only: secret value never stored, only its UUID.
        let all = repo
            .query_audit_events(&AuditFilter::default(), 100, 0)
            .await
            .unwrap();
        for row in &all {
            assert!(
                row.detail.is_none() || !row.detail.as_deref().unwrap().contains("secret_value")
            );
            assert_ne!(row.user_id.as_deref(), Some("dev@example.com"));
        }
    }

    #[tokio::test]
    async fn secret_access_persists_no_payload() {
        let repo = test_repo().await;
        let now = 1_700_000_000_000;
        let secret = uuid::Uuid::new_v4().to_string();
        repo.audit_secret_access("svc-1", &secret, None, "create", None, now)
            .await
            .unwrap();
        let rows = repo
            .query_audit_events(&AuditFilter::default(), 100, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        // resource_id is the secret UUID only; the value is never stored.
        assert_eq!(rows[0].resource_id.as_deref(), Some(secret.as_str()));
        assert_eq!(rows[0].event_type.as_deref(), Some("secret_access"));
        assert_eq!(rows[0].resource_type.as_deref(), Some("secret"));
        // No detail (no secret value, no PII) and no payload-like column.
        assert!(rows[0].detail.is_none());
        assert!(rows[0].ip_address.is_none());
    }
}
