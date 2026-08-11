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
//! When readiness fails, a **server-down** alert is emitted (structured
//! `tracing::error!` log/event) after a cooldown between repeats. The full
//! server-monitoring and webhook-hook primitives live in the `vautr-telemetry`
//! crate (`monitoring` module). The server crate's manifest is frozen for
//! Wave A (it cannot depend on `vautr-telemetry` yet), so this module keeps a
//! small self-contained registry and emits the log/event half directly; the
//! `vautr-telemetry::monitoring::Alerting` (with its optional `WebhookDeliverer`)
//! is the library-grade equivalent for post-integration wiring.
//!
//! ## No-PII rule (telemetry spec §1)
//! All reported fields are integer counters, durations, or fixed status strings.
//! Nothing here can carry a title, username, URL, note, or secret.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use sqlx::Executor;

use crate::handlers::AppState;
use crate::repository::Repository;

/// Cooldown between consecutive server-down alert emissions (5 min).
const ALERT_COOLDOWN: Duration = Duration::from_secs(300);
/// Consecutive readiness failures required before a server-down alert fires.
const ALERT_THRESHOLD: u64 = 2;

/// Process-wide health/metrics state (uptime clock + counters).
///
/// A single shared instance gives a stable "process uptime" across the whole
/// server, matching the liveness semantics of `GET /health`.
struct HealthState {
    started_at: Instant,
    total_requests: AtomicU64,
    ok_requests: AtomicU64,
    server_errors: AtomicU64,
    db_failures: AtomicU64,
    consecutive_failures: AtomicU64,
    last_alert_at: AtomicU64,
}

impl HealthState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total_requests: AtomicU64::new(0),
            ok_requests: AtomicU64::new(0),
            server_errors: AtomicU64::new(0),
            db_failures: AtomicU64::new(0),
            consecutive_failures: AtomicU64::new(0),
            last_alert_at: AtomicU64::new(0),
        }
    }

    fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

fn state() -> &'static HealthState {
    static STATE: OnceLock<HealthState> = OnceLock::new();
    STATE.get_or_init(HealthState::new)
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
    let hs = state();
    hs.total_requests.fetch_add(1, Ordering::Relaxed);
    hs.ok_requests.fetch_add(1, Ordering::Relaxed);
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uptime_secs": hs.uptime_secs(),
        })),
    )
        .into_response()
}

/// Readiness probe (`GET /health/ready`): reflects real DB connectivity.
pub async fn readiness(State(app): State<AppState>) -> Response {
    let hs = state();
    hs.total_requests.fetch_add(1, Ordering::Relaxed);

    let healthy = db_healthy(&app.repo).await;
    if healthy {
        hs.ok_requests.fetch_add(1, Ordering::Relaxed);
        hs.consecutive_failures.store(0, Ordering::Relaxed);
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "db": "up",
                "uptime_secs": hs.uptime_secs(),
                "requests": hs.total_requests.load(Ordering::Relaxed),
                "errors": hs.server_errors.load(Ordering::Relaxed),
            })),
        )
            .into_response()
    } else {
        hs.db_failures.fetch_add(1, Ordering::Relaxed);
        hs.server_errors.fetch_add(1, Ordering::Relaxed);
        let failures = hs.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        fire_server_down_alert_if_due(failures, hs);
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unavailable",
                "db": "down",
                "uptime_secs": hs.uptime_secs(),
                "consecutive_failures": failures,
            })),
        )
            .into_response()
    }
}

/// Basic in-process metrics (`GET /metrics`): requests, errors, DB failures,
/// uptime.
pub async fn metrics(State(_): State<AppState>) -> Response {
    let hs = state();
    (
        StatusCode::OK,
        Json(json!({
            "uptime_secs": hs.uptime_secs(),
            "total_requests": hs.total_requests.load(Ordering::Relaxed),
            "ok_requests": hs.ok_requests.load(Ordering::Relaxed),
            "server_errors": hs.server_errors.load(Ordering::Relaxed),
            "db_failures": hs.db_failures.load(Ordering::Relaxed),
            "consecutive_failures": hs.consecutive_failures.load(Ordering::Relaxed),
        })),
    )
        .into_response()
}

/// Emit a server-down alert (log/event) once the failure threshold is crossed
/// and the cooldown has elapsed since the last alert.
fn fire_server_down_alert_if_due(consecutive_failures: u64, hs: &HealthState) {
    if consecutive_failures < ALERT_THRESHOLD {
        return;
    }
    let now = unix_secs();
    let last = hs.last_alert_at.load(Ordering::Relaxed);
    if now.saturating_sub(last) < ALERT_COOLDOWN.as_secs() {
        return;
    }
    hs.last_alert_at.store(now, Ordering::Relaxed);
    tracing::error!(
        kind = "server_down",
        consecutive_failures = consecutive_failures,
        uptime_secs = hs.uptime_secs(),
        timestamp_unix_secs = now,
        "SERVER-DOWN alert: readiness check failing (database unreachable)"
    );
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
        let path = std::env::temp_dir().join(format!("vautr_health_down_{}.db", uuid::Uuid::new_v4()));
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
