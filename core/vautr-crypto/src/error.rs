//! Cryptographic error types for Vautr.
//!
//! Maps directly to [`docs/architecture/crypto.md`] §3.3 granular decryption
//! errors and [`docs/architecture/data.md`] §8.2 `CryptoError`.

use thiserror::Error;

/// Errors produced by `vautr-crypto`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// AEAD tag mismatch — wrong key or tampered ciphertext.
    #[error("AEAD tag mismatch (wrong key or tampered ciphertext)")]
    TagMismatch,

    /// Correct key family, but wrong epoch (`enc_key_gen`).
    #[error("key generation mismatch (wrong encryption epoch)")]
    KeyGenMismatch,

    /// Incorrect byte lengths / malformed ciphertext envelope.
    #[error("malformed ciphertext")]
    MalformedCiphertext,

    /// Argon2id derivation failure.
    #[error("key derivation failed: {0}")]
    KdfError(String),

    /// OPAQUE protocol failure.
    #[error("OPAQUE protocol error: {0}")]
    OpaqueError(String),

    /// Authentication (login/registration) failure surfaced to the UI.
    #[error("authentication failed: {0}")]
    AuthError(String),

    /// BIP-39 recovery key decode failure.
    #[error("recovery key invalid: {0}")]
    RecoveryError(String),

    /// Serialization / RNG failure.
    #[error("internal crypto error: {0}")]
    Internal(String),
}

/// Result alias used throughout `vautr-crypto`.
pub type Result<T> = core::result::Result<T, CryptoError>;
