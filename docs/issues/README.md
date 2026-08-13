# Vautr Issues — done / open split

This folder was reconciled against the actual codebase on **2026-08-13**. The
original flat `docs/issues/VTR-*.md` set had 60 issues, every one titled
"What to build" with no status, and a `blocked_by` graph that chained
everything off VTR-001 (monorepo) / VTR-003 (OpenAPI). In reality the product
is feature-complete: ~52 of those issues were already implemented in code
(verified by grepping the workspace), and the ui-ux convergence plan
(`docs/plans/ui-ux.md`, Phases 0–10) was also completed and was never tracked
here. So the flat tracker was stale and its `blocked_by` edges falsely gated
all work on foundational tasks.

## Layout
- `done/` — issues whose acceptance criteria are satisfied by code that already
  exists (verified by source grep, not by assertion). These are kept for
  history/audit; do not re-implement them.
- `open/` — issues genuinely not present in the codebase as of the split date.
  These are the real next-phase work.

## Open issues (the real backlog)
| Issue | What's missing | Notes |
|-------|----------------|-------|
| VTR-037 | Nightly cargo-fuzz, memory-leak checks, Pact contract tests | No fuzz targets / pact in repo |
| VTR-039 | Release pipeline: multi-arch binaries + Docker images | Dockerfile exists; buildx/cross-compile not found |
| VTR-040 | Production hardening: mlock() for DashMap pages + crash-report scrub | No mlock in repo |
| VTR-047 | Quarantine reaper UI notification | Engine exists in core; no client UI surface |
| VTR-048 | Mobile native overlay for secret view (bypass RN bridge) | No native-overlay source |
| VTR-049 | Desktop auto-update, signature-verified | No updater source |
| VTR-055 | FTS5 benchmark at 10k items / prefix queries | FTS5 feature exists; no benchmark harness |
| VTR-056 | Conflict-resolution UI modal (toxic vs valid, user choice) | Engine exists; 0 client-modal matches |

## How to update as work lands
1. When you implement an `open/` issue, `git mv` its file into `done/` and add
   a `status: done (YYYY-MM-DD)` line near the top (under the `#` title).
2. If during implementation you discover an `open/` issue was actually already
   done, move it to `done/` and note the evidence (file:symbol).
3. If you start work that reveals a NEW gap not yet tracked, create
   `open/VTR-NNN.md` with a `status: open` line and a `blocked_by` edge if
   relevant.
4. Keep the table above in sync with `open/` (it is the at-a-glance backlog).

The `blocked_by` edges inside each issue file are retained for history but are
NOT authoritative for sequencing — ground "what's next" in a fresh code grep,
not in the dependency graph, because the graph predates most of the shipped code.
