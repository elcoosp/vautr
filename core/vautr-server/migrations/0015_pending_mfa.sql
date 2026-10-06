-- Vautr server schema — pending MFA challenges (VTRFIX-SEC-C03).
-- Append-only. Never edit 0001..0014.
--
-- Rationale: when a user has a TOTP secret configured, `login_finish` MUST
-- NOT mint a session until the second factor is verified. This table holds
-- the short-lived, single-use, hashed pending token that carries the
-- "OPAQUE handshake succeeded but second factor pending" state.
--
-- `token_hash` = hex(SHA-256(raw_pending_token)). The raw token is returned
-- to the client exactly once. `attempts` increments on each failed code and
-- the row is invalidated at 5 attempts or when `expires_at` elapses.

CREATE TABLE IF NOT EXISTS pending_mfa (
    token_hash  TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_pending_mfa_user ON pending_mfa(user_id);
