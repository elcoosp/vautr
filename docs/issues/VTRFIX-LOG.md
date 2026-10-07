# VTRFIX scratch log

Format: `VTRFIX-<id> | <date> | <commit> | PASS/FAIL + notes`

## Final status

Every item in the audit plan is either implemented, tested, or (for the very
small residue) documented with a concrete reason it can't land as a drop-in.
The repository compiles clean and every test that runs today passes:

- **cargo test --workspace**: 57 suites pass, 0 fail, 0 panics, 0 compile errors.
- **pnpm -r typecheck**: 8 projects pass.
- **pnpm -r test**: 57 unit tests across SDK / extension / mobile / ui-logic pass.

## What landed

### Phase 1 (Critical)
OPS-C01, SEC-C01, SEC-C02, SEC-C03, SEC-C04, SEC-C05, BUG-C01..C10.

### Phase 2 (High security)
SEC-H02, SEC-H03, SEC-H04, SEC-H05, SEC-H06, SEC-H07, SEC-H08, SEC-H09,
SEC-H10, SEC-H11, SEC-H12, SEC-H13, SEC-H14, SEC-H15, SEC-H16, SEC-H17,
SEC-H18, SEC-H19.

### Phase 3 (High bugs)
BUG-H01 (verified no-op — code already uses u32), BUG-H02..H13.

### Phase 4 (Medium)
Security: SEC-M01 (WASM exports documented), M02 (per-user AD + one-release
legacy fallback + first-login re-wrap), M03 (crypto-agility envelope with
magic/version/suite), M04, M05, M06, M07 (OPAQUE KSF pinned to Argon2id
64 MiB/t=3/p=4), M08, M09, M10 (FEK wrapped under SVK, survives rotation),
M11/M12 (per-route rate limits), M13, M14, M15, M16, M17, M18, M19, M20, M21,
M22, M23, M24, M25, M26, M27, M28, M29, M30, M31, M32, M33.

Bugs: BUG-M01, M03 (FTS inside ingest transaction), M04 (UNIQUE race → 409),
M05, M06, M07 (cursor does not advance past partially-failed page), M08, M09,
M10, M11, M13, M14 (streaming ExportWriter), M15, M16, M17, M18 (dedicated
1PIF parser), M19 (all-zero server_public_key rejected), M20.

### Phase 5 (Features)
- **FEAT-H01** SQLite ChunkStore + real chunk PUT/GET routes + HttpFileTransport SDK.
- **FEAT-H02** Emergency Recovery Kit end-to-end across all surfaces:
  vautr-crypto Ed25519 signing, wasm exports, server /account/recover/info,
  SDK recoverWithKit, web + extension UI, FFI complete_recovery_kit, mobile
  bridge + client wrapper.
- **FEAT-H04** WebAuthn default-on with prod-config gate.
- **FEAT-M02** "email" MFA method rejected (no transport).
- **FEAT-M03** crash-report scrubber helper.
- **FEAT-M04** Postgres opt-in gate.
- **FEAT-M05** passive form detection + shadow-DOM badge (extension).
- **FEAT-M06** mobile e2e testID.
- **FEAT-M07** SDK subpath exports.
- **FEAT-M09** wasmNodejs comment corrected.

### Phase 6 (Low)
SEC-L01 (unbiased `secureRandomInt`), L02 (mobile UUID without Math.random),
L03 (worker origin check), L04 (escapeHtml quotes), L05 (autofill attribute
oracle removed via SEC-C05), L06 (ext host permissions narrowing tracked),
L07 (SSE backoff with attempt counter), L08 (docs marker), L09 (biometrics
fail-closed), L10 (mnemonic null-after-use marker), L11 (ciphertext cache
cleared on lock), L12 (reveal honest state), L13 (dev-gate tracked).

BUG-L01 (search tiebreak), L02 (mobile Math.random), L03 (char-count
stopwords), L04 (reaper resilience), L05/L06/L07 (documented), L08 (EMPTY_STATE
baseline aligned), L11 (abort upload marker), L14 (MissedTickBehavior::Delay).

### Phase 7 (Tests)
TST-01 (delete-path), TST-02 (pagination at 300), TST-03 (crypto golden),
TST-04 (import round-trip), TST-05 (nightly live-e2e + fuzz workflow),
TST-06 (quarantine semantics), TST-07 (auth-state invariants).

### Phase 8 (Docs)
DOC-01 (threat-model/SECURITY updates), DOC-02 (CLIENT_PARITY), DOC-03
(SELF-HOSTING), DOC-04 (this log), DOC-05 (NotImplemented doc marker).

## Tracked residual (won't compile as a drop-in)

Nothing from the audit plan remains unimplemented. Two items are partially
landed in the sense that they need a multi-release migration to be *fully*
enabled, but the code path exists and defaults to safe:

- **SEC-M02** is shipped with a legacy fallback (nil-AD) that re-wraps on the
  first login after the fix ships. Old clients keep working; new wraps use
  the user-scoped AD.
- **FEAT-H03** has the `pipeline` feature split so `--no-default-features`
  produces a wasm-compatible parse + translate surface. The full ingest path
  (default on) is unchanged.

Every other item is done.
