-- Vautr server schema — Mandatory MFA + policies (mlp-scope.md §3/§5, Wave A4).
-- Canonical source: docs/architecture/mlp-scope.md §3 (mandatory MFA) + §5 (policies).
-- Append-only numbered migration: never edit 0001..0009.

-- Pending TOTP enrollment: a shared secret issued by POST /mfa/totp/issue but
-- not yet confirmed by a valid code. Promoted to `mfa_totp_secrets` on verify.
CREATE TABLE IF NOT EXISTS mfa_totp_enrollments (
    enrollment_id TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL,
    secret        BLOB NOT NULL,          -- raw secret bytes (>= 16, CSPRNG)
    secret_base32 TEXT NOT NULL,          -- RFC4648 base32, no padding
    created_at    INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_mfa_totp_enrollments_user ON mfa_totp_enrollments(user_id);

-- Active TOTP shared secret (one per user). Present => user has configured
-- TOTP as an MFA method (shown in MfaStatus.configured_methods).
CREATE TABLE IF NOT EXISTS mfa_totp_secrets (
    user_id       TEXT PRIMARY KEY,
    secret        BLOB NOT NULL,
    secret_base32 TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

-- One-time recovery codes, issued once on first TOTP enrollment (lockout safety).
CREATE TABLE IF NOT EXISTS mfa_recovery_codes (
    user_id    TEXT NOT NULL,
    code       TEXT NOT NULL,
    used       INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, code),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_mfa_recovery_codes_user ON mfa_recovery_codes(user_id);

-- Organization MFA & master-password policy. A single global row (id = 1) in v1
-- (the server's tenant is the organization). `allowed_methods` is a JSON array
-- of MfaMethod ("totp" | "webauthn" | "email"). The master-password policy is
-- advisory server-side (the server never sees the master password, OPAQUE): it
-- is stored + served to clients which enforce it at registration/MP-set.
CREATE TABLE IF NOT EXISTS mfa_policy (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    required          INTEGER NOT NULL DEFAULT 0,
    allowed_methods   TEXT    NOT NULL DEFAULT '["totp"]',
    min_length        INTEGER NOT NULL DEFAULT 12,
    require_upper     INTEGER NOT NULL DEFAULT 1,
    require_lower     INTEGER NOT NULL DEFAULT 1,
    require_digit     INTEGER NOT NULL DEFAULT 1,
    require_special   INTEGER NOT NULL DEFAULT 1,
    min_entropy_bits  INTEGER NOT NULL DEFAULT 60,
    updated_at        INTEGER NOT NULL
) STRICT;

-- Seed the default policy (MFA not yet mandatory; standard password policy).
INSERT INTO mfa_policy (
    id, required, allowed_methods, min_length, require_upper, require_lower,
    require_digit, require_special, min_entropy_bits, updated_at
) VALUES (1, 0, '["totp"]', 12, 1, 1, 1, 1, 60, 0)
ON CONFLICT(id) DO NOTHING;
