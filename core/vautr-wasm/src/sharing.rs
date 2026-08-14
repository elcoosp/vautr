//! wasm-bindgen wrappers for secure item sharing (ADR-007).
//!
//! The server is an untrusted relay: these functions only ever touch public
//! keys, KEM envelopes (`wrapped_sik` + `ephemeral_public_key`) and the
//! recipient's own sharing secret key. The SIK and plaintext never leave the
//! wasm module except as the unwrapped SIK (returned to the calling client to
//! decrypt a received item locally).
//!
//! Byte layouts at the boundary (all little helpers; the TS SDK base64-encodes
//! for the JSON wire):
//!   * `generate_sharing_keypair` -> `public(32) ‖ secret(32)` (64 bytes)
//!   * `share_item` -> `wrapped_sik ‖ ephemeral_public_key(32)`
//!   * `unwrap_shared_item` -> `sik(32)`

use wasm_bindgen::prelude::*;

use vautr_crypto::sharing::{SharedEnvelope, SharingKeyPair};

fn arr32(bytes: &[u8]) -> Result<[u8; 32], String> {
    let mut out = [0u8; 32];
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Generate a fresh sharing keypair. Returns `public(32) ‖ secret(32)`. The
/// caller stores `secret` (MP-encrypted in local storage) and reconstructs the
/// keypair with [`restore_sharing_keypair`] to unwrap received shares.
#[wasm_bindgen]
pub fn generate_sharing_keypair() -> Result<Vec<u8>, JsValue> {
    let kp = SharingKeyPair::generate();
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&kp.public);
    out.extend_from_slice(&kp.secret_bytes());
    Ok(out)
}

/// Reconstruct a keypair from persisted secret bytes (32 bytes). Returns
/// `public(32) ‖ secret(32)` (same layout as [`generate_sharing_keypair`]).
#[wasm_bindgen]
pub fn restore_sharing_keypair(secret: &[u8]) -> Result<Vec<u8>, JsValue> {
    let secret = arr32(secret).map_err(|e| JsValue::from_str(&e))?;
    let kp = SharingKeyPair::from_secret(secret);
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&kp.public);
    out.extend_from_slice(&kp.secret_bytes());
    Ok(out)
}

/// Wrap `sik` (32 bytes) for `recipient_public` (32 bytes) bound to
/// `item_uuid`. Returns `wrapped_sik ‖ ephemeral_public_key(32)` — exactly the
/// bytes the server stores (`POST /shares/`).
#[wasm_bindgen]
pub fn share_item(
    sik: &[u8],
    recipient_public: &[u8],
    item_uuid: &str,
) -> Result<Vec<u8>, JsValue> {
    let sik = arr32(sik).map_err(|e| JsValue::from_str(&e))?;
    let recipient_pk = arr32(recipient_public).map_err(|e| JsValue::from_str(&e))?;
    let uuid = uuid::Uuid::parse_str(item_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let envelope = vautr_crypto::sharing::share_item(&sik, &recipient_pk, &uuid)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut out = Vec::with_capacity(envelope.wrapped_sik.len() + 32);
    out.extend_from_slice(&envelope.wrapped_sik);
    out.extend_from_slice(&envelope.ephemeral_public_key);
    Ok(out)
}

/// Unwrap a received share envelope to recover the SIK (32 bytes). The
/// recipient passes `wrapped_sik`, `ephemeral_public_key` (32), their
/// `recipient_secret` (32), and `item_uuid`.
#[wasm_bindgen]
pub fn unwrap_shared_item(
    wrapped_sik: &[u8],
    ephemeral_public_key: &[u8],
    recipient_secret: &[u8],
    item_uuid: &str,
) -> Result<Vec<u8>, JsValue> {
    let ephemeral_public_key = arr32(ephemeral_public_key).map_err(|e| JsValue::from_str(&e))?;
    let secret = arr32(recipient_secret).map_err(|e| JsValue::from_str(&e))?;
    let uuid = uuid::Uuid::parse_str(item_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let envelope = SharedEnvelope {
        wrapped_sik: wrapped_sik.to_vec(),
        ephemeral_public_key,
    };
    let sik = vautr_crypto::sharing::unwrap_shared_item(
        &envelope,
        &SharingKeyPair::from_secret(secret),
        &uuid,
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(sik.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vautr_crypto::kdf::MK_LEN;

    #[test]
    fn keypair_generate_and_restore_roundtrip() {
        let generated = generate_sharing_keypair().unwrap();
        assert_eq!(generated.len(), 64);
        let restored = restore_sharing_keypair(&generated[32..]).unwrap();
        assert_eq!(generated, restored);
    }

    #[test]
    fn share_and_unwrap_roundtrip() {
        let owner = generate_sharing_keypair().unwrap();
        let recipient = generate_sharing_keypair().unwrap();
        let sik = vec![5u8; MK_LEN];
        let item_uuid = uuid::Uuid::new_v4().to_string();

        let env = share_item(&sik, &recipient[..32], &item_uuid).unwrap();
        assert!(env.len() > 32);
        let wrapped = &env[..env.len() - 32];
        let ephemeral = &env[env.len() - 32..];
        let recovered =
            unwrap_shared_item(wrapped, ephemeral, &recipient[32..], &item_uuid).unwrap();
        assert_eq!(recovered, sik);
    }
}
