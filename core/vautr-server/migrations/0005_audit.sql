-- Vautr server schema — audit log (verification.md NFR / audit intent).
-- Canonical source: docs/architecture/verification.md (NFR audit requirement)
-- Server hardening (Wave B5): append-only record of security-relevant actions.

CREATE TABLE IF NOT EXISTS audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    TEXT,
    action     TEXT NOT NULL,  -- e.g. 'login', 'recover', 'reclaim', 'delete_account', 'rotate_key'
    actor      TEXT,
    detail     TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_audit_log_user       ON audit_log(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action     ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at);
