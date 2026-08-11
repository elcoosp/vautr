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

use super::{ApiError, AppState, Bearer, auth_user, now_ms};

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
        .map_err(|e| ApiError::internal(&e.to_string()))?;

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

    st.repo
        .set_all_chunks_uploaded(&uuid, &user_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Repository;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use serde_json::json;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    /// Build an in-memory app state with one authed user and a valid session.
    async fn test_state() -> AppState {
        let pool = crate::db::connect("sqlite::memory:").await.expect("connect+migrate");
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
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("tok1")
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

        // 4. Complete -> Available, persisted to the manifest.
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
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("tok2")
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
