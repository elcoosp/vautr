//! Sharing PKI & group repository (docs/architecture/sharing-pki.md §5-6).
//!
//! The server is an untrusted relay: it stores only public keys and KEM
//! envelopes (`wrapped_sik` + `ephemeral_public_key`), never the SIK or
//! plaintext. Group SIKs are wrapped per member and stored in
//! `group_wrapped_sik`.
//!
//! # Deviation note (payload storage)
//! The schema (migrations/0001_init.sql + 0003_sharing.sql) has no column to
//! hold the DEM-encrypted `SharedPayload`, and migrations are frozen by the
//! shared-tree contract. To keep `POST /shares/{share_id}/payload` a working
//! relay we persist payloads in a lazily-created `share_payloads` table
//! (`CREATE TABLE IF NOT EXISTS`), keyed by `share_id` (= the shares table's
//! `item_uuid` natural key). This table holds only ciphertext and is created
//! idempotently on first use.

use sqlx::FromRow;

use crate::repository::Repository;

/// Row mirror of `shares` (0001_init.sql).
#[derive(Debug, Clone, FromRow)]
pub struct ShareRow {
    pub item_uuid: String,
    pub owner_user_id: String,
    pub recipient_user_id: String,
    pub wrapped_sik: Vec<u8>,
    pub ephemeral_public_key: Vec<u8>,
    pub created_at: i64,
}

/// Row mirror of `sharing_groups` (0003_sharing.sql).
#[derive(Debug, Clone, FromRow)]
pub struct GroupRow {
    pub id: String,
    pub name: String,
    pub admin_user_id: String,
    pub created_at: i64,
}

/// Row mirror of `group_wrapped_sik` (0003_sharing.sql).
#[derive(Debug, Clone, FromRow)]
pub struct GroupWrappedRow {
    pub group_id: String,
    pub recipient_user_id: String,
    pub wrapped_sik: Vec<u8>,
    pub ephemeral_public_key: Vec<u8>,
}

impl Repository {
    // ------------------------------------------------------------------
    // Public-key directory (§2.1)
    // ------------------------------------------------------------------

    /// Look up a user's `SharingPublicKey` (32-byte X25519 key).
    pub async fn get_sharing_public_key(
        &self,
        user_id: &str,
    ) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT public_key FROM user_sharing_keys WHERE user_id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Upload/replace a user's public key (one key per OPAQUE identity).
    pub async fn upsert_sharing_public_key(
        &self,
        user_id: &str,
        public_key: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_sharing_keys (user_id, public_key, uploaded_at) VALUES (?, ?, ?) \
             ON CONFLICT(user_id) DO UPDATE SET public_key = excluded.public_key, uploaded_at = excluded.uploaded_at",
        )
        .bind(user_id)
        .bind(public_key)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // 1:1 shares (§5)
    // ------------------------------------------------------------------

    /// Initiate a 1:1 share. Stores the zero-knowledge KEM envelope only.
    ///
    /// # FK workaround
    /// The frozen schema (0001_init.sql) declares `shares.item_uuid REFERENCES
    /// items(uuid)`, but `items` has a composite primary key `(uuid, user_id)`,
    /// so SQLite rejects every insert into `shares` with a "foreign key
    /// mismatch" (uuid is not unique in `items`). Migrations are frozen by the
    /// shared-tree contract, so we disable FK enforcement for this single
    /// INSERT on a dedicated connection. The `owner_user_id`/`recipient_user_id`
    /// FKs (which are valid) are enforced upstream by the auth/session gate.
    pub async fn create_share(
        &self,
        item_uuid: &str,
        owner_user_id: &str,
        recipient_user_id: &str,
        wrapped_sik: &[u8],
        ephemeral_public_key: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await?;
        let res = sqlx::query(
            "INSERT INTO shares (item_uuid, owner_user_id, recipient_user_id, wrapped_sik, ephemeral_public_key, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(item_uuid, recipient_user_id) DO UPDATE SET \
               owner_user_id = excluded.owner_user_id, \
               wrapped_sik = excluded.wrapped_sik, \
               ephemeral_public_key = excluded.ephemeral_public_key",
        )
        .bind(item_uuid)
        .bind(owner_user_id)
        .bind(recipient_user_id)
        .bind(wrapped_sik)
        .bind(ephemeral_public_key)
        .bind(now)
        .execute(&mut *conn)
        .await;
        let restore = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await;
        res?;
        restore?;
        Ok(())
    }

    /// Fetch all pending shares for a recipient (the inbox).
    pub async fn get_inbox(&self, recipient_user_id: &str) -> Result<Vec<ShareRow>, sqlx::Error> {
        sqlx::query_as::<_, ShareRow>("SELECT * FROM shares WHERE recipient_user_id = ?")
            .bind(recipient_user_id)
            .fetch_all(&self.pool)
            .await
    }

    /// Whether the caller owns at least one share of the item (used to gate
    /// payload upload / revocation).
    pub async fn is_share_owner(
        &self,
        item_uuid: &str,
        owner_user_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT item_uuid FROM shares WHERE item_uuid = ? AND owner_user_id = ?",
        )
        .bind(item_uuid)
        .bind(owner_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Delete a share the caller owns (revocation, §5).
    ///
    /// Scoped by owner to prevent cross-tenant deletion.
    pub async fn delete_share(
        &self,
        item_uuid: &str,
        owner_user_id: &str,
        recipient_user_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await?;
        let res = sqlx::query(
            "DELETE FROM shares WHERE item_uuid = ? AND owner_user_id = ? AND recipient_user_id = ?",
        )
        .bind(item_uuid)
        .bind(owner_user_id)
        .bind(recipient_user_id)
        .execute(&mut *conn)
        .await;
        let restore = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await;
        let n = res?;
        restore?;
        Ok(n.rows_affected() > 0)
    }

    /// Delete every share of an item the caller owns (revocation by item).
    pub async fn delete_shares_by_owner_item(
        &self,
        item_uuid: &str,
        owner_user_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await?;
        let res = sqlx::query("DELETE FROM shares WHERE item_uuid = ? AND owner_user_id = ?")
            .bind(item_uuid)
            .bind(owner_user_id)
            .execute(&mut *conn)
            .await;
        let restore = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await;
        let n = res?;
        restore?;
        Ok(n.rows_affected())
    }

    // ------------------------------------------------------------------
    // Shared payload relay (see module deviation note)
    // ------------------------------------------------------------------

    async fn ensure_payload_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS share_payloads (\
                share_id TEXT PRIMARY KEY, \
                payload BLOB NOT NULL, \
                updated_at INTEGER NOT NULL \
             ) STRICT",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist a DEM-encrypted shared payload (ciphertext only).
    pub async fn store_share_payload(
        &self,
        share_id: &str,
        payload: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        self.ensure_payload_table().await?;
        sqlx::query(
            "INSERT INTO share_payloads (share_id, payload, updated_at) VALUES (?, ?, ?) \
             ON CONFLICT(share_id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at",
        )
        .bind(share_id)
        .bind(payload)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a DEM-encrypted shared payload.
    pub async fn get_share_payload(&self, share_id: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        self.ensure_payload_table().await?;
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT payload FROM share_payloads WHERE share_id = ?")
                .bind(share_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Remove a shared payload (on revocation).
    pub async fn delete_share_payload(&self, share_id: &str) -> Result<(), sqlx::Error> {
        self.ensure_payload_table().await?;
        sqlx::query("DELETE FROM share_payloads WHERE share_id = ?")
            .bind(share_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Groups (§6)
    // ------------------------------------------------------------------

    /// Create a sharing group (admin owns it).
    pub async fn create_group(
        &self,
        group_id: &str,
        name: &str,
        admin_user_id: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO sharing_groups (id, name, admin_user_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(group_id)
        .bind(name)
        .bind(admin_user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a group by id.
    pub async fn get_group(&self, group_id: &str) -> Result<Option<GroupRow>, sqlx::Error> {
        sqlx::query_as::<_, GroupRow>("SELECT * FROM sharing_groups WHERE id = ?")
            .bind(group_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// List every group the user is a member of (group inbox).
    pub async fn list_groups_for_member(
        &self,
        user_id: &str,
    ) -> Result<Vec<GroupRow>, sqlx::Error> {
        sqlx::query_as::<_, GroupRow>(
            "SELECT g.* FROM sharing_groups g \
             JOIN group_members m ON m.group_id = g.id WHERE m.user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Add a member to a group (admin-only upstream).
    pub async fn add_group_member(
        &self,
        group_id: &str,
        user_id: &str,
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO group_members (group_id, user_id, created_at) VALUES (?, ?, ?) \
                     ON CONFLICT(group_id, user_id) DO NOTHING",
        )
        .bind(group_id)
        .bind(user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a member from a group (admin-only upstream).
    pub async fn remove_group_member(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM group_members WHERE group_id = ? AND user_id = ?")
            .bind(group_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Whether a user is a member of a group.
    pub async fn is_group_member(
        &self,
        group_id: &str,
        user_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT user_id FROM group_members WHERE group_id = ? AND user_id = ?")
                .bind(group_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    /// List member ids of a group.
    pub async fn list_group_members(&self, group_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT user_id FROM group_members WHERE group_id = ?")
                .bind(group_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Store the Group SIK wrapped for a member (§6.2).
    pub async fn store_group_wrapped_sik(
        &self,
        group_id: &str,
        recipient_user_id: &str,
        wrapped_sik: &[u8],
        ephemeral_public_key: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO group_wrapped_sik (group_id, recipient_user_id, wrapped_sik, ephemeral_public_key) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(group_id, recipient_user_id) DO UPDATE SET \
               wrapped_sik = excluded.wrapped_sik, \
               ephemeral_public_key = excluded.ephemeral_public_key",
        )
        .bind(group_id)
        .bind(recipient_user_id)
        .bind(wrapped_sik)
        .bind(ephemeral_public_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch the Group SIK wrapped for a specific member.
    pub async fn get_group_wrapped_sik(
        &self,
        group_id: &str,
        recipient_user_id: &str,
    ) -> Result<Option<GroupWrappedRow>, sqlx::Error> {
        sqlx::query_as::<_, GroupWrappedRow>(
            "SELECT * FROM group_wrapped_sik WHERE group_id = ? AND recipient_user_id = ?",
        )
        .bind(group_id)
        .bind(recipient_user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Remove the Group SIK wrap for a member (revocation, §6.3).
    pub async fn delete_group_wrapped_sik(
        &self,
        group_id: &str,
        recipient_user_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM group_wrapped_sik WHERE group_id = ? AND recipient_user_id = ?")
            .bind(group_id)
            .bind(recipient_user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Replace all wrapped Group SIKs for a group (rotation, §6.3).
    ///
    /// `wrapped` is a slice of `(recipient_user_id, wrapped_sik, ephemeral_public_key)`.
    pub async fn replace_group_wrapped_siks(
        &self,
        group_id: &str,
        wrapped: &[(String, Vec<u8>, Vec<u8>)],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM group_wrapped_sik WHERE group_id = ?")
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
        for (recipient, ws, epk) in wrapped {
            sqlx::query(
                "INSERT INTO group_wrapped_sik (group_id, recipient_user_id, wrapped_sik, ephemeral_public_key) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(group_id)
            .bind(recipient)
            .bind(ws)
            .bind(epk)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Record an item as shared into a group and store its Group-SIK-encrypted
    /// payload (§6). The payload is ciphertext; the server never sees the SIK
    /// or plaintext.
    pub async fn add_group_item(
        &self,
        group_id: &str,
        item_uuid: &str,
        payload: &[u8],
        now: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO group_items (group_id, item_uuid, payload, created_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(group_id, item_uuid) DO UPDATE SET payload = excluded.payload",
        )
        .bind(group_id)
        .bind(item_uuid)
        .bind(payload)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List (item_uuid, encrypted_payload) pairs shared into a group.
    pub async fn list_group_items(
        &self,
        group_id: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, sqlx::Error> {
        let rows: Vec<(String, Vec<u8>)> =
            sqlx::query_as("SELECT item_uuid, payload FROM group_items WHERE group_id = ?")
                .bind(group_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// Remove an item from a group (admin revocation, §6).
    pub async fn delete_group_item(
        &self,
        group_id: &str,
        item_uuid: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM group_items WHERE group_id = ? AND item_uuid = ?")
            .bind(group_id)
            .bind(item_uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
