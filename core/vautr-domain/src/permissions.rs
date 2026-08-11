//! Per-project permissions (scope.md §2, "Per-Project permissions").
//!
//! Clear granularity without complicating the UX: **Can View** (see + autofill),
//! **Can Edit** (create / modify / delete items), **Can Manage** (manage who has
//! access to the project).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single per-project permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProjectPermission {
    /// See items + autofill. (Optional "hide password in cleartext" is a separate
    /// per-grant flag; see [`ProjectAccessGrant`].)
    CanView,
    /// Create / modify / delete items.
    CanEdit,
    /// Manage who has access to the project.
    CanManage,
}

/// A per-project permission set granted to a grantee.
///
/// Permissions form an inclusive hierarchy: **Can Manage** implies **Can Edit**
/// which implies **Can View**. The `from_iter` / `grants` helpers normalize a raw
/// set so `CanManage` always sets the lower bits, keeping the invariants simple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectPermissionSet {
    /// See items + autofill.
    pub can_view: bool,
    /// Create / modify / delete items.
    pub can_edit: bool,
    /// Manage who has access to the project.
    pub can_manage: bool,
}

impl ProjectPermissionSet {
    /// Empty permission set (no access).
    pub const NONE: Self = Self {
        can_view: false,
        can_edit: false,
        can_manage: false,
    };

    /// View-only access.
    pub const VIEW_ONLY: Self = Self {
        can_view: true,
        can_edit: false,
        can_manage: false,
    };

    /// View + edit access.
    pub const EDIT: Self = Self {
        can_view: true,
        can_edit: true,
        can_manage: false,
    };

    /// Full manage access (implies view + edit).
    pub const MANAGE: Self = Self {
        can_view: true,
        can_edit: true,
        can_manage: true,
    };

    /// Build a permission set from an iterable of individual permissions,
    /// normalizing the inclusive hierarchy (manage ⇒ edit ⇒ view).
    pub fn from_iter(perms: impl IntoIterator<Item = ProjectPermission>) -> Self {
        let mut set = Self::NONE;
        for p in perms {
            match p {
                ProjectPermission::CanView => set.can_view = true,
                ProjectPermission::CanEdit => {
                    set.can_edit = true;
                    set.can_view = true;
                }
                ProjectPermission::CanManage => {
                    set.can_manage = true;
                    set.can_edit = true;
                    set.can_view = true;
                }
            }
        }
        set
    }

    /// Whether this set grants the given permission.
    pub fn grants(&self, p: ProjectPermission) -> bool {
        match p {
            ProjectPermission::CanView => self.can_view,
            ProjectPermission::CanEdit => self.can_edit,
            ProjectPermission::CanManage => self.can_manage,
        }
    }

    /// Whether this set grants every permission in `other`.
    pub fn contains(&self, other: ProjectPermissionSet) -> bool {
        (!other.can_view || self.can_view)
            && (!other.can_edit || self.can_edit)
            && (!other.can_manage || self.can_manage)
    }

    /// Iterate over the granted permissions.
    pub fn iter(self) -> impl Iterator<Item = ProjectPermission> {
        use ProjectPermission::*;
        [CanView, CanEdit, CanManage]
            .into_iter()
            .filter(move |&p| self.grants(p))
    }

    /// Whether this set grants any access at all.
    pub fn is_empty(&self) -> bool {
        self == &Self::NONE
    }
}

/// The grantee of a per-project access grant: a single user or a user group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AccessGrantee {
    /// A grant to an individual user (by id).
    User { id: Uuid },
    /// A grant to a user group (by id).
    Group { id: Uuid },
}

impl AccessGrantee {
    /// The underlying id (user or group) this grant applies to.
    pub fn id(&self) -> Uuid {
        match self {
            AccessGrantee::User { id } | AccessGrantee::Group { id } => *id,
        }
    }
}

/// A grant of a single permission to a user or group on a project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAccessGrant {
    pub project_id: Uuid,
    pub grantee: AccessGrantee,
    pub permission: ProjectPermission,
    /// Optional flag: for `CanView`, hide the password cleartext (scope.md §2).
    pub hide_password: bool,
    /// The user who granted access.
    pub granted_by: Uuid,
    /// Unix epoch millis when access was granted.
    pub granted_at: i64,
}

impl ProjectAccessGrant {
    /// Create a new grant.
    pub fn new(
        project_id: Uuid,
        grantee: AccessGrantee,
        permission: ProjectPermission,
        granted_by: Uuid,
        granted_at: i64,
    ) -> Self {
        Self {
            project_id,
            grantee,
            permission,
            hide_password: false,
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
    fn hierarchy_is_normalized_from_iter() {
        assert_eq!(
            ProjectPermissionSet::from_iter([ProjectPermission::CanView]),
            ProjectPermissionSet::VIEW_ONLY
        );
        assert_eq!(
            ProjectPermissionSet::from_iter([ProjectPermission::CanEdit]),
            ProjectPermissionSet::EDIT
        );
        assert_eq!(
            ProjectPermissionSet::from_iter([ProjectPermission::CanManage]),
            ProjectPermissionSet::MANAGE
        );
        // Editing a single lower permission alone still yields view+edit.
        let set = ProjectPermissionSet::from_iter([ProjectPermission::CanEdit]);
        assert!(set.grants(ProjectPermission::CanView));
        assert!(set.grants(ProjectPermission::CanEdit));
        assert!(!set.grants(ProjectPermission::CanManage));
    }

    #[test]
    fn contains_and_is_empty() {
        assert!(ProjectPermissionSet::MANAGE.contains(ProjectPermissionSet::EDIT));
        assert!(ProjectPermissionSet::EDIT.contains(ProjectPermissionSet::VIEW_ONLY));
        assert!(!ProjectPermissionSet::VIEW_ONLY.contains(ProjectPermissionSet::EDIT));
        assert!(ProjectPermissionSet::NONE.is_empty());
        assert!(!ProjectPermissionSet::VIEW_ONLY.is_empty());
    }

    #[test]
    fn iter_yields_granted_permissions() {
        let granted: Vec<_> = ProjectPermissionSet::EDIT.iter().collect();
        assert_eq!(
            granted,
            vec![ProjectPermission::CanView, ProjectPermission::CanEdit]
        );
        let manage: Vec<_> = ProjectPermissionSet::MANAGE.iter().collect();
        assert_eq!(manage.len(), 3);
    }

    #[test]
    fn grantee_id_and_grant() {
        let u = Uuid::new_v4();
        let g = Uuid::new_v4();
        assert_eq!(AccessGrantee::User { id: u }.id(), u);
        assert_eq!(AccessGrantee::Group { id: g }.id(), g);

        let grant =
            ProjectAccessGrant::new(Uuid::new_v4(), AccessGrantee::User { id: u }, ProjectPermission::CanEdit, g, 7);
        assert_eq!(grant.permission, ProjectPermission::CanEdit);
        assert!(!grant.hide_password);
    }

    #[test]
    fn grantee_serde_discriminated_union() {
        let g = AccessGrantee::User { id: Uuid::nil() };
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(json, r#"{"type":"user","id":"00000000-0000-0000-0000-000000000000"}"#);
        let back: AccessGrantee = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }
}
