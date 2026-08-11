//! Machine Accounts HTTP handlers (Wave A: group A2).
//! Docs: `mlp-wave-plan.md` §3 A2, `mlp-scope.md` §4.
//!
//! Non-human identities for CI/CD, apps, and agents. A user (session-authenticated)
//! provisions machine accounts and manages their lifecycle (create / list / get /
//! update / status / delete). Access tokens for these accounts live in
//! `handlers/tokens.rs`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, Bearer, auth_user, now_ms};
use crate::repository::machine_accounts::MachineAccountRow;

// Scope helpers shared with the tokens handler.
use super::tokens::{scopes_from_json, scopes_to_json, validate_scopes};

/// Response shape (matches OpenAPI `MachineAccount`).
#[derive(Serialize)]
pub(crate) struct MachineAccount {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    pub project_uuid: Option<String>,
    pub status: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

/// Body for `POST /machine-accounts` (`MachineAccountCreateRequest`).
#[derive(Deserialize)]
pub(crate) struct CreateRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub project_uuid: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Body for `PATCH /machine-accounts/{uuid}` (`MachineAccountUpdateRequest`).
/// All fields optional; present fields overwrite.
#[derive(Deserialize, Default)]
pub(crate) struct UpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/machine-accounts", get(list_machine_accounts))
        .route("/machine-accounts", post(create_machine_account))
        .route(
            "/machine-accounts/{uuid}",
            get(get_machine_account).patch(update_machine_account).delete(delete_machine_account),
        )
}

fn to_response(row: MachineAccountRow) -> MachineAccount {
    MachineAccount {
        uuid: row.uuid,
        name: row.name,
        description: row.description,
        project_uuid: row.project_uuid,
        status: row.status,
        scopes: scopes_from_json(&row.scopes),
        expires_at: row.expires_at,
        last_used_at: row.last_used_at,
        created_at: row.created_at,
    }
}

fn not_found(uuid: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        &format!("machine account {uuid} not found"),
    )
}

/// GET /machine-accounts — list machine accounts owned by the caller.
async fn list_machine_accounts(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let rows = st
        .repo
        .list_machine_accounts(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let accounts: Vec<MachineAccount> = rows.into_iter().map(to_response).collect();
    Ok(Json(serde_json::json!({ "machine_accounts": accounts })))
}

/// POST /machine-accounts — provision a new machine account.
async fn create_machine_account(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<CreateRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("invalid_name", "name must not be empty"));
    }
    if name.len() > 128 {
        return Err(ApiError::bad_request("invalid_name", "name must be <= 128 chars"));
    }
    if req.scopes.is_empty() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_scopes",
            "at least one scope is required",
        ));
    }
    if !validate_scopes(&req.scopes) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_scopes",
            "request contains an unknown access scope",
        ));
    }
    if let Some(exp) = req.expires_at {
        if exp <= now_ms() {
            return Err(ApiError::bad_request(
                "invalid_expiry",
                "expires_at must be in the future",
            ));
        }
    }

    let uuid = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .create_machine_account(
            &uuid,
            name,
            req.description.as_deref(),
            &user_id,
            req.project_uuid.as_deref(),
            &scopes_to_json(&req.scopes),
            req.expires_at,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let row = st
        .repo
        .get_machine_account(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| ApiError::internal("machine account was not persisted"))?;
    Ok((StatusCode::CREATED, Json(to_response(row))))
}

/// GET /machine-accounts/{uuid} — fetch one machine account.
async fn get_machine_account(
    State(st): State<AppState>,
    auth: Bearer,
    Path(uuid): Path<String>,
) -> Result<Json<MachineAccount>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = st
        .repo
        .get_machine_account(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| not_found(&uuid))?;
    Ok(Json(to_response(row)))
}

/// PATCH /machine-accounts/{uuid} — update fields and/or status.
async fn update_machine_account(
    State(st): State<AppState>,
    auth: Bearer,
    Path(uuid): Path<String>,
    Json(req): Json<UpdateRequest>,
) -> Result<Json<MachineAccount>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let current = st
        .repo
        .get_machine_account(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| not_found(&uuid))?;

    let name = req.name.as_deref().unwrap_or(&current.name).to_string();
    if name.trim().is_empty() {
        return Err(ApiError::bad_request("invalid_name", "name must not be empty"));
    }
    let description = req
        .description
        .clone()
        .or_else(|| current.description.clone());
    let status = req.status.clone().unwrap_or_else(|| current.status.clone());
    if !matches!(status.as_str(), "active" | "disabled" | "revoked") {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_status",
            "status must be active, disabled, or revoked",
        ));
    }
    let scopes = match &req.scopes {
        Some(s) => {
            if s.is_empty() {
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_scopes",
                    "at least one scope is required",
                ));
            }
            if !validate_scopes(s) {
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_scopes",
                    "request contains an unknown access scope",
                ));
            }
            s.clone()
        }
        None => scopes_from_json(&current.scopes),
    };
    let expires_at = req.expires_at.or(current.expires_at);
    if let Some(exp) = expires_at {
        if exp <= now_ms() {
            return Err(ApiError::bad_request(
                "invalid_expiry",
                "expires_at must be in the future",
            ));
        }
    }

    let ok = st
        .repo
        .update_machine_account(
            &uuid,
            &user_id,
            &name,
            description.as_deref(),
            &status,
            &scopes_to_json(&scopes),
            expires_at,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !ok {
        return Err(not_found(&uuid));
    }
    let row = st
        .repo
        .get_machine_account(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| not_found(&uuid))?;
    Ok(Json(to_response(row)))
}

/// DELETE /machine-accounts/{uuid} — delete a machine account.
async fn delete_machine_account(
    State(st): State<AppState>,
    auth: Bearer,
    Path(uuid): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let ok = st
        .repo
        .delete_machine_account(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !ok {
        return Err(not_found(&uuid));
    }
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        let path = std::env::temp_dir()
            .join(format!("vautr_ma_http_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000;
        repo.create_user(
            "u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        AppState::new(repo)
    }

    #[tokio::test]
    async fn create_list_get_status_delete() {
        let state = test_state().await;
        let app = routes().with_state(state);

        // Create.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/machine-accounts")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"ci-runner","description":"CI","scopes":["secrets:read"],"expires_at":1900000000000}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["name"], "ci-runner");
        assert_eq!(v["status"], "active");
        assert_eq!(v["scopes"][0], "secrets:read");
        let uuid = v["uuid"].as_str().unwrap().to_string();

        // List.
        let resp = app
            .clone()
            .oneshot(
                Request::builder().uri("/machine-accounts").header("authorization", "Bearer tok1").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["machine_accounts"].as_array().unwrap().len(), 1);

        // Update status -> disabled.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/machine-accounts/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"disabled"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["status"], "disabled");

        // Get.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/machine-accounts/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Delete.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/machine-accounts/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Get after delete -> 404.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/machine-accounts/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_requires_auth() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(Request::builder().uri("/machine-accounts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_rejects_unknown_scope() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/machine-accounts")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"x","scopes":["admin:all"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
