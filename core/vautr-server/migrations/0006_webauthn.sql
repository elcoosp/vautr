-- Vautr server schema — WebAuthn (FIDO2) optional second factor (VTR-052).
-- Stores one row per registered security key / passkey credential, scoped to
-- a user. The `serialized` BLOB is the webauthn-rs `SecurityKey` (JSON) which
-- carries the COSE public key + monotonic counter; the `cred_id` / `counter`
-- columns mirror it for cheap listing and "has second factor" lookups.
-- Feature is off by default; the table is created unconditionally so the
-- migration is applied consistently even when the `webauthn` server feature is
-- compiled out.

CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    TEXT NOT NULL,
    cred_id    TEXT NOT NULL,   -- base64url credential id (unique per user)
    label      TEXT NOT NULL,   -- human friendly device label (e.g. "YubiKey 5")
    serialized BLOB NOT NULL,   -- webauthn-rs SecurityKey (JSON: COSE pubkey + counter)
    counter    INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (user_id, cred_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user ON webauthn_credentials(user_id);
