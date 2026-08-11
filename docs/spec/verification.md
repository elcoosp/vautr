# Vautr Behavioral Specification & Test Verification Plan

| Field | Value |
|-------|-------|
| Project | Vautr |
| Document | Behavioral Specification & Test Verification |
| Version | 1.0 (Draft) |
| Date | 2026-06-05 |
| Author | Vautr Core Team (assisted by AI) |
| Status | Draft — Pending Review |
| Upstream | Vision v1.0, BRS v1.0, SRS v1.0, Architecture v1.0 |
| Downstream | Implementation & CI pipelines |

---

## 1. Introduction

### 1.1 Purpose
This document specifies the behavioral acceptance criteria (examples, scenarios) and the test verification strategy for Vautr. It bridges the SRS requirements to executable tests and living documentation.

### 1.2 Scope
- Behavioral specifications (BDD/Gherkin) for critical user journeys.
- Test strategy: unit, integration, contract, E2E, exploratory.
- NFR verification plans (performance, security, accessibility).
- Requirements traceability matrix (RTM) linking requirements to scenarios.
- Living documentation approach.

### 1.3 References
| Document | Version | Use |
|----------|---------|-----|
| SRS | v1.0 | Requirement IDs (REQ‑xxx, NFR‑xxx) |
| Architecture | v1.0 | ADRs, component boundaries |
| API Contract | v1.0 | OpenAPI spec for contract tests |
| BRS | v1.0 | Business rules (BR‑xxx) |

---

## 2. Behavioral Specifications (Specification by Example)

We use **Gherkin** (Given/When/Then) for executable acceptance criteria. Each scenario is tagged with the SRS requirement ID(s) it verifies. Scenarios are stored in `features/` directories alongside the relevant component.

### 2.1 Feature: Vault Unlock & Authentication

```gherkin
Feature: Vault Unlock
  As a user
  I want to unlock my vault with my Master Password or biometrics
  So that I can access my secrets securely

  Background:
    Given a registered user with email "alice@example.com"
    And the user has a vault with at least one saved item

  @REQ-AUTH-01 @REQ-AUTH-03
  Scenario: Successful unlock with correct Master Password
    When Alice enters her correct Master Password
    Then the vault unlocks within 2 seconds
    And the vault list shows her saved items

  @REQ-AUTH-05
  Scenario: Read-Only Gate triggered by key rotation
    Given the server's min_enc_key_gen is 3
    And Alice's local enc_key_gen is 2
    When Alice unlocks the vault
    Then the vault enters Read-Only mode
    And the "Save" button is disabled
    And a banner shows "Re-authenticate to save changes"

  @REQ-AUTH-04 @NFR-UX-04
  Scenario: Biometric unlock (mobile)
    Given biometric unlock is enabled
    And the SVK is cached in the OS Keystore
    When Alice authenticates with FaceID/TouchID
    Then the vault unlocks in under 1 second
    And no Master Password entry is required

  @REQ-AUTH-06
  Scenario: Auto-lock after timeout
    Given the vault is unlocked
    When the user is inactive for 5 minutes (configurable)
    Then the vault locks automatically
    And all decrypted secrets are cleared from memory
```

### 2.2 Feature: Adding & Editing Items

```gherkin
Feature: Vault Item Management
  As a user
  I want to add, edit, and delete passwords
  So that I keep my vault up to date

  @REQ-CRUD-01
  Scenario: Add a new password item
    When Alice creates a new item with title "Example", username "user@ex.com", password "P@ssw0rd"
    Then the item appears in the vault list
    And the item is stored encrypted locally
    And a sync task is enqueued

  @REQ-CRUD-02
  Scenario: Edit existing item (offline then sync)
    Given Alice is offline
    When she edits an existing item's password
    Then the change is saved locally
    When she comes online
    Then the change is pushed to the server
    And the server version increments

  @REQ-CRUD-06
  Scenario: Edit blocked during Read-Only Gate
    Given the vault is in Read-Only mode (REQ-AUTH-05)
    When Alice attempts to edit an item
    Then the save button is disabled
    And a tooltip shows "Re-authenticate to save"

  @REQ-CRUD-03
  Scenario: Soft delete and tombstone
    When Alice deletes an item
    Then the item disappears from the list
    And the local database marks deleted_date
    And the server receives a tombstone on next sync
```

### 2.3 Feature: Search (FTS5)

```gherkin
Feature: Full-Text Search
  As a user
  I want to search my vault by title, username, or URL
  So that I can find items quickly

  @REQ-CRUD-04 @NFR-PERF-02
  Scenario: Search returns results within 100ms
    Given Alice has 10,000 vault items
    When she searches for "bank"
    Then results appear in under 100ms (p95)
    And the results include all items with "bank" in title or URL

  @REQ-CRUD-05
  Scenario: Empty query returns recent items
    When Alice submits an empty search query ("")
    Then the client returns the top 50 items ordered by last_used_at DESC, created_at DESC
```

### 2.4 Feature: Sync & Conflict Resolution

```gherkin
Feature: Optimistic Concurrency Control
  As a user
  I want changes from multiple devices to sync without data loss
  So that my vault stays consistent

  @REQ-SYNC-01
  Scenario: Pull metadata after cursor
    Given Alice's last sync cursor is 42
    When the client calls GET /sync/pull?cursor=42&limit=100
    Then it receives a new_cursor and items with version > 42
    And the local sync_cursor is updated

  @REQ-SYNC-02
  Scenario: Skip downloading payload for blacklisted item
    Given an item with UUID "abc-123" is in the DashMap (ToxicIgnored)
    When the sync engine processes metadata
    Then it does NOT request the payload for "abc-123"

  @REQ-SYNC-03
  Scenario: 412 conflict triggers resolution modal
    Given Alice edits item Y locally (version 5)
    And the server version for Y is 6 (updated by another device)
    When Alice pushes her edit
    Then the server returns 412 Precondition Failed
    And the client fetches server metadata (version 6)
    And shows a conflict modal with options: [Keep Server Version] [Force Overwrite]

  @REQ-SYNC-04
  Scenario: DashMap persisted to SQLite at sync end
    Given the DashMap has 3 entries
    When the sync session completes
    Then the local_blacklist table is atomically wiped and rewritten with those 3 entries
```

### 2.5 Feature: Secret Access (Opaque Handles)

```gherkin
Feature: Reveal and Copy Secrets
  As a user
  I want to view or copy a password without exposing it to the UI framework
  So that secrets never leak into JavaScript memory

  @REQ-SECRET-01 @REQ-SECRET-02
  Scenario: Copy password via opaque handle (Web/Mobile)
    When Alice taps the "Copy" icon next to a masked password
    Then the client calls reveal_secret(uuid) → returns a handle
    Then the client calls perform_action(CopyToClipboard { handle })
    Then the native layer copies the secret to the OS clipboard
    Then release_secret(handle) is called immediately
    And the secret never enters the JavaScript heap

  @REQ-SECRET-04
  Scenario: Release handle on component unmount
    Given Alice is viewing a secret on the detail screen
    When she navigates back to the list
    Then the component's useEffect cleanup calls release_secret(handle)
    And the handle is zeroized

  @REQ-SECRET-05
  Scenario: Safety Reaper zeroizes idle handles after 60s
    Given a secret handle is InUse for 30 seconds, then becomes Idle
    When the Safety Reaper runs 60 seconds after last access
    Then it zeroizes the entry and emits a tracing::warn!
```

### 2.6 Feature: Key Rotation

```gherkin
Feature: Crash-Safe Key Rotation
  As a user
  I want to rotate my SVK if my master password is compromised
  So that old sessions cannot access my vault

  @REQ-ROTATE-01 @REQ-ROTATE-02
  Scenario: Full rotation with batch push
    Given Alice has 250 items encrypted with enc_key_gen=2
    When she initiates key rotation
    Then the client calls POST /account/rotate-key to set min_enc_key_gen=3
    Then it processes items in batches of 100:
      - decrypt with old SVK, encrypt with new SVK
      - update local enc_key_gen to 3
      - push batch via POST /sync/push-batch
    After all batches succeed, all items have enc_key_gen=3

  @REQ-ROTATE-03
  Scenario: Rotation resumes after crash
    Given rotation was interrupted after batch 1 (items 1-100 done)
    When the client restarts
    Then it queries items where enc_key_gen < 3
    And resumes from batch 2 (items 101-200)
```

### 2.7 Feature: Emergency Recovery

```gherkin
Feature: Recovery Key
  As a user who has lost their Master Password
  I want to recover my vault using the 24-word Recovery Key
  So that I don't lose my data

  @REQ-RECOVERY-01
  Scenario: Successful recovery with Recovery Key
    Given Alice has her 24-word BIP-39 Recovery Key
    When she enters it in the recovery flow
    Then the client decodes the mnemonic, derives KEK_RK, unwraps WrappedSVK_RK
    And the vault unlocks

  @REQ-RECOVERY-02
  Scenario: Forced MP reset after recovery
    After unlocking via Recovery Key
    When the client prompts for a new Master Password
    And Alice enters a new MP and confirms
    Then the client generates a new Recovery Key
    And re-wraps the SVK with both KEK_MP and KEK_RK
    And pushes to server
    And displays the new Emergency Kit PDF (no QR code)

  @BR-7
  Scenario: PDF generation has no QR code
    When the Emergency Kit PDF is generated
    Then it contains only human-readable BIP-39 words
    And no QR code or machine-scannable representation
```

### 2.8 Feature: Sharing (1:1)

```gherkin
Feature: Secure Sharing
  As a team admin
  I want to share a vault item with another user
  So that we can collaborate without exposing my master password

  @REQ-SHARE-01
  Scenario: Share an item with another user
    Given Alice wants to share item X with Bob
    And Alice has Bob's SharingPublicKey
    When she initiates share
    Then the client generates a random SIK, re-encrypts item X with SIK
    Then it wraps the SIK using Bob's public key (KEM)
    And uploads wrapped SIK + ephemeral public key to server
    Bob later sees the shared item in his vault

  @REQ-SHARE-03
  Scenario: Update shared item – recipients see changes
    Given Alice shared item X with Bob (SIK=K)
    When Alice updates item X
    Then she re-encrypts the updated plaintext with the same SIK (K)
    Bob's next sync shows the updated content without re-wrapping

  @REQ-SHARE-04
  Scenario: Revoke share – rotate SIK
    Given Alice shared item X with Bob and Charlie
    When Alice revokes Bob's access
    Then she generates a new SIK (K2), re-encrypts item X
    Then wraps K2 with Charlie's public key (only)
    Bob can no longer decrypt
```

### 2.9 Feature: Import from Competitors

```gherkin
Feature: Bulk Import
  As a new user migrating from another password manager
  I want to import all my existing passwords
  So that I don't have to re-enter them manually

  @REQ-IMPORT-01 @REQ-IMPORT-02
  Scenario: Import 1000 items from CSV
    Given Alice has a Bitwarden CSV export with 1000 items
    When she selects the file in the import dialog
    Then the client streams the file, generates new UUIDs for each item
    And encrypts them in parallel (size-aware chunks)
    And stores them in SQLite using a single transaction + FTS5 rebuild
    And shows a success report: "Imported 995 items, 5 skipped"

  @REQ-IMPORT-03
  Scenario: Deduplication skips exact matches
    Given Alice already has an item with title "Gmail" and URL "mail.google.com"
    When she imports a CSV containing the same title and URL
    Then that item is marked as DuplicateSkip
    And not imported again

  @REQ-IMPORT-04
  Scenario: Size-aware chunking prevents OOM
    Given the import includes 10 items each with 5MB of attachment data (50MB total)
    When the import pipeline runs
    Then it closes a chunk at 25MB plaintext
    And processes chunks sequentially to bound memory
```

---

## 3. Test Strategy & Plan

### 3.1 Test Pyramid (Adoption)

| Level | Goal | Proportion | Tools | Owner |
|-------|------|------------|-------|-------|
| **Unit tests** | Verify individual functions (pure logic) | 60% | Rust `#[test]`, `proptest` | Dev |
| **Integration tests** | Verify component interactions (DB, crypto, sync engine) | 25% | `tokio::test`, `wiremock`, `tempfile` | Dev + QA |
| **Contract tests** | Verify API conformance to OpenAPI | 5% | `pact`, `openapi‑test` | QA |
| **E2E / acceptance** | Critical user journeys (BDD scenarios) | 5% | Cucumber / Reqnroll + API/UI | QA |
| **Exploratory / manual** | Risk‑based, edge cases | 5% | Chartered sessions | QA + Security |

### 3.2 Test Automation Toolchain

| Layer | Tools | CI integration |
|-------|-------|----------------|
| Rust unit | `cargo test` | Every PR |
| Rust integration | `cargo test --test integration` | Every PR |
| Loom concurrency | `cargo test -p vautr-app-state --features loom` | Nightly |
| Property-based | `proptest` | Nightly |
| Fuzzing (AEAD) | `cargo-fuzz` | Nightly |
| BDD scenarios | Cucumber (Gherkin) + `cucumber‑rust` or Reqnroll (C#) | Every PR |
| Contract tests | Pact (Rust + TS) | Every PR |
| Web E2E | Playwright | Nightly |
| Mobile E2E | Maestro | Nightly (real devices) |
| Load test | k6 | Weekly |
| Security scans | `cargo-audit`, OWASP ZAP | Weekly |

### 3.3 Test Environment Strategy

| Environment | Purpose | Data | Access |
|-------------|---------|------|--------|
| **Dev** | Local developer testing | Mocked or test SQLite | Dev machine |
| **CI** | PR validation | Ephemeral SQLite, `wiremock` | GitHub Actions |
| **Staging** | Integration & performance | Anonymized synthetic data | QA team |
| **Production** | Live monitoring | Real user data (no test) | SRE |

### 3.4 Test Data Management

- **Synthetic vaults** – generate via script (1, 100, 1000, 10000 items) with known plaintext.
- **Corrupted payloads** – for toxic item testing.
- **OPAQUE test vectors** – deterministic OPAQUE messages for replay.
- **Competitor export samples** – CSV, 1pux, Bitwarden JSON in `test/fixtures/`.

---

## 4. Non‑Functional Requirements Verification Plans

### 4.1 Performance & Load (NFR‑PERF‑01..05)

| Requirement | Verification | Tool | Threshold |
|-------------|--------------|------|-----------|
| NFR‑PERF‑01 (MP unlock ≤2s) | Benchmark 10 measurements | `std::time` + criterion | Avg ≤ 2000ms |
| NFR‑PERF‑02 (search ≤100ms p95) | Load 10,000 items, run 1000 searches | `criterion` | p95 ≤ 100ms |
| NFR‑PERF‑03 (sync pull ≤500ms) | Simulate server latency + DB | `wiremock` + `tokio` | 90th %ile ≤ 500ms |
| NFR‑PERF‑05 (WASM memory ≤50MB) | Measure heap with 1000 items | Chrome DevTools | ≤ 50 MB |

**Load test scenario (k6)** for server:
```javascript
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  stages: [
    { duration: '30s', target: 50 },   // ramp-up
    { duration: '2m', target: 50 },    // steady
    { duration: '10s', target: 0 },    // ramp-down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.01'],
  },
};

export default function () {
  const res = http.get('http://localhost:8080/sync/pull?cursor=0&limit=100', {
    headers: { Authorization: `Bearer ${__ENV.TOKEN}` },
  });
  check(res, { 'status is 200': (r) => r.status === 200 });
}
```

### 4.2 Security (NFR‑SEC‑01..06)

| Requirement | Verification | Frequency | Evidence |
|-------------|--------------|-----------|----------|
| NFR‑SEC‑01 (no plaintext transmitted) | Network capture test | Every release | Wireshark log, audit report |
| NFR‑SEC‑02 (XChaCha20‑Poly1305) | Code review + static analysis | Every PR | Clippy + manual |
| NFR‑SEC‑03 (Argon2id ~300ms) | Benchmark | Nightly | `cargo bench` |
| NFR‑SEC‑04 (rate limiting) | Load test with bursts | Nightly | k6 thresholds |
| NFR‑SEC‑05 (TLS 1.3) | Configuration scan | Every deploy | `testssl.sh` |
| NFR‑SEC‑06 (crash reports no minidumps) | SDK config review | Baseline | Sentry project settings |

**Threat modeling** (STRIDE) is performed quarterly. Key mitigated threats:
- **Tampering** – AEAD with AD (uuid + enc_key_gen) prevents ciphertext swap.
- **Information disclosure** – OPAQUE + zero‑knowledge.
- **Elevation of privilege** – OCC + epoch gating.

### 4.3 Accessibility (WCAG 2.1 Level AA)

| Success criterion | Verification method | Tool | Frequency |
|------------------|---------------------|------|-----------|
| 1.4.3 Contrast (minimum) | Automated + manual | axe, pa11y | Every PR |
| 2.1.1 Keyboard | Manual test | Tab navigation | Every release |
| 2.4.6 Headings and labels | Inspection | WAVE | Every release |
| 4.1.2 Name, role, value | Automated | axe | Every PR |

**Fit criterion:** Automated scans must show zero violations of WCAG 2.1 AA on all public pages.

---

## 5. Requirements Traceability Matrix (RTM)

The RTM is maintained in a version‑controlled Markdown/CSV file. Below is an excerpt.

| Req ID | Description | BDD Scenario | Test Case ID | Status | Last Run |
|--------|-------------|--------------|--------------|--------|----------|
| REQ‑AUTH‑01 | MP unlock ≤2s | `Successful unlock with correct MP` | TC‑AUTH‑01 | ✅ Pass | 2026-06-05 |
| REQ‑AUTH‑05 | Read‑Only Gate | `Read-Only Gate triggered by key rotation` | TC‑AUTH‑05 | ✅ Pass | 2026-06-05 |
| REQ‑CRUD‑01 | Add new item | `Add a new password item` | TC‑CRUD‑01 | ✅ Pass | 2026-06-05 |
| REQ‑CRUD‑02 | Edit offline then sync | `Edit existing item (offline then sync)` | TC‑CRUD‑02 | ⏳ Pending | – |
| REQ‑SYNC‑03 | 412 conflict modal | `412 conflict triggers resolution modal` | TC‑SYNC‑03 | ✅ Pass | 2026-06-05 |
| REQ‑SECRET‑01 | Opaque handle copy | `Copy password via opaque handle` | TC‑SEC‑01 | ✅ Pass | 2026-06-05 |
| REQ‑SECRET‑05 | Safety Reaper zeroization | `Safety Reaper zeroizes idle handles` | TC‑SEC‑05 | ⏳ Pending | – |
| REQ‑ROTATE‑02 | Batch rotation | `Full rotation with batch push` | TC‑ROT‑02 | ✅ Pass | 2026-06-04 |
| REQ‑RECOVERY‑01 | Recovery with RK | `Successful recovery with Recovery Key` | TC‑REC‑01 | ✅ Pass | 2026-06-05 |
| NFR‑PERF‑02 | Search ≤100ms p95 | `Search returns results within 100ms` | PERF‑02 | ✅ Pass | 2026-06-05 |
| NFR‑SEC‑01 | No plaintext transmitted | (capture test) | SEC‑01 | ⏳ Pending (audit) | – |

**Traceability mapping:**
```
Vision Goal → Business Goal → Stakeholder Need → SRS Req → BDD Scenario → Test Case → Evidence (CI run)
```

---

## 6. Living Documentation Strategy

### 6.1 Principle
Documentation that is not continuously validated against the running system becomes stale and misleading. Vautr uses **executable specifications** as the primary source of truth for behavior.

### 6.2 Implementation

| Artifact | Storage | Validation | Publication |
|----------|---------|------------|-------------|
| Gherkin features | `features/` in repo | Run in CI (Cucumber) | HTML report on PR |
| OpenAPI spec | `packages/api-contract/` | Contract tests (Pact) | Swagger UI on staging |
| ADRs | `docs/adr/` | Code review | Markdown rendered |
| RTM | `docs/rtm.csv` | Manual review (pre‑release) | GitHub Wiki |
| NFR SLOs | `monitoring/slos.yaml` | Prometheus + k6 | Grafana dashboard |

### 6.3 Review Cadence
- **Per feature branch** – BDD scenarios must pass CI.
- **Weekly** – RTM reviewed for drift.
- **Per release** – NFR SLOs validated and published.

### 6.4 Tooling Recommendations

| Tool | Purpose | Notes |
|------|---------|-------|
| Cucumber / Reqnroll (Rust) | Gherkin execution | Use `cucumber-rust` or Reqnroll + C# for test harness |
| Pact | Contract testing | Consumer‑driven contracts for API |
| k6 | Load testing | Thresholds defined as code |
| axe-core | Accessibility | Integrated into Playwright |
| Sentry + Prometheus | Production observability | Error budgets for SLOs |

---

## 7. Exploratory Testing Charters

Chartered sessions complement scripted tests. Each session is timeboxed (60–120 min) and targets a risk area.

| Charter ID | Mission | Areas | Focus |
|------------|---------|-------|-------|
| ET‑01 | Explore conflict resolution when two devices edit same item offline | Sync, OCC | Race conditions, data loss |
| ET‑02 | Explore key rotation with network interruptions | Rotation, crash safety | Resumability, idempotency |
| ET‑03 | Explore import of malformed competitor files | Import pipeline | Graceful failure, no panic |
| ET‑04 | Explore extension autofill across 20 popular sites | Extension, SW | Reliability, statelessness |
| ET‑05 | Explore toxic item handling with corrupted ciphertexts | Sync, DashMap | Correct marking, no crash |

**Charter template (ET‑01 example):**
```
Mission: Find scenarios where concurrent offline edits cause OCC conflicts that are not resolved correctly.
Timebox: 90 min
Setup: Two devices, server (local), network control to simulate offline.
Focus areas:
- Device A edits item X offline, device B edits same item offline.
- Device A comes online, pushes.
- Device B comes online, must receive conflict modal.
- User selects "Keep Server Version" → local changes discarded.
- User selects "Force Overwrite" → server version replaced.
Check that DashMap state updates correctly and no data is lost.
Notes: Record any 412 handling where user intent is not preserved.
```

---

## 8. Acceptance Criteria for Release

Before each release (alpha, beta, GA), the following must be satisfied:

| Category | Criteria | Evidence |
|----------|----------|----------|
| **Functional** | All Must‑priority BDD scenarios pass in CI | CI report |
| **Performance** | NFR‑PERF‑01..04 pass on reference hardware | Benchmark report |
| **Security** | No critical/high findings from OWASP ZAP scan | Scan report |
| **Accessibility** | Axe‑core reports zero violations on web client | a11y report |
| **Traceability** | RTM shows 100% coverage of Must requirements | RTM check |
| **Exploratory** | All charters executed and no blocker issues found | Charter logs |
| **Load test** | Server handles 50 concurrent users meeting SLOs | k6 report |
| **Crash safety** | Rotation crash‑recovery test passes | Integration test |

---

## 9. Appendices

### 9.1 Glossary of Test Terms

| Term | Definition |
|------|-------------|
| BDD | Behaviour‑Driven Development (Given/When/Then) |
| Fit criterion | Measurable condition to verify a requirement (Volere) |
| Living documentation | Docs that are kept accurate by continuous validation |
| RTM | Requirements Traceability Matrix |
| SbE | Specification by Example |
| SLO | Service Level Objective (e.g., 99.9% availability) |
| Test charter | Mission‑driven exploratory testing guide |

### 9.2 References to Upstream Requirements

| Test artefact | SRS Req | BRS ID | ADR |
|---------------|---------|--------|-----|
| Unlock BDD scenarios | REQ‑AUTH‑01..06 | SN‑01, BG‑2 | ADR‑003 |
| Conflict scenarios | REQ‑SYNC‑03 | BR‑2 | ADR‑004 |
| Rotation scenarios | REQ‑ROTATE‑01..03 | UC‑07 | ADR‑006 |
| DashMap persistence | REQ‑SYNC‑04 | – | ADR‑002 |
| Extension autofill | REQ‑SECRET‑02 | – | ADR‑005 |

---

**Document status:** Draft – ready for implementation and CI pipeline configuration.
