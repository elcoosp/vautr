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
