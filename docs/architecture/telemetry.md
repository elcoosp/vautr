# Vautr Observability, Telemetry & Crash Reporting Specification

This document defines the exact rules, tools, and architectures for observing the Vautr client in development and production. Operating a distributed, offline-first, Zero-Knowledge system requires strict observability, but traditional logging and crash reporting are massive security liabilities. 

If this specification is not implemented flawlessly, a well-meaning developer, a default Sentry integration, or a high-resolution telemetry timestamp will serialize a `DomainModel`, leak a password, or deanonymize a user to third-party servers.

Deviations from this specification will result in catastrophic PII exposure.

---

## 1. The Zero-Knowledge Observability Boundary

These are hard, immutable rules that override all other observability requirements. They are enforced at the compiler level, the linter level, and the architecture level.

1.  **The No-PII Rule:** Personally Identifiable Information (PII) and Secret Material (Passwords, URLs, Titles, Usernames, Emails, Notes, TOTPs) are strictly forbidden from leaving the device via logs, crashes, or telemetry.
2.  **The Sanitization Contract:** 
    *   `Uuid`s are **SAFE** to transmit in internal crash reports for debugging OCC conflicts.
    *   `CoreError` and `CryptoError` enums are **SAFE** to transmit.
    *   `DecryptedOverview` and `DecryptedSecret` fields are **FATAL** to transmit.
3.  **Compile-Time Enforcement (The `Debug` Ban):** 
    *   `DecryptedSecret`, `Zeroizing<String>`, `SymmetricVaultKey`, and `MasterKey` **MUST NOT** derive the Rust `Debug` trait. If a developer attempts to log these types, the code **MUST NOT COMPILE**.
    *   `DomainModel` and `DecryptedOverview` MAY derive `Debug` only behind `#[cfg(debug_assertions)]`. In release builds, they are opaque.
4.  **Lint-Level Enforcement (Biome):** The `packages/` and `apps/` codebases use Biome. Biome is configured with a strict `noConsoleLog` rule in production builds. All logging must go through the sanitized `vautr-client-sdk` logger, never raw `console.log`, preventing accidental PII leakage in the browser/React Native debug consoles.

---

## 2. Structured Logging (Local & Dev)

Local logging uses the `tracing` crate to provide structured, asynchronous, span-based observability for developers. It is completely disabled in release builds.

### 2.1 Span Hierarchy & Field Skips
Functions processing secrets must explicitly skip secret arguments in span macros.
```rust
#[tracing::instrument(skip(mp, svk))]
pub async fn unlock_with_password(&self, mp: Zeroizing<String>, svk: SymmetricVaultKey) { ... }
```

### 2.2 The `ret` Ban on Secrets
The `#[tracing::instrument(ret)]` attribute is strictly forbidden on any function returning `Zeroizing<String>`, `DecryptedSecret`, or `Result<..., CoreError>` where the Ok type contains secrets. Because `Zeroizing` lacks a `Debug` impl, using `ret` will cause a compilation failure. CI enforces a code review check for this pattern.

### 2.3 Log Levels
*   `error`: Unrecoverable state (DB corruption, OPAQUE protocol abort).
*   `warn`: Recoverable anomalies (Safety Reaper triggered, 412 conflict, `MutationFailed`, `KeyUpdateRequired` gate triggered).
*   `info`: Lifecycle events (SyncStarted, SyncCompleted, VaultLocked).
*   `debug`/`trace`: Detailed flow (DashMap updates, FTS queries). Stripped entirely in release builds.

---

## 3. Crash Reporting (Production Sanitization)

Crash reporting (via Sentry/Bugsnag) is the most dangerous integration. Core dumps and stack traces routinely capture local variable states.

### 3.1 The Hard-Crack Reality & Minidump Ban
If the app hard-crashes (segfault, out-of-memory), the OS immediately intercepts the signal and snapshots the process memory. User-space code like a Sentry `before_send` hook **will never execute**. Relying on pre-send scrubbing is a fatal false promise.
*   **Rule 1: Disable Minidumps:** The Crash Reporting SDK MUST be configured with `attach_minidumps = false` and `capture_local_variables = false`. Only stack traces and breadcrumbs are safe to transmit.
*   **Rule 2: Memory Defense (The `mlock` Mandate):** Since we cannot scrub memory during a crash, we must prevent the OS from writing it to disk where reporters might read it. The Cryptographic Spec's mandate to `mlock()` the `DashMap` pages ensures the OS never pages decrypted secrets to the SSD swap file, protecting them even in the event of a core dump.

### 3.2 Stack Trace Sanitization
*   **Mobile (Swift/Kotlin):** Disable local variable capturing entirely in the SDK configuration to prevent the accidental capture of a `String` that just crossed the UniFFI bridge.
*   **WASM:** WASM crash reports are limited to stack traces. Ensure Biome and bundler plugins strip all `console.log`/`console.error` calls from production JS bundles to prevent leaking payloads.

### 3.3 Breadcrumbs (State-Machine Transitions Only)
Breadcrumbs track what the user did leading up to a crash. Standard UI breadcrumbs ("Clicked on item 'Bank of America'") are PII violations.
*   **Safe Breadcrumb:** `"SyncStarted"`, `"MutationFailed(EpochMismatch)"`, `"VaultLocked"`.
*   **Fatal Breadcrumb:** `"Viewed Item UUID-123"`, `"Copied Password"`. UUIDs are safe in internal crash reports, but correlating them to specific user actions in breadcrumbs provides too much context to third-party servers.

---

## 4. Telemetry & Metrics (Anonymous Health Monitoring)

Telemetry measures the health and performance of the system without tracking user behavior. Per-event telemetry is strictly forbidden, as high-resolution timestamps can be correlated with server logs to deanonymize users.

### 4.1 The 24-Hour Aggregation Mandate
The client MUST NOT stream metrics per-event. Instead, it maintains a local, in-memory metrics buffer. It records counts, sums, minimums, and maximums. This buffer is transmitted **only once every 24 hours** as part of the Daily Heartbeat. This destroys the temporal correlation attack vector.

### 4.2 Aggregated Metrics
*   **Sync Health:** `sync_duration_ms` (Histogram over 24h), `sync_failure_count` (Segmented by `CoreError` type).
*   **Crypto Performance:** `argon2_duration_ms` (Histogram over 24h), `aead_decrypt_duration_us` (Histogram over 24h).
*   **Reaper & Lifecycle:** `reaper_orphans_cleaned` (Counter over 24h), `quarantine_ttl_resets` (Counter over 24h).

### 4.3 Adoption (Strict Anonymization)
*   **No User IDs:** The telemetry server must not receive any linkable user identifier (no email, no OPAQUE session token).
*   **Installation UUID:** On first launch, the client generates a cryptographically random, non-PII `InstallationUuid`. This is persisted locally and naturally rotates on app uninstall/reinstall.
*   **Daily Heartbeat:** The client sends one telemetry ping per day containing *only* the `InstallationUuid`, OS version, and the 24-hour aggregated metrics buffer.

---

## 5. Security Incident & Threat Response

Observability detects active cryptographic or server-side compromises. The telemetry pipeline feeds into automated alerting systems.

### 5.1 Toxic Item Detection (TagMismatch Spike)
If a malicious server or a corrupted rotation causes a spike in `CryptoError::TagMismatch` errors:
*   **Alert:** If `TagMismatch` count > 5% of total sync operations in a 24-hour aggregation window, trigger a P1 alert.
*   **Automated Response:** The Core transitions to the `KeyUpdateRequired` **Read-Only Gate**. It does *not* lock the vault (which would deny the user access to their 95% valid passwords). It disables mutations to prevent accidental data destruction, and prompts the user to re-authenticate to resolve the state mismatch.

### 5.2 Reaper Anomalies (UI Lifecycle Failure)
If the Safety Reaper is firing constantly, the React Native or Web UI is failing to call `release_secret`.
*   **Alert:** If `reaper_orphans_cleaned` > 10 in a 24-hour window per user, trigger a P2 alert.
*   **Response:** This indicates a severe bug in a specific UI component version. The telemetry identifies the `app_version` so the bad release can be rolled back.

### 5.3 Server Breach Detection (OPAQUE Failure)
If the OPAQUE protocol fails in a way that suggests a malicious server (e.g., invalid key exchange, malformed challenges):
*   **Alert:** If `AuthError::InvalidCredentials` spikes in a specific geographic region or server cluster, trigger a P1 security alert.
*   **Automated Response:** The client must refuse to auto-retry the login. It must force a hard-stop, requiring the user to explicitly verify their connection or update the app.
