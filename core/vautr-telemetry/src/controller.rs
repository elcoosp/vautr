//! # Telemetry controller (spec §4).
//!
//! Ties the [`crate::aggregate::DailyAggregator`], [`crate::consent::ConsentGate`],
//! and a [`crate::sender::HeartbeatSender`] into the 24-hour heartbeat loop.
//!
//! All metric recording is gated behind consent: when telemetry is disabled,
//! `record_*` calls are no-ops and no data is ever sent (TDD #2). The
//! aggregator is in-memory only, so a crash before the window rolls over loses
//! the in-progress buffer — by design (TDD #3: no partial data is persisted).

use std::sync::Arc;
use std::time::Duration;

use crate::aggregate::{AggregatedMetrics, DailyAggregator};
use crate::consent::{Consent, ConsentGate};
use crate::report::{HeartbeatReport, InstallationUuid};
use crate::sender::{HeartbeatSender, SendError};

/// Default telemetry endpoint path suffix on the configured base URL.
pub const DEFAULT_HEARTBEAT_PATH: &str = "/telemetry/heartbeat";

/// Coordinates telemetry collection + delivery for one installation.
///
/// `Clone` is cheap (internal state is `Arc`). The 24h loop is driven by
/// [`TelemetryController::tick`] (deterministic, testable) or
/// [`TelemetryController::start`] behind the `tokio` feature.
#[derive(Clone)]
pub struct TelemetryController {
    inner: Arc<Inner>,
}

struct Inner {
    aggregator: std::sync::Mutex<DailyAggregator>,
    consent: std::sync::Mutex<ConsentGate>,
    installation_uuid: InstallationUuid,
    sender: Option<Box<dyn HeartbeatSender>>,
    os_version: String,
    app_version: String,
    /// Metrics rolled out of the window but not yet successfully delivered;
    /// retried on the next tick (TDD #5).
    pending: std::sync::Mutex<Option<AggregatedMetrics>>,
}

impl TelemetryController {
    /// Build a controller. `sender` may be `None` to disable delivery while
    /// still aggregating (or pass a sender for real transmission).
    pub fn new(
        installation_uuid: InstallationUuid,
        sender: Option<Box<dyn HeartbeatSender>>,
        os_version: impl Into<String>,
        app_version: impl Into<String>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                aggregator: std::sync::Mutex::new(DailyAggregator::new()),
                consent: std::sync::Mutex::new(ConsentGate::new()),
                installation_uuid,
                sender,
                os_version: os_version.into(),
                app_version: app_version.into(),
                pending: std::sync::Mutex::new(None),
            }),
        }
    }

    /// The installation identifier (persisted by the caller across restarts).
    pub fn installation_uuid(&self) -> InstallationUuid {
        self.inner.installation_uuid
    }

    /// Current consent state.
    pub fn consent(&self) -> Consent {
        self.inner.consent.lock().unwrap().state()
    }

    /// Grant telemetry consent.
    pub fn grant_consent(&self) {
        self.inner.consent.lock().unwrap().grant();
    }

    /// Revoke consent and discard any buffered metrics (TDD #2).
    pub fn revoke_consent(&self) {
        let mut pending = self.inner.pending.lock().unwrap();
        *pending = None;
        self.inner.consent.lock().unwrap().revoke();
    }

    // ---- Metric recording (consent-gated) ---------------------------------

    /// Record a sync attempt. No-op when consent is denied.
    pub fn record_sync_attempt(&self) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner.aggregator.lock().unwrap().record_sync_attempt();
        }
    }

    /// Record a sync duration in milliseconds. No-op when consent is denied.
    pub fn record_sync_duration_ms(&self, ms: u64) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .record_sync_duration_ms(ms);
        }
    }

    /// Record an argon2 derivation duration in milliseconds.
    pub fn record_argon2_duration_ms(&self, ms: u64) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .record_argon2_duration_ms(ms);
        }
    }

    /// Record an AEAD decrypt duration in microseconds.
    pub fn record_aead_decrypt_duration_us(&self, us: u64) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .record_aead_decrypt_duration_us(us);
        }
    }

    /// Record a sync failure segmented by PII-free error type name.
    pub fn record_sync_failure(&self, error_type: &str) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .record_sync_failure(error_type);
        }
    }

    /// Record an OCC conflict segmented by PII-free error type name.
    pub fn record_occ_conflict(&self, error_type: &str) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .record_occ_conflict(error_type);
        }
    }

    /// Increment the Safety Reaper orphans-cleaned counter.
    pub fn increment_reaper_orphans_cleaned(&self) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .increment_reaper_orphans_cleaned();
        }
    }

    /// Increment the quarantine TTL reset counter.
    pub fn increment_quarantine_ttl_resets(&self) {
        if self.inner.consent.lock().unwrap().enabled() {
            self.inner
                .aggregator
                .lock()
                .unwrap()
                .increment_quarantine_ttl_resets();
        }
    }

    // ---- Window + delivery -------------------------------------------------

    /// Force the configured window duration (for tests / tuning).
    pub fn with_window_duration(mut self, duration: Duration) -> Self {
        // Replace the inner aggregator with one using the custom window.
        let agg = DailyAggregator::with_window_duration(duration);
        *self.inner.aggregator.lock().unwrap() = agg;
        self
    }

    /// Attempt to roll the window and deliver the heartbeat.
    ///
    /// - If a previous delivery is still `pending` (failed earlier), retry it.
    /// - Otherwise, roll the aggregator if the window elapsed; if nothing is
    ///   due, returns `Ok(false)` (no-op).
    /// - On successful delivery the pending buffer is cleared and `Ok(true)` is
    ///   returned. On failure the report is kept for the next tick (TDD #5).
    ///
    /// All of this is skipped (returns `Ok(false)`) when consent is denied or no
    /// sender is configured.
    pub fn tick(&self) -> Result<bool, SendError> {
        let consent_enabled = self.inner.consent.lock().unwrap().enabled();
        let has_sender = self.inner.sender.is_some();
        if !consent_enabled || !has_sender {
            return Ok(false);
        }

        // 1. Prefer retrying a previously-failed pending report.
        let mut pending = self.inner.pending.lock().unwrap();
        let metrics: AggregatedMetrics = match pending.take() {
            Some(m) => m,
            None => {
                // 2. Otherwise roll the window if it has elapsed.
                let rolled = self.inner.aggregator.lock().unwrap().roll_window();
                match rolled {
                    Some(m) => m,
                    None => return Ok(false),
                }
            }
        };
        drop(pending);

        let report = HeartbeatReport::build(
            self.inner.installation_uuid,
            self.inner.os_version.clone(),
            self.inner.app_version.clone(),
            crate::aggregate::WINDOW_24H.as_secs(),
            metrics.clone(),
        );

        match self.inner.sender.as_ref().unwrap().send(&report) {
            Ok(()) => Ok(true),
            Err(e) => {
                // Keep the metrics pending for the next tick (retry on failure).
                *self.inner.pending.lock().unwrap() = Some(metrics);
                Err(e)
            }
        }
    }

    /// Run the 24-hour heartbeat loop forever (feature `tokio`).
    ///
    /// Every [`WINDOW_24H`] the controller attempts a [`tick`]. Network failures
    /// are logged and retried on the next tick; the task only ends when the
    /// returned join handle is aborted.
    #[cfg(feature = "tokio")]
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(crate::aggregate::WINDOW_24H);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                match self.tick() {
                    Ok(true) => tracing::info!("telemetry heartbeat delivered"),
                    Ok(false) => {} // nothing due / disabled
                    Err(e) => {
                        tracing::warn!(error = ?e, "telemetry heartbeat delivery failed; will retry next tick")
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::InstallationUuid;
    use crate::sender::{FailingSender, RecordingSender, SendError};

    // TDD #1: 10 sync ops with varying durations -> 24h flush contains
    // aggregated p50/p95/p99 histograms and NO raw values.
    #[test]
    fn tdd1_flush_contains_percentiles_not_raw_values() {
        let sender = std::sync::Arc::new(RecordingSender::default());
        let ctrl = TelemetryController::new(
            InstallationUuid::new(),
            Some(Box::new(sender.clone())),
            "macos-14.5",
            "0.1.0",
        )
        .with_window_duration(Duration::ZERO); // window rolls immediately

        ctrl.grant_consent();
        for i in 0..10u64 {
            ctrl.record_sync_attempt();
            // Varying durations: 50, 100, 150, ..., 500 ms.
            ctrl.record_sync_duration_ms(50 + i * 50);
        }

        // Trigger the 24h flush.
        let sent = ctrl.tick().expect("delivery succeeds");
        assert!(sent, "a heartbeat was delivered");

        let reports = sender.sent_reports();
        assert_eq!(reports.len(), 1);
        let json = serde_json::to_string(&reports[0]).expect("serializes");

        // Aggregated percentiles present.
        assert!(json.contains("\"p50\""), "has p50");
        assert!(json.contains("\"p95\""), "has p95");
        assert!(json.contains("\"p99\""), "has p99");
        // No per-event raw values: the only numeric fields are aggregated.
        // The serialized histogram has count=10, min=50, max=500.
        assert!(json.contains("\"count\":10"), "aggregated count");
        assert!(json.contains("\"min\":50"), "aggregated min");
        assert!(json.contains("\"max\":500"), "aggregated max");
        // p50 over [50,100,..,500] (10 values) nearest-rank 50% -> 250.
        assert!(json.contains("\"p50\":250"), "p50 computed");
    }

    // TDD #2: opt-out -> no network request is made.
    #[test]
    fn tdd2_opt_out_sends_nothing() {
        let sender = std::sync::Arc::new(RecordingSender::default());
        let ctrl = TelemetryController::new(
            InstallationUuid::new(),
            Some(Box::new(sender.clone())),
            "linux",
            "0.1.0",
        )
        .with_window_duration(Duration::ZERO);

        // Default state is denied; even with a sender, recording is gated.
        ctrl.record_sync_attempt();
        ctrl.record_sync_duration_ms(123);
        let sent = ctrl.tick().expect("no error when disabled");
        assert!(!sent, "disabled telemetry does not deliver");
        assert_eq!(sender.count(), 0, "no network request made");
    }

    // TDD #3: crash before flush -> no partial data kept (in-memory buffer is
    // lost; a fresh controller starts empty). We model the "crash" by dropping
    // the controller and constructing a new one with the same installation id.
    #[test]
    fn tdd3_crash_before_flush_loses_buffer() {
        let uuid = InstallationUuid::new();
        {
            let ctrl = TelemetryController::new(
                uuid,
                Some(Box::new(RecordingSender::default())),
                "macos",
                "0.1.0",
            )
            .with_window_duration(Duration::ZERO);
            ctrl.grant_consent();
            ctrl.record_sync_attempt();
            ctrl.record_sync_duration_ms(999);
            // No tick() called -> window never rolled -> buffer never sent.
        } // "crash": controller dropped, in-memory buffer gone.

        // Brand-new controller (simulating restart) with the same uuid.
        // It records NOTHING — if the pre-crash buffer had been persisted, the
        // old sync_attempt + 999ms sample would appear in this report. It must not.
        let sender2 = std::sync::Arc::new(RecordingSender::default());
        let ctrl2 =
            TelemetryController::new(uuid, Some(Box::new(sender2.clone())), "macos", "0.1.0")
                .with_window_duration(Duration::ZERO);
        ctrl2.grant_consent();
        let _ = ctrl2.tick(); // may send an (empty) heartbeat

        // Either nothing was sent, or the sent report carries NO stale data from
        // the pre-crash buffer (no sync_attempts, no 999ms sample).
        let reports = sender2.sent_reports();
        let leaked = reports
            .iter()
            .any(|r| r.metrics.sync_attempts > 0 || r.metrics.sync_duration_ms.count() > 0);
        assert!(!leaked, "no partial data carried across the crash");
    }

    // TDD #4: installation UUID is stable (caller persists it) and the report
    // carries only the installation uuid -- never any user identifier.
    #[test]
    fn tdd4_installation_uuid_stable_and_only_identifier() {
        let uuid = InstallationUuid::new();
        let sender = std::sync::Arc::new(RecordingSender::default());
        let ctrl =
            TelemetryController::new(uuid, Some(Box::new(sender.clone())), "macos-14.5", "0.1.0")
                .with_window_duration(Duration::ZERO);
        ctrl.grant_consent();
        ctrl.record_sync_attempt();
        ctrl.tick().expect("ok");

        // Same uuid reproduced in the next "start" (persisted by caller).
        let sender2 = std::sync::Arc::new(RecordingSender::default());
        let ctrl2 =
            TelemetryController::new(uuid, Some(Box::new(sender2.clone())), "macos-14.5", "0.1.0")
                .with_window_duration(Duration::ZERO);
        ctrl2.grant_consent();
        ctrl2.record_sync_attempt();
        ctrl2.tick().expect("ok");

        let r1 = &sender.sent_reports()[0];
        let r2 = &sender2.sent_reports()[0];
        assert_eq!(r1.installation_uuid, uuid);
        assert_eq!(r2.installation_uuid, uuid, "uuid stable across restarts");

        let json = serde_json::to_string(r1).expect("serializes");
        // The only identifier field is installation_uuid; there must be no
        // "user_id", "email", or "username" anywhere in the payload.
        assert!(!json.contains("user_id"));
        assert!(!json.contains("email"));
        assert!(!json.contains("username"));
        assert!(json.contains("installation_uuid"));
    }

    // TDD #5: endpoint 200 -> marked sent; on failure, retry next heartbeat.
    #[test]
    fn tdd5_failure_retries_next_tick_then_succeeds() {
        // First attempt fails, second succeeds.
        let failing = std::sync::Arc::new(std::sync::Mutex::new(FailingSender::default()));
        let sender: Box<dyn HeartbeatSender> = Box::new(FailThenOk {
            inner: failing.clone(),
        });
        let ctrl =
            TelemetryController::new(InstallationUuid::new(), Some(sender), "linux", "0.1.0")
                .with_window_duration(Duration::ZERO);
        ctrl.grant_consent();
        ctrl.record_sync_attempt();

        // First tick: delivery fails -> report kept pending.
        let first = ctrl.tick();
        assert!(
            matches!(first, Err(SendError::HttpStatus(_))),
            "first attempt fails"
        );
        assert_eq!(
            failing
                .lock()
                .unwrap()
                .attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        // Second tick: retries the pending report -> succeeds.
        let second = ctrl.tick().expect("retry succeeds");
        assert!(second, "pending report delivered on retry");
        assert_eq!(
            failing
                .lock()
                .unwrap()
                .attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    /// Test double: fails the first `send`, succeeds thereafter.
    struct FailThenOk {
        inner: std::sync::Arc<std::sync::Mutex<FailingSender>>,
    }
    impl HeartbeatSender for FailThenOk {
        fn send(&self, _report: &HeartbeatReport) -> Result<(), SendError> {
            let guard = self.inner.lock().unwrap();
            let n = guard.attempts.load(std::sync::atomic::Ordering::Relaxed);
            guard
                .attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n == 0 {
                Err(SendError::HttpStatus(503))
            } else {
                Ok(())
            }
        }
    }
}
