# Vautr Software Requirements Specification (SRS)

| Field | Value |
|-------|-------|
| Project | Vautr |
| Document | Software Requirements Specification |
| Version | 1.0 (Draft) |
| Date | 2026-06-05 |
| Author | Vautr Core Team (assisted by AI) |
| Status | Draft — Pending Review |
| Upstream | Vision v1.0, BRS v1.0 |
| Downstream | Architecture & Design, Test Verification |

---

## 1. Introduction & Scope

### 1.1 Purpose
This document specifies the functional and non‑functional requirements for the Vautr software system, including the client applications (desktop, mobile, web, extension), the server (self‑hosted and cloud), and supporting infrastructure. It is intended for developers, testers, architects, and validators.

### 1.2 System Scope
Vautr provides:
- Secure, zero‑knowledge storage of passwords, notes, and secrets.
- Cross‑platform clients with offline‑first sync.
- Self‑hostable or cloud‑hosted server.
- Emergency recovery via Recovery Key.
- Sharing of items between users.
- Import from competing password managers.

**Out of scope (for this release):**
- Large file attachments > 100 MB.
- Enterprise SSO / SCIM.
- Offline account creation.

### 1.3 References
| Document | Version | Use |
|----------|---------|-----|
| Vautr Vision | v1.0 | Goals and non‑goals |
| Vautr BRS | v1.0 | Business rules, stakeholder needs, use cases |
| Vautr Cryptographic Specification | v2.0 | Crypto algorithms, key tree |
| Vautr API Contract | v1.0 | Server endpoints |
| Vautr Data Schema | v1.0 | Domain models |

---

## 2. System Context & Overview

### 2.1 System Context Diagram (conceptual)

```
[User] ←→ [Vautr Client] ←→ [Vautr Server] ←→ [SQLite DB]
                │                    │
                ├─ [OS Keystore]      └─ [RustFS (attachments)]
                ├─ [Browser]
                └─ [Competitor Export Files]
```

### 2.2 External Systems & Actors

| Actor / System | Description | Interface |
|----------------|-------------|------------|
| User | Human operator | Client UI |
| Vautr Server | Untrusted encrypted blob store | HTTPS / API |
| SQLite | Server database (self‑hosted or cloud) | Local file or volume |
| OS Keystore | Biometric and key storage (Secure Enclave, TEE) | Platform API |
| Browser | Web extension autofill target | DOM / Chrome APIs |
| Competitor files | CSV, 1pux, Bitwarden JSON for import | File system |

### 2.3 Operating Environments

| Client | Supported OS / Platform | Constraints |
|--------|------------------------|-------------|
| Desktop (GPUI) | Windows 10+, macOS 11+, Linux (glibc 2.31+) | GPUI nightly tested |
| Mobile (React Native) | iOS 15+, Android 8+ | Expo 57 |
| Web (WASM) | Chrome, Firefox, Safari, Edge (latest 2 versions) | WebWorker + OPFS |
| Extension | Chromium, Firefox (MV3) | Stateless SW for autofill |
| Server | Linux (x86_64, ARM64), Docker | SQLite WAL |

---

## 3. Functional Capabilities & Behavior

Requirements are uniquely identified (REQ‑FUNC‑xxx). Priority: **Must** (M), **Should** (S), **Could** (C).  
Syntax follows EARS patterns (ubiquitous, event‑driven, state‑driven, unwanted behaviour).

---

### 3.1 Capability: Authentication & Vault Unlock

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑AUTH‑01 | **When** the user provides a Master Password, **the client shall** derive the Master Key using Argon2id with parameters calibrated to target 300ms derivation time. | M | SN‑01 |
| REQ‑AUTH‑02 | **When** the user registers, **the client shall** execute the OPAQUE registration flow; **the server shall never** receive the Master Password or any password equivalent. | M | BG‑2 |
| REQ‑AUTH‑03 | **When** the user logs in, **the client shall** complete OPAQUE login; upon success, **the client shall** fetch and decrypt the WrappedSVK using KEK derived from MK. | M | UC‑01 |
| REQ‑AUTH‑04 | **When** biometric unlock is available and enabled, **the client shall** use the OS Keystore to unwrap the SVK without re‑entering MP. | S | QE‑5 |
| REQ‑AUTH‑05 | **If** the client’s local `enc_key_gen` is less than the server’s `min_enc_key_gen`, **then the client shall** enter Read‑Only Gate and **shall** disable all mutation operations. | M | BG‑3 |
| REQ‑AUTH‑06 | **When** the user locks the vault (manually or after timeout), **the client shall** clear all decrypted secrets from memory and **shall** call `release_secret` on all active handles. | M | Security |

---

### 3.2 Capability: Vault Management (CRUD)

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑CRUD‑01 | **When** the user creates a new item, **the client shall** generate a new UUID, encrypt the Overview with OEK and the Secret with DEK, and **shall** store the item locally before syncing. | M | UC‑02 |
| REQ‑CRUD‑02 | **When** the user edits an existing item, **the client shall** create a new version, increment the local version counter, and **shall** enqueue the update via the PersistenceWorker. | M | UC‑02 |
| REQ‑CRUD‑03 | **When** the user deletes an item, **the client shall** soft‑delete (tombstone) by setting `deleted_date` and **shall** not immediately purge from server. | M | UC‑02 |
| REQ‑CRUD‑04 | **When** the user searches with a non‑empty query, **the client shall** perform a full‑text search on the local FTS5 index and **shall** return matching `DecryptedOverview` items within 100 ms for up to 10,000 items. | M | QE‑2 |
| REQ‑CRUD‑05 | **When** the user searches with an empty query (`""`), **the client shall** return the top 50 items ordered by `last_used_at DESC, created_at DESC`. | S | UX |
| REQ‑CRUD‑06 | **If** the user attempts to save an item while the vault is in Read‑Only Gate, **then the client shall** reject the mutation and **shall** display a message explaining re‑authentication is required. | M | REQ‑AUTH‑05 |

---

### 3.3 Capability: Sync Engine (OCC & Offline)

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑SYNC‑01 | **When** the client comes online, **it shall** pull metadata changes since the last cursor using `GET /sync/pull` and **shall** update local `sync_cursor`. | M | UC‑04 |
| REQ‑SYNC‑02 | **While** processing metadata, **the client shall** consult the in‑memory DashMap (LocalBlacklist) and **shall** skip downloading payloads for UUIDs present in the map. | M | BG‑3 |
| REQ‑SYNC‑03 | **When** the server returns a 412 conflict during push, **the client shall** fetch the latest metadata for that UUID, update the DashMap, and **shall** present a resolution prompt to the user (Overwrite / Keep Server / Keep Local). | M | BR‑2 |
| REQ‑SYNC‑04 | **The client shall** persist the DashMap to SQLite at the end of every sync session (or on graceful shutdown) by atomically wiping and rewriting the `local_blacklist` table. | M | Crash safety |
| REQ‑SYNC‑05 | **When** the server returns 410 `cursor_expired`, **the client shall** reset its local cursor to 0 and perform a full metadata sync. | M | API |
| REQ‑SYNC‑06 | **The PersistenceWorker shall** accept `SaveCommand` and `DeleteCommand` with a `sync_epoch`. **If** the command’s epoch differs from current epoch, **the worker shall** abort and fire `MutationFailed` with the original state. | M | BG‑3 |

---

### 3.4 Capability: Secret Access & Opaque Handles

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑SECRET‑01 | **When** the user requests to view a secret, **the client shall** call `reveal_secret(uuid)`, which decrypts the secret, stores it in an internal DashMap, and returns a `SecretHandle` (u64). | M | Security |
| REQ‑SECRET‑02 | **The client shall** never pass the decrypted secret string to the JavaScript layer (mobile/web). Instead, **the client shall** perform actions (copy, autofill) via `perform_action` with the handle. | M | Security |
| REQ‑SECRET‑03 | **While** a `SecretHandle` is being used for an action (copy/autofill), **the client shall** set its state to `InUse` and **shall** prevent the Safety Reaper from zeroizing it. | M | Security |
| REQ‑SECRET‑04 | **When** the component that holds the handle unmounts or the user navigates away, **the client shall** call `release_secret(handle)` to zeroize the secret and free the handle. | M | Security |
| REQ‑SECRET‑05 | **The Safety Reaper shall** run every 10 seconds and **shall** zeroize any entry with state `Idle` and `last_accessed > 60 seconds`. | M | Crash safety |

---

### 3.5 Capability: Key Rotation & Emergency Recovery

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑ROTATE‑01 | **When** the user initiates key rotation, **the client shall** generate a new SVK, **then shall** call `POST /account/rotate-key` to update `min_enc_key_gen` on the server. | M | UC‑07 |
| REQ‑ROTATE‑02 | **After** rotation, **the client shall** iterate over all items with `enc_key_gen < new_gen` in batches of 100, re‑encrypt them with the new SVK, and push via `POST /sync/push-batch`. | M | BG‑3 |
| REQ‑ROTATE‑03 | **If** the client crashes during rotation, **upon restart** **it shall** resume from the last unprocessed batch by querying items still at old `enc_key_gen`. | M | Crash safety |
| REQ‑RECOVERY‑01 | **When** the user possesses the Recovery Key (24‑word BIP‑39), **the client shall** decode it, derive `KEK_RK`, decrypt `WrappedSVK_RK`, and **shall** unlock the vault. | M | UC‑05 |
| REQ‑RECOVERY‑02 | **After** recovery, **the client shall** force the user to set a new Master Password, **shall** generate a new Recovery Key, **shall** re‑wrap the SVK with both KEK_MP and KEK_RK, and **shall** push to server. | M | BR‑7 |
| REQ‑RECOVERY‑03 | **The client shall** generate the Emergency Kit PDF locally (no network transmission) and **shall not** include a QR code. | M | BR‑7 |

---

### 3.6 Capability: Sharing (1:1 and Groups)

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑SHARE‑01 | **When** User A shares an item with User B, **the client shall** generate a random Symmetric Item Key (SIK), re‑encrypt the item with SIK, **then shall** wrap the SIK using User B’s `SharingPublicKey` (KEM) and upload the wrapped SIK and ephemeral public key to the server. | S | SN‑02 |
| REQ‑SHARE‑02 | **When** User B receives a share, **the client shall** decapsulate the SIK using B’s `SharingPrivateKey`, **then shall** cache the SIK wrapped under B’s personal KEK for offline access. | S | SN‑02 |
| REQ‑SHARE‑03 | **When** User A updates a shared item, **the client shall** re‑encrypt the item with the same SIK and **shall** not require re‑wrapping of recipient keys. | S | UC‑06 |
| REQ‑SHARE‑04 | **When** User A revokes a share, **the client shall** rotate the SIK, re‑encrypt the item, and re‑wrap the new SIK for remaining recipients. | S | Security |

---

### 3.7 Capability: Import from Competitors

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑IMPORT‑01 | **The client shall** support import from CSV (Chrome/Bitwarden), 1Password `.1pux`, and Bitwarden encrypted JSON. | S | BR‑4 |
| REQ‑IMPORT‑02 | **When** importing, **the client shall** stream the file, parse in chunks, and **shall** generate new UUIDs for each item (discard original IDs). | M | BR‑4 |
| REQ‑IMPORT‑03 | **The client shall** deduplicate imports by exact match on (title, primary URL) using a pre‑flight HashSet. | S | UX |
| REQ‑IMPORT‑04 | **The import pipeline shall** use size‑aware parallel encryption (Rayon) with chunking at 100 items or 25 MB plaintext to bound memory. | M | Performance |
| REQ‑IMPORT‑05 | **After** import, **the client shall** rebuild the FTS5 index from scratch (drop and recreate) to avoid incremental trigger overhead. | M | Performance |

---

### 3.8 Capability: Server API (selected essentials)

See the OpenAPI specification for full details. Key requirements:

| ID | Requirement | Priority | BRS trace |
|----|-------------|----------|-----------|
| REQ‑API‑01 | **The server shall** reject any write with `enc_key_gen < min_enc_key_gen` and return `422 KeyGenerationTooOld`. | M | BG‑2 |
| REQ‑API‑02 | **The server shall** enforce OCC using atomic SQL `UPDATE ... WHERE version = :target_version` and return `412` if rows_affected = 0. | M | BG‑3 |
| REQ‑API‑03 | **The server shall** support batch push of up to 100 items per request, each with individual OCC evaluation, and commit successes atomically. | M | API contract |
| REQ‑API‑04 | **The server shall** never log request or response payloads containing user data. | M | Security |
| REQ‑API‑05 | **When** a client requests a payload with exact version, **the server shall** return the payload if version matches, otherwise return `version_mismatch` with current metadata. | M | API contract |

---

## 4. Quality & Non‑functional Requirements

Organised by ISO/IEC 25010:2023 categories. Each NFR has a fit criterion.

### 4.1 Performance Efficiency

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑PERF‑01 | First MP unlock (cold start) shall complete within 2 seconds on reference hardware (2024 laptop, 16GB RAM, SSD). | Average of 10 measurements ≤ 2s. | Test (benchmark) |
| NFR‑PERF‑02 | Search latency (10,000 items, non‑empty query) shall be ≤ 100 ms at p95. | p95 over 1000 searches ≤ 100 ms. | Test (automated) |
| NFR‑PERF‑03 | Sync pull of 100 metadata items shall complete within 500 ms (server + network ≤ 200ms). | 90th percentile ≤ 500ms. | Analysis + test |
| NFR‑PERF‑04 | Client startup (unlocked state) to interactive shall be ≤ 1 second. | Measured from process start to UI ready. | Test |
| NFR‑PERF‑05 | WebAssembly client memory footprint shall not exceed 50 MB for typical vault (1,000 items). | Heap usage measured in Chrome. | Analysis |

### 4.2 Reliability & Availability

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑REL‑01 | Cloud server monthly availability shall be ≥ 99.9% (excluding planned maintenance). | Uptime measured over calendar month. | Monitoring (SLO) |
| NFR‑REL‑02 | Client shall survive SQLite WAL corruption without data loss (via backup or repair). | Corruption injection test: recovery possible. | Test (chaos) |
| NFR‑REL‑03 | Offline queue of up to 1,000 mutations shall be persisted and replayed correctly after app restart. | Test: offline 1,000 mutations, kill app, restart, sync. | Test |
| NFR‑REL‑04 | Crash‑safe rotation: if client crashes during rotation, no items shall be lost and rotation shall resume. | See test plan. | Test |

### 4.3 Security

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑SEC‑01 | No plaintext PII (passwords, titles, URLs, notes) shall ever be transmitted to the server. | Network capture test + audit. | Inspection + test |
| NFR‑SEC‑02 | All encryption shall use XChaCha20‑Poly1305 with 192‑bit nonce and 128‑bit tag. | Code review, static analysis. | Inspection |
| NFR‑SEC‑03 | Master Password shall be derived using Argon2id with parameters that achieve ~300ms on target device. | Benchmark on reference devices. | Test |
| NFR‑SEC‑04 | Server shall enforce rate limiting: 5 req/min per IP for OPAQUE start endpoints, 30 req/min for sync pull, 10 req/min for push batch. | Load test with bursts. | Analysis + test |
| NFR‑SEC‑05 | All API traffic shall be over TLS 1.3. | Configuration scan. | Inspection |
| NFR‑SEC‑06 | Crash reports shall never include minidumps or local variables. | SDK configuration review. | Inspection |

### 4.4 Interaction Capability (Usability)

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑UX‑01 | 90% of novice users shall be able to add a new password within 60 seconds without assistance. | Usability test (n=10). | Demonstration |
| NFR‑UX‑02 | 80% of self‑hosters shall succeed in deploying the server using Docker Compose within 15 minutes. | User test (n=10). | Demonstration |
| NFR‑UX‑03 | Conflict resolution modal shall be understood by 80% of users (no confusion about options). | Survey after conflict simulation. | Inspection |
| NFR‑UX‑04 | Mobile app shall support biometric unlock (FaceID/TouchID) with fallback to MP. | Platform test. | Test |

### 4.5 Maintainability

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑MAINT‑01 | Adding a new optional field to item schema shall require changes in ≤ 3 modules (domain, DB, API). | Architecture review. | Inspection |
| NFR‑MAINT‑02 | 80% of unit tests shall run in < 1 second. | CI measurement. | Test |
| NFR‑MAINT‑03 | Server can be upgraded without downtime using blue‑green deployment. | Deployment test. | Demonstration |

### 4.6 Flexibility (Portability)

| ID | Requirement | Fit criterion | Verification method |
|----|-------------|---------------|---------------------|
| NFR‑PORT‑01 | Server Docker image shall run on both x86_64 and ARM64 (e.g., Raspberry Pi). | CI builds both architectures. | Test |
| NFR‑PORT‑02 | Web client shall run on Chrome, Firefox, Safari, Edge (latest two versions). | Automated browser tests. | Test |
| NFR‑PORT‑03 | Mobile app shall support iOS 15+ and Android 8+. | CI devices. | Test |

---

## 5. External Interfaces & Data Contracts

### 5.1 User Interfaces (high‑level)

| Interface | Description | Key requirements |
|-----------|-------------|------------------|
| Desktop (GPUI) | Native window, list view, detail view, settings. | Masked secret fields; copy/autofill via native calls. |
| Mobile (React Native) | Tabbed interface, search, biometric prompt. | Native overlays for secret display; never pass secrets to JS. |
| Web (React) | SPA, WASM worker. | Web Component for secret rendering; postMessage bridge. |
| Extension | Popup (same as web), stateless autofill SW. | SW uses `chrome.storage.session` for SVK cache; lightweight crypto WASM. |

### 5.2 Software Interfaces (API contracts)

The canonical API contract is defined in OpenAPI (see `packages/api-contract/openapi.json`).  
Key endpoints and contract requirements:

| Endpoint | Method | Request | Response | Verification |
|----------|--------|---------|----------|--------------|
| `/sync/pull` | GET | `cursor`, `limit` | `{new_cursor, items: [{uuid, version, enc_key_gen, deleted_date}]}` | 410 on expired cursor |
| `/sync/push-batch` | POST | `{items: [{uuid, target_version, enc_key_gen, payload, deleted_date}]}` | `{results: [{status, version, ...}]}` | 422 if any `enc_key_gen` too old |
| `/items/{uuid}` | PUT | Header `If-Match`, body `{enc_key_gen, payload}` | `{version, updated_at}` | 412 on mismatch |
| `/account/rotate-key` | POST | `{new_min_enc_key_gen, new_svk_ciphertext_blob}` | `{min_enc_key_gen}` | Idempotent (200 if already equal) |
| `/auth/*` | Various | OPAQUE messages | Session token | 401 on invalid |

**Data contracts** (payload formats) are defined in the Canonical Data Schema (see `docs/spec/data.md`).

### 5.3 Hardware Interfaces

| Interface | Purpose | Requirement |
|-----------|---------|-------------|
| OS Keystore (Secure Enclave / TEE) | Biometric unlock, SVK caching | Client must use platform API to wrap/unwrap SVK. |
| File system | Import files, attachment storage | Client must read files in streaming fashion, bound memory. |

---

## 6. Constraints, Assumptions & Dependencies

### 6.1 Design & Implementation Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| CON‑1 | Server must use SQLite (not Postgres) for self‑hosting simplicity. | Self‑hoster requirement. |
| CON‑2 | Core crypto must be in pure Rust (no external crypto libraries). | Auditability, memory safety. |
| CON‑3 | All client‑server communication must be over HTTPS. | Security. |
| CON‑4 | Web client must be deployable as static files (no server‑side rendering). | Self‑hosting simplicity. |
| CON‑5 | Extension must comply with Manifest V3 (service worker based). | Chrome Web Store policy. |

### 6.2 Assumptions

| ID | Assumption | Impact if false |
|----|------------|-----------------|
| ASM‑1 | SQLite WAL mode supports up to 50 concurrent users per server. | May need connection pooling tuning. |
| ASM‑2 | WebAssembly performance is sufficient for real‑time search on 10,000 items. | Fallback to IndexedDB. |
| ASM‑3 | Users have reliable internet for sync but tolerate offline periods. | Offline queue must be robust. |
| ASM‑4 | GPUI will remain actively maintained. | Maintain local fork. |

### 6.3 Dependencies

| ID | Dependency | Version | Criticality |
|----|------------|---------|-------------|
| DEP‑1 | Rust (stable) | 1.85+ | High |
| DEP‑2 | Tokio | 1.x | High |
| DEP‑3 | SeaORM | 2.0 | High |
| DEP‑4 | React Native | 0.86 (Expo 57) | Medium |
| DEP‑5 | GPUI | git hash (pinned) | Medium |
| DEP‑6 | SQLite | 3.35+ (WAL) | High |

---

## 7. TBD Log (Open Issues)

| ID | Issue | Owner | Resolution due | Impacted requirements |
|----|-------|-------|----------------|----------------------|
| TBD‑1 | Exact pricing for cloud free tier (e.g., 1 user, 100 items free?) | Product | Pre‑alpha | NFR‑REL‑01 |
| TBD‑2 | Support for WebAuthn as second factor? | Security | Beta | NFR‑SEC‑04 |
| TBD‑3 | EU data residency: will cloud offer region selection? | Legal | Beta | NFR‑SEC‑01 |
| TBD‑4 | Minimum supported client version policy (e.g., last 2 releases) | Engineering | Alpha | REQ‑API‑05 |
| TBD‑5 | Rate limit values for self‑hosted server (configurable?) | Engineering | Alpha | NFR‑SEC‑04 |
| TBD‑6 | Offline account creation – defer to v2.0? | Product | Pre‑alpha | REQ‑AUTH‑01 |

---

## 8. Requirements Attributes & Traceability

### 8.1 Attribute Definitions

Each requirement (functional and NFR) shall have the following attributes:

| Attribute | Values | Notes |
|-----------|--------|-------|
| ID | Unique (REQ‑xxx, NFR‑xxx) | Immutable once baselined |
| Priority | Must / Should / Could | MoSCoW |
| Status | Draft / Approved / Implemented / Verified / Deprecated | Lifecycle |
| Verification method | Inspection / Analysis / Demonstration / Test | Per ISO 29148 |
| Upstream trace | BRS ID (BG, SN, UC, BR) | From BRS |
| Downstream trace | Design element / Test case ID | To be filled |

### 8.2 Traceability Matrix (excerpt)

| Requirement ID | Upstream (BRS) | Verification method | Priority | Status |
|----------------|----------------|---------------------|----------|--------|
| REQ‑AUTH‑01 | SN‑01, BG‑2 | Test | M | Draft |
| REQ‑AUTH‑05 | BG‑3 | Test | M | Draft |
| REQ‑SYNC‑01 | UC‑04, BG‑3 | Test | M | Draft |
| REQ‑SYNC‑03 | BR‑2 | Demonstration, Test | M | Draft |
| REQ‑SECRET‑01 | Security | Test | M | Draft |
| REQ‑ROTATE‑01 | UC‑07 | Test | M | Draft |
| REQ‑IMPORT‑02 | BR‑4 | Test | S | Draft |
| NFR‑PERF‑01 | QE‑1 | Test | M | Draft |
| NFR‑SEC‑01 | BG‑2, SN‑01 | Inspection + Test | M | Draft |

### 8.3 Traceability to Vision & BRS

All functional requirements trace to one or more of:
- **Business Goals (BG‑1..BG‑5)**
- **Stakeholder Needs (SN‑01..SN‑05)**
- **Use Cases (UC‑01..UC‑08)**
- **Business Rules (BR‑1..BR‑9)**

This satisfies ISO 29148 traceability requirements and enables impact analysis.

---

## 9. Appendices

### 9.1 Glossary (extract – see BRS for full)

| Term | Definition |
|------|-------------|
| OCC | Optimistic Concurrency Control – server checks version before update. |
| EARS | Easy Approach to Requirements Syntax – structured natural language patterns. |
| OEK / DEK | Overview Encryption Key / Data Encryption Key – derived from SVK. |
| FTS5 | SQLite full‑text search extension. |

### 9.2 Verification Method Mapping

| Method | Description | Examples |
|--------|-------------|----------|
| Inspection | Human review of artefact | Code review, spec review, checklist |
| Analysis | Modelling, calculation, simulation | Performance modelling, threat modelling |
| Demonstration | Observation of system under controlled conditions | Usability test, deployment demo |
| Test | Execution with specific inputs, comparison to expected outputs | Unit test, integration test, load test |

---

**Document status:** Draft – ready for review and handover to Architecture & Design.
