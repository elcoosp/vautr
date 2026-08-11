//! Sync handlers (api.md §4): /sync/pull, /sync/pull-payloads, /sync/push-batch.
//! Enforces OCC (ADR-004) and epoch gating (REQ-API-01).

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::repository::{ItemRow, UpsertOutcome};

use super::{ApiError, AppState, Bearer, auth_user, b64, decode_b64, now_ms};

#[derive(Deserialize)]
pub(crate) struct PullQuery {
    cursor: u64,
    limit: Option<u32>,
}
#[derive(Serialize)]
pub(crate) struct PullResp {
    new_cursor: u64,
    has_more: bool,
    min_enc_key_gen: i64,
    items: Vec<PullItem>,
}
#[derive(Serialize)]
pub(crate) struct PullItem {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    deleted_date: Option<i64>,
}
#[derive(Deserialize)]
pub(crate) struct PullPayloadsReq {
    items: Vec<PullTarget>,
}
#[derive(Deserialize)]
pub(crate) struct PullTarget {
    uuid: String,
    version: u64,
}
#[derive(Serialize)]
pub(crate) struct PullPayloadsResp {
    results: Vec<PayloadResult>,
}
#[derive(Serialize)]
pub(crate) struct PayloadResult {
    uuid: String,
    status: String, // "payload_delivered" | "version_mismatch"
    version: i64,
    enc_key_gen: i64,
    deleted_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>, // base64
}
#[derive(Deserialize)]
pub(crate) struct PushBatchReq {
    items: Vec<PushItem>,
}
#[derive(Deserialize)]
pub(crate) struct PushItem {
    uuid: String,
    target_version: u64,
    enc_key_gen: u64,
    #[serde(default)]
    payload: Option<String>, // base64 (None = tombstone)
    #[serde(default)]
    deleted_date: Option<i64>,
}
#[derive(Serialize)]
pub(crate) struct PushBatchResp {
    results: Vec<PushResult>,
}
#[derive(Serialize)]
pub(crate) struct PushResult {
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
pub(crate) struct ServerState {
    version: i64,
    enc_key_gen: i64,
}

pub(crate) async fn sync_pull(
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
    let rows = sqlx::query_as::<_, ItemRow>(
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

pub(crate) async fn sync_pull_payloads(
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

pub(crate) async fn sync_push_batch(
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
