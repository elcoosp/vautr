//! `machine_accounts` repository domain: non-human identities + access tokens.
//! Docs: `mlp-scope.md` §4, `mlp-wave-plan.md` (A2).
//!
//! Security invariants enforced here / by the handlers:
//! - Only the **SHA-256 hash** of a token secret is persisted (`token_hash`); the
//!   raw secret is returned to the issuer exactly once and never stored.
//! - Scopes are stored as a JSON array of `AccessScope` strings.
//! - Lookup by token hash is what the verify path uses; expiry + revocation checks
//!   live in the handler so a single query drives the whole decision.

use sqlx::FromRow;

use crate::repository::Repository;

/// Row mirror of `machine_accounts` (0008_machine_accounts.sql).
/// `scopes` is a JSON string; parse with the handler's scope helpers.
#[derive(Debug, Clone, FromRow)]
pub struct MachineAccountRow {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_user_id: String,
    pub project_uuid: Option<String>,
    pub status: String,
    pub scopes: String,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

/// Row mirror of `access_tokens` (0008_machine_accounts.sql).
/// Never expose `token_hash` to clients; map to the response struct in the handler.
#[derive(Debug, Clone, FromRow)]
pub struct AccessTokenRow {
    pub uuid: String,
    pub name: String,
    pub owner_user_id: String,
    pub machine_account_uuid: Option<String>,
    pub project_uuid: Option<String>,
    pub scopes: String,
    pub token_hash: String,
    pub prefix: String,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

impl Repository {
    // ------------------------------------------------------------------
    // Machine accounts
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn create_machine_account(
        &self,
        uuid: &str,
        name: &str,
        description: Option<&str>,
        owner_user_id: &str,
        project_uuid: Option<&str>,
        scopes_json: &str,
        expires_at: Option<i64>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO machine_accounts \
               (uuid, name, description, owner_user_id, project_uuid, status, scopes, expires_at, created_at) \
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?)",
        )
        .bind(uuid)
        .bind(name)
        .bind(description)
        .bind(owner_user_id)
        .bind(project_uuid)
        .bind(scopes_json)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List machine accounts owned by `owner_user_id`, newest first.
    pub async fn list_machine_accounts(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<MachineAccountRow>, sqlx::Error> {
        sqlx::query_as::<_, MachineAccountRow>(
            "SELECT * FROM machine_accounts WHERE owner_user_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch one machine account owned by `owner_user_id`.
    pub async fn get_machine_account(
        &self,
        uuid: &str,
        owner_user_id: &str,
    ) -> Result<Option<MachineAccountRow>, sqlx::Error> {
        sqlx::query_as::<_, MachineAccountRow>(
            "SELECT * FROM machine_accounts WHERE uuid = ? AND owner_user_id = ?",
        )
        .bind(uuid)
        .bind(owner_user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Full update (handler merges patch fields first). Returns rows affected.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_machine_account(
        &self,
        uuid: &str,
        owner_user_id: &str,
        name: &str,
        description: Option<&str>,
        status: &str,
        scopes_json: &str,
        expires_at: Option<i64>,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE machine_accounts \
             SET name = ?, description = ?, status = ?, scopes = ?, expires_at = ? \
             WHERE uuid = ? AND owner_user_id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(status)
        .bind(scopes_json)
        .bind(expires_at)
        .bind(uuid)
        .bind(owner_user_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Delete a machine account owned by `owner_user_id`. Returns whether a row was deleted.
    pub async fn delete_machine_account(
        &self,
        uuid: &str,
        owner_user_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query("DELETE FROM machine_accounts WHERE uuid = ? AND owner_user_id = ?")
            .bind(uuid)
            .bind(owner_user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Bump `last_used_at` on a machine account (called on successful token verify).
    pub async fn touch_machine_account(&self, uuid: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE machine_accounts SET last_used_at = ? WHERE uuid = ?")
            .bind(now)
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Access tokens
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn create_access_token(
        &self,
        uuid: &str,
        name: &str,
        owner_user_id: &str,
        machine_account_uuid: Option<&str>,
        project_uuid: Option<&str>,
        scopes_json: &str,
        token_hash: &str,
        prefix: &str,
        expires_at: Option<i64>,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO access_tokens \
               (uuid, name, owner_user_id, machine_account_uuid, project_uuid, scopes, \
                token_hash, prefix, expires_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid)
        .bind(name)
        .bind(owner_user_id)
        .bind(machine_account_uuid)
        .bind(project_uuid)
        .bind(scopes_json)
        .bind(token_hash)
        .bind(prefix)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List access tokens issued by `owner_user_id`, newest first.
    pub async fn list_access_tokens(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<AccessTokenRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessTokenRow>(
            "SELECT * FROM access_tokens WHERE owner_user_id = ? ORDER BY created_at DESC",
        )
        .bind(owner_user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch one access token issued by `owner_user_id`.
    pub async fn get_access_token(
        &self,
        uuid: &str,
        owner_user_id: &str,
    ) -> Result<Option<AccessTokenRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessTokenRow>(
            "SELECT * FROM access_tokens WHERE uuid = ? AND owner_user_id = ?",
        )
        .bind(uuid)
        .bind(owner_user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Look up an access token by its SHA-256 hash. This is the verify path.
    pub async fn get_token_by_hash(&self, token_hash: &str) -> Result<Option<AccessTokenRow>, sqlx::Error> {
        sqlx::query_as::<_, AccessTokenRow>("SELECT * FROM access_tokens WHERE token_hash = ?")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
    }

    /// Revoke an access token (set `revoked_at`). Returns whether a row matched.
    pub async fn revoke_access_token(
        &self,
        uuid: &str,
        owner_user_id: &str,
        now: i64,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE access_tokens SET revoked_at = ? WHERE uuid = ? AND owner_user_id = ?",
        )
        .bind(now)
        .bind(uuid)
        .bind(owner_user_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Bump `last_used_at` on a token (called on successful verify).
    pub async fn touch_access_token(&self, uuid: &str, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE access_tokens SET last_used_at = ? WHERE uuid = ?")
            .bind(now)
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
