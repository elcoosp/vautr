# CHANGELOG-ux.md — Vautr 1.0 UX Alignment

What the UX convergence (ui-ux plan, Phases 0–10) actually changed, client by
client. Grounded in the code as of 2026-08-13; every item below was verified
(build / `cargo fmt --check` / `pnpm lint` / `hermes verify`), not asserted.

## Foundations (Phase 0–1)
- **Design tokens are now a single source of truth.** `packages/design-tokens/tokens.json`
  generates `tokens.css` (web + extension), `tokens.rs` (desktop), `tokens.ts`
  (mobile), `vautr-theme.json` (GPUI), and `tokens.md`. A `pnpm --filter
  @vautr/design-tokens check:tokens` gate (wired into CI as a PR gate) fails the
  build on any drift between the authored tokens and the generated outputs.
- **Token bug fixed (caught by the A11y gate).** `--muted` (the muted *background*)
  was aliased to the muted *foreground* color (#8f9aa4), so every `bg-muted`
  (tablist, cards, code chips, tables) painted mid-gray. Fixed `--muted` →
  `surface-raised` (#1e252e). This also cleared a real `color-contrast` (ratio
  1.74) axe-core violation on the extension popup's auth tabs.

## Interaction & IA (Phase 2–4)
- Sidebars/nav in web, desktop, and mobile use one canonical section order
  (Dashboard, Projects, Vault, Secrets, Generator, MFA & security, Import/Export)
  plus the admin trio (Machine accounts, Tokens, Settings).
- Empty / loading / error states are explicit per canonical screen (no blank
  panes). First-run prompts ("No projects yet — create one") exist in web,
  mobile, and the extension popup.

## Motion & A11y (Phase 6–7)
- Desktop: focus moves to the first field on every modal/section open; Escape
  closes; native `Button`/`InputState` carry visible focus rings. (Per-element
  focus rings on raw `Div` are blocked by the git gpui checkout's
  `StyleRefinement` API — documented as a known gap, not worked around.)
- Contrast: TEXT 15.4:1, TEXT_MUTED 6.4:1, SUCCESS 9.8:1, RING 7.3:1 on surface
  (WCAG AA). TEXT_DIM 3.87:1 is large-text AA only.
- Extension: axe-core (`wcag2a`/`wcag2aa`) runs against the locked popup in CI
  (nightly `extension-a11y` job); PNG gallery of canonical states captured in
  `apps/extension/tests/screenshots/`.

## Copy & voice (Phase 8)
- `docs/ui/copy.md` is the canonical voice guide: no `!`, ellipsis `…` not `...`,
  canonical verbs (Reveal / Lock / Remove / Revoke), status lines progressive-tense
  + `…` in-flight, past-tense + `.` done.
- Desktop strings aligned: registration message ("Registered. Recovery key
  (save this): …"), all in-flight status lines use `…`, recovery codes now render
  with the "Save these now. They won't be shown again." warning.

## Visual parity & E2E (Phase 9)
- `docs/e2e/scenarios.md` lists 6 canonical cross-client flows with a per-client
  coverage matrix grounded in the real harnesses (`cli/tests/live_e2e.rs`,
  `desktop/tests/live_server_e2e.rs`, `extension/e2e/live-e2e.mjs`,
  `extension/e2e/mlp-probe.cjs`). Gaps are named, not hidden: CLI/desktop/web do
  not assert machine-account scope denial; the web client has no full MLP
  round-trip harness (`webauthn.spec.ts` only).

## Polish release (Phase 10)
- **Settings surface unification (desktop).** `Settings` no longer re-lists every
  machine account and token (that was triple-listed: dedicated `Machine accounts`
  + `Tokens` sections *and* inside `Settings`). `Settings` is now an admin hub:
  count + Refresh + "Manage" buttons that jump to the dedicated sections; the
  day-to-day lists live only in those sections. Create forms remain in `Settings`.
- **Extension popup "Open full app".** The popup footer links to the configured
  API URL (the web client) in a new tab — the popup is too small for a dashboard.
- **Dashboards already ported.** Web (`_authed/dashboard.tsx`) and mobile
  (`_app.dashboard.tsx`) both have stat tiles + backup status; the desktop
  dashboard is the canonical reference. No further porting was needed.

## Known gaps (honest, not closed)
- Nav focus ring on raw `Div` (desktop) — blocked by gpui git-checkout
  `StyleRefinement` (no border/bg/box-shadow builder).
- Mobile A11y CI (`accessibility-snapshot`) — no mobile harness yet.
- Server-backed live E2E (popup-flow / cli / desktop) is still `#[ignore]`/manual;
  CI gating needs a running-server step (deferred).
- Cross-client 1–2px `gap`/`FontWeight` consistency pass — partially addressed by
  the token contract; long tail not exhaustively audited.
