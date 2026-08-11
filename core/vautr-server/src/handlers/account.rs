//! Account & key-management handlers (api.md §5): /account/status,
//! /account/rotate-key. Idempotent epoch forward-gating for key rotation.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, Bearer, auth_user, b64, decode_b64};

#[derive(Serialize)]
pub(crate) struct AccountStatusResp {
    min_enc_key_gen: i64,
    svk_ciphertext_blob: String, // base64
}
#[derive(Deserialize)]
pub(crate) struct RotateKeyReq {
    new_min_enc_key_gen: i64,
    new_svk_ciphertext_blob: String, // base64 (MP-wrapped)
}
#[derive(Serialize)]
pub(crate) struct RotateKeyResp {
    status: String,
    min_enc_key_gen: i64,
}

pub(crate) async fn account_status(
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

pub(crate) async fn account_rotate_key(
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
