-- Vautr server schema — Secrets (mlp-wave-plan.md §3 A3, mlp-scope.md §4).
-- Append-only numbered migration: never edit 0001..0008.
--
-- Secrets live in the same Projects as vault items (one secret -> one Project).
-- Storage is metadata-first and ciphertext-only: `value_ciphertext` holds the
-- client-side AEAD ciphertext (the server never sees the plaintext value),
-- mirroring the existing encrypted-item pattern (`items.payload`).

-- A single secret, scoped to a project. `version` increments on update so
-- clients can detect concurrent edits.
CREATE TABLE IF NOT EXISTS secrets (
    uuid              TEXT PRIMARY KEY,
    project_id        TEXT NOT NULL,
    key               TEXT NOT NULL,
    value_ciphertext  BLOB NOT NULL,
    version           INTEGER NOT NULL DEFAULT 1,
    created_by        TEXT NOT NULL,
    last_accessed_at  INTEGER,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id)    ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_secrets_project     ON secrets(project_id);
CREATE INDEX IF NOT EXISTS idx_secrets_project_key ON secrets(project_id, key);

-- `secrets:reveal` scope grant (Wave A3).
--
-- The full machine-account / access-token scope model is owned by Wave A2
-- (`handlers/tokens.rs`, `handlers/machine_accounts.rs`). Until A2 lands, reveal
-- access to a project's secret VALUES is modelled as an explicit per-project
-- grant so GET /secrets/{uuid}/value can be gated for real. The integrator wires
-- A2's `secrets:reveal` token scope over this table.
--
-- One row per (project, user): the user may reveal secret values in that
-- project (in addition to the project owner, who always may).
CREATE TABLE IF NOT EXISTS secret_reveal_grants (
    project_id  TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    granted_by  TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (project_id, user_id),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id)    REFERENCES users(id)    ON DELETE CASCADE,
    FOREIGN KEY (granted_by) REFERENCES users(id)    ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_secret_reveal_user ON secret_reveal_grants(user_id);
CREATE INDEX IF NOT EXISTS idx_secret_reveal_proj ON secret_reveal_grants(project_id);
