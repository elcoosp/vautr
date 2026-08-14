//! Items handlers (api.md §4): PUT/DELETE /items/{uuid}.
//! Enforces OCC (ADR-004) and epoch gating (REQ-API-01) via `upsert_item_occ`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::repository::UpsertOutcome;

use super::{auth_user, decode_b64, now_ms, ApiError, AppState, Bearer};

#[derive(Deserialize)]
pub(crate) struct ItemPutReq {
    enc_key_gen: u64,
    payload: String, // base64
}
#[derive(Serialize)]
pub(crate) struct ItemPutResp {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    updated_at: i64,
}

pub(crate) async fn item_put(
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
        // A put that cleared a prior tombstone is a successful write; notify
        // clients so they re-sync to fetch the now-readable payload.
        UpsertOutcome::Recovered => {
            let _ = st.event_tx.send(crate::handlers::events::VaultEvent::ItemRecovered {
                uuid: uuid.parse().unwrap_or_default(),
            });
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

pub(crate) async fn item_delete(
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
        UpsertOutcome::Updated => {
            // Tombstone: proactively notify clients (VTR meets reaper, VTR-069).
            let _ = st.event_tx.send(
                crate::handlers::events::VaultEvent::ItemPermanentlyDeleted {
                    uuid: uuid.parse().unwrap_or_default(),
                },
            );
            Ok(Json(ItemPutResp {
                uuid,
                version: row.version + 1,
                enc_key_gen: row.enc_key_gen,
                updated_at: now,
            }))
        }
        // A delete sets deleted_date, so upsert_item_occ never returns Recovered
        // here; the arm exists only for match exhaustiveness.
        UpsertOutcome::Recovered => Ok(Json(ItemPutResp {
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
