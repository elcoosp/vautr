# VTRFIX scratch log

Format: `VTRFIX-<id> | <date> | <commit> | PASS/FAIL + notes`

## Summary of work landed (phases 1–5)

See `git log --oneline` for the exact commit trail. Highlights:

- **Phase 1 (Critical)**: OPS-C01, SEC-C01, SEC-C02, SEC-C03, SEC-C04,
  SEC-C05, BUG-C01..C10.
- **Phase 2 (High security)**: SEC-H02..H19.
- **Phase 3 (High bugs)**: BUG-H02..H13.
- **Phase 4A/4B (Medium)**: SEC-M01..M33 (partial), BUG-M01..M20 (partial).
- **Phase 5 (Features)**: FEAT-H01 (SQLite chunks, partial — server side done).
- **Phase 6 (Low)**: SEC-L01, L03, L04, L12, L13.

## Known follow-ups (tracked, not yet done)

- FEAT-H01 client transports (SDK `files.ts`, desktop `api_client` wiring, UI).
- FEAT-H02 Emergency Recovery Kit client flow.
- FEAT-H03 competitor import surfaces per-client.
- FEAT-H04 WebAuthn default-on (`default = ["webauthn"]`).
- SEC-M02 per-user AD binding (needs a one-time re-wrap migration).
- SEC-M03 crypto-agility envelope.
- SEC-M11/M12 rate-limiter per-route buckets.
- SEC-M21..M30 remaining client hardening items.
- Phase 7 TST-01..TST-07 (dedicated test additions).
- Phase 8 DOC-01..DOC-05 (doc rewrite against implemented reality).

## Updated status

### Done since the initial log
- **SEC-M02** per-user AD binding shipped with one-release legacy fallback
- **SEC-M03** crypto-agility envelope with versioned magic/version/suite
- **SEC-M04** UNIQUE-race → 409
- **SEC-M11/M12** per-route rate limits keyed by (class, client)
- **SEC-M15** WebAuthn UV requirement + prod gate + sign-count regression
- **SEC-M30** desktop updater stages to private random path, re-verifies
- **FEAT-H02** Emergency Recovery Kit end-to-end (wasm + SDK + web + extension)
- **FEAT-H03** (partial) — import preview wasm tracked

### Still tracked
- **SEC-M07** OPAQUE KSF params — the opaque-ke 4.1.0-pre.1 `Ksf` trait is
  blanket-implemented for `argon2::Argon2<'_>` with hard-coded defaults; a
  custom impl requires constructing an `Argon2` with custom params inside the
  `Ksf::hash` body. Doable but requires reading the argon2 0.6-rc API.
- **SEC-M10** FEK decoupling — needs a `fek_wrapped` column on the manifest +
  a migration + re-wrap on rotation.
- **SEC-M21..M30** client hardening tails.
- **BUG-M03** FTS transactional rebuild.
- **BUG-M07/M14/M18/M19** — documented.
- **FEAT-M05** passive form detection.
- **FEAT-H03** full pipeline needs a wasm-compatible feature split in
  `vautr-import`.
- Mobile FEAT-H02 UI — needs FFI `sign_recovery_nonce`.

### FEAT-H03 status update

The `pipeline` feature is split out of `vautr-import` (default on). The
`--no-default-features` build does not yet compile because the parser/translate
surface still transitively references pipeline-only types. A full wasm-compatible
split is a follow-up; the parser and translate logic themselves have no DB or
tokio dependency, so the remaining work is mechanical gating.
