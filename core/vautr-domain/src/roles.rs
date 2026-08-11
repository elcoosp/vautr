//! Fixed organization roles and the organization / membership model.
//!
//! Canonical scope: `docs/architecture/mlp-scope.md` §2 ("Fixed, clear org roles").

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The fixed organization role of a user (scope.md §2).
///
/// Role ordering is strictly **Owner > Admin > Manager > Member**. This is
/// implemented via [`OrgRole::rank`] (not declaration order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrgRole {
    /// Total control over the organization.
    Owner,
    /// Manages users, projects, and permissions.
    Admin,
    /// Creates and manages access on their own projects only.
    Manager,
    /// Standard user.
    Member,
}

impl PartialOrd for OrgRole {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrgRole {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl OrgRole {
    /// Rank used to order roles. `Owner` = 3, `Admin` = 2, `Manager` = 1,
    /// `Member` = 0. Higher is more privileged.
    pub fn rank(self) -> u8 {
        match self {
            OrgRole::Owner => 3,
            OrgRole::Admin => 2,
            OrgRole::Manager => 1,
            OrgRole::Member => 0,
        }
    }

    /// Whether this role can administer the whole organization (manage users,
    /// projects, permissions org-wide).
    pub fn can_manage_org(self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Admin)
    }

    /// Whether this role can create new projects. `Member` cannot.
    pub fn can_create_projects(self) -> bool {
        !matches!(self, OrgRole::Member)
    }

    /// Whether this role can offboard (revoke all access for) other users.
    pub fn can_offboard(self) -> bool {
        self.can_manage_org()
    }
}

/// An organization — the tenant that scopes shared projects, teams, and roles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub created_at: i64,
}

impl Organization {
    /// Create a new organization with the given id/name.
    pub fn new(id: Uuid, name: impl Into<String>, created_at: i64) -> Self {
        Self {
            id,
            name: name.into(),
            created_at,
        }
    }
}

/// A user's fixed role within an organization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgMembership {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: OrgRole,
    pub created_at: i64,
}

impl OrgMembership {
    /// Create a new org membership.
    pub fn new(org_id: Uuid, user_id: Uuid, role: OrgRole, created_at: i64) -> Self {
        Self {
            org_id,
            user_id,
            role,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn role_rank_and_order() {
        assert!(OrgRole::Owner > OrgRole::Admin);
        assert!(OrgRole::Admin > OrgRole::Manager);
        assert!(OrgRole::Manager > OrgRole::Member);
        assert_eq!(OrgRole::Member.rank(), 0);
        assert_eq!(OrgRole::Manager.rank(), 1);
        assert_eq!(OrgRole::Admin.rank(), 2);
        assert_eq!(OrgRole::Owner.rank(), 3);
    }

    #[test]
    fn role_capabilities() {
        assert!(OrgRole::Owner.can_manage_org());
        assert!(OrgRole::Admin.can_manage_org());
        assert!(!OrgRole::Manager.can_manage_org());
        assert!(!OrgRole::Member.can_manage_org());

        assert!(OrgRole::Manager.can_create_projects());
        assert!(!OrgRole::Member.can_create_projects());

        assert!(OrgRole::Admin.can_offboard());
        assert!(!OrgRole::Manager.can_offboard());
    }

    #[test]
    fn org_and_membership() {
        let org = Organization::new(Uuid::new_v4(), "Acme", 1);
        assert_eq!(org.name, "Acme");
        let user = Uuid::new_v4();
        let m = OrgMembership::new(org.id, user, OrgRole::Owner, 2);
        assert_eq!(m.org_id, org.id);
        assert_eq!(m.user_id, user);
        assert_eq!(m.role, OrgRole::Owner);
    }

    #[test]
    fn role_serde_roundtrip() {
        let json = serde_json::to_string(&OrgRole::Admin).unwrap();
        assert_eq!(json, "\"Admin\"");
        let back: OrgRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, OrgRole::Admin);
    }
}
