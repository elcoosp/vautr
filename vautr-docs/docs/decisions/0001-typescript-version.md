# ADR 0001: Pin TypeScript to 6.x for `astro check`

**Status:** Accepted
**Date:** 2026-11-07

## Context

The docs toolchain runs `astro check` (via `@astrojs/check`) as a quality gate.
Astro 7.4's `astro check` does not support TypeScript 7. Attempting to run it
with TypeScript 7 installed fails with:

> `astro check does not currently support TypeScript 7.0`

The Astro guidance is to use the latest TypeScript 6 release; an experimental
workaround (`tsc` + `@astrojs/ts-content-mapper`) exists, but it is not the
stable default and is out of scope for a documentation deliverable.

## Decision

Pin `typescript` to the latest 6.x release (**6.0.3**) in `devDependencies`:

```json
{ "devDependencies": { "typescript": "6.0.3" } }
```

The registry's latest TypeScript at decision time was 7.0.2; `6.0.3` is the
newest 6.x release.

## Consequences

- `pnpm check` runs on a supported TypeScript version.
- Upgrading to TypeScript 7 requires revisiting this decision when Astro
  announces support for it in `astro check`.
