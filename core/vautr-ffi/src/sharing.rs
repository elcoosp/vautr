//! UniFFI sharing surface for mobile (ADR-007 / sharing-pki.md §6).
//!
//! These are the same zero-knowledge sharing primitives the web/extension use
//! via `vautr-wasm`, but exposed through the uniffi `MobileClient` so the React
//! Native layer can run them on-device without a plaintext round-trip through
//! JS. Public keys / secrets cross the bridge as base64 strings; the sharing
//! secret key is held in the `MobileClient` (persisted by the app, mirroring
//! `VautrWebClient.ensureSharingKey`).
//!
//! The HTTP relay (publish/fetch sharing public key, upload/download the share
//! bundle, group inbox) is performed by the mobile SDK's `MobileApiClient` — it
//! calls these pure-crypto functions and ships the resulting blobs to the
//! server. Plaintext secret bytes are produced only here inside Rust and handed
//! straight back to the native caller; they never sit in the JS heap.

use base64::Engine;
use std::sync::RwLock;

use uniffi::Object;
use uuid::Uuid;

use vautr_crypto::kdf::MK_LEN;
use vautr_crypto::sharing::{SharingKeyPair, SharingPublicKey};
use vautr_sharing::{IncomingShare, ShareBundle, ShareGroupKey, WrappedGroupKey};

use crate::FfiError;

fn b64_encode(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, FfiError> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| FfiError::Core(format!("base64 decode: {e}")))
}

fn parse_uuid(s: &str) -> Result<Uuid, FfiError> {
    Uuid::parse_str(s).map_err(|e| FfiError::Uuid(e.to_string()))
}

/// Result of `generate_sharing_keypair` (base64 for wire transport).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct FfiSharingKeyPair {
    pub public_key_b64: String,
    pub secret_key_b64: String,
}

/// A 1:1 share bundle as handed to the untrusted relay (base64 fields).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct FfiShareBundle {
    pub share_id: String,
    pub sender_uuid: String,
    pub recipient_uuid: String,
    pub item_uuid: String,
    pub wrapped_sik: String,
    pub ephemeral_public_key: String,
    pub encrypted_payload: String,
}

/// A group SIK wrapped for a single member (uploaded to the server).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct FfiWrappedGroupKey {
    pub group_id: String,
    pub member_uuid: String,
    pub wrapped_sik: String,
    pub ephemeral_public_key: String,
}

/// Admin-created group context (`{ group, secret_b64 }`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct FfiGroupKey {
    pub group_id: String,
    pub name: String,
    pub admin_uuid: String,
    /// Raw Group SIK (zeroized on drop in Rust); persist under the master key.
    pub secret_b64: String,
}

fn ffi_bundle(b: &ShareBundle) -> FfiShareBundle {
    FfiShareBundle {
        share_id: b.share_id.to_string(),
        sender_uuid: b.sender_uuid.to_string(),
        recipient_uuid: b.recipient_uuid.to_string(),
        item_uuid: b.item_uuid.to_string(),
        wrapped_sik: b.wrapped_sik.clone(),
        ephemeral_public_key: b.ephemeral_public_key.clone(),
        encrypted_payload: b.encrypted_payload.clone(),
    }
}

fn ffi_wrapped(w: &WrappedGroupKey) -> FfiWrappedGroupKey {
    FfiWrappedGroupKey {
        group_id: w.group_id.to_string(),
        member_uuid: w.member_uuid.to_string(),
        wrapped_sik: w.wrapped_sik.clone(),
        ephemeral_public_key: w.ephemeral_public_key.clone(),
    }
}

fn ffi_group_key(g: &ShareGroupKey) -> FfiGroupKey {
    FfiGroupKey {
        group_id: g.group.group_id.to_string(),
        name: g.group.name.clone(),
        admin_uuid: g.group.admin_uuid.to_string(),
        secret_b64: b64_encode(g.secret_bytes()),
    }
}

fn parse_public_key(b64: &str) -> Result<SharingPublicKey, FfiError> {
    let bytes = b64_decode(b64)?;
    if bytes.len() != 32 {
        return Err(FfiError::Core(format!(
            "public key must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&bytes);
    Ok(pk)
}

/// Pure-crypto sharing primitives (no I/O). The mobile SDK layers the HTTP
/// relay (publish/fetch public key, upload/download bundle, group inbox) on top
/// of these. Returned/accepted values are base64 or JSON strings so they cross
/// the uniffi bridge without leaking raw secret bytes into JS.
#[uniffi::export]
pub fn ffi_generate_sharing_keypair() -> FfiSharingKeyPair {
    let kp = SharingKeyPair::generate();
    FfiSharingKeyPair {
        public_key_b64: b64_encode(&kp.public),
        secret_key_b64: b64_encode(&kp.secret_bytes()),
    }
}

/// Build a 1:1 share bundle for `recipient_pubkey_b64` (the recipient's sharing
/// public key, fetched from the server PKI).
#[uniffi::export]
pub fn ffi_share_item(
    sender_uuid: String,
    recipient_uuid: String,
    item_uuid: String,
    recipient_pubkey_b64: String,
    plaintext: Vec<u8>,
) -> Result<FfiShareBundle, FfiError> {
    let sender = parse_uuid(&sender_uuid)?;
    let recipient = parse_uuid(&recipient_uuid)?;
    let item = parse_uuid(&item_uuid)?;
    let pk = parse_public_key(&recipient_pubkey_b64)?;
    vautr_sharing::share_item(sender, recipient, item, &pk, &plaintext)
        .map(|b| ffi_bundle(&b))
        .map_err(|e| FfiError::Core(format!("share_item: {e}")))
}

/// Decrypt an incoming 1:1 share. `incoming_json` is the JSON `IncomingShare`
/// (share_id, sender_uuid, item_uuid, wrapped_sik, ephemeral_public_key,
/// encrypted_payload) returned by the server; `sharing_secret_b64` is the
/// recipient's persisted sharing secret key. Returns the plaintext bytes (which
/// the caller must zeroize after use).
#[uniffi::export]
pub fn ffi_accept_share(
    incoming_json: String,
    sharing_secret_b64: String,
) -> Result<Vec<u8>, FfiError> {
    let incoming: IncomingShare = serde_json::from_str(&incoming_json)
        .map_err(|e| FfiError::Core(format!("parse IncomingShare: {e}")))?;
    let secret = b64_decode(&sharing_secret_b64)?;
    if secret.len() != 32 {
        return Err(FfiError::Core(format!(
            "sharing secret must be 32 bytes, got {}",
            secret.len()
        )));
    }
    let mut sk = [0u8; 32];
    sk.copy_from_slice(&secret);
    let kp = SharingKeyPair::from_secret(sk);
    vautr_sharing::accept_share(&kp, &incoming)
        .map_err(|e| FfiError::Core(format!("accept_share: {e}")))
}

/// Create a sharing group (admin). Returns the admin's `{ group, secret }`.
#[uniffi::export]
pub fn ffi_create_group(name: String, admin_uuid: String) -> Result<FfiGroupKey, FfiError> {
    let admin = parse_uuid(&admin_uuid)?;
    vautr_sharing::create_group(name, admin)
        .map(|g| ffi_group_key(&g))
        .map_err(|e| FfiError::Core(format!("create_group: {e}")))
}

/// Wrap the Group SIK for a new member. `group_json` is the admin's persisted
/// `{ group, secret }` JSON; `member_pubkey_b64` is the member's sharing public
/// key (from the server PKI).
#[uniffi::export]
pub fn ffi_add_group_member(
    group_json: String,
    member_uuid: String,
    member_pubkey_b64: String,
) -> Result<FfiWrappedGroupKey, FfiError> {
    let gk = parse_group_key(&group_json)?;
    let member = parse_uuid(&member_uuid)?;
    let pk = parse_public_key(&member_pubkey_b64)?;
    vautr_sharing::add_group_member(&gk, member, &pk)
        .map(|w| ffi_wrapped(&w))
        .map_err(|e| FfiError::Core(format!("add_group_member: {e}")))
}

/// Member-side: decapsulate the Group SIK from an inbox entry. `inbox_json` is
/// the JSON `WrappedGroupKey`; `sharing_secret_b64` is the member's sharing
/// secret key. Returns the member's `{ group, secret }` JSON for persistence.
#[uniffi::export]
pub fn ffi_unwrap_group_key(
    inbox_json: String,
    sharing_secret_b64: String,
) -> Result<String, FfiError> {
    let wrapped: WrappedGroupKey = serde_json::from_str(&inbox_json)
        .map_err(|e| FfiError::Core(format!("parse WrappedGroupKey: {e}")))?;
    let secret = b64_decode(&sharing_secret_b64)?;
    if secret.len() != 32 {
        return Err(FfiError::Core(format!(
            "sharing secret must be 32 bytes, got {}",
            secret.len()
        )));
    }
    let mut sk = [0u8; 32];
    sk.copy_from_slice(&secret);
    let kp = SharingKeyPair::from_secret(sk);
    let sik = vautr_sharing::unwrap_group_key(&wrapped, &kp)
        .map_err(|e| FfiError::Core(format!("unwrap_group_key: {e}")))?;
    let group = vautr_sharing::ShareGroup {
        group_id: wrapped.group_id,
        name: String::new(),
        admin_uuid: Uuid::nil(),
    };
    let group_key = ShareGroupKey::from_secret(group, &sik[..])
        .map_err(|e| FfiError::Core(format!("rebuild group key: {e}")))?;
    serde_json::to_string(&ffi_group_key(&group_key))
        .map_err(|e| FfiError::Core(format!("serialize group key: {e}")))
}

/// Encrypt a vault item's payload for a group (one encryption for N members).
/// `group_json` is the admin/member's persisted `{ group, secret }`.
#[uniffi::export]
pub fn ffi_encrypt_group_item(
    group_json: String,
    item_uuid: String,
    plaintext: Vec<u8>,
) -> Result<String, FfiError> {
    let gk = parse_group_key(&group_json)?;
    let item = parse_uuid(&item_uuid)?;
    let ct = gk
        .encrypt_item(&item, &plaintext)
        .map_err(|e| FfiError::Core(format!("encrypt: {e}")))?;
    Ok(b64_encode(&ct))
}

/// Decrypt a group item's payload. `group_json` is the member's key; `ct_b64`
/// is the Group-SIK-encrypted payload from the server.
#[uniffi::export]
pub fn ffi_decrypt_group_item(
    group_json: String,
    item_uuid: String,
    ct_b64: String,
) -> Result<Vec<u8>, FfiError> {
    let gk = parse_group_key(&group_json)?;
    let item = parse_uuid(&item_uuid)?;
    let ct = b64_decode(&ct_b64)?;
    gk.decrypt_item(&item, &ct)
        .map_err(|e| FfiError::Core(format!("decrypt: {e}")))
}

/// Parse a persisted `{ group, secret }` JSON back into a `ShareGroupKey`.
fn parse_group_key(group_json: &str) -> Result<ShareGroupKey, FfiError> {
    let parsed: serde_json::Value = serde_json::from_str(group_json)
        .map_err(|e| FfiError::Core(format!("parse group key json: {e}")))?;
    let group_id = parsed
        .get("group_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FfiError::Core("missing group_id".into()))?;
    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let admin_uuid = parsed
        .get("admin_uuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FfiError::Core("missing admin_uuid".into()))?;
    let secret_b64 = parsed
        .get("secret_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FfiError::Core("missing secret_b64".into()))?;
    let secret = b64_decode(secret_b64)?;
    if secret.len() != MK_LEN {
        return Err(FfiError::Core(format!(
            "group sik must be {MK_LEN} bytes, got {}",
            secret.len()
        )));
    }
    let mut sik = [0u8; MK_LEN];
    sik.copy_from_slice(&secret);
    let group = vautr_sharing::ShareGroup {
        group_id: parse_uuid(group_id)?,
        name,
        admin_uuid: parse_uuid(admin_uuid)?,
    };
    ShareGroupKey::from_secret(group, &sik).map_err(|e| FfiError::Core(format!("from_secret: {e}")))
}

/// Sharing helpers attached to the `MobileClient` object (persists the sharing
/// secret key locally so the app does not regenerate it on every launch, and so
/// the HTTP relay can publish the public key once).
#[uniffi::export]
impl MobileSharingStore {
    /// Construct a sharing-key store bound to a persisted secret (base64). The
    /// app loads/saves the secret from its secure store (Keychain/Keystore).
    #[uniffi::constructor]
    pub fn new(sharing_secret_b64: Option<String>) -> Self {
        MobileSharingStore {
            secret: RwLock::new(sharing_secret_b64),
        }
    }

    /// Ensure a sharing keypair exists; if none is persisted, generate one and
    /// return its public key (for the app to publish to the server PKI).
    pub fn ensure_sharing_key(&self) -> Result<String, FfiError> {
        let mut guard = self.secret.write().unwrap();
        if let Some(existing) = guard.as_ref() {
            // Validate it decodes to 32 bytes; derive the public key.
            let bytes = b64_decode(existing)?;
            if bytes.len() != 32 {
                return Err(FfiError::Core(format!(
                    "stored sharing secret {} bytes",
                    bytes.len()
                )));
            }
            let mut sk = [0u8; 32];
            sk.copy_from_slice(&bytes);
            return Ok(b64_encode(&SharingKeyPair::from_secret(sk).public));
        }
        let kp = SharingKeyPair::generate();
        let secret_b64 = b64_encode(&kp.secret_bytes());
        let public_b64 = b64_encode(&kp.public);
        *guard = Some(secret_b64);
        Ok(public_b64)
    }

    /// The persisted sharing secret key (base64), if any.
    pub fn sharing_secret(&self) -> Option<String> {
        self.secret.read().unwrap().clone()
    }
}

/// Thin wrapper holding the persisted sharing secret key for the mobile client.
#[derive(Object)]
pub struct MobileSharingStore {
    secret: RwLock<Option<String>>,
}
