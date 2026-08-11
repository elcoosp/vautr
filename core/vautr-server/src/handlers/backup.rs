//! Backup HTTP handler stubs (Wave A: group A5).
//! Docs: `mlp-wave-plan.md` §3 A5, `mlp-scope.md` §1.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here backed by `vautr-backup` (export + one-click
//! restore test). Until then this module intentionally does not import any
//! `vautr-domain` types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use super::AppState;

/// Return a JSON 501 "not implemented" envelope for the given route.
async fn not_implemented(route: &str) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "not_implemented",
            "message": format!("{route} is not yet implemented (Wave A)."),
        })),
    )
        .into_response()
}

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/backup", get(backup_status))
        .route("/backup/export", post(export_backup))
        .route("/backup/restore", post(restore_backup))
}

/// Stub: report backup status.
async fn backup_status() -> Response {
    not_implemented("GET /backup").await
}

/// Stub: export a backup archive.
async fn export_backup() -> Response {
    not_implemented("POST /backup/export").await
}

/// Stub: restore from a backup (one-click restore test).
async fn restore_backup() -> Response {
    not_implemented("POST /backup/restore").await
}
