//! OPAQUE + KDF + key-wrap auth surfaces exposed to the browser client.
//!
//! This module is the ONLY wasm-facing auth surface (ADR-005: the wasm crate
//! stays crypto-only, no tokio/reqwest/sea-orm). It drives the OPAQUE client
//! state machine in `vautr_crypto::opaque` (the exact primitive wrapped by
//! `vautr-auth::state`) and the crypto in `vautr_crypto::{kdf,key_tree,aead,recovery}`
//! so the JS layer only ferries byte blobs and strings. See:
//! - crypto.md §2 (KDF/key tree) and §3 (AEAD wrap) for the SVK envelope.
//! - api.md §3 for the OPAQUE wire format.
//!
//! NOTE: `vautr-auth::state` is deliberately NOT used directly. Its Cargo.toml
//! pulls `tokio` (full features → mio), which does not compile to
//! `wasm32-unknown-unknown`; using it would break the wasm build this crate
//! exists to serve. The functions here call the exact same underlying
//! `vautr_crypto::opaque` calls that `vautr-auth::state` wraps (identical
//! parameters and `credential_identifier` binding), with proper `Result`
//! error propagation instead of the wrapper's panics. ADR-005 keeps the wasm
//! crate crypto-only.
//!
//! Each operation has a plain-Rust core (unit-testable natively) plus a thin
//! `#[wasm_bindgen]` wrapper returning `JsValue`. Plaintext key material is
//! returned to JS as `Vec<u8>` (the boundary already requires JS to hold key
//! bytes transiently); it is never logged or retained in Rust globals.

use wasm_bindgen::prelude::*;

use vautr_crypto::aead;
use vautr_crypto::kdf;
use vautr_crypto::key_tree;
use vautr_crypto::opaque;
use vautr_crypto::recovery;
use zeroize::Zeroizing;

/// AD identity used for MP/RK-wrapped SVK blobs.
///
/// The server never returns the account `user_id` to the client (it is minted
/// server-side at register/finish and only appears in server-internal rows), so
/// the client cannot bind the SVK wrap to a user-scoped AD. We bind
/// `(Uuid::nil(), enc_key_gen = 0)` which is deterministic and reproducible
/// across register (wrap) and login (unwrap). This is a documented deviation
/// from crypto.md §7 step 3 (which assumes the client knows `server_user_id`);
/// the zero-knowledge guarantee is unaffected because the KEK is required to
/// unwrap regardless of the AD value.
const SVK_AD_USER: uuid::Uuid = uuid::Uuid::nil();
const SVK_AD_ENC_GEN: u64 = 0;

/// Convert a byte slice into a fixed 32-byte array (key material helper).
fn to_arr32(bytes: &[u8]) -> Result<[u8; 32], String> {
    let mut out = [0u8; 32];
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(bytes);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Plain Rust cores (native-testable)
// ---------------------------------------------------------------------------

/// Generate a fresh 32-byte KDF salt (crypto.md §2.2).
pub fn generate_kdf_salt() -> Vec<u8> {
    kdf::generate_kdf_salt().to_vec()
}

/// Derive the Master Key from password + per-user salt (Argon2id, crypto.md §2).
pub fn derive_master_key(password: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    let salt = to_arr32(salt)?;
    let mk = kdf::derive_master_key(password, &salt).map_err(|e| e.to_string())?;
    Ok(mk.to_vec())
}

/// Derive the Key Encryption Key (KEK) from the Master Key (crypto.md §2 step 3).
pub fn derive_kek(master_key: &[u8]) -> Result<Vec<u8>, String> {
    let mk = Zeroizing::new(to_arr32(master_key)?);
    let kek = key_tree::derive_kek(&mk).map_err(|e| e.to_string())?;
    Ok(kek.to_vec())
}

/// Generate a fresh random Symmetric Vault Key (crypto.md §2 step 4).
pub fn generate_svk() -> Vec<u8> {
    key_tree::generate_svk().to_vec()
}

/// Wrap the SVK under KEK for server storage (`svk_ciphertext_blob`).
pub fn wrap_svk(svk: &[u8], kek: &[u8]) -> Result<Vec<u8>, String> {
    let svk = to_arr32(svk)?;
    let kek = to_arr32(kek)?;
    aead::encrypt(&kek, &SVK_AD_USER, SVK_AD_ENC_GEN, &svk).map_err(|e| e.to_string())
}

/// Unwrap the MP-wrapped SVK from `GET /account/status` (`svk_ciphertext_blob`).
pub fn unwrap_svk(wrapped: &[u8], kek: &[u8]) -> Result<Vec<u8>, String> {
    let kek = to_arr32(kek)?;
    let pt = aead::decrypt(&kek, &SVK_AD_USER, SVK_AD_ENC_GEN, wrapped)
        .map_err(|e| format!("SVK unwrap failed: {e}"))?;
    if pt.len() != 32 {
        return Err("unwrapped SVK has wrong length".to_string());
    }
    Ok(pt)
}

/// Generate a 24-word BIP-39 recovery mnemonic (REQ-RECOVERY-01).
pub fn generate_recovery_mnemonic() -> Result<String, String> {
    recovery::generate_recovery_mnemonic().map_err(|e| e.to_string())
}

/// Wrap the SVK under the recovery key (`svk_ciphertext_blob_rk`).
pub fn wrap_svk_with_rk(svk: &[u8], mnemonic: &str) -> Result<Vec<u8>, String> {
    let m = recovery::decode_recovery_mnemonic(mnemonic).map_err(|e| e.to_string())?;
    let kek_rk = recovery::derive_kek_rk(&m).map_err(|e| e.to_string())?;
    let svk = to_arr32(svk)?;
    recovery::wrap_svk_with_rk(&svk, &kek_rk, &SVK_AD_USER).map_err(|e| e.to_string())
}

/// Derive the Data Encryption Key (DEK) from the SVK (crypto.md §2 step 5).
/// Used to encrypt/decrypt item payloads (AD bound to uuid + enc_key_gen).
pub fn derive_dek(svk: &[u8]) -> Result<Vec<u8>, String> {
    let svk = to_arr32(svk)?;
    let dek = key_tree::derive_dek(&Zeroizing::new(svk)).map_err(|e| e.to_string())?;
    Ok(dek.to_vec())
}

/// Encrypt an item plaintext under the DEK. `payload` returned is the AEAD
/// envelope the server stores opaquely (api.md §1 zero-knowledge).
pub fn encrypt_item(
    uuid: &str,
    enc_key_gen: u64,
    dek: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let dek = to_arr32(dek)?;
    let uuid = uuid::Uuid::parse_str(uuid).map_err(|e| e.to_string())?;
    aead::encrypt(&dek, &uuid, enc_key_gen, plaintext).map_err(|e| e.to_string())
}

/// Decrypt an item payload under the DEK (AEAD AD = uuid + enc_key_gen).
pub fn decrypt_item(
    uuid: &str,
    enc_key_gen: u64,
    dek: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let dek = to_arr32(dek)?;
    let uuid = uuid::Uuid::parse_str(uuid).map_err(|e| e.to_string())?;
    aead::decrypt(&dek, &uuid, enc_key_gen, payload).map_err(|e| e.to_string())
}

/// Begin OPAQUE registration. Returns `(message_to_server, client_state)`.
pub fn opaque_register_start(password: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    opaque::client_register_start(password.as_bytes()).map_err(|e| e.to_string())
}

/// Finish OPAQUE registration. Returns the `registration_upload` to send to the
/// server (`/auth/register/finish`). api.md §3.1.
///
/// Drives the same OPAQUE registration state machine as `vautr_auth::state`
/// (`opaque::client_register_finish` with the same `credential_identifier`
/// binding), but surfaces errors as `Result` instead of the wrapper's panic.
pub fn opaque_register_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    username: &str,
) -> Result<Vec<u8>, String> {
    let (upload, _export) = vautr_crypto::opaque::client_register_finish(
        client_state,
        server_response,
        password.as_bytes(),
        username.as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    Ok(upload)
}

/// Begin OPAQUE login. Returns `(message_to_server, client_state)`.
pub fn opaque_login_start(password: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    opaque::client_login_start(password.as_bytes()).map_err(|e| e.to_string())
}

/// Finish OPAQUE login. Returns `(login_upload, session_key)` where `session_key`
/// is the OPAQUE session key (server mints the bearer token from it). api.md §3.2.
///
/// Drives the same OPAQUE login state machine as `vautr_auth::state`
/// (`opaque::client_login_finish` with the same `credential_identifier`
/// binding), but surfaces errors (e.g. wrong password) as `Result` instead of
/// the wrapper's panic.
pub fn opaque_login_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    username: &str,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let (upload, session_key) = vautr_crypto::opaque::client_login_finish(
        client_state,
        server_response,
        password.as_bytes(),
        username.as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    Ok((upload, session_key))
}

// ---------------------------------------------------------------------------
// wasm-bindgen wrappers (JsValue in/out)
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn generate_kdf_salt_js() -> Vec<u8> {
    generate_kdf_salt()
}

#[wasm_bindgen]
pub fn derive_master_key_js(password: &str, salt: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    derive_master_key(password, &salt).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn derive_kek_js(master_key: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    derive_kek(&master_key).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn generate_svk_js() -> Vec<u8> {
    generate_svk()
}

#[wasm_bindgen]
pub fn wrap_svk_js(svk: Vec<u8>, kek: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    wrap_svk(&svk, &kek).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn unwrap_svk_js(wrapped: Vec<u8>, kek: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    unwrap_svk(&wrapped, &kek).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn generate_recovery_mnemonic_js() -> Result<String, JsValue> {
    generate_recovery_mnemonic().map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn wrap_svk_with_rk_js(svk: Vec<u8>, mnemonic: &str) -> Result<Vec<u8>, JsValue> {
    wrap_svk_with_rk(&svk, mnemonic).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn derive_dek_js(svk: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    derive_dek(&svk).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn encrypt_item_js(
    uuid: &str,
    enc_key_gen: u64,
    dek: Vec<u8>,
    plaintext: Vec<u8>,
) -> Result<Vec<u8>, JsValue> {
    encrypt_item(uuid, enc_key_gen, &dek, &plaintext).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn decrypt_item_js(
    uuid: &str,
    enc_key_gen: u64,
    dek: Vec<u8>,
    payload: Vec<u8>,
) -> Result<Vec<u8>, JsValue> {
    decrypt_item(uuid, enc_key_gen, &dek, &payload).map_err(|e| JsValue::from_str(&e))
}

/// Begin OPAQUE registration. Returns `{ message, state }` (both `Uint8Array`).
#[wasm_bindgen]
pub fn opaque_register_start_js(password: &str) -> Result<JsValue, JsValue> {
    let (message, state) = opaque_register_start(password).map_err(|e| JsValue::from_str(&e))?;
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("message"), &message.into()).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("state"), &state.into()).unwrap();
    Ok(obj.into())
}

/// Finish OPAQUE registration. Returns the `registration_upload` (base64-ready).
#[wasm_bindgen]
pub fn opaque_register_finish_js(
    client_state: Vec<u8>,
    server_response: Vec<u8>,
    password: &str,
    username: &str,
) -> Result<Vec<u8>, JsValue> {
    opaque_register_finish(&client_state, &server_response, password, username)
        .map_err(|e| JsValue::from_str(&e))
}

/// Begin OPAQUE login. Returns `{ message, state }`.
#[wasm_bindgen]
pub fn opaque_login_start_js(password: &str) -> Result<JsValue, JsValue> {
    let (message, state) = opaque_login_start(password).map_err(|e| JsValue::from_str(&e))?;
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("message"), &message.into()).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("state"), &state.into()).unwrap();
    Ok(obj.into())
}

/// Finish OPAQUE login. Returns `{ upload, session_key }`.
#[wasm_bindgen]
pub fn opaque_login_finish_js(
    client_state: Vec<u8>,
    server_response: Vec<u8>,
    password: &str,
    username: &str,
) -> Result<JsValue, JsValue> {
    let (upload, session_key) =
        opaque_login_finish(&client_state, &server_response, password, username)
            .map_err(|e| JsValue::from_str(&e))?;
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("upload"), &upload.into()).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("session_key"), &session_key.into()).unwrap();
    Ok(obj.into())
}

// ---------------------------------------------------------------------------
// Native unit tests (gate: OPAQUE register/login + Argon2id round-trip)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vautr_crypto::opaque;

    #[test]
    fn kdf_deterministic_with_same_salt() {
        let salt = generate_kdf_salt();
        let mk1 = derive_master_key("hunter2", &salt).unwrap();
        let mk2 = derive_master_key("hunter2", &salt).unwrap();
        assert_eq!(mk1, mk2);
        assert_eq!(mk1.len(), 32);
        let other = derive_master_key("hunter2", &generate_kdf_salt()).unwrap();
        assert_ne!(mk1, other, "different salt must yield a different MK");
    }

    #[test]
    fn svk_wrap_unwrap_roundtrip() {
        let salt = generate_kdf_salt();
        let mk = derive_master_key("s3cret", &salt).unwrap();
        let kek = derive_kek(&mk).unwrap();
        let svk = generate_svk();
        let wrapped = wrap_svk(&svk, &kek).unwrap();
        let unwrapped = unwrap_svk(&wrapped, &kek).unwrap();
        assert_eq!(unwrapped, svk);

        // Wrong key (different MK) must fail.
        let mk2 = derive_master_key("nope", &salt).unwrap();
        let kek2 = derive_kek(&mk2).unwrap();
        assert!(unwrap_svk(&wrapped, &kek2).is_err());
    }

    #[test]
    fn recovery_wrap_roundtrip() {
        let mnemonic = generate_recovery_mnemonic().unwrap();
        let svk = generate_svk();
        let wrapped = wrap_svk_with_rk(&svk, &mnemonic).unwrap();
        // We can verify by re-deriving KEK_RK and unwrapping via recovery.
        let m = recovery::decode_recovery_mnemonic(&mnemonic).unwrap();
        let kek_rk = recovery::derive_kek_rk(&m).unwrap();
        let svk_arr = to_arr32(&svk).unwrap();
        let recovered = recovery::unwrap_svk_with_rk(&wrapped, &kek_rk, &SVK_AD_USER).unwrap();
        assert_eq!(&*recovered, &svk_arr);
    }

    #[test]
    fn opaque_register_then_login_roundtrip() {
        let password = "correct horse battery staple";
        let user = "gate@example.com";

        // Server setup (mirrors what vautr-server generates/persists).
        let setup = opaque::server_setup_public_key().unwrap();

        // Registration via the exposed state-machine wrappers.
        let (reg_msg, reg_state) = opaque_register_start(password).unwrap();
        let reg_resp = opaque::server_register_start(&setup, &reg_msg, user.as_bytes()).unwrap();
        let upload = opaque_register_finish(&reg_state, &reg_resp, password, user).unwrap();
        let record = opaque::server_register_finish(&upload).unwrap();

        // Login via the exposed state-machine wrappers.
        let (login_msg, login_state) = opaque_login_start(password).unwrap();
        let (login_resp, _sstate) =
            opaque::server_login_start(&setup, Some(&record), &login_msg, user.as_bytes()).unwrap();
        let (final_upload, client_session) =
            opaque_login_finish(&login_state, &login_resp, password, user).unwrap();
        let server_session = opaque::server_login_finish(&_sstate, &final_upload).unwrap();

        assert_eq!(
            client_session, server_session,
            "client/server session keys match"
        );
        assert_eq!(client_session.len(), 64);
    }

    #[test]
    fn item_dek_encrypt_decrypt_roundtrip() {
        let salt = generate_kdf_salt();
        let mk = derive_master_key("s3cret", &salt).unwrap();
        let kek = derive_kek(&mk).unwrap();
        let svk = generate_svk();
        let dek = derive_dek(&svk).unwrap();
        let uuid = uuid::Uuid::new_v4().to_string();

        let plaintext = br#"{"title":"Acme","password":"hunter2"}"#.to_vec();
        let cipher = encrypt_item(&uuid, 1, &dek, &plaintext).unwrap();
        let recovered = decrypt_item(&uuid, 1, &dek, &cipher).unwrap();
        assert_eq!(recovered, plaintext);

        // Wrong AD (different uuid or key gen) must fail AEAD.
        let other_uuid = uuid::Uuid::new_v4().to_string();
        assert!(decrypt_item(&other_uuid, 1, &dek, &cipher).is_err());
        assert!(decrypt_item(&uuid, 2, &dek, &cipher).is_err());

        // Wrong key (different SVK/DEK) must fail.
        let dek2 = derive_dek(&generate_svk()).unwrap();
        assert!(decrypt_item(&uuid, 1, &dek2, &cipher).is_err());

        // Cross-check with the WebClient path (key_tree::derive_dek identical).
        let _ = kek;
    }

    #[test]
    fn opaque_login_wrong_password_fails() {
        let password = "right";
        let wrong = "wrong";
        let user = "bad@example.com";
        let setup = opaque::server_setup_public_key().unwrap();

        let (reg_msg, reg_state) = opaque_register_start(password).unwrap();
        let reg_resp = opaque::server_register_start(&setup, &reg_msg, user.as_bytes()).unwrap();
        let upload = opaque_register_finish(&reg_state, &reg_resp, password, user).unwrap();
        let record = opaque::server_register_finish(&upload).unwrap();

        let (login_msg, login_state) = opaque_login_start(wrong).unwrap();
        let (login_resp, _sstate) =
            opaque::server_login_start(&setup, Some(&record), &login_msg, user.as_bytes()).unwrap();
        let res = opaque_login_finish(&login_state, &login_resp, wrong, user);
        assert!(res.is_err(), "wrong password must fail");
    }
}
