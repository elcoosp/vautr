//! Error types for the streaming file pipeline.

use thiserror::Error;

/// Errors produced by `vautr-files`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FileError {
    /// AEAD tag mismatch — wrong key or tampered chunk ciphertext.
    #[error("chunk authentication failed (wrong key or tampered ciphertext)")]
    TagMismatch,

    /// A chunk's on-wire envelope was too short to contain a nonce + tag.
    #[error("malformed chunk envelope")]
    MalformedChunk,

    /// The recovered plaintext for a chunk did not match the expected size
    /// (e.g. trailing garbage, or a length-truncation attack).
    #[error("chunk length integrity violation")]
    LengthIntegrity,

    /// `total_size` in the manifest is inconsistent with `chunk_size *
    /// (total_chunks - 1) + last_chunk_size`.
    #[error("manifest geometry is inconsistent: {0}")]
    ManifestGeometry(String),

    /// Serialization of the manifest to/from JSON failed.
    #[error("manifest serialization failed: {0}")]
    ManifestSerde(String),

    /// The provided `enc_key_gen` does not match the manifest.
    #[error("encryption epoch mismatch")]
    KeyGenMismatch,

    /// Underlying I/O error from the async stream.
    #[error("io error: {0}")]
    Io(String),
}

impl From<vautr_crypto::error::CryptoError> for FileError {
    fn from(e: vautr_crypto::error::CryptoError) -> Self {
        match e {
            vautr_crypto::error::CryptoError::TagMismatch => FileError::TagMismatch,
            vautr_crypto::error::CryptoError::MalformedCiphertext => FileError::MalformedChunk,
            other => FileError::Io(format!("crypto: {other}")),
        }
    }
}

impl From<serde_json::Error> for FileError {
    fn from(e: serde_json::Error) -> Self {
        FileError::ManifestSerde(e.to_string())
    }
}

impl From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        FileError::Io(e.to_string())
    }
}

/// Result alias for `vautr-files`.
pub type Result<T> = core::result::Result<T, FileError>;
