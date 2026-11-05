# VTRFIX-LOG-2 — second full-repo audit + fix wave (2026-11-05)

A second, independent deep audit of the whole repository (all Rust crates,
all TS apps/packages, infra, CI, deploy, docs) located a batch of issues the
first pass missed or mis-reported. This log records what was found and what
landed. Format mirrors `VTRFIX-LOG.md`.

## Verification status

- `cargo test --workspace --exclude vautr-desktop` — all suites pass
  (desktop is excluded in this sandbox: it builds against git-master `gpui`
  with a pinned nightly and needs system UI libraries; its changes were
  syntax-verified via rustfmt and mirror existing API usage).
- `cargo test -p vautr-server` — 78 passed (7 new/updated tests).
- `pnpm -r typecheck` — all projects pass (wasm-glue artifacts are build
  outputs; regenerate with `pnpm prepare:wasm` in a full toolchain).
- SDK / ui-logic / api-contract / mobile unit tests — pass.

## Critical (fixed)

- **FIX-C01 — `rotate_key` bricked MP unlock** (orchestrator.rs): the rotated
  SVK blob was wrapped under a KEK derived from the NEW SVK; the next
  `unlock_with_password` derives the KEK from the MP, so the first rotation
  made the account permanently un-unlockable. Rotation now requires an
  MP-unlock (caches the MK, cleared on lock / raw-key unlock) and wraps under
  `derive_kek(mk)`. Regression test: fresh client re-unlocks with the same
  MP + the rotated blob.
- **FIX-C02 — rotation pushed `target_version = enc_key_gen` and discarded
  outcomes**: the server OCC only applies `version = target_version`, so every
  previously-edited item 412'd silently and the server kept old-generation
  ciphertext behind an advanced `min_enc_key_gen` (undecryptable data loss for
  every other device). Rotation now pushes at each item's real version
  (`list_rotation_items`), and every outcome is handled — on Conflict the
  server's current payload is fetched, re-encrypted, and re-pushed.
- **FIX-C03 — server tombstones never applied locally** (orchestrator sync
  loop): deleted overviews were pulled then dropped (`uuids_to_delete` was
  always empty) — items resurrected locally. Pages now upsert + delete
  atomically (`persist_synced_page`) and publish `OverviewDeleted`.
- **FIX-C04 — local deletes never reached the server**: the transport's
  tombstone shape (`payload: None`) had no producer. New `pending_deletes`
  table (client migration 002); `commit_delete` records uuid+version+gen in
  the same txn; `push_local_changes` flushes real tombstones and clears them
  only on full `Applied`.
- **FIX-C05 — web stored project-secret plaintext as base64**
  (`_authed/projects/$uuid.tsx`): zero-knowledge violated; now encrypts via
  the SDK `encryptSecretValue` and reveals via `mlp` + `decryptSecretValue`
  (extension's proven path). Same reveal fix in `secrets.tsx`.
- **FIX-C06 — mobile created secrets in plaintext**
  (`_app.projects.$projectId.secrets.new.tsx`): the raw value was uploaded as
  `value_ciphertext`. Mobile secret creation is now hard-gated with an
  explanatory notice until native encryption is wired (no silent plaintext).
- **FIX-C07 — `server_public_key` contract repaired end-to-end**: the server
  now mints + persists a 32-byte public key, returns it in
  `register/start`, and rejects stubs/mismatches on `register/finish`. Every
  client (SDK, desktop, CLI, FFI, wasm live test, extension live e2e) echoes
  it. This also removes the CLI's old behavior of serializing an OPAQUE
  `ServerSetup` — which CONTAINS the OPRF PRIVATE KEY — into that field.

## High (fixed)

- **FIX-H01 — machine-token scoping bypass**: `auth_any` discarded token
  scopes + project binding, so any machine token ran with its owner's full
  authority. Scope + binding checks now enforced on secrets list, sync pull,
  projects list (403 `missing_scope` / `project_mismatch`).
- **FIX-H02 — cross-tenant upload-manifest takeover**: `upsert_file_manifest`
  re-owned other users' in-flight uploads (`ON CONFLICT DO UPDATE`).
  Now `DO NOTHING` + 409 on owner mismatch.
- **FIX-H03 — `upload_complete` flipped uploads to `Available` without
  checking chunk completeness**; now verifies the uploaded-chunk count.
- **FIX-H04 — recovery user enumeration**: `recover_verify` returned distinct
  errors per failure mode; all now return one uniform error.
- **FIX-H05 — recovery + reclaim tokens stored in plaintext** (replicated by
  litestream): hashed like every other token (migration 0022 + repo changes).
- **FIX-H06 — Emergency Kit dead on arrival (RK AD mismatch)**: wrap sites
  bound AD to `Uuid::nil()`, unwrap sites used the real server user id.
  `recover_svk` now tries both (documented convention in `vautr-keyring::wrap`).
- **FIX-H07 — desktop "restore saved session" could never succeed**: treated
  the MP-wrapped SVK blob as a raw key; now unwraps under the MP-derived KEK.
- **FIX-H08 — desktop auto-lock/escape dead**: the key-handler canvas only
  mounted during the feature tour, so the idle clock never ticked (fixed
  5-minute lock regardless of activity). Registration canvas is now
  unconditional; VTR-079's paint-phase requirement preserved.
- **FIX-H09 — wasm wrappers missing recovery/AD exports**: web + extension
  wasm.ts wrappers now re-export `sign_recovery_nonce_js`,
  `recovery_public_key_js`, `recover_svk_js`, `wrap_svk_with_ad_js`,
  `unwrap_svk_with_ad_or_legacy_js` (recovery-kit login + M02 re-wrap work);
  `signRecoveryNonce` now base64-encodes the nonce.
- **FIX-H10 — extension onboarding crash (VTR-081 never ported)**: componentKey
  + registry ported from web; Emergency-Kit handoff fixed in web + extension
  (`__vautrPendingKit` consume-once helper replaces the removed sessionStorage).
- **FIX-H11 — extension autofill/lock hygiene**: `clearCiphertextCache` used a
  key the store never wrote (`vautr:ciphertexts` vs `vautrCiphertext`);
  `secureLock()` existed but was never called. Both fixed.
- **FIX-H12 — file permission hygiene (desktop/FFI/CLI)**: plaintext vault
  export, Emergency Kit HTML, `config.json`, local vault DB and FFI
  `<vault>.account.json` were written world-readable (or non-atomically); all
  now 0600/0700 + atomic, kit uses `create_new` + timestamp; FFI now seals the
  local mnemonic under the KEK.
- **FIX-H13 — `vautr-auth` OPAQUE wrappers panicked on malformed server
  responses**; all four now return `Result<_, AuthError>`; `Session::Debug`
  redacts the bearer token.
- **FIX-H14 — `Debug` on secret-bearing domain types** (telemetry.md §1.3
  Debug Ban): `DecryptedSecret`/`TotpSecret`/`CustomField`/`DomainModel`/
  `RevertibleState`/`SaveCommand` now have redacting manual impls.
- **FIX-H15 — conflict loop (SDK)**: `resolveConflict('pushLocal')` re-pushed
  the same version forever; pending items are now re-based on the server's
  conflict echo (regression tests added).
- **FIX-H16 — infra hard-fails**: compose healthchecks probed an
  unauthenticated route (containers permanently unhealthy → caddy never
  started); prod compose backup service had duplicate `volumes:` and S3
  credentials under the wrong key; Caddyfile `header_up` sat at site level
  (config refused to load); litestream `:-` default unsupported. All fixed;
  HSTS added.

## Medium / Low (fixed)

- Files: `lock()` now clears all secret handles; `read_secret` gained the
  locked gate (desktop-api); engine `push_batch` rejects outcome-count
  mismatches; unknown push statuses surfaced; `get_secret_material` /
  `list_local_items` NULL-payload rows fixed (INNER JOIN / Option decode);
  `apply_sync_batch_txn` cursor update fails loudly; sync cursor is now
  persisted per page and restored at unlock; `created_at` preserved on edit;
  `migrate::run_migrations` consults the bookkeeping table (M02 closed);
  1PIF import maps username/password/totp/notes/URL (was: blank rows) with
  regression test; CSV formula-injection sanitizer actually wired into the
  writer; X25519 `was_contributory()` checked on both share sides; dead
  `next_batch` + deprecated panic wrapper deleted; `resolve_conflict(keep_server)`
  fetches the server payload instead of delete+resync (data loss);
  `wrap_fek`/`resolve_fek` propagate errors and diagnose pre-rotation
  attachments; SSE re-validates the session every 5 min; `LOGIN_STATE` uses
  per-entry TTL; duplicate registration → 409; `purge_expired_sessions`
  called at login; `GET /backup` admin-gated; bind failure fails fast;
  security-headers middleware actually wired; rate-limiter poisoned-mutex
  fix; machine-account token prefix slicing fixed; CLI `--value` warns; CLI
  `run` threads `Zeroizing`; SDK audit list no longer double-encodes the
  query; SSE floating promise + reconnect backoff fixed; worker origin check
  strict; `unlock()` clears read-only state; floating promises catch toasts;
  mobile: biometrics fail-closed, shares loop + demo payloads gated, CSPRNG
  polyfill removed (loud over silent), kit reveal behind biometrics with
  clipboard auto-clear, real idle-lock via AppState, tour replay via
  DeviceEventEmitter, `textContentType` fixed, logout wipes all SecureStore
  keys, duplicate TourOverlay removed; `@vautr/native-legacy` rename ends the
  two-package ambiguity; gen-ffi-bindings mirrors into the linked
  `packages/native`; Makefile fixed (tab indentation, OS-gated ios-sim);
  justfile stale copy step removed + generic simulator destination;
  seed_db path aligned; nightly-e2e polls `/health`; Dockerfile no longer
  bakes `VAUTR_DB_URL`; Postgres profile requires `POSTGRES_PASSWORD`;
  `verify-isolation.yml` gpui check targets a real file; CLIENT_PARITY.md
  reconciled with reality.

## Known residue (tracked, not silently dropped)

- **Desktop GPUI compile** — requires the pinned nightly + git-master gpui +
  system UI libs; changes were pattern-matched against existing usage and
  rustfmt-parse-verified. Run `cd apps/desktop && cargo check` in a full
  desktop toolchain before releasing.
- **Wasm artifacts** — `apps/web/wasm-pkg*` / `apps/extension/sw-wasm-pkg*`
  are gitignored build outputs; run `pnpm prepare:wasm` (wasm-pack) before
  `pnpm -r typecheck` on a fresh clone (pre-existing behavior).
- **UniFFI generated bindings** — committed Swift/Kotlin bindings remain
  stale vs the Rust FFI surface (recovery-kit functions are called from the
  TS/mobile bridge layer, not the generated files). `scripts/gen-ffi-bindings.sh`
  now mirrors fresh bindings into `packages/native`; regenerate in a full
  toolchain (`cargo run -p vautr-ffi --example gen_bindings`).
- **FEK re-wrap on rotation (M10 completion)** — attachments created before a
  key rotation are not decryptable after rotation (no server manifest-list
  API exists to sweep them). `resolve_fek` now fails with a clear, diagnosable
  message instead of an opaque TagMismatch.
- **Telemetry stack** — the crate is consent-gated and tested but remains
  unwired end-to-end (no orchestrator integration / server heartbeat route),
  as documented in telemetry.md.
- **Push version churn** — `push_local_changes` intentionally pushes the full
  local vault (seeding semantics); a dirty-set is future work.
- **openapi.json drift** — the frozen contract predates several implemented
  routes (recovery/rekey/webauthn/groups). Contract regeneration is a
  follow-up; the server is the source of truth.
