//! HTTP handlers for the Vautr server. api.md §3–5.
//! /auth, /sync (pull, pull-payloads, push-batch), /items, /account.
//!
//! The server is a zero-knowledge encrypted blob store: it never sees the
//! Master Password, Master Key, or SVK in plaintext. It stores OPAQUE records
//! and AEAD ciphertext blobs, enforcing OCC (ADR-004) and epoch gating
//! (REQ-API-01).
//!
//! This module keeps the shared surface (`AppState`, `build_router`,
//! `ApiError`) plus the request/response types and helper functions that the
//! per-domain handler modules reuse. Each domain is split into its own module:
//! `auth`, `sync`, `items`, `account`.

use std::sync::Arc;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use vautr_crypto::opaque;

use crate::repository::Repository;

pub mod account;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod events;
pub mod files;
pub mod health;
pub mod items;
pub mod machine_accounts;
pub mod mfa;
pub mod projects;
pub mod recovery;
pub mod secrets;
pub mod sharing;
pub mod sync;
pub mod tokens;
/// WebAuthn (FIDO2) optional second factor (VTR-052). Feature-gated, off by default.
#[cfg(feature = "webauthn")]
pub mod webauthn;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repository>,
    /// VTRFIX-SEC-H07: per-user SSE bus. Replaces the old global broadcast
    /// channel that leaked every tenant's item UUIDs to `/events`.
    pub events: crate::handlers::events::UserEventBus,
    /// WebAuthn (FIDO2) second-factor service (VTR-052). Present only when the
    /// `webauthn` feature is compiled in.
    #[cfg(feature = "webauthn")]
    pub webauthn: Arc<webauthn::WebauthnService>,
}

impl AppState {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self {
            repo,
            events: crate::handlers::events::UserEventBus::new(),
            #[cfg(feature = "webauthn")]
            webauthn: Arc::new(webauthn::WebauthnService::new()),
        }
    }
}

/// Build the full router with state + middleware hooks.
pub fn build_router(state: AppState) -> Router {
    #[allow(unused_mut)]
    let mut router = Router::new()
        // Auth (OPAQUE over HTTP, api.md §3)
        .route("/auth/register/start", post(auth::register_start))
        .route("/auth/register/finish", post(auth::register_finish))
        .route("/auth/login/start", post(auth::login_start))
        .route("/auth/login/finish", post(auth::login_finish))
        // Session revocation (VTRFIX-SEC-H03).
        .route("/auth/logout", post(auth::logout))
        // Sync (api.md §4)
        .route("/sync/pull", get(sync::sync_pull))
        .route("/sync/pull-payloads", post(sync::sync_pull_payloads))
        .route("/sync/push-batch", post(sync::sync_push_batch))
        // Items (api.md §4)
        .route("/items/{uuid}", put(items::item_put))
        .route("/items/{uuid}", delete(items::item_delete))
        // Proactive vault events (VTR-069): SSE stream of tombstone/recovery events.
        .route("/events", get(events::events_stream))
        // Account & key management (api.md §5)
        .route("/account/status", get(account::account_status))
        .route("/account/rotate-key", post(account::account_rotate_key))
        // VTRFIX-SEC-M02: replace only the MP-wrapped SVK blob (no epoch bump).
        .route("/account/rekey-svk", post(account::account_rekey_svk))
        // Feature routers (Wave B): each is implemented in its own module.
        .merge(sharing::routes())
        .merge(files::routes())
        .merge(recovery::routes())
        .merge(audit::routes())
        // Wave 0.2 pre-registered stub routers (Wave A fills these in).
        .merge(projects::routes())
        .merge(machine_accounts::routes())
        .merge(tokens::routes())
        .merge(secrets::routes())
        .merge(mfa::routes())
        .merge(backup::routes())
        // Liveness/readiness probe + metrics (Wave A6 owns the full health
        // surface: /health, /health/ready, /metrics).
        .route("/health", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .route("/metrics", get(health::metrics))
        // OpenAPI 3 contract (VTR-010 TDD #2): served from the embedded spec.
        .route("/openapi.json", get(openapi_json));
    // WebAuthn (FIDO2) optional second factor (VTR-052), feature-gated.
    #[cfg(feature = "webauthn")]
    {
        router = router.merge(webauthn::routes());
    }
    router.with_state(state)
}

/// Serve the embedded OpenAPI 3 contract at `GET /openapi.json` (VTR-010 TDD #2).
/// The bytes come from [`crate::OPENAPI_SPEC`], embedded at compile time from
/// `packages/api-contract/openapi.json`.
async fn openapi_json() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        crate::OPENAPI_SPEC,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// API error type (axum 0.8: local type implements IntoResponse)
// ---------------------------------------------------------------------------

/// JSON error envelope `{ "error": <enum>, "message": <str> }` (api.md §6).
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &str, message: &str) -> Self {
        let body = serde_json::json!({ "error": code, "message": message });
        Self { status, body }
    }
    pub(crate) fn bad_request(code: &str, msg: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, msg)
    }
    pub(crate) fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or expired session",
        )
    }
    pub(crate) fn internal(msg: &str) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_server_error",
            msg,
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Helpers shared by all handler modules
// ---------------------------------------------------------------------------

const SETUP_KEY: &str = "opaque_server_setup";

pub(crate) fn decode_b64(s: &str) -> Result<Vec<u8>, ApiError> {
    B64.decode(s)
        .map_err(|_| ApiError::bad_request("invalid_base64", "base64 decode failed"))
}

pub(crate) fn b64(s: &[u8]) -> String {
    B64.encode(s)
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Load the OPAQUE server setup.
///
/// # Provisioning priority
/// 1. `VAUTR_OPAQUE_SETUP_FILE` env var → read raw bytes from that path.
/// 2. Legacy `server_config` DB row (`SETUP_KEY`) → read bytes.
/// 3. Neither present:
///    - if `VAUTR_ALLOW_DB_OPRF=1` (dev / single-process only), generate a
///      fresh setup and persist it (best-effort; not race-free).
///    - else, fail closed with an operator-facing error.
///
/// The returned bytes contain the OPRF private key. Callers must not log,
/// serialize, or send them anywhere except into `opaque::server_setup_from_bytes`.
pub(crate) async fn server_setup(repo: &Repository) -> Result<Vec<u8>, ApiError> {
    if let Ok(path) = std::env::var("VAUTR_OPAQUE_SETUP_FILE") {
        let bytes = std::fs::read(&path).map_err(|e| {
            ApiError::internal(&format!("VAUTR_OPAQUE_SETUP_FILE read failed: {e}"))
        })?;
        let _ = opaque::server_setup_from_bytes(&bytes)
            .map_err(|_| ApiError::internal("VAUTR_OPAQUE_SETUP_FILE: corrupt OPAQUE setup"))?;
        return Ok(bytes);
    }

    if let Some(bytes) = repo
        .get_config(SETUP_KEY)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        let _ = opaque::server_setup_from_bytes(&bytes)
            .map_err(|_| ApiError::internal("stored OPAQUE setup is corrupt"))?;
        return Ok(bytes);
    }

    // Production MUST provision out-of-band. Dev/test environments (the default)
    // may fall back to DB storage so a fresh `cargo test` or local boot works.
    let prod = std::env::var("VAUTR_ENV")
        .map(|v| matches!(v.as_str(), "prod" | "production"))
        .unwrap_or(false);
    let allow_db = std::env::var("VAUTR_ALLOW_DB_OPRF")
        .ok()
        .as_deref()
        == Some("1");
    if prod && !allow_db {
        return Err(ApiError::internal(
            "no OPAQUE server setup available: set VAUTR_OPAQUE_SETUP_FILE to a provisioned \
             file (recommended for production), or set VAUTR_ALLOW_DB_OPRF=1 to explicitly \
             opt in to DB-stored generation (NOT recommended — the DB row contains the OPRF \
             private key)",
        ));
    }

    let fresh = opaque::generate_server_setup().map_err(|e| ApiError::internal(&e.to_string()))?;
    repo.set_config(SETUP_KEY, fresh.as_slice())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // Re-read: another process may have won the race.
    let stored = repo
        .get_config(SETUP_KEY)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| ApiError::internal("OPAQUE setup disappeared after write"))?;
    Ok(stored)
}

/// Extract + validate a bearer session, returning the user id.
/// VTRFIX-SEC-H12: authenticate a bearer token that may be either a session
/// token or a machine-account access token (`vtr-ak-...`). Returns the
/// effective owner user id. Session-only routes (account, MFA, policy,
/// recovery, admin) MUST keep calling `auth_user` directly.
pub(crate) async fn auth_any(repo: &Repository, token: &str) -> Result<String, ApiError> {
    if token.starts_with("vtr-ak-") {
        let verified = crate::handlers::tokens::verify_access_token(repo, token).await?;
        let owner = repo
            .get_access_token_owner(&verified.token_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
            .ok_or_else(ApiError::unauthorized)?;
        Ok(owner)
    } else {
        auth_user(repo, token).await
    }
}

pub(crate) async fn auth_user(repo: &Repository, token: &str) -> Result<String, ApiError> {
    let Some((user_id, expires_at)) = repo
        .get_session(token)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::unauthorized());
    };
    let now = now_ms();
    if expires_at < now {
        return Err(ApiError::unauthorized());
    }
    // VTRFIX-SEC-H10: enforce the reclaim suspension window — while suspended,
    // the account cannot authenticate.
    let suspended = repo
        .user_is_suspended(&user_id, now)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if suspended {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "account_suspended",
            "account is suspended pending reclaim",
        ));
    }
    Ok(user_id)
}

// ---------------------------------------------------------------------------
// Bearer extractor (axum 0.8: FromRequestParts is an async trait method)
// ---------------------------------------------------------------------------

pub(crate) struct Bearer(pub String);

impl<S> axum::extract::FromRequestParts<S> for Bearer
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Some(auth) = parts.headers.get(axum::http::header::AUTHORIZATION) else {
            return Err(ApiError::unauthorized());
        };
        let Ok(val) = auth.to_str() else {
            return Err(ApiError::unauthorized());
        };
        let Some(token) = val.strip_prefix("Bearer ") else {
            return Err(ApiError::unauthorized());
        };
        Ok(Bearer(token.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_is_valid_json() {
        // VTR-010 TDD #2: the embedded contract must be well-formed JSON so the
        // server's `GET /openapi.json` handler returns a parseable document.
        let parsed: serde_json::Value = serde_json::from_str(crate::OPENAPI_SPEC)
            .expect("embedded openapi.json must be valid JSON");
        assert!(
            parsed.get("openapi").is_some(),
            "spec must declare an openapi version"
        );
        assert!(parsed.get("paths").is_some(), "spec must define paths");
    }
}
