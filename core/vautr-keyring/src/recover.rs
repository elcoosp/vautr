//! Emergency recovery unwrap. REQ-RECOVERY-01/02.
//! Uses `vautr-crypto::recovery::derive_kek_rk` (KEK_RK path defined in crypto.md §7).

use zeroize::Zeroizing;
use vautr_crypto::recovery;
use uuid::Uuid;

/// Decode a 24-word Recovery Key mnemonic (validates checksum).
pub fn validate_recovery_key(words: &str) -> bool {
    recovery::decode_recovery_mnemonic(words).is_ok()
}

/// Recover the SVK from a 24-word Recovery Key + the server-stored RK-wrapped blob.
///
/// This bypasses the Master Password entirely (REQ-RECOVERY-01): the mnemonic
/// derives KEK_RK, which unwraps `svk_ciphertext_blob_rk` to yield the SVK. The
/// caller can then re-wrap the SVK under a fresh Master Password (MP reset).
pub fn recover_svk(
    mnemonic_words: &str,
    wrapped_svk_rk: &[u8],
    server_user_id: &Uuid,
) -> Option<Zeroizing<[u8; 32]>> {
    let mnemonic = recovery::decode_recovery_mnemonic(mnemonic_words).ok()?;
    let kek_rk = recovery::derive_kek_rk(&mnemonic).ok()?;
    recovery::unwrap_svk_with_rk(wrapped_svk_rk, &kek_rk, server_user_id).ok()
}
