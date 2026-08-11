-- Vautr server schema — emergency recovery & account lifecycle (defined in docs/architecture/emergency-recovery-account.md).
-- Canonical source: docs/architecture/emergency-recovery-account.md
-- This migration extends the existing `users` table and adds recovery sessions
-- for the RK-recovery flow (§2.3) and the unauthenticated reclaim flow (§4.2).
-- The `users`, `items`, `sessions`, `shares`, and `server_config` tables already
-- exist from 0001_init.sql and are NOT recreated here.

-- RK public key used to authenticate recovery requests via Ed25519 signature
-- (emergency-recovery-account.md §2.2, REQ-RECOVERY-03).
ALTER TABLE users ADD COLUMN rk_public_key BLOB;

-- Account reclaim flow: verification token bound to the email, single-use.
ALTER TABLE users ADD COLUMN reclaim_token TEXT;

-- Account reclaim suspension: epoch-ms timestamp until which the old vault is
-- suspended (hidden) before permanent purge (emergency-recovery-account.md §4.2,
-- 30-day grace period). NULL = not suspended.
ALTER TABLE users ADD COLUMN suspended_until INTEGER;

-- One-time-use, short-lived recovery session token (emergency-recovery-account.md §2.3).
CREATE TABLE IF NOT EXISTS recovery_sessions (
    token        TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL,
    expires_at   INTEGER NOT NULL,
    created_at   INTEGER NOT NULL,
    one_time_use INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_recovery_sessions_user  ON recovery_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_recovery_sessions_token ON recovery_sessions(token);
