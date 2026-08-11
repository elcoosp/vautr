# Vautr Server Load Suite (k6)

Phase 7 hardening — exercises the server's Optimistic Concurrency Control (OCC)
and sync-pull behavior under concurrency
(`docs/architecture/roadmap.md` §9, `docs/architecture/server-scaling.md` §4).

The tests drive the HTTP API, so they are **database agnostic**: the same
invariants validate a **Postgres-backed** build (the operational target,
`build-env-deploy.md` §4.2 / `docker-compose.yml`) and the current
**SQLite-backed** build, because both enforce the identical atomic
`UPDATE ... WHERE version = ?` semantics behind the API.

## Files

| File                | What it exercises                                          |
| ------------------- | ---------------------------------------------------------- |
| `lib.js`            | Shared helpers (base URL, bearer headers, item discovery). |
| `occ-conflicts.js`  | Concurrent read→write on one item: write conflicts under concurrency, verifies conflict resolution returns correct versions. |
| `sync-pull.js`      | Concurrent `GET /sync/pull` (read-heavy pull under WAL).   |

## Prerequisites

1. A running Vautr server reachable at `VAUTR_BASE_URL` (default
   `http://localhost:8080`). For a local stack: `docker compose up -d` (see
   `docs/SELF-HOSTING.md`).
2. A seeded user with a valid **Bearer session token** → `VAUTR_TOKEN`.
   (The OPAQUE auth flow is intentionally not replayed inside k6; a seeded
   session token is provided out-of-band.)
3. **At least one seeded item** for that user, because `push-batch` performs an
   OCC *update* (it cannot create items — a write against a missing row always
   conflicts). Seed via SQL:

   ```sql
   -- SQLite (stock binary):
   INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at)
   VALUES ('load-item-1', '<user_id>', 1, 1, NULL, X'deadbeef', 1700000000000);

   -- Postgres:
   INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at)
   VALUES ('load-item-1', '<user_id>', 1, 1, NULL, decode('deadbeef','hex'), 1700000000000);
   ```

   Point `VAUTR_ITEM_UUID` at it (optional; defaults to the first item found).

> **Note on the stock image:** the bundled `vautr-server` binary is SQLite-backed
> (sqlx `sqlite` feature only, `server-scaling.md` §1/§4). Pointing it at the
> Postgres service requires a Postgres-enabled build (core is outside this
> suite's scope). The load invariants are identical for both backends.

## Install k6

```bash
# macOS
brew install k6
# Debian/Ubuntu
sudo gpg -k && curl -s https://dl.k6.io/key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/k6.gpg
echo "deb [signed-by=/usr/share/keyrings/k6.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update && sudo apt-get install k6
```

## Run

```bash
export VAUTR_BASE_URL=http://localhost:8080
export VAUTR_TOKEN='<seeded-session-token>'
export VAUTR_ITEM_UUID='load-item-1'

# OCC write-conflict suite (primary gate)
k6 run test/load/occ-conflicts.js

# Concurrent sync-pull suite
k6 run test/load/sync-pull.js
```

### Tuning

| Env var             | Default | Applies to       |
| ------------------- | ------- | ---------------- |
| `OCC_RATE`          | `50`    | occ-conflicts    | iterations/s |
| `OCC_DURATION`      | `20s`   | occ-conflicts    | test duration |
| `OCC_VUS`           | `25`    | occ-conflicts    | pre-allocated VUs |
| `OCC_MAX_VUS`       | `100`   | occ-conflicts    | max VUs |
| `PULL_RATE`         | `100`   | sync-pull        | iterations/s |
| `PULL_DURATION`     | `20s`   | sync-pull        | test duration |
| `PULL_VUS`          | `40`    | sync-pull        | pre-allocated VUs |
| `PULL_MAX_VUS`      | `150`   | sync-pull        | max VUs |

## What "correct conflict resolution" means (asserted)

For every `occ-conflicts.js` iteration the writer reads version `V`, then
pushes `target_version = V`. The server resolves it atomically, so every
response must report the **winning** version `V + 1`:

- a `success` result returns `version == V + 1`;
- a `conflict` result returns `current_server_state.version == V + 1`.

The suite fails (`checks rate must be 1.00`) if any response ever reports a
stale or otherwise incorrect version. This is the concrete, runnable form of
`docs/architecture/server-scaling.md` §4.2 (the `If-Match` / atomic
`UPDATE ... WHERE version = ?` contract).

## Validate syntax

```bash
k6 inspect test/load/occ-conflicts.js   # or
k6 run --vus 1 --iterations 1 --dry-run test/load/occ-conflicts.js
```
