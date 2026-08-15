# Vautr Feature Tour — Shared Spec (anchored product walkthrough)

Companion to `docs/onboarding/spec.md`. The **first-run guided setup** (VTR-073/074/075)
is a one-time, modal, full-screen flow. The **feature tour** is a *separate, lighter*
walkthrough the user can replay from Settings at any time. It points at real UI
surfaces ("here is where you add a secret", "here is your Emergency Kit").

## Why a separate spec
OnboardJS (used for first-run setup) has **no element-spotlight / anchor API** — it
only renders one step at a time in a host-provided overlay. A product tour needs to
*point at* existing controls. So the tour is its own small engine per client, not
built on OnboardJS.

## Tour steps (contract — same copy/targets on all clients)
Each step targets a named UI anchor (a stable `data-tour` id or, on mobile/desktop,
a known section). Skippable. Order:

1. `vault` — "Your Vault" — where secrets live. Anchor: sidebar/nav Vault entry.
2. `add-secret` — "Add a secret" — the primary "Add item" / "New" action.
3. `emergency-kit` — "Emergency Kit" — recovery surface (VTR-076 target).
4. `audit` — "Audit log" — see who accessed what (security). Anchor: Audit entry.

## Per-platform rendering approach (the honest "best")
- **Web + extension** (React + Radix/Base-UI Tooltip): **anchored**. A `TourProvider`
  renders a dimmed full-screen scrim + a highlighted ring around the targeted
  element (found via `data-tour="<id>"` / ref) + a positioned tooltip card next to
  it. Real spotlight. Next/Back/Skip/Finish.
- **Mobile (RN)** + **Desktop (GPUI)**: no element-measurement primitive. Ship a
  **centered-card sequence** (same visual language as first-run onboarding but
  shorter, 4 tips) — faithful in intent, not pixel-anchored. True element anchoring
  on RN/GPUI requires layout measurement (RN `measure()`, GPUI `bounds()`) and is a
  follow-up, not required for VTR-077.

## Triggers
- Manual only (replay from Settings). NOT auto-shown after first-run (that is the
  guided setup's job). Settings copy already says "replay this tour anytime".
- Persistence: web/ext reuse OnboardJS localStorage key namespace
  (`vautr_tour_v1`) but the tour is independent of the setup flow; mobile uses
  AsyncStorage; desktop uses `onboarding.json` (`tour_seen_v1`).

## Files
- `apps/web/src/tour/*` (web) · `apps/extension/src/tour/*` (ext)
- `apps/mobile/src/tour/*` (mobile, centered) · `apps/desktop/src/tour.rs` (desktop, centered)
- Replay buttons live next to the existing "Replay onboarding" controls in Settings.
