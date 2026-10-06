-- Vautr server schema — hash session tokens at rest (VTRFIX-SEC-H03).
-- Append-only. Never edit 0001..0016.
--
-- Before this migration the sessions table held the raw bearer token. Any DB
-- read (backup, replica, log leak) yielded valid 24h sessions for every
-- logged-in user. We now store SHA-256(token) and drop any legacy raw rows
-- (users re-login once; this is a one-time session invalidation).

DELETE FROM sessions;

ALTER TABLE sessions RENAME TO sessions_legacy;

CREATE TABLE sessions (
    token_hash  TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL,
    expires_at  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

DROP TABLE sessions_legacy;

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
