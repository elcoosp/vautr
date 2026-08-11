# Vautr — Next Steps & Parallel Swarm Execution Plan

**Author:** Jcode (AI assistant)
**Date:** 2026-08-11
**Inputs:** All files in `docs/` + all source in `core/` reviewed; `cargo test --workspace` green.

This plan tells you **what to build next** and **how to decompose it so a swarm of
agents can work in parallel with the fewest file conflicts.** It is grounded in the
*actual* state of the code, not just the specs.

---

## 1. Current State (verified in the tree)

### 1.1 What compiles & passes
`cargo test --workspace` is green. All 11 Rust crates build.

| Crate | State | Notes |
|-------|-------|-------|
| `vautr-domain` | **Done** | All data types, `Zeroizing`, serde. |
| `vautr-crypto` | **Mostly done** | KDF, AEAD, key-tree, opaque, recovery, sharing primitives, suite registries. |
| `vautr-db` | **Done** | SeaORM entities, FTS5, txns, query layer. |
| `vautr-sync` | **Done (core)** | Engine trait, DashMap, OCC, Reaper stub. |
| `vautr-auth` | **Done (client)** | OPAQUE client state machine. |
| `vautr-keyring` | **Done** | SVK lifecycle, wrap, rotate, recovery unwrap. |
| `vautr-app-state` | **Done (core)** | Orchestrator (612 L), worker, epoch, event bus, handles, sync transport. |
| `vautr-ffi` | **Done** | UniFFI client surface. |
| `vautr-wasm` | **Done** | `wasm-bindgen` client surface. |
| `vautr-files` | **Done (crypto)** | Streaming chunked file encryption, manifest. |
| `vautr-server` | **Substantial** | OPAQUE auth, sync (pull/pull-payloads/push-batch), items (put/delete), account (status/rotate-key). |

**Verified server endpoints present:** `auth/register/start|finish`, `auth/login/start|finish`,
`sync/pull`, `sync/pull-payloads`, `sync/push-batch`, `items/{uuid}` (PUT/DELETE),
`account/status`, `account/rotate-key`.

### 1.2 What is NOT yet built (the real gaps)
These are the highest-value next tasks. Roughly mapped to VTR issues.

| Gap | VTR refs | Notes |
|-----|----------|-------|
| **Import pipeline** (`vautr-import` crate) | 027, 045 | Crate does not exist. CSV/ZIP streaming, rayon encryption, drop/rebuild FTS5, `ImportReport`. |
| **Sharing** (`vautr-sharing` crate + server + client) | 026, 041, 057 | Crypto primitives exist in `vautr-crypto::sharing`; no crate, no server endpoints, no PKI directory, no groups, no vault-creation keypair. |
| **Attachments / RustFS server** | 051 | `vautr-files` does chunk crypto, but no server multipart gateway, no FileTransferWorker in app-state, no `FileTransferProgress` event. |
| **Emergency recovery server flow + Emergency Kit PDF** | 043, 059 | RK crypto + unwrap exist client-side; **no** `account/recover/challenge|verify|complete`, no reclaim/deletion endpoints, no PDF generator, no onboarding proof-of-possession gate. |
| **Server hardening** | 046, 052, 053, 054, 042, 040 | No rate limiting, no WebAuthn 2FA, no audit log, no graceful shutdown, no cursor-expiration (410) handling, no pagination in `sync/pull`. |
| **Client sync completeness** | 047, 050 | Reaper not wired into sync/UI; offline-queue story incomplete. |
| **Telemetry / observability** | 034 | Not present. |
| **Export** | 058 | `export_vault` not present. |
| **UI / apps layer** | 007, 028, 029, 030, 031, 044, 048, 056 | **No `apps/` dir exists.** Only `packages/api-contract` (OpenAPI) exists. No web, extension, desktop (GPUI), mobile (RN/UniFFI), or `packages/ui-logic`. |
| **Quality / ops** | 035, 037, 038, 039, 060, 010 | No load suite, no nightly CI, no E2E, no release workflow, no self-hosting guide, no Docker compose. |

---

## 2. Why Conflicts Happen — the 5 hot files

These files will be touched by *many* features. Protect them explicitly.

| File | Lines | Conflict risk |
|------|-------|---------------|
| `core/vautr-app-state/src/orchestrator.rs` | 612 | Every feature (import, sharing, files, export, offline, reaper) wants a method here. **Assign to ONE integrator.** |
| `core/vautr-server/src/handlers.rs` | 791 | Every new endpoint lands here. **Refactor into modules first** (see Wave A). |
| `core/vautr-server/src/repository.rs` | 356 | Every new DB query lands here. Split by module alongside handlers. |
| `core/vautr-server/migrations/0001_init.sql` | — | Single migration file. **Do not append**; add numbered `0002_..0005_` files, one per domain. |
| `Cargo.toml` (workspace root) | — | Adding new crates (`vautr-import`, `vautr-sharing`) mutates it. **One owner.** |

---

## 3. Execution Waves (dependency-aware)

### Wave A — Parallelism enablers (small, mostly sequential, do FIRST)
These 4 tasks unblock everything else. They must land before the big swarm launches.

- **A1 · Server handler refactor.** Split `handlers.rs` into `handlers/mod.rs` +
  per-domain modules: `auth.rs`, `sync.rs`, `items.rs`, `account.rs`, and (new)
  `files.rs`, `sharing.rs`, `recovery.rs`. Split `repository.rs` the same way.
  Pure move/rename — no behavior change. Tests stay green. *(Unblocks server swarm.)*
- **A2 · Split migrations.** Keep `0001_init.sql`. Add `0002_recovery.sql`,
  `0003_sharing.sql`, `0004_files.sql`, `0005_audit.sql` scaffolds (one owner).
  *(Unblocks recovery, sharing, files, audit agents in parallel.)*
- **A3 · Scaffold new crates.** Create empty `core/vautr-import` and
  `core/vautr-sharing` (safe-stub `lib.rs`, add to `[workspace].members`). One
  owner touches root `Cargo.toml` + `[workspace.dependencies]`.
- **A4 · Extend OpenAPI.** Add recovery, sharing, files schemas to
  `packages/api-contract/openapi.json` (single owner; regenerates TS types).

**Gate:** `cargo test --workspace` green; `pnpm typecheck` (if A4 done) green.

### Wave B — The parallel swarm (each group owns exclusive files)
Dispatch 12 groups in parallel. **Every group has a hard "files I own / files I never touch" contract.**

| Group | Owns | Never touches | Depends on |
|-------|------|---------------|------------|
| **B1 · Import** | `core/vautr-import/**` | app-state, server | Wave A3 |
| **B2 · Sharing** | `core/vautr-sharing/**`, `core/vautr-server/src/handlers/sharing.rs`, `repository/*sharing*`, `0003_sharing.sql` | orchestrator.rs, other handler modules | Wave A1/A2/A3 |
| **B3 · Files server** | `core/vautr-server/src/handlers/files.rs`, `repository/*files*`, `0004_files.sql`; *client worker later* | orchestrator.rs | Wave A1/A2 |
| **B4 · Recovery server + PDF** | `core/vautr-server/src/handlers/recovery.rs`, `repository/*recovery*`, `0002_recovery.sql`; new `vautr-pdf` crate OR a module in `vautr-files` | orchestrator.rs | Wave A1/A2 |
| **B5 · Server hardening** | `handlers/{account,sync}.rs` additions, `middleware.rs`, `db.rs`; `0005_audit.sql` | other handler modules | Wave A1/A2 |
| **B6 · Telemetry** | new `core/vautr-telemetry/**` or module in server; `telemetry.rs` | — | — |
| **B7 · UI-logic + Web** | `packages/ui-logic/**`, `apps/web/**` | — | domain types |
| **B8 · Extension** | `apps/extension/**`, `packages/vautr-client-sdk/**` | — | — |
| **B9 · Desktop (GPUI)** | `apps/desktop/**` | — | — |
| **B10 · Mobile** | `apps/mobile/**`, `core/vautr-ffi/**` additions | — | — |
| **B11 · Export + offline** | new `vautr-export` module in `core/vautr-db` or `vautr-files`; offline queue module | orchestrator.rs | — |
| **B12 · QA/ops** | `.github/workflows/*`, `test/load/*`, `docker-compose.yml`, `Dockerfile`, `docs/SELF-HOSTING.md` | — | — |

**Conflict rules enforced for the whole swarm:**
1. `orchestrator.rs` is **off-limits** to all of B1–B12. The integrator (Wave C) wires features in.
2. Each server group only edits its **own** handler/repository module + its **own** migration file.
3. `Cargo.toml` workspace edits only by Wave A3 (or the integrator).
4. No agent creates `apps/` files outside its own `apps/<name>/` subtree.

### Wave C — Single integration pass (one agent, after B1–B11 gate)
One designated integrator wires the completed features into `orchestrator.rs` /
`VautrClient`:
- `import` → single `ImportCompleted` event + seeding via `SyncEngine`.
- `sharing` → `share_item`, `accept_share`, group mgmt; vault-creation keypair (057).
- `files` → `FileTransferWorker` in app-state; `FileTransferProgress` event (throttled 4/s).
- `recovery` → RK auth gate, forced MP/RK rotation, onboarding proof-of-possession.
- `export` / `offline` / `reaper` hooks.

**Gate:** full integration test `Client A push → Server → Client B pull` (roadmap Phase 4).

---

## 4. Suggested First Dispatch (recommended order)

If you want to start *now* with a manageable swarm, launch **Wave A (4 tasks)**
first — they are small and conflict-free. Once Wave A merges, launch **B1, B2,
B3, B4, B5, B7** together (they touch no shared file). Add **B6, B8–B12** as
capacity allows. Reserve the integrator for last.

### Quick start checklist for each agent
1. Read its spec: `docs/architecture/{feature}.md` + the matching VTR issue.
2. Follow the TDD instructions already written in each `docs/issues/VTR-*.md`.
3. Respect the file-ownership contract (own your files; never touch the hot files).
4. `cargo test --workspace` must stay green on every merge.

---

## 5. Risks / Notes
- **OpenAPI drift:** VTR-004 (TS SDK gen) is unfinished; finish A4 before any client/UI work so UI agents build against a stable contract.
- **`read_secret` gate:** the CI symbol check (`scripts/check-restricted-api.sh`) already exists — keep `desktop-api` feature-gating intact during app work.
- **Migration conflicts:** the single biggest future conflict is `0001_init.sql`; A2 eliminates it.
- **GPUI pinning:** pin to a tested Zed commit (tech-stack.md §4) before B9 starts.
