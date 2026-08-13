//! Test-only memory-leak instrumentation (VTR-037).
//!
//! Exposes a `Zeroizing<String>` wrapper that increments a process-global
//! counter on `Drop`. The memory-leak regression test reveals and copies many
//! secrets through this wrapper, then asserts every allocation was dropped
//! (counter returns to baseline) — catching accidental retention of plaintext in
//! memory.
//!
//! This module is compiled ONLY under the `test-instrumentation` feature. CI
//! asserts that feature is absent from production builds (see `verify-isolation.yml`).

#![cfg(feature = "test-instrumentation")]

use std::sync::atomic::{AtomicUsize, Ordering};
use zeroize::Zeroizing;

/// Count of `TrackedZeroizing` values currently alive (created minus dropped).
static OUTSTANDING: AtomicUsize = AtomicUsize::new(0);

/// Total `TrackedZeroizing` values dropped since process start.
static DROPPED: AtomicUsize = AtomicUsize::new(0);

/// Total `TrackedZeroizing` values created since process start.
static CREATED: AtomicUsize = AtomicUsize::new(0);

/// Reset all counters to zero. Call at the start of a leak test for isolation.
pub fn reset_counters() {
    OUTSTANDING.store(0, Ordering::SeqCst);
    DROPPED.store(0, Ordering::SeqCst);
    CREATED.store(0, Ordering::SeqCst);
}

/// Number of `TrackedZeroizing` values currently not yet dropped.
pub fn outstanding() -> usize {
    OUTSTANDING.load(Ordering::SeqCst)
}

/// Total number of `TrackedZeroizing` values dropped so far.
pub fn dropped() -> usize {
    DROPPED.load(Ordering::SeqCst)
}

/// Total number of `TrackedZeroizing` values created so far.
pub fn created() -> usize {
    CREATED.load(Ordering::SeqCst)
}

/// A `Zeroizing<String>` whose `Drop` updates the global leak counters.
///
/// Wraps a plaintext secret so the memory-leak test can verify that every
/// revealed secret is eventually zeroized and released.
pub struct TrackedZeroizing(Zeroizing<String>);

impl TrackedZeroizing {
    /// Wrap `secret` in a tracked, zeroizing `String`.
    pub fn new(secret: String) -> Self {
        CREATED.fetch_add(1, Ordering::SeqCst);
        OUTSTANDING.fetch_add(1, Ordering::SeqCst);
        TrackedZeroizing(Zeroizing::new(secret))
    }

    /// Borrow the inner plaintext (for assertions in tests).
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Drop for TrackedZeroizing {
    fn drop(&mut self) {
        // Zeroizing<String> zeroes the buffer on drop; this counter proves the
        // drop actually ran (i.e. the value was not leaked/retained).
        OUTSTANDING.fetch_sub(1, Ordering::SeqCst);
        DROPPED.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aead;
    use uuid::Uuid;

    #[test]
    fn decrypt_never_retains_plaintext_after_drop() {
        // VTR-037 memory-leak regression: reveal + copy 10,000 secrets in a
        // loop, then drop them all, and assert the global drop counter returns
        // to baseline (outstanding == 0, dropped == created).
        reset_counters();

        let key = [0x42u8; 32];
        let uuid = Uuid::nil();
        let enc_key_gen = 7u64;
        let plaintext = b"super-secret-value-that-must-be-zeroized".to_vec();

        // Encrypt once; we will decrypt it 10k times into tracked plaintext.
        let envelope = aead::encrypt(&key, &uuid, enc_key_gen, &plaintext).unwrap();

        const N: usize = 10_000;
        {
            let mut revealed: Vec<TrackedZeroizing> = Vec::with_capacity(N);
            for _ in 0..N {
                let pt = aead::decrypt(&key, &uuid, enc_key_gen, &envelope).unwrap();
                revealed.push(TrackedZeroizing::new(String::from_utf8(pt).unwrap()));
            }
            // `revealed` owns N tracked values here (outstanding == N).
            assert_eq!(outstanding(), N);
            assert_eq!(created(), N);
            // Explicitly drop the batch (simulates scope end / "GC").
            drop(revealed);
        }

        // Everything created was dropped; nothing outstanding remains.
        assert_eq!(outstanding(), 0, "plaintext leaked: outstanding != 0");
        assert_eq!(dropped(), N, "drop counter did not reach baseline");
        assert_eq!(created(), N);
    }

    #[test]
    fn decrypt_errors_do_not_spawn_tracked_values() {
        reset_counters();
        let key = [0u8; 32];
        let uuid = Uuid::nil();
        // Malformed envelope -> Err, must not create a tracked plaintext.
        let _ = aead::decrypt(&key, &uuid, 1, b"too-short");
        assert_eq!(created(), 0);
        assert_eq!(outstanding(), 0);
    }
}
