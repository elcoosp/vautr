# Vautr Server Architecture & Scaling Specification

This document defines the exact infrastructure stack, database topology, and operational boundaries for the Vautr Server. 

A Zero-Knowledge system is only as strong as its server's discipline. The Vautr server is explicitly architected as a high-throughput, untrusted relay. It validates authentication and version integrity; it never parses, indexes, or logs encrypted payloads. If a well-meaning engineer adds an admin endpoint to "search user vaults" or verbose error logging, the Zero-Knowledge guarantee collapses. This specification enforces that boundary at the infrastructure and code level.

Deviations from this specification will result in metadata exfiltration, ZK boundary violations, and the creation of trusted-third-party backdoors.

---

## 1. The Untrusted Relay Philosophy (The Server Boundary)

1.  **The No-Plaintext Rule:** The server must never log, index, or parse the `DomainModel` payload. Payloads are opaque `BYTEA` blobs. Any logging middleware that captures request bodies is a P1 security violation.
2.  **The No-Admin Backdoor:** There are no admin endpoints that return user vault data, even encrypted. Admins can manage account state (suspend, reclaim email), but cannot read or export vault blobs.
3.  **Dumb Storage, Smart Client:** The server validates OPAQUE authentication and OCC versions. It does not validate the semantic content of the encrypted data. If a client pushes malformed encrypted garbage, the server accepts it if the OCC and epoch gates pass.
4.  **SQLite Simplicity:** A ZK blob store does not require complex distributed databases. It requires fast, atomic, key-value updates. SQLite in WAL mode provides perfect ACID compliance, zero network latency (embedded), and a vastly reduced attack surface compared to Postgres.

---

## 2. Tech Stack & Topology

*   **API Layer:** Rust (Axum) for memory safety, strict concurrency, and zero-cost abstractions.
*   **Database:** **SQLite** (WAL mode). 
    *   *Justification:* Vautr is write-light per user (occasional syncs), but read-heavy (pull). SQLite handles this perfectly. It eliminates database network latency and connection pooling overhead.
*   **Object Storage:** **RustFS** (High-performance, Rust-native distributed filesystem/object store). Replaces S3 for file attachments, keeping the entire stack Rust-native.
*   **Caching/Rate Limiting:** Redis for ephemeral OPAQUE challenges, session tokens, and distributed rate limiting.
*   **Backups:** Litestream for continuous, incremental SQLite backups to RustFS.

---

## 3. The Zero-Knowledge Gateway (WAF & Middleware)

The Axum application sits behind a strict reverse proxy (NGINX/Caddy) that enforces physical boundaries.

1.  **TLS Termination:** Strict TLS 1.3. No plaintext traffic inside the cluster.
2.  **PII Stripping:** WAF rules strip custom headers (e.g., `X-User-Email`) before they hit the Axum application. The only user identifier the app trusts is the internal session token.
3.  **Request Size Limits:** 
    *   Standard API (`/sync/push-batch`, `/auth`): 10MB limit.
    *   File Upload Initiate: 1MB limit (metadata only).
4.  **Body Logging Blackout:** The reverse proxy and Axum tracing middleware are strictly configured to *never* log request bodies or response payloads.

---

## 4. Database Architecture & SQLite OCC Implementation

SQLite enforces the Optimistic Concurrency Control (OCC) contract defined in the API Spec with absolute atomic precision.

### 4.1 Schema Design
The `items` table enforces strict typing and isolation.
```sql
CREATE TABLE items (
    uuid TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL, -- Internal ID from OPAQUE session
    version INTEGER NOT NULL,
    enc_key_gen INTEGER NOT NULL,
    deleted_date INTEGER,
    payload BLOB NOT NULL, -- Opaque encrypted blob
    updated_at INTEGER NOT NULL,
    CONSTRAINT ux_user_item UNIQUE (user_id, uuid)
);
```

### 4.2 OCC Enforcement (The `If-Match` Translation)
The server validates versions using atomic SQL updates. There is no read-modify-write cycle.
*   **API Header:** `If-Match: "5"`
*   **SQL Execution:** `UPDATE items SET payload = ?, version = version + 1, updated_at = ? WHERE uuid = ? AND user_id = ? AND version = 5;`
*   **Result:** If `rows_affected == 0`, the server returns `412 Precondition Failed`. This is mathematically immune to race conditions.

### 4.3 Async Concurrency & Connection Management
To prevent blocking the Axum runtime during heavy batch writes, SQLite is managed via `sqlx::SqlitePool`.
*   **Pool Configuration:** The pool is configured with `SQLITE_BUSY_TIMEOUT=5000`. This allows `sqlx` to queue concurrent write requests without immediately failing with `SQLITE_BUSY`, while yielding the async runtime while waiting for the lock.
*   **WAL Mode:** `PRAGMA journal_mode=WAL;` allows concurrent reads while a write is in progress.
*   **Synchronous:** `PRAGMA synchronous=NORMAL;` for performance, safe with WAL.

### 4.4 WAL Bounding & Checkpointing
Under heavy batch syncs, the SQLite WAL file can grow unbounded, consuming disk space and degrading read performance.
*   **Automated Checkpointing:** A dedicated `tokio::task` runs every 60 seconds, executing `PRAGMA wal_checkpoint(TRUNCATE)`. This ensures the WAL is flushed and truncated, bounding disk usage and maintaining read performance.

### 4.5 Application-Level Isolation
SQLite does not have Postgres' Row-Level Security. Isolation is enforced at the Axum middleware layer.
1.  **Session Injection:** Auth middleware validates the OPAQUE session and injects a verified `user_id` into Axum request extensions.
2.  **Repository Enforcement:** The Data Access Layer *always* includes `WHERE user_id = ?`. Parameters are strictly bound via `sqlx` to prevent injection.

---

## 5. File Storage Gateway (RustFS Offloading)

Large binary files bypass the SQLite layer entirely to prevent WAL bloat.

### 5.1 Presigned Generation
1.  **Request:** Client calls `POST /files/{file_uuid}/upload/initiate`.
2.  **Validation:** Server verifies the `file_uuid` does not already exist and the user has storage quota.
3.  **Generation:** Server uses its RustFS IAM credentials to generate `PUT` presigned URLs for each chunk index.
4.  **Handoff:** URLs are returned to the client. The Axum server never sees the file bytes.

### 5.2 RustFS Orphan Cleanup (Lifecycle Parity)
If a client initiates an upload but crashes before `upload/complete`, RustFS will contain orphaned encrypted chunks.
*   **IaC Configuration:** RustFS lifecycle policies (or bucket replication rules) are configured via Infrastructure as Code (Terraform/Helm) to automatically mark incomplete multipart uploads for deletion after 1 day.
*   **Background Janitor:** A secondary `tokio::task` runs every 24 hours, querying RustFS for uncommitted file chunks older than 7 days and forcefully purging them, ensuring no orphaned data persists indefinitely.

### 5.3 Storage Quotas & Abuse Prevention
Since the server cannot scan files for malware (they are encrypted), abuse is bounded by strict quotas.
*   **Free Tier:** 1 GB total attachment storage.
*   **Premium Tier:** 10 GB total attachment storage.
*   **Enforcement:** Before generating presigned URLs, the server sums the `total_size` from the user's `FileManifest` metadata in SQLite. If over quota, the request is rejected (`403 Insufficient Storage`).

---

## 6. Rate Limiting & DoS Protection

The server must protect the computationally expensive OPAQUE registration and the I/O heavy sync endpoints.

*   **OPAQUE Strict Limits:** `POST /auth/login/start` and `/auth/register/start` are limited to 5 requests per minute per IP/Email. This prevents pre-computation attacks.
*   **Sync Pull Limits:** `GET /sync/pull` is limited to 30 requests per minute.
*   **Batch Throttle:** `POST /sync/push-batch` is limited to 10 requests per minute.
*   **Implementation:** Redis-backed sliding window counters. If exceeded, returns `429 Too Many Requests` with a `Retry-After` header.

---

## 7. Observability (SQLite Health)

Standard HTTP metrics are insufficient for embedded databases. vautr exports custom Prometheus metrics to detect degradation.
*   `sqlite_wal_size_bytes`: Current size of the WAL file. Alert if > 100MB.
*   `sqlite_busy_timeouts_total`: Incremented when a query hits the busy timeout. Alert if > 0 over 5 minutes.
*   `sqlite_checkpoint_duration_seconds`: Histogram of checkpoint execution time. Alert if latency spikes.

---

## 8. Enterprise & Admin Boundaries

Operators have the power to manage the system, but zero power to read user data.

*   **Account Reclaim Only:** Operators can trigger the "Unauthenticated Reclaim" flow (email reset), which suspends the old vault and releases the email. They cannot decrypt or restore the vault.
*   **Audit Logging (Metadata Strict):** Audit logs track *only* authentication events (login, logout, reclaim). They **never** log request payloads, encrypted blobs, item UUIDs, or sync metrics (e.g., item counts). Logging sync volume is a metadata exfiltration vector.
*   **Fleet-Wide Key Rotation:** If a severe vulnerability is discovered, operators can increment the global `min_enc_key_gen` variable in SQLite. This immediately forces all clients into the `KeyUpdateRequired` Read-Only gate, preventing further writes until clients update their software and rotate their keys.
*   **No Select *:** Database access for operators is strictly read-only for administrative queries (e.g., counting total users). Queries joining `users` to `items` are forbidden at the database role level.
