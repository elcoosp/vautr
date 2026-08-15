# Vautr Issues — done / open / closed split

This folder was reconciled against the actual codebase on **2026-08-13**, with a
follow-up correction on **2026-08-14**.

The original flat `docs/issues/VTR-*.md` set had 65 issues, every one titled
"What to build" with no status, and a `blocked_by` graph that chained everything
off VTR-001 (monorepo) / VTR-003 (OpenAPI). In reality the product is
feature-complete: all 65 issues through **VTR-065** are implemented in code
(verified by source grep) and carry `status: done (2026-08-13)` in `done/`.

The later batch **VTR-066..070** (web/extension sharing, desktop sharing, mobile
uniffi FFI + native bridges, sea_orm 2.0.2 migration, group-sharing UI parity,
desktop vault export + group-sharing UI) lives in `closed/` — these were tracked
live during implementation rather than through the flat `done/` set.

## Layout
- `done/` — VTR-001..VTR-065. Acceptance criteria satisfied by code that exists
  (verified by source grep, not by assertion). Each file has a
  `status: done (2026-08-13)` line. Kept for history/audit; do not re-implement.
- `closed/` — VTR-066..VTR-073. The most recent workstream batches, tracked live
  and closed as the features landed. VTR-073 added first-run guided onboarding
  (web + extension) via OnboardJS; the deferred feature/product *tour* is tracked
  as a follow-up (not yet a numbered issue).
- `open/` — **VTR-074 + VTR-075 as of 2026-08-15**. Mobile + desktop onboarding to
  match the web/extension flow (VTR-073). The shared step contract is
  `docs/onboarding/spec.md`.

## Open issues (the real backlog)
**None.** As of 2026-08-15 all tracked issues VTR-001..VTR-075 are done/closed.
VTR-074 (mobile onboarding via `@onboardjs/react` + nativewind) and VTR-075
(desktop native Rust port of the same flow) both landed and were verified
(`hermes verify` green; desktop `cargo check` clean) on 2026-08-15.

VTR-076 (dedicated Emergency Kit surface) is newly open — see below. The
intentionally-deferred *feature tour* (anchored product walkthrough) remains a
separate piece of work tracked in `docs/onboarding/spec.md` §"Deferred".

| Issue | Open as | Notes |
|-------|---------|-------|
| VTR-076 | `open/VTR-076.md` | Dedicated Emergency Kit generation/download surface on all four clients (server kit/PDF already exists; clients only show MFA recovery codes). Onboarding step 3 deep-links here. |
OnboardJS has no native spotlight/anchor API, so it is a separate piece of work
tracked in `docs/onboarding/spec.md` §"Deferred".

| Issue | Delivered | Notes |
|-------|-----------|-------|
| VTR-071 | `closed/VTR-071.md` | SDK `auditList`, web `/_authed/audit` route, desktop `do_export_audit` + export card; `GET /audit` contract added to `openapi.json`. `hermes verify` green. |
| VTR-072 | `closed/VTR-072.md` | `docs/self-hosting/README.md` runbook + `docker-compose.prod.yml` (validated `docker compose config` → VALID). |
| VTR-073 | `closed/VTR-073.md` | `@onboardjs/core` + `@onboardjs/react` added to web + extension; `OnboardingFlow` (first-run, `localStoragePersistence`) with welcome → create-vault → Emergency Kit → add-secret → done steps; replay affordance in Settings (web) and popup footer (ext). `hermes verify` green. |
| VTR-074 | `closed/VTR-074.md` | Mobile (Expo/RN/nativewind) reuses `@onboardjs/react` (same engine/steps) via `OnboardingFlow`; AsyncStorage persistence through onboardjs `customOnDataLoad`/`customOnDataPersist`; replay in Settings. `hermes verify` green. |
| VTR-075 | `closed/VTR-075.md` | Desktop (Rust/GPUI) native port of the flow (`apps/desktop/src/onboarding.rs` + overlay in `desktop_view.rs`); first-run-once via `~/.config/vautr/onboarding.json`; replay in Settings. `cargo check` + `hermes verify` green. |

The previous version of this index listed VTR-037/039/040/047/048/049/055/056 as
"open". That table was stale: all eight already carry `status: done (2026-08-13)`
in `done/` and are implemented in code (e.g. VTR-040 mlock is wired via the
`mlock` feature in `apps/desktop/Cargo.toml`; VTR-048 mobile native overlay ships
in the `packages/native` bridge; VTR-056 conflict modal is `ConflictModal` in the
desktop). They were removed from the open table on 2026-08-14.

## How to update as work lands
1. When you implement an `open/` issue, `git mv` its file into `done/` and add a
   `status: done (YYYY-MM-DD)` line near the top (under the `#` title).
2. If during implementation you discover an `open/` issue was actually already
   done, move it to `done/` and note the evidence (file:symbol).
3. If you start work that reveals a NEW gap not yet tracked, create
   `open/VTR-NNN.md` with a `status: open` line and a `blocked_by` edge if
   relevant.
4. Keep the table above in sync with `open/` (it is the at-a-glance backlog).

The `blocked_by` edges inside each issue file are retained for history but are
NOT authoritative for sequencing — ground "what's next" in a fresh code grep,
not in the dependency graph, because the graph predates most of the shipped code.
