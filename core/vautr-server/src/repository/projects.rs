//! Projects repository domain (mlp-wave-plan.md §3 A1, mlp-scope.md §2).
//!
//! Backing schema: `migrations/0007_projects.sql` (owned by this wave).
//! This module owns every query that reads/writes the Projects model:
//! organizations + org roles, projects, per-project access grants, user
//! groups, and offboarding (revoke-all).
//!
//! Storage conventions (match the migration CHECK constraints):
//! * org roles are stored as PascalCase TEXT (`Owner`/`Admin`/`Manager`/`Member`)
//!   matching `vautr-domain::roles::OrgRole`'s default serde form;
//! * per-project permissions are stored as lowercase TEXT
//!   (`can_view`/`can_edit`/`can_manage`);
//! * project kinds are stored as lowercase TEXT (`personal`/`shared`).

use sqlx::FromRow;

use crate::repository::Repository;
use vautr_domain::{
    GroupMemberRole, OffboardingRequest, OrgRole, Project, ProjectKind, ProjectPermission,
};

/// Row mirror of `projects` (0007_projects.sql).
#[derive(Debug, Clone, FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub org_id: Option<String>,
    pub team_id: Option<String>,
    pub owner_user_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProjectRow {
    /// Parse the stored lowercase kind into the domain [`ProjectKind`].
    pub fn kind(&self) -> ProjectKind {
        match self.kind.as_str() {
            "shared" => ProjectKind::Shared,
            _ => ProjectKind::Personal,
        }
    }
}

/// Row mirror of `project_access` (user grants only; `grantee_group_id IS NULL`).
#[derive(Debug, Clone, FromRow)]
pub struct AccessRow {
    pub id: String,
    pub project_id: String,
    pub grantee_user_id: Option<String>,
    pub grantee_group_id: Option<String>,
    pub permission: String,
    pub hide_password: i64,
    pub granted_by: String,
    pub granted_at: i64,
}

/// Row mirror of `user_groups` (0007_projects.sql).
#[derive(Debug, Clone, FromRow)]
pub struct UserGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Row mirror of `user_group_members` (0007_projects.sql).
#[derive(Debug, Clone, FromRow)]
pub struct GroupMemberRow {
    pub group_id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: i64,
}

/// Row mirror of `offboardings` (0007_projects.sql).
#[derive(Debug, Clone, FromRow)]
pub struct OffboardingRow {
    pub id: String,
    pub user_id: String,
    pub requested_by: String,
    pub reason: Option<String>,
    pub status: String,
    pub requested_at: i64,
    pub completed_at: Option<i64>,
}

fn perm_to_db(p: ProjectPermission) -> &'static str {
    match p {
        ProjectPermission::CanView => "can_view",
        ProjectPermission::CanEdit => "can_edit",
        ProjectPermission::CanManage => "can_manage",
    }
}

fn perm_from_db(s: &str) -> Option<ProjectPermission> {
    match s {
        "can_view" => Some(ProjectPermission::CanView),
        "can_edit" => Some(ProjectPermission::CanEdit),
        "can_manage" => Some(ProjectPermission::CanManage),
        _ => None,
    }
}

fn role_to_db(r: OrgRole) -> &'static str {
    match r {
        OrgRole::Owner => "Owner",
        OrgRole::Admin => "Admin",
        OrgRole::Manager => "Manager",
        OrgRole::Member => "Member",
    }
}

fn role_from_db(s: &str) -> Option<OrgRole> {
    match s {
        "Owner" => Some(OrgRole::Owner),
        "Admin" => Some(OrgRole::Admin),
        "Manager" => Some(OrgRole::Manager),
        "Member" => Some(OrgRole::Member),
        _ => None,
    }
}

fn group_role_to_db(r: GroupMemberRole) -> &'static str {
    match r {
        GroupMemberRole::Admin => "Admin",
        GroupMemberRole::Member => "Member",
    }
}

fn group_role_from_db(s: &str) -> Option<GroupMemberRole> {
    match s {
        "Admin" => Some(GroupMemberRole::Admin),
        "Member" => Some(GroupMemberRole::Member),
        _ => None,
    }
}

/// The most privileged of two per-project permissions (CanManage > CanEdit > CanView).
fn max_perm(a: Option<ProjectPermission>, b: ProjectPermission) -> Option<ProjectPermission> {
    Some(match a {
        None => b,
        Some(a) => {
            if a >= b {
                a
            } else {
                b
            }
        }
    })
}

impl Repository {
    // ------------------------------------------------------------------
    // Organizations + org roles
    // ------------------------------------------------------------------

    /// Insert a new organization.
    pub async fn create_org(&self, id: &str, name: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO organizations (id, name, created_at) VALUES (?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert (or update) a user's fixed role within an organization.
    pub async fn set_org_member(
        &self,
        org_id: &str,
        user_id: &str,
        role: OrgRole,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO org_members (org_id, user_id, role, created_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(org_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role_to_db(role))
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a user's role from an organization. Returns rows deleted.
    pub async fn remove_org_member(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM org_members WHERE org_id = ? AND user_id = ?")
            .bind(org_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Look up a user's role in an organization.
    pub async fn get_org_role(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<OrgRole>, sqlx::Error> {
        let role: Option<String> = sqlx::query_scalar("SELECT role FROM org_members WHERE org_id = ? AND user_id = ?")
            .bind(org_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(role.and_then(|r| role_from_db(&r)))
    }

    /// The first organization the user belongs to, if any.
    pub async fn get_user_org_id(&self, user_id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT org_id FROM org_members WHERE user_id = ? ORDER BY created_at LIMIT 1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Every (org_id, role) the user holds (used for offboarding rank checks).
    pub async fn get_user_org_roles(&self, user_id: &str) -> Result<Vec<(String, OrgRole)>, sqlx::Error> {
        #[derive(FromRow)]
        struct OrgRoleRow {
            org_id: String,
            role: String,
        }
        let rows: Vec<OrgRoleRow> =
            sqlx::query_as::<_, OrgRoleRow>("SELECT org_id, role FROM org_members WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| role_from_db(&r.role).map(|role| (r.org_id, role)))
            .collect())
    }

    /// Ensure the user belongs to an organization, creating a default one with
    /// them as `Owner` if they are not yet in any org. Returns the org id.
    pub async fn ensure_org_for_user(&self, user_id: &str, now: i64) -> Result<String, sqlx::Error> {
        if let Some(org) = self.get_user_org_id(user_id).await? {
            return Ok(org);
        }
        let org_id = uuid::Uuid::new_v4().to_string();
        self.create_org(&org_id, "Default Organization", now).await?;
        self.set_org_member(&org_id, user_id, OrgRole::Owner, now).await?;
        Ok(org_id)
    }

    // ------------------------------------------------------------------
    // Projects CRUD
    // ------------------------------------------------------------------

    /// Insert a project row from the domain [`Project`].
    pub async fn create_project(&self, p: &Project) -> Result<(), sqlx::Error> {
        let org_id = p.scope.org_id().map(|u| u.to_string());
        sqlx::query(
            "INSERT INTO projects (id, name, kind, org_id, team_id, owner_user_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, NULL, ?, ?, ?)",
        )
        .bind(p.id.to_string())
        .bind(&p.name)
        .bind(match p.kind {
            ProjectKind::Personal => "personal",
            ProjectKind::Shared => "shared",
        })
        .bind(org_id)
        .bind(p.owner_user_id.to_string())
        .bind(p.created_at)
        .bind(p.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a project by id.
    pub async fn get_project(&self, id: &str) -> Result<Option<ProjectRow>, sqlx::Error> {
        sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Update a project's name/description and bump `updated_at`.
    pub async fn update_project_meta(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE projects SET name = ?, description = ?, updated_at = ? WHERE id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a project (project_access rows cascade via FK).
    pub async fn delete_project(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The owner of a project, if any.
    pub async fn project_owner(&self, id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT owner_user_id FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// All projects visible to `user_id`: owned, directly granted, granted via a
    /// group the user belongs to, or org-wide when the user is an org Owner/Admin.
    pub async fn list_projects_for_user(&self, user_id: &str) -> Result<Vec<ProjectRow>, sqlx::Error> {
        let mut ids = std::collections::HashSet::new();

        // Owned.
        for id in sqlx::query_scalar::<_, String>("SELECT id FROM projects WHERE owner_user_id = ?")
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?
        {
            ids.insert(id);
        }
        // Direct per-user grant.
        for id in sqlx::query_scalar::<_, String>(
            "SELECT project_id FROM project_access WHERE grantee_user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        {
            ids.insert(id);
        }
        // Group membership grant.
        for id in sqlx::query_scalar::<_, String>(
            "SELECT pa.project_id FROM project_access pa \
             JOIN user_group_members ugm ON ugm.group_id = pa.grantee_group_id \
             WHERE ugm.user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        {
            ids.insert(id);
        }
        // Org Owner/Admin sees all projects in their orgs.
        for id in sqlx::query_scalar::<_, String>(
            "SELECT p.id FROM projects p \
             JOIN org_members om ON om.org_id = p.org_id \
             WHERE om.user_id = ? AND om.role IN ('Owner', 'Admin')",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        {
            ids.insert(id);
        }

        let mut projects = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(row) = self.get_project(&id).await? {
                projects.push(row);
            }
        }
        projects.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(projects)
    }

    // ------------------------------------------------------------------
    // Per-project access grants
    // ------------------------------------------------------------------

    /// Grant a per-project permission to a user, replacing any prior grant
    /// (a user holds at most one permission per project — the inclusive
    /// hierarchy is normalized by the domain).
    pub async fn grant_user_project_access(
        &self,
        project_id: &str,
        user_id: &str,
        permission: ProjectPermission,
        hide_password: bool,
        granted_by: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM project_access WHERE project_id = ? AND grantee_user_id = ?")
            .bind(project_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO project_access \
             (id, project_id, grantee_user_id, grantee_group_id, permission, hide_password, granted_by, granted_at) \
             VALUES (?, ?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(project_id)
        .bind(user_id)
        .bind(perm_to_db(permission))
        .bind(i64::from(hide_password))
        .bind(granted_by)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Grant a per-project permission to a group (every member inherits it).
    pub async fn grant_group_project_access(
        &self,
        project_id: &str,
        group_id: &str,
        permission: ProjectPermission,
        hide_password: bool,
        granted_by: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM project_access WHERE project_id = ? AND grantee_group_id = ?")
            .bind(project_id)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO project_access \
             (id, project_id, grantee_user_id, grantee_group_id, permission, hide_password, granted_by, granted_at) \
             VALUES (?, ?, NULL, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(project_id)
        .bind(group_id)
        .bind(perm_to_db(permission))
        .bind(i64::from(hide_password))
        .bind(granted_by)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// A single user's current grant on a project (direct only).
    pub async fn get_user_grant(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<AccessRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessRow>(
            "SELECT * FROM project_access WHERE project_id = ? AND grantee_user_id = ?",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// All per-user grants on a project (for the member list).
    pub async fn list_project_members(&self, project_id: &str) -> Result<Vec<AccessRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessRow>(
            "SELECT * FROM project_access WHERE project_id = ? AND grantee_user_id IS NOT NULL \
             ORDER BY granted_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    /// All per-group grants on a project.
    pub async fn list_project_group_grants(
        &self,
        project_id: &str,
    ) -> Result<Vec<AccessRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessRow>(
            "SELECT * FROM project_access WHERE project_id = ? AND grantee_group_id IS NOT NULL \
             ORDER BY granted_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Remove a user's per-project grant. Returns rows deleted.
    pub async fn remove_user_grant(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM project_access WHERE project_id = ? AND grantee_user_id = ?")
            .bind(project_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// The most-privileged permission `user_id` holds on a project, considering
    /// ownership, direct grants, and inherited group grants.
    pub async fn effective_permission(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<ProjectPermission>, sqlx::Error> {
        if let Some(owner) = self.project_owner(project_id).await? {
            if owner == user_id {
                return Ok(Some(ProjectPermission::CanManage));
            }
        }
        let mut best: Option<ProjectPermission> = None;
        let direct: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT permission FROM project_access WHERE project_id = ? AND grantee_user_id = ?",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        for p in direct {
            if let Some(pp) = perm_from_db(&p) {
                best = max_perm(best, pp);
            }
        }
        let group: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT pa.permission FROM project_access pa \
             JOIN user_group_members ugm ON ugm.group_id = pa.grantee_group_id \
             WHERE pa.project_id = ? AND ugm.user_id = ?",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        for p in group {
            if let Some(pp) = perm_from_db(&p) {
                best = max_perm(best, pp);
            }
        }
        Ok(best)
    }

    // ------------------------------------------------------------------
    // User groups
    // ------------------------------------------------------------------

    /// Create a user group in an organization.
    pub async fn create_user_group(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        org_id: Option<&str>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_groups (id, name, description, org_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(org_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a user group by id.
    pub async fn get_user_group(&self, id: &str) -> Result<Option<UserGroupRow>, sqlx::Error> {
        sqlx::query_as::<_, UserGroupRow>("SELECT * FROM user_groups WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Rename / re-describe a user group.
    pub async fn update_user_group(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE user_groups SET name = ?, description = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(description)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete a user group (members + access grants cascade via FK).
    pub async fn delete_user_group(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM user_groups WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Groups that hold a per-project access grant on `project_id`.
    pub async fn list_groups_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<UserGroupRow>, sqlx::Error> {
        let ids: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT ug.id FROM user_groups ug \
             JOIN project_access pa ON pa.grantee_group_id = ug.id \
             WHERE pa.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut groups = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(g) = self.get_user_group(&id).await? {
                groups.push(g);
            }
        }
        Ok(groups)
    }

    /// Insert (or update) a member of a user group.
    pub async fn add_user_group_member(
        &self,
        group_id: &str,
        user_id: &str,
        role: GroupMemberRole,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_group_members (group_id, user_id, role, created_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(group_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(group_id)
        .bind(user_id)
        .bind(group_role_to_db(role))
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a member from a user group. Returns rows deleted.
    pub async fn remove_user_group_member(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM user_group_members WHERE group_id = ? AND user_id = ?")
            .bind(group_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// List the members of a user group.
    pub async fn list_user_group_members(&self, group_id: &str) -> Result<Vec<GroupMemberRow>, sqlx::Error> {
        sqlx::query_as::<_, GroupMemberRow>(
            "SELECT * FROM user_group_members WHERE group_id = ? ORDER BY created_at",
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Whether `user_id` is a member of `group_id`.
    pub async fn user_in_group(&self, user_id: &str, group_id: &str) -> Result<bool, sqlx::Error> {
        let row: Option<String> = sqlx::query_scalar(
            "SELECT user_id FROM user_group_members WHERE group_id = ? AND user_id = ?",
        )
        .bind(group_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// The caller's in-group role, if any.
    pub async fn group_member_role(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<Option<GroupMemberRole>, sqlx::Error> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM user_group_members WHERE group_id = ? AND user_id = ?",
        )
        .bind(group_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(role.and_then(|r| group_role_from_db(&r)))
    }

    // ------------------------------------------------------------------
    // Offboarding (revoke all)
    // ------------------------------------------------------------------

    /// Record an offboarding request.
    pub async fn create_offboarding(&self, req: &OffboardingRequest) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO offboardings (id, user_id, requested_by, reason, status, requested_at, completed_at) \
             VALUES (?, ?, ?, ?, 'pending', ?, NULL)",
        )
        .bind(req.id.to_string())
        .bind(req.user_id.to_string())
        .bind(req.requested_by.to_string())
        .bind(&req.reason)
        .bind(req.requested_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete every per-project access grant held by the user. Returns count.
    pub async fn revoke_project_access_for_user(&self, user_id: &str) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM project_access WHERE grantee_user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Remove the user from every user group. Returns count.
    pub async fn revoke_group_memberships_for_user(&self, user_id: &str) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM user_group_members WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Revoke every active session for the user. Returns count.
    pub async fn revoke_sessions_for_user(&self, user_id: &str) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Remove the user's fixed org role in every organization. Returns count.
    pub async fn revoke_org_memberships_for_user(&self, user_id: &str) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM org_members WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Revoke the user's sharing PKI keys. Returns count (0 or 1).
    pub async fn revoke_sharing_keys_for_user(&self, user_id: &str) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM user_sharing_keys WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Mark an offboarding request as completed.
    pub async fn complete_offboarding(&self, id: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE offboardings SET status = 'completed', completed_at = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Users (member display)
    // ------------------------------------------------------------------

    /// A user's email (used as the member display name; users have no name column).
    pub async fn get_user_email(&self, user_id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }
}
