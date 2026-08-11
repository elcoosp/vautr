//! On-disk backup archive format, manifest, and integrity checks.
//!
//! A backup archive is an **encrypted envelope**: it never contains plaintext
//! SQLite bytes or plaintext metadata. The on-disk format is:
//!
//! ```text
//! MAGIC "VAUTRBK1" (8 bytes)
//! format_version  (u32, big-endian)
//! nonce           (24 bytes, randomized per archive)
//! ciphertext      (XChaCha20-Poly1305 AEAD: encrypted payload + 16-byte tag)
//! ```
//!
//! The plaintext payload is a JSON [`ArchivePayload`]: the PII-free
//! [`ArchiveMeta`] manifest plus the base64-encoded SQLite snapshot. Sealing /
//! opening is via [`seal`] / [`open`] (spec: docs/architecture/mlp-scope.md §1).

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while reading or validating a backup archive.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArchiveError {
    /// The archive's declared format version is not understood.
    #[error("archive format version {0} is not supported")]
    UnsupportedVersion(u32),
    /// An archive integrity check failed.
    #[error("archive integrity check failed: {0}")]
    Integrity(&'static str),
    /// The archive envelope is truncated / malformed (bad magic or too short).
    #[error("archive envelope is malformed or truncated")]
    Malformed,
    /// AEAD decryption failed (wrong key, tamper, or corrupted bytes).
    #[error("archive decryption failed: wrong key, tampered, or corrupted")]
    Decrypt,
}

/// Result alias for archive operations.
pub type ArchiveResult<T> = Result<T, ArchiveError>;

/// The highest archive format version this crate understands.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// File magic for Vautr backup archives (8 bytes).
pub const MAGIC: &[u8; 8] = b"VAUTRBK1";

/// XChaCha20-Poly1305 nonce length (192-bit).
const NONCE_LEN: usize = 24;

/// Lightweight, serializable archive manifest (PII-free; entry payloads are
/// encrypted separately).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMeta {
    /// Archive format version (see [`CURRENT_FORMAT_VERSION`]).
    pub format_version: u32,
    /// RFC3339-ish timestamp of when the archive was produced.
    pub created_at: String,
    /// Logical id of the vault this archive is a snapshot of.
    pub vault_id: String,
    /// Number of (encrypted) entries recorded in the archive.
    pub entry_count: u64,
}

impl ArchiveMeta {
    /// Construct a manifest from its fields.
    pub fn new(created_at: impl Into<String>, vault_id: impl Into<String>, entry_count: u64) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            created_at: created_at.into(),
            vault_id: vault_id.into(),
            entry_count,
        }
    }

    /// Reject archives from a future/incompatible format version.
    pub fn verify_format(&self) -> ArchiveResult<()> {
        if self.format_version > CURRENT_FORMAT_VERSION {
            Err(ArchiveError::UnsupportedVersion(self.format_version))
        } else {
            Ok(())
        }
    }
}

/// The plaintext inside a sealed archive: manifest + base64 SQLite snapshot.
///
/// The snapshot is the only sensitive part and it is always AEAD-encrypted
/// before reaching disk; this struct is the serialized `Payload` fed to [`seal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivePayload {
    /// PII-free manifest describing the snapshot.
    pub manifest: ArchiveMeta,
    /// The SQLite database snapshot, base64-encoded.
    pub snapshot_b64: String,
}

impl ArchivePayload {
    /// Build a payload from a manifest and raw snapshot bytes.
    pub fn new(manifest: ArchiveMeta, snapshot: &[u8]) -> Self {
        Self {
            manifest,
            snapshot_b64: base64::engine::general_purpose::STANDARD.encode(snapshot),
        }
    }

    /// Recover the raw snapshot bytes (base64-decoded).
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, ArchiveError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&self.snapshot_b64)
            .map_err(|_| ArchiveError::Integrity("snapshot base64 is corrupt"))
    }
}

/// Seal a serialized payload into an encrypted archive envelope.
///
/// The envelope is `MAGIC || version(u32 BE) || nonce(24) || ciphertext`, where
/// `ciphertext` is XChaCha20-Poly1305 output over `payload` with a fresh random
/// nonce per archive.
pub fn seal(key: &[u8; 32], payload: &[u8]) -> ArchiveResult<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);

    let cipher = XChaCha20Poly1305::new(key.into());
    let n: XNonce = nonce.as_slice().try_into().expect("nonce len");
    let ct = cipher
        .encrypt(&n, Payload { msg: payload, aad: MAGIC })
        .map_err(|_| ArchiveError::Integrity("AEAD encryption failed"))?;

    let mut out = Vec::with_capacity(MAGIC.len() + 4 + NONCE_LEN + ct.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&CURRENT_FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Open an encrypted archive envelope, returning the plaintext payload bytes.
pub fn open(key: &[u8; 32], archive: &[u8]) -> ArchiveResult<Vec<u8>> {
    let min_len = MAGIC.len() + 4 + NONCE_LEN;
    if archive.len() < min_len + 1 {
        return Err(ArchiveError::Malformed);
    }
    if &archive[..MAGIC.len()] != MAGIC {
        return Err(ArchiveError::Malformed);
    }
    let ver_bytes: [u8; 4] = archive[MAGIC.len()..MAGIC.len() + 4]
        .try_into()
        .expect("len checked");
    let version = u32::from_be_bytes(ver_bytes);
    if version > CURRENT_FORMAT_VERSION {
        return Err(ArchiveError::UnsupportedVersion(version));
    }
    let (nonce_bytes, ct) = archive.split_at(MAGIC.len() + 4 + NONCE_LEN);
    let nonce_bytes = &nonce_bytes[MAGIC.len() + 4..];

    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce: XNonce = nonce_bytes.try_into().expect("nonce len");
    cipher
        .decrypt(&nonce, Payload { msg: ct, aad: MAGIC })
        .map_err(|_| ArchiveError::Decrypt)
}

/// SHA-256 hex digest of arbitrary bytes (used for archive `checksum`).
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_with_version(v: u32) -> ArchiveMeta {
        ArchiveMeta {
            format_version: v,
            created_at: "2026-08-11T00:00:00Z".to_string(),
            vault_id: "vault-1".to_string(),
            entry_count: 0,
        }
    }

    #[test]
    fn current_version_is_accepted() {
        assert_eq!(meta_with_version(CURRENT_FORMAT_VERSION).verify_format(), Ok(()));
    }

    #[test]
    fn future_version_is_rejected() {
        assert_eq!(
            meta_with_version(CURRENT_FORMAT_VERSION + 1).verify_format(),
            Err(ArchiveError::UnsupportedVersion(CURRENT_FORMAT_VERSION + 1))
        );
    }

    #[test]
    fn manifest_roundtrips_through_json() {
        let meta = meta_with_version(CURRENT_FORMAT_VERSION);
        let json = serde_json::to_string(&meta).expect("serialize");
        let back: ArchiveMeta = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, meta);
    }

    #[test]
    fn seal_open_roundtrip() {
        let key = [7u8; 32];
        let payload = b"the encrypted secret snapshot".to_vec();
        let sealed = seal(&key, &payload).unwrap();
        // No plaintext anywhere in the envelope.
        assert!(!sealed.windows(payload.len()).any(|w| w == payload.as_slice()));
        let opened = open(&key, &sealed).unwrap();
        assert_eq!(opened, payload);
    }

    #[test]
    fn open_rejects_wrong_key() {
        let sealed = seal(&[1u8; 32], b"hello").unwrap();
        assert_eq!(open(&[2u8; 32], &sealed), Err(ArchiveError::Decrypt));
    }

    #[test]
    fn open_rejects_bad_magic() {
        let sealed = seal(&[1u8; 32], b"hello").unwrap();
        let mut bad = sealed.clone();
        bad[0] = b'X';
        assert_eq!(open(&[1u8; 32], &bad), Err(ArchiveError::Malformed));
    }

    #[test]
    fn sha256_hex_is_stable() {
        let h = sha256_hex(b"abc");
        // Known SHA-256("abc").
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
