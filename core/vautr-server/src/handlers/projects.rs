//! Projects HTTP handlers (mlp-wave-plan.md §3 A1, mlp-scope.md §2).
//!
//! Implements the full Projects surface against the Wave 0.1 domain model:
//!
//! * Projects CRUD — create (personal/shared), list, get, patch, delete.
//! * Per-project membership — add/update/remove members with a per-project
//!   permission (`can_view`/`can_edit`/`can_manage`) + `hide_password`, honoring
//!   the fixed org roles (Owner > Admin > Manager > Member) with rank
//!   enforcement from `vautr-domain::roles::OrgRole`.
//! * Per-project user groups — create/update/delete, add/remove group members.
//! * Offboarding — `POST /offboard` revokes all of a user's access
//!   (REVOKE_ALL).
//!
//! Wire-format note: the OpenAPI contract uses lowercase enum values
//! (`owner`, `can_view`, …) which differ from `vautr-domain`'s default
//! (PascalCase) serde form. Domain types are used internally for rank/permission
//! logic; this module owns the wire <-> domain mapping so the two never clash.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, AppState, Bearer, auth_user, now_ms};
use crate::repository::projects::{AccessRow, ProjectRow, UserGroupRow};
use vautr_domain::{
    GroupMemberRole, OffboardingRequest, OrgRole, Project, ProjectKind, ProjectPermission,
};

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects).post(create_project))
        .route(
            "/projects/{uuid}",
            get(get_project).patch(update_project).delete(delete_project),
        )
        .route(
            "/projects/{uuid}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/projects/{uuid}/members/{user_uuid}",
            patch(update_member).delete(remove_member),
        )
        .route(
            "/projects/{uuid}/groups",
            get(list_groups).post(create_group),
        )
        .route(
            "/projects/{uuid}/groups/{group_id}",
            patch(update_group).delete(delete_group),
        )
        .route(
            "/projects/{uuid}/groups/{group_id}/members",
            post(add_group_member),
        )
        .route(
            "/projects/{uuid}/groups/{group_id}/members/{user_uuid}",
            delete(remove_group_member),
        )
        .route("/offboard", post(offboard))
}

// ---------------------------------------------------------------------------
// Wire <-> domain enum mapping (OpenAPI uses lowercase; domain uses PascalCase)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RoleWire {
    Owner,
    Admin,
    Manager,
    Member,
}

impl RoleWire {
    fn to_domain(self) -> OrgRole {
        match self {
            RoleWire::Owner => OrgRole::Owner,
            RoleWire::Admin => OrgRole::Admin,
            RoleWire::Manager => OrgRole::Manager,
            RoleWire::Member => OrgRole::Member,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PermWire {
    CanView,
    CanEdit,
    CanManage,
}

impl PermWire {
    fn to_domain(self) -> ProjectPermission {
        match self {
            PermWire::CanView => ProjectPermission::CanView,
            PermWire::CanEdit => ProjectPermission::CanEdit,
            PermWire::CanManage => ProjectPermission::CanManage,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProjectTypeWire {
    Personal,
    Shared,
}

impl ProjectTypeWire {
    fn to_kind(self) -> ProjectKind {
        match self {
            ProjectTypeWire::Personal => ProjectKind::Personal,
            ProjectTypeWire::Shared => ProjectKind::Shared,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GroupRoleWire {
    Admin,
    Member,
}

impl GroupRoleWire {
    fn to_domain(self) -> GroupMemberRole {
        match self {
            GroupRoleWire::Admin => GroupMemberRole::Admin,
            GroupRoleWire::Member => GroupMemberRole::Member,
        }
    }
}

fn role_wire(r: OrgRole) -> &'static str {
    match r {
        OrgRole::Owner => "owner",
        OrgRole::Admin => "admin",
        OrgRole::Manager => "manager",
        OrgRole::Member => "member",
    }
}

fn perm_wire(p: ProjectPermission) -> &'static str {
    match p {
        ProjectPermission::CanView => "can_view",
        ProjectPermission::CanEdit => "can_edit",
        ProjectPermission::CanManage => "can_manage",
    }
}

fn group_role_wire(r: GroupMemberRole) -> &'static str {
    match r {
        GroupMemberRole::Admin => "admin",
        GroupMemberRole::Member => "member",
    }
}

fn perm_from_db(s: &str) -> ProjectPermission {
    match s {
        "can_edit" => ProjectPermission::CanEdit,
        "can_manage" => ProjectPermission::CanManage,
        _ => ProjectPermission::CanView,
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct CreateProjectReq {
    name: String,
    description: Option<String>,
    #[serde(rename = "type")]
    proj_type: Option<ProjectTypeWire>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateProjectReq {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AddMemberReq {
    user_uuid: String,
    role: Option<RoleWire>,
    permission: Option<PermWire>,
    #[serde(default)]
    hide_password: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateMemberReq {
    role: Option<RoleWire>,
    permission: Option<PermWire>,
    #[serde(default)]
    hide_password: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct CreateGroupReq {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateGroupReq {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AddGroupMemberReq {
    user_uuid: String,
    role: Option<GroupRoleWire>,
}

#[derive(Deserialize)]
pub(crate) struct OffboardReq {
    user_uuid: String,
    reason: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ProjectListResp {
    projects: Vec<ProjectResp>,
}

#[derive(Serialize)]
pub(crate) struct ProjectResp {
    uuid: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "type")]
    proj_type: &'static str,
    role: &'static str,
    permission: Option<&'static str>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
pub(crate) struct ProjectMemberListResp {
    members: Vec<ProjectMemberResp>,
}

#[derive(Serialize)]
pub(crate) struct ProjectMemberResp {
    user_uuid: String,
    display_name: Option<String>,
    role: &'static str,
    permission: &'static str,
    added_at: i64,
    hide_password: bool,
}

#[derive(Serialize)]
pub(crate) struct UserGroupListResp {
    groups: Vec<UserGroupResp>,
}

#[derive(Serialize)]
pub(crate) struct UserGroupResp {
    id: String,
    name: String,
    description: Option<String>,
    members: Vec<GroupMemberResp>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
pub(crate) struct GroupMemberResp {
    user_uuid: String,
    role: &'static str,
}

#[derive(Serialize)]
pub(crate) struct StatusResp {
    status: &'static str,
}

#[derive(Serialize)]
pub(crate) struct OffboardResp {
    status: &'static str,
    user_uuid: String,
    revoked_projects: u64,
    revoked_memberships: u64,
    revoked_tokens: u64,
    revoked_at: i64,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn internal_err(e: sqlx::Error) -> ApiError {
    ApiError::internal(&e.to_string())
}

fn forbidden(msg: &str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, "forbidden", msg)
}

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "project not found")
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.trim().len() > 128 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "precondition_failed",
            "name must be 1-128 characters",
        ));
    }
    Ok(())
}

/// The caller's fixed org role within the project's organization
/// (defaults to Member when the project has no org or the caller is not a member).
async fn caller_role_in_project(
    st: &AppState,
    p: &ProjectRow,
    caller: &str,
) -> Result<OrgRole, ApiError> {
    if let Some(org) = &p.org_id {
        if let Some(r) = st.repo.get_org_role(org, caller).await.map_err(internal_err)? {
            return Ok(r);
        }
    }
    Ok(OrgRole::Member)
}

async fn effective_perm(
    st: &AppState,
    p: &ProjectRow,
    caller: &str,
) -> Result<Option<ProjectPermission>, ApiError> {
    st.repo
        .effective_permission(&p.id, caller)
        .await
        .map_err(internal_err)
}

/// A project is visible to the caller if they own it, hold any per-project
/// permission (directly or via a group), or are an org Owner/Admin.
async fn ensure_visible(st: &AppState, p: &ProjectRow, caller: &str) -> Result<(), ApiError> {
    if p.owner_user_id == caller {
        return Ok(());
    }
    let role = caller_role_in_project(st, p, caller).await?;
    if role.can_manage_org() {
        return Ok(());
    }
    if effective_perm(st, p, caller).await?.is_some() {
        return Ok(());
    }
    Err(not_found())
}

/// The caller may manage (mutate membership / project meta / offboard-scope
/// ops) if they own the project, hold CanManage, or are an org Owner/Admin.
async fn require_can_manage(st: &AppState, p: &ProjectRow, caller: &str) -> Result<(), ApiError> {
    if p.owner_user_id == caller {
        return Ok(());
    }
    if let Some(perm) = effective_perm(st, p, caller).await? {
        if perm == ProjectPermission::CanManage {
            return Ok(());
        }
    }
    let role = caller_role_in_project(st, p, caller).await?;
    if role.can_manage_org() {
        return Ok(());
    }
    Err(forbidden("you do not have manage permission on this project"))
}

/// Rank enforcement (vautr-domain `OrgRole::rank`): a caller may only assign an
/// org role that is strictly below their own rank (no privilege escalation).
fn enforce_role_rank(caller_role: OrgRole, target_role: OrgRole) -> Result<(), ApiError> {
    if caller_role.rank() <= target_role.rank() {
        return Err(forbidden(
            "cannot assign an org role at or above your own rank",
        ));
    }
    Ok(())
}

fn project_resp(p: &ProjectRow, role: OrgRole, perm: Option<ProjectPermission>) -> ProjectResp {
    ProjectResp {
        uuid: p.id.clone(),
        name: p.name.clone(),
        description: p.description.clone(),
        proj_type: match p.kind() {
            ProjectKind::Personal => "personal",
            ProjectKind::Shared => "shared",
        },
        role: role_wire(role),
        permission: perm.map(perm_wire),
        created_at: p.created_at,
        updated_at: p.updated_at,
    }
}

async fn member_resp(st: &AppState, p: &ProjectRow, grant: &AccessRow) -> Result<ProjectMemberResp, ApiError> {
    let uid = grant.grantee_user_id.clone().unwrap_or_default();
    let display_name = st.repo.get_user_email(&uid).await.map_err(internal_err)?;
    let role = match &p.org_id {
        Some(org) => st
            .repo
            .get_org_role(org, &uid)
            .await
            .map_err(internal_err)?
            .unwrap_or(OrgRole::Member),
        None => OrgRole::Member,
    };
    Ok(ProjectMemberResp {
        user_uuid: uid,
        display_name,
        role: role_wire(role),
        permission: perm_wire(perm_from_db(&grant.permission)),
        added_at: grant.granted_at,
        hide_password: grant.hide_password != 0,
    })
}

async fn group_resp(st: &AppState, g: &UserGroupRow) -> Result<UserGroupResp, ApiError> {
    let rows = st.repo.list_user_group_members(&g.id).await.map_err(internal_err)?;
    let members = rows
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "Admin" => GroupMemberRole::Admin,
                _ => GroupMemberRole::Member,
            };
            GroupMemberResp {
                user_uuid: m.user_id,
                role: group_role_wire(role),
            }
        })
        .collect();
    Ok(UserGroupResp {
        id: g.id.clone(),
        name: g.name.clone(),
        description: g.description.clone(),
        members,
        created_at: g.created_at,
        updated_at: g.updated_at,
    })
}

// ---------------------------------------------------------------------------
// Projects CRUD
// ---------------------------------------------------------------------------

/// GET /projects — list projects visible to the caller.
async fn list_projects(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<ProjectListResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let rows = st
        .repo
        .list_projects_for_user(&caller)
        .await
        .map_err(internal_err)?;
    let mut projects = Vec::with_capacity(rows.len());
    for p in rows {
        let role = caller_role_in_project(&st, &p, &caller).await?;
        let perm = effective_perm(&st, &p, &caller).await?;
        projects.push(project_resp(&p, role, perm));
    }
    Ok(Json(ProjectListResp { projects }))
}

/// POST /projects — create a personal or shared project.
async fn create_project(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<CreateProjectReq>,
) -> Result<(StatusCode, Json<ProjectResp>), ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    validate_name(&req.name)?;
    let kind = req.proj_type.map(|t| t.to_kind()).unwrap_or(ProjectKind::Personal);

    let now = now_ms();
    let id = Uuid::new_v4();
    let caller_uuid = Uuid::parse_str(&caller).unwrap_or(Uuid::nil());

    let (project, _org) = match kind {
        ProjectKind::Personal => {
            let p = Project::personal(id, req.name.trim(), caller_uuid, now);
            (p, None)
        }
        ProjectKind::Shared => {
            // Bootstrap/default org so a self-hosted owner can create shared
            // projects; role enforcement still applies to existing orgs.
            let org = st
                .repo
                .ensure_org_for_user(&caller, now)
                .await
                .map_err(internal_err)?;
            let role = st
                .repo
                .get_org_role(&org, &caller)
                .await
                .map_err(internal_err)?
                .unwrap_or(OrgRole::Member);
            if !role.can_create_projects() {
                return Err(forbidden(
                    "only Owner/Admin/Manager may create shared projects",
                ));
            }
            let org_uuid = Uuid::parse_str(&org).unwrap_or(Uuid::nil());
            (Project::shared(id, req.name.trim(), org_uuid, None, caller_uuid, now), Some(org))
        }
    };

    st.repo.create_project(&project).await.map_err(internal_err)?;
    if let Some(desc) = req.description.as_deref() {
        st.repo
            .update_project_meta(&id.to_string(), project.name.trim(), Some(desc), now)
            .await
            .map_err(internal_err)?;
    }

    let row = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    let role = caller_role_in_project(&st, &row, &caller).await?;
    Ok((StatusCode::CREATED, Json(project_resp(&row, role, Some(ProjectPermission::CanManage)))))
}

/// GET /projects/{uuid} — project detail (only if visible).
async fn get_project(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<ProjectResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    ensure_visible(&st, &p, &caller).await?;
    let role = caller_role_in_project(&st, &p, &caller).await?;
    let perm = effective_perm(&st, &p, &caller).await?;
    Ok(Json(project_resp(&p, role, perm)))
}

/// PATCH /projects/{uuid} — rename / re-describe (manage permission required).
async fn update_project(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<UpdateProjectReq>,
) -> Result<Json<ProjectResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;

    let name = req.name.clone().unwrap_or_else(|| p.name.clone());
    validate_name(&name)?;
    let description = req.description.clone().or_else(|| p.description.clone());

    st.repo
        .update_project_meta(&p.id, name.trim(), description.as_deref(), now_ms())
        .await
        .map_err(internal_err)?;
    let updated = st
        .repo
        .get_project(&p.id)
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    let role = caller_role_in_project(&st, &updated, &caller).await?;
    let perm = effective_perm(&st, &updated, &caller).await?;
    Ok(Json(project_resp(&updated, role, perm)))
}

/// DELETE /projects/{uuid} — delete the project (manage permission required).
async fn delete_project(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    st.repo.delete_project(&p.id).await.map_err(internal_err)?;
    Ok(Json(StatusResp { status: "success" }))
}

// ---------------------------------------------------------------------------
// Per-project membership
// ---------------------------------------------------------------------------

/// GET /projects/{uuid}/members — list per-user access grants (manage required).
async fn list_members(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<ProjectMemberListResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    let grants = st
        .repo
        .list_project_members(&p.id)
        .await
        .map_err(internal_err)?;
    let mut members = Vec::with_capacity(grants.len());
    for g in grants {
        members.push(member_resp(&st, &p, &g).await?);
    }
    Ok(Json(ProjectMemberListResp { members }))
}

/// POST /projects/{uuid}/members — add a member with a per-project permission
/// and (optionally) assign an org role under rank enforcement.
async fn add_member(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<AddMemberReq>,
) -> Result<(StatusCode, Json<ProjectMemberResp>), ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;

    // Target user must exist.
    st.repo
        .get_user_by_id(&req.user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, "not_found", "user not found")
        })?;

    let permission = req.permission.map(|p| p.to_domain()).unwrap_or(ProjectPermission::CanView);
    let hide_password = req.hide_password.unwrap_or(false);
    let now = now_ms();

    // Optional org-role assignment with rank enforcement.
    if let Some(role_w) = req.role {
        let target_role = role_w.to_domain();
        let org = match &p.org_id {
            Some(o) => o.clone(),
            None => st.repo.ensure_org_for_user(&caller, now).await.map_err(internal_err)?,
        };
        let caller_role = st
            .repo
            .get_org_role(&org, &caller)
            .await
            .map_err(internal_err)?
            .unwrap_or(OrgRole::Member);
        enforce_role_rank(caller_role, target_role)?;
        st.repo
            .set_org_member(&org, &req.user_uuid, target_role, now)
            .await
            .map_err(internal_err)?;
    }

    st.repo
        .grant_user_project_access(&p.id, &req.user_uuid, permission, hide_password, &caller, now)
        .await
        .map_err(internal_err)?;

    let grant = st
        .repo
        .get_user_grant(&p.id, &req.user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_server_error", "grant missing"))?;
    Ok((StatusCode::CREATED, Json(member_resp(&st, &p, &grant).await?)))
}

/// PATCH /projects/{uuid}/members/{user_uuid} — update permission / role.
async fn update_member(
    State(st): State<AppState>,
    Path((id, user_uuid)): Path<(Uuid, String)>,
    auth: Bearer,
    Json(req): Json<UpdateMemberReq>,
) -> Result<Json<ProjectMemberResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;

    let existing = st
        .repo
        .get_user_grant(&p.id, &user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "member not found"))?;

    let permission = req
        .permission
        .map(|p| p.to_domain())
        .unwrap_or(perm_from_db(&existing.permission));
    let hide_password = req.hide_password.unwrap_or(existing.hide_password != 0);
    let now = now_ms();

    if let Some(role_w) = req.role {
        let target_role = role_w.to_domain();
        let org = match &p.org_id {
            Some(o) => o.clone(),
            None => st.repo.ensure_org_for_user(&caller, now).await.map_err(internal_err)?,
        };
        let caller_role = st
            .repo
            .get_org_role(&org, &caller)
            .await
            .map_err(internal_err)?
            .unwrap_or(OrgRole::Member);
        enforce_role_rank(caller_role, target_role)?;
        st.repo
            .set_org_member(&org, &user_uuid, target_role, now)
            .await
            .map_err(internal_err)?;
    }

    st.repo
        .grant_user_project_access(&p.id, &user_uuid, permission, hide_password, &caller, now)
        .await
        .map_err(internal_err)?;

    let grant = st
        .repo
        .get_user_grant(&p.id, &user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "member not found"))?;
    Ok(Json(member_resp(&st, &p, &grant).await?))
}

/// DELETE /projects/{uuid}/members/{user_uuid} — revoke a member's access.
async fn remove_member(
    State(st): State<AppState>,
    Path((id, user_uuid)): Path<(Uuid, String)>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    let removed = st
        .repo
        .remove_user_grant(&p.id, &user_uuid)
        .await
        .map_err(internal_err)?;
    if removed == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "member not found"));
    }
    Ok(Json(StatusResp { status: "success" }))
}

// ---------------------------------------------------------------------------
// Per-project user groups
// ---------------------------------------------------------------------------

/// GET /projects/{uuid}/groups — user groups that have access to the project.
async fn list_groups(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<UserGroupListResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    ensure_visible(&st, &p, &caller).await?;
    let rows = st
        .repo
        .list_groups_for_project(&p.id)
        .await
        .map_err(internal_err)?;
    let mut groups = Vec::with_capacity(rows.len());
    for g in rows {
        groups.push(group_resp(&st, &g).await?);
    }
    Ok(Json(UserGroupListResp { groups }))
}

/// POST /projects/{uuid}/groups — create a group and grant it access (manage).
async fn create_group(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<CreateGroupReq>,
) -> Result<(StatusCode, Json<UserGroupResp>), ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    validate_name(&req.name)?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;

    let now = now_ms();
    let org = match &p.org_id {
        Some(o) => Some(o.clone()),
        None => Some(st.repo.ensure_org_for_user(&caller, now).await.map_err(internal_err)?),
    };
    let gid = Uuid::new_v4().to_string();
    st.repo
        .create_user_group(&gid, req.name.trim(), req.description.as_deref(), org.as_deref(), now)
        .await
        .map_err(internal_err)?;
    // A freshly-created project group starts with view access so its members
    // immediately inherit access (no separate grant-group endpoint exists).
    st.repo
        .grant_group_project_access(&p.id, &gid, ProjectPermission::CanView, false, &caller, now)
        .await
        .map_err(internal_err)?;

    let g = st
        .repo
        .get_user_group(&gid)
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    Ok((StatusCode::CREATED, Json(group_resp(&st, &g).await?)))
}

/// PATCH /projects/{uuid}/groups/{group_id} — rename / re-describe a group.
async fn update_group(
    State(st): State<AppState>,
    Path((id, group_id)): Path<(Uuid, Uuid)>,
    auth: Bearer,
    Json(req): Json<UpdateGroupReq>,
) -> Result<Json<UserGroupResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    let g = st
        .repo
        .get_user_group(&group_id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "group not found"))?;

    let name = req.name.clone().unwrap_or_else(|| g.name.clone());
    validate_name(&name)?;
    let description = req.description.clone().or_else(|| g.description.clone());
    st.repo
        .update_user_group(&g.id, name.trim(), description.as_deref(), now_ms())
        .await
        .map_err(internal_err)?;
    let updated = st
        .repo
        .get_user_group(&g.id)
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    Ok(Json(group_resp(&st, &updated).await?))
}

/// DELETE /projects/{uuid}/groups/{group_id} — delete a group (manage).
async fn delete_group(
    State(st): State<AppState>,
    Path((id, group_id)): Path<(Uuid, Uuid)>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    st.repo
        .delete_user_group(&group_id.to_string())
        .await
        .map_err(internal_err)?;
    Ok(Json(StatusResp { status: "success" }))
}

/// POST /projects/{uuid}/groups/{group_id}/members — add a member to a group.
async fn add_group_member(
    State(st): State<AppState>,
    Path((id, group_id)): Path<(Uuid, Uuid)>,
    auth: Bearer,
    Json(req): Json<AddGroupMemberReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    st.repo
        .get_user_group(&group_id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "group not found"))?;
    st.repo
        .get_user_by_id(&req.user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "user not found"))?;
    let role = req.role.map(|r| r.to_domain()).unwrap_or(GroupMemberRole::Member);
    st.repo
        .add_user_group_member(&group_id.to_string(), &req.user_uuid, role, now_ms())
        .await
        .map_err(internal_err)?;
    Ok(Json(StatusResp { status: "success" }))
}

/// DELETE /projects/{uuid}/groups/{group_id}/members/{user_uuid} — remove a group member.
async fn remove_group_member(
    State(st): State<AppState>,
    Path((id, group_id, user_uuid)): Path<(Uuid, Uuid, String)>,
    auth: Bearer,
) -> Result<Json<StatusResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let p = st
        .repo
        .get_project(&id.to_string())
        .await
        .map_err(internal_err)?
        .ok_or_else(not_found)?;
    require_can_manage(&st, &p, &caller).await?;
    let removed = st
        .repo
        .remove_user_group_member(&group_id.to_string(), &user_uuid)
        .await
        .map_err(internal_err)?;
    if removed == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "group member not found"));
    }
    Ok(Json(StatusResp { status: "success" }))
}

// ---------------------------------------------------------------------------
// Offboarding: revoke all of a user's access
// ---------------------------------------------------------------------------

/// POST /offboard — revoke all of a user's access (REVOKE_ALL).
async fn offboard(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<OffboardReq>,
) -> Result<Json<OffboardResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    st.repo
        .get_user_by_id(&req.user_uuid)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "unknown user"))?;

    // Authorization + rank: the caller must outrank the target in every org the
    // target belongs to (Owner/Admin may offboard; cannot offboard an equal/higher role).
    let caller_roles = st.repo.get_user_org_roles(&caller).await.map_err(internal_err)?;
    let target_roles = st.repo.get_user_org_roles(&req.user_uuid).await.map_err(internal_err)?;
    if target_roles.is_empty() {
        if !caller_roles.iter().any(|(_, r)| r.can_offboard()) {
            return Err(forbidden("only Owner/Admin may offboard users"));
        }
    } else {
        for (org, trole) in &target_roles {
            match caller_roles.iter().find(|(o, _)| o == org) {
                Some((_, c)) if c.can_offboard() && c.rank() > trole.rank() => {}
                _ => {
                    return Err(forbidden(
                        "not authorized to offboard this user (role rank too high or out of your org)",
                    ))
                }
            }
        }
    }

    let now = now_ms();
    let request = OffboardingRequest::revoke_all(
        Uuid::new_v4(),
        Uuid::parse_str(&req.user_uuid).unwrap_or(Uuid::nil()),
        Uuid::parse_str(&caller).unwrap_or(Uuid::nil()),
        req.reason.clone(),
        now,
    );
    st.repo.create_offboarding(&request).await.map_err(internal_err)?;

    // Apply REVOKE_ALL: project grants, group memberships, sessions, org roles,
    // and sharing PKI keys.
    let revoked_projects = st.repo.revoke_project_access_for_user(&req.user_uuid).await.map_err(internal_err)?;
    let revoked_memberships = st.repo.revoke_group_memberships_for_user(&req.user_uuid).await.map_err(internal_err)?;
    let revoked_tokens = st.repo.revoke_sessions_for_user(&req.user_uuid).await.map_err(internal_err)?;
    st.repo.revoke_org_memberships_for_user(&req.user_uuid).await.map_err(internal_err)?;
    st.repo.revoke_sharing_keys_for_user(&req.user_uuid).await.map_err(internal_err)?;
    st.repo.complete_offboarding(&request.id.to_string(), now).await.map_err(internal_err)?;

    Ok(Json(OffboardResp {
        status: "success",
        user_uuid: req.user_uuid,
        revoked_projects,
        revoked_memberships,
        revoked_tokens,
        revoked_at: now,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Repository;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tower::util::ServiceExt;

    /// Build an in-memory app with three users (u1/u2/u3 + tok1/2/3) and sessions.
    async fn test_state() -> AppState {
        let pool = crate::db::connect("sqlite::memory:").await.expect("connect+migrate");
        let repo = Arc::new(Repository::new(pool));
        let now = now_ms();
        for (id, email, tok) in [
            ("11111111-1111-4111-8111-111111111111", "a@example.com", "tok1"),
            ("22222222-2222-4222-8222-222222222222", "b@example.com", "tok2"),
            ("33333333-3333-4333-8333-333333333333", "c@example.com", "tok3"),
        ] {
            repo.create_user(
                id,
                email,
                &[0u8; 32],
                &[1u8; 16],
                &[2u8; 48],
                &[3u8; 48],
                now,
            )
            .await
            .expect("create user");
            sqlx::query(
                "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(tok)
            .bind(id)
            .bind(now + 60_000)
            .bind(now)
            .execute(repo.pool())
            .await
            .expect("store session");
        }
        AppState::new(repo)
    }

    async fn call(
        router: &Router<()>,
        method: &str,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        let req = match body {
            Some(b) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = router.clone().oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1_048_576).await.unwrap_or_default();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, json)
    }

    /// End-to-end unit test: create project -> add member (CanView) -> offboard
    /// member -> assert access revoked. Mirrors the live-server E2E.
    #[tokio::test]
    async fn member_offboarding_revokes_access() {
        let state = test_state().await;
        let router = super::routes().with_state(state.clone());

        // u1 creates a shared project -> u1 becomes Owner of a default org.
        let (status, json) = call(
            &router,
            "POST",
            "/projects",
            "tok1",
            Some(json!({ "name": "Shared Vault", "type": "shared" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "create project: {json}");
        let project_id = json["uuid"].as_str().unwrap().to_string();
        assert_eq!(json["type"], "shared");
        assert_eq!(json["role"], "owner");
        assert_eq!(json["permission"], "can_manage");

        // u1 adds u2 as a member with CanView.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/members"),
            "tok1",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222", "permission": "can_view" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "add member: {json}");
        assert_eq!(json["user_uuid"], "22222222-2222-4222-8222-222222222222");
        assert_eq!(json["permission"], "can_view");

        // u2 now sees the project in their list.
        let (status, json) = call(&router, "GET", "/projects", "tok2", None).await;
        assert_eq!(status, StatusCode::OK);
        let projects = json["projects"].as_array().unwrap();
        assert_eq!(projects.len(), 1, "u2 sees the project: {json}");

        // A non-admin (u3, no access) cannot offboard.
        let (status, _) = call(
            &router,
            "POST",
            "/offboard",
            "tok3",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "u3 must not offboard");

        // u1 (Owner) offboards u2.
        let (status, json) = call(
            &router,
            "POST",
            "/offboard",
            "tok1",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222", "reason": "left the team" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "offboard: {json}");
        assert_eq!(json["status"], "success");
        assert_eq!(json["revoked_projects"], 1);

        // u2's session was revoked -> their token is now invalid.
        let (status, _) = call(&router, "GET", "/projects", "tok2", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "u2 session revoked");

        // u2's per-project grant is gone (verified from the member list).
        let (status, json) = call(
            &router,
            "GET",
            &format!("/projects/{project_id}/members"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["members"].as_array().unwrap().len(), 0, "no members left: {json}");
    }

    #[tokio::test]
    async fn role_rank_enforced_on_member_role_assignment() {
        let state = test_state().await;
        let router = super::routes().with_state(state.clone());

        // u1 creates a shared project (becomes Owner of a default org).
        let (_, json) = call(
            &router,
            "POST",
            "/projects",
            "tok1",
            Some(json!({ "name": "Rank", "type": "shared" })),
        )
        .await;
        let project_id = json["uuid"].as_str().unwrap().to_string();

        // Owner (rank 3) can assign an Admin role (rank 2).
        let (status, _) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/members"),
            "tok1",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222", "permission": "can_edit", "role": "admin" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Owner cannot assign themselves a role equal to their own rank.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/members"),
            "tok1",
            Some(json!({ "user_uuid": "33333333-3333-4333-8333-333333333333", "role": "owner" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "rank escalation blocked: {json}");
    }

    #[tokio::test]
    async fn groups_crud_and_group_access_inheritance() {
        let state = test_state().await;
        let router = super::routes().with_state(state.clone());

        let (_, json) = call(
            &router,
            "POST",
            "/projects",
            "tok1",
            Some(json!({ "name": "Team", "type": "shared" })),
        )
        .await;
        let project_id = json["uuid"].as_str().unwrap().to_string();

        // u1 creates a group under the project.
        let (status, json) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/groups"),
            "tok1",
            Some(json!({ "name": "Engineering" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let group_id = json["id"].as_str().unwrap().to_string();

        // u1 adds u2 to the group.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/groups/{group_id}/members"),
            "tok1",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // u2 inherits view access via the group -> sees the project.
        let (_, json) = call(&router, "GET", "/projects", "tok2", None).await;
        assert_eq!(json["projects"].as_array().unwrap().len(), 1);

        // u3 (not in group) does not see the project.
        let (_, json) = call(&router, "GET", "/projects", "tok3", None).await;
        assert_eq!(json["projects"].as_array().unwrap().len(), 0);

        // Delete the group; u2 loses inherited access.
        let (status, _) = call(
            &router,
            "DELETE",
            &format!("/projects/{project_id}/groups/{group_id}"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, json) = call(&router, "GET", "/projects", "tok2", None).await;
        assert_eq!(json["projects"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn member_requires_can_manage() {
        let state = test_state().await;
        let router = super::routes().with_state(state.clone());

        let (_, json) = call(
            &router,
            "POST",
            "/projects",
            "tok1",
            Some(json!({ "name": "Priv", "type": "shared" })),
        )
        .await;
        let project_id = json["uuid"].as_str().unwrap().to_string();

        // u1 adds u2 with can_view.
        call(
            &router,
            "POST",
            &format!("/projects/{project_id}/members"),
            "tok1",
            Some(json!({ "user_uuid": "22222222-2222-4222-8222-222222222222", "permission": "can_view" })),
        )
        .await;

        // u2 (only can_view) cannot add a third member.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/projects/{project_id}/members"),
            "tok2",
            Some(json!({ "user_uuid": "33333333-3333-4333-8333-333333333333", "permission": "can_view" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "can_view member cannot manage");
    }
}
