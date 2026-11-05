//! OPAQUE authentication handlers (api.md §3): /auth/register/start|finish,
//! /auth/login/start|finish. The server only ever sees OPAQUE messages and
//! AEAD ciphertext blobs, never the Master Password or Master Key.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use vautr_crypto::opaque;

use super::Bearer;
use super::{b64, decode_b64, now_ms, server_setup, ApiError, AppState};

/// In-memory OPAQUE server-login state, keyed by handle (and a legacy username
/// alias). Value is `(ServerLogin state, created_at epoch ms)`.
///
/// OPAQUE is a multi-round protocol: `login_start` must persist the ephemeral
/// `ServerLogin` state so `login_finish` can complete the key exchange. There
/// is no cross-request state store in the server, so we keep a small in-memory
/// map. Entries are short-lived (a single login round) and scoped to this
/// process (dev/self-host topology).
static LOGIN_STATE: LazyLock<Mutex<HashMap<String, (Vec<u8>, i64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// VTRFIX (LOW): per-entry TTL for `LOGIN_STATE` — a login round that is never
/// finished is swept after 10 minutes instead of the old wholesale `clear()`
/// at 10k entries (which could evict other users' in-flight logins).
const LOGIN_STATE_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Deserialize)]
pub(crate) struct RegisterStartReq {
    username: String,
    registration_start: String, // base64
}
#[derive(Serialize)]
pub(crate) struct RegisterStartResp {
    registration_response: String,
    /// VTRFIX (HIGH): server-key distribution. A random 32-byte server public
    /// key, persisted in `server_config` (`server_public_key`) and handed out
    /// here; the client echoes it back in `register/finish` where it is
    /// verified. Replaces the old `[0u8;32]` placeholder (rejected by M19) and
    /// the dangerous legacy behavior of clients sending a serialized OPAQUE
    /// ServerSetup (which contains the OPRF PRIVATE key).
    server_public_key: String,
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

/// VTRFIX-SEC-M02: registration response carries the server-minted user id so
/// the client can immediately re-wrap its SVK under the user-scoped AD.
#[derive(Serialize)]
pub(crate) struct RegisterFinishResp {
    pub status: String,
    pub user_id: String,
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
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// VTRFIX (HIGH): get-or-create the server's 32-byte public key (base64 wire
/// format). Persisted in `server_config` under [`SERVER_PUBLIC_KEY`]; on a
/// multi-process deployment the first writer wins and everyone serves the
/// persisted value (re-read after write, mirroring `server_setup`).
const SERVER_PUBLIC_KEY: &str = "server_public_key";

async fn server_public_key(repo: &crate::repository::Repository) -> Result<Vec<u8>, ApiError> {
    if let Some(pk) = repo
        .get_config(SERVER_PUBLIC_KEY)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        if pk.len() == 32 {
            return Ok(pk);
        }
    }
    let mut fresh = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut fresh);
    repo.set_config(SERVER_PUBLIC_KEY, fresh.as_slice())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // Re-read: another process may have won the race.
    repo.get_config(SERVER_PUBLIC_KEY)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .filter(|pk| pk.len() == 32)
        .ok_or_else(|| ApiError::internal("server_public_key disappeared after write"))
}

/// VTRFIX (LOW): true when the sqlx error is a SQLite UNIQUE constraint
/// violation (used to map the `users.email` unique-index race to a 409).
fn is_unique_violation(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Database(db) => {
            // 2067 = SQLITE_CONSTRAINT_UNIQUE.
            db.code().as_deref() == Some("2067")
                || db.message().contains("UNIQUE constraint failed")
        }
        _ => false,
    }
}

pub(crate) async fn register_start(
    State(st): State<AppState>,
    Json(req): Json<RegisterStartReq>,
) -> Result<Json<RegisterStartResp>, ApiError> {
    let setup = server_setup(&st.repo).await?;
    let creq = decode_b64(&req.registration_start)?;
    let sresp = opaque::server_register_start(&setup, &creq, req.username.as_bytes())
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // VTRFIX (HIGH): distribute the server public key so clients can pin it
    // (echo protocol — see `RegisterStartResp.server_public_key`).
    let server_pk = server_public_key(&st.repo).await?;
    Ok(Json(RegisterStartResp {
        registration_response: b64(&sresp),
        server_public_key: b64(&server_pk),
    }))
}

pub(crate) async fn register_finish(
    State(st): State<AppState>,
    Json(req): Json<RegisterFinishReq>,
) -> Result<Json<RegisterFinishResp>, ApiError> {
    let setup_bytes = server_setup(&st.repo).await?;
    let _ = setup_bytes; // kept for the (future) public-key derivation
                         // VTRFIX-BUG-M19: reject the all-zeros placeholder that every client used
                         // to send (the audit found this pattern). A real OPRF public key is a
                         // non-trivial 32-byte curve point; a zero blob can never be one. This
                         // closes the "silently accept a stub" hole while a proper server-key
                         // distribution + pinning flow lands (tracked in VTRFIX-LOG).
    let pk = decode_b64(&req.server_public_key)?;
    if pk.iter().all(|b| *b == 0) {
        return Err(ApiError::bad_request(
            "invalid_server_key",
            "server_public_key must not be the all-zero placeholder",
        ));
    }
    if pk.len() < 32 {
        return Err(ApiError::bad_request(
            "invalid_server_key",
            "server_public_key is too short to be a valid OPRF public key",
        ));
    }
    // VTRFIX (HIGH): server-key pinning. The key must match the one this
    // server distributed in `register/start` (stored in `server_config`).
    // This rejects the legacy `[0u8;32]` placeholder, random garbage, and —
    // critically — a serialized OPAQUE ServerSetup containing the OPRF
    // PRIVATE key that old clients used to send.
    let stored_pk = server_public_key(&st.repo).await?;
    if pk != stored_pk {
        return Err(ApiError::bad_request(
            "invalid_server_key",
            "server_public_key does not match this server",
        ));
    }
    let cupload = decode_b64(&req.registration_finish)?;
    let record =
        opaque::server_register_finish(&cupload).map_err(|e| ApiError::internal(&e.to_string()))?;

    let kdf_salt = decode_b64(&req.kdf_salt)?;
    let svk = decode_b64(&req.svk_ciphertext_blob)?;
    let svk_rk = decode_b64(&req.svk_ciphertext_blob_rk)?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    // VTRFIX (LOW): duplicate email is a client error (409), not a 500 from the
    // UNIQUE constraint. Pre-check first; the race window is closed by mapping
    // the unique-index violation to the same error below.
    if st
        .repo
        .get_user_by_email(&req.username)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .is_some()
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "email_taken",
            "an account with this email already exists",
        ));
    }
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
        .map_err(|e| {
            if is_unique_violation(&e) {
                ApiError::new(
                    StatusCode::CONFLICT,
                    "email_taken",
                    "an account with this email already exists",
                )
            } else {
                ApiError::internal(&e.to_string())
            }
        })?;
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
    // VTRFIX-SEC-M02: return the server-minted user_id.
    Ok(Json(RegisterFinishResp {
        status: "success".into(),
        user_id: user_id.clone(),
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
        // VTRFIX (LOW): per-entry expiry sweep — drop entries older than 10
        // minutes instead of the old bulk `clear()` at 10k entries, which
        // could evict other users' in-flight login rounds.
        let cutoff = now_ms() - LOGIN_STATE_TTL_MS;
        map.retain(|_, (_, created)| *created >= cutoff);
        // Store under the handle (canonical) and under the username as a
        // one-release compatibility alias so older clients that don't echo
        // `login_handle` still work.
        let now = now_ms();
        map.insert(handle.clone(), (sstate.clone(), now));
        map.insert(req.username.clone(), (sstate, now));
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
            .audit_org_event(
                None,
                None,
                "auth.login_failed",
                "session",
                None,
                Some(&req.username),
                None,
                now_ms(),
            )
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
        .map(|(state, _created)| state)
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
    // VTRFIX (LOW): opportunistically prune expired sessions (the
    // `purge_expired_sessions` helper was previously never called).
    let _ = st.repo.purge_expired_sessions(now_ms()).await;
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
