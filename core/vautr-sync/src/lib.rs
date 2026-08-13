//! # vautr-sync
//!
//! Client sync engine. Enforces metadata-first pull (docs/architecture/api.md),
//! DashMap-bounded payload downloads (ADR-002), and atomic OCC resolution
//! (ADR-004). The `SafetyReaper` zeroizes idle secret handles (REQ-SECRET-05).
//!
//! Implemented surfaces:
//! - `engine`   — `SyncEngine` trait + state machine.
//! - `dashmap`  — in-memory `LocalBlacklist` (ToxicIgnored / ValidIgnored).
//! - `reaper`   — background zeroization task (every 10s, 60s idle TTL).
//! - `occ`      — 412 / version_mismatch resolution -> DashMap.

pub mod dashmap;
pub mod engine;
pub mod occ;
pub mod quarantine;
pub mod reaper;
