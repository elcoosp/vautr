//! WebAuthn (FIDO2) second-factor credential storage (VTR-052).
//!
//! Each row stores a user's registered security key / passkey as a serialized
//! webauthn-rs `SecurityKey` (JSON carrying the COSE public key + monotonic
//! counter) alongside a base64url `cred_id` and a human `label`. All queries are
//! scoped strictly by `user_id`.
//!
//! This module is compiled only when the `webauthn` server feature is enabled
//! (VTR-052 is behind a feature flag, off by default).

use crate::repository::Repository;

/// A stored WebAuthn credential row for a user.
#[derive(Debug, Clone)]
pub struct WebauthnCredentialRow {
    pub id: i64,
    pub user_id: String,
    /// Base64url credential id.
    pub cred_id: String,
    /// Human friendly device label.
    pub label: String,
    /// Serialized webauthn-rs `SecurityKey` (JSON).
    pub serialized: Vec<u8>,
    /// Current signature counter.
    pub counter: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Repository {
    /// Insert a newly registered WebAuthn credential for a user.
    pub async fn add_webauthn_credential(
        &self,
        user_id: &str,
        cred_id: &str,
        label: &str,
        serialized: &[u8],
        counter: i64,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO webauthn_credentials \
             (user_id, cred_id, label, serialized, counter, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(cred_id)
        .bind(label)
        .bind(serialized)
        .bind(counter)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all WebAuthn credentials for a user (credential ids + labels).
    pub async fn list_webauthn_credentials(
        &self,
        user_id: &str,
    ) -> Result<Vec<WebauthnCredentialRow>, sqlx::Error> {
        let rows: Vec<(i64, String, String, Vec<u8>, i64, i64, i64)> = sqlx::query_as(
            "SELECT id, cred_id, label, serialized, counter, created_at, updated_at \
             FROM webauthn_credentials WHERE user_id = ? ORDER BY created_at ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, cred_id, label, serialized, counter, created_at, updated_at)| {
                    WebauthnCredentialRow {
                        id,
                        user_id: user_id.to_string(),
                        cred_id,
                        label,
                        serialized,
                        counter,
                        created_at,
                        updated_at,
                    }
                },
            )
            .collect())
    }

    /// True when the user has registered at least one WebAuthn credential.
    /// Used to decide whether MP unlock must be gated behind a second factor.
    pub async fn webauthn_has_credentials(&self, user_id: &str) -> Result<bool, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM webauthn_credentials WHERE user_id = ?")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0 > 0)
    }

    /// Fetch the stored serialized `SecurityKey` + counter for one credential.
    pub async fn get_webauthn_credential(
        &self,
        user_id: &str,
        cred_id: &str,
    ) -> Result<Option<(Vec<u8>, i64)>, sqlx::Error> {
        let row: Option<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT serialized, counter FROM webauthn_credentials WHERE user_id = ? AND cred_id = ?",
        )
        .bind(user_id)
        .bind(cred_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update a credential's signature counter after a successful assertion.
    pub async fn update_webauthn_counter(
        &self,
        user_id: &str,
        cred_id: &str,
        counter: i64,
        serialized: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE webauthn_credentials SET counter = ?, serialized = ?, updated_at = ? \
             WHERE user_id = ? AND cred_id = ?",
        )
        .bind(counter)
        .bind(serialized)
        .bind(now)
        .bind(user_id)
        .bind(cred_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a registered credential (used to disable the second factor).
    /// Returns true if a row was removed.
    pub async fn remove_webauthn_credential(
        &self,
        user_id: &str,
        cred_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query("DELETE FROM webauthn_credentials WHERE user_id = ? AND cred_id = ?")
            .bind(user_id)
            .bind(cred_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Delete every WebAuthn credential for a user (account deletion / reclaim).
    pub async fn clear_webauthn_credentials(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM webauthn_credentials WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
