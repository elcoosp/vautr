//! SVK wrapping under KEK (MP) and KEK_RK (Recovery Key). REQ-RECOVERY-02.
//! Uses XChaCha20-Poly1305 with AD = (uuid=server_user_id, enc_key_gen=0).

use zeroize::Zeroizing;
use vautr_crypto::aead;
use vautr_crypto::recovery;
use uuid::Uuid;

/// Wrap the SVK under a KEK for server storage.
pub fn wrap_svk(kek: &Zeroizing<[u8; 32]>, svk: &Zeroizing<[u8; 32]>) -> Vec<u8> {
    // user_id-bound AD; for the wrapped-SVK blob enc_key_gen is 0.
    let user_id = Uuid::nil();
    aead::encrypt(kek, &user_id, 0, &**svk).expect("svk wrap")
}

/// Wrap the SVK under the Recovery Key (KEK_RK) for recovery-based unlock.
pub fn wrap_svk_with_rk(
    svk: &Zeroizing<[u8; 32]>,
    kek_rk: &Zeroizing<[u8; 32]>,
    server_user_id: &Uuid,
) -> Vec<u8> {
    recovery::wrap_svk_with_rk(svk, kek_rk, server_user_id).expect("svk rk wrap")
}

/// Unwrap the SVK from its RK-wrapped blob (emergency recovery, REQ-RECOVERY-01/02).
///
/// # Panics
/// This is the historical, non-fallible API. A wrong mnemonic or tampered
/// blob is an *expected* error path, not a bug: callers must use
/// [`try_unwrap_svk_with_rk`] instead. Kept only for signatures that cannot
/// yet propagate a `Result`.
#[deprecated(note = "use try_unwrap_svk_with_rk; a wrong mnemonic must not panic (VTRFIX-SEC-H02)")]
pub fn unwrap_svk_with_rk(
    wrapped_svk_rk: &[u8],
    kek_rk: &Zeroizing<[u8; 32]>,
    server_user_id: &Uuid,
) -> Zeroizing<[u8; 32]> {
    try_unwrap_svk_with_rk(wrapped_svk_rk, kek_rk, server_user_id)
        .expect("svk rk unwrap (unexpected — this path should use try_unwrap_svk_with_rk)")
}

/// VTRFIX-SEC-H02: fallible unwrap. Returns `Err` on a wrong mnemonic, wrong
/// server_user_id, tampered ciphertext, or a malformed blob.
pub fn try_unwrap_svk_with_rk(
    wrapped_svk_rk: &[u8],
    kek_rk: &Zeroizing<[u8; 32]>,
    server_user_id: &Uuid,
) -> Result<Zeroizing<[u8; 32]>, vautr_crypto::error::CryptoError> {
    recovery::unwrap_svk_with_rk(wrapped_svk_rk, kek_rk, server_user_id)
}
