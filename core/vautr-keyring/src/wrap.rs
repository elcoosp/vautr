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
pub fn unwrap_svk_with_rk(
    wrapped_svk_rk: &[u8],
    kek_rk: &Zeroizing<[u8; 32]>,
    server_user_id: &Uuid,
) -> Zeroizing<[u8; 32]> {
    recovery::unwrap_svk_with_rk(wrapped_svk_rk, kek_rk, server_user_id).expect("svk rk unwrap")
}
