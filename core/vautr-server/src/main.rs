//! Vautr server binary entrypoint.
//! Spawns the Axum app with middleware, SQLite WAL, OCC enforcement, and the
//! full OPAQUE auth + sync + account API (api.md §3–5).

use std::sync::Arc;

use vautr_server::db;
use vautr_server::handlers::{build_router, AppState};
use vautr_server::middleware;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db_url = std::env::var("VAUTR_DB_URL").unwrap_or_else(|_| "sqlite:vautr.db".into());
    let pool = db::connect(&db_url)
        .await
        .expect("failed to open server database / run migrations");
    tracing::info!(%db_url, "vautr-server database ready");

    let repo = Arc::new(vautr_server::repository::Repository::new(pool));
    let state = AppState::new(repo);

    let app = build_router(state).layer(middleware::layer());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("vautr-server listening on :8080");
    axum::serve(listener, app).await.unwrap();
}
