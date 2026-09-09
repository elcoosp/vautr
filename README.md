<div align="center">
  <img src="brand/logo.svg" alt="Vautr Logo" width="200"/>
  
  <p><strong>The password manager that belongs to you.</strong></p>
  <p>Constitutionally open‑source. Forever free to self‑host. Built in Rust.</p>

  <!-- Badges -->
  <div style="display: flex; flex-wrap: wrap; gap: 6px; justify-content: center; align-items: center;">
    <a href="./LICENSE" style="text-decoration: none;">
      <img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=flat-square" alt="License">
    </a>
    <a href="./CONSTITUTION.md" style="text-decoration: none;">
      <img src="https://img.shields.io/badge/Self‑Host-Free%20Forever-success?style=flat-square" alt="Self-Host">
    </a>
    <a href="https://matrix.to/#/#vautr:matrix.org" style="text-decoration: none;">
      <img src="https://img.shields.io/badge/Chat-Matrix-purple?style=flat-square" alt="Chat">
    </a>
    <img src="https://img.shields.io/badge/Security-AI%20Reviewed-brightgreen?style=flat-square" alt="Security">
    <img src="https://img.shields.io/badge/Built%20with-Rust-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Client-React%20Native-20232A?style=flat-square&logo=react&logoColor=61DAFB" alt="React Native">
    <img src="https://img.shields.io/badge/Language-TypeScript-007ACC?style=flat-square&logo=typescript&logoColor=white" alt="TypeScript">
    <img src="https://img.shields.io/badge/Target-WASM-654FF0?style=flat-square&logo=webassembly&logoColor=white" alt="WASM">
  </div>
</div>

---

## The Vautr Constitution

Security software requires more than open‑source code – it requires **open governance**.  
The [**Vautr Constitution**](./CONSTITUTION.md) legally binds the project to its core principles:

1. **AGPL Forever** – No “source‑available” enterprise forks. All core code stays open.  
2. **Self‑Hosting is a Right** – Free and fully featured for individuals and small teams. No artificial paywalls.  
3. **Data Sovereignty** – Standard exports, always. No vendor lock‑in.  
4. **Transparent Pricing** – 90‑day notice for any cloud pricing changes, directly communicated.

---

## Why Vautr?

- **Zero Enshittification** – Structural guarantees prevent PE‑style erosion.  
- **Rust Core** – Memory‑safe, blazingly fast, and lightweight.  
- **First‑Class Self‑Hosting** – Run it on a Raspberry Pi or a cloud VM in seconds.  
- **Modern Clients** – Native performance across all platforms (desktop, mobile, web, extension).  
- **Offline‑First Sync** – Optimistic concurrency control (OCC) with crash‑safe key rotation.  
- **Opaque Handles** – Secrets never leak into JavaScript or React Native bridges.  

---

## Specification‑Driven Development

Vautr is built from a complete, layered specification suite (see [`docs/spec/`](./docs/spec/)):

| Document | Level | Purpose |
|----------|-------|---------|
| [**Vision**](./docs/spec/vision.md) | L0 | Why we build, goals, non‑goals, success metrics |
| [**Business & Stakeholder Requirements**](./docs/spec/brs.md) | L1 | Business rules, user classes, stakeholder needs |
| [**Software Requirements**](./docs/spec/srs.md) | L2 | Functional & non‑functional requirements (EARS, NFRs) |
| [**Architecture & Design**](./docs/spec/architecture.md) | L3 | C4 model, ADRs, API contracts, cross‑cutting concerns |
| [**Behavioral Spec & Test Verification**](./docs/spec/verification.md) | L4 | Gherkin scenarios, test plan, RTM, living documentation |

Every PR is checked against these specifications. Requirements are traced from vision → BRS → SRS → scenarios → tests.

---

## Architecture (High‑Level)

Vautr uses a **monorepo** with a Rust core (Cargo workspace) and platform‑specific frontends (Turborepo).

```text
vautr/
├── core/                         # Rust workspace
│   ├── vautr-crypto/            # Pure crypto (Argon2id, XChaCha, HKDF, OPAQUE)
│   ├── vautr-domain/            # Shared data types (no logic)
│   ├── vautr-db/                # SQLite + SeaORM + FTS5
│   ├── vautr-sync/              # Sync engine, DashMap, Safety Reaper
│   ├── vautr-auth/              # OPAQUE client state machine
│   ├── vautr-keyring/           # SVK lifecycle, rotation, dual‑wrapping
│   ├── vautr-app-state/         # Orchestrator, event bus, persistence worker
│   ├── vautr-ffi/               # UniFFI bindings (mobile)
│   ├── vautr-wasm/              # wasm‑bindgen bindings (web/extension)
│   └── vautr-server/            # Axum HTTP server (SQLite, OCC)
│
├── apps/
│   ├── desktop/                 # GPUI (Rust) – full API, native
│   ├── mobile/                  # React Native + Turbo Module
│   ├── web/                     # React + WASM worker
│   └── extension/               # Manifest V3 + stateless autofill SW
│
└── packages/                    # Shared frontend tooling
    ├── ui-components/           # React primitives
    ├── api-contract/            # OpenAPI + Zod
    └── vautr-client-sdk/        # TypeScript SDK
```

**Key design decisions (ADRs):**
- **SQLite** (server) – simplicity for self‑hosters, WAL mode, atomic OCC updates.  
- **DashMap** (client) – lock‑free in‑memory blacklist, batch‑persisted to SQLite.  
- **Opaque handles** – secrets never cross JS boundary; zeroized by Safety Reaper.  
- **Stateless extension SW** – only crypto WASM, SVK cached in `chrome.storage.session`.  

See [`docs/spec/architecture.md`](./docs/spec/architecture.md) for all ADRs and C4 diagrams.

---

## Self‑Hosting (Quick Start)

Vautr is designed to be yours. Spin up your own instance in seconds using Docker:

```bash
docker run -d \
  -p 8080:80 \
  -v vautr-data:/data \
  --name vautr-server \
  vautr/server:latest
```

Then connect any client (desktop, mobile, web, extension) by pointing it to `http://localhost:8080`.

*Detailed setup, reverse proxy, and configuration guides are available in [`docs/self-hosting/`](./docs/self-hosting/).*

---

## AI‑Assisted Development

Every Pull Request is automatically vetted by AI security review pipelines (PR‑Agent) alongside human maintainers to catch vulnerabilities before merge.  
We also enforce:

- **DCO** (Developer Certificate of Origin) – each commit must be signed off.  
- **Feature flags** – `desktop-api` enables `read_secret` only on GPUI builds.  
- **Symbol checks** – CI verifies that `read_secret` is absent from WASM/UniFFI artifacts.  

---

## Contributing

We welcome contributions from the community – Rust optimisations, UI improvements, documentation, or bug reports.  

Please read our [**Contributing Guide**](./CONTRIBUTING.md) for details on the code of conduct, DCO process, and AI review workflow.

---

## License

Vautr is licensed under the [GNU Affero General Public License v3.0](./LICENSE).  

This ensures that any modifications made to the Vautr server or core must also be open‑sourced. It is the legal enforcement of our Constitution.

---

**Happy building – and thank you for helping us build the moat.**
