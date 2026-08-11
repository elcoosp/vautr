//! Projects HTTP handler stubs (Wave A: group A1).
//! Docs: `mlp-wave-plan.md` §3 A1, `mlp-scope.md` §2.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here against the Wave 0.1 Projects domain model
//! (org roles, per-project permissions, user groups, offboarding). Until then
//! this module intentionally does not import any `vautr-domain` types.

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
        .route("/projects", get(list_projects))
        .route("/projects", post(create_project))
}

/// Stub: list projects visible to the caller.
async fn list_projects() -> Response {
    not_implemented("GET /projects").await
}

/// Stub: create a new project.
async fn create_project() -> Response {
    not_implemented("POST /projects").await
}
