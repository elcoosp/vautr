//! `mfa` repository domain: TOTP enrollment + active secrets, one-time recovery
//! codes, and the organization MFA / master-password policy (Wave A4).
//!
//! All queries are scoped strictly by `user_id`. Schema: `migrations/0010_mfa.sql`.

use serde::{Deserialize, Serialize};

use crate::repository::Repository;

/// Master-password strength policy. Advisory server-side (the server never sees
/// the master password under OPAQUE); stored + served to clients and validated
/// here so the PUT endpoint rejects nonsensical values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterPasswordPolicy {
    pub min_length: u32,
    pub require_upper: bool,
    pub require_lower: bool,
    pub require_digit: bool,
    pub require_special: bool,
    pub min_entropy_bits: u32,
}

impl Default for MasterPasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            require_upper: true,
            require_lower: true,
            require_digit: true,
            require_special: true,
            min_entropy_bits: 60,
        }
    }
}

/// The organization MFA + password policy (single global row in v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaPolicy {
    /// Whether MFA is mandatory (enforced at login).
    pub required: bool,
    /// Allowed MFA methods: "totp" | "webauthn" | "email" (JSON array).
    pub allowed_methods: Vec<String>,
    pub master_password_policy: MasterPasswordPolicy,
}

/// A pending (not-yet-confirmed) TOTP enrollment row.
#[derive(Debug, Clone)]
pub struct TotpEnrollmentRow {
    pub enrollment_id: String,
    pub user_id: String,
    pub secret: Vec<u8>,
    pub secret_base32: String,
    pub expires_at: i64,
}

/// A stored one-time recovery code for a user.
#[derive(Debug, Clone)]
pub struct RecoveryCodeRow {
    pub code: String,
    pub used: bool,
}

impl Repository {
    // --- TOTP enrollment (pending) ---

    pub async fn create_totp_enrollment(
        &self,
        enrollment_id: &str,
        user_id: &str,
        secret: &[u8],
        secret_base32: &str,
        now: i64,
        expires_at: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO mfa_totp_enrollments \
             (enrollment_id, user_id, secret, secret_base32, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(enrollment_id)
        .bind(user_id)
        .bind(secret)
        .bind(secret_base32)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a pending enrollment by id, together with its owning user.
    pub async fn get_totp_enrollment(
        &self,
        enrollment_id: &str,
    ) -> Result<Option<TotpEnrollmentRow>, sqlx::Error> {
        let row: Option<(String, Vec<u8>, String, i64)> = sqlx::query_as(
            "SELECT user_id, secret, secret_base32, expires_at \
             FROM mfa_totp_enrollments WHERE enrollment_id = ?",
        )
        .bind(enrollment_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(user_id, secret, secret_base32, expires_at)| TotpEnrollmentRow {
            enrollment_id: enrollment_id.to_string(),
            user_id,
            secret,
            secret_base32,
            expires_at,
        }))
    }

    pub async fn delete_totp_enrollment(&self, enrollment_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM mfa_totp_enrollments WHERE enrollment_id = ?")
            .bind(enrollment_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Remove all stale enrollments for a user (only the newest is meaningful).
    pub async fn clear_totp_enrollments(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM mfa_totp_enrollments WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Active TOTP secret ---

    /// Set (or replace) the user's active TOTP secret, atomically with removing
    /// any pending enrollment for the same user.
    pub async fn activate_totp_secret(
        &self,
        user_id: &str,
        secret: &[u8],
        secret_base32: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM mfa_totp_enrollments WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO mfa_totp_secrets (user_id, secret, secret_base32, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(user_id) DO UPDATE SET \
               secret = excluded.secret, secret_base32 = excluded.secret_base32, \
               updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(secret)
        .bind(secret_base32)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Fetch the active TOTP secret bytes + base32 for a user.
    pub async fn get_totp_secret(
        &self,
        user_id: &str,
    ) -> Result<Option<(Vec<u8>, String)>, sqlx::Error> {
        let row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT secret, secret_base32 FROM mfa_totp_secrets WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Whether the user has configured TOTP.
    pub async fn mfa_has_totp(&self, user_id: &str) -> Result<bool, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM mfa_totp_secrets WHERE user_id = ?")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0 > 0)
    }

    pub async fn delete_totp_secret(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM mfa_totp_secrets WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Recovery codes ---

    /// Store the freshly-issued one-time recovery codes for a user.
    pub async fn insert_recovery_codes(
        &self,
        user_id: &str,
        codes: &[String],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        // First enrollment only: replace any previously issued (unused) codes.
        sqlx::query("DELETE FROM mfa_recovery_codes WHERE user_id = ? AND used = 0")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        for code in codes {
            sqlx::query(
                "INSERT INTO mfa_recovery_codes (user_id, code, used, created_at) VALUES (?, ?, 0, ?)",
            )
            .bind(user_id)
            .bind(code)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Atomically redeem a recovery code. Returns true if the code existed,
    /// was unused, and is now marked used.
    pub async fn redeem_recovery_code(&self, user_id: &str, code: &str) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE mfa_recovery_codes SET used = 1 \
             WHERE user_id = ? AND code = ? AND used = 0",
        )
        .bind(user_id)
        .bind(code)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// List all recovery codes for a user (with used flag).
    pub async fn list_recovery_codes(
        &self,
        user_id: &str,
    ) -> Result<Vec<RecoveryCodeRow>, sqlx::Error> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT code, used FROM mfa_recovery_codes WHERE user_id = ? ORDER BY created_at ASC")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(code, used)| RecoveryCodeRow { code, used: used != 0 })
            .collect())
    }

    pub async fn clear_recovery_codes(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM mfa_recovery_codes WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Policy ---

    /// Read the current policy. Falls back to defaults if the row is missing.
    pub async fn mfa_get_policy(&self) -> Result<MfaPolicy, sqlx::Error> {
        let row: Option<(i64, String, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT required, allowed_methods, min_length, require_upper, require_lower, \
                    require_digit, require_special, min_entropy_bits \
             FROM mfa_policy WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        let (required, allowed, min_length, up, lo, di, sp, ent) = match row {
            Some(r) => r,
            None => return Ok(MfaPolicy::default_policy()),
        };
        let allowed_methods: Vec<String> =
            serde_json::from_str(&allowed).unwrap_or_else(|_| vec!["totp".to_string()]);
        Ok(MfaPolicy {
            required: required != 0,
            allowed_methods,
            master_password_policy: MasterPasswordPolicy {
                min_length: min_length as u32,
                require_upper: up != 0,
                require_lower: lo != 0,
                require_digit: di != 0,
                require_special: sp != 0,
                min_entropy_bits: ent as u32,
            },
        })
    }

    /// Persist the policy (upsert of the single row, id = 1).
    pub async fn mfa_set_policy(&self, policy: &MfaPolicy, now: i64) -> Result<(), sqlx::Error> {
        let allowed =
            serde_json::to_string(&policy.allowed_methods).unwrap_or_else(|_| "[]".to_string());
        let mp = &policy.master_password_policy;
        sqlx::query(
            "INSERT INTO mfa_policy (id, required, allowed_methods, min_length, require_upper, \
                    require_lower, require_digit, require_special, min_entropy_bits, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
               required = excluded.required, allowed_methods = excluded.allowed_methods, \
               min_length = excluded.min_length, require_upper = excluded.require_upper, \
               require_lower = excluded.require_lower, require_digit = excluded.require_digit, \
               require_special = excluded.require_special, \
               min_entropy_bits = excluded.min_entropy_bits, updated_at = excluded.updated_at",
        )
        .bind(policy.required as i64)
        .bind(&allowed)
        .bind(mp.min_length as i64)
        .bind(mp.require_upper as i64)
        .bind(mp.require_lower as i64)
        .bind(mp.require_digit as i64)
        .bind(mp.require_special as i64)
        .bind(mp.min_entropy_bits as i64)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl MfaPolicy {
    fn default_policy() -> Self {
        Self {
            required: false,
            allowed_methods: vec!["totp".to_string()],
            master_password_policy: MasterPasswordPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_repo() -> Repository {
        let path = std::env::temp_dir().join(format!("vautr_mfa_repo_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        Repository::new(pool)
    }

    async fn seed_user(repo: &Repository) {
        repo.create_user("u1", "a@b.c", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], 1_700_000_000_000)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn totp_enrollment_lifecycle() {
        let repo = test_repo().await;
        seed_user(&repo).await;
        let now = 1_700_000_000_000;
        repo.create_totp_enrollment("e1", "u1", &[7u8; 20], "SECRETB32", now, now + 900_000)
            .await
            .unwrap();
        let row = repo.get_totp_enrollment("e1").await.unwrap().unwrap();
        assert_eq!(row.user_id, "u1");
        assert_eq!(row.secret, vec![7u8; 20]);
        // Activating the secret drops the pending enrollment.
        repo.activate_totp_secret("u1", &[8u8; 20], "SECRETB32B", now).await.unwrap();
        assert!(repo.get_totp_enrollment("e1").await.unwrap().is_none());
        assert!(repo.mfa_has_totp("u1").await.unwrap());
        let (secret, _) = repo.get_totp_secret("u1").await.unwrap().unwrap();
        assert_eq!(secret, vec![8u8; 20]);
        repo.delete_totp_secret("u1").await.unwrap();
        assert!(!repo.mfa_has_totp("u1").await.unwrap());
    }

    #[tokio::test]
    async fn recovery_codes_redeem_once() {
        let repo = test_repo().await;
        seed_user(&repo).await;
        let now = 1_700_000_000_000;
        let codes = vec!["AAAA-BBBB".to_string(), "CCCC-DDDD".to_string()];
        repo.insert_recovery_codes("u1", &codes, now).await.unwrap();
        assert_eq!(repo.list_recovery_codes("u1").await.unwrap().len(), 2);
        assert!(repo.redeem_recovery_code("u1", "AAAA-BBBB").await.unwrap());
        // Second redemption of the same code fails.
        assert!(!repo.redeem_recovery_code("u1", "AAAA-BBBB").await.unwrap());
        assert!(!repo.redeem_recovery_code("u1", "UNKNOWN").await.unwrap());
    }

    #[tokio::test]
    async fn policy_roundtrip() {
        let repo = test_repo().await;
        let def = repo.mfa_get_policy().await.unwrap();
        assert!(!def.required);
        assert_eq!(def.master_password_policy.min_length, 12);

        let mut p = MfaPolicy::default_policy();
        p.required = true;
        p.allowed_methods = vec!["totp".into(), "webauthn".into()];
        p.master_password_policy.min_length = 16;
        repo.mfa_set_policy(&p, 1_700_000_000_000).await.unwrap();
        let got = repo.mfa_get_policy().await.unwrap();
        assert!(got.required);
        assert_eq!(got.allowed_methods, vec!["totp".to_string(), "webauthn".to_string()]);
        assert_eq!(got.master_password_policy.min_length, 16);
    }
}
