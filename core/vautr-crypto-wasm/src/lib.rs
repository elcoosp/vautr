//! Stateless crypto wasm module for the Manifest V3 extension Service Worker.
//!
//! Build-env-deploy §3.3: The SW is 100% stateless — it wakes on autofill,
//! decrypts ONE item, and terminates. This module exposes just the two functions
//! the SW needs: `init()` and `decrypt_secret_with_svk()`.
//!
//! Built with `wasm-pack build --target nodejs`.

use wasm_bindgen::prelude::*;

use vautr_crypto::aead;
use vautr_crypto::key_tree;
use zeroize::Zeroizing;

/// Convert a byte slice into a fixed 32-byte array.
fn to_arr32(bytes: &[u8]) -> Result<[u8; 32], String> {
    let mut out = [0u8; 32];
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(bytes);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Plain Rust core (testable natively)
// ---------------------------------------------------------------------------

/// Derive DEK from SVK, then AEAD-decrypt the item payload and parse out the
/// `password` field from the JSON plaintext.
pub fn decrypt_secret_with_svk_core(
    svk: &[u8],
    uuid: &str,
    enc_key_gen: u64,
    payload: &[u8],
) -> Result<String, String> {
    let svk = Zeroizing::new(to_arr32(svk)?);
    let dek = key_tree::derive_dek(&svk).map_err(|e| e.to_string())?;
    let item_uuid = uuid::Uuid::parse_str(uuid).map_err(|e| e.to_string())?;
    let pt =
        aead::decrypt(&dek, &item_uuid, enc_key_gen, payload).map_err(|e| format!("decrypt: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_slice(&pt).map_err(|e| format!("json parse: {e}"))?;
    let password = json["password"]
        .as_str()
        .ok_or_else(|| "missing 'password' field in item plaintext".to_string())?;
    Ok(password.to_string())
}

// ---------------------------------------------------------------------------
// wasm-bindgen wrappers
// ---------------------------------------------------------------------------

/// One-time WASM initialisation (required by wasm-pack `--target nodejs` glue).
#[wasm_bindgen]
pub fn init() {
    // no-op; the wasm-pack glue uses this to know the module is ready.
}

/// Statelessly decrypt a single item, returning the password string.
/// Called by the Service Worker on every autofill wake.
#[wasm_bindgen]
pub fn decrypt_secret_with_svk(
    svk: Vec<u8>,
    uuid: &str,
    enc_key_gen: u64,
    payload: Vec<u8>,
) -> Result<String, JsValue> {
    decrypt_secret_with_svk_core(&svk, uuid, enc_key_gen, &payload)
        .map_err(|e| JsValue::from_str(&e))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vautr_crypto::kdf;

    #[test]
    fn svk_decrypt_roundtrip() {
        let salt = kdf::generate_kdf_salt();
        let mk = kdf::derive_master_key("hunter2", &salt).unwrap();
        let _kek = key_tree::derive_kek(&mk).unwrap();
        let svk = *key_tree::generate_svk(); // [u8; 32]

        let uuid = uuid::Uuid::new_v4();
        let plaintext = br#"{"title":"Example","subtitle":"user@example.com","iconKey":"key","urls":["https://example.com"],"password":"s3cret-pass"}"#;
        let dek = key_tree::derive_dek(&Zeroizing::new(svk)).unwrap();
        let payload = aead::encrypt(&dek, &uuid, 1, plaintext).unwrap();

        let recovered = decrypt_secret_with_svk_core(&svk, &uuid.to_string(), 1, &payload).unwrap();
        assert_eq!(recovered, "s3cret-pass");
    }

    #[test]
    fn wrong_svk_fails() {
        let svk = *key_tree::generate_svk();
        let wrong_svk = *key_tree::generate_svk();
        let dek = key_tree::derive_dek(&Zeroizing::new(svk)).unwrap();
        let uuid = uuid::Uuid::new_v4();
        let payload = aead::encrypt(&dek, &uuid, 1, b"{\"password\":\"x\"}").unwrap();
        assert!(decrypt_secret_with_svk_core(&wrong_svk, &uuid.to_string(), 1, &payload).is_err());
    }

    #[test]
    fn wrong_enc_key_gen_fails() {
        let svk = *key_tree::generate_svk();
        let dek = key_tree::derive_dek(&Zeroizing::new(svk)).unwrap();
        let uuid = uuid::Uuid::new_v4();
        let payload = aead::encrypt(&dek, &uuid, 1, b"{\"password\":\"x\"}").unwrap();
        assert!(decrypt_secret_with_svk_core(&svk, &uuid.to_string(), 2, &payload).is_err());
    }
}
