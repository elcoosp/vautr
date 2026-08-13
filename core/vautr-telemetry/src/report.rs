//! # Daily Heartbeat report.
//!
//! Defines the payload the client transmits once per 24 hours (spec §4.3).
//! The heartbeat contains **only** the non-PII [`InstallationUuid`], OS and app
//! version strings, and the 24-hour [`AggregatedMetrics`] buffer. It carries no
//! user ID, no email, no OPAQUE session token, and no high-resolution event
//! timestamps.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::aggregate::AggregatedMetrics;

/// A cryptographically random, non-PII installation identifier (spec §4.3).
///
/// Generated on first launch and persisted locally. It contains no linkable
/// user information and naturally rotates on uninstall/reinstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallationUuid(Uuid);

impl InstallationUuid {
    /// Generate a new random installation identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse from its string representation.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(s).map(Self)
    }

    /// The string representation of the installation identifier.
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for InstallationUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstallationUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata describing the 24-hour window a report covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowMeta {
    /// Unix timestamp (seconds) when the window opened.
    pub window_start_unix_secs: u64,
    /// Unix timestamp (seconds) when the window closed.
    pub window_end_unix_secs: u64,
}

/// The complete Daily Heartbeat payload (spec §4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatReport {
    /// Non-PII installation identifier.
    pub installation_uuid: InstallationUuid,
    /// OS version string (e.g. `"macos-14.5"`). No device serial or hostname.
    pub os_version: String,
    /// Vautr app version string (spec §5.2 rollback target).
    pub app_version: String,
    /// The 24-hour window this report covers.
    pub window: WindowMeta,
    /// The aggregated, anonymous metrics buffer.
    pub metrics: AggregatedMetrics,
}

impl HeartbeatReport {
    /// Build a report for a just-rolled window.
    ///
    /// `window_end_unix_secs` defaults to "now"; `window_start_unix_secs` is the
    /// provided duration subtracted from it. All fields are caller-supplied and
    /// must be PII-free.
    pub fn build(
        installation_uuid: InstallationUuid,
        os_version: impl Into<String>,
        app_version: impl Into<String>,
        window_duration_secs: u64,
        metrics: AggregatedMetrics,
    ) -> Self {
        let now = now_unix_secs();
        let start = now.saturating_sub(window_duration_secs);
        Self {
            installation_uuid,
            os_version: os_version.into(),
            app_version: app_version.into(),
            window: WindowMeta {
                window_start_unix_secs: start,
                window_end_unix_secs: now,
            },
            metrics,
        }
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::DailyAggregator;

    #[test]
    fn installation_uuid_is_random_and_parseable() {
        let a = InstallationUuid::new();
        let b = InstallationUuid::new();
        assert_ne!(a, b);

        let reparsed = InstallationUuid::parse(&a.as_str()).expect("roundtrips");
        assert_eq!(reparsed, a);
        assert_eq!(a.as_str().len(), 36); // standard UUID
    }

    #[test]
    fn heartbeat_serializes_and_deserializes_json() {
        let mut agg = DailyAggregator::new();
        agg.record_sync_attempt();
        agg.record_sync_duration_ms(120);
        agg.record_sync_failure("CoreError::OccConflict");

        let report = HeartbeatReport::build(
            InstallationUuid::new(),
            "macos-14.5",
            "0.1.0",
            crate::aggregate::WINDOW_24H.as_secs(),
            agg.snapshot().clone(),
        );

        let json = serde_json::to_string(&report).expect("serializes to JSON");
        assert!(json.contains("installation_uuid"));
        assert!(json.contains("os_version"));
        assert!(json.contains("sync_duration_ms"));

        let back: HeartbeatReport = serde_json::from_str(&json).expect("deserializes back");
        assert_eq!(back, report);
        assert_eq!(back.os_version, "macos-14.5");
        assert_eq!(back.metrics.sync_duration_ms.sum(), 120);
        assert_eq!(
            back.metrics.sync_failure_counts["CoreError::OccConflict"],
            1
        );
    }

    #[test]
    fn window_meta_spans_requested_duration() {
        let report = HeartbeatReport::build(
            InstallationUuid::new(),
            "linux",
            "0.1.0",
            86400,
            AggregatedMetrics::default(),
        );
        assert_eq!(
            report.window.window_end_unix_secs - report.window.window_start_unix_secs,
            86400
        );
    }
}
