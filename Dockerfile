# syntax=docker/dockerfile:1

# =============================================================================
# Vautr server image (Axum + SQLite/Postgres OCC blob store).
# See docs/architecture/build-env-deploy.md §4.2/§5 and
# docs/architecture/server-scaling.md §4. Two-stage build: compile with
# rust:1.94 (matching rust-toolchain.toml), run on a minimal debian-slim
# runtime.
# =============================================================================

# ---- Build stage ----------------------------------------------------------
FROM rust:1.94-bookworm AS builder
WORKDIR /build

# Cache dependencies: copy the workspace manifest + lockfile + all core crates.
COPY Cargo.toml Cargo.lock ./
COPY core ./core

# Compile only the server binary. `sqlx::migrate!` embeds the SQL migrations at
# compile time, so no separate schema step is needed at runtime.
RUN cargo build --release -p vautr-server

# ---- Runtime stage --------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/vautr-server /usr/local/bin/vautr-server

# Server configuration (build-env-deploy.md §5). The DB URL may be injected at
# build time (`--build-arg VAUTR_DB_URL=...`) or overridden at runtime
# (`-e VAUTR_DB_URL=...` / compose). Defaults to a SQLite file under /data.
# For a Postgres-backed build:
#   VAUTR_DB_URL=postgres://vautr:vautr@postgres:5432/vautr
ARG VAUTR_DB_URL=sqlite:vautr.db
ENV VAUTR_DB_URL=$VAUTR_DB_URL
ENV RUST_LOG=info

# The server binds 0.0.0.0:8080 (core/vautr-server/src/main.rs).
EXPOSE 8080

# Persistent data (SQLite db + WAL files when VAUTR_DB_URL is sqlite:...).
VOLUME ["/data"]
WORKDIR /data

ENTRYPOINT ["vautr-server"]
