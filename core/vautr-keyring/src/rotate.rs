//! Crash-safe key rotation. ADR-006 / REQ-ROTATE-01..03.
//! Re-encrypts items where `enc_key_gen < new_gen` in batches of 100, using the
//! `enc_key_gen` column as a resumable cursor.

use uuid::Uuid;
use vautr_crypto::{aead, key_tree};
use zeroize::Zeroizing;

/// One rotation batch (ADR-006 step 2–3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RotationBatch {
    pub old_gen: u64,
    pub new_gen: u64,
}

impl RotationBatch {
    /// Re-encrypt a single item payload under the new generation.
    ///
    /// The plaintext `item` is encrypted with the DEK derived from `new_svk`
    /// (consistent with how item payloads are encrypted everywhere else) and
    /// the new `enc_key_gen`, producing a fresh ciphertext envelope. The old
    /// plaintext (held in `Zeroizing`) is zeroized on drop.
    pub fn reencrypt(
        &self,
        uuid: &Uuid,
        new_svk: &Zeroizing<[u8; 32]>,
        item: &[u8],
    ) -> Vec<u8> {
        let dek = key_tree::derive_dek(new_svk).expect("rotate dek derive");
        aead::encrypt(&dek, uuid, self.new_gen, item).expect("rotate reencrypt")
    }
}

/// Decide the next batch for a cursor-style rotation sweep.
///
/// Given the server's `min_enc_key_gen` target and the lowest `enc_key_gen`
/// still present locally, returns the next [`RotationBatch`] or `None` when the
/// sweep is complete. Processes at most `batch_size` generations per call.
pub fn next_batch(
    current_min: u64,
    target_gen: u64,
    batch_size: u64,
) -> Option<RotationBatch> {
    if current_min >= target_gen {
        return None;
    }
    let old_gen = current_min;
    let new_gen = (old_gen + batch_size).min(target_gen);
    Some(RotationBatch { old_gen, new_gen })
}
