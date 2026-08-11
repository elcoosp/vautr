Based on our extensive conversation and the five‑layer specification suite we just built, here is the **PRD for Vautr** – a zero‑knowledge, constitutionally protected password manager.

I have synthesised the Vision, BRS, SRS, Architecture, and Test Verification documents into this single PRD. No additional interview was needed.

---

# Vautr Product Requirements Document (PRD)

## Problem Statement

Users of existing password managers face a growing erosion of trust (“enshittification”): companies quietly move features behind paywalls, inject unremovable telemetry, change privacy policies after lock‑in, or make self‑hosting artificially difficult. Even open‑source managers often lack strong governance guarantees, leaving users vulnerable to future commercial pressure.

At the same time, many password managers rely on a server‑side trust model – the server sees encrypted blobs but still holds the keys to decrypt them. A server breach could expose all secrets.

Users want a password manager that is **verifiably zero‑knowledge**, **constitutionally protected** from enshittification, **offline‑first**, and **easy to self‑host** without feature degradation.

## Solution

Vautr is a password manager built on a strict zero‑knowledge architecture: the server never receives plaintext or decryption keys. All cryptography happens on the client. The project is legally bound by a **Constitution** that guarantees:

- AGPL‑3.0 licensing for all core code (no proprietary forks)
- Free, fully featured self‑hosting for individuals and small teams
- Data sovereignty (standard exports, no vendor lock‑in)
- Transparent pricing for the optional cloud service

Vautr provides cross‑platform clients (desktop, mobile, web, browser extension) with offline‑first sync, optimistic concurrency control (OCC), crash‑safe key rotation, and an opaque‑handle pattern that ensures secrets never leak into JavaScript or React Native bridges.

## User Stories

The following user stories are derived from the BRS and SRS. They cover all major capabilities.

1. As a **privacy‑conscious individual**, I want to create a vault with a strong master password, so that my secrets are encrypted before they ever leave my device.
2. As a **privacy‑conscious individual**, I want to unlock my vault using my master password or biometrics (FaceID/TouchID), so that I can access my passwords quickly.
3. As a **privacy‑conscious individual**, I want to add, edit, and delete passwords (title, username, password, URL, notes), so that I can manage my digital identity.
4. As a **privacy‑conscious individual**, I want to search my vault by title, username, or URL, so that I can find items instantly even with thousands of entries.
5. As a **privacy‑conscious individual**, I want to copy a password to the clipboard without the secret ever entering the JavaScript heap (web/mobile), so that I am protected against XSS or malicious extensions.
6. As a **privacy‑conscious individual**, I want the vault to lock automatically after a period of inactivity, so that an unattended device does not expose my secrets.
7. As a **user with multiple devices**, I want my changes to sync automatically when I come online, so that all my devices are up to date.
8. As a **user with multiple devices**, I want offline edits to be queued locally and applied when connectivity returns, so that I can work without internet.
9. As a **user who uses shared devices**, I want the vault to lock and require re‑authentication after 30 seconds in the background (mobile) or 5 minutes of inactivity (desktop), so that others cannot access my secrets.
10. As a **user who suspects their master password is compromised**, I want to rotate my vault key, so that old sessions (e.g., a lost laptop) can no longer read my vault.
11. As a **user who has lost their master password**, I want to recover my vault using a 24‑word BIP‑39 recovery key, so that I don’t lose all my data.
12. As a **user who recovers with the recovery key**, I want to be forced to set a new master password and generate a new recovery key, so that the old recovery key becomes invalid.
13. As a **small team admin**, I want to share a vault item with another user without revealing my master password, so that team members can collaborate securely.
14. As a **small team admin**, I want to revoke a user’s access to a shared item, so that former members can no longer see it.
15. As a **self‑hoster**, I want to run the Vautr server on a Raspberry Pi or any Linux machine using Docker, so that I have full control over my data.
16. As a **self‑hoster**, I want the server to use SQLite (no separate database process), so that administration is simple and resource usage is low.
17. As a **user migrating from another password manager**, I want to import my data from CSV, 1Password `.1pux`, or Bitwarden JSON, so that I don’t have to re‑enter hundreds of passwords manually.
18. As a **user who imports many items**, I want the import to complete in seconds (not minutes) and to bound memory usage, so that my browser or mobile device does not crash.
19. As a **cloud user**, I want a free tier that allows me to use the service without self‑hosting, with the option to pay for higher limits or support.
20. As a **cloud user**, I want to export my entire vault in a standard format at any time, even if I stop paying, so that I am never locked in.
21. As a **developer**, I want a CLI to add, retrieve, and delete items, so that I can script vault operations.
22. As a **browser extension user**, I want the extension to autofill passwords on websites without keeping the vault unlocked in the background, so that security is maximised.
23. As a **user with accessibility needs**, I want the web client to conform to WCAG 2.1 Level AA, so that I can use the interface with screen readers and keyboard navigation.
24. As a **security‑conscious user**, I want the server to return 410 `cursor_expired` if my sync cursor is too old, so that I can perform a fresh sync rather than corrupt my state.
25. As a **user who encounters a conflict**, I want a clear modal asking whether to keep the server version or overwrite it, so that I don’t lose data accidentally.
26. As a **user who ignores a toxic item**, I want the client to remember that decision across restarts, so that I am not prompted repeatedly.
27. As a **user whose device crashes during key rotation**, I want rotation to resume automatically where it left off, so that no items are left unrotated.
28. As a **user who enables telemetry**, I want aggregated metrics sent only every 24 hours, with no PII, so that the project can improve without invading my privacy.

## Implementation Decisions

The following decisions are derived from the Architecture & Design Specification (ADRs) and the SRS.

### Modules to be built (Rust workspace)

| Module | Responsibility |
|--------|----------------|
| `vautr-crypto` | All cryptographic primitives (Argon2id, XChaCha20‑Poly1305, HKDF, OPAQUE client). Zero I/O. |
| `vautr-domain` | Shared data structures (`DecryptedOverview`, `DecryptedSecret`, `CoreError`, etc.). No logic. |
| `vautr-db` | SQLite + SeaORM, FTS5, transaction boundaries (save, batch sync, dashmap persist). |
| `vautr-sync` | Sync engine, DashMap blacklist, OCC resolution, Safety Reaper. |
| `vautr-auth` | OPAQUE registration and login flows (client side). |
| `vautr-keyring` | SVK lifecycle, key rotation, dual‑wrapping (MP and Recovery Key). |
| `vautr-app-state` | Orchestrator, event bus, persistence worker, epoch management. |
| `vautr-ffi` | UniFFI bindings for mobile (Swift/Kotlin). |
| `vautr-wasm` | wasm‑bindgen bindings for web and extension popup. |
| `vautr-server` | Axum HTTP server, SQLite OCC enforcement, rate limiting, file endpoints (RustFS). |

### Key architectural decisions (ADRs)

1. **Server uses SQLite with WAL** – single‑file simplicity for self‑hosters. OCC enforced via `UPDATE ... WHERE version = ?`. (ADR‑001)
2. **Client uses DashMap for local blacklist** – lock‑free, batch‑persisted to SQLite at sync end (no dirty flag). (ADR‑002)
3. **Opaque handle pattern for secrets** – `reveal_secret` returns `u64` handle; `perform_action` uses handle; `read_secret` only available on desktop via `desktop-api` feature flag. (ADR‑003)
4. **Atomic OCC update on server** – no read‑modify‑write. (ADR‑004)
5. **Extension stateless autofill** – service worker uses `vautr-crypto` only (nodejs target), SVK cached in `chrome.storage.session`. Popup uses full client. (ADR‑005)
6. **Crash‑safe rotation via `enc_key_gen` cursor** – client re‑encrypts items where `enc_key_gen < new_gen` in batches; on restart resumes from incomplete batches. (ADR‑006)
7. **Sharing via X25519 ECDH + XChaCha20‑Poly1305** – the SIK is wrapped to the recipient's `SharingPublicKey` using ephemeral X25519 ECDH and an XChaCha20‑Poly1305 envelope (AEAD AD = recipient_sharing_pk ‖ item_uuid). No PQC requirement for v1; ML‑KEM reserved as a post‑1.0 upgrade behind the same `share_item`/`unwrap_shared_item` interface. (ADR‑007)

### API contracts (design‑first)

- OpenAPI 3.1 specification in `packages/api-contract/openapi.json` defines all endpoints.
- Versioning via `Content-Type: application/vnd.vautr.sync.v1+json`.
- Authentication: Bearer token from OPAQUE session.
- Error codes: 400, 401, 404, 410, 412, 422, 429, 5xx.
- Batch push: max 100 items per request; atomic commit per batch; per‑item OCC results returned.

### Schema changes (SQLite)

- Server: `items` table (`uuid`, `user_id`, `version`, `enc_key_gen`, `deleted_date`, `payload`, `updated_at`).
- Client (local):
  - `item_overviews` (hot data, includes FTS5 indexed fields)
  - `item_payloads` (cold BLOB)
  - `sync_meta` (cursor, `min_enc_key_gen`, wrapped SVK)
  - `local_blacklist` (`uuid`, `ignored_version`, `state`)
  - `quarantine` (`uuid`, `target_version`, `quarantine_until`, `retries`)

### Specific interactions

- **Sync pull** – client sends `cursor`; server returns metadata only (no payloads). Client consults DashMap to decide which payloads to download.
- **Payload fetch** – client sends `{uuid, version}`; server returns payload only if version matches; else returns `version_mismatch` with current metadata.
- **Conflict resolution** – on 412, client fetches latest metadata, updates DashMap, shows modal, then either overwrites (with new `If‑Match`) or discards.
- **Key rotation** – client calls `POST /account/rotate-key` first to raise `min_enc_key_gen`. Then iterates items in batches of 100, re‑encrypts, updates local `enc_key_gen`, pushes batch.
- **Emergency recovery** – client decodes BIP‑39 mnemonic, derives `KEK_RK`, unwraps SVK, then forces MP reset and new Recovery Key generation.

## Testing Decisions

### What makes a good test

- Test **external behaviour**, not implementation details. Use the public API of each module.
- For the core, prefer **property‑based tests** (`proptest`) and **concurrency models** (`loom`) over mocks.
- For the sync engine, use **`wiremock`** to simulate server responses (OCC conflicts, network errors).
- For FFI boundaries, use **end‑to‑end tests** with the actual native bridge (Maestro for mobile, Playwright for web) and a test‑instrumented core that exposes drop counters.
- Never test secret material directly in logs or assertions; use `Zeroizing` and drop counters to verify memory cleanup.

### Modules to be tested

| Module | Test types | Prior art / tools |
|--------|------------|-------------------|
| `vautr-crypto` | Unit, property‑based, fuzzing | `proptest`, `cargo-fuzz`, `criterion` benchmarks |
| `vautr-db` | Integration (temp SQLite), transaction rollback | `sqlx::test`, `tempfile` |
| `vautr-sync` | Concurrency (`loom`), `wiremock` for OCC, state machine tests | `loom`, `wiremock`, `tokio::test` |
| `vautr-auth` | Unit + OPAQUE test vectors | Mock OPAQUE server |
| `vautr-app-state` | Persistence worker simulation, epoch gating, crash recovery | `tokio::test` with deterministic time, drop counters |
| `vautr-server` | Integration with SQLite, contract tests (Pact), load tests (k6) | `sqlx::test`, `pact`, `k6` |
| FFI (mobile/web) | E2E with drop counters, unmount release checks | Detox / Maestro (mobile), Playwright (web) |
| Extension | Stateless autofill + service worker termination tests | Playwright with Chrome extension API |

### Specific test scenarios (from verification spec)

- Unlock with correct MP – within 2 seconds.
- Read‑only gate when `local_gen < server_min_gen`.
- Conflict modal appears on 412 with `is_toxic` flag.
- Safety Reaper zeroizes idle handles after 60s.
- Rotation resumes after crash – verified via `enc_key_gen` cursor.
- Import of 1000 items completes in <5 seconds, no OOM.
- Extension autofill works after service worker is terminated and restarted.

## Out of Scope

The following items are **explicitly excluded** from the initial release (v1.0):

- Large file attachments > 100 MB (deferred to post‑v1.0)
- Enterprise SSO / SCIM / directory sync
- Offline account creation (must contact server at least once to register)
- Real‑time push sync (polling only)
- Server‑side password recovery (by design impossible)
- Native Windows 7 / macOS < 11 / iOS < 15 / Android < 8 support
- On‑premise Active Directory integration
- Hardware security key (WebAuthn) as second factor for MP unlock (post‑v1.0)
- Built‑in password health report / dark web monitoring (deferred)
- End‑to‑end encrypted messaging or file sharing beyond vault items
- A mobile SDK for third‑party apps

## Further Notes

- The **Constitution** (see `CONSTITUTION.md`) is a legally binding document that cannot be changed without 80% board approval and 66% contributor vote. This guarantees that the out‑of‑scope items above will never become paywalled or degrade self‑hosting.
- The project uses **AI‑assisted PR reviews** (PR‑Agent) to catch vulnerabilities early, but all critical security changes require human review.
- **Telemetry** is opt‑in, aggregated over 24 hours, and contains no PII. Crash reports never include minidumps or local variables.
- The first vertical slice (MVP) should focus on: vault creation, unlocking, adding one item, local SQLite persistence, and desktop client (GPUI). Sync and key rotation come in the next slice.
- All code must pass the **DCO** (Signed‑off‑by) and the **feature‑flag symbol check** (no `read_secret` in WASM/UniFFI).

---

This PRD is the single source of truth for engineering. All future issues, pull requests, and milestones should trace back to these user stories and implementation decisions.
