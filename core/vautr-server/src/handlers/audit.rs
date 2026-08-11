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

fn default_limit() -> u32 {
    100
}

#[derive(Deserialize)]
pub(crate) struct AuditQuery {
    /// Optional filter: only return entries for this account (admin scoping).
    pub(crate) user_id: Option<String>,
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
    let rows = st
        .repo
        .list_audit_logs(q.user_id.as_deref(), limit, offset)
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
}
