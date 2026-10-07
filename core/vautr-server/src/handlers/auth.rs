//! OPAQUE authentication handlers (api.md §3): /auth/register/start|finish,
//! /auth/login/start|finish. The server only ever sees OPAQUE messages and
//! AEAD ciphertext blobs, never the Master Password or Master Key.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use axum::{extract::State, Json, http::StatusCode,};
use serde::{Deserialize, Serialize};
use vautr_crypto::opaque;

use super::{b64, decode_b64, now_ms, server_setup, ApiError, AppState};
use super::Bearer;

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
    registration_finish: String,    // base64
    server_public_key: String,      // base64 (opaque server setup, first-time)
    kdf_salt: String,               // base64 (stored, never used server-side)
    svk_ciphertext_blob: String,    // base64 (MP-wrapped SVK)
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

    /// VTRFIX-SEC-M20: opaque handle to pass back on finish.
    pub login_handle: String,
}
#[derive(Deserialize)]
pub(crate) struct LoginFinishReq {
    username: String,
    login_finish: String, // base64

    /// VTRFIX-SEC-M20: handle returned by login_start (optional for legacy).
    #[serde(default)]
    pub login_handle: Option<String>,
}
/// Response payload indicating a second factor is required (VTRFIX-SEC-C03).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MfaChallenge {
    /// Single-use token; the client passes it to `/mfa/totp/verify-login`.
    pub(crate) pending_token: String,
    /// Methods the client can present (currently `["totp"]`).
    pub(crate) methods: Vec<String>,
    /// Epoch ms when the pending token expires.
    pub(crate) expires_at: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginFinishResp {
    /// `None` when `mfa_required` is `Some(_)` — the second factor must be
    /// completed via `POST /mfa/totp/verify-login` before any session exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mfa_required: Option<MfaChallenge>,
}

/// Hex-encoded SHA-256 of `bytes`. Used to hash single-use pending-MFA
/// tokens so the DB never contains the raw secret (VTRFIX-SEC-C03).
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
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

// VTRFIX-BUG-M19 (tracked): `server_public_key` in the register request is
// currently ignored. Real distribution requires a bootstrap endpoint so the
// client can pin the server setup on first use. Tracked for a follow-up in
// docs/issues/VTRFIX-LOG.md.
pub(crate) async fn register_finish(
    State(st): State<AppState>,
    Json(req): Json<RegisterFinishReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let _setup = server_setup(&st.repo).await?;
    let _pk = decode_b64(&req.server_public_key)?;
    let cupload = decode_b64(&req.registration_finish)?;
    let record =
        opaque::server_register_finish(&cupload).map_err(|e| ApiError::internal(&e.to_string()))?;

    let kdf_salt = decode_b64(&req.kdf_salt)?;
    let svk = decode_b64(&req.svk_ciphertext_blob)?;
    let svk_rk = decode_b64(&req.svk_ciphertext_blob_rk)?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .create_user(
            &user_id,
            &req.username,
            &kdf_salt,
            &record,
            &svk,
            &svk_rk,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // VTRFIX-SEC-M18: audit registration.
    if let Ok(Some(u)) = st.repo.get_user_by_email(&req.username).await {
        let _ = st
            .repo
            .audit_org_event(
                Some(&u.id),
                Some(&u.id),
                "auth.register",
                "user",
                Some(&u.id),
                None,
                None,
                now_ms(),
            )
            .await;
    }
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
    let (sresp, sstate) = opaque::server_login_start(
        &setup,
        Some(&user.opaque_record),
        &lreq,
        req.username.as_bytes(),
    )
    .map_err(|e| ApiError::internal(&e.to_string()))?;
    // VTRFIX-SEC-M20: use a random handle so concurrent logins for the same
    // username cannot clobber each other, and the handle never leaks the
    // username. Bound the map to prevent unbounded growth from probes.
    let handle = format!("vtr-lh-{}", uuid::Uuid::new_v4().simple());
    {
        let mut map = LOGIN_STATE.lock().unwrap_or_else(|p| p.into_inner());
        // Prune opportunistically.
        if map.len() > 10_000 {
            map.clear();
        }
        // Store under the handle (canonical) and under the username as a
        // one-release compatibility alias so older clients that don't echo
        // `login_handle` still work.
        map.insert(handle.clone(), sstate.clone());
        map.insert(req.username.clone(), sstate);
    }
    Ok(Json(LoginStartResp {
        login_response: b64(&sresp),
        login_handle: handle,
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
        // VTRFIX-SEC-M18: log failed login attempts (identifier only).
        let _ = st
            .repo
            .audit_org_event(None, None, "auth.login_failed", "session", None, Some(&req.username), None, now_ms())
            .await;
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    let lupload = decode_b64(&req.login_finish)?;
    // VTRFIX-SEC-M20: prefer the random handle; fall back to the legacy
    // username-keyed lookup for one release while older clients catch up.
    let sstate = {
        let mut map = LOGIN_STATE.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(h) = req.login_handle.as_deref() {
            map.remove(h)
        } else {
            map.remove(&req.username)
        }
    }
    .ok_or_else(|| ApiError::bad_request("invalid_login", "no login in progress"))?;
    let sfin = opaque::server_login_finish(&sstate, &lupload)
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Mandatory-MFA enforcement (Wave A4): when the org policy makes MFA
    // required, a user with no configured method cannot complete login. This
    // keeps the OPAQUE handshake intact and only adds a business gate before a
    // session token is minted.
    // Policy gate (unchanged): if MFA is mandatory and the user has no method
    // configured, reject at login.
    super::mfa::enforce_mfa_required(&st, &user.id).await?;

    // VTRFIX-SEC-C03: if the user has a TOTP secret configured, DO NOT mint a
    // session yet. Instead return a short-lived pending token; the client must
    // complete POST /mfa/totp/verify-login with a valid code to obtain one.
    let has_totp = st
        .repo
        .mfa_has_totp(&user.id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if has_totp {
        // Opportunistic pruning of expired rows.
        let _ = st.repo.purge_expired_pending_mfa(now_ms()).await;

        let raw = format!("vtr-mfa-{}", uuid::Uuid::new_v4().simple());
        let hash = sha256_hex(raw.as_bytes());
        let expires_at = now_ms() + 5 * 60 * 1000; // 5 minutes
        st.repo
            .store_pending_mfa(&hash, &user.id, expires_at, now_ms())
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;

        return Ok(Json(LoginFinishResp {
            session_token: None,
            expires_at: None,
            mfa_required: Some(MfaChallenge {
                pending_token: raw,
                methods: vec!["totp".to_string()],
                expires_at,
            }),
        }));
    }

    // No second factor required — mint the session as before.
    let token = b64(&sfin);
    let expires_at = now_ms() + 86_400_000; // 24h
    st.repo
        .store_session(&token, &user.id, expires_at)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // VTRFIX-SEC-M18: audit successful logins (no secrets in detail).
    let _ = st
        .repo
        .audit_org_event(
            Some(&user.id),
            Some(&user.id),
            "auth.login_success",
            "session",
            None,
            None,
            None,
            now_ms(),
        )
        .await;
    Ok(Json(LoginFinishResp {
        session_token: Some(token),
        expires_at: Some(expires_at),
        mfa_required: None,
    }))
}

/// `POST /auth/logout` — deletes the presented session token (VTRFIX-SEC-H03).
pub(crate) async fn logout(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<StatusCode, ApiError> {
    let _ = super::auth_user(&st.repo, &auth.0).await?;
    st.repo
        .delete_session(&auth.0)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
