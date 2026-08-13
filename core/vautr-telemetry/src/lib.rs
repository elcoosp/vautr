//! # vautr-telemetry
//!
//! User-opt-in, anonymous client telemetry aggregator for Vautr.
//!
//! Spec: [`docs/architecture/telemetry.md`]. Telemetry measures the *health and
//! performance* of the system (sync timings, OCC conflict counts, crash-adjacent
//! diagnostics) **without tracking user behavior**.
//!
//! ## Zero-Knowledge boundary (spec §1)
//! This crate enforces the No-PII rule at the type level:
//! - All aggregated metrics are integer-only `u64` counters and timings
//!   ([`aggregate`]).
//! - Error-type segmenters are keyed by public, PII-free error discriminant
//!   names (e.g. `CoreError::OccConflict`), which the spec §1.2 marks **SAFE**.
//! - The only identifier in a heartbeat is a random, non-linkable
//!   [`report::InstallationUuid`] (spec §4.3).
//! - Nothing in this crate can carry a title, username, URL, note, or secret.
//!
//! ## 24-hour aggregation mandate (spec §4.1)
//! The client never streams per-event telemetry. Metrics accumulate in an
//! in-memory buffer ([`aggregate::DailyAggregator`]) and are transmitted at most
//! once per 24 hours as a [`report::HeartbeatReport`], destroying the temporal
//! correlation attack vector.
//!
//! ## Consent (spec §4)
//! All telemetry is strictly opt-in and gated behind [`consent::ConsentGate`].
//! The default state is disabled; nothing is collected until the user grants it.
//!
//! **Scaffold note:** every entry point is a safe, non-panicking implementation.
//! The `no todo!()` roadmap rule is honored throughout.

pub mod aggregate;
pub mod consent;
pub mod controller;
pub mod monitoring;
pub mod report;
pub mod sender;

pub use aggregate::{AggregatedMetrics, DailyAggregator, Histogram, HistogramPercentiles};
pub use consent::{Consent, ConsentGate};
pub use controller::{TelemetryController, DEFAULT_HEARTBEAT_PATH};
#[cfg(feature = "webhook")]
pub use monitoring::WebhookDeliverer;
pub use monitoring::{
    AlertEvent, AlertSeverity, Alerting, HealthStatus, MetricsSnapshot, ServerMetrics, WebhookHook,
};
pub use report::{HeartbeatReport, InstallationUuid, WindowMeta};
#[cfg(feature = "http")]
pub use sender::HttpHeartbeatSender;
pub use sender::{HeartbeatSender, RecordingSender, SendError};
