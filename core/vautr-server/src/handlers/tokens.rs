//! Access Token HTTP handlers (Wave A: group A2).
//! Docs: `mlp-wave-plan.md` §3 A2, `mlp-scope.md` §4.
//!
//! Issues / lists / reads / revokes access tokens with **expiry** (chrono epoch ms)
//! and **fine-grained scopes**. Only a SHA-256 hash of a token secret is persisted;
//! the raw secret is returned exactly once (`AccessTokenCreateResponse.token`).
//!
//! Also publishes `verify_access_token`, the token-verify function other handlers
//! (secrets, projects) call to authenticate a machine-account request.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{auth_user, now_ms, ApiError, AppState, Bearer};
use crate::repository::machine_accounts::AccessTokenRow;
use crate::repository::Repository;

/// The complete set of fine-grained access scopes (OpenAPI `AccessScope`).
pub(crate) const VALID_SCOPES: [&str; 7] = [
    "secrets:read",
    "secrets:write",
    "secrets:reveal",
    "projects:read",
    "projects:write",
    "tokens:manage",
    "machine_accounts:manage",
];

/// True when every requested scope is a known `AccessScope`.
pub(crate) fn validate_scopes(scopes: &[String]) -> bool {
    scopes.iter().all(|s| VALID_SCOPES.contains(&s.as_str()))
}

/// Serialize scopes to the JSON string stored in the DB.
pub(crate) fn scopes_to_json(scopes: &[String]) -> String {
    serde_json::to_string(scopes).unwrap_or_else(|_| "[]".to_string())
}

/// Parse the stored JSON scopes back into a `Vec<String>`.
pub(crate) fn scopes_from_json(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

/// SHA-256 of the raw token secret, base64url-encoded. This is what is stored.
fn token_hash(secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    B64URL.encode(h.finalize())
}

/// Generate a fresh 256-bit token secret. Returns (secret, hash, display-prefix).
fn generate_token() -> (String, String, String) {
    let bytes: [u8; 32] = rand::rngs::OsRng.gen();
    let secret = B64URL.encode(bytes);
    let hash = token_hash(&secret);
    let prefix: String = secret.chars().take(8).collect();
    (secret, hash, prefix)
}

/// Result of a successful machine-account token verification.
///
/// This is the token-verify API published for other handlers (secrets/projects,
/// Wave A3/A1) and machine-account clients to authenticate requests.
#[derive(Debug, Clone)]
pub struct VerifiedToken {
    pub token_id: String,
    pub machine_account_uuid: Option<String>,
    pub project_uuid: Option<String>,
    pub scopes: Vec<String>,
    pub expires_at: Option<i64>,
}

/// Authenticate a machine-account request by hashing the presented bearer token,
/// looking it up, and enforcing revocation + expiry (+ machine-account state).
/// Other handlers call this instead of `auth_user` for machine-account traffic.
pub async fn verify_access_token(
    repo: &Repository,
    token: &str,
) -> Result<VerifiedToken, ApiError> {
    let now = now_ms();
    let hash = token_hash(token);
    let row = repo
        .get_token_by_hash(&hash)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(ApiError::unauthorized)?;

    // Revocation.
    if row.revoked_at.is_some() {
        return Err(ApiError::unauthorized());
    }
    // Token expiry.
    if let Some(exp) = row.expires_at {
        if exp <= now {
            return Err(ApiError::unauthorized());
        }
    }
    // If bound to a machine account, enforce its state + expiry too.
    if let Some(ma_uuid) = &row.machine_account_uuid {
        let ma = repo
            .get_machine_account(ma_uuid, &row.owner_user_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
            .ok_or_else(ApiError::unauthorized)?;
        if ma.status != "active" {
            return Err(ApiError::unauthorized());
        }
        if let Some(exp) = ma.expires_at {
            if exp <= now {
                return Err(ApiError::unauthorized());
            }
        }
    }

    // Successful authentication: bump last-used markers.
    let _ = repo.touch_access_token(&row.uuid, now).await;
    if let Some(ma) = &row.machine_account_uuid {
        let _ = repo.touch_machine_account(ma, now).await;
    }

    Ok(VerifiedToken {
        token_id: row.uuid,
        machine_account_uuid: row.machine_account_uuid,
        project_uuid: row.project_uuid,
        scopes: scopes_from_json(&row.scopes),
        expires_at: row.expires_at,
    })
}

/// Response shape (OpenAPI `AccessToken`).
#[derive(Serialize)]
pub(crate) struct AccessToken {
    pub uuid: String,
    pub name: String,
    pub machine_account_uuid: Option<String>,
    pub project_uuid: Option<String>,
    pub scopes: Vec<String>,
    pub prefix: String,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

/// Body for `POST /tokens` (`AccessTokenCreateRequest`).
#[derive(Deserialize)]
pub(crate) struct CreateRequest {
    pub name: String,
    #[serde(default)]
    pub machine_account_uuid: Option<String>,
    #[serde(default)]
    pub project_uuid: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Body returned by `POST /tokens` (`AccessTokenCreateResponse`). Full secret, once.
#[derive(Serialize)]
pub(crate) struct CreateResponse {
    pub token: String,
    pub token_id: String,
    pub expires_at: Option<i64>,
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tokens", get(list_tokens))
        .route("/tokens", post(create_token))
        .route("/tokens/{uuid}", get(get_token))
        .route("/tokens/{uuid}", delete(revoke_token))
}

fn to_response(row: AccessTokenRow) -> AccessToken {
    AccessToken {
        uuid: row.uuid,
        name: row.name,
        machine_account_uuid: row.machine_account_uuid,
        project_uuid: row.project_uuid,
        scopes: scopes_from_json(&row.scopes),
        prefix: row.prefix,
        expires_at: row.expires_at,
        revoked_at: row.revoked_at,
        last_used_at: row.last_used_at,
        created_at: row.created_at,
    }
}

fn not_found(uuid: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        &format!("access token {uuid} not found"),
    )
}

fn validate_create(req: &CreateRequest) -> Result<(), ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "name must not be empty",
        ));
    }
    if req.name.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_name",
            "name must be <= 128 chars",
        ));
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
    Ok(())
}

/// GET /tokens — list access tokens issued to the caller.
async fn list_tokens(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let rows = st
        .repo
        .list_access_tokens(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let tokens: Vec<AccessToken> = rows.into_iter().map(to_response).collect();
    Ok(Json(serde_json::json!({ "tokens": tokens })))
}

/// POST /tokens — issue a new access token; returns the full secret once.
async fn create_token(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<CreateRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    validate_create(&req)?;

    // If bound to a machine account, require it exists, is active, not expired,
    // and that the requested scopes are a subset of the account's scopes.
    if let Some(ma_uuid) = &req.machine_account_uuid {
        let ma = st
            .repo
            .get_machine_account(ma_uuid, &user_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "machine account not found",
                )
            })?;
        if ma.status != "active" {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "machine_account_not_active",
                "machine account is not active",
            ));
        }
        if let Some(exp) = ma.expires_at {
            if exp <= now_ms() {
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "machine_account_expired",
                    "machine account has expired",
                ));
            }
        }
        let allowed = scopes_from_json(&ma.scopes);
        if !req.scopes.iter().all(|s| allowed.contains(s)) {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "scope_not_allowed",
                "requested scopes exceed the machine account's allowed scopes",
            ));
        }
    }

    let uuid = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    let (secret, hash, prefix) = generate_token();
    st.repo
        .create_access_token(
            &uuid,
            req.name.trim(),
            &user_id,
            req.machine_account_uuid.as_deref(),
            req.project_uuid.as_deref(),
            &scopes_to_json(&req.scopes),
            &hash,
            &prefix,
            req.expires_at,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    st.repo
        .audit_org_event(
            Some(&user_id),
            Some(&user_id),
            "token_create",
            "access_token",
            Some(&uuid),
            Some(&format!("prefix:{prefix}")),
            None,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateResponse {
            token: secret,
            token_id: uuid,
            expires_at: req.expires_at,
        }),
    ))
}

/// GET /tokens/{uuid} — fetch one access token (metadata only, no secret).
async fn get_token(
    State(st): State<AppState>,
    auth: Bearer,
    Path(uuid): Path<String>,
) -> Result<Json<AccessToken>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let row = st
        .repo
        .get_access_token(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| not_found(&uuid))?;
    Ok(Json(to_response(row)))
}

/// DELETE /tokens/{uuid} — revoke an access token.
async fn revoke_token(
    State(st): State<AppState>,
    auth: Bearer,
    Path(uuid): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let ok = st
        .repo
        .revoke_access_token(&uuid, &user_id, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !ok {
        return Err(not_found(&uuid));
    }
    st.repo
        .audit_org_event(
            Some(&user_id),
            Some(&user_id),
            "token_revoke",
            "access_token",
            Some(&uuid),
            None,
            None,
            now_ms(),
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(serde_json::json!({ "status": "revoked" })))
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
            std::env::temp_dir().join(format!("vautr_tok_http_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(Repository::new(pool));
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
    async fn issue_returns_secret_once_and_verifies() {
        let st = test_state().await;
        let app = routes().with_state(st.clone());

        // Issue a token.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tokens")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"ci","scopes":["secrets:read","projects:read"],"expires_at":1900000000000}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let secret = v["token"].as_str().unwrap().to_string();
        let token_id = v["token_id"].as_str().unwrap().to_string();
        assert!(!secret.is_empty());
        assert!(!token_id.is_empty());

        // List returns metadata, never the secret.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/tokens")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = v["tokens"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["uuid"], token_id);
        assert!(arr[0].get("token").is_none());
        assert_eq!(arr[0]["prefix"].as_str().unwrap().len(), 8);

        // Verify the raw secret authenticates; a random string does not.
        let vt = verify_access_token(&st.repo, &secret).await.unwrap();
        assert!(vt.scopes.contains(&"secrets:read".to_string()));
        assert!(verify_access_token(&st.repo, "not-the-secret")
            .await
            .is_err());

        // Revoke, then the secret no longer authenticates.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/tokens/{token_id}"))
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(verify_access_token(&st.repo, &secret).await.is_err());
    }

    #[tokio::test]
    async fn expired_token_is_denied() {
        let st = test_state().await;
        let app = routes().with_state(st.clone());

        // expires_at in the past relative to real wall clock.
        let past = 1_000_000_000_000i64;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tokens")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name":"exp","scopes":["secrets:read"],"expires_at":{past}}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // A token with a future-expiry issued, then revoked_at set directly.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tokens")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"soon","scopes":["secrets:read"],"expires_at":1900000000000}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let secret = v["token"].as_str().unwrap().to_string();

        // Force expiry in the past and confirm denial.
        sqlx::query("UPDATE access_tokens SET expires_at = ?")
            .bind(past)
            .execute(st.repo.pool())
            .await
            .unwrap();
        assert!(verify_access_token(&st.repo, &secret).await.is_err());
    }

    #[tokio::test]
    async fn create_requires_auth() {
        let st = test_state().await;
        let app = routes().with_state(st.clone());
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/tokens")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
