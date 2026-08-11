//! Basic user groups (scope.md §2, "Basic user groups (optional but recommended)").
//!
//! Groups let an admin grant the same per-project permissions to many users at
//! once. A user group belongs to an organization and has member roles.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::permissions::ProjectPermission;

/// A basic user group (optional, recommended for v1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserGroup {
    pub id: Uuid,
    pub name: String,
    /// The organization this group belongs to (groups are org-scoped).
    pub org_id: Option<Uuid>,
    pub created_at: i64,
}

impl UserGroup {
    /// Create a new user group.
    pub fn new(id: Uuid, name: impl Into<String>, org_id: Option<Uuid>, created_at: i64) -> Self {
        Self {
            id,
            name: name.into(),
            org_id,
            created_at,
        }
    }
}

/// The role of a user *within* a user group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroupMemberRole {
    /// Can manage group membership (add/remove members).
    Admin,
    /// Standard group member.
    Member,
}

/// Membership of a user in a group, with the member's in-group role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembership {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub role: GroupMemberRole,
    pub created_at: i64,
}

impl GroupMembership {
    /// Create a new group membership.
    pub fn new(group_id: Uuid, user_id: Uuid, role: GroupMemberRole, created_at: i64) -> Self {
        Self {
            group_id,
            user_id,
            role,
            created_at,
        }
    }
}

/// A per-project permission granted to a group (so every member inherits it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupAccessGrant {
    pub group_id: Uuid,
    pub project_id: Uuid,
    pub permission: ProjectPermission,
    pub granted_by: Uuid,
    pub granted_at: i64,
}

impl GroupAccessGrant {
    /// Create a new group access grant.
    pub fn new(
        group_id: Uuid,
        project_id: Uuid,
        permission: ProjectPermission,
        granted_by: Uuid,
        granted_at: i64,
    ) -> Self {
        Self {
            group_id,
            project_id,
            permission,
            granted_by,
            granted_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn user_group_is_org_scoped() {
        let org = Uuid::new_v4();
        let g = UserGroup::new(Uuid::new_v4(), "Eng", Some(org), 1);
        assert_eq!(g.name, "Eng");
        assert_eq!(g.org_id, Some(org));

        let personal = UserGroup::new(Uuid::new_v4(), "Default", None, 1);
        assert_eq!(personal.org_id, None);
    }

    #[test]
    fn group_membership_carries_in_group_role() {
        let group = Uuid::new_v4();
        let user = Uuid::new_v4();
        let m = GroupMembership::new(group, user, GroupMemberRole::Admin, 2);
        assert_eq!(m.group_id, group);
        assert_eq!(m.user_id, user);
        assert_eq!(m.role, GroupMemberRole::Admin);
    }

    #[test]
    fn group_access_grant() {
        let g = GroupAccessGrant::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProjectPermission::CanManage,
            Uuid::new_v4(),
            3,
        );
        assert_eq!(g.permission, ProjectPermission::CanManage);
    }

    #[test]
    fn group_member_role_serde_roundtrip() {
        let json = serde_json::to_string(&GroupMemberRole::Member).unwrap();
        assert_eq!(json, "\"Member\"");
        let back: GroupMemberRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, GroupMemberRole::Member);
    }
}
