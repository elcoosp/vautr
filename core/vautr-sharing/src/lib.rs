//! # vautr-sharing
//!
//! Zero-knowledge, end-to-end-encrypted item and group sharing for Vautr.
//!
//! Spec: [`docs/architecture/sharing-pki.md`] and [`docs/architecture/adr-007-sharing-kem.md`].
//! Sharing is a boundary-crossing operation: the sender's SVK never leaves the
//! domain, and the server is only an untrusted relay for encrypted blobs and
//! public keys. Items are encrypted under a dedicated Symmetric Item Key (SIK)
//! via a KEM-DEM protocol built on X25519 + XChaCha20-Poly1305 (see
//! `vautr_crypto::sharing`).
//!
//! This crate is **pure logic / no I/O**. It produces and consumes the
//! zero-knowledge envelopes that a transport (HTTP relay, FFI, wasm) sends
//! between peers. All key material is held in [`zeroize::Zeroizing`] buffers and
//! zeroed on drop. Every entry point returns `Err(ShareError)` on failure — never
//! panics.
//!
//! # Flow (1:1)
//! 1. Sender calls [`share_item`] with the recipient's `SharingPublicKey`: a
//!    fresh SIK is generated, the plaintext is sealed under it (DEM), and the SIK
//!    is wrapped for the recipient (KEM).
//! 2. The resulting [`ShareBundle`] is relayed through the server.
//! 3. Recipient calls [`accept_share`] with their `SharingKeyPair` to
//!    decapsulate the SIK and decrypt the payload.
//!
//! # Groups (1:N)
//! A group shares a unified Group SIK. The admin holds it in a [`ShareGroupKey`],
//! wraps it per member (see [`add_group_member`]), and rotates it on membership
//! changes ([`rotate_group_sik`] / [`remove_group_member`]) for forward secrecy.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use vautr_crypto::aead::{decrypt_with_ad, encrypt_with_nonce};
use vautr_crypto::kdf::MK_LEN;
use vautr_crypto::sharing::{
    share_item as kem_share_item, unwrap_shared_item, SharedEnvelope, SharingKeyPair,
    SharingPublicKey,
};
use zeroize::Zeroizing;

pub mod error;

pub use error::{Result, ShareError};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// DEM associated-data prefix (sharing-pki.md §3.3: `AD = "vautr-share-{share_id}"`).
const SHARE_PAYLOAD_PREFIX: &str = "vautr-share-";

/// Build the DEM associated data binding a payload to its share id.
fn payload_ad(id: &Uuid) -> Vec<u8> {
    format!("{SHARE_PAYLOAD_PREFIX}{id}").into_bytes()
}

fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

fn unb64(s: &str, what: &str) -> Result<Vec<u8>> {
    B64.decode(s)
        .map_err(|_| ShareError::Crypto(format!("invalid {what} base64")))
}

/// A 1:1 share of one vault item (sharing-pki.md §3–§5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareInfo {
    pub share_id: Uuid,
    pub sender_uuid: Uuid,
    pub recipient_uuid: Uuid,
    pub item_uuid: Uuid,
    /// Whether the share is currently active (not revoked).
    pub active: bool,
}

/// A single share bundle produced by the sender and relayed via the server.
///
/// Contains the KEM envelope (`wrapped_sik`, `ephemeral_public_key`) plus the
/// DEM-encrypted payload. Base64 strings are the on-the-wire representation so
/// the type serializes directly over JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareBundle {
    pub share_id: Uuid,
    pub sender_uuid: Uuid,
    pub recipient_uuid: Uuid,
    pub item_uuid: Uuid,
    /// `WrappedSIK` ciphertext blob (ADR-007).
    pub wrapped_sik: String,
    /// Ephemeral X25519 public key needed to decapsulate the SIK.
    pub ephemeral_public_key: String,
    /// Payload encrypted under the SIK (DEM).
    pub encrypted_payload: String,
}

/// An incoming share awaiting decryption/ingestion by the recipient (§4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingShare {
    pub share_id: Uuid,
    pub sender_uuid: Uuid,
    pub item_uuid: Uuid,
    /// `WrappedSIK` blob delivered by the server.
    pub wrapped_sik: String,
    /// Ephemeral public key needed to decapsulate the SIK.
    pub ephemeral_public_key: String,
    /// Payload encrypted under the SIK (DEM).
    pub encrypted_payload: String,
}

impl From<ShareBundle> for IncomingShare {
    fn from(b: ShareBundle) -> Self {
        Self {
            share_id: b.share_id,
            sender_uuid: b.sender_uuid,
            item_uuid: b.item_uuid,
            wrapped_sik: b.wrapped_sik,
            ephemeral_public_key: b.ephemeral_public_key,
            encrypted_payload: b.encrypted_payload,
        }
    }
}

// ---------------------------------------------------------------------------
// 1:1 flow
// ---------------------------------------------------------------------------

/// Share one vault item with a single recipient (1:1 flow, §3).
///
/// 1. Generates a fresh 256-bit SIK.
/// 2. Seals `plaintext` under the SIK (DEM, `AD = "vautr-share-{share_id}"`).
/// 3. Wraps the SIK for `recipient_public_key` via ephemeral X25519 KEM (ADR-007).
///
/// The returned [`ShareBundle`] is safe to hand to the untrusted relay.
pub fn share_item(
    sender_uuid: Uuid,
    recipient_uuid: Uuid,
    item_uuid: Uuid,
    recipient_public_key: &SharingPublicKey,
    plaintext: &[u8],
) -> Result<ShareBundle> {
    // The server keys shares by `item_uuid` (shares table natural key), and the
    // recipient's `accept_share` derives the DEM associated-data from
    // `incoming.share_id` (which the server returns as `item_uuid`). Using
    // `item_uuid` as the `share_id` keeps the AD consistent end-to-end.
    let share_id = item_uuid;
    let mut sik = Zeroizing::new([0u8; MK_LEN]);
    rand::thread_rng().fill_bytes(&mut *sik);

    // DEM: encrypt the payload under the SIK.
    let ad = payload_ad(&share_id);
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let encrypted_payload = encrypt_with_nonce(&sik, &nonce, &ad, plaintext)
        .map_err(|e| ShareError::Crypto(e.to_string()))?;

    // KEM: wrap the SIK for the recipient.
    let env = kem_share_item(&sik, recipient_public_key, &item_uuid)
        .map_err(|e| ShareError::Crypto(e.to_string()))?;

    Ok(ShareBundle {
        share_id,
        sender_uuid,
        recipient_uuid,
        item_uuid,
        wrapped_sik: b64(&env.wrapped_sik),
        ephemeral_public_key: b64(&env.ephemeral_public_key),
        encrypted_payload: b64(&encrypted_payload),
    })
}

/// Decapsulate and decrypt an incoming share (§4).
///
/// The recipient's `SharingKeyPair` unwraps the SIK from the KEM envelope
/// (`ephemeral_public_key`), which then decrypts the DEM payload. The SIK is
/// held in a [`Zeroizing`] buffer only for the duration of the call.
pub fn accept_share(recipient: &SharingKeyPair, incoming: &IncomingShare) -> Result<Vec<u8>> {
    let wrapped_sik = unb64(&incoming.wrapped_sik, "wrapped_sik")?;
    let ephemeral = unb64(&incoming.ephemeral_public_key, "ephemeral_public_key")?;
    if ephemeral.len() != 32 {
        return Err(ShareError::Crypto(
            "ephemeral_public_key must be 32 bytes".into(),
        ));
    }
    let mut epk = [0u8; 32];
    epk.copy_from_slice(&ephemeral);

    let env = SharedEnvelope {
        wrapped_sik,
        ephemeral_public_key: epk,
    };
    let sik = unwrap_shared_item(&env, recipient, &incoming.item_uuid)
        .map_err(|e| ShareError::Crypto(e.to_string()))?;

    let payload = unb64(&incoming.encrypted_payload, "encrypted_payload")?;
    let ad = payload_ad(&incoming.share_id);
    decrypt_with_ad(&sik, &ad, &payload).map_err(|e| ShareError::Crypto(e.to_string()))
}

// ---------------------------------------------------------------------------
// Groups (1:N, §6)
// ---------------------------------------------------------------------------

/// A sharing group (1:N context) using a unified Group SIK (§6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareGroup {
    pub group_id: Uuid,
    pub name: String,
    pub admin_uuid: Uuid,
}

/// The admin-held group context: the group metadata plus the unified Group SIK.
///
/// The SIK is held in [`Zeroizing`] and is never serialized.
pub struct ShareGroupKey {
    pub group: ShareGroup,
    group_sik: Zeroizing<[u8; MK_LEN]>,
}

impl ShareGroupKey {
    /// Export the raw Group SIK (zeroized on drop). Callers must protect this at
    /// rest (e.g. under the user's master key); it is the basis for decrypting
    /// every item shared into the group.
    pub fn secret_bytes(&self) -> &[u8] {
        &self.group_sik[..]
    }

    /// Reconstruct a [`ShareGroupKey`] from exported metadata + Group SIK.
    pub fn from_secret(group: ShareGroup, secret: &[u8]) -> Result<ShareGroupKey> {
        if secret.len() != MK_LEN {
            return Err(ShareError::Crypto(format!(
                "group sik must be {MK_LEN} bytes, got {}",
                secret.len()
            )));
        }
        let mut sik = Zeroizing::new([0u8; MK_LEN]);
        sik.copy_from_slice(secret);
        Ok(ShareGroupKey {
            group,
            group_sik: sik,
        })
    }

    /// Encrypt a group item's payload under the Group SIK (DEM, §6.1).
    pub fn encrypt_item(&self, item_uuid: &Uuid, plaintext: &[u8]) -> Result<Vec<u8>> {
        let ad = payload_ad(item_uuid);
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        encrypt_with_nonce(&self.group_sik, &nonce, &ad, plaintext)
            .map_err(|e| ShareError::Crypto(e.to_string()))
    }

    /// Decrypt a group item's payload under the Group SIK (DEM, §6.1).
    pub fn decrypt_item(&self, item_uuid: &Uuid, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let ad = payload_ad(item_uuid);
        decrypt_with_ad(&self.group_sik, &ad, ciphertext)
            .map_err(|e| ShareError::Crypto(e.to_string()))
    }
}

/// A Group SIK wrapped for a single member (stored on the server, §6.1-6.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedGroupKey {
    pub group_id: Uuid,
    pub member_uuid: Uuid,
    pub wrapped_sik: String,
    pub ephemeral_public_key: String,
}

/// Result of a group SIK rotation: the new admin key + re-wrapped member keys.
pub struct GroupKeyRotation {
    pub new_key: ShareGroupKey,
    pub rewrapped: Vec<WrappedGroupKey>,
}

/// Create a new sharing group: generates a fresh Group SIK (§6.1).
pub fn create_group(name: String, admin_uuid: Uuid) -> Result<ShareGroupKey> {
    let mut group_sik = Zeroizing::new([0u8; MK_LEN]);
    rand::thread_rng().fill_bytes(&mut *group_sik);
    Ok(ShareGroupKey {
        group: ShareGroup {
            group_id: Uuid::new_v4(),
            name,
            admin_uuid,
        },
        group_sik,
    })
}

/// Wrap the Group SIK for a new member using their `SharingPublicKey` (§6.2).
///
/// The KEM envelope AD is bound to the `group_id`, so it is specific to this
/// group and cannot be replayed against another group.
pub fn add_group_member(
    group_key: &ShareGroupKey,
    member_uuid: Uuid,
    member_public_key: &SharingPublicKey,
) -> Result<WrappedGroupKey> {
    let env = kem_share_item(
        &group_key.group_sik,
        member_public_key,
        &group_key.group.group_id,
    )
    .map_err(|e| ShareError::Crypto(e.to_string()))?;
    Ok(WrappedGroupKey {
        group_id: group_key.group.group_id,
        member_uuid,
        wrapped_sik: b64(&env.wrapped_sik),
        ephemeral_public_key: b64(&env.ephemeral_public_key),
    })
}

/// Member-side: decapsulate the Group SIK from a [`WrappedGroupKey`].
pub fn unwrap_group_key(
    wrapped: &WrappedGroupKey,
    recipient: &SharingKeyPair,
) -> Result<Zeroizing<[u8; MK_LEN]>> {
    let wrapped_sik = unb64(&wrapped.wrapped_sik, "wrapped_sik")?;
    let ephemeral = unb64(&wrapped.ephemeral_public_key, "ephemeral_public_key")?;
    if ephemeral.len() != 32 {
        return Err(ShareError::Crypto(
            "ephemeral_public_key must be 32 bytes".into(),
        ));
    }
    let mut epk = [0u8; 32];
    epk.copy_from_slice(&ephemeral);
    let env = SharedEnvelope {
        wrapped_sik,
        ephemeral_public_key: epk,
    };
    unwrap_shared_item(&env, recipient, &wrapped.group_id)
        .map_err(|e| ShareError::Crypto(e.to_string()))
}

/// Rotate the Group SIK and re-wrap it for `members` (forward secrecy, §6.3).
pub fn rotate_group_sik(
    group: &ShareGroup,
    members: &[(Uuid, SharingPublicKey)],
) -> Result<GroupKeyRotation> {
    let mut group_sik = Zeroizing::new([0u8; MK_LEN]);
    rand::thread_rng().fill_bytes(&mut *group_sik);
    let new_key = ShareGroupKey {
        group: group.clone(),
        group_sik,
    };
    let mut rewrapped = Vec::with_capacity(members.len());
    for (member_uuid, pk) in members {
        let env = kem_share_item(&new_key.group_sik, pk, &group.group_id)
            .map_err(|e| ShareError::Crypto(e.to_string()))?;
        rewrapped.push(WrappedGroupKey {
            group_id: group.group_id,
            member_uuid: *member_uuid,
            wrapped_sik: b64(&env.wrapped_sik),
            ephemeral_public_key: b64(&env.ephemeral_public_key),
        });
    }
    Ok(GroupKeyRotation { new_key, rewrapped })
}

/// Remove a member from a group (§6.3).
///
/// Forward secrecy requires rotating the Group SIK; the caller supplies the
/// remaining members (including the admin) with their public keys. This returns
/// the rotation result the caller must persist and relay.
pub fn remove_group_member(
    group_key: &ShareGroupKey,
    member_to_remove: Uuid,
    remaining_members: &[(Uuid, SharingPublicKey)],
) -> Result<GroupKeyRotation> {
    if remaining_members
        .iter()
        .any(|(u, _)| *u == member_to_remove)
    {
        return Err(ShareError::Group(
            "member being removed must not be in the remaining set".into(),
        ));
    }
    rotate_group_sik(&group_key.group, remaining_members)
}

/// Encrypt a vault item's payload for sharing to every member of a group (§6).
///
/// The payload is sealed under the unified Group SIK (one encryption for N
/// recipients); each member already holds the Group SIK via their wrapped key.
pub fn share_to_group(
    group_key: &ShareGroupKey,
    item_uuid: &Uuid,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    group_key.encrypt_item(item_uuid, plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_accept_roundtrip() {
        // Gate (VTR-026): A shares an item to B's public key, B decapsulates the
        // SIK, decrypts the payload.
        let recipient = SharingKeyPair::generate();
        let sender_uuid = Uuid::new_v4();
        let recipient_uuid = Uuid::new_v4();
        let item_uuid = Uuid::new_v4();
        let plaintext = b"vault item secret payload";

        let bundle = share_item(
            sender_uuid,
            recipient_uuid,
            item_uuid,
            &recipient.public,
            plaintext,
        )
        .unwrap();
        let incoming = IncomingShare::from(bundle);

        let recovered = accept_share(&recipient, &incoming).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_recipient_cannot_accept() {
        let alice = SharingKeyPair::generate();
        let mallory = SharingKeyPair::generate();
        let bundle = share_item(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &alice.public,
            b"top secret",
        )
        .unwrap();
        let incoming = IncomingShare::from(bundle);
        assert!(
            accept_share(&mallory, &incoming).is_err(),
            "non-recipient must not decrypt"
        );
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let recipient = SharingKeyPair::generate();
        let bundle = share_item(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &recipient.public,
            b"integrity check",
        )
        .unwrap();
        let mut incoming = IncomingShare::from(bundle);
        // Flip a byte in the ciphertext; base64 stays valid.
        let mut raw = B64.decode(&incoming.encrypted_payload).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        incoming.encrypted_payload = B64.encode(&raw);
        assert!(accept_share(&recipient, &incoming).is_err());
    }

    #[test]
    fn group_add_remove_rotate() {
        let admin = SharingKeyPair::generate();
        let member = SharingKeyPair::generate();
        let admin_uuid = Uuid::new_v4();
        let member_uuid = Uuid::new_v4();

        let group_key = create_group("Family".into(), admin_uuid).unwrap();

        // Add member: wrap the Group SIK for them.
        let wrapped = add_group_member(&group_key, member_uuid, &member.public).unwrap();
        let member_sik = unwrap_group_key(&wrapped, &member).unwrap();
        let item_uuid = Uuid::new_v4();

        // Admin and member both encrypt/decrypt the same group item.
        let ct = group_key
            .encrypt_item(&item_uuid, b"shared family secret")
            .unwrap();
        let member_ctx = ShareGroupKey {
            group: group_key.group.clone(),
            group_sik: member_sik,
        };
        assert_eq!(
            member_ctx.decrypt_item(&item_uuid, &ct).unwrap(),
            b"shared family secret"
        );

        // Remove member: rotation excludes them, so they can no longer decrypt.
        let rotation =
            remove_group_member(&group_key, member_uuid, &[(admin_uuid, admin.public)]).unwrap();
        let new_ct = rotation
            .new_key
            .encrypt_item(&item_uuid, b"post-removal secret")
            .unwrap();
        // Old member SIK no longer matches the new group payload.
        assert!(member_ctx.decrypt_item(&item_uuid, &new_ct).is_err());
        // Admin can decrypt with the rotated key.
        assert_eq!(
            rotation.new_key.decrypt_item(&item_uuid, &new_ct).unwrap(),
            b"post-removal secret"
        );
        assert_eq!(rotation.rewrapped.len(), 1);
    }
}
