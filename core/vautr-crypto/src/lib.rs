//! # vautr-crypto
//!
//! Pure cryptographic primitives for Vautr. **No I/O.** All key material is held
//! in [`zeroize::Zeroizing`] and overwritten on drop.
//!
//! Spec: [`docs/architecture/crypto.md`], [`docs/spec/arch-design.md`] ADR-002/003/006.
//!
//! ## Implemented here
//! - `kdf`     — Argon2id Master Key (MK) derivation (m_cost=64 MiB, t=3, p=4).
//! - `aead`    — XChaCha20-Poly1305 with 192-bit nonce + AD binding (uuid + enc_key_gen).
//! - `key_tree`— HKDF-SHA256 derivation MK→KEK→SVK→OEK/DEK.
//! - `opaque`  — OPAQUE PAKE client/server helpers (server never sees password equivalents).
//! - `recovery`— BIP-39 24-word Recovery Key → KEK_RK (see crypto.md §6 + REQ-RECOVERY-01).
//! - `sharing` — X25519 + XChaCha20-Poly1305 secure item sharing (ADR-007, `sharing` feature).
//!
//! ## Safety
//! `#![forbid(unsafe_code)]` — audited pure-Rust crypto only (SRS CON-2).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod aead;
pub mod error;
pub mod kdf;
pub mod key_tree;
pub mod opaque;
pub mod recovery;

#[cfg(feature = "sharing")]
pub mod sharing;

// Test-only memory-leak instrumentation (VTR-037). Absent from production
// builds — CI asserts the `test-instrumentation` feature is never enabled there.
#[cfg(feature = "test-instrumentation")]
pub mod instrumentation;
