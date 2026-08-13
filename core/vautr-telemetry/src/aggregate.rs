//! # 24-hour aggregation of anonymous telemetry metrics.
//!
//! Implements the **24-hour aggregation mandate** (spec §4.1): the client never
//! streams per-event telemetry. Instead it records integer-only counters and
//! timings into an in-memory buffer, which is rolled over once per 24-hour
//! window. The completed window becomes the payload of the Daily Heartbeat.
//!
//! ## No-PII rule (spec §1)
//! Every value in this module is a bare `u64` counter or timing. There is no
//! string that can carry a title, username, URL, note, or secret. Error "type"
//! segmenters are limited to the discriminant names of public error enums
//! (e.g. `CryptoError::TagMismatch`), which the spec §1.2 classifies as **SAFE**.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// A fixed window duration of exactly 24 hours.
pub const WINDOW_24H: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum number of raw samples retained per histogram for percentile
/// estimation. A fixed cap keeps memory bounded regardless of event volume
/// (spec §1: no unbounded growth, no per-event timestamps stored).
const RESERVOIR_CAP: usize = 4096;

/// Percentile estimates derived from a [`Histogram`]'s bounded reservoir.
///
/// Serialized into the heartbeat so the backend receives aggregated timing
/// distribution (p50/p95/p99) without any raw per-event values (spec §4.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistogramPercentiles {
    /// 50th percentile (median), if any samples were recorded.
    pub p50: Option<u64>,
    /// 95th percentile, if any samples were recorded.
    pub p95: Option<u64>,
    /// 99th percentile, if any samples were recorded.
    pub p99: Option<u64>,
}

/// An integer-only histogram over a 24-hour window.
///
/// Records the `count`, `sum`, `min` and `max` of a single metric. A bounded
/// reservoir of raw samples is kept solely to estimate percentiles; no
/// per-event timestamps are retained, so the temporal correlation attack
/// vector described in spec §4.1 is eliminated.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Histogram {
    count: u64,
    sum: u64,
    min: Option<u64>,
    max: Option<u64>,
    /// Percentile estimates, recomputed on each `record` from the reservoir.
    #[serde(default)]
    percentiles: HistogramPercentiles,
    /// Bounded raw-sample reservoir for percentile estimation (not serialized).
    #[serde(skip)]
    reservoir: Vec<u64>,
}

impl PartialEq for Histogram {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count
            && self.sum == other.sum
            && self.min == other.min
            && self.max == other.max
            && self.percentiles == other.percentiles
    }
}

impl Eq for Histogram {}

impl Histogram {
    /// Record a single integer observation.
    pub fn record(&mut self, value: u64) {
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.saturating_add(value);
        self.min = Some(self.min.map_or(value, |m| m.min(value)));
        self.max = Some(self.max.map_or(value, |m| m.max(value)));
        // Maintain a bounded reservoir for percentile estimation.
        if self.reservoir.len() >= RESERVOIR_CAP {
            self.reservoir.remove(0);
        }
        self.reservoir.push(value);
        self.recompute_percentiles();
    }

    /// Recompute p50/p95/p99 from the current reservoir (nearest-rank).
    fn recompute_percentiles(&mut self) {
        if self.reservoir.is_empty() {
            self.percentiles = HistogramPercentiles::default();
            return;
        }
        let mut sorted = self.reservoir.clone();
        sorted.sort_unstable();
        let rank = |p: f64| -> u64 {
            // Nearest-rank: index = ceil(p * n) - 1, clamped.
            let n = sorted.len();
            let idx = ((p * n as f64).ceil() as usize)
                .saturating_sub(1)
                .min(n - 1);
            sorted[idx]
        };
        self.percentiles = HistogramPercentiles {
            p50: Some(rank(0.50)),
            p95: Some(rank(0.95)),
            p99: Some(rank(0.99)),
        };
    }

    /// The percentile estimates for this histogram (p50/p95/p99).
    pub fn percentiles(&self) -> HistogramPercentiles {
        self.percentiles
    }

    /// 50th percentile (median) of recorded samples, if any.
    pub fn p50(&self) -> Option<u64> {
        self.percentiles.p50
    }

    /// 95th percentile of recorded samples, if any.
    pub fn p95(&self) -> Option<u64> {
        self.percentiles.p95
    }

    /// 99th percentile of recorded samples, if any.
    pub fn p99(&self) -> Option<u64> {
        self.percentiles.p99
    }

    /// Merge another histogram into this one (used when folding windows).
    pub fn merge(&mut self, other: &Histogram) {
        self.count = self.count.saturating_add(other.count);
        self.sum = self.sum.saturating_add(other.sum);
        self.min = match (self.min, other.min) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.max = match (self.max, other.max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        // Fold reservoirs (bounded), then recompute percentiles.
        self.reservoir.extend_from_slice(&other.reservoir);
        if self.reservoir.len() > RESERVOIR_CAP {
            let drop = self.reservoir.len() - RESERVOIR_CAP;
            self.reservoir.drain(0..drop);
        }
        self.recompute_percentiles();
    }

    /// Number of observations recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Sum of all observations.
    pub fn sum(&self) -> u64 {
        self.sum
    }

    /// Minimum observation, if any.
    pub fn min(&self) -> Option<u64> {
        self.min
    }

    /// Maximum observation, if any.
    pub fn max(&self) -> Option<u64> {
        self.max
    }

    /// Integer mean (sum / count), or `None` when empty.
    pub fn avg(&self) -> Option<u64> {
        if self.count == 0 {
            None
        } else {
            Some(self.sum / self.count)
        }
    }
}

/// The full set of metrics aggregated over one 24-hour window.
///
/// All fields are integer-only. Error-type segmenters are `BTreeMap<String, u64>`
/// keyed by public, PII-free error discriminant names.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AggregatedMetrics {
    /// Histogram of `sync_duration_ms` over the window (spec §4.2).
    pub sync_duration_ms: Histogram,
    /// Histogram of `argon2_duration_ms` over the window (spec §4.2).
    pub argon2_duration_ms: Histogram,
    /// Histogram of `aead_decrypt_duration_us` over the window (spec §4.2).
    pub aead_decrypt_duration_us: Histogram,

    /// Sync failures segmented by `CoreError` type name (spec §4.2).
    pub sync_failure_counts: BTreeMap<String, u64>,
    /// OCC conflict counts segmented by `CoreError` type name.
    pub occ_conflict_counts: BTreeMap<String, u64>,

    /// Counter: orphans cleaned by the Safety Reaper (spec §4.2 / §5.2).
    pub reaper_orphans_cleaned: u64,
    /// Counter: quarantine TTL resets (spec §4.2).
    pub quarantine_ttl_resets: u64,
    /// Counter: total sync attempts in the window.
    pub sync_attempts: u64,
}

impl AggregatedMetrics {
    /// Record a sync failure, segmenting by the (PII-free) error type name.
    pub fn record_sync_failure(&mut self, error_type: &str) {
        *self
            .sync_failure_counts
            .entry(error_type.to_string())
            .or_insert(0) += 1;
    }

    /// Record an OCC conflict, segmenting by the (PII-free) error type name.
    pub fn record_occ_conflict(&mut self, error_type: &str) {
        *self
            .occ_conflict_counts
            .entry(error_type.to_string())
            .or_insert(0) += 1;
    }
}

/// An in-memory aggregator that accumulates metrics for a rolling window.
///
/// Records into the current window until [`DailyAggregator::window_elapsed`]
/// reports the configured duration has passed, at which point
/// [`DailyAggregator::roll_window`] yields the completed [`AggregatedMetrics`]
/// and starts a fresh window.
pub struct DailyAggregator {
    current: AggregatedMetrics,
    window_started_at: Instant,
    window_duration: Duration,
}

impl Default for DailyAggregator {
    fn default() -> Self {
        Self {
            current: AggregatedMetrics::default(),
            window_started_at: Instant::now(),
            window_duration: WINDOW_24H,
        }
    }
}

impl DailyAggregator {
    /// Create an aggregator with the default 24-hour window.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an aggregator with a custom window duration (for tests / tuning).
    pub fn with_window_duration(duration: Duration) -> Self {
        Self {
            current: AggregatedMetrics::default(),
            window_started_at: Instant::now(),
            window_duration: duration,
        }
    }

    /// The configured window duration (defaults to 24h).
    pub fn window_duration(&self) -> Duration {
        self.window_duration
    }

    /// Record a sync attempt.
    pub fn record_sync_attempt(&mut self) {
        self.current.sync_attempts = self.current.sync_attempts.saturating_add(1);
    }

    /// Record a sync duration in milliseconds.
    pub fn record_sync_duration_ms(&mut self, ms: u64) {
        self.current.sync_duration_ms.record(ms);
    }

    /// Record an argon2 derivation duration in milliseconds.
    pub fn record_argon2_duration_ms(&mut self, ms: u64) {
        self.current.argon2_duration_ms.record(ms);
    }

    /// Record an AEAD decrypt duration in microseconds.
    pub fn record_aead_decrypt_duration_us(&mut self, us: u64) {
        self.current.aead_decrypt_duration_us.record(us);
    }

    /// Record a sync failure segmented by error type name.
    pub fn record_sync_failure(&mut self, error_type: &str) {
        self.current.record_sync_failure(error_type);
    }

    /// Record an OCC conflict segmented by error type name.
    pub fn record_occ_conflict(&mut self, error_type: &str) {
        self.current.record_occ_conflict(error_type);
    }

    /// Increment the Safety Reaper orphans-cleaned counter.
    pub fn increment_reaper_orphans_cleaned(&mut self) {
        self.current.reaper_orphans_cleaned = self.current.reaper_orphans_cleaned.saturating_add(1);
    }

    /// Increment the quarantine TTL reset counter.
    pub fn increment_quarantine_ttl_resets(&mut self) {
        self.current.quarantine_ttl_resets = self.current.quarantine_ttl_resets.saturating_add(1);
    }

    /// Snapshot of the metrics recorded so far in the current window.
    pub fn snapshot(&self) -> &AggregatedMetrics {
        &self.current
    }

    /// True when the current window duration has elapsed and is ready to roll.
    pub fn window_elapsed(&self) -> bool {
        self.window_started_at.elapsed() >= self.window_duration
    }

    /// Time remaining in the current window, saturating at zero.
    pub fn window_remaining(&self) -> Duration {
        self.window_duration
            .saturating_sub(self.window_started_at.elapsed())
    }

    /// Roll the completed window over, returning the finished metrics and
    /// starting a fresh window. Returns `None` if the window has not elapsed.
    pub fn roll_window(&mut self) -> Option<AggregatedMetrics> {
        if !self.window_elapsed() {
            return None;
        }
        let finished = std::mem::take(&mut self.current);
        self.window_started_at = Instant::now();
        tracing::info!(finished_metrics = ?finished, "telemetry window rolled over");
        Some(finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_tracks_count_sum_min_max_avg() {
        let mut h = Histogram::default();
        assert_eq!(h.count(), 0);
        assert_eq!(h.min(), None);
        assert_eq!(h.max(), None);
        assert_eq!(h.avg(), None);

        for v in [10, 20, 15] {
            h.record(v);
        }

        assert_eq!(h.count(), 3);
        assert_eq!(h.sum(), 45);
        assert_eq!(h.min(), Some(10));
        assert_eq!(h.max(), Some(20));
        assert_eq!(h.avg(), Some(15));
    }

    #[test]
    fn histogram_merge_combines_windows() {
        let mut a = Histogram::default();
        a.record(10);
        a.record(30);

        let mut b = Histogram::default();
        b.record(20);

        a.merge(&b);
        assert_eq!(a.count(), 3);
        assert_eq!(a.sum(), 60);
        assert_eq!(a.min(), Some(10));
        assert_eq!(a.max(), Some(30));
    }

    #[test]
    fn aggregator_segments_failures_by_error_type() {
        let mut agg = DailyAggregator::new();
        agg.record_sync_attempt();
        agg.record_sync_attempt();
        agg.record_sync_failure("CoreError::OccConflict");
        agg.record_sync_failure("CoreError::OccConflict");
        agg.record_sync_failure("CoreError::TagMismatch");

        let snap = agg.snapshot();
        assert_eq!(snap.sync_attempts, 2);
        assert_eq!(snap.sync_failure_counts["CoreError::OccConflict"], 2);
        assert_eq!(snap.sync_failure_counts["CoreError::TagMismatch"], 1);

        agg.record_occ_conflict("CoreError::OccConflict");
        let snap = agg.snapshot();
        assert_eq!(snap.occ_conflict_counts["CoreError::OccConflict"], 1);
    }

    #[test]
    fn aggregator_records_timings_and_counters() {
        let mut agg = DailyAggregator::new();
        agg.record_sync_duration_ms(120);
        agg.record_sync_duration_ms(180);
        agg.record_argon2_duration_ms(250);
        agg.record_aead_decrypt_duration_us(42);
        agg.increment_reaper_orphans_cleaned();
        agg.increment_reaper_orphans_cleaned();
        agg.increment_quarantine_ttl_resets();

        let snap = agg.snapshot();
        assert_eq!(snap.sync_duration_ms.count(), 2);
        assert_eq!(snap.sync_duration_ms.sum(), 300);
        assert_eq!(snap.argon2_duration_ms.max(), Some(250));
        assert_eq!(snap.aead_decrypt_duration_us.sum(), 42);
        assert_eq!(snap.reaper_orphans_cleaned, 2);
        assert_eq!(snap.quarantine_ttl_resets, 1);
    }

    #[test]
    fn window_does_not_roll_early() {
        let mut agg = DailyAggregator::with_window_duration(Duration::from_secs(60));
        agg.record_sync_duration_ms(10);
        // Window not yet elapsed.
        assert!(!agg.window_elapsed());
        assert_eq!(agg.roll_window(), None);
        // Metrics still present in the current window.
        assert_eq!(agg.snapshot().sync_duration_ms.count(), 1);
    }

    #[test]
    fn window_rolls_and_resets_after_duration() {
        // A zero-duration window is immediately "elapsed" on the first check.
        let mut agg = DailyAggregator::with_window_duration(Duration::ZERO);
        agg.record_sync_duration_ms(7);
        agg.record_sync_failure("CoreError::OccConflict");

        let finished = agg
            .roll_window()
            .expect("zero-duration window rolls immediately");
        assert_eq!(finished.sync_duration_ms.sum(), 7);
        assert_eq!(finished.sync_failure_counts["CoreError::OccConflict"], 1);

        // A fresh window starts empty.
        let snap = agg.snapshot();
        assert_eq!(snap.sync_duration_ms.count(), 0);
        assert!(snap.sync_failure_counts.is_empty());
    }

    #[test]
    fn default_window_is_24h() {
        assert_eq!(WINDOW_24H, Duration::from_secs(24 * 60 * 60));
        assert_eq!(DailyAggregator::new().window_duration(), WINDOW_24H);
    }

    #[test]
    fn histogram_computes_percentiles_from_reservoir() {
        let mut h = Histogram::default();
        // 1..=100 inclusive.
        for v in 1u64..=100 {
            h.record(v);
        }
        // Nearest-rank percentiles over 1..=100.
        assert_eq!(h.p50(), Some(50));
        assert_eq!(h.p95(), Some(95));
        assert_eq!(h.p99(), Some(99));
        // Count/sum/min/max still tracked alongside percentiles.
        assert_eq!(h.count(), 100);
        assert_eq!(h.min(), Some(1));
        assert_eq!(h.max(), Some(100));
        // Percentiles are serialized into the report (no raw values leaked).
        let json = serde_json::to_string(&h).expect("serializes");
        assert!(json.contains("\"p50\""));
        assert!(json.contains("\"p95\""));
        assert!(json.contains("\"p99\""));
        // Reservoir is never serialized (no per-event data on the wire).
        assert!(!json.contains("reservoir"));
    }

    #[test]
    fn empty_histogram_has_no_percentiles() {
        let h = Histogram::default();
        assert_eq!(h.p50(), None);
        assert_eq!(h.p95(), None);
        assert_eq!(h.p99(), None);
    }
}
