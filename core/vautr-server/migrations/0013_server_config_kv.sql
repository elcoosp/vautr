-- Reconcile `server_config` to the generic (key, value) shape expected by
-- `repository/config.rs` and the OPAQUE server setup persistence in
-- `handlers/mod.rs` (`SETUP_KEY = "opaque_server_setup"`).
--
-- migration `0001` created `server_config` as a single-row table
-- `(id, opaque_server_public_key, created_at)` with `id = 1`. The active
-- repository code reads/writes it as a key/value store (`get_config(key)` /
-- `set_config(key, value)`), which would otherwise fail at runtime with a
-- "no such column: value" error on a freshly-migrated database.
--
-- `0001` is frozen (append-only rule), so this numbered migration converts the
-- table in place, preserving any existing OPAQUE server public key under the
-- key the code looks up.

ALTER TABLE server_config RENAME TO server_config_legacy;

CREATE TABLE server_config (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
) STRICT;

INSERT INTO server_config (key, value)
SELECT 'opaque_server_setup', opaque_server_public_key
FROM server_config_legacy
WHERE opaque_server_public_key IS NOT NULL;

DROP TABLE server_config_legacy;
