# Vautr Architecture & Design Specification

| Field | Value |
|-------|-------|
| Project | Vautr |
| Document | Architecture & Design Specification |
| Version | 1.0 (Draft) |
| Date | 2026-06-05 |
| Author | Vautr Core Team (assisted by AI) |
| Status | Draft — Pending Review |
| Upstream | Vision v1.0, BRS v1.0, SRS v1.0 |
| Downstream | Test Verification Plan |

---

## 1. Context & Scope

### 1.1 Objective
This document describes the software architecture of Vautr – the decomposition into components, key design decisions, and rationale. It is intended for architects, developers, and reviewers.

### 1.2 Problem Summary
Vautr must provide zero‑knowledge, offline‑first, cross‑platform secret storage with an untrusted server. The architecture must enforce:
- No plaintext PII leaves the client.
- OCC for safe concurrent edits.
- Crash‑safe key rotation.
- High I/O bounding (DashMap, batch persistence).
- Stateless extension autofill.

### 1.3 Key Constraints (from SRS)
| ID | Constraint |
|----|------------|
| CON‑1 | Server must use SQLite (self‑hosting simplicity). |
| CON‑2 | Core crypto in pure Rust. |
| CON‑5 | Extension must comply with Manifest V3. |
| NFR‑SEC‑01 | No plaintext transmitted. |

### 1.4 Architecturally Significant Requirements (ASRs)
| ASR ID | Description | Source (SRS) |
|--------|-------------|---------------|
| ASR‑1 | Zero‑knowledge guarantee: server never sees plaintext. | NFR‑SEC‑01, BG‑2 |
| ASR‑2 | Offline‑first with OCC and crash‑safe rotation. | REQ‑SYNC‑01..06, BG‑3 |
| ASR‑3 | Cross‑platform clients (desktop, mobile, web, extension). | NFR‑PORT‑01..03 |
| ASR‑4 | Self‑hostable server with SQLite. | CON‑1, BG‑1 |
| ASR‑5 | Opaque handles for secrets – never pass plaintext to JS. | REQ‑SECRET‑01..05 |
| ASR‑6 | Bounded I/O: DashMap batch persistence, limited sync batches. | REQ‑SYNC‑04, NFR‑PERF‑05 |

---

## 2. Goals & Non‑goals (Design‑Level)

### 2.1 Goals
| ID | Goal | ASR |
|----|------|-----|
| DG‑1 | Client and server must be independently deployable (separate binaries). | ASR‑4 |
| DG‑2 | All cryptography must be isolated in `vautr-crypto` crate with no I/O. | ASR‑1 |
| DG‑3 | Sync engine must use in‑memory DashMap to bound payload downloads. | ASR‑6 |
| DG‑4 | Secret handles must be zeroized deterministically (Safety Reaper). | ASR‑5 |
| DG‑5 | The server must enforce OCC via atomic SQL updates. | ASR‑2 |

### 2.2 Non‑goals
| ID | Non‑goal | Rationale |
|----|----------|-----------|
| DNG‑1 | No distributed database (e.g., CockroachDB) for server. | SQLite sufficient for target scale. |
| DNG‑2 | No microservice decomposition – monolithic server. | Simplify self‑hosting. |
| DNG‑3 | No client‑side encryption of local SQLite (relies on OS FDE). | Out of scope for v1. |
| DNG‑4 | No real‑time push sync (polling only). | Simpler, offline‑first. |

---

## 3. C4 Model

### 3.1 C1 – System Context

```
┌─────────────────────────────────────────────────────────────┐
│                           User                              │
└───────────────────┬─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│                     Vautr Client                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌───────────┐         │
│  │ Desktop │ │ Mobile  │ │   Web   │ │ Extension │         │
│  │ (GPUI)  │ │(RNative)│ │ (WASM)  │ │ (MV3 SW)  │         │
│  └────┬────┘ └────┬────┘ └────┬────┘ └─────┬─────┘         │
│       └───────────┴───────────┴─────────────┘               │
│                        │                                    │
│                 [Core Rust lib]                             │
└────────────────────────┼────────────────────────────────────┘
                         │ HTTPS / JSON
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                     Vautr Server                            │
│  ┌────────────────────────────────────────────────────┐     │
│  │             Axum HTTP API (Rust)                   │     │
│  └───────────────────────────┬────────────────────────┘     │
│                              │                              │
│              ┌───────────────┼───────────────┐              │
│              ▼               ▼               ▼              │
│        ┌──────────┐   ┌─────────────┐   ┌───────────┐       │
│        │ SQLite   │   │ Redis (opt) │   │ RustFS    │       │
│        │ (primary)│   │ rate limit  │   │ (files)   │       │
│        └──────────┘   └─────────────┘   └───────────┘       │
└─────────────────────────────────────────────────────────────┘
```

**Actors & external systems:**
- **User** – human operating the client.
- **OS Keystore** – biometric key storage (Secure Enclave / TEE).
- **Browser** – autofill target for extension.
- **Competitor files** – import sources.

### 3.2 C2 – Container Diagram (Client Side)

```
┌─────────────────────────────────────────────────────────────────┐
│                      Vautr Client (Rust Core)                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ vautr-auth  │  │ vautr-sync  │  │ vautr-db    │             │
│  │ (OPAQUE)    │  │ (SyncEngine,│  │ (SQLite     │             │
│  │             │  │  DashMap,   │  │  SeaORM)    │             │
│  │             │  │  Reaper)    │  │             │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                    │
│         └────────────────┼────────────────┘                    │
│                          │                                     │
│  ┌───────────────────────┼───────────────────────┐             │
│  │               vautr-app-state                  │             │
│  │  (Orchestrator, Event Bus, PersistenceWorker) │             │
│  └───────────────────────┬───────────────────────┘             │
│                          │                                     │
│          ┌───────────────┼───────────────┐                     │
│          ▼               ▼               ▼                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ vautr-crypto│  │ vautr-keyring│  │ vautr-domain│             │
│  │ (primitives)│  │ (SVK rotation)│  │ (data types)│             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
          │                           │
          ▼ (FFI)                     ▼ (WASM)
┌──────────────────┐          ┌──────────────────┐
│  Mobile (UniFFI) │          │  Web (wasm-bindgen)│
│  Swift / Kotlin  │          │  JavaScript       │
└──────────────────┘          └──────────────────┘
```

**Containers (client):**
- **vautr-auth** – OPAQUE client state machine.
- **vautr-sync** – Sync engine, DashMap, Safety Reaper.
- **vautr-db** – SQLite + SeaORM, FTS5.
- **vautr-app-state** – Orchestrator, event bus, PersistenceWorker.
- **vautr-crypto** – Pure crypto (Argon2id, XChaCha, HKDF, OPAQUE primitives).
- **vautr-keyring** – SVK lifecycle, rotation, dual‑wrapping (MP + RK).
- **vautr-domain** – Shared data structures (no logic).

### 3.3 C3 – Component Diagram (Server)

```
┌─────────────────────────────────────────────────────────────┐
│                     Vautr Server (Axum)                      │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    Middleware Stack                  │    │
│  │  (Tracing, Rate Limiting, Auth Extractor, CORS)     │    │
│  └───────────────────────────┬─────────────────────────┘    │
│                              │                              │
│  ┌───────────────────────────┼───────────────────────────┐  │
│  │                           ▼                           │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │  │
│  │  │ /auth    │ │ /sync    │ │ /items   │ │ /account │ │  │
│  │  │ handlers │ │ handlers │ │ handlers │ │ handlers │ │  │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ │  │
│  │       └────────────┴────────────┴────────────┘       │  │
│  │                      │                               │  │
│  │              ┌───────▼───────┐                       │  │
│  │              │  Repository   │                       │  │
│  │              │  (sqlx)       │                       │  │
│  │              └───────┬───────┘                       │  │
│  │                      │                               │  │
│  └──────────────────────┼───────────────────────────────┘  │
│                         │                                  │
│                         ▼                                  │
│                ┌────────────────┐                         │
│                │  SQLite (WAL)  │                         │
│                │  - items       │                         │
│                │  - users       │                         │
│                │  - shares      │                         │
│                └────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

**Key components:**
- **Auth handlers** – OPAQUE server endpoints (register/start, finish, login/start, finish).
- **Sync handlers** – `GET /sync/pull`, `POST /sync/push-batch`.
- **Item handlers** – `PUT /items/{uuid}`, `DELETE /items/{uuid}`.
- **Account handlers** – status, rotate‑key, recovery endpoints.
- **Repository** – `sqlx` queries with strict user_id injection.
- **SQLite** – with WAL, synchronous=NORMAL, and background checkpointing.

---

## 4. Architecture Decision Records (ADRs)

### ADR‑001: Use SQLite as primary server database

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** ASR‑4 (self‑hostable), CON‑1, simplicity, low ops burden.

**Context:**  
Vautr server must be easy to self‑host. Traditional choice is Postgres, but it adds complexity (separate process, connection pooling, backup tooling). SQLite can run embedded, reduces attack surface, and simplifies backups (single file).

**Options considered:**
- **PostgreSQL** – Feature‑rich, better for very high concurrency. Rejected because self‑hosters prefer simplicity, and target load (< 50 concurrent users) is fine for SQLite WAL.
- **MySQL** – Similar complexity to Postgres. Rejected.
- **SQLite** – Chosen.

**Decision:**  
Use SQLite with WAL mode, `synchronous=NORMAL`, and a background checkpoint task.

**Consequences:**
- **Positive:** Single‑file backup, no separate database process, low memory footprint.
- **Negative:** Limited write concurrency (WAL helps but not infinite). Must enforce OCC via `WHERE version = ?` to avoid row locking.
- **Mitigations:** Connection pooling with `SQLITE_BUSY_TIMEOUT`, checkpoint every 60s.

**Links:** SRS CON‑1, ASR‑4.

---

### ADR‑002: Client uses DashMap for in‑memory blacklist (LocalBlacklist)

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** ASR‑6 (bounded I/O), crash safety, performance.

**Context:**  
Sync engine must avoid downloading payloads for toxic or ignored items. The blacklist must be fast (O(1)) and shared across sync threads. It also must survive client restarts.

**Options considered:**
- **Query SQLite on every sync** – High overhead, defeats I/O bounding.
- **In‑memory HashMap with Mutex** – Contention under concurrent sync.
- **DashMap (lock‑free sharded)** – Chosen.

**Decision:**  
Use `DashMap<Uuid, DashMapEntry>` where `DashMapEntry` contains `ignored_version` and `state` (ToxicIgnored / ValidIgnored). Persist the entire map to SQLite at the end of each sync session (atomic wipe + rewrite) – no dirty flag.

**Consequences:**
- **Positive:** O(1) lookups, lock‑free concurrency, crash recovery via SQLite batch persist.
- **Negative:** Entire map rewritten on every sync – acceptable for typical blacklist size (< 1000).
- **Mitigations:** Use `Arc<DashMap>`, background persist task.

**Links:** SRS REQ‑SYNC‑02, REQ‑SYNC‑04.

---

### ADR‑003: Opaque handle pattern for secrets (never pass to JS)

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** ASR‑5, NFR‑SEC‑01, compile‑time feature flags.

**Context:**  
Mobile (React Native) and Web clients must never receive plaintext secrets in JavaScript memory. Desktop (GPUI) can, because it’s native Rust.

**Options considered:**
- **Pass plaintext string over FFI** – Rejected: exposes secret to JS heap.
- **Encrypted string that JS decrypts** – Rejected: key still in JS memory.
- **Opaque handle (u64)** – Chosen.

**Decision:**  
Client exposes `reveal_secret(uuid) -> SecretHandle (u64)`. The secret is stored in a DashMap inside Rust. Platform adapter (Swift/Kotlin/WASM glue) calls `perform_action(handle)` or `read_secret(handle)` (desktop only). The handle is zeroized on unmount or by Safety Reaper.

**Consequences:**
- **Positive:** JS never touches secret strings. Compile‑time feature flag (`desktop-api`) enables `read_secret` only for GPUI.
- **Negative:** More complex FFI bridge (handles instead of direct strings). Must enforce release on unmount.
- **Mitigations:** UI components must call `release_secret` in `useEffect` cleanup.

**Links:** SRS REQ‑SECRET‑01..05, ASR‑5.

---

### ADR‑004: Server enforces OCC via atomic SQL update (no read‑modify‑write)

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** ASR‑2, OCC requirements from API contract.

**Context:**  
Server must provide strict optimistic concurrency control. The client sends `If-Match` header with expected version. The server must update only if version matches, and return 412 otherwise.

**Options considered:**
- **Read current version, compare, then update** – Race condition (TOCTOU).
- **Atomic UPDATE with WHERE version = :expected** – Chosen.
- **Use Postgres row‑level locking** – Not applicable (SQLite), also heavier.

**Decision:**  
For each mutation: `UPDATE items SET payload = ?, version = version + 1, ... WHERE uuid = ? AND user_id = ? AND version = :target_version`. Check `rows_affected`. If 0 → 412.

**Consequences:**
- **Positive:** No race conditions, simple, works with SQLite.
- **Negative:** Client must retry on conflict (handled by DashMap resolution).
- **Mitigations:** DashMap caches current server version to reduce conflicts.

**Links:** SRS REQ‑API‑02.

---

### ADR‑005: Extension autofill uses stateless WASM + chrome.storage.session

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** Manifest V3, ASR‑5, extension ephemerality.

**Context:**  
Manifest V3 service workers are killed after ~30 seconds of inactivity. Holding a full `VautrClient` (with SQLite WAL and DashMap) in the SW would cause corruption. Yet autofill must work.

**Options considered:**
- **Store SVK in chrome.storage.local and re‑instantiate client on every autofill** – Too slow, cannot hold SQLite connection.
- **Stateless approach: only vautr-crypto WASM, no SQLite, no DashMap** – Chosen.

**Decision:**  
Extension split:
- **Popup** – Uses standard `--target web` WASM build (full client, ephemeral). When popup closes, client shuts down.
- **Service Worker (autofill)** – Uses `--target nodejs` WASM build of **only `vautr-crypto`**. Caches raw SVK in `chrome.storage.session` (cleared on browser close). On autofill, reads SVK, fetches encrypted item from `chrome.storage.local`, decrypts with minimal crypto WASM, fills form.

**Consequences:**
- **Positive:** Compliant with MV3, no SQLite corruption, fast autofill.
- **Negative:** Cannot sync in background (popup must be opened to sync). Acceptable trade‑off.
- **Mitigations:** Popup triggers sync when opened.

**Links:** SRS REQ‑SECRET‑01, ASR‑5, CON‑5.

---

### ADR‑006: Crash‑safe key rotation via batch cursor on enc_key_gen

**Status:** Accepted  
**Date:** 2026-06-05  
**Decision drivers:** ASR‑2, SRS REQ‑ROTATE‑02, REQ‑ROTATE‑03.

**Context:**  
Key rotation re‑encrypts every vault item. If client crashes mid‑rotation, it must resume without redoing already‑rotated items.

**Options considered:**
- **Mark each item as rotated in SQLite** – Works, but requires extra column.
- **Use existing `enc_key_gen` column as cursor** – Chosen.

**Decision:**  
Rotation flow:
1. Client generates new SVK, calls `POST /account/rotate-key` (server increments `min_enc_key_gen`).
2. Client queries local DB for items where `enc_key_gen < new_gen` (old key). Batches of 100.
3. For each batch: decrypt with old SVK, encrypt with new SVK, update local `enc_key_gen` to `new_gen`, push to server.
4. After batch commit locally, continue. If crash, on restart `enc_key_gen` column indicates progress.

**Consequences:**
- **Positive:** No extra state, crash‑resilient, resumable.
- **Negative:** Rotation is O(N) network requests (but batched).
- **Mitigations:** Batches of 100, background worker, exponential backoff on 429.

**Links:** SRS REQ‑ROTATE‑02, REQ‑ROTATE‑03.

---

## 5. API & Interface Contracts (Design‑First)

### 5.1 OpenAPI Specification

The canonical API contract is defined in `packages/api-contract/openapi.json`. Key design decisions:

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Versioning | Content‑type `application/vnd.vautr.sync.v1+json` | Avoids URL versioning cruft. |
| Authentication | Bearer token (OPAQUE session) | Stateless, revocable. |
| OCC | `If-Match` header with u64 version | Standard HTTP pattern. |
| Batch operations | `POST /sync/push-batch` (max 100 items) | Atomic server commit. |
| Error codes | 400, 401, 404, 410, 412, 422, 429, 5xx | Aligned with API spec. |

### 5.2 Internal Data Contracts (Rust crates)

| Boundary | Contract | Enforced by |
|----------|----------|-------------|
| `vautr-crypto` → others | Pure functions, no I/O, `Zeroizing<T>` returns | Compiler, no_std |
| `vautr-db` → others | SeaORM entities, transaction closures | Type system |
| `vautr-sync` → `vautr-app-state` | `SyncEngine` trait, `DashMap` reference | Trait bounds |
| `vautr-app-state` → FFI | UniFFI / wasm‑bindgen stubs | Feature flags |

### 5.3 FFI Boundaries

| Platform | Mechanism | Exported API surface |
|----------|-----------|----------------------|
| Mobile (iOS) | UniFFI → Swift | `VautrClient` methods (no `read_secret`) |
| Mobile (Android) | UniFFI → Kotlin | Same as iOS |
| Web | wasm‑bindgen → JS | `VautrClient` wrapped in Worker, `postMessage` bridge |
| Desktop | Direct Rust (GPUI) | Full API including `read_secret` (feature‑gated) |

---

## 6. Cross‑Cutting Concerns

### 6.1 Observability

| Concern | Implementation | SRS link |
|---------|----------------|----------|
| Structured logging | `tracing` crate, spans, `instrument` | NFR‑MAINT‑02 |
| Metrics | Prometheus exporter (server), aggregated client telemetry (24h) | NFR‑REL‑01 |
| Crash reporting | Sentry (minidumps disabled, local vars off) | NFR‑SEC‑06 |
| Sync health | `tracing::warn!` on Reaper zeroization, OCC conflicts | SRS REQ‑SYNC‑03 |

### 6.2 Security (Implementation Details)

| Aspect | Approach |
|--------|----------|
| Memory locking | `mlock()` on DashMap pages to prevent swap (desktop only) |
| Zeroization | `Zeroizing<T>` for all keys, `ActiveSecret` overwrites on drop |
| Compile‑time API restrictions | Feature flags + CI symbol check: `read_secret` absent from WASM/UniFFI |
| Server authentication | OPAQUE + session token (Bearer) |
| SQL injection | `sqlx` query macros with compile‑time checked parameters |

### 6.3 Deployment & Environment

| Component | Deployment |
|-----------|------------|
| Self‑hosted server | Docker image, environment variables, SQLite volume |
| Cloud server | Same Docker image, managed SQLite (or single‑node), optional Redis |
| Desktop app | Bundled binary (`.app`, `.exe`, `.deb`) |
| Mobile app | App Store / Play Store via Fastlane |
| Web app | Static hosting (Vercel / Cloudflare) |
| Extension | Chrome Web Store / Firefox Add‑ons |

---

## 7. Alternatives Considered

### 7.1 Use of Postgres instead of SQLite

**Option:** Use PostgreSQL for server.  
**Pros:** Better write concurrency, row‑level security.  
**Cons:** More complex for self‑hosters, higher memory footprint.  
**Decision:** Rejected – SQLite with WAL meets target scale and is significantly easier for self‑hosting.

### 7.2 Use of IndexedDB instead of SQLite for Web Client

**Option:** Store vault in IndexedDB instead of SQLite via WASM.  
**Pros:** Native browser API, no need to ship SQLite.  
**Cons:** No full‑text search (FTS5), slower, less reliable.  
**Decision:** Rejected – we ship SQLite compiled to WASM (OPFS persistent).

### 7.3 Use of Signal Protocol for Sharing

**Option:** Use double ratchet for forward secrecy.  
**Pros:** Stronger cryptographic guarantees.  
**Cons:** Overkill for static shared items (updates rare).  
**Decision:** Rejected – KEM‑DEM with SIK rotation on revocation is sufficient.

### 7.4 Single binary for client (no FFI separation)

**Option:** Embed Rust core directly into React Native via RN Rust crate.  
**Pros:** No UniFFI overhead.  
**Cons:** Less portable, harder to maintain.  
**Decision:** Rejected – UniFFI provides stable ABI, well‑supported.

---

## 8. Traceability (ASR → ADRs → C4)

| ASR | ADRs | C4 elements |
|-----|------|-------------|
| ASR‑1 (zero‑knowledge) | ADR‑003 (opaque handles), ADR‑006 (rotation) | `vautr-crypto`, `vautr-keyring` |
| ASR‑2 (offline OCC) | ADR‑004 (atomic SQL), ADR‑006 (rotation) | `vautr-sync`, server repository |
| ASR‑3 (cross‑platform) | ADR‑003, ADR‑005 (extension) | Client containers, FFI layers |
| ASR‑4 (self‑host) | ADR‑001 (SQLite) | Server + SQLite |
| ASR‑5 (opaque handles) | ADR‑003, ADR‑005 | `vautr-app-state` + platform adapters |
| ASR‑6 (bounded I/O) | ADR‑002 (DashMap) | `vautr-sync` (DashMap, batch persist) |

---

## 9. Appendices

### 9.1 Glossary of Architecture Terms

| Term | Definition |
|------|-------------|
| ADR | Architecture Decision Record |
| C4 model | Hierarchical diagramming standard (Context, Container, Component, Code) |
| OCC | Optimistic Concurrency Control |
| OPAQUE | Asymmetric PAKE protocol |
| RustFS | Rust‑native object storage (replaces S3) |
| UniFFI | Mozilla tool for generating foreign language bindings |
| WAL | Write‑Ahead Logging (SQLite mode) |

### 9.2 References to Upstream Documents

| Document | Section | Used in ADR |
|----------|---------|-------------|
| SRS | REQ‑SYNC‑02, REQ‑SYNC‑04 | ADR‑002 |
| SRS | REQ‑SECRET‑01..05 | ADR‑003 |
| SRS | REQ‑API‑02 | ADR‑004 |
| SRS | REQ‑ROTATE‑02, REQ‑ROTATE‑03 | ADR‑006 |
| API Contract | OCC, batch push | ADR‑004 |

---

**Document status:** Draft – ready for review and handover to Test Verification Plan.
