//! XChaCha20-Poly1305 AEAD with associated-data (AD) binding.
//!
//! Spec: [`docs/architecture/crypto.md`] §3. 192-bit nonce, 128-bit tag.
//! AD binds `(uuid, enc_key_gen)` so ciphertexts cannot be swapped or replayed.

use crate::error::{CryptoError, Result};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Ciphertext envelope: `Nonce(24) || Ciphertext(N) || Tag(16)`.
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

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

/// Encrypt `plaintext` under `key` (32 bytes) with AD = `(uuid, enc_key_gen)`.
pub fn encrypt(
    key: &[u8; 32],
    uuid: &Uuid,
    enc_key_gen: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce_bytes = random_nonce();
    let nonce: XNonce = nonce_bytes.try_into().expect("nonce len");
    let ad = construct_ad(uuid, enc_key_gen);
    let mut ct = cipher
        .encrypt(&nonce, Payload { msg: plaintext, aad: &ad })
        .map_err(|_| CryptoError::Internal("encryption failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.append(&mut ct);
    Ok(out)
}

/// Decrypt an envelope produced by [`encrypt`].
///
/// Returns `TagMismatch` on wrong key/tamper, `MalformedCiphertext` on bad length.
pub fn decrypt(
    key: &[u8; 32],
    uuid: &Uuid,
    enc_key_gen: u64,
    envelope: &[u8],
) -> Result<Vec<u8>> {
    if envelope.len() < NONCE_LEN + TAG_LEN {
        return Err(CryptoError::MalformedCiphertext);
    }
    let (nonce_bytes, ct) = envelope.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce: XNonce = nonce_bytes.try_into().expect("nonce len");
    let ad = construct_ad(uuid, enc_key_gen);
    cipher
        .decrypt(&nonce, Payload { msg: ct, aad: &ad })
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
