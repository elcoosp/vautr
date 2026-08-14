//! Secrets HTTP handlers (Wave A3). Docs: `mlp-wave-plan.md` §3 A3,
//! `mlp-scope.md` §4, `packages/api-contract/openapi.json` (secrets).
//!
//! Secrets are project-scoped, metadata-first, ciphertext-only: the server
//! stores the client-side AEAD ciphertext and never sees the plaintext value
//! (zero-knowledge). Endpoints:
//!
//! - `GET  /projects/{uuid}/secrets`   list secret metadata in a project.
//! - `POST /secrets`                   create a secret (requires CanEdit).
//! - `GET  /secrets/{uuid}`            fetch secret metadata.
//! - `PATCH /secrets/{uuid}`           update key / value (requires CanEdit).
//! - `DELETE /secrets/{uuid}`          delete a secret (requires CanEdit).
//! - `GET  /secrets/{uuid}/value`      return the secret value, gated by the
//!   `secrets:reveal` scope. The value is the stored ciphertext (client-side
//!   AEAD) — the server cannot decrypt it by design.
//!
//! Access control: project membership is enforced via the Wave 0.1 project
//! schema (`project_access` + group membership + ownership). The `secrets:reveal`
//! scope is enforced via `secret_reveal_grants` (0009_secrets.sql); Wave A2 owns
//! the machine-account / token scope model and the integrator wires it over this
//! gate. Every value reveal is recorded to the audit log (who/what/when) by
//! calling the existing `Repository::audit_log` API.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repository::secrets::{ProjectAccessLevel, SecretRow};

use super::{auth_user, b64, decode_b64, now_ms, ApiError, AppState, Bearer};

/// Create request: `{ project_uuid, key, value_ciphertext }`.
#[derive(Deserialize)]
pub(crate) struct SecretCreateRequest {
    project_uuid: String,
    key: String,
    value_ciphertext: String,
}

/// Update request: all fields optional (partial update).
#[derive(Deserialize, Default)]
pub(crate) struct SecretUpdateRequest {
    key: Option<String>,
    value_ciphertext: Option<String>,
}

/// Secret metadata (mirrors `Secret` in the API contract).
#[derive(Serialize)]
pub(crate) struct Secret {
    uuid: String,
    project_uuid: String,
    key: String,
    version: i64,
    created_by: String,
    last_accessed_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

/// List response: `{ "secrets": [...] }`.
#[derive(Serialize)]
pub(crate) struct SecretListResponse {
    secrets: Vec<Secret>,
}

/// Reveal response: `{ uuid, key, value_ciphertext }`.
#[derive(Serialize)]
pub(crate) struct SecretValue {
    uuid: String,
    key: String,
    value_ciphertext: String,
}

/// Delete response: `{ "status": "success" }`.
#[derive(Serialize)]
pub(crate) struct StatusResponse {
    status: &'static str,
}

fn to_secret(r: &SecretRow) -> Secret {
    Secret {
        uuid: r.uuid.clone(),
        project_uuid: r.project_id.clone(),
        key: r.key.clone(),
        version: r.version,
        created_by: r.created_by.clone(),
        last_accessed_at: r.last_accessed_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}

fn forbidden(msg: &str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, "forbidden", msg)
}

fn not_found(msg: &str) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", msg)
}

fn internal(e: sqlx::Error) -> ApiError {
    ApiError::internal(&e.to_string())
}

/// Validate a secret key per the contract (1..=256 chars).
fn validate_key(key: &str) -> Result<(), ApiError> {
    if key.is_empty() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "precondition_failed",
            "secret key must not be empty",
        ));
    }
    if key.chars().count() > 256 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "precondition_failed",
            "secret key exceeds 256 characters",
        ));
    }
    Ok(())
}

/// Resolve the caller's permission on a project, returning 404 if the project
/// does not exist and 403 if the caller has no access.
async fn require_project_access(
    st: &AppState,
    project_id: &str,
    user_id: &str,
    need_edit: bool,
) -> Result<ProjectAccessLevel, ApiError> {
    if !st
        .repo
        .project_exists(project_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        return Err(not_found("project not found"));
    }
    let lvl = st
        .repo
        .user_project_permission(project_id, user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| forbidden("you do not have access to this project"))?;
    if need_edit && !lvl.can_edit() {
        return Err(forbidden("edit permission required on this project"));
    }
    Ok(lvl)
}

/// Fetch a secret and enforce the caller's view-level project access.
/// Returns the row plus the caller's permission level.
async fn load_secret_for_user(
    st: &AppState,
    uuid: &str,
    user_id: &str,
    need_edit: bool,
) -> Result<SecretRow, ApiError> {
    let row = st
        .repo
        .get_secret(uuid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| not_found("secret not found"))?;
    require_project_access(st, &row.project_id, user_id, need_edit).await?;
    Ok(row)
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{uuid}/secrets", get(list_secrets))
        .route("/secrets", post(create_secret))
        .route(
            "/secrets/{uuid}",
            get(get_secret).patch(update_secret).delete(delete_secret),
        )
        .route("/secrets/{uuid}/value", get(get_secret_value))
}

/// `GET /projects/{uuid}/secrets` — list secret metadata within a project.
async fn list_secrets(
    State(st): State<AppState>,
    Path(project_id): Path<String>,
    auth: Bearer,
) -> Result<Json<SecretListResponse>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    require_project_access(&st, &project_id, &user_id, false).await?;
    let rows = st.repo.list_secrets(&project_id).await.map_err(internal)?;
    Ok(Json(SecretListResponse {
        secrets: rows.iter().map(to_secret).collect(),
    }))
}

/// `POST /secrets` — create a secret in a project (requires CanEdit).
async fn create_secret(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<SecretCreateRequest>,
) -> Result<(StatusCode, Json<Secret>), ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    require_project_access(&st, &req.project_uuid, &user_id, true).await?;
    validate_key(&req.key)?;
    let value = decode_b64(&req.value_ciphertext)?;

    let uuid = Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .create_secret(&uuid, &req.project_uuid, &req.key, &value, &user_id, now)
        .await
        .map_err(internal)?;

    // Record the write (who/what/when) in the audit log.
    st.repo
        .audit_log(
            Some(&user_id),
            "secret_create",
            Some(&user_id),
            Some(&format!("project:{}", req.project_uuid)),
            now,
        )
        .await
        .map_err(internal)?;

    let row = st
        .repo
        .get_secret(&uuid)
        .await
        .map_err(internal)?
        .ok_or_else(|| not_found("secret not found"))?;
    Ok((StatusCode::CREATED, Json(to_secret(&row))))
}

/// `GET /secrets/{uuid}` — fetch secret metadata.
async fn get_secret(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
) -> Result<Json<Secret>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = load_secret_for_user(&st, &uuid, &user_id, false).await?;
    Ok(Json(to_secret(&row)))
}

/// `PATCH /secrets/{uuid}` — update key and/or value (requires CanEdit).
async fn update_secret(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
    Json(req): Json<SecretUpdateRequest>,
) -> Result<Json<Secret>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = load_secret_for_user(&st, &uuid, &user_id, true).await?;

    let new_key = match &req.key {
        Some(k) => {
            validate_key(k)?;
            k.clone()
        }
        None => row.key.clone(),
    };
    // If value is absent, keep the existing ciphertext.
    let new_value: Vec<u8> = match &req.value_ciphertext {
        Some(v) => decode_b64(v)?,
        None => row.value_ciphertext.clone(),
    };

    let now = now_ms();
    st.repo
        .update_secret(&uuid, &new_key, &new_value, now)
        .await
        .map_err(internal)?;
    st.repo
        .audit_log(
            Some(&user_id),
            "secret_update",
            Some(&user_id),
            Some(&format!("project:{}", row.project_id)),
            now,
        )
        .await
        .map_err(internal)?;

    let updated = st
        .repo
        .get_secret(&uuid)
        .await
        .map_err(internal)?
        .ok_or_else(|| not_found("secret not found"))?;
    Ok(Json(to_secret(&updated)))
}

/// `DELETE /secrets/{uuid}` — delete a secret (requires CanEdit).
async fn delete_secret(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
) -> Result<Json<StatusResponse>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = load_secret_for_user(&st, &uuid, &user_id, true).await?;
    st.repo.delete_secret(&uuid).await.map_err(internal)?;
    st.repo
        .audit_log(
            Some(&user_id),
            "secret_delete",
            Some(&user_id),
            Some(&format!("project:{}", row.project_id)),
            now_ms(),
        )
        .await
        .map_err(internal)?;
    Ok(Json(StatusResponse { status: "success" }))
}

/// `GET /secrets/{uuid}/value` — return the secret value, gated by the
/// `secrets:reveal` scope. Requires view-level project access AND a reveal grant
/// (or project ownership). The returned value is the client-side AEAD
/// ciphertext; the zero-knowledge server never sees the plaintext.
async fn get_secret_value(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
) -> Result<Json<SecretValue>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = load_secret_for_user(&st, &uuid, &user_id, false).await?;

    // secrets:reveal gate.
    if !st
        .repo
        .can_reveal(&row.project_id, &user_id)
        .await
        .map_err(internal)?
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "secrets_reveal_required",
            "the `secrets:reveal` scope is required to read secret values",
        ));
    }

    let now = now_ms();
    st.repo.touch_secret(&uuid, now).await.map_err(internal)?;

    // Record secret access (who/what/when) in the audit log.
    st.repo
        .audit_log(
            Some(&user_id),
            "secret_reveal",
            Some(&user_id),
            Some(&format!("project:{}", row.project_id)),
            now,
        )
        .await
        .map_err(internal)?;

    Ok(Json(SecretValue {
        uuid: row.uuid.clone(),
        key: row.key.clone(),
        value_ciphertext: b64(&row.value_ciphertext),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        let pool = crate::db::connect("sqlite::memory:")
            .await
            .expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000i64;
        repo.create_user(
            "u1",
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
            "u2",
            "member@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();
        for (tok, u) in [("tok1", "u1"), ("tok2", "u2")] {
            sqlx::query(
                "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(tok)
            .bind(u)
            .bind(4_000_000_000_000i64)
            .bind(now)
            .execute(repo.pool())
            .await
            .unwrap();
        }
        // Project "p1" owned by u1.
        sqlx::query(
            "INSERT INTO projects (id, name, kind, org_id, team_id, owner_user_id, created_at, updated_at) \
             VALUES ('p1', 'Secrets', 'personal', NULL, NULL, 'u1', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        // Grant u2 CanView on p1 (can read metadata, cannot edit, no reveal).
        sqlx::query(
            "INSERT INTO project_access (id, project_id, grantee_user_id, grantee_group_id, permission, hide_password, granted_by, granted_at) \
             VALUES ('pa1', 'p1', 'u2', NULL, 'can_view', 0, 'u1', ?)",
        )
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        AppState::new(repo)
    }

    fn b64(s: &str) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    }

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn create_secret(app: &axum::Router, token: &str) -> (StatusCode, serde_json::Value) {
        let req = serde_json::json!({
            "project_uuid": "p1",
            "key": "db_pass",
            "value_ciphertext": b64("super-secret")
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/secrets")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(req.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let st = resp.status();
        let body = body_json(resp).await;
        (st, body)
    }

    #[tokio::test]
    async fn full_secret_lifecycle_e2e() {
        let state = test_state().await;
        let app = routes().with_state(state.clone());

        // Owner (u1) creates a secret.
        let (st, body) = create_secret(&app, "tok1").await;
        assert_eq!(st, StatusCode::CREATED, "body: {body}");
        let uuid = body["uuid"].as_str().unwrap().to_string();
        assert_eq!(body["project_uuid"], "p1");
        assert_eq!(body["key"], "db_pass");

        // Owner lists project secrets.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/projects/p1/secrets")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list = body_json(resp).await;
        assert_eq!(list["secrets"][0]["key"], "db_pass");
        assert!(list["secrets"][0].get("value_ciphertext").is_none()); // metadata only

        // u2 (CanView) can list metadata.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/projects/p1/secrets")
                    .header("authorization", "Bearer tok2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Owner can read the value (owner reveal).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/secrets/{uuid}/value"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let val = body_json(resp).await;
        assert_eq!(val["value_ciphertext"], b64("super-secret"));

        // u2 has CanView but no secrets:reveal scope -> denied.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/secrets/{uuid}/value"))
                    .header("authorization", "Bearer tok2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "deny without secrets:reveal"
        );

        // Grant u2 the reveal scope; now allowed.
        state
            .repo
            .grant_secret_reveal("p1", "u2", "u1", now_ms())
            .await
            .unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/secrets/{uuid}/value"))
                    .header("authorization", "Bearer tok2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // u2 (CanView only) cannot create -> denied.
        let req = serde_json::json!({
            "project_uuid": "p1",
            "key": "other",
            "value_ciphertext": b64("x")
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/secrets")
                    .header("authorization", "Bearer tok2")
                    .header("content-type", "application/json")
                    .body(Body::from(req.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Owner updates the secret.
        let req = serde_json::json!({
            "key": "db_pass_2",
            "value_ciphertext": b64("new-secret")
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/secrets/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(req.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let upd = body_json(resp).await;
        assert_eq!(upd["key"], "db_pass_2");

        // Revealed value reflects the update.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/secrets/{uuid}/value"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let val = body_json(resp).await;
        assert_eq!(val["value_ciphertext"], b64("new-secret"));

        // Delete.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/secrets/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let del = body_json(resp).await;
        assert_eq!(del["status"], "success");

        // Gone.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/secrets/{uuid}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn secrets_require_auth() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/projects/p1/secrets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn secrets_in_unknown_project_404() {
        let state = test_state().await;
        let app = routes().with_state(state);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/projects/nope/secrets")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
