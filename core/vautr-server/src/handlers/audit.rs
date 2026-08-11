//! Audit-log HTTP handlers — admin `GET /audit`.
//! Spec: docs/spec/verification.md §4.2 (NFR-SEC compliance), server-db.md.
//! Backed by the `audit_log` table (0005_audit.sql) via `repository::audit`.
//!
//! The endpoint is metadata-only (server-scaling.md §8): it returns action /
//! user_id / timestamp, never payloads, item UUIDs, or sync metrics.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, Bearer, auth_user};
use crate::repository::audit::AuditFilter;

fn default_limit() -> u32 {
    100
}

#[derive(Deserialize)]
pub(crate) struct AuditQuery {
    /// Optional filter: only return entries for this account (admin scoping).
    pub(crate) user_id: Option<String>,
    /// Filter by who triggered the event.
    pub(crate) actor: Option<String>,
    /// Filter by category: `org`, `secret_access`, `auth`, ...
    pub(crate) event_type: Option<String>,
    /// Filter by kind of resource, e.g. `project`, `org_member`, `secret`.
    pub(crate) resource_type: Option<String>,
    /// Filter by specific resource UUID.
    pub(crate) resource_id: Option<String>,
    /// Lower bound (inclusive) on `created_at`, ms since epoch.
    pub(crate) from: Option<i64>,
    /// Upper bound (inclusive) on `created_at`, ms since epoch.
    pub(crate) to: Option<i64>,
    #[serde(default = "default_limit")]
    pub(crate) limit: u32,
    #[serde(default)]
    pub(crate) offset: u32,
}

#[derive(Serialize)]
pub(crate) struct AuditEntry {
    pub(crate) id: i64,
    pub(crate) user_id: Option<String>,
    pub(crate) action: String,
    pub(crate) actor: Option<String>,
    pub(crate) detail: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) event_type: Option<String>,
    pub(crate) resource_type: Option<String>,
    pub(crate) resource_id: Option<String>,
    pub(crate) ip_address: Option<String>,
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new().route("/audit", get(list_audit))
}

async fn list_audit(
    State(st): State<AppState>,
    auth: Bearer,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEntry>>, ApiError> {
    // Require a valid session. A dedicated admin-role gate is a follow-up;
    // the endpoint is metadata-only and never exposes payloads.
    auth_user(&st.repo, &auth.0).await?;
    let limit = q.limit.clamp(1, 1000) as i64;
    let offset = q.offset as i64;
    let filter = AuditFilter {
        user_id: q.user_id.as_deref(),
        actor: q.actor.as_deref(),
        event_type: q.event_type.as_deref(),
        resource_type: q.resource_type.as_deref(),
        resource_id: q.resource_id.as_deref(),
        from: q.from,
        to: q.to,
    };
    let rows = st
        .repo
        .query_audit_events(&filter, limit, offset)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|r| AuditEntry {
                id: r.id,
                user_id: r.user_id,
                action: r.action,
                actor: r.actor,
                detail: r.detail,
                created_at: r.created_at,
                event_type: r.event_type,
                resource_type: r.resource_type,
                resource_id: r.resource_id,
                ip_address: r.ip_address,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        let path =
            std::env::temp_dir().join(format!("vautr_audit_http_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000;
        repo.create_user("u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64) // far-future expiry relative to real wall-clock
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        repo.audit_log(Some("u1"), "rotate_key", Some("u1"), None, now)
            .await
            .unwrap();
        AppState::new(repo)
    }

    #[tokio::test]
    async fn get_audit_returns_entries() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/audit")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["action"], "rotate_key");
        assert_eq!(arr[0]["user_id"], "u1");
        // No payload keys ever appear.
        assert!(arr[0].get("payload").is_none());
    }

    #[tokio::test]
    async fn get_audit_requires_auth() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(Request::builder().uri("/audit").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    async fn http_get_audit(state: &AppState, query: &str) -> serde_json::Value {
        let app = routes().with_state(state.clone());
        let uri = format!("/audit{query}");
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(&uri)
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn get_audit_filters_by_actor_resource_and_time() {
        let state = test_state().await;
        let now = 1_700_000_000_000;
        // The org event affects user u2; create it to satisfy the users FK.
        state
            .repo
            .create_user("u2", "bob@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        let secret = uuid::Uuid::new_v4().to_string();
        // Record a secret-access event and an org event (published API).
        state
            .repo
            .audit_secret_access("u1", &secret, Some("proj-1"), "read", Some("10.0.0.9"), now + 10)
            .await
            .unwrap();
        state
            .repo
            .audit_org_event(Some("u1"), Some("u2"), "project_create", "project", Some("proj-1"), None, None, now + 20)
            .await
            .unwrap();

        // Filter by event_type.
        let secret_only = http_get_audit(&state, "?event_type=secret_access").await;
        let arr = secret_only.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["resource_type"], "secret");
        assert_eq!(arr[0]["resource_id"], serde_json::json!(secret));
        assert_eq!(arr[0]["action"], "read");
        assert_eq!(arr[0]["ip_address"], "10.0.0.9");

        // Filter by resource (project UUID) → the org event.
        let by_resource = http_get_audit(&state, "?resource_type=project&resource_id=proj-1").await;
        let arr = by_resource.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["action"], "project_create");
        assert_eq!(arr[0]["event_type"], "org");

        // Filter by time window that excludes the org event.
        let window = http_get_audit(&state, &format!("?from={}&to={}", now + 5, now + 15)).await;
        let arr = window.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["action"], "read");
    }

    /// Live-server E2E: record an org event + a secret-access event through the
    /// published recording API (what A3 calls), then list audit events over the
    /// full HTTP router and assert both are recorded and queryable.
    #[tokio::test]
    async fn e2e_org_and_secret_access_events_queryable() {
        // Build the FULL server router (real session/auth middleware path).
        let state = test_state().await;
        let app = crate::handlers::build_router(state.clone());

        let now = crate::handlers::now_ms();
        let secret = uuid::Uuid::new_v4().to_string();
        let project = uuid::Uuid::new_v4().to_string();

        // "Perform" an org action + a secret access via the published API.
        state
            .repo
            .audit_org_event(Some("u1"), Some("u1"), "project_create", "project", Some(&project), None, Some("127.0.0.1"), now)
            .await
            .unwrap();
        state
            .repo
            .audit_secret_access("u1", &secret, Some(&project), "read", Some("127.0.0.1"), now)
            .await
            .unwrap();

        // List all audit events (admin view) over the live router.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/audit")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = v.as_array().unwrap();
        // rotate_key (test_state) + org event + secret-access event.
        assert!(arr.len() >= 3);
        let actions: Vec<&str> = arr.iter().map(|e| e["action"].as_str().unwrap()).collect();
        assert!(actions.contains(&"project_create"), "org event recorded: {actions:?}");
        assert!(actions.contains(&"read"), "secret-access event recorded: {actions:?}");

        // Secret-access event carries who/what/when.
        let secret_ev = arr.iter().find(|e| e["action"] == "read").unwrap();
        assert_eq!(secret_ev["event_type"], "secret_access");
        assert_eq!(secret_ev["actor"], "u1");
        assert_eq!(secret_ev["resource_type"], "secret");
        assert_eq!(secret_ev["resource_id"], serde_json::json!(secret));
        // Metadata-only: never a payload / value.
        assert!(secret_ev.get("payload").is_none());
    }
}
