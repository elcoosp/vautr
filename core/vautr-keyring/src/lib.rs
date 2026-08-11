//! # vautr-keyring
//!
//! Symmetrical Vault Key (SVK) lifecycle: generation, dual-wrapping under
//! `KEK_MP` and `KEK_RK`, crash-safe rotation (ADR-006 / REQ-ROTATE-01..03),
//! and forced re-wrap on emergency recovery (REQ-RECOVERY-02).
//!
//! Implemented surfaces:
//! - `svk`      — SVK generation + KEK/DEK derivation via `vautr-crypto`.
//! - `wrap`     — XChaCha20-Poly1305 wrapping of SVK blob for server storage.
//! - `rotate`   — batch re-encryption driven by `enc_key_gen` cursor (ADR-006).
//! - `recover`  — unwrap via Recovery Key, force MP reset (⚠ needs KEK_RK spec).

pub mod svk;
pub mod wrap;
pub mod rotate;
pub mod recover;
