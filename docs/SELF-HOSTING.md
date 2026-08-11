# Vautr Self-Hosting Guide

This guide explains how to run the Vautr server (the untrusted, zero-knowledge
encrypted blob store) yourself. The server validates OPAQUE authentication and
version integrity (Optimistic Concurrency Control, OCC); it **never** sees the
Master Password, Master Key, SVK, or plaintext payloads
(`docs/architecture/server-scaling.md` §1).

---

## 1. Topology

```
Browser / Mobile / Desktop client
        │  HTTPS (TLS 1.3 behind a reverse proxy)
        ▼
   vautr-server  (Axum, binds 0.0.0.0:8080)
        │
        ▼
   Database  (SQLite WAL by default; Postgres is the production target)
```

- **Server binary:** `core/vautr-server` → `vautr-server` (Axum).
- **Port:** `8080` (hardcoded bind in `main.rs`).
- **Config:** read from environment variables (12-factor). None are compiled
  into the binary except the runtime defaults.
- **Database:** the stock binary is SQLite-backed (`sqlx` `sqlite` feature only,
  `server-scaling.md` §1/§4). The production/compose target is Postgres
  (`build-env-deploy.md` §4.2). See §"Database" below.

---

## 2. Quick start with Docker (recommended)

The repo ships a `Dockerfile` (multi-stage Rust build) and a `docker-compose.yml`
that provisions a **Postgres** service plus the **server**.

```bash
cd /path/to/vautr

# Build + start both services in the background
docker compose up -d --build

# Follow server logs
docker compose logs -f server

# Confirm it is up
docker compose ps
```

The server is reachable at `http://localhost:8080`.

> **Default database note:** the bundled image runs on a SQLite file mounted at
> `/data` (`VAUTR_DB_URL=sqlite:/data/vautr.db`) so `docker compose up` works
> immediately. To target the bundled **Postgres** service you must use a
> Postgres-enabled server build (see §"Database"). The `postgres` service and
> the load suite are both designed for that target.

---

## 3. Environment variables

| Variable          | Default             | Description                                                        |
| ----------------- | ------------------- | ------------------------------------------------------------------ |
| `VAUTR_DB_URL`    | `sqlite:vautr.db`   | Database connection URL (see §"Database"). Injected at build time via `--build-arg` or overridden at runtime. |
| `RUST_LOG`        | (empty ⇒ `info`)    | `tracing` level filter, e.g. `debug`, `vautr_server=info,info`.    |
| `POSTGRES_USER`   | `vautr`             | Compose-only: Postgres role.                                       |
| `POSTGRES_PASSWORD`| `vautr`            | Compose-only: Postgres password. **Change in production.**         |
| `POSTGRES_DB`     | `vautr`             | Compose-only: database name.                                       |
| `VAUTR_PORT`      | `8080`              | Compose-only: host port published to `8080`.                       |

**Server URL injection (build-env-deploy.md §5):** clients inject server URLs at
*build time* via `--cfg`/env (`build-env-deploy.md` §5.1). The server itself
reads `VAUTR_DB_URL` from the environment; the Web/Extension clients resolve
their API URL at runtime via `window.__VAUTR_CONFIG__` / `chrome.storage.local`.

---

## 4. Run without Docker (raw binary)

```bash
# Build
cargo build --release -p vautr-server

# Run (SQLite default)
VAUTR_DB_URL=sqlite:vautr.db \
RUST_LOG=info \
./target/release/vautr-server

# Run with an explicit data directory (SQLite)
mkdir -p /var/lib/vautr
VAUTR_DB_URL=sqlite:/var/lib/vautr/vautr.db ./target/release/vautr-server
```

The server runs migrations automatically at startup
(`sqlx::migrate!` embeds the `migrations/*.sql` schema).

---

## 5. Database

### SQLite (stock binary)

- **URL:** `sqlite:<path>` (or `sqlite::memory:` for tests).
- The pool is opened in **WAL mode** with `synchronous=NORMAL`,
  `busy_timeout=5000`, and foreign keys enabled (`core/vautr-server/src/db.rs`).
- A background task periodically runs `PRAGMA wal_checkpoint(TRUNCATE)` to bound
  WAL growth (`server-scaling.md` §4.4); a `wal_checkpoint(TRUNCATE)` also runs
  on graceful shutdown.

### Postgres (production target)

The `docker-compose.yml` provides a `postgres:16` service as the production
database. To use it you need a **Postgres-enabled server build** (the core sqlx
feature wiring is outside the ownership of this guide; see `server-scaling.md`
§1/§4 and `build-env-deploy.md` §4.2 for the intended topology). Once available:

```bash
VAUTR_DB_URL=postgres://vautr:vautr@postgres:5432/vautr docker compose up -d
```

or bake it in at build time:

```bash
docker build \
  --build-arg VAUTR_DB_URL=postgres://vautr:vautr@postgres:5432/vautr \
  -t vautr/server .
```

The OCC contract is enforced identically on both backends: every write is an
atomic `UPDATE ... WHERE version = ?` that returns a conflict (`412` /
`conflict` result with the current server version) if the version moved
(`server-scaling.md` §4.2). The k6 suite in `test/load/` validates this
behavior under concurrency for either backend (see `test/load/README.md`).

---

## 6. WAL & backup notes

- **SQLite WAL:** keep the `.db` **and** the `-wal`/`-shm` files together when
  snapshotting. Do **not** snapshot a live WAL without a checkpoint. Prefer
  `sqlite3 vautr.db ".backup /backup/vautr-$(date +%F).db"` for consistent
  online backups (works with WAL), or run `PRAGMA wal_checkpoint(TRUNCATE)`
  (the server does this on shutdown) before copying files.
- **Postgres:** use `pg_dump` (logical) for cross-version portability, or
  `pg_basebackup`/WAL archival (physical) for point-in-time recovery. Enable
  continuous WAL archiving to object storage if you need PITR; retain backups
  off-host.
- **Restore:** stop the server, replace the DB files (SQLite) or restore the
  Postgres cluster, then start the server. Migrations are idempotent-safe
  (they run on a fresh DB; on an existing DB the schema is already applied).
- **Encryption at rest:** the server stores only opaque ciphertext blobs, so a
  database leak exposes no plaintext, but you should still encrypt volumes /
  buckets at rest and restrict access. Backups inherit the same
  ciphertext-only property.

---

## 7. Reverse proxy / TLS (recommended for production)

Terminate TLS in front of the server with a strict reverse proxy
(`server-scaling.md` §3):

- **TLS 1.3 only.**
- **Request size limits:** `/sync/*`, `/auth/*` → 10 MB; file-upload initiate →
  1 MB.
- **Body logging blackout:** never log request bodies or response payloads
  (no-plaintext rule).
- **PII stripping:** strip `X-User-Email`-style headers at the proxy.

Example Caddy:

```caddyfile
vautr.example.com {
    tls {
        protocols tls1.3
    }
    reverse_proxy vautr-server:8080
}
```

---

## 8. Verification

- **Health / smoke:** `curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/account/status`
  → an unauthenticated `401` confirms the server is up (auth-gated endpoint).
- **Isolation guardrail:** the CI workflow `.github/workflows/verify-isolation.yml`
  hard-fails if `read_secret` or GPUI markers ever leak into a non-desktop
  artifact, and runs `scripts/check-restricted-api.sh`.
- **Load / OCC:** `test/load/` (k6) — concurrent sync pulls and write
  conflicts, asserting conflict resolution returns correct versions
  (`test/load/README.md`).

---

## 9. Observability

The client emits **anonymous, 24-hour-aggregated** telemetry
(`core/vautr-telemetry`, `docs/architecture/telemetry.md` §4) and a **Daily
Heartbeat** once per 24 hours containing only an `InstallationUuid`, OS version,
and the aggregated metrics buffer (no per-event data, no user IDs, no PII). The
**Safety Reaper** alert (`telemetry.md` §5.2) triggers a P2 alert when
`reaper_orphans_cleaned` exceeds 10 in a 24-hour window — an early signal that a
UI component is failing to call `release_secret`; the offending `app_version`
should be rolled back.
