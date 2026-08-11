//! UniFFI `VautrClient` surface for mobile (Swift/Kotlin). data.md, ADR-003.
//! Secrets cross only as opaque `u64` handles; `read_secret` is desktop-gated.

#![allow(unused)]

/// Opaque handle to a decrypted secret (u64). ADR-003.
pub type SecretHandle = u64;

/// Mobile-facing client stub. Real impl wraps `vautr_app_state::VautrClient`.
pub struct MobileClient;
