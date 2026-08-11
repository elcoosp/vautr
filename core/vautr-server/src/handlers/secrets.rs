//! Secrets HTTP handler stubs (Wave A: group A3).
//! Docs: `mlp-wave-plan.md` §3 A3, `mlp-scope.md` §4.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here (same-Project secrets) against the Wave 0.1
//! domain model. Until then this module intentionally does not import any
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
        .route("/projects/{uuid}/secrets", get(list_secrets))
        .route("/secrets", post(create_secret))
}

/// Stub: list secrets within a project.
async fn list_secrets() -> Response {
    not_implemented("GET /projects/{uuid}/secrets").await
}

/// Stub: create a new secret.
async fn create_secret() -> Response {
    not_implemented("POST /secrets").await
}
