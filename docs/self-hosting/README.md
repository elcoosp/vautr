# Vautr — Self-Hosting Guide

Vautr is a zero-knowledge password & secrets manager: the server is an
**untrusted blob store** and never sees your decrypted data. This guide covers
running your own server with Docker Compose (recommended) or the
`scripts/install.sh` one-command installer.

- **Minimum hardware:** 1 vCPU, 1 GB RAM (handles sync + search for ~100 items
  comfortably; scale up for more).
- **Database:** SQLite is the default and needs no extra service. PostgreSQL is
  opt-in (requires the `postgres` cargo feature in the manifests first).

---

## Option A — Docker Compose (recommended)

The repo-root `docker-compose.yml` ships a working stack:

```bash
# 1. (optional) copy and edit env overrides
cp .env.example .env   # set VAUTR_PORT, VAUTR_DB_URL if desired

# 2. start the SQLite-backed server (no TLS)
docker compose up -d --build

# 3. verify
curl -fsS http://127.0.0.1:8080/account/status
```

The server listens on `${VAUTR_PORT:-8080}` and persists SQLite at
`/data/vautr.db` (the `serverdata` volume). Data survives container recreation
because the volume is mounted.

### HTTPS with Let's Encrypt (Caddy)

```bash
export VAUTR_DOMAIN=vault.example.com
docker compose --profile https up -d
```

Caddy (the `caddy` service, enabled by the `https` profile) obtains and renews
a certificate for `$VAUTR_DOMAIN` and reverse-proxies to the server. The
`deploy/Caddyfile` is used (`{$VAUTR_DOMAIN}` substitution).

### PostgreSQL (opt-in)

```bash
VAUTR_DB_URL='postgres://vautr:vautr@postgres:5432/vautr' \
VAUTR_FEATURES=postgres \
docker compose --profile postgres up -d --build
```

This starts the `postgres` service and builds a Postgres-enabled server image.
The `postgres` cargo feature must exist in the server manifest for this to work.

---

## Option B — `scripts/install.sh` (bare-metal / VM)

A portable POSIX installer that builds the binary, writes a config env file,
provisions a reverse proxy with automatic Let's Encrypt (Caddy by default,
nginx+certbot optional), installs a systemd/launchd unit, and smoke-tests.

```bash
# Quick local run, no proxy:
sudo ./scripts/install.sh --no-build --proxy none

# Production with HTTPS:
sudo ./scripts/install.sh --domain vault.example.com --email you@example.com
```

Flags: `--domain`, `--email`, `--port`, `--db-url`, `--data-dir`, `--user`,
`--proxy caddy|nginx|none`, `--no-build`, `--bin <path>`, `--no-root`, `--help`.

The installer writes config to `/etc/vautr/vautr.env` (root) or
`$HOME/.config/vautr` (non-root), mode `0600`.

---

## Production hardening (`docker-compose.prod.yml`)

`docker-compose.prod.yml` is a production variant with explicit healthchecks and
`restart: unless-stopped` on every service, plus a backup sidecar (litestream)
for continuous SQLite backups. Use it instead of the base file for real
deployments:

```bash
docker compose -f docker-compose.prod.yml --profile https up -d
```

---

## Backup & restore

SQLite is a single file — back it up atomically:

```bash
# backup (server stopped or via the sidecar)
docker exec vautr-server sqlite3 /data/vautr.db ".backup /backup/vautr.db.backup"

# restore to a fresh volume / VM, then start the server
docker compose down
docker volume rm vautr_serverdata
# mount / copy the .backup into /data/vautr.db, then:
docker compose up -d
```

With `docker-compose.prod.yml`, litestream continuously replicates
`/data/vautr.db` to an S3/object-store target you configure in the sidecar.

---

## Upgrade

The server auto-migrates its database on startup (sea-orm migrations run
against `VAUTR_DB_URL`). To upgrade:

```bash
docker compose pull          # or: docker compose up -d --build
docker compose up -d
```

No manual migration step is required; data is preserved across upgrades.

---

## Reverse proxy (own TLS terminator)

If you run the server behind your own Nginx/Caddy (or `--proxy none`), terminate
TLS and forward to `127.0.0.1:8080`:

```nginx
server {
    listen 443 ssl http2;
    server_name vault.example.com;
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

---

## Operations

- **Logs:** `docker compose logs -f server` (or `journalctl -u vautr-server -f`
  for the installer unit).
- **Config:** env vars in `vautr.env` or the compose `environment:` block.
  Key variables: `VAUTR_DB_URL` (DB location), `RUST_LOG` (log level, default
  `info`), `VAUTR_PORT` (bind port).
- **Reset:** stop the stack and remove the `serverdata` volume to wipe all data.

See `docs/issues/closed/VTR-070.md` for the full feature/architecture
reconciliation, and `README.md` for the project constitution (AGPL v3, forever
free to self-host).
