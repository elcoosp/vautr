# Vautr Implementation Roadmap & Milestone Specification (v3.0 - The Exhaustive Blueprint)

This document defines the exact sequence of engineering execution for the Vautr ecosystem. This roadmap is explicitly architected for **maximum parallelism**, designed to be executed by multiple autonomous AI coding agents and human engineering pods simultaneously. 

> **MLP v1 target:** The product scope to ship is defined in
> [`mlp-scope.md`](./mlp-scope.md) (authoritative English scope), with its
> conflict-free parallel execution plan in [`mlp-wave-plan.md`](./mlp-wave-plan.md).
> The phases below are the delivery vehicle for that MLP v1 scope.

A distributed, zero-Knowledge system cannot be built horizontally; it must be built vertically, proving the hardest constraints first. However, vertical slicing often creates merge conflicts and dependency bottlenecks. This specification solves that by enforcing a **Contract-Driven, Skeleton-First** methodology. Interfaces are locked before implementations begin, allowing agents to work in isolated swimlanes with zero blocking.

The previous version of this document underspecified the massive scope defined in our 18 architectural specifications. This version maps every specification—Agile Crypto, RustFS, SQLite, Recovery PKI, Sharing KEM, File Import—into precise, gated phases.

Deviations from this phase sequence will result in broken FFI boundaries, divergent API contracts, and unrecoverable merge conflicts.

---

## 1. Principles of Execution (The AI Agent Protocol)

1.  **Contract-Driven Development:** Before any logic is written, all traits, structs, and API signatures (The Skeleton) are generated and merged. Agents code against empty stubs, eliminating "waiting on dependency" bottlenecks.
2.  **API-First Design:** The OpenAPI specification is the single source of truth, hand-written in Phase 0. Server and Client code is generated from or validated against it, never the reverse.
3.  **Safe Stubs Only (The `todo!()` Ban):** Agents do not use `todo!()` in cross-crate stubs. A `todo!()` macro causes a runtime panic, which will crash an autonomous agent's test suite. All Phase 0 stubs must return safe, non-panicking defaults (e.g., `Ok(Default::default())` or `Err(CoreError::NotImplemented)`).
4.  **Workspace Isolation:** Agents are assigned to specific Cargo crates or Turborepo packages. An agent working on `vautr-crypto` is strictly forbidden from modifying `vautr-domain` or `apps/mobile`.
5.  **Halt and Escalate Protocol:** If an agent fails a Phase Validation Gate, it must immediately halt execution, revert its unmerged local commits, and flag the blockage for human architectural intervention. Agents must not attempt to autonomously rewrite core specifications to pass failed fuzzing or concurrency gates.
6.  **Visual UI Isolation:** To prevent cross-platform merge conflicts, shared UI packages contain *only* logic hooks (e.g., `useVaultState`). Visual components are strictly implemented locally within their respective `apps/` directories.

---

## 2. Phase 0: The Contract & Skeleton (Sequential Foundation)

This phase establishes the monorepo, the build pipeline, and locks every single interface. **No logic is written.** This must be completed by a human architect before agents are dispatched.

*   **Scope:** Build System, CI Pipeline, Type Dictionaries, API Contracts, Trait Definitions, Agile Registries.
*   **Actions:**
    1.  Initialize Turborepo and Cargo workspaces.
    2.  **Handwrite the OpenAPI 3.1 spec** based on the Server API Contract document. Save to `packages/api-contract/openapi.json`.
    3.  Generate the Server route stubs and Client TS SDK (`vautr-client-sdk`) from the OpenAPI spec using `openapi-typescript` and Zod.
    4.  Generate `vautr-domain` purely from the Canonical Data Schema (all structs, enums, `DashMapState`, zero logic).
    5.  Define the `VautrClient` trait in `vautr-app-state` with safe, non-panicking stubs returning `Err(CoreError::NotImplemented)`.
    6.  **Agile Crypto Lock:** Define the Decoupled Suite Registries (`DataSuite`, `SharingSuite`, `KDFSuite`) in `vautr-crypto` as enums. Define the minimal 3-byte and Sharing Envelope formats.
    7.  Setup `packages/ui-logic`: Shared Zustand stores, event bus subscriptions, and Draft recovery hooks. No visual components.
    8.  Configure CI (Biome, Clippy, FFI Symbol Check, Type Dictionary Drift Check).
*   **Parallelism:** None. This is the foundation.
*   **Validation Gate:** CI pipeline is green. Empty desktop/mobile/web apps compile and render a "Hello Vautr" screen using the `VautrClient` stub.

---

## 3. Phase 1: Pure Cryptography & Domain (Highly Parallelizable)

With the skeleton and safe stubs locked, agents implement the foundational mathematical crates. These crates have zero I/O and zero network dependencies.

*   **Swimlane A: Agent Core-Crypto** (`vautr-crypto`)
    *   **Task:** Implement Argon2id, XChaCha20-Poly1305, HKDF, OPAQUE client wrappers, AD binding logic, and the **Decoupled Suite factories**. Implement Dual-Wrapping (KEK_MP, KEK_RK, Ed25519 Auth Key).
    *   **Gate:** `cargo-fuzz` passes on AEAD. `proptest` passes on AD context separation. Envelope parsing tests pass.
*   **Swimlane B: Agent Sharing-Crypto** (`vautr-crypto` / `vautr-sharing` - new crate)
    *   **Task:** Implement X25519 KEM-DEM logic, Group SIK wrapping, and Ephemeral Key generation.
    *   **Gate:** Unit tests pass for SIK encapsulation/decapsulation. 
*   **Swimlane C: Agent Client-DB** (`vautr-db`)
    *   **Task:** Implement SeaORM 2.0 entities (Hot/Cold split), FTS5 raw SQL migrations, and the atomic transaction boundaries (Save, Batch, DashMap wipe).
    *   **Gate:** `tokio::test` WAL crash recovery passes. Drop counter baseline = 0.
*   **Swimlane D: Agent Server-DB** (`vautr-server` DB layer)
    *   **Task:** Implement the SQLite schema (sqlx), WAL mode tuning, `sqlx::SqlitePool` setup, and the strict OCC enforcement logic (`UPDATE ... WHERE version = ?`).
    *   **Gate:** SQLite concurrent write test passes. WAL checkpoint test passes.
*   **Parallelism:** Maximum. Agents A, B, C, and D never touch the same files and rely only on `vautr-domain`.

---

## 4. Phase 2: State Machines, Sync & Import (Parallelizable)

This phase connects the Phase 1 implementations to the state machines and local persistence.

*   **Swimlane A: Agent Auth** (`vautr-auth`)
    *   **Task:** Implement the OPAQUE registration/login flows against the `vautr-crypto` stubs.
    *   **Gate:** Deterministic abort test passes against malicious server mock.
*   **Swimlane B: Agent Sync** (`vautr-sync`)
    *   **Task:** Implement the Sync Engine, DashMap logic, Context-Sensitive 412 resolution, and the Validated Reaper.
    *   **Gate:** `wiremock` OCC simulation passes. `loom` model passes for DashMap interleavings. Reaper TTL reset test passes.
*   **Swimlane C: Agent Import** (`vautr-import` - new crate)
    *   **Task:** Implement the Streaming Pipeline, BIP-39 validation, Translation Layer, Size-Aware Rayon Encryption, and the FTS5 Drop/Rebuild strategy.
    *   **Gate:** 1000-item mock CSV import completes in <5 seconds. OOM test passes on malformed ZIPs.
*   **Parallelism:** High. Auth, Sync, and Import are isolated.

---

## 5. Phase 3: The Untrusted Relay Server (Parallelizable)

This phase builds the Axum server infrastructure and RustFS integration, consuming the Server-DB and Crypto stubs.

*   **Swimlane A: Agent Server-Infra** (`vautr-server` HTTP layer)
    *   **Task:** Implement Axum routes, OPAQUE server flows, Redis rate limiting, and SQLite WAL checkpointing task. **Strictly validate** routes against the OpenAPI spec.
    *   **Gate:** `k6` load test verifies SQLite OCC under concurrency. 422 epoch rejection works. OpenAPI spec validation passes.
*   **Swimlane B: Agent File-Server** (`vautr-server` RustFS integration)
    *   **Task:** Implement RustFS presigned URL generation, Multipart Upload commit/revoke logic, and the Orphan Janitor task.
    *   **Gate:** Presigned URL generation test passes. Janitor purges orphaned chunks successfully.
*   **Parallelism:** High.

---

## 6. Phase 4: The Integration Phase (Sequential Bottleneck)

This phase wires the state machines together into the `VautrClient` and generates the FFI boundaries. It must consume the completed Phase 1, 2 & 3 work. To prevent intractable merge conflicts, this is assigned to a single entity.

*   **Scope:** `vautr-app-state`, `vautr-ffi`, `vautr-wasm`, `vautr-keyring`.
*   **The Integrator Agent (or Human Architect):**
    1.  Swap out the safe stubs in `vautr-app-state` for real implementations (Sync, DB, Crypto, Import).
    2.  Implement the Safety Reaper and Opaque Handle DashMap logic.
    3.  Implement the FileTransferWorker for RustFS uploads/downloads.
    4.  Implement Emergency Recovery logic (Dual-unwrap, Ed25519 auth bridge).
    5.  Generate UniFFI headers and WASM bindings.
    6.  Build the React Native Turbo Module wrappers and the Web Worker postMessage bridge.
*   **Parallelism:** None. Single Integrator.
*   **Validation Gate:** 
    *   `test-instrumentation` gate passes.
    *   **CI Symbol Check strictly verifies:** `read_secret` is absent from WASM and UniFFI artifacts.
    *   UniFFI drop counter decrements to 0 on component unmount.
    *   End-to-end local sync test passes (Client A pushes to Server SQLite, Client B pulls).

---

## 7. Phase 5: UX Logic & Hooks (Highly Parallelizable)

Before visual UI is built, the state management and logic hooks must be finalized so UI agents have a stable API.

*   **Swimlane A: Agent UI-Logic** (`packages/ui-logic`)
    *   **Task:** Implement Zustand stores, Event Bus subscriptions, Draft Recovery logic, and Import Progress tracking.
*   **Swimlane B: Agent UI-SDK** (`packages/vautr-client-sdk`)
    *   **Task:** Finalize the Promise-based TS SDK wrapping the Turbo Module/WASM Worker.
*   **Parallelism:** High.
*   **Validation Gate:** TypeScript compiles without errors. Zustand state transitions match UX State Charts.

---

## 8. Phase 6: Isolated UX Layer (Highly Parallelizable)

With the Core and FFI bridge locked, the UI teams build in maximum parallelism. They consume `packages/ui-logic` and `vautr-client-sdk`, but implement visual components locally.

*   **Swimlane A: Agent Mobile** (`apps/mobile`)
    *   **Task:** Implement React Native Lock Screen, List View, Detail View (Opaque Handles), OS Keystore Biometrics, and Stateless Extension Autofill.
*   **Swimlane B: Agent Web** (`apps/web`)
    *   **Task:** Implement React Web App using shared hooks. Implement Web Worker WASM bridge.
*   **Swimlane C: Agent Extension** (`apps/extension`)
    *   **Task:** Implement Manifest V3 popup (reusing Web logic hooks) and the **Stateless Autofill Service Worker** (Node WASM target).
*   **Swimlane D: Agent Desktop** (`apps/desktop`)
    *   **Task:** Implement GPUI Rust interfaces, directly consuming `desktop-api` features (including `read_secret` and Local File Transfer).
*   **Parallelism:** Maximum. All UI agents work in isolated `apps/` directories.
*   **Validation Gate:** Maestro/Playwright E2E tests pass using direct Event Injection. Biome `noConsoleLog` passes. UI unmount hooks correctly trigger `release_secret`.

---

## 9. Phase 7: Production Hardening & Deployment (Parallelizable)

The final operationalization phase. The system is feature-complete; we now harden it for hostile environments.

*   **Swimlane A: Agent Observability**
    *   **Task:** Integrate Sentry (Minidumps OFF, local vars OFF). Implement 24-hour Telemetry aggregation and Daily Heartbeat. Configure Biome enforcement.
    *   **Gate:** Daily heartbeat transmits successfully. Reaper alert triggers on high orphan count.
*   **Swimlane B: Agent Infrastructure**
    *   **Task:** Finalize Docker builds, Helm charts, and Vercel/Cloudflare deployments. Implement `wasm-opt -Oz` for release builds. Configure RustFS IaC lifecycle policies.
    *   **Gate:** Artifacts build and deploy to staging.
*   **Swimlane C: Agent Security Audit**
    *   **Task:** Run `k6` server load tests. Verify `mlock` prevents swap paging. Run OPAQUE timing side-channel analysis. Verify Agile Crypto fallback logic.
    *   **Gate:** No PII leakage in crash reports. Load tests sustain target RPS.
*   **Parallelism:** High.
*   **Final Validation Gate:** All Nightly CI checks pass. App Store / Chrome Web Store submissions approved. Vautr is live.
