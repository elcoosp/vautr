-- Vautr server schema — persisted recovery challenge nonce (VTRFIX-SEC-H09).
-- Append-only. Never edit 0001..0018.
--
-- The recovery flow requires the client to sign a server-issued nonce with
-- the Ed25519 Recovery Key. Previously the nonce was never persisted, so a
-- captured (nonce, signature) pair could be replayed forever. Now the nonce
-- is stored per user, single-use, and expires in 5 minutes.

CREATE TABLE IF NOT EXISTS recovery_challenges (
    user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    nonce       BLOB NOT NULL,
    expires_at  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL
) STRICT;
