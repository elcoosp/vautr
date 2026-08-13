# Navigation & Information Architecture Canon

Canonical IA for Vautr across all clients. Companion to `screen-catalog.md`. This
is the single source of truth for *where things live* and *what they are called*.
Phase 2 of the UI/UX roadmap (2026-08-12).

## Canonical sections

Stable `id` (used as route/state key) · display `label` · canonical icon.

| Order | id                | label            | icon (lucide)     | GPUI `IconName`     |
|------:|-------------------|------------------|-------------------|---------------------|
| 1     | `dashboard`       | Dashboard        | `LayoutDashboard` | `LayoutDashboard`    |
| 2     | `projects`        | Projects         | `Folder`/`FolderKanban` | `Folder`       |
| 3     | `vault`           | Vault            | `Eye`/`KeyRound`  | `Eye`                |
| 4     | `generator`       | Generator        | `Settings2`/`Wrench` | `Settings2`      |
| 5     | `secrets`         | Secrets          | `HardDrive`/`Lock`| `HardDrive`          |
| 6     | `machine-accounts`| Machine accounts | `Bot`             | `Bot`                |
| 7     | `tokens`          | Tokens           | `Globe`/`Ticket`  | `Globe`              |
| 8     | `mfa`             | MFA & security   | `CircleCheck`/`ShieldCheck` | `CircleCheck` |
| 9     | `backup`          | Import / export  | `Replace`/`ArrowLeftRight` | `Replace`  |
| 10    | `settings`        | Settings         | `Settings`        | `Settings`           |

**Label rule (hard):** every client renders the exact `label` above. No
abbreviations, no synonyms. In particular `mfa` is always **"MFA & security"** —
never just "MFA".

**Order rule:** the canonical order is fixed. A client may *omit* a section
(documented below) but must never reorder the ones it shows.

## Per-client mapping

### Desktop — full sidebar (reference implementation) ✓
All 10 sections, in canonical order, in the left sidebar. Brand header =
`size-8` rounded tile (`ACCENT_DIM` bg, `ACCENT` icon) + "Vautr" wordmark
(`FontWeight::BOLD`). Log out row pinned at the bottom. Conforms.

### Web — full sidebar ✓
`_authed.tsx` renders all 10 sections, canonical order + labels + brand tile
(`size-8` `bg-accent/15 text-accent` + "Vautr" `font-semibold`). Conforms.

### Extension popup (360×420) — 5 tabs ✓ (fixed 2026-08-12)
Tabbed. Shows the 5 most relevant sections in canonical order:

`Projects · Vault · Generator · Secrets · MFA & security`

`Dashboard`, `Machine accounts`, `Tokens`, `Import / export`, `Settings` are
omitted from the popup and reached via an **"Open full app"** link → web. Brand
header upgraded to the canonical `size-8` accent tile + "Vautr" wordmark +
username subtitle (was text-only). Tabs relabeled `MFA` → `MFA & security` and
reordered to canonical sequence.

### Mobile — bottom-tab + "More" sheet ✓ (implemented 2026-08-12)
`_app.tsx` implements the canonical pattern: a **bottom tab bar** with three
primary tabs and a **"More" sheet** for the rest. Mobile has no `/vault` route
yet, so the primary tabs are `Secrets · Generator · Settings` (Secrets is the
mobile view/use-credentials surface); the More sheet holds `Projects` and
`MFA & security`. Brand header upgraded to the canonical `size-8` accent tile +
"Vautr" wordmark + username. Sections without a mobile screen yet (Dashboard,
Machine accounts, Tokens, Import / export) are documented here as not-yet-built,
not dead links.

### CLI — subcommand → screen map (N/A visual, document only)
No IA; map subcommands to canonical screens in `--help` so a GUI-learned user
can find the CLI equivalent:

| Screen            | CLI                                      |
|-------------------|------------------------------------------|
| Vault → Reveal    | `vautr get <item>`                       |
| Generator         | `vautr gen`                              |
| Secrets           | `vautr secret <get|set|rm>`              |
| Projects          | `vautr project <ls|new>`                 |
| MFA & security    | `vautr mfa <status|enable>`              |
| Import / export   | `vautr export` / `vautr import`          |
| Settings          | `vautr config`                           |

## Login / register pattern

**Canonical:** a single segmented control ("Log in" / "Register") above one form,
with a mode toggle. Desktop already uses this (segmented toggle). Extension uses a
two-tab `Tabs` (`grid-cols-2`) — acceptable variant of the same pattern. Mobile
exposes `/login` as a route with the same segmented control. Keep one form, one
mode toggle; do not multiply auth entry points.

## Brand header canon

- `size-8` (desktop) / `size-8` (web/extension) rounded tile, accent-tinted
  background (`ACCENT_DIM` / `bg-accent/15`), accent-colored glyph.
- Wordmark "Vautr" in the heading font, semibold/bold.
- Optional username subtitle beneath/beside the wordmark once authenticated.

## Conformance (as of 2026-08-12)

| Client    | Labels match | Order matches | Brand header | Status |
|-----------|--------------|---------------|--------------|--------|
| Desktop   | ✓            | ✓             | ✓            | done   |
| Web       | ✓            | ✓             | ✓            | done   |
| Extension | ✓            | ✓ (fixed)     | ✓ (fixed)    | done   |
| Mobile    | ✓            | ✓ (bottom-tab + More) | ✓ (fixed)    | done   |
| CLI       | n/a          | n/a           | n/a          | doc only |

Next: Phase 3 (primitive parity) — icon sets should be unified to the table
above; desktop currently uses `Eye` for the brand tile glyph (cosmetic, leave).
