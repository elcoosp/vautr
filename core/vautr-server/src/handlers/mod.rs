//! HTTP handlers for the Vautr server. api.md §3–5.
//! /auth, /sync (pull, pull-payloads, push-batch), /items, /account.
//!
//! The server is a zero-knowledge encrypted blob store: it never sees the
//! Master Password, Master Key, or SVK in plaintext. It stores OPAQUE records
//! and AEAD ciphertext blobs, enforcing OCC (ADR-004) and epoch gating
//! (REQ-API-01).

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use vautr_crypto::opaque;

use crate::repository::{Repository, UpsertOutcome};

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
        .route("/auth/register/start", post(register_start))
        .route("/auth/register/finish", post(register_finish))
        .route("/auth/login/start", post(login_start))
        .route("/auth/login/finish", post(login_finish))
        // Sync (api.md §4)
        .route("/sync/pull", get(sync_pull))
        .route("/sync/pull-payloads", post(sync_pull_payloads))
        .route("/sync/push-batch", post(sync_push_batch))
        // Items (api.md §4)
        .route("/items/:uuid", put(item_put))
        .route("/items/:uuid", delete(item_delete))
        // Account & key management (api.md §5)
        .route("/account/status", get(account_status))
        .route("/account/rotate-key", post(account_rotate_key))
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
    fn new(status: StatusCode, code: &str, message: &str) -> Self {
        let body = serde_json::json!({ "error": code, "message": message });
        Self { status, body }
    }
    fn bad_request(code: &str, msg: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, msg)
    }
    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or expired session",
        )
    }
    fn internal(msg: &str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_server_error", msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RegisterStartReq {
    username: String,
    registration_start: String, // base64
}
#[derive(Serialize)]
struct RegisterStartResp {
    registration_response: String,
}
#[derive(Deserialize)]
struct RegisterFinishReq {
    username: String,
    registration_finish: String, // base64
    server_public_key: String,   // base64 (opaque server setup, first-time)
    kdf_salt: String,            // base64 (stored, never used server-side)
    svk_ciphertext_blob: String, // base64 (MP-wrapped SVK)
    svk_ciphertext_blob_rk: String, // base64 (RK-wrapped SVK, REQ-RECOVERY-02)
}
#[derive(Serialize)]
struct StatusResp {
    status: String,
}
#[derive(Deserialize)]
struct LoginStartReq {
    username: String,
    login_start: String, // base64
}
#[derive(Serialize)]
struct LoginStartResp {
    login_response: String,
}
#[derive(Deserialize)]
struct LoginFinishReq {
    username: String,
    login_finish: String, // base64
}
#[derive(Serialize)]
struct LoginFinishResp {
    session_token: String,
    expires_at: i64,
}
#[derive(Deserialize)]
struct PullQuery {
    cursor: u64,
    limit: Option<u32>,
}
#[derive(Serialize)]
struct PullResp {
    new_cursor: u64,
    has_more: bool,
    min_enc_key_gen: i64,
    items: Vec<PullItem>,
}
#[derive(Serialize)]
struct PullItem {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    deleted_date: Option<i64>,
}
#[derive(Deserialize)]
struct PullPayloadsReq {
    items: Vec<PullTarget>,
}
#[derive(Deserialize)]
struct PullTarget {
    uuid: String,
    version: u64,
}
#[derive(Serialize)]
struct PullPayloadsResp {
    results: Vec<PayloadResult>,
}
#[derive(Serialize)]
struct PayloadResult {
    uuid: String,
    status: String, // "payload_delivered" | "version_mismatch"
    version: i64,
    enc_key_gen: i64,
    deleted_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>, // base64
}
#[derive(Deserialize)]
struct PushBatchReq {
    items: Vec<PushItem>,
}
#[derive(Deserialize)]
struct PushItem {
    uuid: String,
    target_version: u64,
    enc_key_gen: u64,
    #[serde(default)]
    payload: Option<String>, // base64 (None = tombstone)
    #[serde(default)]
    deleted_date: Option<i64>,
}
#[derive(Serialize)]
struct PushBatchResp {
    results: Vec<PushResult>,
}
#[derive(Serialize)]
struct PushResult {
    uuid: String,
    status: String, // "success" | "conflict" | "epoch_too_old"
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enc_key_gen: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_server_state: Option<ServerState>,
}
#[derive(Serialize)]
struct ServerState {
    version: i64,
    enc_key_gen: i64,
}
#[derive(Deserialize)]
struct ItemPutReq {
    enc_key_gen: u64,
    payload: String, // base64
}
#[derive(Serialize)]
struct ItemPutResp {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    updated_at: i64,
}
#[derive(Serialize)]
struct AccountStatusResp {
    min_enc_key_gen: i64,
    svk_ciphertext_blob: String, // base64
}
#[derive(Deserialize)]
struct RotateKeyReq {
    new_min_enc_key_gen: i64,
    new_svk_ciphertext_blob: String, // base64 (MP-wrapped)
}
#[derive(Serialize)]
struct RotateKeyResp {
    status: String,
    min_enc_key_gen: i64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SETUP_KEY: &str = "opaque_server_setup";

fn decode_b64(s: &str) -> Result<Vec<u8>, ApiError> {
    B64.decode(s).map_err(|_| ApiError::bad_request("invalid_base64", "base64 decode failed"))
}

fn b64(s: &[u8]) -> String {
    B64.encode(s)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Load the persisted OPAQUE server setup, generating + storing it on first use.
async fn server_setup(repo: &Repository) -> Result<Vec<u8>, ApiError> {
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
async fn auth_user(repo: &Repository, token: &str) -> Result<String, ApiError> {
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
// Auth handlers
// ---------------------------------------------------------------------------

async fn register_start(
    State(st): State<AppState>,
    Json(req): Json<RegisterStartReq>,
) -> Result<Json<RegisterStartResp>, ApiError> {
    let setup = server_setup(&st.repo).await?;
    let creq = decode_b64(&req.registration_start)?;
    let sresp = opaque::server_register_start(&setup, &creq, req.username.as_bytes())
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(RegisterStartResp {
        registration_response: b64(&sresp),
    }))
}

async fn register_finish(
    State(st): State<AppState>,
    Json(req): Json<RegisterFinishReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let _setup = server_setup(&st.repo).await?;
    let _pk = decode_b64(&req.server_public_key)?;
    let cupload = decode_b64(&req.registration_finish)?;
    let record = opaque::server_register_finish(&cupload)
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let kdf_salt = decode_b64(&req.kdf_salt)?;
    let svk = decode_b64(&req.svk_ciphertext_blob)?;
    let svk_rk = decode_b64(&req.svk_ciphertext_blob_rk)?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .create_user(&user_id, &req.username, &kdf_salt, &record, &svk, &svk_rk, now)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

async fn login_start(
    State(st): State<AppState>,
    Json(req): Json<LoginStartReq>,
) -> Result<Json<LoginStartResp>, ApiError> {
    let setup = server_setup(&st.repo).await?;
    let Some(user) = st
        .repo
        .get_user_by_email(&req.username)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    let lreq = decode_b64(&req.login_start)?;
    let (sresp, _sstate) = opaque::server_login_start(&setup, Some(&user.opaque_record), &lreq, req.username.as_bytes())
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(LoginStartResp {
        login_response: b64(&sresp),
    }))
}

async fn login_finish(
    State(st): State<AppState>,
    Json(req): Json<LoginFinishReq>,
) -> Result<Json<LoginFinishResp>, ApiError> {
    let setup = server_setup(&st.repo).await?;
    let Some(user) = st
        .repo
        .get_user_by_email(&req.username)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    let lupload = decode_b64(&req.login_finish)?;
    let sfin = opaque::server_login_finish(&setup, &lupload)
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let token = b64(&sfin);
    let expires_at = now_ms() + 86_400_000; // 24h
    st.repo
        .store_session(&token, &user.id, expires_at)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(LoginFinishResp {
        session_token: token,
        expires_at,
    }))
}

// ---------------------------------------------------------------------------
// Sync handlers
// ---------------------------------------------------------------------------

async fn sync_pull(
    State(st): State<AppState>,
    Query(q): Query<PullQuery>,
    auth: Bearer,
) -> Result<Json<PullResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let min_gen = st
        .repo
        .min_enc_key_gen(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .unwrap_or(1);

    let limit = q.limit.unwrap_or(100).max(1) as i64;
    let rows = sqlx::query_as::<_, crate::repository::ItemRow>(
        "SELECT * FROM items WHERE user_id = ? AND version > ? ORDER BY version ASC LIMIT ?",
    )
    .bind(&user_id)
    .bind(q.cursor as i64)
    .bind(limit + 1)
    .fetch_all(st.repo.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let has_more = rows.len() as i64 > limit;
    let items: Vec<PullItem> = rows
        .into_iter()
        .take(limit as usize)
        .map(|r| PullItem {
            uuid: r.uuid,
            version: r.version,
            enc_key_gen: r.enc_key_gen,
            deleted_date: r.deleted_date,
        })
        .collect();
    let new_cursor = items.last().map(|i| i.version as u64).unwrap_or(q.cursor);

    Ok(Json(PullResp {
        new_cursor,
        has_more,
        min_enc_key_gen: min_gen,
        items,
    }))
}

async fn sync_pull_payloads(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<PullPayloadsReq>,
) -> Result<Json<PullPayloadsResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    if req.items.len() > 100 {
        return Err(ApiError::bad_request(
            "payload_limit_exceeded",
            "max 100 items per pull-payloads",
        ));
    }
    let mut results = Vec::with_capacity(req.items.len());
    for target in &req.items {
        let row = st
            .repo
            .get_item(&target.uuid, &user_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
        match row {
            Some(r) if r.version as u64 == target.version => results.push(PayloadResult {
                uuid: r.uuid,
                status: "payload_delivered".into(),
                version: r.version,
                enc_key_gen: r.enc_key_gen,
                deleted_date: r.deleted_date,
                payload: r.payload.map(|p| b64(&p)),
            }),
            Some(r) => results.push(PayloadResult {
                uuid: r.uuid,
                status: "version_mismatch".into(),
                version: r.version,
                enc_key_gen: r.enc_key_gen,
                deleted_date: r.deleted_date,
                payload: None,
            }),
            None => results.push(PayloadResult {
                uuid: target.uuid.clone(),
                status: "version_mismatch".into(),
                version: 0,
                enc_key_gen: 0,
                deleted_date: None,
                payload: None,
            }),
        }
    }
    Ok(Json(PullPayloadsResp { results }))
}

async fn sync_push_batch(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<PushBatchReq>,
) -> Result<Json<PushBatchResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    if req.items.len() > 100 {
        return Err(ApiError::bad_request(
            "payload_limit_exceeded",
            "max 100 items per push-batch",
        ));
    }
    let now = now_ms();
    let mut results = Vec::with_capacity(req.items.len());
    for item in &req.items {
        let min_gen = st
            .repo
            .min_enc_key_gen(&user_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
            .unwrap_or(1);
        if item.enc_key_gen < min_gen as u64 {
            results.push(PushResult {
                uuid: item.uuid.clone(),
                status: "epoch_too_old".into(),
                version: None,
                enc_key_gen: None,
                updated_at: None,
                current_server_state: None,
            });
            continue;
        }
        let payload = match &item.payload {
            Some(p) => Some(decode_b64(p)?),
            None => None,
        };
        let outcome = st
            .repo
            .upsert_item_occ(
                &item.uuid,
                &user_id,
                item.target_version as i64,
                item.enc_key_gen as i64,
                payload.as_deref(),
                item.deleted_date,
                now,
            )
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
        match outcome {
            UpsertOutcome::Updated => {
                let row = st
                    .repo
                    .get_item(&item.uuid, &user_id)
                    .await
                    .map_err(|e| ApiError::internal(&e.to_string()))?
                    .unwrap();
                results.push(PushResult {
                    uuid: item.uuid.clone(),
                    status: "success".into(),
                    version: Some(row.version),
                    enc_key_gen: Some(row.enc_key_gen),
                    updated_at: Some(row.updated_at),
                    current_server_state: None,
                });
            }
            UpsertOutcome::Conflict => {
                let row = st
                    .repo
                    .get_item(&item.uuid, &user_id)
                    .await
                    .map_err(|e| ApiError::internal(&e.to_string()))?;
                results.push(PushResult {
                    uuid: item.uuid.clone(),
                    status: "conflict".into(),
                    version: None,
                    enc_key_gen: None,
                    updated_at: None,
                    current_server_state: row.map(|r| ServerState {
                        version: r.version,
                        enc_key_gen: r.enc_key_gen,
                    }),
                });
            }
            UpsertOutcome::EpochTooOld => results.push(PushResult {
                uuid: item.uuid.clone(),
                status: "epoch_too_old".into(),
                version: None,
                enc_key_gen: None,
                updated_at: None,
                current_server_state: None,
            }),
        }
    }
    Ok(Json(PushBatchResp { results }))
}

// ---------------------------------------------------------------------------
// Items handlers
// ---------------------------------------------------------------------------

async fn item_put(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
    Json(req): Json<ItemPutReq>,
) -> Result<Json<ItemPutResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let Some(row) = st
        .repo
        .get_item(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "item not found",
        ));
    };
    let target = (row.version + 1) as u64;
    let payload = decode_b64(&req.payload)?;
    let now = now_ms();
    match st
        .repo
        .upsert_item_occ(
            &uuid,
            &user_id,
            target as i64,
            req.enc_key_gen as i64,
            Some(&payload),
            None,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        UpsertOutcome::Updated => {
            let row = st
                .repo
                .get_item(&uuid, &user_id)
                .await
                .map_err(|e| ApiError::internal(&e.to_string()))?
                .unwrap();
            Ok(Json(ItemPutResp {
                uuid,
                version: row.version,
                enc_key_gen: row.enc_key_gen,
                updated_at: row.updated_at,
            }))
        }
        UpsertOutcome::Conflict => Err(ApiError::new(
            StatusCode::PRECONDITION_FAILED,
            "precondition_failed",
            "version conflict",
        )),
        UpsertOutcome::EpochTooOld => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "key_generation_too_old",
            "enc_key_gen behind server epoch",
        )),
    }
}

async fn item_delete(
    State(st): State<AppState>,
    Path(uuid): Path<String>,
    auth: Bearer,
) -> Result<Json<ItemPutResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let Some(row) = st
        .repo
        .get_item(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "item not found",
        ));
    };
    let target = (row.version + 1) as u64;
    let now = now_ms();
    match st
        .repo
        .upsert_item_occ(
            &uuid,
            &user_id,
            target as i64,
            row.enc_key_gen,
            None,
            Some(now),
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        UpsertOutcome::Updated => Ok(Json(ItemPutResp {
            uuid,
            version: row.version + 1,
            enc_key_gen: row.enc_key_gen,
            updated_at: now,
        })),
        UpsertOutcome::Conflict => Err(ApiError::new(
            StatusCode::PRECONDITION_FAILED,
            "precondition_failed",
            "version conflict",
        )),
        UpsertOutcome::EpochTooOld => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "key_generation_too_old",
            "enc_key_gen behind server epoch",
        )),
    }
}

// ---------------------------------------------------------------------------
// Account & key management handlers
// ---------------------------------------------------------------------------

async fn account_status(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<AccountStatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let Some(user) = st
        .repo
        .get_user_by_id(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    Ok(Json(AccountStatusResp {
        min_enc_key_gen: user.min_enc_key_gen,
        svk_ciphertext_blob: b64(&user.svk_ciphertext_blob),
    }))
}

async fn account_rotate_key(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<RotateKeyReq>,
) -> Result<Json<RotateKeyResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let svk = decode_b64(&req.new_svk_ciphertext_blob)?;
    let Some(user) = st
        .repo
        .get_user_by_id(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    // Idempotent: only bump the epoch forward.
    if req.new_min_enc_key_gen <= user.min_enc_key_gen {
        return Ok(Json(RotateKeyResp {
            status: "success".into(),
            min_enc_key_gen: user.min_enc_key_gen,
        }));
    }
    st.repo
        .update_min_enc_key_gen(&user_id, req.new_min_enc_key_gen)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    st.repo
        .update_svk(&user_id, &svk, &user.svk_ciphertext_blob_rk)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(RotateKeyResp {
        status: "success".into(),
        min_enc_key_gen: req.new_min_enc_key_gen,
    }))
}

// ---------------------------------------------------------------------------
// Bearer extractor (axum 0.8: FromRequestParts is an async trait method)
// ---------------------------------------------------------------------------

struct Bearer(pub String);

impl<S> axum::extract::FromRequestParts<S> for Bearer
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Some(auth) = parts.headers.get(header::AUTHORIZATION) else {
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
