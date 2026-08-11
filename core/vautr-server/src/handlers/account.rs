//! Account & key-management handlers (api.md §5): /account/status,
//! /account/rotate-key. Idempotent epoch forward-gating for key rotation.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, Bearer, auth_user, b64, decode_b64, now_ms};

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
    // Audit hook: metadata-only rotation event (VTR-053, server-scaling.md §8).
    st.repo
        .audit_log(Some(&user_id), "rotate_key", Some(&user_id), None, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(RotateKeyResp {
        status: "success".into(),
        min_enc_key_gen: req.new_min_enc_key_gen,
    }))
}

#[derive(Serialize)]
pub(crate) struct DeleteResp {
    status: String,
}

/// Account deletion (admin/reclaim). Writes an `account_deleted` audit entry
/// BEFORE any data purge (VTR-053). The physical purge of the user's rows is
/// performed by the account-lifecycle owner after this hook returns; this file
/// is owned by the hardening agent and the route lives in mod.rs.
#[allow(dead_code)] // endpoint route is wired by the router owner
pub(crate) async fn account_delete(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<DeleteResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    st.repo
        .audit_log(Some(&user_id), "account_deleted", Some(&user_id), None, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(DeleteResp {
        status: "success".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use std::sync::Arc;

    async fn test_state() -> AppState {
        let path =
            std::env::temp_dir().join(format!("vautr_acc_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000i64;
        repo.create_user("u1", "a@b.c", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64) // far-future expiry relative to real wall-clock
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        AppState::new(repo)
    }

    #[tokio::test]
    async fn rotate_key_writes_audit_entry() {
        let st = test_state().await;
        let svk_b64 = b64(&[7u8; 48]);
        let _resp = account_rotate_key(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(RotateKeyReq {
                new_min_enc_key_gen: 2,
                new_svk_ciphertext_blob: svk_b64,
            }),
        )
        .await
        .unwrap();
        let rows = st.repo.list_audit_logs(Some("u1"), 100, 0).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "rotate_key");
    }

    #[tokio::test]
    async fn delete_writes_audit_entry_before_purge() {
        let st = test_state().await;
        let _resp = account_delete(State(st.clone()), Bearer("tok1".into()))
            .await
            .unwrap();
        let rows = st.repo.list_audit_logs(Some("u1"), 100, 0).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "account_deleted");
    }
}
