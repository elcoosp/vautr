//! Key-tree derivation: MK → KEK → SVK → OEK/DEK.
//!
//! Spec: [`docs/architecture/crypto.md`] §2, Steps 3–5.
//! HKDF-HMAC-SHA256 with fixed salt/info strings. SVK is randomly generated.

use crate::error::{CryptoError, Result};
use crate::kdf::MK_LEN;
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

const SALT_KEK: &[u8] = b"Vautr-kek-salt";
const INFO_KEK: &[u8] = b"Vautr-kek";
const SALT_OEK: &[u8] = b"Vautr-oek-salt";
const INFO_OEK: &[u8] = b"Vautr-oek";
const SALT_DEK: &[u8] = b"vautr-dek-salt";
const INFO_DEK: &[u8] = b"Vautr-dek";

/// Key Encryption Key — unwraps the SVK blob fetched from the server.
pub fn derive_kek(mk: &Zeroizing<[u8; MK_LEN]>) -> Result<Zeroizing<[u8; 32]>> {
    expand(&**mk, SALT_KEK, INFO_KEK)
}

/// Overview Encryption Key — encrypts the plaintext `DecryptedOverview`.
pub fn derive_oek(svk: &Zeroizing<[u8; 32]>) -> Result<Zeroizing<[u8; 32]>> {
    expand(&**svk, SALT_OEK, INFO_OEK)
}

/// Data Encryption Key — encrypts the `DecryptedSecret` payload.
pub fn derive_dek(svk: &Zeroizing<[u8; 32]>) -> Result<Zeroizing<[u8; 32]>> {
    expand(&**svk, SALT_DEK, INFO_DEK)
}

/// Generate a fresh random Symmetric Vault Key.
pub fn generate_svk() -> Zeroizing<[u8; 32]> {
    let mut svk = Zeroizing::new([0u8; 32]);
    rand::thread_rng().fill_bytes(&mut *svk);
    svk
}

fn expand(ikm: &[u8], salt: &[u8], info: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(info, &mut *okm)
        .map_err(|e| CryptoError::Internal(e.to_string()))?;
    Ok(okm)
}

/// Re-export the SVK length for callers.
pub const SVK_LEN: usize = 32;
