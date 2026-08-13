# Vautr Design System — "The Vault Ledger"

> Phase 0 canon. Single source of truth for the *intent* of the Vautr product
> surface. Authored before any code changes so later phases (tokens package,
> primitive parity, motion) have a fixed target to converge on.
>
> Companion docs: [screen-catalog.md](./screen-catalog.md),
> [patterns.md](./patterns.md), [copy.md](./copy.md).
> Roadmap: [../plans/ui-ux.md](../plans/ui-ux.md).

---

## 1. Product identity

**Vautr is "The Vault Ledger"** — a zero-knowledge, self-hostable, offline-first
password/credential vault. The UI should feel like a **ledger**: calm,
deliberate, mono for real data, flat structure, no decoration for its own sake.
A user should trust what they see because the surface never performs for them.

This identity is already named in the code: `apps/extension/src/styles/globals.css`
(header comment) and `apps/desktop/src/theme.rs` (module doc) both call the
surface "The Vault Ledger". We canonize it here so it is no longer just a comment.

---

## 2. The four pillars

1. **Graphite ink + emerald-teal accent.** A near-black graphite canvas with one
   confident accent — emerald-teal. No second accent, no rainbow. The accent is
   reserved for primary actions, active states, focus rings, and links.
2. **Hairline structure.** Separation is drawn with 1px hairline borders, not
   shadows or fills. Surfaces are distinguished by *value*, not elevation.
3. **Flat elevation.** No drop shadows except on popovers/dialogs (which float
   above the ledger). Cards and panels are flat; depth is implied by
   background value steps (`background → surface → surface-raised`).
4. **Mono only for real data.** Monospace is used *exclusively* for values the
   user must read or compare exactly: revealed secrets, tokens, UUIDs, public
   keys, recovery codes. Never for labels, headings, or chrome.

---

## 3. Rules (must hold on every client)

- **Dark is the operating default.** A first-class **light** companion ships
  alongside (not as an afterthought). The extension already declares `.light`
  overrides; desktop ships "Vautr Dark" only; mobile ships none today.
- **No pure greys.** Neutrals are *tinted* (a hair of the accent's hue axis,
  `~255` chromaticity) so the surface reads as "Vautr", not a neutral skeleton.
- **One accent.** Emerald-teal only. `danger`/`warn`/`success` are semantic, not
  decorative, and used sparingly.
- **Reveal appears instantly.** A revealed secret must render with no fade or
  motion — appearing instantly conveys trust (see [patterns.md](./patterns.md)
  §Reveal). The brand mark and top-level navigation changes are also
  non-animated.
- **Focus is visible everywhere.** A consistent focus ring on every interactive
  element (extension: `focus-visible:ring-3 ring-ring/50`; desktop must render
  a ring, not just a `FocusHandle`; mobile must expose VoiceOver labels).

---

## 4. Current token reality (grounding for Phase 1)

The plan assumed a `packages/design-tokens/` SSoT does not yet exist. **True
today** — there is no generated token package. The *de-facto* SSoT is:

- **`apps/extension/src/styles/globals.css`** — W3C-ish CSS custom properties
  (`--background`, `--primary`, `--sidebar-*`, `--chart-1..5`, `--radius`, …) in
  oklch. This file is `@import`ed by `apps/web/src/index.css` too, so **web and
  extension already share one token source.** Dark is `:root`/`.dark`; light is
  `.light`.
- **`apps/desktop/src/theme.rs`** — a hand-typed `Rgba` mirror of the same
  contract, built with a `const fn c(hex)`. It **drifts** from the CSS:
  - `ACCENT_DIM` is hard-coded at `0x42b59a` @ 15% alpha (exists nowhere in the
    CSS contract as a named token).
  - `DANGER_BG = #3c1517`, `DANGER_TEXT = #f2b4b5` are desktop-only tuned
    constants.
  - `TEXT_DIM = #6b7480` is a third ink step not present in the CSS scale
    (which has `foreground` / `muted-foreground` only).
- **`apps/desktop/src/vautr-theme.json`** — a GPUI theme JSON that duplicates
  ~80 hex strings by hand; it must be **generated** from the token source in
  Phase 1, not maintained.
- **`mobile/lib/theme.ts`** — referenced by the plan; verify path during Phase 1
  (the mobile theme location should be re-confirmed before codifying it).

**Phase 1 takeaway:** the token contract already *exists* in `globals.css`; the
work is to promote it to a machine-readable `tokens.json`, generate
`tokens.css` (replacing the hand-typed oklch in `globals.css`), `tokens.rs`
(replacing `theme.rs` constants), `tokens.ts` (mobile), and regenerate
`vautr-theme.json`. A single accent change must then propagate in one
`pnpm build:tokens` step.

---

## 5. Color contract (canonical values)

These are the values `globals.css` renders today. They are the seed for
`tokens.json` in Phase 1.

| Token | Dark (oklch) | Resolved hex | Role |
| --- | --- | --- | --- |
| `background` | `0.16 0.02 255` | `#080e16` | Deep canvas |
| `surface` / `card` | `0.195 0.018 255` | `#0f151d` | Raised card |
| `surface-raised` | `0.26 0.02 255` | `#1e252e` | Hover/selection well |
| `border` | `0.27 0.018 255` | `#21272f` | Hairline |
| `foreground` | `0.94 0.015 250` | `#e4ecf5` | Primary ink |
| `muted-foreground` | `0.68 0.02 250` | `#8f9aa4` | Muted ink |
| `accent` / `primary` | `0.70 0.11 175` | `#42b59a` | Emerald-teal accent |
| `accent-foreground` / `primary-foreground` | `0.16 0.02 250` | `#070e16` | Ink on accent |
| `destructive` | `0.66 0.17 22` | `#e85f61` | Danger |
| `sidebar` | `0.19 0.018 255` | `#0d1219` | Nav rail |
| `sidebar-primary` | `0.70 0.11 175` | `#42b59a` | Active nav item |
| `chart-1..5` | see globals.css | — | Categorical chart hues |

> The desktop `theme.rs` `TEXT_DIM = #6b7480` is an **un-codified** third ink
> step. Phase 1 either promotes it to a named `foreground-dim` token or removes
> it in favor of `muted-foreground`. Do not let it persist as a one-off.

---

## 6. Type, radius, motion (seeds for Phase 1 / 6)

- **Fonts:** `font-sans` = Geist Variable; `font-mono` = `ui-monospace,
  SFMono-Regular, JetBrains Mono, Cascadia Code, Menlo, Consolas, monospace`;
  `font-heading` = `font-sans` (weight semibold for wordmarks/titles).
- **Radius:** base `--radius: 0.625rem`; derived `sm/md/lg/xl` via `calc`, plus
  `2xl/3xl/4xl` multipliers. Codify the full scale in Phase 1.
- **Motion (Phase 6 seed):** `fast=100ms`, `base=150ms`, `slow=200ms`;
  `standard=cubic-bezier(0.4,0,0.2,1)`, `emphasized=cubic-bezier(0.2,0,0,1)`.
  Matches extension `duration-100` / `tw-animate-css` today.

---

## 7. Exit criteria (Phase 0)

- A maintainer can answer *"what is a Vautr screen?"* and *"what does reveal look
  like?"* by reading `docs/ui/` alone. → satisfied by this doc set.
- The four pillars + rules are named once, here, and referenced by later phases
  instead of re-argued per client.
