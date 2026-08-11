-- Vautr server schema — sharing PKI & groups (defined in docs/architecture/sharing-pki.md).
-- Canonical source: docs/architecture/sharing-pki.md
-- This migration adds the sharing public-key directory (§2.1) and group model
-- (§6). The existing `shares` table from 0001_init.sql is NOT recreated here.

-- Public sharing key directory: one key per OPAQUE identity
-- (sharing-pki.md §2.1, GET /users/{uuid}/public-key).
CREATE TABLE IF NOT EXISTS user_sharing_keys (
    user_id     TEXT PRIMARY KEY,
    public_key  BLOB NOT NULL,
    uploaded_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Sharing groups (1:N). Admin owns group membership and SIK wrapping.
CREATE TABLE IF NOT EXISTS sharing_groups (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    admin_user_id TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    FOREIGN KEY (admin_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Group membership (sharing-pki.md §6.1).
CREATE TABLE IF NOT EXISTS group_members (
    group_id   TEXT NOT NULL,
    user_id    TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (group_id, user_id),
    FOREIGN KEY (group_id) REFERENCES sharing_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Group SIK wrapped for each member with that member's public key
-- (sharing-pki.md §6.1-6.2, group_wrapped_sik).
CREATE TABLE IF NOT EXISTS group_wrapped_sik (
    group_id            TEXT NOT NULL,
    recipient_user_id   TEXT NOT NULL,
    wrapped_sik         BLOB NOT NULL,
    ephemeral_public_key BLOB NOT NULL,
    PRIMARY KEY (group_id, recipient_user_id),
    FOREIGN KEY (group_id) REFERENCES sharing_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Items shared into a group (sharing-pki.md §6, group_items).
CREATE TABLE IF NOT EXISTS group_items (
    group_id   TEXT NOT NULL,
    item_uuid  TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (group_id, item_uuid),
    FOREIGN KEY (group_id) REFERENCES sharing_groups(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_sharing_groups_admin      ON sharing_groups(admin_user_id);
CREATE INDEX IF NOT EXISTS idx_group_members_user        ON group_members(user_id);
CREATE INDEX IF NOT EXISTS idx_group_wrapped_sik_recip   ON group_wrapped_sik(recipient_user_id);
CREATE INDEX IF NOT EXISTS idx_group_items_item          ON group_items(item_uuid);
