//! Recovery HTTP handlers — Wave B stub.
//! Spec: docs/architecture/emergency-recovery-account.md §2-4. Implemented by
//! the Wave B recovery agent; owns this file exclusively. Returns an empty
//! router until implemented.

use axum::Router;

use super::AppState;

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
}
