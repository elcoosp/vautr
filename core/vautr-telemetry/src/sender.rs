//! # Heartbeat delivery (spec §4.3).
//!
//! The 24-hour [`crate::aggregate::DailyAggregator`] produces an
//! [`crate::report::HeartbeatReport`]; this module defines how that report is
//! transmitted. Transmission is abstracted behind [`HeartbeatSender`] so the
//! controller can be tested without real network I/O, and so the concrete
//! transport (HTTP, or a platform-specific channel) is swappable.
//!
//! ## No-PII rule (spec §1)
//! The report itself contains only the non-PII [`InstallationUuid`], OS/app
//! versions, and aggregated metrics. This module transmits exactly that and
//! nothing else.

use crate::report::HeartbeatReport;

/// Error returned when a heartbeat cannot be delivered.
///
/// Delivery is best-effort: a failure here does **not** discard the report —
/// the controller retains it and retries on the next 24-hour tick (TDD #5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// The transport returned a non-success HTTP status.
    HttpStatus(u16),
    /// The transport failed to complete (DNS, TLS, connection, serialization).
    Transport(String),
}

/// A sink for [`HeartbeatReport`]s.
///
/// Implementors perform the actual transmission. The trait is intentionally
/// minimal so callers can inject a fake that records calls for tests (TDD #2:
/// opt-out ⇒ `send` is never invoked).
pub trait HeartbeatSender: Send + Sync {
    /// Deliver a single heartbeat report.
    fn send(&self, report: &HeartbeatReport) -> Result<(), SendError>;
}

/// Records every report handed to it. Useful as a test double and as a no-network
/// default that simply logs what *would* have been sent.
#[derive(Debug, Default)]
pub struct RecordingSender {
    sent: std::sync::Mutex<Vec<HeartbeatReport>>,
}

impl RecordingSender {
    /// All reports delivered so far, in order.
    pub fn sent_reports(&self) -> Vec<HeartbeatReport> {
        self.sent.lock().unwrap().clone()
    }

    /// Number of reports delivered so far.
    pub fn count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }
}

impl HeartbeatSender for RecordingSender {
    fn send(&self, report: &HeartbeatReport) -> Result<(), SendError> {
        self.sent.lock().unwrap().push(report.clone());
        Ok(())
    }
}

/// Blanket impl so an `Arc<dyn HeartbeatSender>` (or `Arc<ConcreteSender>`)
/// can be used wherever a sender is expected. Lets callers keep a handle to the
/// sender for inspection (e.g. counting delivered reports in tests) while still
/// handing ownership to the controller.
impl<T: HeartbeatSender + ?Sized> HeartbeatSender for std::sync::Arc<T> {
    fn send(&self, report: &HeartbeatReport) -> Result<(), SendError> {
        (**self).send(report)
    }
}

/// A sender that always fails — used to exercise the retry path (TDD #5).
#[derive(Debug, Default)]
pub struct FailingSender {
    /// Number of `send` attempts made against this sender.
    pub attempts: std::sync::atomic::AtomicU64,
    /// The status code reported on each failure.
    pub status: u16,
}

impl HeartbeatSender for FailingSender {
    fn send(&self, _report: &HeartbeatReport) -> Result<(), SendError> {
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Err(SendError::HttpStatus(self.status))
    }
}

/// An HTTP heartbeat sender (spec §4.3). Posts the report as JSON to the
/// configured endpoint. Enabled by the `http` feature.
#[cfg(feature = "http")]
pub struct HttpHeartbeatSender {
    url: String,
    client: reqwest::blocking::Client,
}

#[cfg(feature = "http")]
impl HttpHeartbeatSender {
    /// Create a sender targeting `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

#[cfg(feature = "http")]
impl HeartbeatSender for HttpHeartbeatSender {
    fn send(&self, report: &HeartbeatReport) -> Result<(), SendError> {
        self.client
            .post(&self.url)
            .json(report)
            .send()
            .map_err(|e| SendError::Transport(e.to_string()))?
            .error_for_status()
            .map(|_| ())
            .map_err(|e| match e.status() {
                Some(s) => SendError::HttpStatus(s.as_u16()),
                None => SendError::Transport(e.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::DailyAggregator;
    use crate::report::InstallationUuid;

    #[test]
    fn recording_sender_captures_reports() {
        let sender = RecordingSender::default();
        let mut agg = DailyAggregator::new();
        agg.record_sync_attempt();
        agg.record_sync_duration_ms(120);

        let report = HeartbeatReport::build(
            InstallationUuid::new(),
            "macos-14.5",
            "0.1.0",
            crate::aggregate::WINDOW_24H.as_secs(),
            agg.snapshot().clone(),
        );
        assert!(sender.send(&report).is_ok());
        assert_eq!(sender.count(), 1);
        assert_eq!(sender.sent_reports()[0], report);
    }

    #[test]
    fn failing_sender_reports_attempts_and_status() {
        let sender = FailingSender::default();
        let report = HeartbeatReport::build(
            InstallationUuid::new(),
            "linux",
            "0.1.0",
            crate::aggregate::WINDOW_24H.as_secs(),
            DailyAggregator::new().snapshot().clone(),
        );
        assert!(sender.send(&report).is_err());
        assert_eq!(
            sender.attempts.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }
}
