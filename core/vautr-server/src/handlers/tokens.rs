//! Access Token HTTP handler stubs (Wave A: group A2).
//! Docs: `mlp-wave-plan.md` §3 A2, `mlp-scope.md` §4.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here (token issue, expiry, revocation, scopes) against
//! the Wave 0.1 domain model. Until then this module intentionally does not
//! import any `vautr-domain` types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
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
        .route("/tokens", get(list_tokens))
        .route("/tokens", post(create_token))
        .route("/tokens/{uuid}", delete(revoke_token))
}

/// Stub: list access tokens issued to the caller.
async fn list_tokens() -> Response {
    not_implemented("GET /tokens").await
}

/// Stub: issue a new access token.
async fn create_token() -> Response {
    not_implemented("POST /tokens").await
}

/// Stub: revoke an access token by id.
async fn revoke_token() -> Response {
    not_implemented("DELETE /tokens/{uuid}").await
}
