//! Health, readiness, metrics and server-down alerting (Wave A6, MLP scope §1).
//!
//! Endpoints:
//! - `GET /health`       — **liveness**: the process is serving. Always 200.
//! - `GET /health/ready` — **readiness**: real DB connectivity check. 200 when
//!   the database answers `SELECT 1`, 503 `unavailable` when it does not.
//! - `GET /metrics`      — basic in-process counters (request counts, error
//!   counts, DB failures, uptime).
//!
//! ## Alerting
//! Readiness failures feed `vautr_telemetry::monitoring::Alerting`, which fires
//! a **server-down** alert (structured `tracing::error!`) after the configured
//! number of consecutive failures, repeating at most once per cooldown window.
//! The same primitives (`ServerMetrics`, `Alerting`) live in the `vautr-telemetry`
//! crate; this module is the server-side wiring into them.
//!
//! ## No-PII rule (telemetry spec §1)
//! All reported fields are integer counters, durations, or fixed status strings.
//! Nothing here can carry a title, username, URL, note, or secret.

use std::sync::{Mutex, OnceLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use vautr_telemetry::monitoring::{Alerting, HealthStatus, ServerMetrics};

use crate::handlers::AppState;
use crate::repository::Repository;

/// Process-wide server metrics (uptime clock + counters). A single shared
/// instance gives a stable "process uptime" across the whole server, matching
/// the liveness semantics of `GET /health`.
fn server_metrics() -> &'static ServerMetrics {
    static METRICS: OnceLock<ServerMetrics> = OnceLock::new();
    METRICS.get_or_init(ServerMetrics::new)
}

/// Process-wide server-down alert coordinator. Defaults: fires after 2
/// consecutive readiness failures, cooldown 5 minutes (see telemetry docs).
fn alerting() -> &'static Mutex<Alerting> {
    static ALERTING: OnceLock<Mutex<Alerting>> = OnceLock::new();
    ALERTING.get_or_init(|| Mutex::new(Alerting::new()))
}

/// Real DB connectivity check: acquire a connection and run `SELECT 1`.
async fn db_healthy(repo: &Repository) -> bool {
    let mut conn = match repo.pool().acquire().await {
        Ok(conn) => conn,
        Err(_) => return false,
    };
    sqlx::query("SELECT 1").execute(&mut *conn).await.is_ok()
}

/// Liveness probe (`GET /health`): reports the process is serving.
pub async fn liveness(State(_): State<AppState>) -> Response {
    let m = server_metrics();
    m.record_ok();
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uptime_secs": m.uptime_secs(),
        })),
    )
        .into_response()
}

/// Readiness probe (`GET /health/ready`): reflects real DB connectivity.
pub async fn readiness(State(app): State<AppState>) -> Response {
    let m = server_metrics();

    if db_healthy(&app.repo).await {
        m.record_ok();
        let _ = alerting().lock().unwrap().evaluate(HealthStatus::Up);
        let snap = m.snapshot();
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "db": "up",
                "uptime_secs": snap.uptime_secs,
                "requests": snap.total_requests,
                "errors": snap.error_count,
            })),
        )
            .into_response()
    } else {
        m.record_db_failure();
        m.record_server_error();
        let mut alert = alerting().lock().unwrap();
        let _event = alert.evaluate(HealthStatus::Down);
        let consecutive_failures = alert.consecutive_failures();
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unavailable",
                "db": "down",
                "uptime_secs": m.uptime_secs(),
                "consecutive_failures": consecutive_failures,
            })),
        )
            .into_response()
    }
}

/// Basic in-process metrics (`GET /metrics`): requests, errors, DB failures,
/// uptime.
pub async fn metrics(State(_): State<AppState>) -> Response {
    let m = server_metrics();
    let snap = m.snapshot();
    let consecutive_failures = alerting().lock().unwrap().consecutive_failures();
    (
        StatusCode::OK,
        Json(json!({
            "uptime_secs": snap.uptime_secs,
            "total_requests": snap.total_requests,
            "ok_requests": snap.ok_requests,
            "server_errors": snap.error_count,
            "db_failures": snap.db_failures,
            "consecutive_failures": consecutive_failures,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Method, Request, StatusCode};
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_app() -> axum::Router {
        let path = std::env::temp_dir().join(format!("vautr_health_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        crate::handlers::build_router(AppState::new(Arc::new(Repository::new(pool))))
    }

    async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }

    #[tokio::test]
    async fn liveness_returns_200() {
        let app = test_app().await;
        let (status, body) = get(&app, "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert!(body["uptime_secs"].as_u64().is_some());
    }

    #[tokio::test]
    async fn readiness_succeeds_when_db_up() {
        let app = test_app().await;
        let (status, body) = get(&app, "/health/ready").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ready");
        assert_eq!(body["db"], "up");
    }

    #[tokio::test]
    async fn readiness_fails_when_db_down() {
        // Build the state separately so we can close its pool to simulate a
        // real database outage.
        let path =
            std::env::temp_dir().join(format!("vautr_health_down_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let state = AppState::new(Arc::new(Repository::new(pool.clone())));
        let app = crate::handlers::build_router(state.clone());

        // Simulate the DB going down: close the connection pool so acquire fails.
        state.repo.pool().close().await;

        let (status, body) = get(&app, "/health/ready").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unavailable");
        assert_eq!(body["db"], "down");
    }

    #[tokio::test]
    async fn metrics_reports_counters() {
        let app = test_app().await;
        let (status, body) = get(&app, "/metrics").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["uptime_secs"].as_u64().is_some());
        assert!(body["total_requests"].as_u64().is_some());
        assert!(body["server_errors"].as_u64().is_some());
        assert!(body["db_failures"].as_u64().is_some());
    }
}
