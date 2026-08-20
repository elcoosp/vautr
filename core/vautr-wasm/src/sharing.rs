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
    // The server recomputes `sender_uuid` from the authenticated session
    // (create_share ignores the bundle's sender_uuid field), so the client may
    // pass a non-UUID identifier (e.g. the local email). Accept it by mapping
    // any non-UUID sender to a placeholder; the authoritative sender is the
    // token's user id on the server.
    let sender = match uuid::Uuid::parse_str(sender_uuid) {
        Ok(u) => u,
        Err(_) => uuid::Uuid::nil(),
    };
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

// ---------------------------------------------------------------------------
// Group sharing (sharing-pki.md §6)
// ---------------------------------------------------------------------------

/// Create a sharing group. Returns JSON:
/// `{ "group": { group_id, name, admin_uuid }, "secret": <base64 Group SIK> }`.
/// The caller stores both at rest (under the user's master key) and passes the
/// object back into `add_group_member` / `encrypt_group_item` / etc.
#[wasm_bindgen]
pub fn create_sharing_group(name: &str, admin_uuid: &str) -> Result<String, JsValue> {
    let admin = uuid::Uuid::parse_str(admin_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let key = vautr_sharing::create_group(name.to_string(), admin)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let out = serde_json::json!({
        "group": {
            "group_id": key.group.group_id.to_string(),
            "name": key.group.name,
            "admin_uuid": key.group.admin_uuid.to_string(),
        },
        "secret": base64::engine::general_purpose::STANDARD.encode(key.secret_bytes()),
    });
    serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Wrap the Group SIK for a new member. `group_json` is the object returned by
/// `create_sharing_group` / `unwrap_group_key`. Returns a WrappedGroupKey JSON
/// `{ group_id, member_uuid, wrapped_sik, ephemeral_public_key }` to upload to
/// `POST /groups/{id}/members`.
#[wasm_bindgen]
pub fn add_group_member(
    group_json: &str,
    member_uuid: &str,
    member_public_key_b64: &str,
) -> Result<String, JsValue> {
    let g: GroupKeyJson =
        serde_json::from_str(group_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let key = group_key_from_json(&g)?;
    let member =
        uuid::Uuid::parse_str(member_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let pk_bytes = decode_b64(member_public_key_b64)?;
    if pk_bytes.len() != 32 {
        return Err(JsValue::from_str("member public key must be 32 bytes"));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);
    let wrapped = vautr_sharing::add_group_member(&key, member, &pk)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&wrapped).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Member-side: decapsulate the Group SIK from a group-inbox entry and the
/// member's sharing secret. `inbox_json` is the `GroupInboxItem` JSON returned by
/// `GET /groups/inbox` (`group_id`, `name`, `admin_uuid`, `wrapped_sik`,
/// `ephemeral_public_key`). Returns the same `{ group, secret }` shape as
/// `create_sharing_group`, which the member passes to `decrypt_group_item`.
#[wasm_bindgen]
pub fn unwrap_group_key(inbox_json: &str, recipient_secret_b64: &str) -> Result<String, JsValue> {
    #[derive(serde::Deserialize)]
    struct Inbox {
        group_id: String,
        name: String,
        admin_uuid: String,
        wrapped_sik: String,
        ephemeral_public_key: String,
    }
    let inbox: Inbox =
        serde_json::from_str(inbox_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let wrapped = vautr_sharing::WrappedGroupKey {
        group_id: uuid::Uuid::parse_str(&inbox.group_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
        member_uuid: uuid::Uuid::nil(),
        wrapped_sik: inbox.wrapped_sik,
        ephemeral_public_key: inbox.ephemeral_public_key,
    };
    let kp = parse_keypair(recipient_secret_b64).map_err(|e| JsValue::from_str(&e))?;
    let sik = vautr_sharing::unwrap_group_key(&wrapped, &kp)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let out = serde_json::json!({
        "group": {
            "group_id": inbox.group_id,
            "name": inbox.name,
            "admin_uuid": inbox.admin_uuid,
        },
        "secret": base64::engine::general_purpose::STANDARD.encode(&sik[..]),
    });
    serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Encrypt a vault item's payload under the Group SIK. `group_json` is the
/// `{ group, secret }` object; returns base64 ciphertext to upload to
/// `POST /groups/{id}/items`.
#[wasm_bindgen]
pub fn encrypt_group_item(
    group_json: &str,
    item_uuid: &str,
    plaintext: &[u8],
) -> Result<String, JsValue> {
    let g: GroupKeyJson =
        serde_json::from_str(group_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let key = group_key_from_json(&g)?;
    let item = uuid::Uuid::parse_str(item_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let ct = key
        .encrypt_item(&item, plaintext)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(ct))
}

/// Decrypt a group item's payload. `group_json` is the member's `{ group, secret }`.
#[wasm_bindgen]
pub fn decrypt_group_item(
    group_json: &str,
    item_uuid: &str,
    ciphertext_b64: &str,
) -> Result<Vec<u8>, JsValue> {
    let g: GroupKeyJson =
        serde_json::from_str(group_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let key = group_key_from_json(&g)?;
    let item = uuid::Uuid::parse_str(item_uuid).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let ct = decode_b64(ciphertext_b64)?;
    key.decrypt_item(&item, &ct)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// --- group helpers --------------------------------------------------------

#[derive(serde::Deserialize)]
struct GroupKeyJson {
    group: ShareGroupLite,
    secret: String,
}

#[derive(serde::Deserialize)]
struct ShareGroupLite {
    group_id: String,
    name: String,
    admin_uuid: String,
}

fn group_key_from_json(g: &GroupKeyJson) -> Result<vautr_sharing::ShareGroupKey, JsValue> {
    let group = vautr_sharing::ShareGroup {
        group_id: uuid::Uuid::parse_str(&g.group.group_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
        name: g.group.name.clone(),
        admin_uuid: uuid::Uuid::parse_str(&g.group.admin_uuid)
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
    };
    let secret = decode_b64(&g.secret)?;
    vautr_sharing::ShareGroupKey::from_secret(group, &secret)
        .map_err(|e| JsValue::from_str(&e.to_string()))
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

    #[test]
    fn group_share_roundtrip() {
        fn fail<T, E: std::fmt::Debug>(r: Result<T, E>) -> T {
            match r {
                Ok(v) => v,
                Err(e) => panic!("sharing fn returned err: {e:?}"),
            }
        }

        let admin = generate_sharing_keypair().unwrap();
        let member = generate_sharing_keypair().unwrap();
        let a: serde_json::Value = serde_json::from_str(&admin).unwrap();
        let m: serde_json::Value = serde_json::from_str(&member).unwrap();
        let admin_uuid = uuid::Uuid::new_v4();

        // Admin creates the group.
        let group = fail(create_sharing_group("team", &admin_uuid.to_string()));
        let g: serde_json::Value = serde_json::from_str(&group).unwrap();
        assert_eq!(g["group"]["name"], "team");
        assert!(!g["secret"].as_str().unwrap().is_empty());

        // Admin wraps the Group SIK for the member.
        let wrapped = fail(add_group_member(
            &group,
            &uuid::Uuid::new_v4().to_string(),
            m["public"].as_str().unwrap(),
        ));
        let w: serde_json::Value = serde_json::from_str(&wrapped).unwrap();
        assert!(!w["wrapped_sik"].as_str().unwrap().is_empty());

        // Member unwraps the Group SIK (from the group inbox entry).
        let inbox = serde_json::json!({
            "group_id": w["group_id"],
            "name": g["group"]["name"],
            "admin_uuid": g["group"]["admin_uuid"],
            "wrapped_sik": w["wrapped_sik"],
            "ephemeral_public_key": w["ephemeral_public_key"],
        });
        let member_key = fail(unwrap_group_key(
            &inbox.to_string(),
            m["secret"].as_str().unwrap(),
        ));

        // Admin encrypts an item under the Group SIK.
        let item_uuid = uuid::Uuid::new_v4();
        let plaintext = b"shared-to-the-whole-team";
        let ct = fail(encrypt_group_item(
            &group,
            &item_uuid.to_string(),
            plaintext,
        ));

        // Member decrypts it.
        let recovered = fail(decrypt_group_item(&member_key, &item_uuid.to_string(), &ct));
        assert_eq!(recovered, plaintext);
    }
}
