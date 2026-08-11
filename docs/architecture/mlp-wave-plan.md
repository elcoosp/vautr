# Vautr — MLP v1 Parallel Wave Plan (no merge conflicts)

**Author:** Jcode
**Date:** 2026-08-11
**Inputs:** [`mlp-scope.md`](./mlp-scope.md) (authoritative scope), current tree state.
**Companion:** [`next-steps-swarm-plan.md`](./next-steps-swarm-plan.md) (earlier, pre-MLP plan; this document supersedes it as the v1 target).

This plan implements **everything in the MLP v1 scope** (`mlp-scope.md`) *for real*.
It is decomposed so a swarm of agents can work **in parallel with zero merge
conflicts**: every workstream has a hard *files I own / files I never touch*
contract, and every shared "hot" file has exactly one owner.

> Disciplines that make this conflict-free:
> 1. **Projects** is a new domain concept, so it is introduced by **one** foundation
>    agent before anything else builds against it.
> 2. Every server feature owns its **own** handler module, repository module, and
>    numbered migration file. Nobody edits `0001_init.sql`.
> 3. Router registration, root `Cargo.toml`, `vautr-domain`, `orchestrator.rs`, and
>    `packages/api-contract` each have exactly **one** owner.
> 4. Clients are isolated by their `apps/<name>/` subtree.

---

## 0. Verified current state (grounding)

- **Server handlers already split** into modules: `account`, `audit`, `auth`,
  `files`, `items`, `recovery`, `sharing`, `sync`, `webauthn`
  (`core/vautr-server/src/handlers/`).
- **Migrations already split**: `0001_init` … `0006_webauthn`.
- **`vautr-domain` has NO Projects concept** — no `folder`/`collection`/`project`
  types. Projects is a net-new foundational model.
- **No CLI exists** (`apps/cli`, `core/vautr-cli` absent). The `bws`-style CLI is new.
- **`audit.rs` handler + `0005_audit.sql` + `vautr-telemetry`** exist but audit is
  not expanded to org events + secret-access events.
- **Clients**: `apps/web`, `apps/extension`, `apps/desktop` are real and wired to the
  live server. `apps/mobile` exists but is **de-scoped** in MLP v1 (responsive web +
  extensions first).
- `cargo test --workspace` (nightly) is green; `pnpm typecheck` is green.

---

## 1. Hot / shared files — exactly one owner each

| File | Owner | Why it's hot |
|------|-------|--------------|
| `core/vautr-domain/src/**` (Projects model, roles, perms, groups) | **Wave 0.1** | Imported by every crate & client. |
| `core/vautr-server/src/handlers/mod.rs` + router in `lib.rs` | **Wave 0.2** | Every new handler needs a registration line. |
| Root `Cargo.toml` + `[workspace.dependencies]` (new crates) | **Wave 0.3** | Adding `vautr-cli`, `vautr-backup`, etc. |
| `packages/api-contract/**` (OpenAPI + TS gen) | **Wave 0.4** | Shared contract for all clients/server. |
| `core/vautr-app-state/src/orchestrator.rs` | **Wave C** (integrator) | Every feature wants a method here. |
| `core/vautr-server/migrations/0001_init.sql` | **Nobody** | Never append; add numbered files only. |

---

## 2. Wave 0 — Foundation (small, mostly sequential, DO FIRST)

These unblock everything. Launch in order; merge before Wave A/B starts.

- **0.1 · Projects domain model.** Introduce the single **Projects** model in
  `vautr-domain`: replace the Folders/Collections mental model with **Project**;
  item → one Project; `personal | shared`; fixed roles **Owner / Admin / Manager /
  Member**; per-project permissions **Can View / Can Edit / Can Manage**; basic user
  groups; offboarding types. Owns `core/vautr-domain/**` + `core/vautr-server/migrations/0007_projects.sql`.
- **0.2 · Router scaffold.** Pre-register all new handler modules as safe stubs:
  `projects`, `machine_accounts`, `tokens`, `secrets`, `mfa`, `backup`, plus
  `health` additions — in `handlers/mod.rs` + `lib.rs` router. Owns those two files
  *only* (stubs are wired later by their owners).
- **0.3 · New crates scaffold.** Create `apps/cli` (`vautr-cli`) and
  `core/vautr-backup` (safe-stub `lib.rs`). Owns root `Cargo.toml` +
  `[workspace.dependencies]` + the two new crate dirs.
- **0.4 · Contract.** Extend `packages/api-contract` with Projects, roles,
  permissions, groups, machine accounts, access tokens, secrets, MFA, backup,
  offboarding schemas; regenerate TS types. Owns `packages/api-contract/**`.

**Gate:** `cargo test --workspace` green · `pnpm typecheck` green.

---

## 3. Wave A — Server & backend (parallel, disjoint files)

Each group owns its **own** handler module + repository module + numbered migration.

| Group | Owns | Depends on |
|-------|------|------------|
| **A1 · Projects server** (org roles, per-project perms, groups, **offboarding** = revoke all access) | `handlers/projects.rs`, `repository/projects.rs`, `0007_projects.sql` | 0.1, 0.2 |
| **A2 · Machine Accounts + Access Tokens** (expiry, revocation, fine scopes) | `handlers/machine_accounts.rs`, `handlers/tokens.rs`, `repository/machine_accounts.rs`, `0008_machine_accounts.sql` | 0.1, 0.2, 0.3 |
| **A3 · Secrets endpoints** (same-Projects secrets) | `handlers/secrets.rs`, `repository/secrets.rs`, `0009_secrets.sql` | 0.1, 0.2 |
| **A4 · Mandatory MFA + policies** (TOTP issue/verify, WebAuthn enforcement, master-password policy) | `handlers/mfa.rs`, `repository/mfa.rs`, `0010_mfa.sql` | 0.1, 0.2 |
| **A5 · Backups + one-click restore test** | `core/vautr-backup/**`, `handlers/backup.rs`, `repository/backup.rs`, `0011_backup.sql` | 0.3 |
| **A6 · Monitoring & alerting** (server-down alerts, health) | `core/vautr-telemetry/**`, `handlers/health.rs` | — |
| **A7 · Audit logs** (org events + secret-access: who/what/when) | `handlers/audit.rs`, `repository/audit.rs`, `0005_audit.sql` additions | 0.1 |

> **Conflict rule:** A3 *calls* the audit API published by A7 but never edits
> `audit.rs`. A7 owns `audit.rs` exclusively.

**Gate:** every server feature has a `cargo test` unit/integration test against the
live server (`:8080`); `cargo test --workspace` green.

---

## 4. Wave B — Clients (parallel, disjoint `apps/` subtrees)

| Group | Owns | Depends on |
|-------|------|------------|
| **B1 · Web vault** — Projects UI, role/permission management, generator + weak/reused detection, autofill, TOTP/passkey, Secrets UI, MFA setup, import/export UI | `apps/web/**`, `packages/ui-logic/**` | 0.4, A1–A4 |
| **B2 · Extension** — priority autofill, Projects, generator, secrets, MFA | `apps/extension/**`, `packages/vautr-client-sdk/**` | 0.4, A1–A4 |
| **B3 · Desktop (GPUI)** — Projects/roles/secrets UI; keep `read_secret` desktop-only (`desktop-api` feature) | `apps/desktop/**` | 0.4, A1–A4 |
| **B4 · CLI (`bws`-style)** — `get`, `list`, `run` (env injection), machine-account login, basic create/edit | `apps/cli/**` | 0.3, A2, client-sdk |

> **Conflict rule:** no agent writes outside its own `apps/<name>/` subtree. `mobile`
> is out of MLP v1 scope.

**Gate:** each client passes `pnpm typecheck` + a live-server E2E (register → login →
add → reveal → sync).

---

## 5. Wave C — Single integration pass (one agent, after A+B gate)

One designated integrator wires features into the shared seam:
- Projects filtering across `orchestrator.rs` + sync.
- Machine-account session in the shared client lib / SDK.
- Mandatory-MFA enforcement in the client handshake.
- Audit-event emission wiring.
- Cross-client end-to-end: `Client A push → Server → Client B pull`; **Machine
  Account `get/list/run` E2E**; **offboarding revoke E2E**; **backup restore-test E2E**.

Owns `core/vautr-app-state/src/orchestrator.rs` + shared client SDK seams + final
integration tests.

**Gate:** full integration test suite green; `cargo test --workspace` green;
`pnpm typecheck` green.

### §5.1 Canonical route / tab map (single source of truth)

One route/tab map is shared by every client so the **same screens exist
everywhere with equivalent UI/UX**. The canonical set below is what each client
must present as its top-level navigation. Screens outside this set may exist on
a client as extras, but the canonical screens must always be present and
equally reachable.

| # | Screen | Web (TanStack) | Mobile (TanStack) | Desktop (GPUI tab) |
|---|--------|----------------|-------------------|--------------------|
| 1 | **Projects** | `/projects` | `/` (index) | `Projects` |
| 2 | **Secrets** | `/secrets` | `/secrets` | `Secrets` (inside Projects detail) |
| 3 | **Generator** | `/generator` | `/generator` | `Generator` |
| 4 | **MFA** | `/mfa` | `/mfa` | `MFA` |
| 5 | **Settings** | `/settings` | `/settings` | `Settings` |

**Additional web-only screens** (superset, not required on mobile/desktop):
`/dashboard`, `/vault`, `/machine-accounts`, `/tokens`, `/import-export`.
**Desktop-only:** the local **Vault** (holds the `read_secret` desktop-gated
API; the mobile/web clients never expose it).

**Rules enforced here:**
- **Secrets** always render inside a Project context on mobile & desktop; web
  additionally has a standalone `/secrets` aggregator. Every client can reach
  Secrets from the Projects screen.
- **Generator** is the pure-JS `@vautr/ui-logic`/SDK password generator +
  weak/reused detection; no wasm is required, so all clients share it.
- **MFA** covers TOTP issue/verify + mandatory-MFA status.
- **Settings** covers machine accounts + access tokens + account/session.

### §5.2 Wave C integration seams (final wiring)

- **Projects filtering** across `orchestrator.rs` + sync: the Rust orchestrator
  exposes a project-scoped query and the sync layer honors per-project scoping.
- **Machine-account session** in the shared client lib / SDK: a
  token-authenticated session for non-human identities.
- **Mandatory-MFA enforcement** in the client handshake: login refuses to
  complete unlock until MFA is satisfied when the server reports it required.
- **Audit-event emission wiring**: server-side handlers emit org/secret-access
  audit events on offboarding, machine-account, and secret operations.
- **Cross-client E2E**: Client A push → Server → Client B pull; Machine Account
  `get/list/run`; offboarding revoke; backup restore-test.

---

## 6. Dispatch order

1. **Wave 0** (0.1 → 0.2 → 0.3 → 0.4) — small, sequential, merges fast.
2. **Wave A** — dispatch A1–A7 together after Wave 0 (no shared file among them).
3. **Wave B** — dispatch B1–B4 after A gate (or overlap A when contract is stable).
4. **Wave C** — single integrator after A+B gate.

---

## 7. MLP scope traceability

| MLP scope (§) | Delivered by |
|---------------|--------------|
| §1 Architecture & deployment | A5 (backups/restore), A6 (monitoring), deploy/install (see below), Wave C |
| §2 Projects & sharing & roles/permissions & offboarding | 0.1, A1, B1–B4 |
| §3 Password Manager (storage, generator, detection, autofill, TOTP/passkey, MFA, import/export, sync) | A4 (MFA), B1 (web), B2 (extension) |
| §4 Secrets Manager + Machine Accounts + Tokens + CLI + Rust SDK + audit | A2, A3, A7, B4, Wave C |
| §5 Security & privacy | A4 (MFA/policies), A7 (audit), existing ZK core |

**One-command install / HTTPS / Let's Encrypt / Postgres / no-downtime updates**
(§1) land in a dedicated **Ops** workstream owned by a single agent:
`deploy/**`, `Dockerfile`, `docker-compose.yml`, `scripts/`, `docs/SELF-HOSTING.md`,
`docs/build-env-deploy.md`. Disjoint from all Rust/JS code, so it can run alongside
any wave.

---

## 8. Risks / notes

- **Projects model is the highest-risk shared change.** Do 0.1 as a single gate and
  freeze the model before Wave A/B build against it.
- **MFA mandatory** may break existing client logins; land A4 + client changes
  together, gated by the full integration test.
- **`read_secret` gate** (`scripts/check-restricted-api.sh`) must stay intact — the
  CLI and desktop must not leak it into web/extension/SDK.
- **Nightly toolchain** (root `rust-toolchain.toml`) is required for the desktop's
  git-master gpui; new CLI is stable-compatible and builds under the same nightly.
- **New migrations** are append-only numbered files; never edit `0001_init.sql`.
