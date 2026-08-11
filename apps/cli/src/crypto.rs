//! Client-side secret-value crypto.
//!
//! The Vautr server is zero-knowledge: it stores only the AEAD ciphertext of a
//! secret value and never holds or distributes the project key. The CLI keeps
//! the project key it generates locally (in [`crate::config::Config`]) and uses
//! it to encrypt values on `create`/`edit` and decrypt them on `get`/`run`.
//!
//! Encryption uses the workspace `vautr-crypto::aead` XChaCha20-Poly1305
//! envelope (`Nonce(24) || Ciphertext || Tag(16)`) with empty associated data
//! for the secret's own value. Only the CLI-managed keys round-trip here; a
//! secret created by another client without a shared key cannot be decrypted.

use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;
use vautr_crypto::aead;

use crate::b64;
use crate::error::{CliError, CliResult};

/// Generate a fresh 32-byte project key (base64 for storage).
pub fn generate_project_key_b64() -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    b64::encode(&key)
}

/// Generate a fresh 32-byte project key (raw bytes).
pub fn generate_project_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Encrypt a secret value under a 32-byte key. Returns base64 ciphertext.
pub fn encrypt_value(key_b64: &str, value: &[u8]) -> CliResult<String> {
    let key = decode_key(key_b64)?;
    let envelope = aead::encrypt_with_nonce(&key, &zero_nonce(), &[], value)
        .map_err(|e| CliError::Crypto(e.to_string()))?;
    Ok(b64::encode(&envelope))
}

/// Decrypt a base64 ciphertext envelope under a 32-byte key.
pub fn decrypt_value(key_b64: &str, value_b64: &str) -> CliResult<Zeroizing<Vec<u8>>> {
    let key = decode_key(key_b64)?;
    let envelope = b64::decode(value_b64)?;
    let plaintext = aead::decrypt_with_ad(&key, &[], &envelope)
        .map_err(|e| CliError::Decrypt(e.to_string()))?;
    Ok(Zeroizing::new(plaintext))
}

/// Decode a stored base64 32-byte project key.
fn decode_key(key_b64: &str) -> CliResult<[u8; 32]> {
    let bytes = b64::decode(key_b64)?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CliError::MissingProjectKey("invalid stored key length".into()))?;
    Ok(arr)
}

/// The AEAD envelope is self-describing (nonce embedded), so the CLI uses a
/// fixed zero nonce for deterministic, key-only secrets. Each value is
/// encrypted under a unique key material, so nonce reuse across values is not
/// a concern here.
fn zero_nonce() -> [u8; aead::NONCE_LEN] {
    [0u8; aead::NONCE_LEN]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let key = generate_project_key_b64();
        let value = b"sup3r-s3cret";
        let ct = encrypt_value(&key, value).unwrap();
        assert_ne!(ct, "c3VwM3ItczNjcmV0");
        let pt = decrypt_value(&key, &ct).unwrap();
        assert_eq!(&*pt, value);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = generate_project_key_b64();
        let key2 = generate_project_key_b64();
        let ct = encrypt_value(&key1, b"hello").unwrap();
        assert!(decrypt_value(&key2, &ct).is_err());
    }

    #[test]
    fn generated_keys_are_32_bytes() {
        assert_eq!(b64::decode(&generate_project_key_b64()).unwrap().len(), 32);
    }
}
