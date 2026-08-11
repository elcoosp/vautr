//! MFA HTTP handler stubs (Wave A: group A4).
//! Docs: `mlp-wave-plan.md` §3 A4, `mlp-scope.md` §3/§5.
//!
//! These are compile-safe, not-yet-implemented stubs that return HTTP 501.
//! Wave A wires real logic here (TOTP issue/verify, WebAuthn enforcement,
//! master-password policy) against the Wave 0.1 domain model. Until then this
//! module intentionally does not import any `vautr-domain` types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
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
        .route("/mfa/totp/issue", post(issue_totp))
        .route("/mfa/totp/verify", post(verify_totp))
}

/// Stub: begin TOTP enrollment for the caller.
async fn issue_totp() -> Response {
    not_implemented("POST /mfa/totp/issue").await
}

/// Stub: verify a TOTP code to complete enrollment/login.
async fn verify_totp() -> Response {
    not_implemented("POST /mfa/totp/verify").await
}
