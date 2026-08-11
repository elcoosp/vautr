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
pub mod auth;
pub mod items;
pub mod sync;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repository>,
}

impl AppState {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }
}

/// Build the full router with state + middleware hooks.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Auth (OPAQUE over HTTP, api.md §3)
        .route("/auth/register/start", post(auth::register_start))
        .route("/auth/register/finish", post(auth::register_finish))
        .route("/auth/login/start", post(auth::login_start))
        .route("/auth/login/finish", post(auth::login_finish))
        // Sync (api.md §4)
        .route("/sync/pull", get(sync::sync_pull))
        .route("/sync/pull-payloads", post(sync::sync_pull_payloads))
        .route("/sync/push-batch", post(sync::sync_push_batch))
        // Items (api.md §4)
        .route("/items/:uuid", put(items::item_put))
        .route("/items/:uuid", delete(items::item_delete))
        // Account & key management (api.md §5)
        .route("/account/status", get(account::account_status))
        .route("/account/rotate-key", post(account::account_rotate_key))
        .with_state(state)
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
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_server_error", msg)
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
    B64.decode(s).map_err(|_| ApiError::bad_request("invalid_base64", "base64 decode failed"))
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

/// Load the persisted OPAQUE server setup, generating + storing it on first use.
pub(crate) async fn server_setup(repo: &Repository) -> Result<Vec<u8>, ApiError> {
    if let Some(bytes) = repo
        .get_config(SETUP_KEY)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        return Ok(bytes);
    }
    let pk = opaque::server_setup_public_key().map_err(|e| ApiError::internal(&e.to_string()))?;
    repo.set_config(SETUP_KEY, &pk)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(pk)
}

/// Extract + validate a bearer session, returning the user id.
pub(crate) async fn auth_user(repo: &Repository, token: &str) -> Result<String, ApiError> {
    let Some((user_id, expires_at)) = repo
        .get_session(token)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::unauthorized());
    };
    if expires_at < now_ms() {
        return Err(ApiError::unauthorized());
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
