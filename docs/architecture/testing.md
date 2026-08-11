# Vautr Testing & Verification Strategy

This document defines the rigorous testing, fuzzing, and simulation strategy required to verify the Vautr Zero-Knowledge architecture. Standard unit tests are insufficient for a concurrent, offline-first, cryptographically rigid distributed system. We must prove deterministic behavior under chaos, extreme interleaving, and malicious inputs.

Deviations from this strategy will result in shipped race conditions, silent data loss, and zero-knowledge boundary violations.

---

## 1. Concurrency & State Machine Verification

The Core runs complex, overlapping async tasks. We must prove there are no deadlocks, race conditions, or corrupted states.

### 1.1 Model-Based Concurrency (The `loom` Strategy)
The `sync_epoch` (AtomicU64) and the state machine transitions of the DashMap are highly concurrent. `loom` cannot hook into the internal sharding of a `DashMap` struct; therefore, we must separate pure logic from data structure.
*   **Tool:** `loom` crate.
*   **Strategy:** Extract the pure state transition logic (Epoch checks, DashMap state updates) into a model that uses primitive `loom::AtomicU64` and `loom::AtomicU8` (representing `DashMapState`). Use `loom` to exhaustively test all possible thread interleavings of this model.
*   **Invariant:** Under no interleaving can a worker commit data with a stale epoch, nor can a concurrent 412 resolution destroy a user's "Ignore" intent.

### 1.2 Structural Stress Testing (The `DashMap` Reality)
Once the logic model is proven, we must stress-test the actual `DashMap` struct under heavy async load.
*   **Tool:** `tokio::test` with `tokio::spawn`.
*   **Strategy:** Spawn 100 concurrent tasks performing `reveal_secret`, `perform_action`, and `release_secret` simultaneously against a real `DashMap`. 
*   **Invariant:** The `InUse`/`Idle` atomic state transitions must never deadlock, and the Safety Reaper must correctly skip `InUse` handles.

### 1.3 PersistenceWorker Simulation
We must verify crash consistency and backoff logic.
*   **Tool:** `tokio::test` with deterministic time (`tokio::time::pause()`).
*   **Strategy:** 
    *   **Crash Simulation:** Enqueue 100 tasks. Force-abort the worker on task 50. Verify that on re-instantiation, the worker recovers and processes the remaining 50.
    *   **Epoch Gating:** Enqueue a task, increment the `sync_epoch` asynchronously, then execute the worker. Verify the task is aborted and the Core fires `MutationFailed`.
    *   **SQLITE_BUSY Retry:** Mock the SQLite layer to return `SQLITE_BUSY` 3 times. Verify the worker implements exponential backoff and eventually succeeds, or correctly fires `SaveFailed` on the 4th attempt.

### 1.4 Safety Reaper Race Conditions & Time Advancement
Verify the Reaper's TTL check and zeroization do not race with an active `perform_action` call, and that post-crash cleanups execute correctly.
*   **Tool:** `tokio::test` with deterministic time advancement.
*   **Strategy:** 
    1.  **Active Race:** Call `reveal_secret`, triggering `InUse` state. Advance time past 60s TTL. Trigger Reaper. Assert Reaper *skips* zeroization.
    2.  **Post-Crash Cleanup:** Abruptly `std::mem::drop` the `VautrClient` instance without calling graceful shutdown methods, simulating a sudden loss of scope. Re-instantiate the client. **Advance mock time by 60 seconds.** Trigger the Reaper. Assert the Reaper zeroizes the orphaned entry and fires the `tracing::warn!`.

---

## 2. Cryptographic & Zero-Knowledge Fuzzing

Crypto implementations fail on edge cases, malformed inputs, and malicious servers.

### 2.1 AEAD Fuzzing
Malicious servers must never cause a panic in the decryption layer.
*   **Tool:** `cargo-fuzz` / `proptest`.
*   **Strategy:** Feed random byte sequences into the `open` (decrypt) function of XChaCha20-Poly1305.
*   **Invariant:** The function must *never* panic or unwrap. It must cleanly return `Err(CryptoError::TagMismatch)` or `Err(CryptoError::MalformedCiphertext)` for all invalid inputs, including truncated payloads, altered tags, and zero-length buffers.

### 2.2 Associated Data (AD) Validation
Prove that ciphertexts cannot be swapped between contexts.
*   **Tool:** Custom `proptest` strategy.
*   **Strategy:** 
    1. Encrypt a payload with `AD = construct_ad(uuid1, key_gen1)`.
    2. Attempt to decrypt the *same ciphertext* using `AD = construct_ad(uuid2, key_gen1)` or `AD = construct_ad(uuid1, key_gen2)`.
*   **Invariant:** Decryption must *always* fail with `TagMismatch`.

### 2.3 OPAQUE Integration (Deterministic Aborts)
Test the PAKE flow against a malicious server returning malformed data.
*   **Tool:** Mock OPAQUE server returning invalid challenges.
*   **Strategy:** Simulate a server returning malformed `login_response` bytes.
*   **Invariant:** The client must deterministically abort the protocol without panicking and return `AuthError::InvalidCredentials`. (Note: Statistical timing side-channel analysis requires specialized cryptographic auditing and is outside the scope of functional integration tests).

---

## 3. FFI Boundary & Memory Safety

The bridge between Rust and React Native / WASM is the most dangerous surface.

### 3.1 Opaque Handle Lifecycle Testing
Verify the explicit `release_secret` contract and JS unmount behavior.
*   **Tool:** Instrumented Rust Core + Detox/Maestro (React Native E2E).
*   **Strategy:** 
    *   **Test Mode:** Compile the Rust core with a feature flag that wraps `Zeroizing` drops in an `AtomicUsize` counter.
    *   **E2E Test:** The app reveals a secret, asserts the counter increments. The app navigates away (triggering the `useEffect` cleanup and `release_secret`). The test asserts the counter decrements back to 0.
    *   **Crash Test:** The app reveals a secret and is force-killed. On next launch, advance mock time by 60s. Verify the Safety Reaper cleans the orphaned DashMap entries and the counter decrements.

### 3.2 Type Dictionary Validation
Ensure FFI types never drift across language boundaries.
*   **Tool:** Custom CI script.
*   **Strategy:** Generate UniFFI headers and WASM bindings. Parse the generated Swift/Kotlin/TS files and assert that the field names and types match the exact definitions in the Canonical Data Schema document. Fail CI on any mismatch.

### 3.3 Memory Leak Checks
Verify no secrets survive beyond their intended scope.
*   **Tool:** Instrumented `Zeroizing` drop counter (from 3.1).
*   **Strategy:** Run a sustained stress test (1000 reveals, copies, and rotations). Assert that the `AtomicUsize` drop counter returns to the exact baseline of 0 after all operations complete, proving no `Zeroizing<String>` or `SecretHandle`s are leaking in the Native Module layer. (JS GC heap size is too noisy for this assertion; we rely solely on the Rust drop counter).

---

## 4. Sync Engine & OCC Simulation

The server is untrusted. We must simulate network failures and conflicts.

### 4.1 412 Resolution Simulation
Test the Context-Sensitive Resolution logic.
*   **Tool:** `wiremock` (Rust HTTP mocking).
*   **Strategy:**
    *   **Toxic 412:** Client edits item Y (ignored v5). Mock server returns 412 with metadata (v6, `enc_key_gen=0`). Assert DashMap updates to v6 `ToxicIgnored`.
    *   **User Discard:** User clicks "Keep Local". Assert DashMap *keeps* the entry at v6, preserving the ignore intent.
    *   **User Overwrite:** User clicks "Overwrite". Assert DashMap *removes* the entry.
    *   **Valid 412:** Repeat with `enc_key_gen=3`. Assert DashMap updates to `ValidIgnored` and UI shows the "Restore" badge.

### 4.2 Network Partition Simulation
Test offline mutations, optimistic UI, and eventual sync.
*   **Tool:** `wiremock` with forced network drops.
*   **Strategy:** Client goes offline. User creates 5 items, deletes 2. Client comes online. Assert the `POST /sync/push-batch` is called correctly, and the resulting `OverviewUpserted` events perfectly align the local state.

### 4.3 Crash-Safe Rotation
Verify rotation resumes after a crash.
*   **Tool:** `wiremock` + stateful mock responses.
*   **Strategy:**
    1. Client calls `POST /account/rotate-key` (returns 200).
    2. Client pushes batch 1 (items 1-100, `enc_key_gen=3`). Returns 200.
    3. Client pushes batch 2 (items 101-200). Abruptly `std::mem::drop` the `VautrClient` instance without graceful shutdown, simulating a sudden crash.
    4. Restart app (re-instantiate client). Call `GET /sync/pull`.
    5. Assert: Client correctly identifies items 201+ still at `enc_key_gen < 3` and resumes rotation.

---

## 5. End-to-End UI Integration

Testing the UI state charts against the Core event bus. E2E tests must be strictly deterministic; OS time manipulation and non-deterministic network latency are forbidden.

### 5.1 Draft Recovery (Direct Event Injection)
Test the `MutationFailed` flow to ensure the UI successfully recovers the Draft.
*   **Tool:** Maestro (Mobile) / Playwright (Web) + Test-Only Core API.
*   **Strategy:**
    1. User edits an item and clicks "Save". Core returns `TaskReceipt`.
    2. Instead of relying on a non-deterministic network failure, the E2E test harness calls a test-only Core method `inject_mutation_failed(receipt, error)` which directly pushes `VaultStateUpdate::MutationFailed` into the event bus.
    3. Assert: UI navigates back to the Detail View.
    4. Assert: The text input fields contain the *exact* text the user typed (recovered from the Zustand Draft slice via the injected `receipt`), and a red error banner is visible.

### 5.2 Biometric Gate & Read-Only Mode (Direct Event Injection)
Test the UI reaction to the `KeyUpdateRequired` gate without relying on flaky OS timer manipulation in E2E environments.
*   **Tool:** Maestro / Playwright + Test-Only Core API + Platform Layer Unit Tests.
*   **Strategy:**
    *   **UI E2E Test:** The test harness directly injects `VaultStateUpdate::KeyUpdateRequired` into the event bus. Assert: "Save" button is disabled, Read-Only banner is visible. User can still copy passwords.
    *   **Platform Layer Unit Test (Swift/Kotlin):** The 5-minute background timeout logic is tested exclusively at the OS native layer using mocked OS timers, verifying that the Platform Layer correctly triggers the Core's `lock()` or `release_secret()` methods.

### 5.3 Silent Copy vs View Flows
Verify the exact lifecycle of handles in Copy vs View scenarios.
*   **Tool:** Instrumented Core + UI Automation.
*   **Strategy (Silent Copy):** User taps "Copy" on a masked field. Assert `reveal_secret` is called, followed *immediately* by `perform_action` and `release_secret`. Assert the Native Overlay (View) *never* mounts.
*   **Strategy (View):** User taps "Eye" icon. Assert `reveal_secret` is called. Assert Native Overlay mounts. After 30s, assert visual mask applies. Assert `release_secret` is *not* called until the user navigates away.

---

## 6. Test Execution Matrix

| Category | Tool | Run Frequency | Failure Stance |
| :--- | :--- | :--- | :--- |
| **Concurrency Model** | `loom` (Primitive Atomics) | Every PR | Block Merge |
| **Structural Stress** | `tokio` (DashMap/Worker) | Every PR | Block Merge |
| **Crypto Fuzzing** | `cargo-fuzz` / `proptest` | Nightly CI | Block Release |
| **FFI Lifecycle** | Detox / Maestro + Drop Counter | Every PR | Block Merge |
| **OCC / Sync Sim** | `wiremock` + `tokio::test` | Every PR | Block Merge |
| **Memory Leaks** | Drop Counter Baselines | Weekly CI | Block Release |
| **E2E UI Flows** | Maestro / Playwright (Event Injection) | Nightly CI | Block Release |
| **OS Lifecycle Logic** | Swift/Kotlin Unit Tests | Every PR | Block Merge |
