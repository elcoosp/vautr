//! Fuzz target for `vautr_crypto::aead::decrypt`.
//!
//! Contract under test (VTR-037): `decrypt` must NEVER panic on arbitrary
//! input. It must return `Err(CryptoError)` for malformed/tampered ciphertext
//! and only `Ok` for a genuine envelope produced by `encrypt`.
//!
//! Run locally:
//!   cargo +nightly fuzz run aead_decrypt           # until crash / manual stop
//!   cargo +nightly fuzz run aead_decrypt -- -max_total_time=3600   # 1 hour (CI-bound)
//!
//! A regression corpus is committed under `corpus/aead_decrypt/`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use uuid::Uuid;
use vautr_crypto::aead;

fuzz_target!(|data: &[u8]| {
    // Fixed, well-formed key + binding so we exercise the decrypt path itself,
    // not the AD construction. Randomness comes entirely from `data` (the
    // ciphertext envelope fed to the target).
    let key = [0x42u8; 32];
    let uuid = Uuid::nil();
    let enc_key_gen = 1u64;

    // The only hard requirement: this call must not panic. `Result` is the
    // success/failure channel; a panic here would be a fuzzing regression.
    let _ = aead::decrypt(&key, &uuid, enc_key_gen, data);
});
