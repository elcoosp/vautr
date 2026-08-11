//! Machine Accounts HTTP handler stubs (Wave A: group A2).
//! Docs: `mlp-wave-plan.md` §3 A2, `mlp-scope.md` §4.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here (expiry, revocation, fine-grained scopes) against
//! the Wave 0.1 domain model. Until then this module intentionally does not
//! import any `vautr-domain` types.

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
        .route("/machine-accounts", get(list_machine_accounts))
        .route("/machine-accounts", post(create_machine_account))
}

/// Stub: list machine accounts owned by the caller.
async fn list_machine_accounts() -> Response {
    not_implemented("GET /machine-accounts").await
}

/// Stub: provision a new machine account.
async fn create_machine_account() -> Response {
    not_implemented("POST /machine-accounts").await
}
