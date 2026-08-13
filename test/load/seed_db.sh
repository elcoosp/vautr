#!/usr/bin/env bash
# Seed a SQLite DB for the Vautr k6 load suite (test/load/).
#
# Creates a `users` + `sessions` row for a throwaway load-test account and a
# batch of `items` rows so that GET /sync/pull has real data to page through.
# The OPAQUE auth flow is intentionally NOT replayed here — we insert a session
# token directly (the same shape the server writes on login), which is what the
# k6 scripts authenticate with via VAUTR_TOKEN.
#
# Usage:
#   DB=/abs/path/vautr.db TOKEN=loadtok ./seed_db.sh
# Defaults: DB=./.loadtest/vautr.db, TOKEN=loadtok, 2000 seeded items.

set -euo pipefail

DB="${DB:-./.loadtest/vautr.db}"
TOKEN="${TOKEN:-loadtok}"
N_ITEMS="${N_ITEMS:-2000}"
USER_ID="load-user-1"
NOW_MS="$(($(date +%s) * 1000))"
FAR_FUTURE="$((NOW_MS + 86400000000))"   # ~2.7 years out

mkdir -p "$(dirname "$DB")"

sqlite3 "$DB" <<SQL
-- NOTE: foreign_keys is left OFF on purpose. The seeded schema has a
-- composite-PK FK (shares/project_items -> items) that SQLite validates as a
-- "mismatch" at insert time; the runtime server enforces its own integrity, so
-- we seed without FK enforcement to avoid tripping that schema quirk.
-- PRAGMA foreign_keys = ON;

INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob, svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at)
VALUES ('$USER_ID', 'load@example.com', X'0000000000000000000000000000000000000000000000000000000000000000',
        X'0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
        X'0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
        X'0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
        1, $NOW_MS, $NOW_MS)
ON CONFLICT(id) DO NOTHING;

INSERT INTO sessions (token, user_id, expires_at, created_at)
VALUES ('$TOKEN', '$USER_ID', $FAR_FUTURE, $NOW_MS)
ON CONFLICT(token) DO UPDATE SET user_id = excluded.user_id, expires_at = excluded.expires_at;

DELETE FROM items WHERE user_id = '$USER_ID';
WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c WHERE n < $N_ITEMS)
INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at)
SELECT
  'load-item-' || n,
  '$USER_ID',
  n,
  1,
  NULL,
  X'deadbeef',
  $NOW_MS + n
FROM c;
SQL

echo "Seeded $N_ITEMS items for user '$USER_ID' (token='$TOKEN') into $DB"
