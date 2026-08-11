# Vautr Self-Hosting Guide

This guide explains how to run the Vautr server (the untrusted, zero-knowledge
encrypted blob store) yourself. The server validates OPAQUE authentication and
version integrity (Optimistic Concurrency Control, OCC); it **never** sees the
Master Password, Master Key, SVK, or plaintext payloads
(`docs/architecture/server-scaling.md` §1).

There are two supported paths, both "one-click":

1. **One-command install script** (`scripts/install.sh`) — builds the server,
   provisions an HTTPS reverse proxy with automatic Let's Encrypt certificates
   (Caddy by default), installs a service-manager unit, and starts the server.
2. **Docker Compose** — `docker compose up -d --build` for the SQLite server,
   with opt-in profiles for Caddy HTTPS and PostgreSQL.

---

## 1. Topology

```
Browser / Mobile / Desktop client
        │  HTTPS (TLS 1.3 behind a reverse proxy)
        ▼
   vautr-server  (Axum, binds 0.0.0.0:8080)
        │
        ▼
   Database  (SQLite WAL by default; PostgreSQL is opt-in)
```

- **Server binary:** `core/vautr-server` → `vautr-server` (Axum).
- **Port:** `8080` (bind in `core/vautr-server/src/main.rs`).
- **Config:** read from environment variables (12-factor). None are compiled
  into the binary except the runtime defaults.
- **Database:** SQLite WAL by default; PostgreSQL is opt-in behind the
  `postgres` cargo feature (`core/vautr-server/src/db.rs`). See §"Database".

---

## 2. Quick start (recommended)

### 2a. One-command install (bare metal)

```bash
cd /path/to/vautr
./scripts/install.sh --domain vault.example.com --email admin@example.com
```

What it does (all idempotent):

- Builds `vautr-server` (`cargo build --release -p vautr-server`).
- Writes a config/env file (default `/etc/vautr/vautr.env` as root, else
  `$HOME/.config/vautr/vautr.env`) holding `VAUTR_DB_URL` (SQLite default) and
  `RUST_LOG`.
- Installs **Caddy** (or `--proxy nginx` for nginx+certbot) and provisions an
  **HTTPS** site for your domain with automatic **Let's Encrypt** certificates.
- Installs a **systemd** unit (Linux) or **launchd** agent (macOS) named
  `vautr-server` and starts it.

Common options:

| Flag | Purpose |
| ---- | ------- |
| `--domain <fqdn>` | Public hostname; required for automatic HTTPS. |
| `--email <addr>`  | Let's Encrypt account email. |
| `--port <port>`   | Local bind port (default `8080`). |
| `--db-url <url>`  | Database URL; default `sqlite:vautr.db`. |
| `--proxy caddy\|nginx\|none` | Reverse proxy / TLS provider (default `caddy`). |
| `--no-build --bin <path>` | Use a prebuilt binary instead of building. |
| `-h` / `--help`   | Full option reference. |

Use `--proxy none` for a local/LAN install with no TLS:

```bash
./scripts/install.sh --no-build --proxy none
```

Verify it is up:

```bash
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/account/status   # 401 = up
systemctl status vautr-server    # Linux
```

### 2b. Docker Compose (one click)

```bash
cd /path/to/vautr

# SQLite server, no TLS (works immediately)
docker compose up -d --build

# + HTTPS via Caddy (Let's Encrypt), requires VAUTR_DOMAIN
VAUTR_DOMAIN=vault.example.com docker compose --profile https up -d

# Follow logs / status
docker compose logs -f server
docker compose ps
```

The server is reachable at `http://localhost:8080` (or `https://vault.example.com`
with the `https` profile). Data is stored in the `serverdata` named volume
(SQLite file at `/data/vautr.db`).

---

## 3. Environment variables

| Variable | Default | Description |
| -------- | ------- | ----------- |
| `VAUTR_DB_URL` | `sqlite:vautr.db` | Database connection URL (see §"Database"). |
| `RUST_LOG` | (empty ⇒ `info`) | `tracing` level filter, e.g. `debug`, `vautr_server=info,info`. |
| `VAUTR_PORT` | `8080` | Compose-only: host port published to `8080`. |
| `VAUTR_DOMAIN` | — | Compose `https` profile: hostname for Caddy / Let's Encrypt. |
| `VAUTR_EMAIL` | — | Optional ACME account email (Compose Caddy). |
| `VAUTR_FEATURES` | — | Compose-only: extra cargo features for the server build (e.g. `postgres`). |
| `POSTGRES_USER` | `vautr` | Compose-only: Postgres role. |
| `POSTGRES_PASSWORD` | `vautr` | Compose-only: Postgres password. **Change in production.** |
| `POSTGRES_DB` | `vautr` | Compose-only: database name. |

**Server URL injection (build-env-deploy.md §5):** clients inject server URLs at
*build time* via `--cfg`/env. The server itself reads `VAUTR_DB_URL` from the
environment; the Web/Extension clients resolve their API URL at runtime via
`window.__VAUTR_CONFIG__` / `chrome.storage.local`.

---

## 4. Reverse proxy / HTTPS (Let's Encrypt)

Terminate TLS in front of the server with a strict reverse proxy
(`server-scaling.md` §3). Both install paths automate this:

- **`scripts/install.sh`** installs **Caddy** by default and writes a Caddyfile
  for your domain. Caddy obtains and auto-renews Let's Encrypt certificates.
  `--proxy nginx` instead installs nginx + certbot with a systemd renewal timer.
- **Docker Compose `https` profile** runs `caddy:2` using `deploy/Caddyfile`,
  which proxies `https://$VAUTR_DOMAIN` → `server:8080` and handles ACME.

Recommended proxy hardening (already applied by the provided Caddyfile):

- **TLS 1.3 only.**
- **Request size limits:** `/sync/*`, `/auth/*` → 10 MB; file-upload initiate →
  1 MB.
- **Body logging blackout:** never log request bodies or response payloads
  (no-plaintext rule).
- **PII stripping:** strip `X-User-Email`-style headers at the proxy.

Example manual Caddyfile (if you manage Caddy yourself):

```caddyfile
vautr.example.com {
    tls { protocols tls1.3 }
    header_up -X-User-Email
    reverse_proxy 127.0.0.1:8080
}
```

---

## 5. Database

### SQLite (default)

- **URL:** `sqlite:<path>` (or `sqlite::memory:` for tests).
- The pool is opened in **WAL mode** with `synchronous=NORMAL`,
  `busy_timeout=5000`, and foreign keys enabled (`core/vautr-server/src/db.rs`).
- A background task periodically runs `PRAGMA wal_checkpoint(TRUNCATE)` to bound
  WAL growth; a `wal_checkpoint(TRUNCATE)` also runs on graceful shutdown.

### PostgreSQL (opt-in)

PostgreSQL support is real but **opt-in** — SQLite stays the default. It is
compiled behind the `postgres` cargo feature of `vautr-server`
(`core/vautr-server/src/db.rs` adds a `postgres::connect_pg` path and a
`connect_any` dispatcher keyed on the `VAUTR_DB_URL` scheme).

**Prerequisite (coordinator / manifest):** two manifest changes are required and
are **not** owned by this workstream (see the install section of
`docs/architecture/build-env-deploy.md` and the report):

1. Enable the sqlx **`postgres`** runtime feature in the workspace
   `[workspace.dependencies] sqlx` features (root `Cargo.toml`).
2. Declare the **`postgres`** feature on the `vautr-server` crate
   (`core/vautr-server/Cargo.toml`).

**Once enabled**, point `VAUTR_DB_URL` at a `postgres://` URL:

```bash
VAUTR_DB_URL=postgres://vautr:vautr@localhost:5432/vautr \
RUST_LOG=info \
./target/release/vautr-server
```

**Docker Compose** (after the features exist):

```bash
VAUTR_DB_URL=postgres://vautr:vautr@postgres:5432/vautr \
VAUTR_FEATURES=postgres \
docker compose --profile postgres up -d --build
```

> **Migrations caveat:** `sqlx::migrate!` applies the *same* `migrations/*.sql`
> files on either backend, and those files are currently **SQLite-specific DDL**
> (`STRICT`, `BLOB`, …). The migrations directory is owned by another workstream.
> A Postgres deployment therefore needs a set of **Postgres-compatible
> migrations** in addition to enabling the feature. Until those exist, run
> Postgres only against a schema created from Postgres migrations.

The OCC contract is enforced identically on both backends: every write is an
atomic `UPDATE ... WHERE version = ?` that returns a conflict (`412` /
`conflict` result with the current server version) if the version moved
(`server-scaling.md` §4.2).

---

## 6. Smart backups

The server stores only opaque ciphertext blobs, so a database leak exposes no
plaintext; still encrypt volumes at rest and keep backups **off-host**.

### SQLite

- Keep the `.db` **and** the `-wal`/`-shm` files together when snapshotting.
- Prefer a consistent online backup (works with WAL):

  ```bash
  sqlite3 /var/lib/vautr/vautr.db ".backup /backup/vautr-$(date +%F).db"
  ```

- Or run `PRAGMA wal_checkpoint(TRUNCATE)` (the server does this on shutdown)
  before copying files.

### PostgreSQL

- Use `pg_dump` (logical) for portability, or `pg_basebackup`/WAL archival
  (physical) for point-in-time recovery.

  ```bash
  docker compose exec postgres pg_dump -U vautr vautr | gzip > /backup/vautr-$(date +%F).sql.gz
  ```

- Enable continuous WAL archiving to object storage if you need PITR; retain
  backups off-host.

### Restore

1. Stop the server (`systemctl stop vautr-server` or `docker compose stop server`).
2. Replace the DB files (SQLite) or restore the Postgres cluster.
3. Start the server. Migrations are idempotent-safe (they run on a fresh DB; on
   an existing DB the schema is already applied).

---

## 7. Updating (no-downtime)

A helper `scripts/update.sh` handles both deployment styles:

```bash
# Bare metal (systemd/launchd): build + atomic swap + graceful restart
./scripts/update.sh

# Docker Compose: build a new image and recreate the server container
./scripts/update.sh --compose
```

- **Bare metal:** the server's graceful shutdown (`main.rs`) stops accepting new
  requests, drains in-flight requests within 30s, flushes the WAL, and exits;
  `systemctl restart` then brings the new binary up. The swap is atomic (build
  to a temp path, then `mv`), so a crash mid-update never leaves a partial
  binary.
- **Compose:** `docker compose up -d --build` starts the new container before
  stopping the old one where possible.

If you manage the service yourself, a no-downtime update is: build the new
binary → `systemctl restart vautr-server` (or `docker compose up -d --build`).

---

## 8. Verification

- **Health / smoke:** `curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/account/status`
  → an unauthenticated `401` confirms the server is up (auth-gated endpoint).
- **Container healthcheck:** the `Dockerfile` and `docker-compose.yml` healthcheck
  the same endpoint.
- **Isolation guardrail:** `.github/workflows/verify-isolation.yml` hard-fails if
  `read_secret` or GPUI markers ever leak into a non-desktop artifact, and runs
  `scripts/check-restricted-api.sh`.
- **Load / OCC:** `test/load/` (k6) — concurrent sync pulls and write conflicts.

---

## 9. Observability

The client emits **anonymous, 24-hour-aggregated** telemetry
(`core/vautr-telemetry`, `docs/architecture/telemetry.md` §4) and a **Daily
Heartbeat** once per 24 hours containing only an `InstallationUuid`, OS version,
and the aggregated metrics buffer (no per-event data, no user IDs, no PII). The
**Safety Reaper** alert (`telemetry.md` §5.2) triggers a P2 alert when
`reaper_orphans_cleaned` exceeds 10 in a 24-hour window.

For **server-down alerting**, monitor the container/service liveness (the
healthcheck endpoint above) with your favourite uptime probe, and alert on the
`systemctl status vautr-server` / compose health state.
