# Vautr Build System, Environment & Deployment Specification (v3.0 - The Final Form)

This document defines the exact monorepo topology, build matrix, FFI generation pipeline, and CI/CD contracts for the complete Vautr ecosystem. 

Vautr is a polyglot, cross-platform distributed system. If the build system is not architected as rigorously as the cryptography, FFI boundary violations will leak secrets, and merge conflicts will destroy parallel development velocity. This specification enforces strict workspace isolation to maximize code parallelism, mandates compile-time feature gates to guarantee Zero-Knowledge boundaries, and explicitly accounts for the hostile, ephemeral lifecycle of modern browser extensions.

Deviations from this specification will result in broken CI pipelines, exposed `read_secret` APIs in JavaScript, and unreproducible release artifacts.

---

## 1. Monorepo Workspace Topology

Vautr uses a root-level workspace managed by Turborepo (for Node/React) and a nested Cargo Workspace (for Rust). The Server and Client live in the same monorepo to ensure API contracts never drift. Separate lockfiles ensure frontend, server, and core Rust engineers never block each other.

```text
vautr/ (Root Turborepo)
├── apps/
│   ├── desktop/           # GPUI (Rust) - Direct Core consumer
│   ├── mobile/            # React Native (TS/JS/Swift/Kotlin)
│   ├── web/               # React (TS/JS) + WASM Worker (Hosted SPA)
│   ├── extension/         # Browser Extension (Manifest V3)
│   └── server/            # Vautr Server (Rust - Axum) - The ZK Blob Store
│
├── packages/              # Shared Frontend Logic (Turborepo managed)
│   ├── ui-components/     # Shared React Native / Web primitives
│   ├── ts-config/         # Shared TypeScript configs
│   ├── api-contract/      # Shared Zod schemas & Types generated from OpenAPI spec
│   └── vautr-client-sdk/  # The TS Promise-based SDK wrapping the Turbo Module/WASM Worker
│
└── core/                  # Rust Cargo Workspace
    ├── Cargo.toml         # Workspace root
    ├── vautr-crypto/      # PURE MATH. No IO, no FFI.
    ├── vautr-domain/      # PURE DATA. Shared cryptographic & data payloads ONLY.
    ├── vautr-db/          # SeaORM 2.0, SQLite WAL, Persistence Worker (Client only).
    ├── vautr-auth/        # OPAQUE state machine.
    ├── vautr-sync/        # Sync Engine, OCC, DashMap logic (Client only).
    ├── vautr-keyring/     # Key lifecycle, SVK rotation (Client only).
    ├── vautr-app-state/   # The client orchestrator. Exposes VaultrClient API.
    ├── vautr-ffi/         # UniFFI bindings (Mobile)
    ├── vautr-wasm/        # wasm-bindgen bindings (Web/Extension Popup)
    └── vautr-server/      # Axum HTTP server, Postgres WAL, OCC enforcement.
```

### The `vautr-domain` Isolation Contract
To prevent boundary pollution, `vautr-domain` is strictly restricted to pure, shared cryptographic and data payloads (e.g., `DecryptedOverview`, `DecryptedSecret`, `EncryptedBlob`). Client-specific state wrappers (like `SaveCommand { item, sync_epoch }` or `DashMapState`) are forbidden in `vautr-domain` and must reside in `vautr-app-state`. Server-specific wrappers must reside in `vautr-server`.

---

## 2. The Feature Flag Matrix (Compile-Time Security)

The most critical security boundary in Vautr is enforced at the compiler level. If `read_secret` compiles into the Web or Extension bundle, the Zero-Knowledge guarantee is void.

| Feature Flag | Desktop (GPUI) | Mobile (UniFFI) | Web (WASM) | Extension (WASM) | Test/CI | Server |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `desktop-api` | **Enabled** | Disabled | Disabled | Disabled | Disabled | Disabled |
| `test-instrumentation`| Disabled | Disabled | Disabled | Disabled | **Enabled** | Disabled |
| `entity-registry` | Disabled | Disabled | Disabled | Disabled | **Enabled** | Disabled |

**CI Enforcement:** A mandatory CI script diffs the compiled WASM and UniFFI symbols. If `read_secret` is found in Web, Extension, or Mobile artifacts, the pipeline hard-fails.

---

## 3. FFI Generation & Extension Pipeline

The bridge between Rust and the UI layer must be fully automated and deterministic.

### 3.1 Mobile Pipeline (React Native + UniFFI)
*   **Build Steps:**
    1.  Cross-compile via `cargo-ndk` for `arm64-v8a` / `x86_64` (Android) and `aarch64` / `x86_64-apple-ios` (iOS).
    2.  Generate Swift/Kotlin headers via `uniffi-bindgen`.
    3.  Inject generated headers into Turbo Module wrappers.
*   **Automation:** Fastlane orchestrates the build and injects the compiled `.so`/`.dylib` into XCode/Gradle.

### 3.2 Web App Pipeline (React + WASM)
*   **Build Steps:**
    1.  `wasm-pack build ./core/vautr-wasm --target web --out-dir ../../apps/web/wasm-pkg`
*   **Lifecycle:** The Web App instantiates a persistent Web Worker running the `VaultrClient`. The Worker holds the DashMap and SQLite connection for the duration of the tab session.

### 3.3 Browser Extension Pipeline (Manifest V3 - The Stateless Boundary)
Manifest V3 Service Workers are ephemeral; the browser will kill them after ~30 seconds of inactivity. Holding a `VaultrClient` (with its in-memory DashMap and open SQLite WAL) in a Service Worker will cause corrupted databases and lost state on termination.

*   **Architecture Split:**
    1.  **Extension Popup:** Uses the standard `--target web` WASM build. When the user interacts with the popup, it instantiates a full `VaultrClient` (just like the Web App) in a popup-scoped Worker. When the popup closes, the client gracefully shuts down.
    2.  **Extension Service Worker (Autofill/Copy):** Must be 100% stateless. It wakes on OS events, executes a single action, and terminates. It *cannot* use SQLite.
*   **The Stateless Autofill Flow:**
    1.  When the user unlocks the vault in the Popup, the Popup caches the raw SVK in `chrome.storage.session` (which is encrypted by the browser and survives SW restarts, but is wiped on browser close).
    2.  The SW wakes on an `autofill` request.
    3.  The SW reads the SVK from `chrome.storage.session`.
    4.  The SW fetches the specific item's ciphertext from `chrome.storage.local`.
    5.  The SW uses a lightweight, stateless `vautr-crypto` WASM build (`--target nodejs` for SW compatibility) to decrypt the secret directly.
    6.  The SW fulfills the autofill request and terminates.
*   **Build Step for SW:** `wasm-pack build ./core/vautr-crypto --target nodejs --out-dir ../../apps/extension/sw-wasm-pkg`

---

## 4. Server Build & API Contract Pipeline

The Server is a first-class citizen in the monorepo.

### 4.1 API Contract Generation
*   The Server's OpenAPI 3.1 spec is auto-generated from the Axum routes using `utoipa`.
*   On CI, the spec is exported to `packages/api-contract/openapi.json`.
*   `openapi-typescript` runs on this spec to generate Zod schemas and TS types for the `vautr-client-sdk`. This guarantees the Client TS SDK and the Rust Server are always perfectly in sync.

### 4.2 Server Infrastructure
*   **Database:** SQLite (WAL mode) is the **default** for all environments
    (zero-config). **PostgreSQL is opt-in** behind the `postgres` cargo feature
    of `vautr-server` (see §7 "Deployment Operations" and `docs/SELF-HOSTING.md`
    §"Database"). See server-scaling.md for justification and tuning.
*   **Deployment:** Docker container deployed to AWS ECS / Google Cloud Run, or
    a one-command bare-metal install (`scripts/install.sh`). See §7.
*   **Local Dev:** `docker compose up` spins up the Axum server (SQLite by
    default; use `--profile postgres` for a local Postgres DB).

---

## 5. Environment & Configuration Injection

The Core must remain pure. Configuration is injected at the application boundary.

### 5.1 Server Endpoints
*   **Desktop/Mobile:** Server URLs are injected at *build time* via `--cfg` flags or environment variables baked into the binaries.
*   **Web/Extension:** The API URL is injected at *runtime* via `window.__VAUTR_CONFIG__` (Web) or `chrome.storage.local` (Extension) to allow dynamic environment steering without rebuilding the WASM bundle.
*   **Server:** Reads DB connection strings and OPAQUE server keys from environment variables (Standard 12-Factor).

### 5.2 KDF Calibration (Runtime)
The Argon2id parameters are *not* hardcoded. The client `vautr-app-state` runs a calibration benchmark on first launch and passes the optimal parameters down to `vautr-crypto::derive_mk`.

---

## 6. CI/CD Pipeline (The Automation Contract)

The pipeline enforces the Testing & Verification Strategy using Biome for all frontend linting/formatting, ensuring zero-config, blazing-fast static analysis.

### 6.1 PR Gate (Fast Feedback - < 10 mins)
Runs on every Pull Request. Blocks merging.
*   **Lint & Format:** `cargo fmt --check`, `cargo clippy --all-targets`, `npx biome check ./apps ./packages`.
*   **Rust Unit Tests:** `cargo test --workspace --features test-instrumentation`
*   **Loom Concurrency:** `cargo test -p vautr-app-state --features loom`
*   **Frontend Typecheck:** `tsc --noEmit` across `apps/` and `packages/`.
*   **FFI Symbol Check:** Verify `read_secret` is absent from WASM/Mobile targets.
*   **API Contract Drift:** Verify `packages/api-contract/openapi.json` matches the current Axum routes. If a server engineer changes an endpoint, they must also commit the updated TS types.

### 6.2 Nightly Build (Exhaustive Verification - ~2 hours)
Runs on schedule against `main`.
*   **Crypto Fuzzing:** `cargo fuzz run decrypt_aead -- -max_total_time=3600`
*   **Structural Stress:** `cargo test --workspace --features stress_test`
*   **Memory Leak Profile:** Run instrumented desktop binary; assert `Zeroizing` drop counter == 0.
*   **E2E UI (Mobile):** Maestro suite against physical TestFlight/Firebase build.
*   **E2E UI (Web):** Playwright suite against local WASM build.
*   **E2E UI (Extension):** Playwright suite using the Chrome extension testing API, specifically testing SW termination/restart cycles during autofill.
*   **Server Load Test:** Run `k6` load testing suite against staging to verify Postgres OCC behavior under extreme concurrency.

### 6.3 Release Pipeline (Artifact Generation)
Triggered by semantic versioning tags (e.g., `v1.2.0`).
*   **Desktop:** Cross-compile via `cargo-bundle`. Notarize Mac app. Sign Windows executable.
*   **Mobile:** Build AAB/IPA via Fastlane. Push to Google Play Internal / Apple TestFlight.
*   **Web:** Optimize WASM (`wasm-opt -Oz`), minify React, deploy to Vercel/Cloudflare.
*   **Extension:** Bundle Popup (Web WASM) and SW (Node WASM), zip, and publish to Chrome Web Store / Firefox Add-ons.
*   **Server:** Build Docker image, push to ECR, deploy to ECS/Cloud Run via Helm/ArgoCD.

---

## 7. Deployment Operations (Wave Ops)

This section describes the one-command install, container improvements, the
opt-in PostgreSQL path, and no-downtime update procedures. It complements
`docs/SELF-HOSTING.md`.

### 7.1 One-command install (`scripts/install.sh`)

`scripts/install.sh` is the ultra-simple install path (MLP scope §1):

```bash
./scripts/install.sh --domain vault.example.com --email admin@example.com
```

It is idempotent and performs, in order:

1. **Build** — `cargo build --release -p vautr-server` (or `--no-build --bin <path>`
   for a prebuilt binary).
2. **Config** — writes `VAUTR_DB_URL` / `RUST_LOG` into `/etc/vautr/vautr.env`
   (root) or `$HOME/.config/vautr/vautr.env` (non-root), chmod 0600.
3. **HTTPS reverse proxy + Let's Encrypt** — installs **Caddy** (default) and
   writes a Caddyfile for the given domain; Caddy auto-issues and auto-renews
   certificates. `--proxy nginx` instead installs nginx + certbot with a systemd
   renewal timer. `--proxy none` skips TLS.
4. **Service unit** — installs a `vautr-server` systemd unit (Linux) or a
   `org.vautr.server` launchd agent (macOS) and starts it.
5. **Smoke test** — polls `http://127.0.0.1:<port>/account/status` (auth-gated,
   `401` = up).

SQLite is the default DB (`--db-url sqlite:vautr.db`); PostgreSQL is opt-in via
`--db-url postgres://...` (see §7.3).

### 7.2 Container improvements (Dockerfile / docker-compose.yml)

- **Dockerfile** — two-stage build on the pinned nightly toolchain (rustup in
  the builder auto-installs `nightly-2026-08-10` from `rust-toolchain.toml`),
  a non-root `vautr` runtime user, an HTTP healthcheck on `/account/status`,
  a persistent `/data` volume, and a `--build-arg FEATURES` passthrough for
  optional cargo features (e.g. `postgres`). A `.dockerignore` keeps the build
  context lean and free of secrets/local state.
- **docker-compose.yml** — SQLite **default** (one-click, no DB service needed).
  Opt-in profiles:
  - `--profile https` adds a `caddy` service (using `deploy/Caddyfile`) for
    HTTPS + Let's Encrypt behind `VAUTR_DOMAIN`.
  - `--profile postgres` adds a `postgres:16` service.
  Persistent named volumes (`serverdata`, `pgdata`, `caddydata`), restart
  policies, and service healthchecks are configured.

```bash
docker compose up -d --build                                   # SQLite, no TLS
VAUTR_DOMAIN=... docker compose --profile https up -d          # + HTTPS
VAUTR_DB_URL=postgres://vautr:vautr@postgres:5432/vautr \
VAUTR_FEATURES=postgres \
docker compose --profile postgres up -d --build                # + Postgres
```

### 7.3 PostgreSQL (opt-in)

PostgreSQL support is real but **opt-in**; SQLite remains the default.
`core/vautr-server/src/db.rs` adds:

- `db::postgres::connect_pg(url)` — opens a `PgPool` and runs migrations
  (`sqlx::postgres::{PgConnectOptions, PgPoolOptions}`).
- `db::postgres::ensure_schema_pg(pool)` — migration check helper.
- `db::connect_any(url)` / `db::AnyPool` — a scheme-keyed dispatcher
  (`sqlite:` → SQLite, `postgres://` → Postgres) that errors clearly when
  Postgres is requested but not compiled in.

**Required manifest changes (owned by the coordinator — this workstream must
not edit `Cargo.toml`):**

1. **Root `Cargo.toml`** — add the sqlx **`postgres`** runtime feature:
   `sqlx = { version = "0.9", features = ["sqlite", "postgres", "runtime-tokio", "tls-rustls", "chrono", "migrate", "macros"] }`.
2. **`core/vautr-server/Cargo.toml`** — declare and enable the **`postgres`**
   feature: `postgres = ["sqlx/postgres"]` (add to `[features]`).

**Migrations caveat:** `sqlx::migrate!` applies the same `migrations/*.sql` on
either backend; those files are currently SQLite DDL (`STRICT`, `BLOB`, …). A
Postgres deployment additionally requires **Postgres-compatible migrations**
(migrations are owned by another workstream). Until those exist, run Postgres
only against a schema created from Postgres migrations.

### 7.4 Backups

- **SQLite** — `sqlite3 vautr.db ".backup /backup/vautr-$(date +%F).db"` (WAL-safe
  online backup), or checkpoint + copy the `.db`/`-wal`/`-shm` together.
- **PostgreSQL** — `pg_dump` (logical) or `pg_basebackup`/WAL archival (physical,
  PITR). Keep backups off-host and encrypted at rest.

### 7.5 No-downtime updates (`scripts/update.sh`)

```bash
./scripts/update.sh              # bare metal: build + atomic swap + graceful restart
./scripts/update.sh --compose    # compose: build new image, recreate server
```

Bare metal builds to a temp path and atomically `mv`s the new binary; the server's
graceful shutdown (stop accepting, drain in-flight ≤ 30s, flush WAL, exit) makes
`systemctl restart` a short blip. Compose recreates the container with the new
image, starting it before stopping the old one where possible.
