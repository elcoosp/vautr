//! Sharing PKI transport + group store (sharing-pki.md §5-6).
//!
//! `VautrClient` routes 1:1 shares and group management through a [`ShareTransport`]
//! (HTTP in production, an in-memory relay in tests). The transport is a dumb,
//! untrusted relay: it carries public keys, KEM envelopes (`wrapped_sik` +
//! `ephemeral_public_key`) and DEM ciphertext, never the SIK or plaintext.
//!
//! This module also owns the in-memory [`ShareGroupStore`] the client uses to
//! hold admin-side `ShareGroupKey`s during a session (forward secrecy on
//! membership change, §6.3).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use base64::Engine;
use vautr_crypto::sharing::{SharingKeyPair, SharingPublicKey};
use vautr_sharing::{IncomingShare, ShareBundle, ShareGroupKey, WrappedGroupKey};

/// Handle type for the sharing transport (avoids nested `>` in struct fields).
pub type ShareTransportHandle = Arc<dyn ShareTransport>;

/// The untrusted relay boundary for sharing (§5-6).
pub trait ShareTransport: Send + Sync {
    /// Look up a user's published X25519 `SharingPublicKey` (§5 directory).
    fn fetch_public_key(
        &self,
        user_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<SharingPublicKey, String>> + Send>>;
    /// Relay a 1:1 share (KEM envelope) to the recipient's inbox (§5).
    fn post_share(&self, bundle: &ShareBundle) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Deliver the DEM ciphertext for a share (§5 "Upload Shared Payload").
    fn post_share_payload(
        &self,
        share_id: Uuid,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Fetch the recipient's inbox of pending shares (KEM + payload) (§5).
    fn fetch_inbox(&self) -> Pin<Box<dyn Future<Output = Result<Vec<IncomingShare>, String>> + Send>>;
    /// Revoke a share and its payload (§5 "Revoke Share (1:1)").
    fn revoke_share(&self, share_id: Uuid) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Persist a wrapped Group SIK for a member (§6.2).
    fn store_group_wrapped_key(
        &self,
        wrapped: &WrappedGroupKey,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Replace all wrapped Group SIKs after a rotation (§6.3).
    fn replace_group_wrapped_keys(
        &self,
        wrapped: Vec<WrappedGroupKey>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// State backing the in-memory [`InMemoryShareRelay`].
#[derive(Default)]
struct RelayState {
    public_keys: HashMap<Uuid, SharingPublicKey>,
    inbox: HashMap<Uuid, Vec<IncomingShare>>,
    payloads: HashMap<Uuid, Vec<u8>>,
    group_wrapped: HashMap<(Uuid, Uuid), WrappedGroupKey>, // (group, member)
}

/// An in-memory [`ShareTransport`] for tests and offline/standalone use.
///
/// Round-trips public keys, 1:1 shares and group wrapped keys between two
/// `VautrClient` instances without a server.
#[derive(Clone, Default)]
pub struct InMemoryShareRelay {
    state: Arc<Mutex<RelayState>>,
}

impl InMemoryShareRelay {
    /// Publish `user_uuid`'s sharing public key (the sender looks it up later).
    pub fn publish_public_key(&self, user_uuid: Uuid, pk: SharingPublicKey) {
        self.state.lock().unwrap().public_keys.insert(user_uuid, pk);
    }
}

impl ShareTransport for InMemoryShareRelay {
    fn fetch_public_key(
        &self,
        user_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<SharingPublicKey, String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            state
                .lock()
                .unwrap()
                .public_keys
                .get(&user_uuid)
                .copied()
                .ok_or_else(|| "recipient has no sharing public key".into())
        })
    }

    fn post_share(
        &self,
        bundle: &ShareBundle,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        let bundle = bundle.clone();
        Box::pin(async move {
            let mut g = state.lock().unwrap();
            g.inbox
                .entry(bundle.recipient_uuid)
                .or_default()
                .push(bundle.clone().into());
            Ok(())
        })
    }

    fn post_share_payload(
        &self,
        share_id: Uuid,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            state.lock().unwrap().payloads.insert(share_id, payload);
            Ok(())
        })
    }

    fn fetch_inbox(&self) -> Pin<Box<dyn Future<Output = Result<Vec<IncomingShare>, String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            // Merged view: every share's payload is attached from the payload map
            // (the DEM ciphertext is delivered separately in the protocol).
            let g = state.lock().unwrap();
            let mut out = Vec::new();
            for shares in g.inbox.values() {
                for s in shares {
                    let mut s = s.clone();
                    if let Some(p) = g.payloads.get(&s.share_id) {
                        s.encrypted_payload =
                            base64::engine::general_purpose::STANDARD.encode(p);
                    }
                    out.push(s);
                }
            }
            Ok(out)
        })
    }

    fn revoke_share(
        &self,
        share_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            let mut g = state.lock().unwrap();
            g.payloads.remove(&share_id);
            for shares in g.inbox.values_mut() {
                shares.retain(|s| s.share_id != share_id);
            }
            Ok(())
        })
    }

    fn store_group_wrapped_key(
        &self,
        wrapped: &WrappedGroupKey,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        let wrapped = wrapped.clone();
        Box::pin(async move {
            state
                .lock()
                .unwrap()
                .group_wrapped
                .insert((wrapped.group_id, wrapped.member_uuid), wrapped);
            Ok(())
        })
    }

    fn replace_group_wrapped_keys(
        &self,
        wrapped: Vec<WrappedGroupKey>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            let mut g = state.lock().unwrap();
            let group_id = wrapped.first().map(|w| w.group_id);
            if let Some(gid) = group_id {
                g.group_wrapped
                    .retain(|(g, _), _| *g != gid);
            }
            for w in wrapped {
                g.group_wrapped.insert((w.group_id, w.member_uuid), w);
            }
            Ok(())
        })
    }
}

/// In-memory store of the client's own sharing keypair + admin group keys.
///
/// Held in `Zeroizing`-style memory (the keypair type itself zeroes its secret).
/// The sharing keypair is generated at vault creation (VTR-057) and kept here.
/// `ShareGroupKey` is stored behind an `Arc` because it is not `Clone`
/// (`vautr-sharing` holds its Group SIK in a `Zeroizing` buffer).
pub struct ShareGroupStore {
    groups: Mutex<HashMap<Uuid, Arc<ShareGroupKey>>>,
}

impl ShareGroupStore {
    /// Fresh (empty) group store.
    pub fn new() -> Self {
        Self {
            groups: Mutex::new(HashMap::new()),
        }
    }

    /// Insert an admin-held group key (on `create_group`).
    pub fn put(&self, key: ShareGroupKey) {
        self.groups
            .lock()
            .unwrap()
            .insert(key.group.group_id, Arc::new(key));
    }

    /// Look up an admin-held group key by id (Arc clone; not a deep copy).
    pub fn get(&self, group_id: &Uuid) -> Option<Arc<ShareGroupKey>> {
        self.groups.lock().unwrap().get(group_id).cloned()
    }

    /// Replace a group's key after a rotation (§6.3).
    pub fn replace(&self, key: ShareGroupKey) {
        self.put(key);
    }

    /// Remove a group (deletion / local cleanup).
    pub fn remove(&self, group_id: &Uuid) {
        self.groups.lock().unwrap().remove(group_id);
    }

    /// Len (introspection/tests).
    pub fn len(&self) -> usize {
        self.groups.lock().unwrap().len()
    }
}

impl Default for ShareGroupStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a fresh sharing keypair for vault creation (VTR-057).
pub fn generate_vault_sharing_keypair() -> SharingKeyPair {
    SharingKeyPair::generate()
}
