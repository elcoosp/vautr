//! Files (attachment) HTTP handlers — RustFS/S3 multipart upload gateway
//! (docs/architecture/file-storage.md §4).
//!
//! The server is a dumb, encrypted blob store: it never possesses the FEK. It
//! tracks file manifests + per-chunk state and hands the client presigned URLs
//! for chunk transfer. The upload and sync commit are strictly ordered so
//! manifests only become `Available` (and thus downloadable/syncable) after the
//! underlying object is fully realized (file-storage.md §4.1).
//!
//! Presigned URLs are deterministic mock URLs for now (real S3 presigning goes
//! behind a `rustfs` feature / env toggle). The goal is a correct, race-free
//! protocol shape with auth + ownership checks.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{auth_user, now_ms, ApiError, AppState, Bearer};

/// Deterministic mock presigned URLs for each chunk. Real S3 presigning will
/// replace this behind a `rustfs` feature / env toggle; the protocol shape is
/// unchanged.
fn presign_chunk_urls(file_uuid: &str, total_chunks: i64) -> Vec<String> {
    (0..total_chunks)
        .map(|i| format!("/internal/chunk/{file_uuid}/{i}"))
        .collect()
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct InitiateReq {
    total_size: u64,
    chunk_size: u32,
    total_chunks: u32,
    enc_key_gen: u64,
    content_type: String,
    last_modified: i64,
}

#[derive(Serialize)]
pub(crate) struct InitiateResp {
    file_uuid: String,
    upload_id: String,
    presigned_urls: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct StatusResp {
    file_uuid: String,
    status: String,
    uploaded_chunks: Vec<u32>,
    total_chunks: u32,
}

#[derive(Serialize)]
pub(crate) struct CompleteResp {
    file_uuid: String,
    status: String,
}

#[derive(Serialize)]
pub(crate) struct DownloadResp {
    file_uuid: String,
    presigned_urls: Vec<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /files/{file_uuid}/upload/initiate
/// Create a multipart session for a pending manifest. Returns an `upload_id`
/// and one presigned URL per chunk.
async fn upload_initiate(
    State(st): State<AppState>,
    Path(file_uuid): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<InitiateReq>,
) -> Result<Json<InitiateResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    // VTRFIX-SEC-H08: hard caps so a malicious client cannot allocate a
    // multi-billion-element Vec or a monstrous chunk-row loop.
    const MAX_CHUNKS: u32 = 512;
    const MAX_CHUNK_SIZE: u32 = 8 * 1024 * 1024;
    const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;
    if req.total_chunks == 0 || req.total_chunks > MAX_CHUNKS {
        return Err(ApiError::bad_request(
            "invalid_chunks",
            "total_chunks must be in 1..=512",
        ));
    }
    if req.chunk_size == 0 || req.chunk_size > MAX_CHUNK_SIZE {
        return Err(ApiError::bad_request(
            "invalid_chunk_size",
            "chunk_size must be in 1..=8MiB",
        ));
    }
    let computed_size = req.chunk_size as u64 * (req.total_chunks as u64);
    if computed_size > MAX_FILE_SIZE {
        return Err(ApiError::bad_request(
            "invalid_file_size",
            "chunk_size * total_chunks exceeds 256 MiB",
        ));
    }
    if req.total_size > MAX_FILE_SIZE {
        return Err(ApiError::bad_request(
            "invalid_file_size",
            "total_size exceeds 256 MiB",
        ));
    }

    // Quota: reject when this upload would exceed the per-user quota.
    let quota: u64 = std::env::var("VAUTR_FILE_QUOTA_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024 * 1024 * 1024);
    let used = st
        .repo
        .sum_file_bytes_owned_by(&user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if used.saturating_add(req.total_size as i64) > quota as i64 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "quota_exceeded",
            "file quota exceeded for this account",
        ));
    }

    // Re-initiation of an already-realized file is a protocol violation: the
    // manifest should only be synced after commit (§4.1 step 5).
    if let Some(existing) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    {
        if existing.status == "Available" {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "already_available",
                "file upload already completed",
            ));
        }
    }

    st.repo
        .upsert_file_manifest(
            &uuid,
            &user_id,
            req.total_size as i64,
            req.chunk_size as i64,
            req.total_chunks as i64,
            req.enc_key_gen as i64,
            &req.content_type,
            req.last_modified,
            now_ms(),
        )
        .await
        .map_err(|e| match e {
            // VTRFIX (MEDIUM): another tenant already owns this file_uuid (the
            // repository no longer re-points the row at a new owner). Surface a
            // 409 instead of silently taking the upload over.
            crate::repository::files::UpsertFileManifestError::OwnerConflict => ApiError::new(
                StatusCode::CONFLICT,
                "file_owner_conflict",
                "file already claimed by another owner",
            ),
            crate::repository::files::UpsertFileManifestError::Db(e) => {
                ApiError::internal(&e.to_string())
            }
        })?;

    let upload_id = Uuid::new_v4().to_string();
    let presigned_urls = presign_chunk_urls(&uuid, req.total_chunks as i64);
    Ok(Json(InitiateResp {
        file_uuid: uuid,
        upload_id,
        presigned_urls,
    }))
}

/// GET /files/{file_uuid}/upload/status
/// Report which chunks the object store received, so an interrupted upload can
/// resume from the first missing chunk (file-storage.md §4.1 step 3).
async fn upload_status(
    State(st): State<AppState>,
    Path(file_uuid): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    let Some(manifest) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "file not found",
        ));
    };

    let chunks = st
        .repo
        .get_chunk_statuses(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let uploaded_chunks = chunks
        .iter()
        .filter(|c| c.status == "uploaded")
        .map(|c| c.chunk_index as u32)
        .collect();

    Ok(Json(StatusResp {
        file_uuid: uuid,
        status: manifest.status,
        uploaded_chunks,
        total_chunks: manifest.total_chunks as u32,
    }))
}

/// POST /files/{file_uuid}/upload/complete
/// Atomically mark a pending manifest `Available` once all chunks are pushed.
/// Race-free: only one caller can flip `PendingUpload` -> `Available`; repeat
/// calls are idempotently acknowledged.
async fn upload_complete(
    State(st): State<AppState>,
    Path(file_uuid): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<CompleteResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    let Some(manifest) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "file not found",
        ));
    };

    if manifest.status == "Available" {
        return Ok(Json(CompleteResp {
            file_uuid: uuid,
            status: "Available".into(),
        }));
    }

    // VTRFIX (MEDIUM): completeness gate. `PendingUpload → Available` used to
    // happen unconditionally (and `set_all_chunks_uploaded` then marked every
    // — including never-received — chunk `uploaded`), so a client could commit
    // a truncated/corrupt upload. Only flip when every chunk row reports
    // `uploaded`.
    let uploaded = st
        .repo
        .count_chunks_uploaded(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if uploaded != manifest.total_chunks {
        return Err(ApiError::bad_request(
            "chunks_incomplete",
            &format!(
                "received {} of {} chunks; upload the remaining chunks before completing",
                uploaded, manifest.total_chunks
            ),
        ));
    }

    let flipped = st
        .repo
        .set_file_status(&uuid, &user_id, "PendingUpload", "Available")
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    if !flipped {
        // Lost a concurrent commit race: re-read; treat an already-Available
        // manifest as success (idempotent), anything else as a real error.
        let current = st
            .repo
            .get_file_manifest(&uuid, &user_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
        if let Some(cur) = current {
            if cur.status == "Available" {
                return Ok(Json(CompleteResp {
                    file_uuid: uuid,
                    status: "Available".into(),
                }));
            }
        }
        return Err(ApiError::internal("file in unexpected upload state"));
    }

    // All chunks were verified `uploaded` above; nothing to backfill here.
    // (The previous unconditional `set_all_chunks_uploaded` call was removed —
    // it marked un-received chunks as uploaded, masking data loss.)

    Ok(Json(CompleteResp {
        file_uuid: uuid,
        status: "Available".into(),
    }))
}

/// GET /files/{file_uuid}/download
/// Return presigned URLs for on-demand chunk download. Only fully realized
/// (`Available`) files can be downloaded (file-storage.md §4.2).
async fn download(
    State(st): State<AppState>,
    Path(file_uuid): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<DownloadResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    let Some(manifest) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "file not found",
        ));
    };

    if manifest.status != "Available" {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "not_available",
            "file not fully uploaded",
        ));
    }

    let presigned_urls = presign_chunk_urls(&uuid, manifest.total_chunks);
    Ok(Json(DownloadResp {
        file_uuid: uuid,
        presigned_urls,
    }))
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/files/{file_uuid}/upload/initiate", post(upload_initiate))
        .route("/files/{file_uuid}/upload/status", get(upload_status))
        .route("/files/{file_uuid}/upload/complete", post(upload_complete))
        .route("/files/{file_uuid}/download", get(download))
        // VTRFIX-FEAT-H01: real chunk transfer (replaces the mock presigned URLs).
        .route(
            "/files/{file_uuid}/chunks/{idx}",
            axum::routing::put(upload_chunk).get(download_chunk),
        )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Repository;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use serde_json::json;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    /// Build an in-memory app state with one authed user and a valid session.
    async fn test_state() -> AppState {
        let pool = crate::db::connect("sqlite::memory:")
            .await
            .expect("connect+migrate");
        let repo = Arc::new(Repository::new(pool));
        let now = now_ms();
        repo.create_user(
            "u1",
            "a@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .expect("create user");
        // Insert the session directly: `sessions` has a NOT NULL `created_at`
        // that the `store_session` helper does not populate.
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(crate::repository::sessions::hash_session_token("tok1"))
        .bind("u1")
        .bind(now + 60_000)
        .bind(now)
        .execute(repo.pool())
        .await
        .expect("store session");
        AppState::new(repo)
    }

    async fn call(
        router: &Router<()>,
        method: &str,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        let req = match body {
            Some(b) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = router.clone().oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1_048_576)
            .await
            .unwrap_or_default();
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    fn initiate_body(total_chunks: u32) -> serde_json::Value {
        json!({
            "total_size": total_chunks as u64 * 1_048_576,
            "chunk_size": 1_048_576,
            "total_chunks": total_chunks,
            "enc_key_gen": 1,
            "content_type": "application/octet-stream",
            "last_modified": 1_700_000_000_000_i64,
        })
    }

    #[tokio::test]
    async fn initiate_complete_marks_available() {
        let state = test_state().await;
        let repo = state.repo.clone();
        let router = super::routes().with_state(state);
        let fuuid = Uuid::new_v4().to_string();

        // 1. Initiate -> returns upload_id + one presigned URL per chunk.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(3)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let upload_id = json["upload_id"].as_str().expect("upload_id").to_string();
        assert!(!upload_id.is_empty());
        let urls = json["presigned_urls"].as_array().expect("presigned_urls");
        assert_eq!(urls.len(), 3);
        assert_eq!(
            urls[0].as_str().unwrap(),
            &format!("/internal/chunk/{fuuid}/0")
        );

        // 2. Status shows PendingUpload with no chunks yet uploaded.
        let (status, json) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/upload/status"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "PendingUpload");
        assert_eq!(json["uploaded_chunks"].as_array().unwrap().len(), 0);
        assert_eq!(json["total_chunks"], 3);

        // 3. Download is refused before the upload commits.
        let (status, _) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/download"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // 3b. VTRFIX: complete WITHOUT the chunks uploaded is rejected (400
        // `chunks_incomplete`) — the manifest must not flip prematurely.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "chunks_incomplete");
        assert_eq!(json["message"].as_str().unwrap().contains("0 of 3"), true);

        // 4. Upload every chunk, then complete -> Available, persisted.
        for idx in 0..3u32 {
            let (status, _) = call(
                &router,
                "PUT",
                &format!("/files/{fuuid}/chunks/{idx}"),
                "tok1",
                Some(json!({ "bytes": "Y2h1bms=" })),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "chunk {idx} upload");
        }
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "Available");

        let row = repo
            .get_file_manifest(&fuuid, "u1")
            .await
            .expect("repo")
            .expect("manifest exists");
        assert_eq!(row.status, "Available");

        // 5. Complete is idempotent; download now works; status shows all chunks.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "Available");

        let (status, json) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/download"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["presigned_urls"].as_array().unwrap().len(), 3);

        let (status, json) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/upload/status"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "Available");
        assert_eq!(json["uploaded_chunks"].as_array().unwrap().len(), 3);

        // 6. Re-initiate of an Available file is rejected (409).
        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(3)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    /// VTRFIX (MEDIUM): completing before all chunks arrive must fail with 400
    /// `chunks_incomplete` carrying received/total in the message.
    #[tokio::test]
    async fn complete_with_missing_chunks_rejected() {
        let state = test_state().await;
        let router = super::routes().with_state(state);
        let fuuid = Uuid::new_v4().to_string();

        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(4)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Only chunk 1 of 4 arrives.
        let (status, _) = call(
            &router,
            "PUT",
            &format!("/files/{fuuid}/chunks/1"),
            "tok1",
            Some(json!({ "bytes": "Y2h1bms=" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "chunks_incomplete");
        let msg = json["message"].as_str().unwrap();
        assert!(
            msg.contains("1 of 4"),
            "message must carry received/total: {msg}"
        );

        // Manifest is still PendingUpload; the remaining chunks complete it.
        let (status, _) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/download"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        for idx in [0u32, 2, 3] {
            let (status, _) = call(
                &router,
                "PUT",
                &format!("/files/{fuuid}/chunks/{idx}"),
                "tok1",
                Some(json!({ "bytes": "Y2h1bms=" })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "Available");
    }

    /// VTRFIX (MEDIUM): a second tenant re-initiating an in-flight upload must
    /// get 409 `file_owner_conflict`, not take the manifest over.
    #[tokio::test]
    async fn cross_tenant_reinitiate_conflicts() {
        let state = test_state().await;
        let now = now_ms();
        state
            .repo
            .create_user(
                "u2",
                "b@example.com",
                &[0u8; 32],
                &[1u8; 16],
                &[2u8; 48],
                &[3u8; 48],
                now,
            )
            .await
            .expect("create user 2");
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(crate::repository::sessions::hash_session_token("tok2"))
        .bind("u2")
        .bind(now + 60_000)
        .bind(now)
        .execute(state.repo.pool())
        .await
        .expect("session 2");
        let router = super::routes().with_state(state.clone());
        let fuuid = Uuid::new_v4().to_string();

        // u1 starts an in-flight upload.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(2)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // u2 re-initiating the same file_uuid is a takeover attempt -> 409.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok2",
            Some(initiate_body(2)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "file_owner_conflict");

        // u2 still cannot see or complete the file.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok2",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // The legitimate path still works: u1 re-initiates its OWN pending
        // upload (manifest reset, chunks re-armed).
        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(2)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // Ownership unchanged.
        let manifest = state
            .repo
            .get_file_manifest(&fuuid, "u1")
            .await
            .expect("repo")
            .expect("manifest still owned by u1");
        assert_eq!(manifest.status, "PendingUpload");
    }

    #[tokio::test]
    async fn ownership_is_isolated() {
        let state = test_state().await;
        let now = now_ms();
        state
            .repo
            .create_user(
                "u2",
                "b@example.com",
                &[0u8; 32],
                &[1u8; 16],
                &[2u8; 48],
                &[3u8; 48],
                now,
            )
            .await
            .expect("create user 2");
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(crate::repository::sessions::hash_session_token("tok2"))
        .bind("u2")
        .bind(now + 60_000)
        .bind(now)
        .execute(state.repo.pool())
        .await
        .expect("session 2");
        let router = super::routes().with_state(state);
        let fuuid = Uuid::new_v4().to_string();

        // u1 initiates.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/initiate"),
            "tok1",
            Some(initiate_body(2)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // u2 cannot see, mutate, or download u1's file.
        let (status, _) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/upload/status"),
            "tok2",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &router,
            "POST",
            &format!("/files/{fuuid}/upload/complete"),
            "tok2",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/download"),
            "tok2",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Unauthenticated request is rejected.
        let (status, _) = call(
            &router,
            "GET",
            &format!("/files/{fuuid}/upload/status"),
            "bogus",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

// ---------------------------------------------------------------------------
// VTRFIX-FEAT-H01: real chunk transfer
// ---------------------------------------------------------------------------

/// One-chunk upload body (base64 ciphertext). The client encrypts each chunk
/// with the per-file FEK; the server never sees plaintext.
#[derive(serde::Deserialize)]
pub(crate) struct UploadChunkReq {
    /// base64 of the AEAD-sealed chunk bytes.
    bytes: String,
}

/// `PUT /files/{file_uuid}/chunks/{idx}` — store one encrypted chunk and mark
/// its per-chunk row `uploaded`.
async fn upload_chunk(
    State(st): State<AppState>,
    Path((file_uuid, idx)): Path<(Uuid, u32)>,
    auth: Bearer,
    Json(req): Json<UploadChunkReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    // Caller must own the manifest.
    let Some(manifest) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "file not found",
        ));
    };
    if idx >= manifest.total_chunks as u32 {
        return Err(ApiError::bad_request(
            "invalid_chunk_index",
            "chunk index out of range",
        ));
    }
    let bytes = super::decode_b64(&req.bytes)?;
    let store = crate::repository::files::SqliteChunkStore::new(st.repo.pool().clone());
    use crate::repository::files::ChunkStore;
    store
        .put_chunk(&uuid, idx, bytes)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    st.repo
        .set_chunk_uploaded(&uuid, idx as i64, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(serde_json::json!({
        "file_uuid": uuid,
        "idx": idx,
        "status": "uploaded",
    })))
}

/// `GET /files/{file_uuid}/chunks/{idx}` — stream one encrypted chunk back.
async fn download_chunk(
    State(st): State<AppState>,
    Path((file_uuid, idx)): Path<(Uuid, u32)>,
    auth: Bearer,
) -> Result<axum::response::Response, ApiError> {
    use axum::body::Body;
    use axum::response::IntoResponse;

    let user_id = auth_user(&st.repo, &auth.0).await?;
    let uuid = file_uuid.to_string();

    let Some(manifest) = st
        .repo
        .get_file_manifest(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "file not found",
        ));
    };
    if manifest.status != "Available" {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "not_available",
            "file not fully uploaded",
        ));
    }
    let store = crate::repository::files::SqliteChunkStore::new(st.repo.pool().clone());
    use crate::repository::files::ChunkStore;
    let Some(bytes) = store
        .get_chunk(&uuid, idx)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "chunk_not_found",
            "chunk not stored",
        ));
    };
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/octet-stream".to_string(),
        )],
        Body::from(bytes),
    )
        .into_response())
}
