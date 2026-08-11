# Vautr Product Vision & Strategic Alignment

| Field | Value |
|-------|-------|
| Project | Vautr |
| Document | Vision & Strategic Alignment |
| Version | 1.0 (Draft) |
| Date | 2026-06-05 |
| Author | Vautr Core Team (assisted by AI) |
| Status | Draft — Pending Review |

---

## 1. Vision & Elevator Pitch

### Vision Statement
**A world where digital identity belongs to the individual – not to corporations, not to servers, not to backdoors.**  
Vautr is the password manager that is constitutionally bound to remain open, self‑hostable, and zero‑knowledge forever.

### Elevator Pitch (Geoffrey Moore template)
*For individuals and teams who are tired of password managers that quietly erode trust through enshittification, Vautr is a zero‑knowledge password manager that provides constitutionally guaranteed self‑hosting, AGPL‑licensed transparency, and strict client‑side encryption. Unlike proprietary managers that can change their terms overnight, Vautr’s legal charter permanently locks in user sovereignty – no vendor lock‑in, no backdoors, no surprises.*

---

## 2. Problem Statement & Business Context

### The problem
Existing password managers suffer from two systemic failures:
- **Trust erosion** – Companies quietly move features behind paywalls, inject telemetry, or alter privacy promises after user lock‑in.
- **Architectural fragility** – Many managers rely on server‑side trust, lack offline‑first sync, or use weak encryption boundaries.

Users increasingly want **verifiable control** over their secrets, not promises.

### Why now
- Rising awareness of “enshittification” (Cory Doctorow) in SaaS.
- Growing demand for self‑hosted, open‑source alternatives.
- Maturity of Rust, WebAssembly, and SQLite enables a client‑first, zero‑knowledge architecture that was previously impractical.

### Business context
Vautr is organised as a Public Benefit Corporation (PBC) / Foundation with a binding **Constitution** (see `CONSTITUTION.md`) that legally prohibits future enshittification. The project is funded by:
- **Paid cloud hosting** (fully optional) for those who don’t self‑host.
- **Enterprise support & compliance tooling** (never core cryptography).
- **Donations / foundation grants**.

Self‑hosting is **free forever** for qualifying users (individuals & small teams).

---

## 3. Target Users & Customers

### Primary user classes
| Class | Description |
|-------|-------------|
| **Security‑conscious individuals** | Tech‑savvy users who want zero‑knowledge, offline‑first, and open source. |
| **Small teams / families** | Need shared vaults without SaaS lock‑in or per‑seat extortion. |
| **Self‑hosters** | Run their own infrastructure (Raspberry Pi, VPS, etc.) and demand full control. |
| **Developers / DevOps** | Use CLI, API, or programmatic access for secrets. |
| **Enterprises (future)** | Require SSO, audit logs, compliance – but *never* backdoor access to encrypted data. |

### Explicit non‑targets (anti‑scope)
- Users who want server‑side password recovery (impossible by design).
- Organisations that require vendor to hold decryption keys.
- Platforms that cannot support WebAssembly or local SQLite (e.g., legacy browsers without WASM).

---

## 4. User Needs & Value Proposition

### Top user needs
1. **Absolute data privacy** – Server never sees plaintext; even the vendor cannot decrypt.
2. **Offline‑first & fast** – Unlock vault, search, edit, sync – all working without internet.
3. **Constitutional guarantees** – No bait‑and‑switch; self‑hosting and AGPL are permanent.
4. **Cross‑platform** – Desktop (GPUI), mobile (React Native), web (WASM), browser extension.
5. **Easy self‑hosting** – Docker, SQLite, minimal resources.

### Value proposition & differentiators
| Versus | Vautr advantage |
|--------|----------------|
| 1Password / Bitwarden (cloud) | Constitutionally protected self‑hosting, zero‑knowledge by design, no telemetry. |
| KeePass | Modern cross‑platform sync, real‑time collaboration, OPAQUE authentication. |
| Vaultwarden | Official AGPL server with full client suite, legal charter, crash‑safe rotation. |

**Key differentiator:** The *Vautr Constitution* legally binds the project – not just open source, but open governance.

---

## 5. Desired Outcomes & Success Metrics

### Business outcomes (OKR style)
| Objective | Key Results |
|-----------|-------------|
| **Establish trust as the default** | – Publish independent security audit before v1.0 general release.<br>– Achieve 0 reported PII leaks in first 12 months. |
| **Grow self‑hosted community** | – 10,000 active self‑hosted instances by end of year 1.<br>– Maintain `docker pull` count > 50k/month. |
| **Sustainable open‑source funding** | – 5% of cloud users convert to paid plan.<br>– Foundation receives $100k in donations / grants by year 2. |

### Product outcomes (behavioural metrics)
- **Time to unlock** – median < 500ms (biometric) / < 2s (first MP unlock).
- **Sync reliability** – > 99.9% of sync operations complete without OCC conflict requiring user intervention.
- **Autofill success rate** – > 95% on top 500 websites.
- **User retention** – 90% after 30 days (self‑hosted + cloud combined).

---

## 6. Strategic Constraints

| Constraint | Impact |
|------------|--------|
| **AGPL‑3.0 license** | All core server & client code must remain open; no proprietary forks. |
| **No cloud dependency for core features** | Self‑hosted version must be fully functional without calling Vautr‑operated servers. |
| **Regulatory** | GDPR / CCPA compliant by design (no PII stored on server). |
| **Platform limitations** | Web extension must work with Manifest V3 (stateless SW for autofill). |
| **Budget** | Initial development funded by founders + grants; must avoid paid cloud APIs for self‑hosters. |
| **Timeline** | Alpha by Q4 2026, Beta Q1 2027, v1.0 Q2 2027. |

---

## 7. Goals and Non‑goals

### Goals (what this initiative must achieve)
| ID | Goal |
|----|------|
| G‑1 | Provide fully functional, self‑hostable server and clients for individuals & small teams. |
| G‑2 | Zero‑knowledge guarantee: server never receives plaintext or decryption keys. |
| G‑3 | Support offline‑first sync with OCC and crash‑safe key rotation. |
| G‑4 | Offer a paid cloud hosting option that is *opt‑in* and does not degrade self‑hosting. |
| G‑5 | Publish AGPL‑licensed source code for all core components. |

### Non‑goals (explicitly out of scope for initial release)
| ID | Non‑goal | Rationale |
|----|----------|-----------|
| NG‑1 | **Server‑side password recovery** | Breaks zero‑knowledge model. |
| NG‑2 | **Native desktop app for Windows 7 / macOS < 11** | GPUI requires modern OS. |
| NG‑3 | **Offline account creation** | Server must at least see public key material. |
| NG‑4 | **End‑to‑end encrypted file attachments > 100MB** | Post‑v1.0 optimisation. |
| NG‑5 | **Biometric unlock on web / extension** | Browser limitations (except OS credential API). |
| NG‑6 | **Optimising for large enterprise SSO** | Enterprise features are v2.0. |

---

## 8. Operational Concept & High‑Level Scenarios

### Concept of operations
Vautr operates as a **client‑first** system:
- User installs client (desktop / mobile / extension / web app).
- Client communicates with a server (self‑hosted or Vautr cloud) only to fetch/push encrypted blobs and metadata.
- All cryptography happens locally.
- Server is an untrusted, versioned blob store with OCC.

### Key scenarios
1. **First‑time user (self‑hosted)**  
   → Downloads server Docker image → Sets up SQLite → Registers via client → Creates vault → Saves first password.

2. **Multi‑device sync**  
   → User unlocks on laptop → Creates new item → Item encrypts locally → Pushed to server → Phone syncs → Item appears.

3. **Key rotation after suspected compromise**  
   → User initiates rotation → Server increments `min_enc_key_gen` → Client re‑encrypts all items locally → Batched push → Old client enters read‑only mode.

4. **Emergency recovery (lost MP)**  
   → User enters 24‑word BIP‑39 recovery key → Client unwraps SVK → Forces new MP & new recovery key → Re‑wraps vault.

5. **Offline operation**  
   → User edits vault without internet → Changes stored locally in SQLite WAL → On reconnection, sync pushes in background.

---

## 9. Stakeholders & Governance

| Role | Responsibility |
|------|----------------|
| **Executive Sponsor** | Vautr Foundation Board – approves constitutional amendments, major strategy pivots. |
| **Product Lead** | Owns vision, roadmap, and outcome metrics. |
| **Engineering Lead** | Owns architecture, security audit coordination, and release quality. |
| **Community Council** | Elected representatives from active contributors; ratifies non‑goal changes. |
| **Legal / Compliance** | Ensures AGPL compliance and constitutional enforcement. |

### Decision model for this document
- **Goals & Non‑goals** – require sponsor + engineering lead approval.
- **Success metrics** – product lead adjusts quarterly, no board vote needed.
- **Constitutional changes** – 80% board + 66% contributor vote (see Constitution Article VI).

---

## 10. Risks, Assumptions & Open Questions

### Top risks
| Risk | Mitigation |
|------|-------------|
| **Adoption inertia** (users comfortable with existing managers) | Provide one‑click importers (Bitwarden, 1Password, CSV). |
| **Self‑hosting complexity** | Publish Docker‑Compose templates, one‑command setup. |
| **GPUI stability** | Maintain fork of tested commit; contribute upstream fixes. |
| **WASM performance on mobile** | Use worker + OPFS; fallback to local storage if unsupported. |
| **Regulatory attack** (law requiring backdoors) | AGPL + self‑hosting makes compliance user‑side; Vautr Inc. cannot comply on behalf of users. |

### Assumptions
- SQLite WAL mode provides sufficient concurrency for typical family/team sizes (< 50 concurrent users).
- Rust async ecosystem (Tokio, Axum) matures without breaking changes.
- Users will trust a new password manager if it offers constitutional guarantees.

### Open questions
| Q# | Question | Owner | Due |
|----|----------|-------|-----|
| Q1 | What is the exact pricing model for cloud hosting (free tier limits)? | Product lead | Pre‑alpha |
| Q2 | Will we support hardware security keys (WebAuthn) for MP unlock? | Engineering | Beta |
| Q3 | How do we handle EU data residency for cloud customers? | Legal | Beta |

---

## 11. Traceability & Next Artefacts

### Traceability anchors
| ID | Element | Will trace to |
|----|---------|----------------|
| G‑1 | Self‑hostable server | BRS capability “Self‑hosted deployment” → SRS reqs on packaging |
| G‑2 | Zero‑knowledge guarantee | BRS constraint “No plaintext to server” → SRS crypto reqs |
| G‑3 | Offline‑first sync | BRS stakeholder need → SRS sync engine reqs |
| G‑4 | Paid cloud option | BRS business model → separate operational spec |
| G‑5 | AGPL open source | BRS licensing constraint → verified in CI |

### Next documents to be created
1. **Business & Stakeholder Requirements (BRS)** – expands goals into stakeholder needs and business rules.
2. **Software Requirements Specification (SRS)** – detailed functional & non‑functional requirements.
3. **Architecture & Design Specification** – C4 models, ADRs, API contracts.
4. **Behavioral Spec & Test Verification** – BDD scenarios, test plans, traceability.

---

**Document status:** Draft – ready for review by Vautr Core Team and Foundation Board.
