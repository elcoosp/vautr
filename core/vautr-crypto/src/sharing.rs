//! Secure item sharing (ADR-007).
//!
//! Wraps a Secure Item Key (SIK) to a recipient's `SharingPublicKey` using
//! ephemeral X25519 ECDH + an XChaCha20-Poly1305 envelope. The server stores
//! only `wrapped_sik` + `ephemeral_public_key` (zero-knowledge).
//!
//! Enabled by the `sharing` feature (SRS: "Should" priority).

#![cfg(feature = "sharing")]

use crate::aead::NONCE_LEN;
use crate::error::{CryptoError, Result};
use crate::kdf::MK_LEN;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use zeroize::Zeroizing;

/// X25519 public key (32 bytes).
pub type SharingPublicKey = [u8; 32];
/// X25519 secret key (32 bytes).
pub type SharingSecretKey = [u8; 32];

/// A user's long-term sharing keypair.
#[derive(Clone)]
pub struct SharingKeyPair {
    pub public: SharingPublicKey,
    secret: Zeroizing<SharingSecretKey>,
}

impl SharingKeyPair {
    /// Generate a fresh sharing keypair (OsRng-backed).
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self {
            public: public.to_bytes(),
            secret: Zeroizing::new(secret.to_bytes()),
        }
    }

    /// Reconstruct a keypair from stored bytes (e.g. OS keystore).
    pub fn from_secret(secret: SharingSecretKey) -> Self {
        let sk = StaticSecret::from(secret);
        let public = PublicKey::from(&sk);
        Self {
            public: public.to_bytes(),
            secret: Zeroizing::new(sk.to_bytes()),
        }
    }

    fn static_secret(&self) -> StaticSecret {
        StaticSecret::from(*self.secret)
    }
}

/// HKDF info string for the sharing envelope key (ADR-007).
const SHARE_INFO: &[u8] = b"Vautr-share";

fn derive_share_key(shared_secret: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(None, shared_secret);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(SHARE_INFO, &mut *okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Associated-data for the sharing envelope: binds to the recipient key + item.
fn share_ad(recipient_pk: &SharingPublicKey, item_uuid: &Uuid) -> [u8; 48] {
    let mut ad = [0u8; 48];
    ad[..32].copy_from_slice(recipient_pk);
    ad[32..48].copy_from_slice(item_uuid.as_bytes());
    ad
}

/// Output of [`share_item`]: what the server stores.
#[derive(Clone)]
pub struct SharedEnvelope {
    /// `wrapped_sik` ciphertext blob (server column `wrapped_sik`).
    pub wrapped_sik: Vec<u8>,
    /// Ephemeral X25519 public key (server column `ephemeral_public_key`).
    pub ephemeral_public_key: SharingPublicKey,
}

/// Wrap `sik` for `recipient_pk` (ADR-007).
///
/// Generates a fresh ephemeral X25519 key per share; the envelope key is
/// `HKDF-SHA256(X25519(ephemeral_sk, recipient_pk))`, and the SIK is sealed
/// with XChaCha20-Poly1305 under AD = `recipient_pk ‖ item_uuid`.
pub fn share_item(
    sik: &[u8; MK_LEN],
    recipient_pk: &SharingPublicKey,
    item_uuid: &Uuid,
) -> Result<SharedEnvelope> {
    let ephemeral = EphemeralSecret::random_from_rng(rand::thread_rng());
    let ephemeral_pk = PublicKey::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(&PublicKey::from(*recipient_pk));
    let key = derive_share_key(shared.as_bytes());
    let ad = share_ad(recipient_pk, item_uuid);

    // Seal with custom AD using the same XChaCha20-Poly1305 primitive as the
    // rest of the vault (ADR-007: AD = recipient_pk ‖ item_uuid).
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key[..]));
    let nonce_bytes = random_nonce();
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
    let mut ct = cipher
        .encrypt(nonce, chacha20poly1305::aead::Payload { msg: sik, aad: &ad })
        .map_err(|_| CryptoError::Internal("share seal failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len() + 16);
    out.extend_from_slice(&nonce_bytes);
    out.append(&mut ct);

    Ok(SharedEnvelope {
        wrapped_sik: out,
        ephemeral_public_key: ephemeral_pk.to_bytes(),
    })
}

/// Unwrap a [`SharedEnvelope`] held by the recipient (ADR-007).
pub fn unwrap_shared_item(
    envelope: &SharedEnvelope,
    recipient: &SharingKeyPair,
    item_uuid: &Uuid,
) -> Result<Zeroizing<[u8; MK_LEN]>> {
    if envelope.wrapped_sik.len() < NONCE_LEN + 16 {
        return Err(CryptoError::MalformedCiphertext);
    }
    let (nonce_bytes, ct) = envelope.wrapped_sik.split_at(NONCE_LEN);
    let shared = recipient
        .static_secret()
        .diffie_hellman(&PublicKey::from(envelope.ephemeral_public_key));
    let key = derive_share_key(shared.as_bytes());
    let ad = share_ad(&recipient.public, item_uuid);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key[..]));
    let nonce = chacha20poly1305::XNonce::from_slice(nonce_bytes);
    let pt = cipher
        .decrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: ct,
                aad: &ad,
            },
        )
        .map_err(|_| CryptoError::TagMismatch)?;

    if pt.len() != MK_LEN {
        return Err(CryptoError::MalformedCiphertext);
    }
    let mut sik = Zeroizing::new([0u8; MK_LEN]);
    sik.copy_from_slice(&pt);
    Ok(sik)
}

fn random_nonce() -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut n);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn share_and_unwrap_roundtrip() {
        let sik = [7u8; MK_LEN];
        let owner = SharingKeyPair::generate();
        let recipient = SharingKeyPair::generate();
        let item_uuid = uuid!("11112222-3333-4444-5555-666677778888");

        // owner wraps for recipient
        let env = share_item(&sik, &recipient.public, &item_uuid).unwrap();
        assert_eq!(env.ephemeral_public_key.len(), 32);
        assert!(env.wrapped_sik.len() > NONCE_LEN + 16);

        // recipient unwraps
        let recovered = unwrap_shared_item(&env, &recipient, &item_uuid).unwrap();
        assert_eq!(&*recovered, &sik);
    }

    #[test]
    fn wrong_recipient_cannot_unwrap() {
        let sik = [9u8; MK_LEN];
        let owner = SharingKeyPair::generate();
        let recipient = SharingKeyPair::generate();
        let attacker = SharingKeyPair::generate();
        let item_uuid = uuid!("aaaa1111-bbbb-2222-cccc-3333dddd4444");

        let env = share_item(&sik, &recipient.public, &item_uuid).unwrap();
        let res = unwrap_shared_item(&env, &attacker, &item_uuid);
        assert!(res.is_err(), "non-recipient must fail to unwrap");
    }
}
