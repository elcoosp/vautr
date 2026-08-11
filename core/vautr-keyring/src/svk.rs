//! SVK generation and OEK/DEK derivation. crypto.md §2.4–2.5.
//! Wraps `vautr-crypto::key_tree`.

use zeroize::Zeroizing;
use vautr_crypto::key_tree;

/// Generate a fresh SVK and derive its contextual domain keys (OEK, DEK).
pub fn new_vault_keys() -> (
    Zeroizing<[u8; 32]>,
    Zeroizing<[u8; 32]>,
    Zeroizing<[u8; 32]>,
) {
    let svk = key_tree::generate_svk();
    let oek = key_tree::derive_oek(&svk).expect("oek derive");
    let dek = key_tree::derive_dek(&svk).expect("dek derive");
    (svk, oek, dek)
}
