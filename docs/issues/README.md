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
- `closed/` — VTR-066..VTR-070. The most recent workstream batch, tracked live
  and closed as the features landed (see `closed/VTR-070.md` for the full
  reconciled final state, including the iOS/Android native bridges that the
  earlier "not verifiable in sandbox" note wrongly claimed could not be built —
  both `xcodebuild` and `gradlew :app:assembleDebug` were executed and verified).
- `open/` — **empty as of 2026-08-14**. No issue through VTR-072 is genuinely
  open in the codebase.

## Open issues (the real backlog)
**None.** As of 2026-08-14 all tracked issues VTR-001..VTR-072 are done/closed.
The two last open items, VTR-071 (audit-log viewer + export in web/desktop
clients) and VTR-072 (complete VTR-060 self-hosting docs), landed and were
moved to `closed/` on 2026-08-14:

| Issue | Delivered | Notes |
|-------|-----------|-------|
| VTR-071 | `closed/VTR-071.md` | SDK `auditList`, web `/_authed/audit` route, desktop `do_export_audit` + export card; `GET /audit` contract added to `openapi.json`. `hermes verify` green. |
| VTR-072 | `closed/VTR-072.md` | `docs/self-hosting/README.md` runbook + `docker-compose.prod.yml` (validated `docker compose config` → VALID). |

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
