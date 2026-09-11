<div align="center">
  <img src="brand/logo.svg" alt="Vautr Logo" width="200"/>
  
  <p><strong>The password manager that belongs to you.</strong></p>
  <p>Constitutionally open‑source. Forever free to self‑host. Built in Rust.</p>

  <!-- Badges -->
  <div style="display: flex; flex-wrap: wrap; gap: 6px; justify-content: center; align-items: center;">
    <img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=flat-square" alt="License">
    <img src="https://img.shields.io/badge/Self‑Host-Free%20Forever-success?style=flat-square" alt="Self-Host">
    <img src="https://img.shields.io/github/actions/workflow/status/elcoosp/vautr/release.yml?style=flat-square&label=build" alt="Build">
    <img src="https://img.shields.io/badge/PRs-welcome-brightgreen?style=flat-square" alt="PRs Welcome">
    <img src="https://img.shields.io/badge/Security-AI%20Reviewed-brightgreen?style=flat-square" alt="Security">
    <img src="https://img.shields.io/badge/Built%20with-Rust-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Client-React%20Native-20232A?style=flat-square&logo=react&logoColor=61DAFB" alt="React Native">
    <img src="https://img.shields.io/badge/Language-TypeScript-007ACC?style=flat-square&logo=typescript&logoColor=white" alt="TypeScript">
    <img src="https://img.shields.io/badge/Target-WASM-654FF0?style=flat-square&logo=webassembly&logoColor=white" alt="WASM">
  </div>
</div>

---

## Demo

https://github.com/user-attachments/assets/f0cf4705-73cb-42f9-abb1-2048322f73b0

<div align="center">
  <p><em>Full walkthrough: register → onboarding → vault → projects → MFA → audit → backup.</em></p>
</div>

---

## The Vautr Constitution

Security software requires more than open‑source code – it requires **open governance**.  
The [**Vautr Constitution**](./CONSTITUTION.md) legally binds the project to its core principles:

1. **AGPL Forever** – No "source‑available" enterprise forks. All core code stays open.
2. **Self‑Hosting is a Right** – Free and fully featured for individuals and small teams. No artificial paywalls.
3. **Data Sovereignty** – Standard exports, always. No vendor lock‑in.
4. **Transparent Pricing** – 90‑day notice for any cloud pricing changes, directly communicated.
5. **Exit Clause** – If the project is ever acquired by an entity that does not share these values, the community retains the right to fork and continue independently.

---

## Why Vautr?

- **Zero Enshittification** – Structural guarantees prevent PE‑style erosion.
- **Zero‑Knowledge by Design** – Plaintext never reaches the server. Authentication uses OPAQUE, so even the master password never leaves the client.
- **Rust Core** – Memory‑safe, fast, audited, and lightweight.
- **First‑Class Self‑Hosting** – Run it on a Raspberry Pi or a cloud VM in seconds.
- **Modern Clients** – Native performance across desktop, mobile, web, and browser extension.
- **CLI for Automation** – A `bws`‑style CLI for `get`, `list`, `run`, machine‑account login, and more.
- **Projects, Secrets & Machine Accounts** – Organize vaults with projects, per‑project permissions, shared groups, machine identities, and scoped access tokens.
- **MFA & WebAuthn** – TOTP and FIDO2/WebAuthn second factors, plus organization‑wide MFA policy.
- **Secure Sharing** – 1:1 item sharing and group sharing using X25519 + XChaCha20‑Poly1305. The server only stores wrapped keys and ciphertext.
- **Encrypted Attachments** – Chunked, resumable, encrypted file storage with bounded memory.
- **Emergency Recovery Kit** – 24‑word BIP‑39 Recovery Key, onboarding proof‑of‑possession, and forced rotation after recovery.
- **Smart Backups** – Encrypted backup archives plus a one‑click restore test that proves an archive still decrypts.
- **Offline‑First Sync** – Optimistic concurrency control (OCC) with crash‑safe key rotation and an offline mutation queue.
- **Opaque Handles** – Secrets never leak into JavaScript or React Native bridges; `read_secret` is desktop‑only.

---

## Feature Overview

| Area | What you get |
|------|--------------|
| **Vault** | Local item list, search, reveal, copy, add, edit, delete, auto‑lock |
| **Projects** | Personal/shared projects, members, roles, per‑project permissions, user groups |
| **Secrets** | Project‑scoped secrets, versioning, reveal gated by `secrets:reveal` scope |
| **Generator** | Password generation, entropy analysis, weak/reused detection |
| **MFA** | TOTP enrollment, WebAuthn/FIDO2, recovery codes, org policy |
| **Machine Accounts** | Non‑human identities for CI/CD, apps, and agents |
| **Tokens** | Scoped access tokens with expiration and revocation |
| **Sharing** | 1:1 shares, group shares, inbox, revoke, forward secrecy on member removal |
| **Files** | Encrypted attachments, resumable chunked upload/download |
| **Import/Export** | CSV/JSON export, competitor import (Bitwarden/1Password‑style), server backup/restore |
| **Audit** | Metadata‑only security log (logins, key rotations, account changes) |
| **Recovery** | Emergency Kit, recovery key onboarding, forced post‑recovery rotation |
| **Sync** | Metadata‑first pull, selective payload download, DashMap blacklist, quarantine reaper |
| **Security** | mlock, crash‑report scrubbing, CI isolation gates, cargo‑fuzz |

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

Additional architecture docs live under [`docs/architecture/`](./docs/architecture/), including:

- `crypto.md` – Argon2id, XChaCha20‑Poly1305, OPAQUE, key tree
- `data.md` – domain model, opaque handles, event bus
- `db-contract.md` – SQLite schema, FTS5, transactions
- `api.md` – server API and OPAQUE handshake
- `file-storage.md` – encrypted chunked attachments
- `sharing-pki.md` – 1:1 and group sharing
- `emergency-recovery-account.md` – Recovery Key flow
- `build-env-deploy.md` – build, release, CI
- `mlp-scope.md` / `mlp-wave-plan.md` – projects, secrets, machine accounts, tokens

---

## Architecture (High‑Level)

Vautr uses a **monorepo** with a Rust core (Cargo workspace) and platform‑specific frontends (Turborepo/pnpm).

```text
vautr/
├── core/                         # Rust workspace
│   ├── vautr-crypto/            # Argon2id, XChaCha20‑Poly1305, HKDF, OPAQUE, BIP‑39
│   ├── vautr-domain/            # Shared data types (no logic)
│   ├── vautr-db/                # SQLite + SeaORM + FTS5
│   ├── vautr-sync/              # Sync engine, DashMap, Safety Reaper, quarantine
│   ├── vautr-auth/              # OPAQUE client state machine
│   ├── vautr-keyring/           # SVK lifecycle, rotation, dual‑wrapping
│   ├── vautr-app-state/         # Orchestrator, event bus, persistence worker, epoch gate
│   ├── vautr-ffi/               # UniFFI bindings (mobile)
│   ├── vautr-wasm/              # wasm‑bindgen bindings (web/extension)
│   ├── vautr-crypto-wasm/       # Stateless crypto‑only WASM for extension SW
│   ├── vautr-server/            # Axum HTTP server (SQLite, OCC, MFA, sharing, backup)
│   ├── vautr-files/             # Chunked encrypted attachments
│   ├── vautr-import/            # Competitor import + bulk seeding
│   ├── vautr-export/            # Offline streaming CSV/JSON export
│   ├── vautr-sharing/           # 1:1 + group sharing crypto
│   ├── vautr-backup/            # Encrypted backup archives + restore test
│   └── vautr-telemetry/         # Opt‑in, anonymized telemetry
│
├── apps/
│   ├── desktop/                 # GPUI (Rust) – full API, native, sole read_secret
│   ├── mobile/                  # React Native + Expo + UniFFI native module
│   ├── web/                     # React + Vite + WASM worker
│   ├── extension/               # Manifest V3 + stateless autofill SW
│   └── cli/                     # bws‑style CLI
│
└── packages/
    ├── api-contract/            # OpenAPI + Zod schemas
    ├── vautr-client-sdk/        # TypeScript SDK
    ├── design-tokens/           # Single source of truth for themes
    ├── native/                  # UniFFI native module (Swift/Kotlin)
    ├── ui-logic/                # Shared client logic
    └── ui-components/           # Shared React primitives
```

**Key design decisions (ADRs):**

- **OPAQUE** – Password‑authenticated key exchange. The server never sees the master password or any password equivalent.
- **Argon2id + HKDF key tree** – MK → KEK → SVK → OEK/DEK. All key material is zeroized on drop.
- **XChaCha20‑Poly1305** – AEAD with associated data binding to `(uuid, enc_key_gen)`.
- **SQLite (server)** – Simplicity for self‑hosters, WAL mode, atomic OCC updates. PostgreSQL is opt‑in via Docker build args.
- **DashMap (client)** – Lock‑free in‑memory blacklist, batch‑persisted to SQLite.
- **Opaque handles** – Secrets never cross the JS/React Native bridge. `read_secret` is desktop‑only and feature‑gated.
- **Stateless extension SW** – Only crypto WASM, SVK cached in `chrome.storage.session`.
- **Sharing PKI** – X25519 KEM + XChaCha20‑Poly1305 DEM. The relay stores only wrapped SIKs and ciphertext.
- **Encrypted backups** – Archives are sealed with XChaCha20‑Poly1305. The restore test validates in a scratch DB without touching the live store.

See [`docs/spec/architecture.md`](./docs/spec/architecture.md) and [`docs/architecture/`](./docs/architecture/) for all ADRs and C4 diagrams.

---

## Zero‑Knowledge Security Model

Vautr's zero‑knowledge guarantee is structural, not cosmetic:

- **OPAQUE authentication** – The master password is used in a PAKE. The server stores only an OPAQUE registration record.
- **Local key derivation** – Argon2id derives the Master Key; HKDF derives KEK, SVK, OEK, and DEK.
- **AEAD everywhere** – Item payloads, overviews, sharing envelopes, file chunks, and backups are encrypted with XChaCha20‑Poly1305.
- **Opaque secret handles** – `reveal_secret` returns a `u64` handle. The plaintext stays in Rust. `perform_action` delegates copy/autofill to the native platform adapter.
- **Desktop‑only `read_secret`** – The restricted API is compiled only for GPUI. CI asserts it never appears in mobile, web, or extension artifacts.
- **Memory hardening** – Secret heap pages can be locked with `mlock` (VTR‑040). Crash reports are scrubbed of secrets, UUIDs, emails, and titles.
- **CI isolation gates** – `verify-isolation.yml` audits the zero‑knowledge boundary on every push and nightly.
- **Fuzzing** – `cargo-fuzz` targets AEAD decryption to ensure malformed input never panics.

Report vulnerabilities responsibly via [`SECURITY.md`](./SECURITY.md).

---

## Self‑Hosting (Quick Start)

Vautr is designed to be yours. Spin up your own instance in seconds using Docker:

```bash
docker run -d \
  -p 8080:8080 \
  -v vautr-data:/data \
  --name vautr-server \
  ghcr.io/vautrorg/vautr-server:latest
```

Or build the image locally:

```bash
docker build -t vautr-server .
docker run -d \
  -p 8080:8080 \
  -v vautr-data:/data \
  --name vautr-server \
  vautr-server
```

The server uses SQLite by default (`VAUTR_DB_URL=sqlite:vautr.db`) and stores its database under `/data`. Then connect any client — desktop, mobile, web, extension, or CLI — by pointing it to `http://localhost:8080`.

For a PostgreSQL‑enabled image, the Dockerfile supports `--build-arg FEATURES=postgres` and `--build-arg VAUTR_DB_URL=postgres://…` once the server Postgres feature is enabled.

*Detailed setup, reverse proxy, TLS, and configuration guides are available in [`docs/self-hosting/`](./docs/self-hosting/).*

---

## Local Development

### Prerequisites

- **Rust** 1.85+ (workspace `rust-version`; desktop uses a pinned nightly for GPUI)
- **Node.js** 20+ and **pnpm** 10+
- **Docker** for server images
- **Android NDK** for mobile Android cross‑compilation
- **wasm-pack** for web/extension WASM builds

### Common commands

The repo provides thin wrappers via `make`:

```bash
make help              # list all targets
make web-bootstrap     # build WASM crypto modules + install web deps
make mobile-bootstrap  # build FFI native lib + generate bindings + install mobile deps
make server-build      # release build of the Rust server
make test-all          # cargo test --workspace + web/extension TS suites
make prepare-wasm      # build all WASM crypto artifacts
make check-wasm        # fail if any prebuilt WASM artifact is missing
```

Run the server locally:

```bash
cargo build -p vautr-server --release
VAUTR_DB_URL=sqlite:vautr.db ./target/release/vautr-server
```

Run the web client:

```bash
pnpm install
pnpm prepare:wasm
pnpm --filter @vautr/web dev
```

Run the desktop client (from `apps/desktop`, using the pinned nightly toolchain):

```bash
cd apps/desktop
cargo run --bin vautr-desktop
```

Run the CLI:

```bash
cargo run -p vautr-cli -- --help
```

### Verification gate

Before opening a PR, run the same gates CI runs:

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -D warnings
cargo test --workspace
pnpm -r typecheck && pnpm -r test
pnpm prepare:wasm
```

---

## Release & Deployment

- **`release.yml`** – Triggered by `v*` tags. Builds:
  - Multi‑arch server image (`linux/amd64`, `linux/arm64`) pushed to GHCR.
  - Desktop binaries: macOS universal, Windows x86_64, Linux x86_64 + aarch64.
  - Web app static bundle and browser‑extension zip.
  - A GitHub release collecting desktop/extension artifacts.
- **`staging.yml`** – Pushes a `:staging` image to GHCR on `main`/`staging` pushes, runs a smoke test, and never touches `:latest` or production tags.
- **`verify-isolation.yml`** – Static boundary audit + nightly tests, typecheck, Playwright a11y/gallery, cargo‑fuzz, and Pact contract verification.

The server image is built from the `Dockerfile` in two stages: a Rust builder and a minimal `debian:bookworm-slim` runtime with a non‑root `vautr` user.

---

## AI‑Assisted Development

Every Pull Request is automatically vetted by AI security review pipelines (PR‑Agent) alongside human maintainers to catch vulnerabilities before merge.  
We also enforce:

- **DCO** (Developer Certificate of Origin) – each commit must be signed off.
- **Feature flags** – `desktop-api` enables `read_secret` only on GPUI builds.
- **Symbol checks** – CI verifies that `read_secret` is absent from WASM/UniFFI artifacts.
- **Token drift gate** – design tokens and generated outputs must stay in sync.
- **Test‑instrumentation guard** – memory‑leak instrumentation must never leak into production builds.

---

## Contributing

We welcome contributions from the community – Rust optimisations, UI improvements, documentation, or bug reports.

Please read our [**Contributing Guide**](./CONTRIBUTING.md) for details on:

- The [Code of Conduct](./CODE_OF_CONDUCT.md)
- The DCO process (`git commit -s`)
- AI security review workflow
- Code style (`rustfmt`, `clippy`, `biome`, Conventional Commits)
- Local onboarding via `make`

If you find a security vulnerability, **do not open a public issue**. Follow [`SECURITY.md`](./SECURITY.md) instead.

---

## License

Vautr is licensed under the [GNU Affero General Public License v3.0](./LICENSE).

This ensures that any modifications made to the Vautr server or core must also be open‑sourced. It is the legal enforcement of our Constitution.

---

**Happy building – and thank you for helping us build the moat.**
