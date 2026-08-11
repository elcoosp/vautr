//! Offboarding: revoke all of a user's access in a few clicks (scope.md §2,
//! "Ultra-simple offboarding").
//!
//! An [`OffboardingRequest`] records the intent to revoke a user's access; an
//! [`OffboardingScope`] controls *what* gets revoked; the resulting
//! [`OffboardingResult`] summarises what was actually revoked.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The scope of an offboarding: which access surfaces to revoke.
///
/// Defaults to nothing; use [`OffboardingScope::REVOKE_ALL`] for the full
/// "revoke all access" one-click action, or build a narrower set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffboardingScope {
    /// Revoke the user's fixed org role (org membership / admin privileges).
    pub revoke_org_role: bool,
    /// Revoke all per-project access grants (project memberships + permissions).
    pub revoke_project_access: bool,
    /// Remove the user from all user groups.
    pub revoke_group_memberships: bool,
    /// Revoke all active sessions for the user.
    pub revoke_sessions: bool,
    /// Revoke the user's sharing PKI keys (no longer able to decrypt shared items).
    pub revoke_sharing_keys: bool,
}

impl Default for OffboardingScope {
    fn default() -> Self {
        Self::REVOKE_ALL
    }
}

impl OffboardingScope {
    /// Revoke *everything*: the full one-click offboarding.
    pub const REVOKE_ALL: Self = Self {
        revoke_org_role: true,
        revoke_project_access: true,
        revoke_group_memberships: true,
        revoke_sessions: true,
        revoke_sharing_keys: true,
    };

    /// An empty scope that revokes nothing (safety default for partial flows).
    pub const NONE: Self = Self {
        revoke_org_role: false,
        revoke_project_access: false,
        revoke_group_memberships: false,
        revoke_sessions: false,
        revoke_sharing_keys: false,
    };

    /// Whether this scope revokes at least one surface.
    pub fn is_empty(&self) -> bool {
        *self == Self::NONE
    }
}

/// The lifecycle status of an offboarding request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffboardingStatus {
    /// Request recorded, revocation not yet completed.
    Pending,
    /// All requested access has been revoked.
    Completed,
}

/// An offboarding request: revoke all of a user's access.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffboardingRequest {
    pub id: Uuid,
    /// The user whose access is being revoked.
    pub user_id: Uuid,
    /// The admin/owner who requested the offboarding.
    pub requested_by: Uuid,
    /// What should be revoked.
    pub scope: OffboardingScope,
    /// Optional audit note describing why.
    pub reason: Option<String>,
    pub status: OffboardingStatus,
    /// Unix epoch millis when the request was created.
    pub requested_at: i64,
    /// Unix epoch millis when revocation completed, if any.
    pub completed_at: Option<i64>,
}

impl OffboardingRequest {
    /// Start a new full-revocation offboarding request.
    pub fn revoke_all(
        id: Uuid,
        user_id: Uuid,
        requested_by: Uuid,
        reason: Option<String>,
        now_ms: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            requested_by,
            scope: OffboardingScope::REVOKE_ALL,
            reason,
            status: OffboardingStatus::Pending,
            requested_at: now_ms,
            completed_at: None,
        }
    }
}

/// Summary of what was actually revoked, produced after applying an offboarding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffboardingResult {
    pub user_id: Uuid,
    /// Number of project access grants revoked.
    pub revoked_project_grants: u64,
    /// Number of group memberships removed.
    pub revoked_group_memberships: u64,
    /// Number of sessions revoked.
    pub revoked_sessions: u64,
    /// Whether the org role was removed.
    pub org_role_revoked: bool,
    /// Whether the sharing PKI keys were revoked.
    pub sharing_keys_revoked: bool,
    /// Unix epoch millis when the offboarding completed.
    pub completed_at: i64,
}

impl OffboardingResult {
    /// Whether every requested surface was fully revoked (nothing left over).
    pub fn is_complete(&self) -> bool {
        self.org_role_revoked && self.sharing_keys_revoked
    }

    /// Total number of individual revocations performed.
    pub fn total_revocations(&self) -> u64 {
        self.revoked_project_grants
            + self.revoked_group_memberships
            + self.revoked_sessions
            + u64::from(self.org_role_revoked)
            + u64::from(self.sharing_keys_revoked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn scope_defaults_to_revoke_all() {
        assert_eq!(OffboardingScope::default(), OffboardingScope::REVOKE_ALL);
        assert!(OffboardingScope::REVOKE_ALL.revoke_org_role);
        assert!(!OffboardingScope::REVOKE_ALL.is_empty());
        assert!(OffboardingScope::NONE.is_empty());
    }

    #[test]
    fn revoke_all_request_starts_pending() {
        let user = Uuid::new_v4();
        let admin = Uuid::new_v4();
        let req = OffboardingRequest::revoke_all(Uuid::new_v4(), user, admin, Some("left company".to_string()), 5);
        assert_eq!(req.user_id, user);
        assert_eq!(req.requested_by, admin);
        assert_eq!(req.status, OffboardingStatus::Pending);
        assert_eq!(req.completed_at, None);
        assert_eq!(req.scope, OffboardingScope::REVOKE_ALL);
    }

    #[test]
    fn result_totals_and_completeness() {
        let user = Uuid::new_v4();
        let r = OffboardingResult {
            user_id: user,
            revoked_project_grants: 4,
            revoked_group_memberships: 2,
            revoked_sessions: 3,
            org_role_revoked: true,
            sharing_keys_revoked: true,
            completed_at: 9,
        };
        assert!(r.is_complete());
        assert_eq!(r.total_revocations(), 4 + 2 + 3 + 1 + 1);

        let partial = OffboardingResult {
            sharing_keys_revoked: false,
            ..r.clone()
        };
        assert!(!partial.is_complete());
    }

    #[test]
    fn offboarding_status_serde_roundtrip() {
        let json = serde_json::to_string(&OffboardingStatus::Completed).unwrap();
        assert_eq!(json, "\"Completed\"");
        let back: OffboardingStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, OffboardingStatus::Completed);
    }
}
