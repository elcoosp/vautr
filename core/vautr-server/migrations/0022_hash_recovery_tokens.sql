-- VTRFIX (MEDIUM): recovery + reclaim tokens are hashed at rest (SHA-256 hex),
-- matching every other bearer token in the system (session hashes: 0017,
-- access tokens, pending-MFA tokens).
--
-- `recovery_sessions.token` used to hold the RAW one-time recovery token and
-- `users.reclaim_token` the RAW reclaim token. Both are now stored as
-- SHA-256 hex (see repository/recovery.rs); the raw token is still handed to
-- the client exactly once.
--
-- Safe to clear on upgrade:
-- - recovery_sessions rows are 5-minute single-use challenges (the client
--   simply re-runs /account/recover/verify to mint a fresh one).
-- - reclaim tokens are re-issuable: POST /account/reclaim (with RK
--   proof-of-possession) mints a fresh token each time it is called.
--
-- SQLite cannot drop/replace a PRIMARY KEY in place, so the table is rebuilt
-- with `token_hash TEXT PRIMARY KEY` and the plaintext column is dropped.

CREATE TABLE recovery_sessions_hashed (
    token_hash   TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL,
    expires_at   INTEGER NOT NULL,
    created_at   INTEGER NOT NULL,
    one_time_use INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- Plaintext tokens are never copied over: pre-migration rows are 5-minute
-- single-use challenges and re-issuable on demand.
DELETE FROM recovery_sessions;

DROP TABLE recovery_sessions;
ALTER TABLE recovery_sessions_hashed RENAME TO recovery_sessions;

CREATE INDEX IF NOT EXISTS idx_recovery_sessions_user ON recovery_sessions(user_id);

-- Old plaintext reclaim tokens are invalidated; the reclaim flow re-issues a
-- fresh token on the next POST /account/reclaim.
UPDATE users SET reclaim_token = NULL;
