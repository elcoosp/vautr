//! # vautr-domain
//!
//! Shared data structures for Vautr — **no logic**. These types are the atomic
//! aggregates that cross the FFI boundary (see [`docs/architecture/data.md`]).
//!
//! Every struct mirrors a `data.md` §3 definition. Types marked `[RESTRICTED]`
//! must never cross the JS bridge — they are compiled out of mobile/web via the
//! `desktop-api` feature flag in the FFI crates, not here.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Fast-rendered vault item overview. Encrypted by OEK. Plaintext in local DB.
/// (data.md §3.1)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecryptedOverview {
    pub uuid: Uuid,
    pub title: String,
    pub subtitle: String,
    pub icon_key: String,
    pub urls: Vec<String>,
    pub updated_at: i64,
}

/// TOTP secret. `[RESTRICTED]` — never crosses the JS bridge. (data.md §3.3)
///
/// VTRFIX (telemetry.md §1.3 Debug Ban): no derived `Debug` — `Zeroizing`'s
/// `Debug` forwards to the inner value, so a stray `{:?}` would print the
/// TOTP seed. The manual impl redacts the secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpSecret {
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u8,
    pub secret_base32: Zeroizing<String>,
}

impl std::fmt::Debug for TotpSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TotpSecret")
            .field("algorithm", &self.algorithm)
            .field("digits", &self.digits)
            .field("period", &self.period)
            .field("secret_base32", &"[REDACTED]")
            .finish()
    }
}

/// TOTP hash algorithm. (data.md §3.3)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

/// A custom item field. `[RESTRICTED]`. (data.md §3.4)
///
/// VTRFIX (telemetry.md §1.3 Debug Ban): manual `Debug` — the field value is
/// redacted; only the field name and type render.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    pub name: String,
    pub value: Zeroizing<String>,
    pub field_type: CustomFieldType,
}

impl std::fmt::Debug for CustomField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomField")
            .field("name", &self.name)
            .field("value", &"[REDACTED]")
            .field("field_type", &self.field_type)
            .finish()
    }
}

/// Custom field visibility. (data.md §3.4)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomFieldType {
    Text,
    Hidden,
}

/// Full secret payload. Encrypted by DEK. `[RESTRICTED]` — never leaves core.
/// (data.md §3.2)
///
/// VTRFIX (telemetry.md §1.3 Debug Ban): manual `Debug` — no field of the
/// secret payload is printable. `Zeroizing<String>`'s derived `Debug` would
/// forward to the plaintext.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecryptedSecret {
    pub password: Zeroizing<String>,
    pub totp: Option<TotpSecret>,
    pub notes: Zeroizing<String>,
    pub fields: Vec<CustomField>,
}

impl std::fmt::Debug for DecryptedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecryptedSecret")
            .field("password", &"[REDACTED]")
            .field("totp", &self.totp)
            .field("notes", &"[REDACTED]")
            .field("fields", &self.fields)
            .finish()
    }
}

/// Unencrypted item metadata. (data.md §3.5)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemMetadata {
    pub created_at: i64,
    pub updated_at: i64,
    pub trashed: bool,
}

/// Full aggregate root. `[RESTRICTED]` — JS never sees this. (data.md §3.6)
///
/// VTRFIX (telemetry.md §1.3 Debug Ban): manual `Debug` — renders identity
/// fields only (`uuid`, `enc_key_gen`); the embedded secret never renders.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainModel {
    pub uuid: Uuid,
    pub enc_key_gen: u64,
    pub overview: DecryptedOverview,
    pub secret: DecryptedSecret,
    pub metadata: ItemMetadata,
}

impl std::fmt::Debug for DomainModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomainModel")
            .field("uuid", &self.uuid)
            .field("enc_key_gen", &self.enc_key_gen)
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// TOTP hash algorithm. (data.md §3.3)
pub use TotpAlgorithm as TotpAlg;

// --- Projects model (mlp-scope.md §2, Wave 0.1) -----------------------------
// New, purely additive types. The pre-existing item types above are unchanged;
// item → project is modelled by `ProjectItem` so existing constructors compile.

pub mod groups;
pub mod offboarding;
pub mod permissions;
pub mod project;
pub mod roles;

pub use groups::{GroupAccessGrant, GroupMemberRole, GroupMembership, UserGroup};
pub use offboarding::{OffboardingRequest, OffboardingResult, OffboardingScope, OffboardingStatus};
pub use permissions::{AccessGrantee, ProjectAccessGrant, ProjectPermission, ProjectPermissionSet};
pub use project::{Project, ProjectItem, ProjectKind, ProjectScope};
pub use roles::{OrgMembership, OrgRole, Organization};
