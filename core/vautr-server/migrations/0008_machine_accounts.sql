-- Vautr server schema — Machine Accounts + Access Tokens (mlp-scope.md §4, Wave A2).
-- Canonical source: docs/architecture/mlp-scope.md §4, docs/architecture/mlp-wave-plan.md (A2).
-- Append-only numbered migration: never edit 0001..0007.

-- Machine accounts: non-human identities for CI/CD, apps, and agents. Owned by a
-- user (the admin who provisions them), scoped to one optional project, carrying a
-- fine-grained scope set and an optional hard expiry.
CREATE TABLE IF NOT EXISTS machine_accounts (
    uuid            TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    owner_user_id   TEXT NOT NULL,
    project_uuid    TEXT,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'disabled', 'revoked')),
    scopes          TEXT NOT NULL DEFAULT '[]',      -- JSON array of AccessScope
    expires_at      INTEGER,                          -- epoch ms; NULL = no expiry
    last_used_at    INTEGER,
    created_at      INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Access tokens issued to machine accounts (or, for standalone programmatic use,
-- to a user directly). Only a SHA-256 hash of the token secret is stored; the raw
-- secret is returned to the issuer exactly once. Revocation and expiry are enforced
-- on every verify.
CREATE TABLE IF NOT EXISTS access_tokens (
    uuid                 TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    owner_user_id        TEXT NOT NULL,
    machine_account_uuid TEXT,
    project_uuid         TEXT,
    scopes               TEXT NOT NULL DEFAULT '[]',  -- JSON array of AccessScope
    token_hash           TEXT NOT NULL UNIQUE,         -- base64(sha256(token secret))
    prefix               TEXT NOT NULL,                -- first 8 chars, for display
    expires_at           INTEGER,                      -- epoch ms; NULL = no expiry
    revoked_at           INTEGER,
    last_used_at         INTEGER,
    created_at           INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id)        REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (machine_account_uuid) REFERENCES machine_accounts(uuid) ON DELETE SET NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_machine_accounts_owner  ON machine_accounts(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_machine_accounts_status ON machine_accounts(status);
CREATE INDEX IF NOT EXISTS idx_access_tokens_owner     ON access_tokens(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_access_tokens_machine   ON access_tokens(machine_account_uuid);
CREATE INDEX IF NOT EXISTS idx_access_tokens_hash      ON access_tokens(token_hash);
