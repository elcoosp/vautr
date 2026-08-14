//! wasm-bindgen wrappers for secure item sharing (ADR-007 / sharing-pki.md).
//!
//! These delegate to `vautr_sharing` (the pure-logic, no-I/O sharing crate) so
//! the SIK generation, DEM, and KEM steps are identical to the desktop client.
//! The server is an untrusted relay: these functions only ever touch public
//! keys, KEM envelopes (`wrapped_sik` + `ephemeral_public_key`) and the
//! recipient's own sharing secret key. The SIK and plaintext never leave the
//! wasm module except as the unwrapped plaintext (returned to the calling
//! client to ingest a received item locally).
//!
//! Wire format: each wrapper returns a JSON string of the corresponding
//! `vautr_sharing` struct (base64 fields), which the TS SDK forwards to the
//! server / parses from the inbox.

use wasm_bindgen::prelude::*;

use base64::Engine;
use vautr_crypto::sharing::SharingKeyPair;
use vautr_sharing::{accept_share as vs_accept_share, share_item as vs_share_item, IncomingShare};

fn parse_keypair(secret_b64: &str) -> Result<SharingKeyPair, String> {
    let secret = decode_b64(secret_b64)?;
    if secret.len() != 32 {
        return Err(format!(
            "sharing secret must be 32 bytes, got {}",
            secret.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&secret);
    Ok(SharingKeyPair::from_secret(arr))
}

fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("base64 decode: {e}"))
}

/// Generate a fresh sharing keypair. Returns a JSON object
/// `{ public: <b64>, secret: <b64> }`. The caller stores `secret` (e.g.
/// MP-encrypted in local storage) and reconstructs the keypair with
/// [`restore_sharing_keypair`] to unwrap received shares.
#[wasm_bindgen]
pub fn generate_sharing_keypair() -> Result<String, JsValue> {
    let kp = SharingKeyPair::generate();
    let out = serde_json::json!({
        "public": base64::engine::general_purpose::STANDARD.encode(kp.public),
        "secret": base64::engine::general_purpose::STANDARD.encode(kp.secret_bytes()),
    });
    serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Reconstruct a keypair from persisted secret bytes (base64).
#[wasm_bindgen]
pub fn restore_sharing_keypair(secret_b64: &str) -> Result<String, JsValue> {
    let kp = parse_keypair(secret_b64).map_err(|e| JsValue::from_str(&e))?;
    let out = serde_json::json!({
        "public": base64::engine::general_purpose::STANDARD.encode(kp.public),
        "secret": base64::engine::general_purpose::STANDARD.encode(kp.secret_bytes()),
    });
    serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Build a 1:1 share of `plaintext` for `recipient_public_b64` (base64 X25519
/// key), bound to `item_uuid`. Returns a `vautr_sharing::ShareBundle` JSON:
/// `{ share_id, sender_uuid, recipient_uuid, item_uuid, wrapped_sik,
/// ephemeral_public_key, encrypted_payload }` (all base64 except uuids). The
/// caller POSTs `wrapped_sik` + `ephemeral_public_key` to `POST /shares/` and
/// `encrypted_payload` to `POST /shares/{item_uuid}/payload`.
#[wasm_bindgen]
pub fn share_item(
    sender_uuid: &str,
    recipient_uuid: &str,
    item_uuid: &str,
    recipient_public_b64: &str,
    plaintext: &[u8],
) -> Result<String, JsValue> {
    let sender =
        uuid::Uuid::parse_str(sender_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let recipient =
        uuid::Uuid::parse_str(recipient_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let item = uuid::Uuid::parse_str(item_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let recipient_pk = decode_b64(recipient_public_b64)?;
    if recipient_pk.len() != 32 {
        return Err(JsValue::from_str("recipient public key must be 32 bytes"));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&recipient_pk);

    let bundle = vs_share_item(sender, recipient, item, &pk, plaintext)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&bundle).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decrypt a received share. `incoming` is the `IncomingShare` JSON from
/// `GET /shares/inbox` (base64 fields). Returns the recovered plaintext bytes.
#[wasm_bindgen]
pub fn accept_share(incoming_json: &str, recipient_secret_b64: &str) -> Result<Vec<u8>, JsValue> {
    let incoming: IncomingShare =
        serde_json::from_str(incoming_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let kp = parse_keypair(recipient_secret_b64).map_err(|e| JsValue::from_str(&e))?;
    vs_accept_share(&kp, &incoming).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_generate_and_restore_roundtrip() {
        let generated = generate_sharing_keypair().unwrap();
        let v: serde_json::Value = serde_json::from_str(&generated).unwrap();
        let secret = v["secret"].as_str().unwrap();
        let restored = restore_sharing_keypair(secret).unwrap();
        let r: serde_json::Value = serde_json::from_str(&restored).unwrap();
        assert_eq!(v["public"], r["public"]);
        assert_eq!(v["secret"], r["secret"]);
    }

    #[test]
    fn share_and_accept_roundtrip() {
        let owner = generate_sharing_keypair().unwrap();
        let recipient = generate_sharing_keypair().unwrap();
        let o: serde_json::Value = serde_json::from_str(&owner).unwrap();
        let r: serde_json::Value = serde_json::from_str(&recipient).unwrap();

        let item_uuid = uuid::Uuid::new_v4();
        let plaintext = b"vault item secret payload";
        let bundle = share_item(
            &uuid::Uuid::new_v4().to_string(),
            &uuid::Uuid::new_v4().to_string(),
            &item_uuid.to_string(),
            r["public"].as_str().unwrap(),
            plaintext,
        )
        .unwrap();
        let b: serde_json::Value = serde_json::from_str(&bundle).unwrap();
        // Recipient rebuilds the IncomingShare JSON from the bundle.
        let incoming = serde_json::json!({
            "share_id": b["share_id"],
            "sender_uuid": b["sender_uuid"],
            "item_uuid": b["item_uuid"],
            "wrapped_sik": b["wrapped_sik"],
            "ephemeral_public_key": b["ephemeral_public_key"],
            "encrypted_payload": b["encrypted_payload"],
        });
        let recovered = accept_share(&incoming.to_string(), r["secret"].as_str().unwrap()).unwrap();
        assert_eq!(recovered, plaintext);
    }
}
