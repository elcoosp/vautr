//! # vautr-sharing
//!
//! Zero-knowledge, end-to-end-encrypted item and group sharing for Vautr.
//!
//! Spec: [`docs/architecture/sharing-pki.md`]. Sharing is a boundary-crossing
//! operation: the sender's SVK never leaves the domain, and the server is only
//! an untrusted relay for encrypted blobs and public keys. Items are encrypted
//! under a dedicated Symmetric Item Key (SIK) via a KEM-DEM protocol built on
//! X25519 (see `vautr_crypto::sharing`).
//!
//! **Scaffold state:** the public types below and the 1:1 / group flow entry
//! points are safe, non-panicking stubs that return
//! [`ShareError::NotImplemented`]. The `no todo!()` roadmap rule is honored:
//! stubs return `Err(...)`, never panic.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod error;

pub use error::{Result, ShareError};

/// A single 1:1 share of one vault item. (sharing-pki.md §3–§5)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareInfo {
    pub share_id: Uuid,
    pub sender_uuid: Uuid,
    pub recipient_uuid: Uuid,
    pub item_uuid: Uuid,
    /// Whether the share is currently active (not revoked).
    pub active: bool,
}

/// An incoming share awaiting decryption/ingestion by the recipient. (§4)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingShare {
    pub share_id: Uuid,
    pub sender_uuid: Uuid,
    pub item_uuid: Uuid,
    /// `WrappedSIK` blob delivered by the server.
    pub wrapped_sik: String,
    /// Ephemeral public key needed to decapsulate the SIK.
    pub ephemeral_public_key: String,
}

/// A sharing group (1:N context) using a unified Group SIK. (§6)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareGroup {
    pub group_id: Uuid,
    pub name: String,
    pub admin_uuid: Uuid,
}

/// Share one vault item with a single recipient (1:1 flow, §3).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn share_item(
    _sender_uuid: Uuid,
    _recipient_uuid: Uuid,
    _item_uuid: Uuid,
) -> Result<ShareInfo> {
    Err(ShareError::NotImplemented)
}

/// Decapsulate and ingest an incoming share into the local vault (§4).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn accept_share(_incoming: IncomingShare) -> Result<ShareInfo> {
    Err(ShareError::NotImplemented)
}

/// Create a new sharing group (Group SIK generation, §6.1).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn create_group(_name: String, _admin_uuid: Uuid) -> Result<ShareGroup> {
    Err(ShareError::NotImplemented)
}

/// Add a member to a group (wrap Group SIK for the new member, §6.2).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn add_group_member(
    _group_id: Uuid,
    _admin_uuid: Uuid,
    _member_uuid: Uuid,
) -> Result<ShareGroup> {
    Err(ShareError::NotImplemented)
}

/// Remove a member from a group (Group SIK rotation, §6.3).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn remove_group_member(
    _group_id: Uuid,
    _admin_uuid: Uuid,
    _member_uuid: Uuid,
) -> Result<ShareGroup> {
    Err(ShareError::NotImplemented)
}

/// Share a vault item to every member of a group (§6).
///
/// # Stub
/// Returns [`ShareError::NotImplemented`] until the sharing pipeline lands.
pub fn share_to_group(_group_id: Uuid, _item_uuid: Uuid) -> Result<ShareInfo> {
    Err(ShareError::NotImplemented)
}
