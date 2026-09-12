# syntax=docker/dockerfile:1

# =============================================================================
# Vautr server image (Axum + SQLite/Postgres OCC blob store).
# See docs/architecture/build-env-deploy.md §4.2/§5 and
# docs/architecture/server-scaling.md §4. Two-stage build: compile on a Rust
# toolchain, run on a minimal debian-slim runtime.
#
# The repo's rust-toolchain.toml pins nightly-2026-08-10; rustup in the builder
# image auto-installs it when cargo reads the toolchain file.
#
# PostgreSQL: the bundled binary is SQLite-backed by default. To build a
# Postgres-enabled image (once the sqlx `postgres` feature + the vautr-server
# `postgres` feature exist in the manifests — see docs/SELF-HOSTING.md §"Database"):
#
#   docker build --build-arg FEATURES=postgres --build-arg VAUTR_DB_URL=postgres://... .
# =============================================================================

# ---- Build stage ----------------------------------------------------------
FROM rust:1.98-bookworm AS builder
WORKDIR /build

# Copy the whole source context so the Cargo workspace resolves all members.
# .dockerignore keeps target/, node_modules/, .git, DB files and docs out.
COPY . .

# Cache dependencies first: build once, rely on layer caching for the rest.
RUN cargo fetch

# Compile only the server binary. `sqlx::migrate!` embeds the SQL migrations at
# compile time, so no separate schema step is needed at runtime.
# Optional extra cargo features (e.g. `postgres`) come from --build-arg FEATURES.
ARG FEATURES=""
RUN if [ -n "$FEATURES" ]; then \
      cargo build --release -p vautr-server --features "$FEATURES"; \
    else \
      cargo build --release -p vautr-server; \
    fi

# ---- Runtime stage --------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root runtime user (uid/gid 1000) — drop privileges for the server.
RUN groupadd -r -g 1000 vautr && useradd -r -g vautr -u 1000 -d /data -s /usr/sbin/nologin vautr

COPY --from=builder /build/target/release/vautr-server /usr/local/bin/vautr-server

# Server configuration (build-env-deploy.md §5). The DB URL may be injected at
# build time (`--build-arg VAUTR_DB_URL=...`) or overridden at runtime
# (`-e VAUTR_DB_URL=...` / compose). Defaults to a SQLite file under /data.
ARG VAUTR_DB_URL=sqlite:vautr.db
ENV VAUTR_DB_URL=$VAUTR_DB_URL
ENV RUST_LOG=info

# The server binds 0.0.0.0:8080 (core/vautr-server/src/main.rs).
EXPOSE 8080

# Persistent data (SQLite db + WAL files when VAUTR_DB_URL is sqlite:...).
# Postgres users keep this volume for /data but store rows in their Postgres.
VOLUME ["/data"]
WORKDIR /data
# The named volume is mounted as root-owned; the non-root `vautr` runtime user
# must own /data to create the SQLite db + WAL files (fixes SQLite CANTOPEN
# panics at startup).
RUN chown -R vautr:vautr /data

# Health check: /account/status is auth-gated and returns HTTP 401 when the
# server is up (curl succeeds on any HTTP response); a connection refusal
# (server down) fails the check.
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS -o /dev/null http://127.0.0.1:8080/account/status || exit 1

USER vautr
ENTRYPOINT ["vautr-server"]
