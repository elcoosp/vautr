# Vautr Business & Stakeholder Requirements Specification (BRS)

| Field | Value |
|-------|-------|
| Project | Vautr |
| Document | Business & Stakeholder Requirements Specification |
| Version | 1.0 (Draft) |
| Date | 2026-06-05 |
| Author | Vautr Core Team (assisted by AI) |
| Status | Draft — Pending Review |
| Upstream | Vautr Vision v1.0 |
| Downstream | Software Requirements Specification (SRS) |

---

## 1. Business Context

### 1.1 Purpose
This document specifies the business-level and stakeholder-level requirements for Vautr, a zero‑knowledge, constitutionally protected password manager. It serves as the bridge between the **Product Vision** (why) and the **Software Requirements Specification** (what the system must do).

### 1.2 Business Problem / Opportunity
- **Problem:** Existing password managers suffer from trust erosion (enshittification), opaque data handling, and vendor lock‑in. Users cannot verify that their secrets remain private.
- **Opportunity:** Growing demand for verifiable, self‑hostable, open‑source alternatives. Mature Rust, WebAssembly, and SQLite ecosystems enable a client‑first, zero‑knowledge architecture that was previously impractical.

### 1.3 Business Scope
| In‑scope | Out‑of‑scope (for this initiative) |
|----------|-------------------------------------|
| Core password manager (CRUD, sync, search) | Large‑enterprise SSO / SCIM integration |
| Self‑hosted server (Docker, SQLite) | Native Windows 7 support |
| Cross‑platform clients (Desktop, Mobile, Web, Extension) | Server‑side password recovery |
| Zero‑knowledge encryption (OPAQUE, XChaCha20‑Poly1305) | Full disk encryption of client devices |
| Emergency recovery via BIP‑39 Recovery Key | Offline account creation |
| Bulk import from competitors (1Password, Bitwarden, CSV) | File attachments > 100 MB (post‑v1.0) |
| Optional paid cloud hosting | Telemetry that includes PII |

---

## 2. Business Goals, Objectives & Success Metrics

### 2.1 Business Goals (G‑1 to G‑5 from Vision)
| ID | Goal | Fit criterion |
|----|------|---------------|
| BG‑1 | Provide fully functional, self‑hostable server and clients for individuals & small teams. | A user following the official Docker Compose guide can have a running server and connect a client within 15 minutes, with all core features working. |
| BG‑2 | Zero‑knowledge guarantee: server never receives plaintext or decryption keys. | External security audit confirms no plaintext or key material is ever transmitted to the server. |
| BG‑3 | Support offline‑first sync with OCC and crash‑safe key rotation. | User can create 50 items offline, reconnect, and all sync without data loss or manual conflict. |
| BG‑4 | Offer a paid cloud hosting option that is opt‑in and does not degrade self‑hosting. | Self‑hosted version has identical feature set to cloud version (excluding managed uptime). |
| BG‑5 | Publish AGPL‑licensed source code for all core components. | CI pipeline verifies every release ships with complete, buildable source under AGPL‑3.0. |

### 2.2 Business Success Metrics (OKR style)
| Objective | Key Results | Measurement method |
|-----------|-------------|--------------------|
| **Establish trust as default** | KR1: Independent security audit published before v1.0 GA. | Audit report on website. |
|  | KR2: 0 reported PII leaks in first 12 months. | Incident tracker. |
| **Grow self‑hosted community** | KR3: 10,000 active self‑hosted instances by end‑of‑year 1. | Anonymous telemetry (opt‑in). |
|  | KR4: `docker pull` count > 50k/month. | Docker Hub stats. |
| **Sustainable open‑source funding** | KR5: 5% of cloud users convert to paid plan. | Billing data. |
|  | KR6: Foundation receives $100k in donations / grants by year 2. | Financial records. |

---

## 3. Business Model & Processes

### 3.1 Business Model
Vautr operates as a **Public Benefit Corporation (PBC)** with a binding Constitution. Revenue streams:
- **Paid cloud hosting** – users pay for uptime, backups, managed service (self‑hosting is free).
- **Enterprise support & compliance tooling** – audit logs, SSO (future), never core crypto.
- **Donations / grants** – to sustain development.

### 3.2 Core Business Processes
| Process | Description | Key actors |
|---------|-------------|------------|
| **User onboarding** | User registers (self‑hosted or cloud), creates vault, optionally purchases cloud plan. | User, self‑hoster (if on‑prem), Vautr cloud billing (if used). |
| **Vault operation** | User adds, edits, deletes, searches, syncs passwords. | User, client software, server. |
| **Key rotation** | User triggers rotation, server increments epoch, client re‑encrypts. | User, client, server. |
| **Emergency recovery** | User loses MP, uses Recovery Key, resets MP. | User, client, server. |
| **Self‑hosting deployment** | User downloads Docker image, configures, runs. | User, Vautr documentation, community support. |

### 3.3 Business Operational Modes
- **Self‑hosted mode** – User runs their own server; no interaction with Vautr‑operated infrastructure.
- **Cloud mode** – User uses Vautr‑operated server; may be free tier or paid.
- **Offline mode** – User operates without any server connection (sync deferred).

---

## 4. Business Rules & Policies

All business rules are given unique identifiers (BR‑xxx) for traceability.

| ID | Rule | Source | Applies to |
|----|------|--------|-------------|
| BR‑1 | The server shall never receive unencrypted vault data or keys. | Constitutional (Article II) | Server, Clients |
| BR‑2 | Self‑hosting must be free for individuals and small teams (≤ 10 users or < $1M revenue). | Constitutional (Article II) | Vautr Inc. |
| BR‑3 | Core software must be licensed under AGPL‑3.0. | Constitutional (Article III) | All code |
| BR‑4 | Users must be able to export all vault data in standard formats (CSV, JSON). | Constitutional (Article IV) | Clients |
| BR‑5 | No export functionality may be gated by internet connection or subscription status. | Constitutional (Article IV) | Clients |
| BR‑6 | Account deletion must be possible both authenticated (immediate) and unauthenticated (email‑verified, 30‑day suspension). | Emergency Recovery spec | Server |
| BR‑7 | Recovery Key must be a 24‑word BIP‑39 mnemonic; PDF must not contain QR code. | Emergency Recovery spec | Clients |
| BR‑8 | Telemetry must be opt‑in, aggregated over 24 hours, and contain no PII. | Telemetry spec | Clients, Server |
| BR‑9 | Crash reports must never include minidumps or local variables. | Telemetry spec | Clients |

---

## 5. Stakeholders & User Classes

### 5.1 Stakeholder Map
| Stakeholder | Role | Main concerns |
|-------------|------|----------------|
| **End user (individual)** | Uses Vautr to store passwords. | Privacy, ease of use, reliability, no lock‑in. |
| **Self‑hoster** | Deploys and maintains their own server. | Easy deployment, minimal resource usage, security updates. |
| **Team / family admin** | Manages shared vaults and user access. | Sharing controls, audit trails (future). |
| **Developer / integrator** | Uses Vautr API or CLI. | API stability, documentation, open source. |
| **Vautr Foundation board** | Governs the project. | Constitutional compliance, long‑term viability. |
| **Cloud subscriber** | Pays for hosted service. | Uptime, support, data portability. |
| **Security auditor** | Reviews code and architecture. | Clear security boundaries, reproducible builds. |

### 5.2 User Classes (Primary, Secondary, Disfavored)
| Class | Description | Priority |
|-------|-------------|----------|
| **Privacy‑first individual** | Single user, self‑hosts or uses free cloud. | Primary |
| **Small team (≤10)** | Shares vaults, needs basic role separation. | Primary |
| **Self‑hoster** | Runs own server, may not pay. | Primary |
| **Developer** | Uses CLI/API for automation. | Secondary |
| **Enterprise user** | Needs SSO, compliance tooling (v2.0). | Secondary (future) |
| **Cloud‑only user** | Prefers managed service, does not self‑host. | Secondary |
| **Unwilling adopter** | Does not trust open source or new managers. | Disfavored (not targeted) |

### 5.3 Jobs to Be Done (JTBD) – Selected Primary Classes

**Privacy‑first individual**
> When I’m using any online service, I want to generate and fill strong unique passwords without exposing them to any server, so that my secrets remain mine even if the service is breached.

**Small team admin**
> When a new member joins, I want to share selected vault items without giving them my master password or exposing plaintext to the server, so that I control access without compromising security.

**Self‑hoster**
> When I spin up a new Linux VM, I want to deploy Vautr server in under 10 minutes with Docker, so that I can have a private password manager without paying for cloud.

---

## 6. Glossary / Ubiquitous Language

| Term | Definition | Bounded context | Synonyms (avoid) |
|------|------------|-----------------|------------------|
| **Vault** | The encrypted collection of a user’s items. | Core | “Database”, “safe” |
| **Master Password (MP)** | The user’s password used to derive the Master Key. Never transmitted. | Auth, Crypto | “Passphrase” |
| **Symmetrical Vault Key (SVK)** | Root 256‑bit key that encrypts all vault data. Rotatable. | Crypto, Keyring | “Master key” (conflicts with MK) |
| **Encryption key generation (enc_key_gen)** | Monotonically increasing integer identifying which SVK version encrypted an item. | Sync, Server | “Epoch” |
| **Optimistic Concurrency Control (OCC)** | Server‑side version check to prevent lost updates. | Sync, Server | “If‑Match” |
| **DashMap** | In‑memory concurrent hashmap storing blacklisted items. | Sync | “Blacklist” |
| **Toxic item** | An item that cannot be decrypted (wrong key or corrupted ciphertext). | Sync | “Unreadable” |
| **Recovery Key (RK)** | 24‑word BIP‑39 mnemonic allowing vault recovery without MP. | Emergency | “Emergency kit” |
| **Read‑Only Gate** | Client state where mutations are disabled due to key epoch mismatch. | Keyring | “Locked mutations” |
| **Safety Reaper** | Background task zeroizing idle secret handles. | App State | “Cleaner” |

---

## 7. Conceptual Domain Model

This is a **conceptual** model – not a database schema or architecture.

```
[User] ── owns ──> [Vault]
[Vault] ── contains ──> [Item]
[Item] has exactly one [Overview] (plaintext for UI)
[Item] has exactly one [Secret] (encrypted separately)
[Item] has [Metadata] (created_at, updated_at, trashed)

[Vault] has [Keyring] (manages SVK generations)
[Vault] has [SyncState] (cursor, min_enc_key_gen)

[User] may have [RecoveryKey] (BIP‑39 mnemonic)
[User] may have [SharingKeypair] (for sharing)

[Server] stores [EncryptedBlob] (opaque) and [VersionedMetadata]
[Server] enforces [OCC] on updates
```

**Key relationships:**
- Each user has exactly one vault (initially).
- Each item is encrypted under a specific `enc_key_gen`.
- The server never sees the plaintext model.

---

## 8. Stakeholder Needs & User Requirements

### 8.1 Goals per user class (high‑level)

| User class | Stakeholder need (ID) | Description | Fit criterion |
|------------|----------------------|-------------|----------------|
| Privacy‑first individual | SN‑01 | “I never want my passwords to leave my device unencrypted.” | External audit confirms no plaintext network transmission. |
| Small team admin | SN‑02 | “I can share a password with a team member without that member seeing my master password.” | Sharing flow works without exposing sender’s MP. |
| Self‑hoster | SN‑03 | “I can run the server on a Raspberry Pi with 1GB RAM and SQLite.” | Server runs and passes basic load test on RPi 3 with 1GB RAM. |
| Developer | SN‑04 | “I can script vault operations via CLI.” | CLI allows adding, retrieving, deleting items without UI. |
| Cloud subscriber | SN‑05 | “My cloud‑hosted vault is available 99.9% of the time.” | Monthly uptime measured and published. |

### 8.2 High‑level user tasks (candidate use cases)

| Use case | Primary actor | Brief description |
|----------|---------------|-------------------|
| UC‑01 – Create vault | Privacy‑first individual | User provides MP, client generates SVK, registers with server. |
| UC‑02 – Add item | Any user | User enters title, username, password; client encrypts and syncs. |
| UC‑03 – Search vault | Any user | User types query; client searches local FTS5 index; results shown instantly. |
| UC‑04 – Sync across devices | Any user | Device A pushes changes; Device B pulls metadata and payloads. |
| UC‑05 – Recover vault | Privacy‑first individual | User enters 24‑word Recovery Key; client unwraps SVK; forces MP reset. |
| UC‑06 – Share item | Small team admin | Admin shares an item with another user; recipient can view/use. |
| UC‑07 – Rotate keys | Any user (suspected compromise) | User initiates rotation; server increments epoch; client re‑encrypts all items. |
| UC‑08 – Self‑host deployment | Self‑hoster | User pulls Docker image, sets env vars, runs server, connects client. |

---

## 9. System‑in‑Context & Operational Concept

### 9.1 System context diagram (conceptual)
```
[User] <──> [Vautr Client] <──> [Vautr Server (self‑hosted or cloud)]
                                    │
                                    └── [SQLite database]
                                    └── [File storage (RustFS for attachments)]

[User] also interacts with:
- OS Keystore (biometrics)
- Web browser (extension autofill)
- Competitor export files (CSV, 1pux, etc.)
```

### 9.2 Operational concept
Vautr operates as a **client‑first, offline‑capable** system:
- **Normal operation:** Client is online, syncs changes with server. Server acts as untrusted blob store.
- **Offline operation:** Client queues mutations locally in SQLite. On reconnection, pushes batch.
- **Read‑only gate:** If client’s `enc_key_gen` is below server’s `min_enc_key_gen`, mutations disabled until user re‑authenticates.
- **Emergency recovery:** User uses Recovery Key to regain access without MP, then forced MP rotation.
- **Self‑hosting:** User deploys server themselves. Vautr Inc. provides Docker images and documentation, no remote control.

### 9.3 High‑level scenarios (narrative)
**Scenario 1 – First‑time user (cloud free tier)**
> Alice downloads Vautr mobile app, taps “Create new vault”, enters email and Master Password. Client derives keys, registers with Vautr cloud server using OPAQUE. Server stores encrypted SVK and OPAQUE record. Alice adds her first password. All good.

**Scenario 2 – Offline edit & sync**
> Bob edits a password on his laptop while on a plane. Changes are saved to local SQLite. After landing, laptop reconnects. Client pushes the changed item to server using OCC. If conflict, client resolves with user guidance.

**Scenario 3 – Key rotation after lost laptop**
> Carol loses her laptop (unlocked). She initiates key rotation from her phone. Server increments `min_enc_key_gen` to reject old keys. Her laptop, when recovered, enters Read‑Only mode until she logs in again, re‑encrypts all items, and pushes.

---

## 10. Stakeholder‑Level Constraints & Quality Expectations

### 10.1 Operational constraints
| ID | Constraint | Stakeholder concern |
|----|------------|---------------------|
| OC‑1 | Server must run on SQLite (no Postgres required for self‑hosting). | Self‑hoster ease. |
| OC‑2 | Client must work without internet for at least 7 days. | Travel / remote work. |
| OC‑3 | Extension must not store decrypted secrets in persistent storage. | Browser security. |
| OC‑4 | Web client must work in all modern browsers (Chrome, Firefox, Safari, Edge). | Cross‑platform. |

### 10.2 Quality expectations (stakeholder‑level)
| ID | Quality | Target | Fit criterion |
|----|---------|--------|----------------|
| QE‑1 | Performance – unlock time | First MP unlock < 2s on typical hardware (2024+). | Measured on reference device. |
| QE‑2 | Performance – search latency | Search returns results < 100ms for 10,000 items. | Benchmark test. |
| QE‑3 | Reliability – sync conflict rate | < 1% of edits require manual conflict resolution. | Telemetry from opt‑in users. |
| QE‑4 | Security – zero‑knowledge | No plaintext PII leaves device. | Audit. |
| QE‑5 | Usability – self‑host setup | 90% of technical users succeed on first attempt. | User testing (n=20). |
| QE‑6 | Accessibility – web client | WCAG 2.1 Level AA. | Automated + manual testing. |

---

## 11. Risks, Assumptions & Open Issues

### 11.1 Risks (stakeholder‑facing)
| Risk | Impact | Mitigation |
|------|--------|-------------|
| R‑1: Low adoption due to switching cost | High | Provide one‑click importers; highlight constitutional guarantees. |
| R‑2: Self‑hosting complexity turns away non‑technical users | Medium | Publish Docker‑Compose templates; offer paid cloud as alternative. |
| R‑3: GPUI desktop framework immature | Medium | Maintain stable fork; contribute upstream. |
| R‑4: WebAssembly performance on older devices | Low | Fallback to local storage; advise upgrade. |

### 11.2 Assumptions
- SQLite WAL mode supports up to 50 concurrent users per server without tuning.
- Target users have reliable internet for sync but need offline capability.
- Security auditors will find no critical flaws before v1.0.

### 11.3 Open issues (TBDs)
| ID | Issue | Owner | Resolution due |
|----|-------|-------|----------------|
| TBD‑1 | Exact pricing tiers for cloud hosting (free limits, paid features). | Product lead | Pre‑alpha |
| TBD‑2 | Support for WebAuthn as second factor for MP unlock? | Security lead | Beta |
| TBD‑3 | Handling of EU data residency for cloud customers. | Legal | Beta |
| TBD‑4 | Minimum client version policy (e.g., support last 2 major releases). | Engineering | Alpha |

---

## 12. Traceability & Handover to SRS

### 12.1 Traceability model
```
Vision Goals (G‑1..G‑5)
    ↓
Business Goals (BG‑1..BG‑5)
    ↓
Stakeholder Needs (SN‑01..SN‑05) + Business Rules (BR‑1..BR‑9)
    ↓
Use Cases (UC‑01..UC‑08)
    ↓
System Requirements (to be defined in SRS)
```

### 12.2 Traceability matrix example
| Business Goal | Stakeholder Need | Use case | SRS requirement (to be written) |
|---------------|------------------|----------|----------------------------------|
| BG‑1 (self‑hostable) | SN‑03 (RPi capable) | UC‑08 (self‑host deploy) | REQ‑DEPLOY‑01: Server must run on SQLite with minimal RAM. |
| BG‑2 (zero‑knowledge) | SN‑01 (no plaintext) | UC‑02 (add item) | REQ‑CRYPTO‑01: Client must encrypt item before sending. |
| BG‑3 (offline sync) | SN‑02 (sharing) | UC‑04 (sync) | REQ‑SYNC‑01: Client must queue mutations when offline. |

### 12.3 Handover to SRS
The Software Requirements Specification (next document) will:
- Decompose each use case into functional requirements (EARS syntax).
- Define measurable non‑functional requirements (performance, security, etc.).
- Specify external interfaces (API contracts, OpenAPI).
- Maintain traceability back to BRS IDs (BG, SN, UC, BR, TBD).

---

**Document status:** Draft – ready for review and handover to SRS.
