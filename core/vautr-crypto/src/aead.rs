//! XChaCha20-Poly1305 AEAD with associated-data (AD) binding.
//!
//! Spec: [`docs/architecture/crypto.md`] §3. 192-bit nonce, 128-bit tag.
//! AD binds `(uuid, enc_key_gen)` so ciphertexts cannot be swapped or replayed.
//!
//! # Nonce policy
//!
//! [`encrypt`]/[`decrypt`] use a **randomized** 24-byte nonce (crypto.md §2.8).
//!
//! [`encrypt_with_nonce`]/[`decrypt_with_ad`] take an **explicit** nonce. This
//! is used by the streaming file pipeline ([`docs/architecture/file-storage.md`]
//! §2.2), which derives a *deterministic* nonce per chunk
//! (`SHA-256(file_uuid ‖ chunk_index)[..24]`) so interrupted uploads can resume
//! from `FileManifest.total_chunks` without persisting per-chunk nonces.
//!
//! **Deviation note:** deterministic nonces are normally forbidden by
//! crypto.md §2.8. They are safe here ONLY because (a) the nonce is bound to
//! `(file_uuid, chunk_index)` and (b) the key (`FEK`) is unique per file. This
//! is a documented, intentional exception for resumable streaming — not a
//! general-purpose AEAD mode. Never reuse [`encrypt_with_nonce`] for item
//! payloads.

use crate::error::{CryptoError, Result};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Ciphertext envelope: `Nonce(24) || Ciphertext(N) || Tag(16)`.
pub const NONCE_LEN: usize = 24;
/// XChaCha20-Poly1305 authentication tag length (128-bit).
pub const TAG_LEN: usize = 16;
/// Length of the file-chunk Associated Data (uuid(16) + enc_key_gen(8) + chunk_index(4)).
///
/// AD length is not constrained to the nonce length; XChaCha20-Poly1305 AD can
/// be any size.
pub const FILE_AD_LEN: usize = 16 + 8 + 4;

/// Build the 24-byte Associated Data from item uuid + enc_key_gen.
///
/// Matches crypto.md §3.2 `construct_ad` exactly:
/// `ad[..16] = uuid bytes`, `ad[16..24] = enc_key_gen.to_be_bytes()`.
pub fn construct_ad(uuid: &Uuid, enc_key_gen: u64) -> [u8; 24] {
    let mut ad = [0u8; 24];
    ad[..16].copy_from_slice(uuid.as_bytes());
    ad[16..24].copy_from_slice(&enc_key_gen.to_be_bytes());
    ad
}

/// Build the 28-byte Associated Data for a file chunk.
///
/// Binds the chunk to its file, vault epoch, and position (file-storage.md
/// §2.2) to prevent chunk reordering or insertion by a malicious server:
/// `ad[..16] = file_uuid`, `ad[16..24] = enc_key_gen.to_be_bytes()`,
/// `ad[24..28] = chunk_index.to_be_bytes()`.
pub fn construct_file_ad(
    file_uuid: &Uuid,
    enc_key_gen: u64,
    chunk_index: u32,
) -> [u8; FILE_AD_LEN] {
    let mut ad = [0u8; FILE_AD_LEN];
    ad[..16].copy_from_slice(file_uuid.as_bytes());
    ad[16..24].copy_from_slice(&enc_key_gen.to_be_bytes());
    ad[24..28].copy_from_slice(&chunk_index.to_be_bytes());
    ad
}

/// Derive the deterministic per-chunk nonce for streaming file encryption.
///
/// `Nonce = SHA-256(file_uuid_bytes ‖ chunk_index.to_be_bytes())[..24]`
/// (file-storage.md §2.2). Unique per `(file_uuid, chunk_index)`, so resumable
/// uploads recompute it without persisted state.
pub fn chunk_nonce(file_uuid: &Uuid, chunk_index: u32) -> [u8; NONCE_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(file_uuid.as_bytes());
    hasher.update(chunk_index.to_be_bytes());
    let digest = hasher.finalize();
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&digest[..NONCE_LEN]);
    nonce
}

/// Encrypt `plaintext` under `key` (32 bytes) with AD = `(uuid, enc_key_gen)`.
pub fn encrypt(key: &[u8; 32], uuid: &Uuid, enc_key_gen: u64, plaintext: &[u8]) -> Result<Vec<u8>> {
    let nonce = random_nonce();
    encrypt_with_nonce(key, &nonce, &construct_ad(uuid, enc_key_gen), plaintext)
}

/// Encrypt with an **explicit** nonce and explicit AD.
///
/// The output envelope is identical to [`encrypt`]: `Nonce(24) || Ciphertext || Tag(16)`.
/// Used by the streaming file pipeline where the nonce is derived deterministically
/// (see module docs). The caller is responsible for nonce uniqueness for the given key.
pub fn encrypt_with_nonce(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    ad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let n: XNonce = nonce.as_slice().try_into().expect("nonce len");
    let ct = cipher
        .encrypt(
            &n,
            Payload {
                msg: plaintext,
                aad: ad,
            },
        )
        .map_err(|_| CryptoError::Internal("encryption failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt an envelope produced by [`encrypt`].
///
/// Returns `TagMismatch` on wrong key/tamper, `MalformedCiphertext` on bad length.
pub fn decrypt(key: &[u8; 32], uuid: &Uuid, enc_key_gen: u64, envelope: &[u8]) -> Result<Vec<u8>> {
    decrypt_with_ad(key, &construct_ad(uuid, enc_key_gen), envelope)
}

/// Decrypt an envelope produced by [`encrypt_with_nonce`] using an explicit AD.
pub fn decrypt_with_ad(key: &[u8; 32], ad: &[u8], envelope: &[u8]) -> Result<Vec<u8>> {
    if envelope.len() < NONCE_LEN + TAG_LEN {
        return Err(CryptoError::MalformedCiphertext);
    }
    let (nonce_bytes, ct) = envelope.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce: XNonce = nonce_bytes.try_into().expect("nonce len");
    cipher
        .decrypt(&nonce, Payload { msg: ct, aad: ad })
        .map_err(|_| CryptoError::TagMismatch)
}

/// CSPRNG 24-byte nonce.
fn random_nonce() -> [u8; 24] {
    let mut n = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut n);
    n
}

/// Helper: wrap a key in [`Zeroizing`] for caller convenience.
pub fn zeroizing_key(bytes: [u8; 32]) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(bytes)
}
