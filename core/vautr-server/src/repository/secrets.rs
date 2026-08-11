//! `secrets` repository domain (Wave A3): project-scoped, ciphertext-only
//! secret storage plus project-permission and `secrets:reveal` scope checks.
//!
//! Secrets are scoped to the same Projects as vault items (`mlp-scope.md` §4).
//! The server only ever stores the client-side AEAD ciphertext
//! (`value_ciphertext`), mirroring the existing encrypted-item pattern
//! (`items.payload`). Project access is resolved against the Wave 0.1 schema
//! (`projects`, `project_access`, `user_group_members`); the `secrets:reveal`
//! scope is resolved against `secret_reveal_grants` (0009_secrets.sql).

use sqlx::FromRow;

use crate::repository::Repository;

/// The strongest per-project permission a user holds (inclusive hierarchy:
/// Manage > Edit > View). Mirrors `vautr-domain` `ProjectPermissionSet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectAccessLevel {
    /// Can see + read items/secrets.
    View,
    /// Can create / modify / delete items and secrets.
    Edit,
    /// Can manage who has access to the project.
    Manage,
}

impl ProjectAccessLevel {
    /// Whether this level allows creating/modifying/deleting secrets.
    pub fn can_edit(self) -> bool {
        self >= ProjectAccessLevel::Edit
    }
}

/// Row mirror of `secrets` (0009_secrets.sql).
#[derive(Debug, Clone, FromRow)]
pub struct SecretRow {
    pub uuid: String,
    pub project_id: String,
    pub key: String,
    pub value_ciphertext: Vec<u8>,
    pub version: i64,
    pub created_by: String,
    pub last_accessed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Repository {
    /// Whether a project with the given id exists.
    pub async fn project_exists(&self, project_id: &str) -> Result<bool, sqlx::Error> {
        let id: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?")
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id.is_some())
    }

    /// Resolve the strongest project permission a user holds, considering direct
    /// grants, group membership grants, and project ownership (owner => Manage).
    ///
    /// Returns `None` when the user holds no access at all on the project.
    pub async fn user_project_permission(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<ProjectAccessLevel>, sqlx::Error> {
        // Project owner always has full manage access.
        let owned: Option<String> = sqlx::query_scalar(
            "SELECT id FROM projects WHERE id = ? AND owner_user_id = ?",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        if owned.is_some() {
            return Ok(Some(ProjectAccessLevel::Manage));
        }

        // Gather granted permissions (directly or through group membership),
        // then keep the strongest.
        let perms: Vec<String> = sqlx::query_scalar(
            "SELECT permission FROM project_access \
             WHERE project_id = ? AND (grantee_user_id = ? \
                OR grantee_group_id IN (SELECT group_id FROM user_group_members WHERE user_id = ?))",
        )
        .bind(project_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut best: Option<ProjectAccessLevel> = None;
        for p in perms {
            let lvl = match p.as_str() {
                "can_manage" => ProjectAccessLevel::Manage,
                "can_edit" => ProjectAccessLevel::Edit,
                _ => ProjectAccessLevel::View,
            };
            if best.map_or(true, |b| lvl > b) {
                best = Some(lvl);
            }
        }
        Ok(best)
    }

    /// Create a new secret in a project. `value` is the raw ciphertext bytes.
    pub async fn create_secret(
        &self,
        uuid: &str,
        project_id: &str,
        key: &str,
        value: &[u8],
        created_by: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO secrets \
               (uuid, project_id, key, value_ciphertext, version, created_by, last_accessed_at, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 1, ?, NULL, ?, ?)",
        )
        .bind(uuid)
        .bind(project_id)
        .bind(key)
        .bind(value)
        .bind(created_by)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a secret (metadata + ciphertext) by uuid.
    pub async fn get_secret(&self, uuid: &str) -> Result<Option<SecretRow>, sqlx::Error> {
        sqlx::query_as::<_, SecretRow>("SELECT * FROM secrets WHERE uuid = ?")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await
    }

    /// List all secrets within a project, ordered by key.
    pub async fn list_secrets(&self, project_id: &str) -> Result<Vec<SecretRow>, sqlx::Error> {
        sqlx::query_as::<_, SecretRow>(
            "SELECT * FROM secrets WHERE project_id = ? ORDER BY key ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Update a secret's key/value, incrementing its version.
    pub async fn update_secret(
        &self,
        uuid: &str,
        key: &str,
        value: &[u8],
        now: i64,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE secrets SET key = ?, value_ciphertext = ?, version = version + 1, updated_at = ? \
             WHERE uuid = ?",
        )
        .bind(key)
        .bind(value)
        .bind(now)
        .bind(uuid)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    /// Delete a secret by uuid.
    pub async fn delete_secret(&self, uuid: &str) -> Result<bool, sqlx::Error> {
        let res = sqlx::query("DELETE FROM secrets WHERE uuid = ?")
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() == 1)
    }

    /// Record that a secret's value was read (last-accessed timestamp).
    pub async fn touch_secret(&self, uuid: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE secrets SET last_accessed_at = ? WHERE uuid = ?")
            .bind(now)
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Grant a user the `secrets:reveal` scope on a project (idempotent).
    pub async fn grant_secret_reveal(
        &self,
        project_id: &str,
        user_id: &str,
        granted_by: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO secret_reveal_grants (project_id, user_id, granted_by, created_at) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT (project_id, user_id) DO NOTHING",
        )
        .bind(project_id)
        .bind(user_id)
        .bind(granted_by)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Whether a user may reveal a project's secret VALUES: true for the project
    /// owner, or when the user holds a `secrets:reveal` grant on the project.
    pub async fn can_reveal(&self, project_id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
        let owned: Option<String> = sqlx::query_scalar(
            "SELECT id FROM projects WHERE id = ? AND owner_user_id = ?",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        if owned.is_some() {
            return Ok(true);
        }
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM secret_reveal_grants \
             WHERE user_id = ? AND project_id = ?",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        let path =
            std::env::temp_dir().join(format!("vautr_secrets_repo_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    async fn seed(repo: &Repository) -> (String, String) {
        let now = 1_700_000_000_000i64;
        repo.create_user(
            "u1",
            "owner@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();
        repo.create_user(
            "u2",
            "member@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, kind, org_id, team_id, owner_user_id, created_at, updated_at) \
             VALUES ('p1', 'Secrets', 'personal', NULL, NULL, 'u1', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        ("u1".into(), "p1".into())
    }

    #[tokio::test]
    async fn owner_gets_manage_project_permission() {
        let repo = test_repo().await;
        let (u1, p1) = seed(&repo).await;
        let lvl = repo.user_project_permission(&p1, &u1).await.unwrap();
        assert_eq!(lvl, Some(ProjectAccessLevel::Manage));
    }

    #[tokio::test]
    async fn group_grant_gives_permission_and_reveal_is_gated() {
        let repo = test_repo().await;
        let (_, p1) = seed(&repo).await;
        let now = 1_700_000_000_000i64;

        // u2 has no access yet.
        assert_eq!(
            repo.user_project_permission(&p1, "u2").await.unwrap(),
            None
        );
        assert!(!repo.can_reveal(&p1, "u2").await.unwrap());

        // Grant u2 CanEdit directly.
        sqlx::query(
            "INSERT INTO project_access (id, project_id, grantee_user_id, grantee_group_id, permission, hide_password, granted_by, granted_at) \
             VALUES ('pa1', 'p1', 'u2', NULL, 'can_edit', 0, 'u1', ?)",
        )
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        assert_eq!(
            repo.user_project_permission(&p1, "u2").await.unwrap(),
            Some(ProjectAccessLevel::Edit)
        );

        // CanEdit but no reveal scope -> still cannot reveal.
        assert!(!repo.can_reveal(&p1, "u2").await.unwrap());

        // Grant reveal scope.
        repo.grant_secret_reveal(&p1, "u2", "u1", now)
            .await
            .unwrap();
        assert!(repo.can_reveal(&p1, "u2").await.unwrap());
    }

    #[tokio::test]
    async fn secret_crud_roundtrip() {
        let repo = test_repo().await;
        let (_, p1) = seed(&repo).await;
        let now = 1_700_000_000_000i64;

        repo.create_secret("s1", &p1, "db_pass", b"ciphertext-v1", "u1", now)
            .await
            .unwrap();

        let row = repo.get_secret("s1").await.unwrap().unwrap();
        assert_eq!(row.key, "db_pass");
        assert_eq!(row.value_ciphertext, b"ciphertext-v1");
        assert_eq!(row.version, 1);

        // Update increments version.
        assert!(repo
            .update_secret("s1", "db_pass", b"ciphertext-v2", now + 1)
            .await
            .unwrap());
        let row = repo.get_secret("s1").await.unwrap().unwrap();
        assert_eq!(row.value_ciphertext, b"ciphertext-v2");
        assert_eq!(row.version, 2);

        // List within project.
        let list = repo.list_secrets(&p1).await.unwrap();
        assert_eq!(list.len(), 1);

        // Delete.
        assert!(repo.delete_secret("s1").await.unwrap());
        assert!(repo.get_secret("s1").await.unwrap().is_none());
    }
}
