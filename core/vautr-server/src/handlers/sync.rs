//! Sync handlers (api.md §4): /sync/pull, /sync/pull-payloads, /sync/push-batch.
//! Enforces OCC (ADR-004) and epoch gating (REQ-API-01).

use axum::{
    extract::{Query, State},
    http::StatusCode,
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

    // Bounded pagination (VTR-042): default 100, max 1000.
    let limit = q.limit.unwrap_or(100);
    if limit == 0 || limit > 1000 {
        return Err(ApiError::bad_request(
            "payload_limit_exceeded",
            "limit must be between 1 and 1000",
        ));
    }

    // Cursor expiration (api.md §2, server-db.md §5): the cursor is a
    // point-in-time marker over the per-user item history. If the requested
    // cursor points before the oldest retained version (i.e. history was
    // pruned/aged out), the server can no longer serve a consistent delta and
    // MUST return 410 `cursor_expired` so the client drops its cursor and does
    // a full resync from version 0.
    let min_ver: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(version) FROM items WHERE user_id = ?",
    )
    .bind(&user_id)
    .fetch_optional(st.repo.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;
    if let Some(min_ver) = min_ver {
        if (q.cursor as i64) < min_ver.saturating_sub(1) {
            return Err(ApiError::new(
                StatusCode::GONE,
                "cursor_expired",
                "The requested sync cursor is no longer available.",
            ));
        }
    }

    let limit_i = limit as i64;
    let rows = sqlx::query_as::<_, ItemRow>(
        "SELECT * FROM items WHERE user_id = ? AND version > ? ORDER BY version ASC LIMIT ?",
    )
    .bind(&user_id)
    .bind(q.cursor as i64)
    .bind(limit_i + 1)
    .fetch_all(st.repo.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let has_more = rows.len() as i64 > limit_i;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn seed() -> (AppState, String) {
        let path =
            std::env::temp_dir().join(format!("vautr_sync_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000;
        repo.create_user("u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        // NB: insert the session directly (created_at is NOT NULL) rather than
        // via Repository::store_session, which is owned by another agent.
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64) // far-future expiry relative to real wall-clock
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        for v in 1..=500i64 {
            sqlx::query(
                "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
                 VALUES (?, ?, ?, 1, NULL, NULL, ?)",
            )
            .bind(format!("item-{v}"))
            .bind("u1")
            .bind(v)
            .bind(now + v)
            .execute(repo.pool())
            .await
            .unwrap();
        }
        (AppState::new(repo), "tok1".to_string())
    }

    /// Seed a user whose oldest retained item version is 101 (versions 1..100
    /// absent, modelling pruned/aged-out history) without issuing a DELETE that
    /// would trip the `shares` FK mismatch.
    async fn seed_gapped() -> (AppState, String) {
        let path =
            std::env::temp_dir().join(format!("vautr_sync_gap_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000;
        repo.create_user("u2", "gap@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok2', 'u2', ?, ?)",
        )
        .bind(4_000_000_000_000i64) // far-future expiry relative to real wall-clock
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        for v in 101..=500i64 {
            sqlx::query(
                "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
                 VALUES (?, ?, ?, 1, NULL, NULL, ?)",
            )
            .bind(format!("gitem-{v}"))
            .bind("u2")
            .bind(v)
            .bind(now + v)
            .execute(repo.pool())
            .await
            .unwrap();
        }
        (AppState::new(repo), "tok2".to_string())
    }

    #[tokio::test]
    async fn pagination_walks_500_items() {
        let (st, tok) = seed().await;
        let mut cursor = 0u64;
        let mut got = 0u64;
        let mut has_more = true;
        while has_more {
            let resp = sync_pull(
                State(st.clone()),
                Query(PullQuery { cursor, limit: Some(100) }),
                Bearer(tok.clone()),
            )
            .await
            .unwrap()
            .0;
            // A page is never larger than the requested limit.
            assert!(resp.items.len() <= 100);
            got += resp.items.len() as u64;
            has_more = resp.has_more;
            cursor = resp.new_cursor;
        }
        assert_eq!(got, 500);
        assert_eq!(cursor, 500);
    }

    #[tokio::test]
    async fn pagination_first_page_reports_has_more() {
        let (st, tok) = seed().await;
        let resp = sync_pull(
            State(st),
            Query(PullQuery { cursor: 0, limit: Some(100) }),
            Bearer(tok),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(resp.items.len(), 100);
        assert!(resp.has_more);
        assert_eq!(resp.new_cursor, 100);
    }

    #[tokio::test]
    async fn limit_over_max_rejected() {
        let (st, tok) = seed().await;
        let res = sync_pull(
            State(st),
            Query(PullQuery { cursor: 0, limit: Some(2000) }),
            Bearer(tok),
        )
        .await;
        match res {
            Err(e) => assert_eq!(e.status, StatusCode::BAD_REQUEST),
            Ok(_) => panic!("expected limit rejection"),
        }
    }

    #[tokio::test]
    async fn expired_cursor_returns_410() {
        let (st, tok) = seed_gapped().await;
        // u2's oldest retained version is 101 (versions 1..100 are absent,
        // modelling pruned/aged-out history), so a cursor pointing before that
        // window is expired.
        let res = sync_pull(
            State(st),
            Query(PullQuery { cursor: 50, limit: Some(100) }),
            Bearer(tok),
        )
        .await;
        match res {
            Err(e) => assert_eq!(e.status, StatusCode::GONE),
            Ok(_) => panic!("expected cursor_expired"),
        }
    }

    #[tokio::test]
    async fn cursor_within_window_succeeds_after_pruning() {
        let (st, tok) = seed_gapped().await;
        // Cursor 200 is within the retained window (min version 101).
        let resp = sync_pull(
            State(st),
            Query(PullQuery { cursor: 200, limit: Some(100) }),
            Bearer(tok),
        )
        .await
        .unwrap()
        .0;
        assert!(resp.items.len() > 0);
    }
}
