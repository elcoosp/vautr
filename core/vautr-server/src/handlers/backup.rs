//! Backup HTTP handlers — `GET /backup`, `POST /backup/export`,
//! `POST /backup/restore` (one-click restore test).
//! Spec: docs/architecture/mlp-scope.md §1, packages/api-contract/openapi.json
//! (tag `backup`). Backed by the `vautr-backup` crate.
//!
//! - `export` creates an **encrypted** archive of the live SQLite store
//!   (VACUUM INTO → seal with XChaCha20-Poly1305) and records it on disk.
//! - `restore` runs the one-click **restore test**: it decrypts an archive,
//!   writes it to a throwaway scratch DB, validates it, and discards it —
//!   **never touching the live store.**

use std::path::Path;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, Bearer, auth_user, now_ms};
use crate::repository::backup::{BackupRun, BackupState};

/// Stable logical vault id recorded in every archive manifest.
const VAULT_ID: &str = "vautr-main-vault";

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/backup", get(backup_status))
        .route("/backup/export", post(export_backup))
        .route("/backup/restore", post(restore_backup))
}

// ---------------------------------------------------------------------------
// Request / response types (mirror packages/api-contract/openapi.json)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct BackupStatus {
    enabled: bool,
    location: Option<String>,
    schedule: String,
    last_backup_at: Option<i64>,
    last_backup_size_bytes: Option<u64>,
    last_restore_test_at: Option<i64>,
    last_restore_test_status: Option<String>,
}

#[derive(Deserialize, Default)]
pub(crate) struct BackupExportRequest {
    include_secrets: Option<bool>,
}

#[derive(Serialize)]
pub(crate) struct BackupExportResponse {
    backup_id: String,
    download_url: Option<String>,
    size_bytes: u64,
    checksum: String,
    created_at: i64,
}

#[derive(Deserialize, Default)]
pub(crate) struct BackupRestoreRequest {
    backup_id: Option<String>,
    archive_base64: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct BackupRestoreResponse {
    status: String,
    test_id: String,
    restored_records: u64,
    restored_at: i64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /backup` — backup configuration and last-run state.
async fn backup_status(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<BackupStatus>, ApiError> {
    auth_user(&st.repo, &auth.0).await?;
    let s: BackupState = st
        .repo
        .backup_state()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(BackupStatus {
        enabled: s.enabled,
        location: s.location,
        schedule: s.schedule,
        last_backup_at: s.last_backup_at,
        last_backup_size_bytes: s.last_backup_size_bytes.map(|v| v as u64),
        last_restore_test_at: s.last_restore_test_at,
        last_restore_test_status: s.last_restore_test_status,
    }))
}

/// `POST /backup/export` — create an encrypted backup archive of the live store.
async fn export_backup(
    State(st): State<AppState>,
    auth: Bearer,
    body: Option<Json<BackupExportRequest>>,
) -> Result<Json<BackupExportResponse>, ApiError> {
    auth_user(&st.repo, &auth.0).await?;
    // include_secrets is accepted by the contract; in v1 the whole snapshot is
    // captured, so the flag is advisory and defaults to true.
    let _include_secrets = body.as_ref().and_then(|b| b.include_secrets).unwrap_or(true);

    let key = st
        .repo
        .get_or_create_backup_key()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let location = resolve_location(&st).await?;
    std::fs::create_dir_all(&location)
        .map_err(|e| ApiError::internal(&format!("create backup dir: {e}")))?;

    // 1. Consistent snapshot of the live store (VACUUM INTO → bytes).
    let snapshot = vautr_backup::backup::create_snapshot(st.repo.pool(), Path::new(&location))
        .await
        .map_err(|e| ApiError::internal(&format!("snapshot failed: {e}")))?;

    // 2. Entry count for the manifest (missing items table => 0).
    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_optional(st.repo.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .unwrap_or(0);

    // 3. Seal the snapshot + manifest into an encrypted archive.
    let meta = vautr_backup::ArchiveMeta::new(now_rfc3339(), VAULT_ID, entry_count as u64);
    let archive = vautr_backup::backup::create_archive(&key, &snapshot, &meta)
        .map_err(|e| ApiError::internal(&format!("archive creation failed: {e}")))?;

    let backup_id = uuid::Uuid::new_v4().to_string();
    let created_at = now_ms();
    let checksum = vautr_backup::archive::sha256_hex(&archive);
    let file_name = format!("backup-{backup_id}.vautr");
    let archive_path = Path::new(&location).join(&file_name);
    std::fs::write(&archive_path, &archive)
        .map_err(|e| ApiError::internal(&format!("write archive: {e}")))?;

    let run = BackupRun {
        id: backup_id.clone(),
        created_at,
        size_bytes: archive.len() as i64,
        checksum: checksum.clone(),
        archive_path: archive_path.display().to_string(),
        vault_id: VAULT_ID.to_string(),
        entry_count,
    };
    st.repo
        .record_backup_run(&run, created_at)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(Json(BackupExportResponse {
        backup_id,
        download_url: Some(format!("file://{}", archive_path.display())),
        size_bytes: archive.len() as u64,
        checksum,
        created_at,
    }))
}

/// `POST /backup/restore` — one-click restore test against an existing archive.
///
/// Restores to a scratch DB, validates it, and discards it. The live store is
/// never opened or modified. Returns `status: "success"` when the archive
/// decrypts and the restored DB passes integrity + entry checks.
async fn restore_backup(
    State(st): State<AppState>,
    auth: Bearer,
    Json(body): Json<BackupRestoreRequest>,
) -> Result<Json<BackupRestoreResponse>, ApiError> {
    auth_user(&st.repo, &auth.0).await?;

    // Resolve the archive bytes from the request.
    let archive = if let Some(archive_b64) = &body.archive_base64 {
        super::decode_b64(archive_b64)?
    } else if let Some(backup_id) = &body.backup_id {
        let run = st
            .repo
            .get_backup_run(backup_id)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("unknown_backup", "backup_id not found"))?;
        std::fs::read(&run.archive_path)
            .map_err(|e| ApiError::internal(&format!("read archive: {e}")))?
    } else {
        return Err(ApiError::bad_request(
            "missing_archive",
            "provide backup_id or archive_base64",
        ));
    };

    let key = st
        .repo
        .get_or_create_backup_key()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let outcome = vautr_backup::restore_test::run_restore_test(&key, &archive)
        .await
        .map_err(|e| ApiError::internal(&format!("restore test failed: {e}")))?;

    let status = if outcome.passed { "success" } else { "failed" };
    let at = now_ms();
    st.repo
        .record_restore_test(status, at)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(Json(BackupRestoreResponse {
        status: status.to_string(),
        test_id: uuid::Uuid::new_v4().to_string(),
        restored_records: outcome.restored_records,
        restored_at: at,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve where archive files are written: configured location first, then the
/// `VAUTR_BACKUP_DIR` env var, then `./backups`.
async fn resolve_location(st: &AppState) -> Result<String, ApiError> {
    let configured = st
        .repo
        .backup_state()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
        .location
        .filter(|l| !l.trim().is_empty());
    if let Some(l) = configured {
        return Ok(l);
    }
    if let Ok(l) = std::env::var("VAUTR_BACKUP_DIR") {
        if !l.trim().is_empty() {
            return Ok(l);
        }
    }
    Ok("./backups".to_string())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_state(backup_dir: &str) -> AppState {
        let path =
            std::env::temp_dir().join(format!("vautr_backup_http_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000;
        repo.create_user("u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        // Seed two items so the restore test can assert they come back intact.
        for (i, item_uuid) in ["item-1", "item-2"].iter().enumerate() {
            sqlx::query(
                "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
                 VALUES (?, ?, 1, 1, NULL, ?, ?)",
            )
            .bind(item_uuid)
            .bind("u1")
            .bind(&[i as u8 + 1; 8][..])
            .bind(now + i as i64)
            .execute(repo.pool())
            .await
            .unwrap();
        }
        repo.set_backup_location(backup_dir).await.unwrap();
        AppState::new(repo)
    }

    #[tokio::test]
    async fn export_then_restore_test_passes() {
        let backup_dir = std::env::temp_dir().join(format!("vautr_bk_dir_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&backup_dir).unwrap();
        let state = test_state(backup_dir.to_str().unwrap()).await;
        let app = routes().with_state(state.clone());

        // 1. Export a backup.
        let export = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/backup/export")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(export.status(), StatusCode::OK);
        let export_body = axum::body::to_bytes(export.into_body(), usize::MAX).await.unwrap();
        let export_json: serde_json::Value = serde_json::from_slice(&export_body).unwrap();
        let backup_id = export_json["backup_id"].as_str().unwrap().to_string();
        assert!(export_json["checksum"].as_str().unwrap().len() == 64);

        // 2. One-click restore test against that archive.
        let restore = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/backup/restore")
                    .header("authorization", "Bearer tok1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "backup_id": backup_id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(restore.status(), StatusCode::OK);
        let restore_body = axum::body::to_bytes(restore.into_body(), usize::MAX).await.unwrap();
        let restore_json: serde_json::Value = serde_json::from_slice(&restore_body).unwrap();
        assert_eq!(restore_json["status"], "success", "restore test must pass");
        assert_eq!(restore_json["restored_records"], 2);
        assert!(restore_json["test_id"].as_str().unwrap().len() > 0);

        // 3. Live store is untouched: items still present.
        let live_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(state.repo.pool())
            .await
            .unwrap();
        assert_eq!(live_count, 2, "live DB must not be modified by restore test");

        // 4. Status reflects the backup + restore test.
        let status = app
            .oneshot(
                Request::builder()
                    .uri("/backup")
                    .header("authorization", "Bearer tok1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = axum::body::to_bytes(status.into_body(), usize::MAX).await.unwrap();
        let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["enabled"], true);
        assert!(status_json["last_backup_at"].is_number());
        assert_eq!(status_json["last_restore_test_status"], "success");

        let _ = std::fs::remove_dir_all(&backup_dir);
    }

    #[tokio::test]
    async fn endpoints_require_auth() {
        let state = test_state("/tmp/vautr_bk_unused").await;
        let app = routes().with_state(state);
        for (method, uri) in [
            ("GET", "/backup"),
            ("POST", "/backup/export"),
            ("POST", "/backup/restore"),
        ] {
            let req = Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{method} {uri}");
        }
    }
}
