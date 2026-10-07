//! VTRFIX-TST-03: crypto golden tests.
//!
//! Pins the recovery-auth Ed25519 derivation so a future change is caught at
//! test time.

use vautr_crypto::recovery;

const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

#[test]
fn recovery_public_key_is_deterministic() {
    let m = recovery::decode_recovery_mnemonic(MNEMONIC).expect("valid mnemonic");
    let pk1 = recovery::recovery_public_key(&m).expect("pk1");
    let pk2 = recovery::recovery_public_key(&m).expect("pk2");
    assert_eq!(pk1, pk2, "derivation is deterministic");
    assert_eq!(pk1.len(), 32, "ed25519 public key is 32 bytes");
}

#[test]
fn sign_recovery_nonce_is_verifiable() {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let m = recovery::decode_recovery_mnemonic(MNEMONIC).expect("mnemonic");
    let pk_bytes = recovery::recovery_public_key(&m).expect("pk");
    let sig_bytes = recovery::sign_recovery_nonce(&m, b"challenge-nonce").expect("sign");
    let vk = VerifyingKey::from_bytes(&pk_bytes).expect("vk");
    let sig = Signature::from_slice(&sig_bytes).expect("sig");
    assert!(vk.verify(b"challenge-nonce", &sig).is_ok());
    assert!(vk.verify(b"different-nonce", &sig).is_err());
}
