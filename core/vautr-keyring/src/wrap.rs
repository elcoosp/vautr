use uuid::Uuid;
use vautr_crypto::aead;
use vautr_crypto::recovery;
/// SVK wrapping under KEK (MP) and KEK_RK (Recovery Key). REQ-RECOVERY-02.
/// Uses XChaCha20-Poly1305 with AD = (uuid=server_user_id, enc_key_gen=0).
///
/// VTRFIX (AD convention, documented): the MP-wrapped SVK blob is bound to the
/// legacy nil-UUID AD. Every client surface (desktop, CLI, FFI, wasm/SDK)
/// wraps and unwraps with this same convention (`unlock_with_password`
/// receives `Uuid::nil()` as its AD from the desktop, and the orchestrator's
/// rotation re-wrap path mirrors it), so blobs are portable across clients.
/// The RK-wrapped blob may carry either the nil AD (registration-time wraps)
/// or the real server user id (post-recovery rotation wraps) — `recover_svk`
/// tries both. Do NOT introduce a third convention without updating every
/// wrap/unwrap site at once.
use zeroize::Zeroizing;

/// Wrap the SVK under a KEK for server storage.
pub fn wrap_svk(kek: &Zeroizing<[u8; 32]>, svk: &Zeroizing<[u8; 32]>) -> Vec<u8> {
    // user_id-bound AD; for the wrapped-SVK blob enc_key_gen is 0. The nil
    // UUID is the documented cross-client AD convention (see module docs).
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

/// VTRFIX-SEC-H02: fallible unwrap. Returns `Err` on a wrong mnemonic, wrong
/// server_user_id, tampered ciphertext, or a malformed blob.
pub fn try_unwrap_svk_with_rk(
    wrapped_svk_rk: &[u8],
    kek_rk: &Zeroizing<[u8; 32]>,
    server_user_id: &Uuid,
) -> Result<Zeroizing<[u8; 32]>, vautr_crypto::error::CryptoError> {
    recovery::unwrap_svk_with_rk(wrapped_svk_rk, kek_rk, server_user_id)
}
