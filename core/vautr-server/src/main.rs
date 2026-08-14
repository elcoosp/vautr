//! Vautr server binary entrypoint.
//! Spawns the Axum app with middleware, SQLite WAL, OCC enforcement, and the
//! full OPAQUE auth + sync + account API (api.md §3–5).
//!
//! Graceful shutdown (VTR-054): on SIGTERM/SIGINT we stop accepting new
//! requests, let in-flight requests finish (bounded by a 30s timeout), flush
//! the SQLite WAL, close the connection pool, then exit.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::SqlitePool;
use tower_http::trace::TraceLayer;
use vautr_server::db;
use vautr_server::handlers::{build_router, AppState};
use vautr_server::middleware;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

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

    let repo = Arc::new(vautr_server::repository::Repository::new(pool.clone()));
    let state = AppState::new(repo.clone());

    // Server-side quarantine reaper (VTR-069): periodically pushes tombstone
    // events so web/extension clients drop stale items without waiting for sync.
    let _reaper = vautr_server::handlers::events::spawn_reaper(repo, state.event_tx.clone());

    // Middleware stack (arch-design §3.3): tracing outermost, then rate
    // limiting, then CORS. Tracing never captures bodies (no-plaintext rule).
    let app = build_router(state)
        .layer(middleware::cors())
        .layer(middleware::rate_limiter())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind 0.0.0.0:8080");
    tracing::info!("vautr-server listening on :8080");

    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal());

    // Stop accepting, finish in-flight with a bounded timeout, then tear down.
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, server)
        .await
        .is_err()
    {
        tracing::warn!(
            "graceful shutdown did not drain in {:.0}s; forcing pool close",
            SHUTDOWN_TIMEOUT.as_secs()
        );
    }

    flush_wal(&pool).await;
    pool.close().await;
    tracing::info!("vautr-server shut down cleanly");
}

/// Resolves when SIGINT (Ctrl-C) or SIGTERM is received.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("SIGINT received, starting graceful shutdown"),
        _ = terminate => tracing::info!("SIGTERM received, starting graceful shutdown"),
    }
}

/// Flush and truncate the SQLite WAL (server-scaling.md §4.4) so no committed
/// writes are left in the WAL after shutdown.
async fn flush_wal(pool: &SqlitePool) {
    match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
    {
        Ok(_) => tracing::info!("SQLite WAL flushed"),
        Err(e) => tracing::warn!(error = %e, "wal_checkpoint(TRUNCATE) failed"),
    }
}
