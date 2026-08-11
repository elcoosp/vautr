//! Projects model.
//!
//! Introduces the single **Project** concept that replaces the Folders /
//! Collections mental model (canonical scope: `docs/architecture/mlp-scope.md` §2,
//! plan: `docs/architecture/mlp-wave-plan.md` Wave 0.1).
//!
//! A project is `Personal` (owned by one user) or `Shared` (shared within an
//! organization/team). Items belong to **exactly one** project; this is modelled
//! additively by [`ProjectItem`] so the pre-existing item types (`DomainModel`,
//! `DecryptedOverview`, ...) keep compiling unchanged.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The kind of a project (scope.md §2).
///
/// * `Personal` — belongs to a single user; never shared.
/// * `Shared` — shared within the organization / team.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Personal,
    Shared,
}

/// The org/team scope a project belongs to (scope.md §2, "org/team scope").
///
/// `None` fields mean the project is not scoped to that dimension, which is the
/// case for `Personal` projects and for org-independent shared projects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectScope {
    /// The organization that owns this project (shared projects).
    pub org_id: Option<Uuid>,
    /// The team within the organization this project belongs to (optional).
    pub team_id: Option<Uuid>,
}

impl ProjectScope {
    /// A project scoped to no org and no team (e.g. a personal project).
    pub fn unset() -> Self {
        Self::default()
    }

    /// A project scoped to an organization and an optional team.
    pub fn new(org_id: Option<Uuid>, team_id: Option<Uuid>) -> Self {
        Self { org_id, team_id }
    }

    /// Whether this scope pins the project to any org or team.
    pub fn is_scoped(&self) -> bool {
        self.org_id.is_some() || self.team_id.is_some()
    }

    /// The organization this project belongs to, if any.
    pub fn org_id(&self) -> Option<Uuid> {
        self.org_id
    }
}

/// A single **Project** — the atomic organizational unit for vaults and secrets
/// (scope.md §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Stable project identifier.
    pub id: Uuid,
    /// Human-readable project name.
    pub name: String,
    /// Personal or shared within the org.
    pub kind: ProjectKind,
    /// Org/team scope the project belongs to.
    pub scope: ProjectScope,
    /// The user that created / owns this project.
    pub owner_user_id: Uuid,
    /// Unix epoch millis at creation.
    pub created_at: i64,
    /// Unix epoch millis at last update.
    pub updated_at: i64,
}

impl Project {
    /// Create a new personal project owned by `owner_user_id`.
    pub fn personal(id: Uuid, name: impl Into<String>, owner_user_id: Uuid, now_ms: i64) -> Self {
        Self {
            id,
            name: name.into(),
            kind: ProjectKind::Personal,
            scope: ProjectScope::unset(),
            owner_user_id,
            created_at: now_ms,
            updated_at: now_ms,
        }
    }

    /// Create a new shared project within an organization (and optional team).
    pub fn shared(
        id: Uuid,
        name: impl Into<String>,
        org_id: Uuid,
        team_id: Option<Uuid>,
        owner_user_id: Uuid,
        now_ms: i64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind: ProjectKind::Shared,
            scope: ProjectScope::new(Some(org_id), team_id),
            owner_user_id,
            created_at: now_ms,
            updated_at: now_ms,
        }
    }
}

/// Additive association: an item belongs to **exactly one** project (scope.md §2).
///
/// This is a standalone type rather than a field on `DomainModel` so the existing
/// item types keep compiling and no constructor across the workspace is disturbed.
/// The backing `project_items` table uses `item_uuid` as its primary key, which
/// enforces the "one item → one project" invariant at the schema level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectItem {
    /// The vault item being associated.
    pub item_uuid: Uuid,
    /// The single project the item belongs to.
    pub project_id: Uuid,
    /// The user who added the item to the project.
    pub added_by: Uuid,
    /// Unix epoch millis when the association was created.
    pub added_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn personal_project_is_unscoped() {
        let now = 1_700_000_000_000i64;
        let owner = Uuid::new_v4();
        let p = Project::personal(Uuid::new_v4(), "My Vault", owner, now);
        assert_eq!(p.kind, ProjectKind::Personal);
        assert_eq!(p.owner_user_id, owner);
        assert_eq!(p.created_at, now);
        assert_eq!(p.updated_at, now);
        assert!(!p.scope.is_scoped());
        assert_eq!(p.scope.org_id(), None);
    }

    #[test]
    fn shared_project_carries_org_and_optional_team_scope() {
        let org = Uuid::new_v4();
        let team = Uuid::new_v4();
        let p = Project::shared(Uuid::new_v4(), "Secrets", org, Some(team), Uuid::new_v4(), 5);
        assert_eq!(p.kind, ProjectKind::Shared);
        assert!(p.scope.is_scoped());
        assert_eq!(p.scope.org_id, Some(org));
        assert_eq!(p.scope.team_id, Some(team));

        let org_only = Project::shared(Uuid::new_v4(), "Secrets", org, None, Uuid::new_v4(), 5);
        assert_eq!(org_only.scope.team_id, None);
    }

    #[test]
    fn project_scope_constructors() {
        assert!(!ProjectScope::unset().is_scoped());
        let s = ProjectScope::new(Some(Uuid::new_v4()), None);
        assert!(s.is_scoped());
        assert!(s.org_id().is_some());
        let d = ProjectScope::default();
        assert!(!d.is_scoped());
    }

    #[test]
    fn project_item_associates_item_to_one_project() {
        let item = Uuid::new_v4();
        let project = Uuid::new_v4();
        let by = Uuid::new_v4();
        let assoc = ProjectItem {
            item_uuid: item,
            project_id: project,
            added_by: by,
            added_at: 99,
        };
        assert_eq!(assoc.item_uuid, item);
        assert_eq!(assoc.project_id, project);
        assert_eq!(assoc.added_by, by);
    }

    #[test]
    fn project_kind_serde_roundtrip() {
        let json = serde_json::to_string(&ProjectKind::Shared).unwrap();
        assert_eq!(json, "\"shared\"");
        let back: ProjectKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ProjectKind::Shared);
    }
}
