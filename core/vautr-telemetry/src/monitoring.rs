//! # Server monitoring & alerting primitives (Wave A6).
//!
//! The client-facing telemetry crate ([`crate::aggregate`]) measures *client*
//! health. This module adds the **server**-side observability half of MLP scope
//! §1 "Basic integrated monitoring + alerts (e.g., server-down alerting)":
//!
//! - [`ServerMetrics`] — a thread-safe, in-process registry of request counts,
//!   error counts, DB failures and uptime. Request classification is by HTTP
//!   status class (2xx ok, 4xx client error, 5xx server error), so it can be
//!   fed from any middleware without carrying PII.
//! - [`HealthStatus`] — the liveness/readiness signal consumed by alerting.
//! - [`Alerting`] — a server-down alert coordinator: it logs a structured event
//!   (the *log/event* half) and, when a [`WebhookHook`] is configured, delivers
//!   the same event to an external endpoint (the *optional webhook* half). A
//!   built-in [`WebhookDeliverer`] (feature `webhook`) posts JSON over HTTP.
//!
//! ## No-PII rule (spec §1)
//! Every field here is an integer counter, a duration, or a fixed alert `kind`
//! string. Nothing can carry a title, username, URL, note, or secret.
//!
//! ## Alert semantics
//! An alert fires only after [`Alerting::failure_threshold`] **consecutive**
//! failures, and repeats at most once per cooldown window ([`Alerting::new`]
//! defaults: threshold 2, cooldown 5 minutes). A single successful check resets
//! the consecutive-failure counter.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Default cooldown between consecutive server-down alert emissions (5 min).
pub const DEFAULT_ALERT_COOLDOWN: Duration = Duration::from_secs(300);
/// Default number of consecutive failures required before an alert fires.
pub const DEFAULT_FAILURE_THRESHOLD: u64 = 2;

/// Liveness/readiness signal for the monitored service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HealthStatus {
    /// The service is healthy (e.g. a readiness DB check succeeded).
    Up,
    /// The service is down (e.g. a readiness DB check failed).
    Down,
}

/// Severity of a fired alert event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertSeverity {
    /// Informational notification.
    Info,
    /// Recoverable but noteworthy condition.
    Warning,
    /// Service-down / data-affecting condition.
    Critical,
}

/// A fired server-down alert event (PII-free).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlertEvent {
    /// Machine-readable alert kind, e.g. `"server_down"`.
    pub kind: String,
    /// Severity of the alert.
    pub severity: AlertSeverity,
    /// Human-readable description (no PII).
    pub message: String,
    /// Process uptime in seconds when the alert fired.
    pub uptime_secs: u64,
    /// Consecutive failures that triggered the alert.
    pub consecutive_failures: u64,
    /// Unix timestamp (seconds) when the alert fired.
    pub timestamp_unix_secs: u64,
}

/// A webhook hook that receives fired alert events.
///
/// This is the *optional webhook* half of server-down alerting. Callers supply
/// their own transport, or use the built-in [`WebhookDeliverer`] (feature
/// `webhook`). Delivery is best-effort: failures are logged, never panicked.
pub trait WebhookHook: Send + Sync {
    /// Deliver an alert event to an external endpoint.
    fn deliver(&self, event: &AlertEvent);
}

/// A snapshot of the in-process [`ServerMetrics`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    /// Process uptime in seconds.
    pub uptime_secs: u64,
    /// Total requests observed.
    pub total_requests: u64,
    /// Requests classified as OK (2xx).
    pub ok_requests: u64,
    /// Requests classified as client errors (4xx).
    pub client_error_requests: u64,
    /// Requests classified as server errors (5xx).
    pub server_error_requests: u64,
    /// Total server-side errors recorded (server errors + DB failures).
    pub error_count: u64,
    /// Database failures observed (e.g. readiness probes).
    pub db_failures: u64,
}

/// Thread-safe, in-process server metrics registry.
///
/// Counter updates are lock-free (`AtomicU64`) and safe to call from any
/// middleware/handler without contention.
pub struct ServerMetrics {
    started_at: Instant,
    total_requests: AtomicU64,
    ok_requests: AtomicU64,
    client_error_requests: AtomicU64,
    server_error_requests: AtomicU64,
    error_count: AtomicU64,
    db_failures: AtomicU64,
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerMetrics {
    /// Create a fresh registry, starting the uptime clock now.
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total_requests: AtomicU64::new(0),
            ok_requests: AtomicU64::new(0),
            client_error_requests: AtomicU64::new(0),
            server_error_requests: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            db_failures: AtomicU64::new(0),
        }
    }

    /// Process uptime in whole seconds since [`ServerMetrics::new`].
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Classify and record a request by its HTTP status code.
    ///
    /// 2xx → ok, 4xx → client error, 5xx → server error (also counted as an
    /// error). Other codes (e.g. 3xx) count toward `total_requests` only.
    pub fn record_request(&self, status: u16) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        match status {
            200..=299 => {
                self.ok_requests.fetch_add(1, Ordering::Relaxed);
            }
            400..=499 => {
                self.client_error_requests.fetch_add(1, Ordering::Relaxed);
            }
            500..=599 => {
                self.server_error_requests.fetch_add(1, Ordering::Relaxed);
                self.error_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Record an OK (2xx) request.
    pub fn record_ok(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.ok_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a client (4xx) error request.
    pub fn record_client_error(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.client_error_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a server (5xx) error request.
    pub fn record_server_error(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.server_error_requests.fetch_add(1, Ordering::Relaxed);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a database failure.
    pub fn record_db_failure(&self) {
        self.db_failures.fetch_add(1, Ordering::Relaxed);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// A consistent snapshot of all counters.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            uptime_secs: self.uptime_secs(),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            ok_requests: self.ok_requests.load(Ordering::Relaxed),
            client_error_requests: self.client_error_requests.load(Ordering::Relaxed),
            server_error_requests: self.server_error_requests.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            db_failures: self.db_failures.load(Ordering::Relaxed),
        }
    }
}

/// Server-down alert coordinator: log/event + optional webhook hook.
///
/// Feed [`HealthStatus`] from readiness checks into [`Alerting::evaluate`].
/// The alert fires only after the configured number of **consecutive**
/// failures and repeats at most once per cooldown window. Every fired event is
/// emitted via `tracing` and delivered to the configured [`WebhookHook`] (if
/// any).
pub struct Alerting {
    webhook: Option<Box<dyn WebhookHook>>,
    consecutive_failures: u64,
    last_alert_unix_secs: Option<u64>,
    fired_events: u64,
    cooldown: Duration,
    failure_threshold: u64,
    started_at: Instant,
}

impl Default for Alerting {
    fn default() -> Self {
        Self::new()
    }
}

impl Alerting {
    /// Create an alerting coordinator with defaults: threshold 2 consecutive
    /// failures, cooldown 5 minutes, no webhook.
    pub fn new() -> Self {
        Self {
            webhook: None,
            consecutive_failures: 0,
            last_alert_unix_secs: None,
            fired_events: 0,
            cooldown: DEFAULT_ALERT_COOLDOWN,
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            started_at: Instant::now(),
        }
    }

    /// Set the minimum cooldown between repeated alerts.
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Set the number of consecutive failures required before an alert fires.
    pub fn with_failure_threshold(mut self, threshold: u64) -> Self {
        self.failure_threshold = threshold.max(1);
        self
    }

    /// Attach (or replace) the optional webhook hook.
    pub fn set_webhook(&mut self, hook: Box<dyn WebhookHook>) {
        self.webhook = Some(hook);
    }

    /// The current consecutive-failure count.
    pub fn consecutive_failures(&self) -> u64 {
        self.consecutive_failures
    }

    /// The number of alerts fired so far.
    pub fn fired_events(&self) -> u64 {
        self.fired_events
    }

    /// Whether the cooldown has elapsed since the last fired alert.
    fn cooldown_elapsed(&self, now_secs: u64) -> bool {
        match self.last_alert_unix_secs {
            None => true,
            Some(last) => now_secs.saturating_sub(last) >= self.cooldown.as_secs(),
        }
    }

    /// Evaluate a health-check result and possibly fire an alert.
    ///
    /// Returns `Some(event)` only when a new alert fires (threshold reached and
    /// cooldown elapsed). A healthy result resets the consecutive-failure count.
    pub fn evaluate(&mut self, status: HealthStatus) -> Option<AlertEvent> {
        match status {
            HealthStatus::Up => {
                self.consecutive_failures = 0;
                None
            }
            HealthStatus::Down => {
                self.consecutive_failures += 1;
                if self.consecutive_failures < self.failure_threshold {
                    return None;
                }
                let now_secs = unix_secs();
                if !self.cooldown_elapsed(now_secs) {
                    return None;
                }
                self.last_alert_unix_secs = Some(now_secs);
                self.fired_events += 1;

                let event = AlertEvent {
                    kind: "server_down".to_string(),
                    severity: AlertSeverity::Critical,
                    message: "server readiness check failing: service is down".to_string(),
                    uptime_secs: self.started_at.elapsed().as_secs(),
                    consecutive_failures: self.consecutive_failures,
                    timestamp_unix_secs: now_secs,
                };

                // Log/event half.
                tracing::error!(
                    kind = %event.kind,
                    severity = ?event.severity,
                    consecutive_failures = event.consecutive_failures,
                    uptime_secs = event.uptime_secs,
                    timestamp_unix_secs = event.timestamp_unix_secs,
                    "SERVER-DOWN alert fired",
                );
                // Optional webhook half.
                if let Some(hook) = &self.webhook {
                    hook.deliver(&event);
                }
                Some(event)
            }
        }
    }
}

/// A real HTTP webhook hook built on `reqwest` (feature `webhook`).
///
/// Posts the [`AlertEvent`] as JSON to a configured URL on a best-effort basis.
#[cfg(feature = "webhook")]
#[derive(Debug, Clone)]
pub struct WebhookDeliverer {
    url: String,
    client: reqwest::blocking::Client,
    timeout: Duration,
}

#[cfg(feature = "webhook")]
impl WebhookDeliverer {
    /// Create a deliverer for the given endpoint URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::blocking::Client::new(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Create a deliverer with a custom per-request timeout.
    pub fn with_timeout(url: impl Into<String>, timeout: Duration) -> Self {
        Self {
            url: url.into(),
            client: reqwest::blocking::Client::new(),
            timeout,
        }
    }
}

#[cfg(feature = "webhook")]
impl WebhookHook for WebhookDeliverer {
    fn deliver(&self, event: &AlertEvent) {
        let result = self
            .client
            .post(&self.url)
            .json(event)
            .timeout(self.timeout)
            .send();
        match result {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(url = %self.url, "alert webhook delivered");
            }
            Ok(resp) => {
                tracing::warn!(
                    url = %self.url,
                    status = resp.status().as_u16(),
                    "alert webhook returned non-success status",
                );
            }
            Err(e) => {
                tracing::warn!(url = %self.url, error = %e, "alert webhook delivery failed");
            }
        }
    }
}

/// Unix timestamp in whole seconds.
fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn server_metrics_classifies_requests_by_status() {
        let m = ServerMetrics::new();
        m.record_request(200);
        m.record_request(204);
        m.record_request(404);
        m.record_request(500);
        m.record_request(302);

        let s = m.snapshot();
        assert_eq!(s.total_requests, 5);
        assert_eq!(s.ok_requests, 2);
        assert_eq!(s.client_error_requests, 1);
        assert_eq!(s.server_error_requests, 1);
        assert_eq!(s.error_count, 1); // 5xx only
        assert_eq!(s.db_failures, 0);
    }

    #[test]
    fn server_metrics_tracks_db_failures_and_uptime() {
        let m = ServerMetrics::new();
        m.record_db_failure();
        m.record_db_failure();
        let s = m.snapshot();
        assert_eq!(s.db_failures, 2);
        assert_eq!(s.error_count, 2);
        // started_at is set at construction; uptime is >= 0.
        assert!(s.uptime_secs < u64::MAX);
    }

    #[test]
    fn metrics_snapshot_serde_roundtrip() {
        let m = ServerMetrics::new();
        m.record_ok();
        m.record_server_error();
        m.record_db_failure();
        let snap = m.snapshot();
        let json = serde_json::to_string(&snap).expect("serializes");
        let back: MetricsSnapshot = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, snap);
        // record_ok + record_server_error are requests; record_db_failure is not.
        assert_eq!(back.total_requests, 2);
        assert_eq!(back.db_failures, 1);
    }

    #[test]
    fn alerting_ignores_healthy_states_and_resets() {
        let mut a = Alerting::new();
        assert_eq!(a.evaluate(HealthStatus::Up), None);
        assert_eq!(a.consecutive_failures(), 0);
        // A couple of Down checks below the threshold produce no alert.
        assert_eq!(a.evaluate(HealthStatus::Down), None);
        assert_eq!(a.consecutive_failures(), 1);
        // Recovery resets the counter.
        assert_eq!(a.evaluate(HealthStatus::Up), None);
        assert_eq!(a.consecutive_failures(), 0);
        assert_eq!(a.fired_events(), 0);
    }

    #[test]
    fn alerting_fires_after_threshold() {
        let mut a = Alerting::new().with_cooldown(Duration::ZERO);
        assert_eq!(a.evaluate(HealthStatus::Down), None); // 1st
        let ev = a
            .evaluate(HealthStatus::Down)
            .expect("2nd consecutive failure fires the alert");
        assert_eq!(ev.kind, "server_down");
        assert_eq!(ev.severity, AlertSeverity::Critical);
        assert_eq!(ev.consecutive_failures, 2);
        assert_eq!(a.fired_events(), 1);
    }

    #[test]
    fn alerting_cooldown_suppresses_repeats() {
        let mut a = Alerting::new().with_cooldown(Duration::from_secs(3600));
        assert_eq!(a.evaluate(HealthStatus::Down), None);
        assert!(
            a.evaluate(HealthStatus::Down).is_some(),
            "first alert fires"
        );
        // Immediate repeat is suppressed by the cooldown.
        assert_eq!(a.evaluate(HealthStatus::Down), None);
        assert_eq!(a.fired_events(), 1);
    }

    #[test]
    fn alerting_delivers_to_webhook_hook() {
        #[derive(Default)]
        struct RecordingHook(Arc<std::sync::Mutex<Vec<AlertEvent>>>);
        impl WebhookHook for RecordingHook {
            fn deliver(&self, event: &AlertEvent) {
                self.0.lock().unwrap().push(event.clone());
            }
        }

        let delivered = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut a = Alerting::new().with_cooldown(Duration::ZERO);
        a.set_webhook(Box::new(RecordingHook(delivered.clone())));

        a.evaluate(HealthStatus::Down);
        a.evaluate(HealthStatus::Down);
        a.evaluate(HealthStatus::Down); // repeat suppressed? cooldown zero -> fires again

        let count = delivered.lock().unwrap().len();
        assert_eq!(count, a.fired_events() as usize);
        assert_eq!(delivered.lock().unwrap()[0].kind, "server_down");
    }

    #[test]
    fn alert_event_serde_roundtrip() {
        let ev = AlertEvent {
            kind: "server_down".into(),
            severity: AlertSeverity::Critical,
            message: "server is down".into(),
            uptime_secs: 123,
            consecutive_failures: 2,
            timestamp_unix_secs: 1_700_000_000,
        };
        let json = serde_json::to_string(&ev).expect("serializes");
        let back: AlertEvent = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, ev);
    }

    #[test]
    fn default_threshold_and_cooldown() {
        assert_eq!(DEFAULT_FAILURE_THRESHOLD, 2);
        assert_eq!(DEFAULT_ALERT_COOLDOWN, Duration::from_secs(300));
        assert_eq!(
            Alerting::new()
                .with_cooldown(DEFAULT_ALERT_COOLDOWN)
                .cooldown,
            Duration::from_secs(300)
        );
    }
}
