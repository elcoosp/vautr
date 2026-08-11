//! Emergency Recovery Key handling (BIP-39 24-word mnemonic).
//!
//! Spec: REQ-RECOVERY-01/02, BR-7, [`docs/architecture/crypto.md`] §2 & §6.
//!
//! Recovery key path (this closes gap #4):
//!   1. `mnemonic` (24 words) → BIP-39 seed (64 bytes).
//!   2. `KEK_RK = HKDF-SHA256(seed, salt="Vautr-kek-rk-salt", info="Vautr-kek-rk", L=32)`.
//!   3. The SVK is sealed under KEK_RK with AD = (server_user_id, enc_key_gen=0),
//!      producing `WrappedSVK_RK` stored server-side in `users.svk_ciphertext_blob_rk`
//!      (see docs/architecture/server-db.md §3). On recovery the user enters the
//!      24 words, KEK_RK is re-derived, and `WrappedSVK_RK` is decrypted to recover
//!      the SVK without the Master Password (REQ-RECOVERY-01/02).

use crate::aead;
use crate::error::{CryptoError, Result};
use crate::kdf::MK_LEN;
use bip39::{Language, Mnemonic};
use hkdf::Hkdf;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Info string for the recovery KEK derivation (crypto.md §6).
const RK_INFO: &[u8] = b"Vautr-kek-rk";
/// Salt for the recovery KEK derivation (crypto.md §6).
const RK_SALT: &[u8] = b"Vautr-kek-rk-salt";
/// `enc_key_gen` bound into the RK-wrapped SVK envelope (always 0 for RK).
const RK_ENC_KEY_GEN: u64 = 0;

/// Generate a fresh 24-word BIP-39 Recovery Key (no passphrase).
pub fn generate_recovery_mnemonic() -> Result<String> {
    let m = Mnemonic::generate_in(Language::English, 24)
        .map_err(|e| CryptoError::RecoveryError(e.to_string()))?;
    Ok(m.to_string())
}

/// Decode a 24-word mnemonic into its entropy-backed phrase (validates checksum).
pub fn decode_recovery_mnemonic(words: &str) -> Result<Mnemonic> {
    Mnemonic::parse_in(Language::English, words.trim())
        .map_err(|e| CryptoError::RecoveryError(e.to_string()))
}

/// Derive the Recovery Key Encryption Key (KEK_RK) from a recovery mnemonic.
///
/// `KEK_RK = HKDF-SHA256(BIP39_seed, salt, info, 32)` (crypto.md §6, step 2).
pub fn derive_kek_rk(mnemonic: &Mnemonic) -> Result<Zeroizing<[u8; MK_LEN]>> {
    let seed = mnemonic.to_seed(""); // no passphrase (BR-7)
    let hk = Hkdf::<sha2::Sha256>::new(Some(RK_SALT), &seed[..]);
    let mut okm = Zeroizing::new([0u8; MK_LEN]);
    hk.expand(RK_INFO, &mut *okm)
        .map_err(|e| CryptoError::RecoveryError(format!("KEK_RK expand failed: {e}")))?;
    Ok(okm)
}

/// Wrap the SVK under KEK_RK for server storage (REQ-RECOVERY-02).
///
/// AD binds `(server_user_id, enc_key_gen=0)` so the blob is item/user-scoped.
pub fn wrap_svk_with_rk(
    svk: &[u8; MK_LEN],
    kek_rk: &[u8; MK_LEN],
    server_user_id: &Uuid,
) -> Result<Vec<u8>> {
    aead::encrypt(kek_rk, server_user_id, RK_ENC_KEY_GEN, svk)
        .map_err(|e| CryptoError::RecoveryError(format!("RK wrap failed: {e}")))
}

/// Recover the SVK from its RK-wrapped blob using the derived KEK_RK.
pub fn unwrap_svk_with_rk(
    wrapped_svk_rk: &[u8],
    kek_rk: &[u8; MK_LEN],
    server_user_id: &Uuid,
) -> Result<Zeroizing<[u8; MK_LEN]>> {
    let pt = aead::decrypt(kek_rk, server_user_id, RK_ENC_KEY_GEN, wrapped_svk_rk)
        .map_err(|_| CryptoError::RecoveryError("RK unwrap failed (bad mnemonic?)".into()))?;
    if pt.len() != MK_LEN {
        return Err(CryptoError::MalformedCiphertext);
    }
    let mut svk = Zeroizing::new([0u8; MK_LEN]);
    svk.copy_from_slice(&pt);
    Ok(svk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_roundtrip() {
        let mnemonic = decode_recovery_mnemonic(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
        )
        .expect("valid 24-word mnemonic");
        let kek_rk = derive_kek_rk(&mnemonic).unwrap();
        let svk = [7u8; MK_LEN];
        let user_id = Uuid::nil();
        let wrapped = wrap_svk_with_rk(&svk, &kek_rk, &user_id).unwrap();
        let recovered = unwrap_svk_with_rk(&wrapped, &kek_rk, &user_id).unwrap();
        assert_eq!(&*recovered, &svk);

        // Wrong mnemonic-derived key must fail (TagMismatch → RecoveryError).
        let other = decode_recovery_mnemonic(
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
        )
        .unwrap();
        let bad_kek = derive_kek_rk(&other).unwrap();
        assert!(unwrap_svk_with_rk(&wrapped, &bad_kek, &user_id).is_err());
    }
}
