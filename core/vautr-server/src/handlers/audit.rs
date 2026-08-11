//! Audit-log HTTP handlers — Wave B stub.
//! Spec: docs/spec/verification.md §4.2 (NFR-SEC compliance), server-db.md.
//! Implemented by the Wave B hardening agent; owns this file exclusively.
//! Returns an empty router until implemented.

use axum::Router;

use super::AppState;

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
}
