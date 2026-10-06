-- Vautr server schema — TOTP replay cache + failure tracking + hashed recovery codes (VTRFIX-SEC-H11).
-- Append-only. Never edit 0001..0019.

-- Used TOTP time-steps per user, so a code cannot be presented twice inside
-- its validity window.
CREATE TABLE IF NOT EXISTS totp_used_steps (
    user_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    time_step INTEGER NOT NULL,
    used_at   INTEGER NOT NULL,
    PRIMARY KEY (user_id, time_step)
) STRICT;

-- Brute-force tracking. Six failed codes in a row lock the account for 15 min.
ALTER TABLE users ADD COLUMN totp_fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN totp_lock_until INTEGER NOT NULL DEFAULT 0;

-- Hash recovery codes at rest. Replace the old plaintext `code` column with a
-- SHA-256 hash; the plaintext is returned to the user exactly once.
DELETE FROM mfa_recovery_codes;
DROP TABLE mfa_recovery_codes;
CREATE TABLE mfa_recovery_codes (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash   TEXT NOT NULL,
    used        INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (user_id, code_hash)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_mfa_recovery_codes_user ON mfa_recovery_codes(user_id);
