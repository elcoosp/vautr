# Vautr Server Database Schema & Persistence Contract

This document defines the exact server-side SQLite schema, indexing strategy,
PRAGMAs, and write rules for the Vautr **server** (the untrusted, zero-knowledge
encrypted blob store). It is the server counterpart to
[`docs/architecture/db-contract.md`], which defines the *client* local schema.

Where the client schema is absent (this was gap #2), this document is the
canonical source. It is derived from:
- [`docs/architecture/api.md`] — endpoint shapes, OCC, epoch gating, cursor expiry.
- [`docs/architecture/crypto.md`] — per-user `kdf_salt`, SVK wrapping.
- [`docs/spec/arch-design.md`] §3.3 — server container (users / items / shares).
- ADR-001 (SQLite + WAL), ADR-004 (atomic OCC), ADR-006 (epoch/rotation).

---

## 1. Principles (server persistence rules)

1. **WAL + Normal** — `journal_mode=WAL`, `synchronous=NORMAL` (ADR-001).
2. **STRICT tables** — every `CREATE TABLE` uses `STRICT` to reject affinity drift.
3. **Foreign keys ON** — referential integrity (`user_id` scoping) enforced.
4. **Zero-knowledge** — the server stores only `uuid`, `version`, `enc_key_gen`,
   `deleted_date`, and opaque `payload` BLOB. Never plaintext PII (BR-1, NFR-SEC-01).
5. **Atomic OCC** — mutations use `UPDATE ... WHERE version = :target_version`;
   `rows_affected == 0` ⇒ `412` (ADR-004, REQ-API-02).
6. **Epoch gating** — writes with `enc_key_gen < users.min_enc_key_gen` are
   rejected with `422 key_generation_too_old` (REQ-API-01).
7. **i64 storage** — SQLite has no `u64`; all versions/epochs/cursors are cast to
   `i64`. Callers must validate against `i64::MAX` on ingest (per db-contract §1.6).

---

## 2. Connection PRAGMAs

Applied once at pool creation (see `vautr-server::db::connect`):

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;   -- SQLITE_BUSY backoff under WAL contention (ADR-001)
```

---

## 3. Schema (migrations/0001_init.sql)

### `users` — one row per account
Holds the per-user KDF salt (crypto.md §2.2), the OPAQUE registration record,
the KEK-wrapped SVK blob (fetched at login, REQ-AUTH-03 / `/account/status`),
and the global encryption epoch `min_enc_key_gen`.

```sql
CREATE TABLE IF NOT EXISTS users (
    id                    TEXT PRIMARY KEY,          -- account uuid
    email                 TEXT NOT NULL UNIQUE,
    kdf_salt              BLOB NOT NULL,             -- 32 bytes, crypto.md §2.2
    opaque_record        BLOB NOT NULL,             -- OPAQUE registration record (opaque-ke, VautrSuite)
    svk_ciphertext_blob  BLOB NOT NULL,             -- KEK-wrapped SVK
    svk_ciphertext_blob_rk BLOB NOT NULL,           -- KEK_RK-wrapped SVK (crypto.md §7, REQ-RECOVERY-02)
    min_enc_key_gen      INTEGER NOT NULL DEFAULT 1,-- global epoch (ADR-006)
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL
) STRICT;
```

### `items` — one row per vault item (per user)
Opaque blob store. `version` powers OCC; `enc_key_gen` powers epoch gating and
rotation resumability. `deleted_date IS NULL` ⇒ live; tombstones keep the row
with `payload = NULL` so `/sync/pull` can still report metadata.

```sql
CREATE TABLE IF NOT EXISTS items (
    uuid         TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 1,
    enc_key_gen  INTEGER NOT NULL DEFAULT 1,
    deleted_date INTEGER,                          -- NULL = live; set on tombstone
    payload      BLOB,                             -- opaque ciphertext; NULL when tombstoned
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (uuid, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_items_user_version ON items(user_id, version);
CREATE INDEX IF NOT EXISTS idx_items_user_encgen  ON items(user_id, enc_key_gen);
```

### `sessions` — bearer tokens (OPAQUE login/finish)
Short-lived auth tokens returned by `/auth/login/finish`. Scoped by `user_id`.

```sql
CREATE TABLE IF NOT EXISTS sessions (
    token       TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL,
    expires_at  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;
```

### `shares` — 1:1 secure sharing (REQ-SHARE-01..04)
Per-recipient wrapped SIK + ephemeral public key. PK includes recipient so a
user can be re-shared/revoked independently. (Sharing is "Should" priority; the
table exists and is written by the sharing flow (ADR-007, KEM locked).
See docs/architecture/adr-007-sharing-kem.md.

```sql
CREATE TABLE IF NOT EXISTS shares (
    item_uuid            TEXT NOT NULL,
    owner_user_id        TEXT NOT NULL,
    recipient_user_id    TEXT NOT NULL,
    wrapped_sik          BLOB NOT NULL,
    ephemeral_public_key BLOB NOT NULL,
    created_at           INTEGER NOT NULL,
    PRIMARY KEY (item_uuid, recipient_user_id),
    FOREIGN KEY (item_uuid) REFERENCES items(uuid) ON DELETE CASCADE,
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_user_id) REFERENCES users(id) ON DELETE CASCADE
) STRICT;
```

### `server_config` — singleton (OPAQUE server public key)
OPAQUE server long-term public key, published at `/auth/register/finish`.
Singleton via `id = 1` CHECK. (`opaque_record` on `users` is the *per-client*
record; this is the *server* keypair — distinct.)

```sql
CREATE TABLE IF NOT EXISTS server_config (
    id                        INTEGER PRIMARY KEY CHECK (id = 1),
    opaque_server_public_key  BLOB,
    created_at                INTEGER NOT NULL
) STRICT;
```

---

## 4. Indexing & write rules

- **OCC update (REQ-API-02 / ADR-004):**
  ```sql
  UPDATE items
     SET payload = :payload,
         version = version + 1,
         enc_key_gen = :enc_key_gen,
         updated_at = :updated_at
   WHERE uuid = :uuid
     AND user_id = :user_id
     AND version = :target_version;   -- rows_affected == 0  ⇒ 412
  ```
- **Epoch gate (REQ-API-01):** reject (or return 422) if `:enc_key_gen <
  (SELECT min_enc_key_gen FROM users WHERE id = :user_id)`. For batch push the
  whole batch is rejected with `422` if any item fails the gate.
- **Read-Only gate source:** `min_enc_key_gen` is read from `users` on every
  `/sync/pull`; if `client.local_gen < min_enc_key_gen` the client enters
  Read-Only (REQ-AUTH-05).
- **Rotation (ADR-006):** `POST /account/rotate-key` raises
  `users.min_enc_key_gen`; re-encrypted items are pushed with the new
  `enc_key_gen`. `enc_key_gen` on `items` is the resumable cursor.

---

## 5. Sync cursor & `cursor_expired` (410)

`GET /sync/pull?cursor=N` returns items with `version > N` for the user, plus
the new max version as the next cursor. For v1 the cursor is **the max item
version seen** (simple, correct for single-writer-per-device sync).

⚠ **Future:** long offline periods require history pruning → `410 cursor_expired`
(api.md §2). That needs a `sync_changelog(user_id, uuid, version, op, ts)` table
so old deltas can be aged out. This is intentionally **out of scope for the
initial migration** and tracked as a follow-up (see `docs/issues/`).

---

## 6. Migration management

- Migrations live in `core/vautr-server/migrations/NNNN_*.sql`.
- Applied at startup via `sqlx::migrate!("./migrations").run(&pool)`.
- `vautr-server::db::connect` opens the pool with the §2 PRAGMAs and runs all
  pending migrations. See `core/vautr-server/src/db.rs`.
- Server schema is **not** FTS5 (search is client-side only, db-contract §4).
