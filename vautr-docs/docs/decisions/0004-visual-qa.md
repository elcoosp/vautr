# ADR 0004: Visual QA — Playwright screenshots of every route

**Status:** Accepted
**Date:** 2026-11-07

## Context

A green build does not prove the site renders correctly. The documentation
architecture requires that every built route is rendered, screenshotted, and
inspected in both viewports (desktop 1440×900, mobile 390×844) and both themes
(light, dark) before release.

## Decision

- `scripts/qa-screenshots.mjs` enumerates routes from the build output
  (`dist/**/index.html` plus `dist/404.html`) — never from a hand-maintained
  list — and for each route × viewport × theme:
  - fails on console errors, uncaught page errors, HTTP ≥ 4xx responses
    (including missing assets and broken images),
  - fails if the requested Starlight theme did not apply,
  - fails if the page has missing or duplicate `<h1>`,
  - fails if `<main>` has fewer than 200 characters of text. Two generated
    cases are exempt because they are structurally short by design: the 404
    page and the OpenAPI tag index pages (`/api/operations/tags/**`) generated
    by starlight-openapi (tag name, one-line description, operation links).
  - fails on horizontal overflow,
  - fails on broken `<img>` elements,
  - captures a full-page screenshot into `.qa/screenshots/`.
- `pnpm qa` must exit `0` against a production preview (`pnpm preview`), not
  the dev server.
- `scripts/qa-interactions.mjs` (`pnpm qa:interactions`) complements the
  screenshot suite by exercising interactive behaviour that static rendering
  cannot prove: the Pagefind search modal opens and returns titled excerpts
  for a domain term, the theme `<select>` flips `data-theme`, the mobile
  hamburger opens the `starlight__sidebar` popover, and MDX `<Tabs>` swap
  their visible panel. It exits non-zero on any failure and writes
  `.qa/interactions.json` plus screenshots to `.qa/interactions/`. Selectors
  are pinned to the Starlight 0.42.10 markup (`button[data-open-modal]`,
  `starlight-theme-select select`, `button.sl-menu-button`,
  `ul[role="tablist"] a[role="tab"]`).
- A manual eyeball pass samples the home page, one page per Diátaxis section,
  and the longest page in both viewports and both themes, and records the
  outcome in `docs/qa-report.md`.
- `.qa/` is git-ignored and excluded from the deliverable archive.

## Tooling

`playwright` (dev dependency) with the Chromium browser installed via
`pnpm exec playwright install chromium`. No axe-core additions were made; if a
future iteration adds `@axe-core/playwright`, update this ADR.

## Consequences

- Any change to config, CSS, logo, theme, or content invalidates the QA pass
  and requires a re-run (`pnpm build` → `pnpm preview` → `pnpm qa`).
- The final shipped state must be one that passed QA as the last action before
  packaging.
