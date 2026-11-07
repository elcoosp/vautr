# ADR 0002: Logo format and variants

**Status:** Accepted
**Date:** 2026-11-07

## Context

The Vautr repository ships three brand assets in `brand/`:

| File | Contents | Notes |
| --- | --- | --- |
| `logo.svg` | Ribbon icon + "Vautr" wordmark (440×120) | Wordmark text is rendered with an SVG `<text>` element styled with the `Inter` font and a `#0F172A` fill. |
| `logo-app-light.svg` | Rounded-square app icon, light background (`#F8FAFC → #E2E8F0`), aurora gradient ribbon (`#4F46E5 → #06B6D4`) | Pure vector paths. |
| `logo-app-dark.svg` | Rounded-square app icon, dark background (`#0F172A → #1E293B`), high-luminance ribbon (`#818CF8 → #22D3EE`) | Pure vector paths. |

All three are SVGs with a `viewBox`, so all three scale losslessly.

## Decision

Use `logo-app-light.svg` and `logo-app-dark.svg` as the header logo's
light/dark variants (`src/assets/logo-light.svg` and `logo-dark.svg`), and
`logo-app-dark.svg` as the favicon (`public/favicon.svg`).

Rationale:

1. **The wordmark logo depends on viewer fonts.** Its `<text>` element requests
   `Inter`, which most systems and contexts do not have; the fallback
   (`system-ui`) renders with different metrics per platform, changing the
   header appearance unpredictably. The app icons contain only paths and
   gradients, so they render identically everywhere.
2. **Theming.** The project itself ships explicit light and dark app-icon
   variants, so both themes get a logo that was designed for that background —
   no runtime color swapping required.
3. **No pixelation risk.** Both assets are vector SVGs with a preserved
   `viewBox`; no raster upscaling is involved anywhere in the pipeline.

## Pipeline

- Copied from `brand/` into `src/assets/` unchanged in structure.
- Optimized once with SVGO (`pnpm dlx svgo --multipass`), which reduced both
  files by ~31–42% (1.1 KiB → 0.77 KiB each).
- SVGO config constraint: `removeViewBox: false` (the default preset is
  overridden if needed); the `id` attributes referenced by the gradients are
  retained by SVGO's `cleanupIds` since they are referenced.
- The logo does not contain the project name, so `replacesTitle` is not used;
  the "Vautr" text title is rendered by Starlight next to the icon.

## Consequences

- The header shows the app icon plus the text title, sharp at any zoom in both
  themes.
- If the project later ships a font-independent (path-outlined) wordmark SVG,
  switching to it would restore the wordmark in the header.
