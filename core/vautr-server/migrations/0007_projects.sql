-- Vautr server schema — Projects, org roles, per-project permissions,
-- basic user groups, and offboarding (mlp-scope.md §2, Wave 0.1).
-- Canonical source: docs/architecture/mlp-scope.md §2, docs/architecture/mlp-wave-plan.md (0.1).
-- Append-only numbered migration: never edit 0001..0006.

-- Organizations: the tenant that scopes shared projects, teams, and roles.
CREATE TABLE IF NOT EXISTS organizations (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

-- Fixed org roles (Owner/Admin/Manager/Member): one row per user per org.
CREATE TABLE IF NOT EXISTS org_members (
    org_id     TEXT NOT NULL,
    user_id    TEXT NOT NULL,
    role       TEXT NOT NULL CHECK (role IN ('Owner', 'Admin', 'Manager', 'Member')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (org_id, user_id),
    FOREIGN KEY (org_id)  REFERENCES organizations(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id)        ON DELETE CASCADE
) STRICT;

-- Projects: the single replacement for Folders/Collections. Personal or shared.
CREATE TABLE IF NOT EXISTS projects (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT,
    kind          TEXT NOT NULL CHECK (kind IN ('personal', 'shared')),
    org_id        TEXT,
    team_id       TEXT,
    owner_user_id TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    FOREIGN KEY (org_id)        REFERENCES organizations(id) ON DELETE CASCADE,
    FOREIGN KEY (owner_user_id) REFERENCES users(id)        ON DELETE CASCADE
) STRICT;

-- Item -> one Project association. item_uuid is the PRIMARY KEY, so an item can
-- belong to at most (and by app convention exactly) one project.
CREATE TABLE IF NOT EXISTS project_items (
    item_uuid  TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    added_by   TEXT NOT NULL,
    added_at   INTEGER NOT NULL,
    FOREIGN KEY (item_uuid)  REFERENCES items(uuid)  ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (added_by)   REFERENCES users(id)    ON DELETE CASCADE
) STRICT;

-- Basic user groups (optional but recommended for v1), org-scoped.
CREATE TABLE IF NOT EXISTS user_groups (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    description TEXT,
    org_id     TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE
) STRICT;

-- Group membership with an in-group role (Member/Admin). Named distinctly from
-- the sharing-PKI `group_members` table created in 0003_sharing.sql.
CREATE TABLE IF NOT EXISTS user_group_members (
    group_id   TEXT NOT NULL,
    user_id    TEXT NOT NULL,
    role       TEXT NOT NULL DEFAULT 'Member' CHECK (role IN ('Member', 'Admin')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (group_id, user_id),
    FOREIGN KEY (group_id) REFERENCES user_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id)  REFERENCES users(id)      ON DELETE CASCADE
) STRICT;

-- Per-project permission grants: a grantee (one user OR one group) gets one
-- permission (CanView/CanEdit/CanManage) on a project.
CREATE TABLE IF NOT EXISTS project_access (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL,
    grantee_user_id  TEXT,
    grantee_group_id TEXT,
    permission       TEXT NOT NULL CHECK (permission IN ('can_view', 'can_edit', 'can_manage')),
    hide_password    INTEGER NOT NULL DEFAULT 0,
    granted_by       TEXT NOT NULL,
    granted_at       INTEGER NOT NULL,
    FOREIGN KEY (project_id)        REFERENCES projects(id)   ON DELETE CASCADE,
    FOREIGN KEY (grantee_user_id)   REFERENCES users(id)      ON DELETE CASCADE,
    FOREIGN KEY (grantee_group_id)  REFERENCES user_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (granted_by)        REFERENCES users(id)      ON DELETE CASCADE,
    CHECK ((grantee_user_id IS NULL) <> (grantee_group_id IS NULL))
) STRICT;

-- Offboarding: a request to revoke all of a user's access.
CREATE TABLE IF NOT EXISTS offboardings (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL,
    requested_by  TEXT NOT NULL,
    reason        TEXT,
    status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed')),
    requested_at  INTEGER NOT NULL,
    completed_at  INTEGER,
    FOREIGN KEY (user_id)      REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_by) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_org_members_user          ON org_members(user_id);
CREATE INDEX IF NOT EXISTS idx_projects_org              ON projects(org_id);
CREATE INDEX IF NOT EXISTS idx_projects_owner            ON projects(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_project_items_project     ON project_items(project_id);
CREATE INDEX IF NOT EXISTS idx_project_access_project    ON project_access(project_id);
CREATE INDEX IF NOT EXISTS idx_project_access_user       ON project_access(grantee_user_id);
CREATE INDEX IF NOT EXISTS idx_project_access_group      ON project_access(grantee_group_id);
CREATE INDEX IF NOT EXISTS idx_user_groups_org           ON user_groups(org_id);
CREATE INDEX IF NOT EXISTS idx_user_group_members_user   ON user_group_members(user_id);
CREATE INDEX IF NOT EXISTS idx_offboardings_user         ON offboardings(user_id);
CREATE INDEX IF NOT EXISTS idx_offboardings_status       ON offboardings(status);
