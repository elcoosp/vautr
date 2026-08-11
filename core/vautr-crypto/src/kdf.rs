//! Master Key (MK) derivation via Argon2id.
//!
//! Spec: [`docs/architecture/crypto.md`] §2.2, Step 1.
//! Baseline params: m_cost=64 MiB, t_cost=3, p_cost=4, output_len=32.
//! KDF salt: 32-byte CSPRNG, stored unencrypted per-user on the server.

use crate::error::{CryptoError, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroizing;

/// Argon2id baseline parameters (crypto.md §2.2).
pub const ARGON2_M_COST: u32 = 64 * 1024; // 64 MiB in KiB
pub const ARGON2_T_COST: u32 = 3;
pub const ARGON2_P_COST: u32 = 4;
pub const MK_LEN: usize = 32;

/// A freshly generated KDF salt (32 bytes).
pub fn generate_kdf_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    // CSPRNG from the `rand` crate (OsRng-backed).
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Derive the Master Key from MP + per-user salt.
///
/// Returns `Zeroizing<[u8; 32]>` so the MK is wiped on drop.
pub fn derive_master_key(mp: &str, salt: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(
        ARGON2_M_COST,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(MK_LEN),
    )
    .map_err(|e| CryptoError::KdfError(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut raw = Zeroizing::new([0u8; MK_LEN]);
    argon2
        .hash_password_into(mp.as_bytes(), salt, &mut *raw)
        .map_err(|e| CryptoError::KdfError(e.to_string()))?;
    Ok(raw)
}
