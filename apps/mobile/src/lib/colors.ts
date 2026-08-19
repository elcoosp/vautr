/**
 * Shared brand accent color for native primitives that need a raw color string
 * (e.g. `ActivityIndicator` `color` prop, inline `borderColor`).
 *
 * Kept in sync with the design-token `--primary` / `color.accent` (emerald-teal
 * `#42b59a` in dark mode, `#1f8f74` in light). Most mobile UI should use the
 * token classes (`text-accent`, `bg-primary`) instead; this constant exists
 * only for the few RN APIs that require a literal color string.
 */
export const ACCENT = '#42b59a';
