//! OPAQUE authentication handlers (api.md §3): /auth/register/start|finish,
//! /auth/login/start|finish. The server only ever sees OPAQUE messages and
//! AEAD ciphertext blobs, never the Master Password or Master Key.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use vautr_crypto::opaque;

use super::{ApiError, AppState, b64, decode_b64, now_ms, server_setup};

/// In-memory OPAQUE server-login state, keyed by username.
///
/// OPAQUE is a multi-round protocol: `login_start` must persist the ephemeral
/// `ServerLogin` state so `login_finish` can complete the key exchange. There
/// is no cross-request state store in the server, so we keep a small in-memory
/// map keyed by username. Entries are short-lived (a single login round) and
/// scoped to this process (dev/self-host topology). This is a minimal fix to
/// thread the state that `login_start` previously discarded (the old code
/// passed the server *setup* bytes to `finish`, which cannot validate login).
static LOGIN_STATE: LazyLock<Mutex<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Deserialize)]
pub(crate) struct RegisterStartReq {
    username: String,
    registration_start: String, // base64
}
#[derive(Serialize)]
pub(crate) struct RegisterStartResp {
    registration_response: String,
}
#[derive(Deserialize)]
pub(crate) struct RegisterFinishReq {
    username: String,
    registration_finish: String, // base64
    server_public_key: String,   // base64 (opaque server setup, first-time)
    kdf_salt: String,            // base64 (stored, never used server-side)
    svk_ciphertext_blob: String, // base64 (MP-wrapped SVK)
    svk_ciphertext_blob_rk: String, // base64 (RK-wrapped SVK, REQ-RECOVERY-02)
}
#[derive(Serialize)]
pub(crate) struct StatusResp {
    status: String,
}
#[derive(Deserialize)]
pub(crate) struct LoginStartReq {
    username: String,
    login_start: String, // base64
}
#[derive(Serialize)]
pub(crate) struct LoginStartResp {
    login_response: String,
}
#[derive(Deserialize)]
pub(crate) struct LoginFinishReq {
    username: String,
    login_finish: String, // base64
}
#[derive(Serialize)]
pub(crate) struct LoginFinishResp {
    session_token: String,
    expires_at: i64,
}

pub(crate) async fn register_start(
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

pub(crate) async fn register_finish(
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

pub(crate) async fn login_start(
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
    let (sresp, sstate) =
        opaque::server_login_start(&setup, Some(&user.opaque_record), &lreq, req.username.as_bytes())
            .map_err(|e| ApiError::internal(&e.to_string()))?;
    // Persist the ephemeral login state for `login_finish`.
    LOGIN_STATE
        .lock()
        .unwrap()
        .insert(req.username.clone(), sstate);
    Ok(Json(LoginStartResp {
        login_response: b64(&sresp),
    }))
}

pub(crate) async fn login_finish(
    State(st): State<AppState>,
    Json(req): Json<LoginFinishReq>,
) -> Result<Json<LoginFinishResp>, ApiError> {
    let Some(user) = st
        .repo
        .get_user_by_email(&req.username)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    let lupload = decode_b64(&req.login_finish)?;
    let sstate = LOGIN_STATE
        .lock()
        .unwrap()
        .remove(&req.username)
        .ok_or_else(|| ApiError::bad_request("invalid_login", "no login in progress"))?;
    let sfin = opaque::server_login_finish(&sstate, &lupload)
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
