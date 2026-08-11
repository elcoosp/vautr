---
name: Vautr
description: Zero-knowledge password & secrets manager — "The Vault Ledger"
colors:
  primary: "#42b59a"
  neutral-bg: "#080e16"
  neutral-fg: "#e4ecf5"
typography:
  display:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontWeight: 700
    letterSpacing: "-0.02em"
  body:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontSize: "1rem"
    lineHeight: 1.5
  mono:
    fontFamily: "ui-monospace, JetBrains Mono, Menlo, monospace"
    fontVariantNumeric: "tabular-nums"
rounded:
  sm: "6px"
  md: "8px"
  lg: "10px"
spacing:
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.neutral-bg}"
    rounded: "{rounded.md}"
---

# Vautr Design System — "The Vault Ledger"

## Overview

Vautr is a zero-knowledge password and secrets manager. Its clients — web,
mobile, desktop, and extension — must feel like one instrument, no matter which
surface the user is on. **The Vault Ledger** is that instrument: a precise,
ruled, tabular security console. It speaks the language of ciphertext, hashes,
and audit trails: legible, verifiable, and calm.

The system refuses the two category defaults: the dark-neon "hacker vault" and
the generic light SaaS card grid. Instead it is a flat, graphite-and-ink console
with **one** confident emerald-teal accent that means "verified." Structure comes
from hairlines and rules, not shadows and glow.

**Operating scene:** developers at a desk at night with an IDE open. This is why
dark is the default; a first-class light companion exists on `.light`.

## Colors

The palette is **Restrained**: tinted cool neutrals plus one accent. Neutrals are
tinted graphite-blue, never pure gray.

### Dark (default)
| Token | Value | Role |
|-------|-------|------|
| `--background` | `oklch(0.16 0.02 255)` / `#080e16` | canvas |
| `--card` / `--popover` | `oklch(0.195 0.018 255)` / `#0f151d` | surface |
| `--foreground` | `oklch(0.94 0.015 250)` / `#e4ecf5` | ink |
| `--muted-foreground` | `oklch(0.68 0.02 250)` / `#8f9aa4` | secondary ink |
| `--primary` | `oklch(0.70 0.11 175)` / `#42b59a` | emerald accent, "verified" |
| `--primary-foreground` | `oklch(0.16 0.02 250)` / `#070e16` | dark ink on accent |
| `--border` | `oklch(0.27 0.018 255)` / `#21272f` | hairline |
| `--destructive` | `oklch(0.66 0.17 22)` / `#e85f61` | error |
| `--warn` | `oklch(0.80 0.12 75)` / `#ebb25f` | warning |
| `--success` | `oklch(0.78 0.15 165)` / `#37d59f` | verified |

### Light (`.light`)
Tuned daylight variants of the same world: cool paper `oklch(0.985 0.004 250)`,
deep emerald primary `oklch(0.50 0.10 175)` with white foreground, hairline
`oklch(0.90 0.012 250)`.

**Named rule:** the accent is reserved for primary actions, focus, selection,
and "verified/encrypted" signals. It is never scattered as decoration.

## Typography

- **Display/body:** Geist Variable (self-hosted via `@fontsource-variable/geist`).
- **Monospace:** reserved for **real data only** — UUIDs, hashes, secret values,
  TOTP codes, timestamps, checksums. Set `tabular-nums`. Never a costume for "tech."
- Headings rely on **weight and size**, never gradient or kicker/eyebrow labels.
- Tracking floor at `-0.02em` to `-0.03em`; never beyond `-0.04em`.

## Layout

- A 4-unit spacing rhythm (`8 / 16 / 24 / 32`). More space above a heading than below.
- Canonical five-tab navigation across all clients: **Projects / Secrets /
  Generator / MFA / Settings** (plus Vault where present).
- Structure via proximity and hairlines first; cards are containers of record,
  never nested for their own sake.

## Elevation & Depth

Flat. No glow, no glass, no hard offset block shadows. Elevation is declared
**once** — a 1px hairline border or a subtle offset shadow, never both (no
"ghost cards"). Surfaces are layered by tone (`background` → `card` → raised).

## Shapes

- Card and control radii `8–12px` (`--radius: 0.625rem`).
- Pills (`rounded-full`) reserved for small chips/badges, not controls.

## Components

- **Primary button:** emerald background, dark ink foreground (dark theme) —
  a bright "go/verified" signal. Focus ring uses `--ring` (accent).
- **Inputs:** `--input` surface, hairline `--border`, placeholder at
  `--muted-foreground` (≥4.5:1). Focus ring accent.
- **Destructive:** `--destructive` surface + `--destructive-foreground`; never
  green-washing an error.
- **Data rows:** hairline separators; selected row uses raised surface.
- **Icons:** Lucide (web/extension) and the platform icon system, one consistent
  stroke weight. Never emoji/Unicode glyphs standing in for icons.

## Do's and Don'ts

**Do:** keep one accent doing one job (verify/primary). Use hairlines for
structure. Set real data in tabular mono. Keep dark as the default scene. Use
tinted neutrals, not gray. Make focus visible on every control.

**Don't:** glow or glass as decoration. Gradient text. Kicker labels above
headings. Hard offset "neobrutalist" shadows. Emoji as icons. Monospace as a
"technical" costume. Nest cards. Split elevation into border + shadow.
