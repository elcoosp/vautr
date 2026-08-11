//! `users` repository domain: account lifecycle, credential record storage,
//! epoch gate (`min_enc_key_gen`), and SVK blob rotation/recovery reads.

use sqlx::FromRow;

use crate::repository::Repository;

/// Row mirror of `users` (server-db.md §3).
#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub kdf_salt: Vec<u8>,
    pub opaque_record: Vec<u8>,
    pub svk_ciphertext_blob: Vec<u8>,
    pub svk_ciphertext_blob_rk: Vec<u8>,
    pub min_enc_key_gen: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Repository {
    /// Insert a new user. `min_enc_key_gen` defaults to 1.
    pub async fn create_user(
        &self,
        id: &str,
        email: &str,
        kdf_salt: &[u8],
        opaque_record: &[u8],
        svk_ciphertext_blob: &[u8],
        svk_ciphertext_blob_rk: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob, svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(id)
        .bind(email)
        .bind(kdf_salt)
        .bind(opaque_record)
        .bind(svk_ciphertext_blob)
        .bind(svk_ciphertext_blob_rk)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a user by email (login/registration lookup).
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
    }

    /// Fetch the RK-wrapped SVK blob for recovery (crypto.md §7, REQ-RECOVERY-02).
    pub async fn get_user_svk_rk(&self, user_id: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT svk_ciphertext_blob_rk FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Read the user's current `min_enc_key_gen` (epoch gate source).
    pub async fn min_enc_key_gen(&self, user_id: &str) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT min_enc_key_gen FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Fetch a user by id.
    pub async fn get_user_by_id(&self, id: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Update the global epoch gate (ADR-006 / REQ-API-01).
    pub async fn update_min_enc_key_gen(
        &self,
        user_id: &str,
        new_gen: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET min_enc_key_gen = ? WHERE id = ?")
            .bind(new_gen)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Replace the stored (MP-wrapped and RK-wrapped) SVK blobs (rotation / recovery).
    pub async fn update_svk(
        &self,
        user_id: &str,
        svk_ciphertext_blob: &[u8],
        svk_ciphertext_blob_rk: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET svk_ciphertext_blob = ?, svk_ciphertext_blob_rk = ? WHERE id = ?",
        )
        .bind(svk_ciphertext_blob)
        .bind(svk_ciphertext_blob_rk)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
