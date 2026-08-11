//! # vautr-ffi
//!
//! UniFFI bindings exposing `VautrClient` to Swift/Kotlin. The opaque-handle
//! pattern (ADR-003) means secrets are never passed as strings across the
//! bridge; `perform_action(handle)` / `read_secret(handle)` (desktop only)
//! service copy/autofill natively.
//!
//! `read_secret` is gated behind the `desktop-api` feature (data.md §1 rule 4).

uniffi::setup_scaffolding!();

pub mod client;
