//! Emergency recovery & account lifecycle HTTP handlers.
//! Spec: docs/architecture/emergency-recovery-account.md §2-4.
//! Routes:
//!   GET    /account/recover/challenge  — issue a fresh nonce (§2.3).
//!   POST   /account/recover/verify     — verify Ed25519 sig over nonce vs
//!                                        `users.rk_public_key`; issue one-time
//!                                        recovery token (§2.3).
//!   POST   /account/recover/complete   — atomically replace OPAQUE record,
//!                                        wrapped SVKs and RK pubkey (§2.3 step 7).
//!   POST   /account/reclaim            — initiate unauthenticated reclaim (§4.2).
//!   POST   /account/reclaim/confirm    — confirm reclaim link, release email (§4.2).
//!   DELETE /account                    — authenticated account deletion (§4.1).
//! Owned by the Wave B recovery agent.

use axum::{
    extract::State,
    routing::{delete, post},
    Json, Router,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::{auth_user, b64, decode_b64, now_ms, ApiError, AppState, Bearer};

/// Lifetime of a recovery session token (short-lived, per spec §2.3).
const RECOVERY_TOKEN_TTL_MS: i64 = 600_000; // 10 minutes
/// 30-day grace period before a reclaimed vault is purged (§4.2).
const RECLAIM_SUSPENSION_MS: i64 = 30 * 24 * 3_600_000;

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        // VTRFIX-SEC-H09: POST (email in body) so the nonce is bound to a user.
        .route("/account/recover/challenge", post(recover_challenge))
        .route("/account/recover/verify", post(recover_verify))
        // VTRFIX-FEAT-H02: return the RK-wrapped SVK so the client can unwrap it
        // with the mnemonic before calling /recover/complete.
        .route("/account/recover/info", post(recover_info))
        .route("/account/recover/complete", post(recover_complete))
        .route("/account/reclaim", post(reclaim))
        .route("/account/reclaim/confirm", post(reclaim_confirm))
        .route("/account", delete(delete_account))
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// VTRFIX-SEC-H09: challenge now requires the account email so the nonce can
/// be bound to a specific user and persisted for single-use verification.
#[derive(Deserialize)]
pub(crate) struct ChallengeReq {
    email: String,
}

#[derive(Serialize)]
pub(crate) struct ChallengeResp {
    nonce: String, // base64
}

#[derive(Deserialize)]
pub(crate) struct VerifyReq {
    email: String,
    /// VTRFIX-SEC-H09: optional echo of the challenge nonce; if provided it
    /// must match the stored one. If omitted, the stored nonce is used.
    #[serde(default)]
    nonce: Option<String>,
    signature: String, // base64 (Ed25519 over the stored nonce)
}
#[derive(Serialize)]
pub(crate) struct VerifyResp {
    recovery_token: String,
    expires_at: i64,
}

#[derive(Deserialize)]
pub(crate) struct CompleteReq {
    recovery_token: String,
    opaque_record: String,          // base64
    kdf_salt: String,               // base64
    svk_ciphertext_blob: String,    // base64 (MP-wrapped SVK)
    svk_ciphertext_blob_rk: String, // base64 (RK-wrapped SVK)
    rk_public_key: String,          // base64 (new RK Ed25519 public key)
}

#[derive(Deserialize)]
pub(crate) struct ReclaimReq {
    email: String,
    /// VTRFIX-SEC-H10: base64 Ed25519 signature over the challenge nonce for
    /// the same email. Obtained from /account/recover/challenge.
    signature: String,
}
#[derive(Deserialize)]
pub(crate) struct ReclaimConfirmReq {
    token: String,
}
#[derive(Serialize)]
pub(crate) struct StatusResp {
    status: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Issue a fresh, server-side random nonce for the RK proof-of-possession gate.
pub(crate) async fn recover_challenge(
    State(st): State<AppState>,
    Json(req): Json<ChallengeReq>,
) -> Result<Json<ChallengeResp>, ApiError> {
    // VTRFIX-SEC-H09: persist the nonce per user (single-use, 5-min TTL).
    // Return the same shape for unknown emails to avoid enumeration.
    const CHALLENGE_TTL_MS: i64 = 5 * 60 * 1000;
    let nonce = {
        let mut n = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut n);
        n.to_vec()
    };
    if let Some(user) = st
        .repo
        .get_user_by_email(&req.email)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        st.repo
            .store_recovery_challenge(&user.id, &nonce, now_ms() + CHALLENGE_TTL_MS, now_ms())
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
    }
    Ok(Json(ChallengeResp { nonce: b64(&nonce) }))
}

/// Verify an Ed25519 signature over the challenge nonce against the stored RK
/// public key. On success, issue a short-lived, one-time recovery session token.
pub(crate) async fn recover_verify(
    State(st): State<AppState>,
    Json(req): Json<VerifyReq>,
) -> Result<Json<VerifyResp>, ApiError> {
    let Some(user) = st
        .repo
        .get_user_by_email(&req.email)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request("not_found", "unknown user"));
    };
    let Some(pk_bytes) = st
        .repo
        .get_rk_public_key(&user.id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::bad_request(
            "rk_not_configured",
            "no recovery key registered for this account",
        ));
    };
    if pk_bytes.len() != 32 {
        return Err(ApiError::bad_request(
            "invalid_rk_key",
            "malformed recovery key",
        ));
    }
    // VTRFIX-SEC-H09: verify against the STORED nonce (single-use).
    let (stored_nonce, expires_at) = st
        .repo
        .get_recovery_challenge(&user.id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| {
            ApiError::bad_request("no_challenge", "call /account/recover/challenge first")
        })?;
    if now_ms() > expires_at {
        let _ = st.repo.delete_recovery_challenge(&user.id).await;
        return Err(ApiError::bad_request(
            "challenge_expired",
            "challenge expired; request a new one",
        ));
    }
    // Optional echo: if supplied, must match the stored nonce.
    if let Some(echo) = req.nonce.as_deref() {
        if decode_b64(echo)? != stored_nonce {
            return Err(ApiError::unauthorized());
        }
    }
    let nonce = stored_nonce.clone();
    let sig_bytes = decode_b64(&req.signature)?;
    if sig_bytes.len() != 64 {
        return Err(ApiError::bad_request(
            "invalid_signature",
            "signature must be 64 bytes",
        ));
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk_bytes);
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_arr) else {
        return Err(ApiError::bad_request(
            "invalid_rk_key",
            "malformed recovery key",
        ));
    };
    let Ok(signature) = Signature::from_slice(&sig_bytes) else {
        return Err(ApiError::bad_request(
            "invalid_signature",
            "malformed signature",
        ));
    };
    // Signature over the exact challenge nonce bytes.
    if verifying_key.verify_strict(&nonce, &signature).is_err() {
        return Err(ApiError::unauthorized());
    }

    // VTRFIX-SEC-H09: single-use — consume the challenge NOW, before
    // minting the recovery token, so a replayed (nonce, sig) fails.
    st.repo
        .delete_recovery_challenge(&user.id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let token = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    let expires_at = now + RECOVERY_TOKEN_TTL_MS;
    st.repo
        .create_recovery_session(&token, &user.id, expires_at, now)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(VerifyResp {
        recovery_token: token,
        expires_at,
    }))
}

/// Authenticate with a one-time recovery token and atomically replace the
/// account's OPAQUE record, wrapped SVKs and RK public key (forced MP/RK
/// rotation, spec §2.3 step 7).
pub(crate) async fn recover_complete(
    State(st): State<AppState>,
    Json(req): Json<CompleteReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let Some((user_id, expires_at)) = st
        .repo
        .consume_recovery_session(&req.recovery_token)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::unauthorized());
    };
    if expires_at < now_ms() {
        return Err(ApiError::unauthorized());
    }

    let opaque_record = decode_b64(&req.opaque_record)?;
    let kdf_salt = decode_b64(&req.kdf_salt)?;
    let svk_mp = decode_b64(&req.svk_ciphertext_blob)?;
    let svk_rk = decode_b64(&req.svk_ciphertext_blob_rk)?;
    let rk_pk = decode_b64(&req.rk_public_key)?;
    if rk_pk.len() != 32 {
        return Err(ApiError::bad_request(
            "invalid_rk_key",
            "RK public key must be 32 bytes",
        ));
    }

    let now = now_ms();
    st.repo
        .complete_recovery(
            &user_id,
            &opaque_record,
            &kdf_salt,
            &svk_mp,
            &svk_rk,
            &rk_pk,
            now,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // Force re-authentication everywhere: recovery rotates all credentials.
    st.repo
        .revoke_user_sessions(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

/// Initiate the unauthenticated reclaim flow (§4.2): bind a single-use reclaim
/// token to the user's email and suspend (hide) the vault for 30 days. In a
/// deployed server the token is delivered via an emailed verification link;
/// here we persist it for the confirm endpoint.
pub(crate) async fn reclaim(
    State(st): State<AppState>,
    Json(req): Json<ReclaimReq>,
) -> Result<Json<StatusResp>, ApiError> {
    // VTRFIX-SEC-H10: reclaim must not be triggerable by an unauthenticated
    // attacker who merely knows the email. Require proof of RK possession
    // (a valid Ed25519 signature over the server-issued challenge nonce).
    // Respond uniformly to avoid account enumeration.
    let Some(user) = st
        .repo
        .get_user_by_email(&req.email)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        // Unknown email: same 202 shape, no state change.
        return Ok(Json(StatusResp {
            status: "verification_required".into(),
        }));
    };

    let proof_ok = (|| async {
        let Some(pk_bytes) = st
            .repo
            .get_rk_public_key(&user.id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
        else {
            return Ok::<bool, ApiError>(false);
        };
        if pk_bytes.len() != 32 {
            return Ok(false);
        }
        let Some((nonce, expires_at)) = st
            .repo
            .get_recovery_challenge(&user.id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
        else {
            return Ok(false);
        };
        if now_ms() > expires_at {
            let _ = st.repo.delete_recovery_challenge(&user.id).await;
            return Ok(false);
        }
        let sig_bytes = decode_b64(&req.signature)?;
        if sig_bytes.len() != 64 {
            return Ok(false);
        }
        let mut pk_arr = [0u8; 32];
        pk_arr.copy_from_slice(&pk_bytes);
        let Ok(vk) = VerifyingKey::from_bytes(&pk_arr) else {
            return Ok(false);
        };
        let Ok(sig) = Signature::from_slice(&sig_bytes) else {
            return Ok(false);
        };
        Ok(vk.verify_strict(&nonce, &sig).is_ok())
    })()
    .await?;

    if !proof_ok {
        // Same response shape as success — no state change, no enumeration.
        return Ok(Json(StatusResp {
            status: "verification_required".into(),
        }));
    }

    // Consume the challenge.
    let _ = st.repo.delete_recovery_challenge(&user.id).await;

    let token = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .store_reclaim(&user.id, &token, now + RECLAIM_SUSPENSION_MS)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(StatusResp {
        status: "verification_required".into(),
    }))
}

/// Confirm a reclaim via its token (§4.2 step 2): clear the token, lift the
/// suspension so the email can be re-registered for a fresh vault.
pub(crate) async fn reclaim_confirm(
    State(st): State<AppState>,
    Json(req): Json<ReclaimConfirmReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let Some((user_id, _email)) = st
        .repo
        .get_user_by_reclaim_token(&req.token)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::unauthorized());
    };
    st.repo
        .clear_reclaim(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

/// Authenticated account deletion (§4.1): permanently delete the user and all
/// cascaded rows (OPAQUE record, wrapped SVKs, encrypted payloads).
pub(crate) async fn delete_account(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    st.repo
        .delete_account(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Method, Request, StatusCode};
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_signing_key(seed: u8) -> SigningKey {
        let mut bytes = [0u8; 32];
        bytes.fill(seed);
        SigningKey::from_bytes(&bytes)
    }

    /// Build a router exposing just this feature's routes, so tests are
    /// independent of `build_router` (which lives in mod.rs, owned elsewhere).
    fn feature_router(state: AppState) -> axum::Router {
        super::routes().with_state(state)
    }

    async fn test_state() -> AppState {
        let path = std::env::temp_dir().join(format!("vautr_rec_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        AppState::new(Arc::new(crate::repository::Repository::new(pool)))
    }

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn seed_user(state: &AppState, email: &str) -> String {
        let user_id = uuid::Uuid::new_v4().to_string();
        state
            .repo
            .create_user(
                &user_id,
                email,
                &[0u8; 16],
                &[1u8; 32],
                &[2u8; 48],
                &[3u8; 48],
                now_ms(),
            )
            .await
            .unwrap();
        user_id
    }

    /// Full recovery gate: challenge → verify → complete.
    #[tokio::test]
    async fn recovery_challenge_verify_complete_flow() {
        let state = test_state().await;
        let email = "recover@example.com";
        let user_id = seed_user(&state, email).await;
        let signing = test_signing_key(1);
        let rk_pk = signing.verifying_key().to_bytes().to_vec();
        state
            .repo
            .set_rk_public_key(&user_id, &rk_pk)
            .await
            .unwrap();

        let app = feature_router(state.clone());

        // 1. Challenge (VTRFIX-SEC-H09: POST + email body).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/challenge")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "email": email }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let nonce = decode_b64(&json_body(resp).await["nonce"].as_str().unwrap()).unwrap();

        // 2. Sign the nonce and verify.
        let sig = signing.sign(&nonce).to_bytes();
        let body = serde_json::json!({
            "email": email,
            "nonce": b64(&nonce),
            "signature": b64(&sig),
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/verify")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "verify should succeed");
        let v = json_body(resp).await;
        let token = v["recovery_token"].as_str().unwrap().to_string();

        // 3. Complete with a fresh OPAQUE record + wrapped SVKs + new RK key.
        let new_pk = test_signing_key(2).verifying_key().to_bytes().to_vec();
        let body = serde_json::json!({
            "recovery_token": token,
            "opaque_record": b64(&[9u8; 32]),
            "kdf_salt": b64(&[8u8; 16]),
            "svk_ciphertext_blob": b64(&[7u8; 48]),
            "svk_ciphertext_blob_rk": b64(&[6u8; 48]),
            "rk_public_key": b64(&new_pk),
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/complete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "complete should succeed");

        // Credentials were atomically replaced.
        let stored_pk = state
            .repo
            .get_rk_public_key(&user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored_pk, new_pk);

        // The one-time token is consumed: reusing it must fail.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/complete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({
                            "recovery_token": token,
                            "opaque_record": b64(&[1u8; 32]),
                            "kdf_salt": b64(&[2u8; 16]),
                            "svk_ciphertext_blob": b64(&[3u8; 48]),
                            "svk_ciphertext_blob_rk": b64(&[4u8; 48]),
                            "rk_public_key": b64(&new_pk),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "token is one-time use"
        );
    }

    /// A signature made with the wrong key must be rejected.
    #[tokio::test]
    async fn recovery_verify_rejects_wrong_signature() {
        let state = test_state().await;
        let email = "recover2@example.com";
        let user_id = seed_user(&state, email).await;
        let signing = test_signing_key(1);
        state
            .repo
            .set_rk_public_key(&user_id, &signing.verifying_key().to_bytes().to_vec())
            .await
            .unwrap();
        let app = feature_router(state.clone());

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/challenge")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "email": email }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let nonce = decode_b64(&json_body(resp).await["nonce"].as_str().unwrap()).unwrap();

        let wrong = test_signing_key(3);
        let sig = wrong.sign(&nonce).to_bytes();
        let body = serde_json::json!({
            "email": email,
            "nonce": b64(&nonce),
            "signature": b64(&sig),
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/verify")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Reclaim lifecycle: initiate → confirm → suspension cleared.
    #[tokio::test]
    async fn reclaim_lifecycle() {
        let state = test_state().await;
        let email = "lifecycle@example.com";
        let user_id = seed_user(&state, email).await;
        // VTRFIX-SEC-H10: reclaim now requires RK proof-of-possession.
        let signing = test_signing_key(7);
        state
            .repo
            .set_rk_public_key(&user_id, &signing.verifying_key().to_bytes().to_vec())
            .await
            .unwrap();
        let app = feature_router(state.clone());

        // 1. Get a challenge.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/recover/challenge")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "email": email }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let nonce = decode_b64(&json_body(resp).await["nonce"].as_str().unwrap()).unwrap();

        // 2. Sign + reclaim.
        let sig = signing.sign(&nonce).to_bytes();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/reclaim")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({
                            "email": email,
                            "signature": b64(&sig),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Look up the token the way the emailed link would (repo read).
        let row = sqlx::query_as::<_, (Option<String>, Option<i64>)>(
            "SELECT reclaim_token, suspended_until FROM users WHERE id = ?",
        )
        .bind(&user_id)
        .fetch_one(state.repo.pool())
        .await
        .unwrap();
        let (token, suspended) = row;
        let token = token.unwrap();
        assert!(
            suspended.unwrap() > now_ms(),
            "vault suspended for grace period"
        );

        // Confirm reclaim via the token.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/account/reclaim/confirm")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "token": token }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Token cleared.
        let row =
            sqlx::query_as::<_, (Option<String>,)>("SELECT reclaim_token FROM users WHERE id = ?")
                .bind(&user_id)
                .fetch_one(state.repo.pool())
                .await
                .unwrap();
        assert!(row.0.is_none(), "reclaim token cleared");
    }
}

// ---------------------------------------------------------------------------
// VTRFIX-FEAT-H02: recovery info
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct RecoverInfoReq {
    recovery_token: String,
}

#[derive(Serialize)]
pub(crate) struct RecoverInfoResp {
    /// base64 of the current `svk_ciphertext_blob_rk` (RK-wrapped SVK).
    svk_ciphertext_blob_rk: String,
    /// Server user id (needed as AD for the unwrap).
    user_id: String,
}

/// `POST /account/recover/info` — after /recover/verify succeeds, the client
/// needs the current RK-wrapped SVK blob so it can unwrap the SVK with the
/// mnemonic and re-wrap it under a new password. This route returns that blob
/// gated on the recovery token.
pub(crate) async fn recover_info(
    State(st): State<AppState>,
    Json(req): Json<RecoverInfoReq>,
) -> Result<Json<RecoverInfoResp>, ApiError> {
    let Some((user_id, _)) = st
        .repo
        .peek_recovery_session(&req.recovery_token)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::unauthorized());
    };
    let user = st
        .repo
        .get_user_by_id(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .ok_or_else(|| ApiError::unauthorized())?;
    Ok(Json(RecoverInfoResp {
        svk_ciphertext_blob_rk: b64(&user.svk_ciphertext_blob_rk),
        user_id,
    }))
}
