# ADR 0005: Known third-party warnings (narrowly allowlisted)

**Status:** Accepted
**Date:** 2026-11-07

The zero-warning gate treats every `warn`/`warning` line in the build, check,
and lint logs as a release blocker. Two warnings originate in third-party
dependencies, cannot be fixed at this repository's level, and are therefore
allowlisted as narrowly as possible. **Blanket suppression remains forbidden.**

## 1. `@astrojs/sitemap` requires the `site` option

Exact line:

```
[WARN] [@astrojs/sitemap] The Sitemap integration requires the `site` astro.config option. Skipping.
```

**Why it happens.** Starlight automatically adds `@astrojs/sitemap`. The
integration skips itself (with this warning) unless `site` is set in
`astro.config.mjs`.

**Why it is not fixed here.** The docs site is a standalone deliverable whose
deployment host is not known at authoring time, and inventing a domain (for a
sitemap that would then contain fake URLs) is worse than shipping without a
sitemap URL base. The general guidance "do not add `site` unless the project
requires it" applies.

**How to fix at deploy time.** Set `site` in `astro.config.mjs` to the real
production URL; the warning disappears and the sitemap is emitted.

**Narrow filter** (in `scripts/qa-gate.sh`, with this comment):

```sh
# Allowlisted: @astrojs/sitemap has no `site` for a standalone docs deliverable
# whose production URL is chosen at deploy time (ADR 0005 §1).
grep -v 'The Sitemap integration requires the `site` astro.config option'
```

## 2. `httpsnippet` (via starlight-openapi) calls deprecated `url.parse()`

Exact line (Node.js `DEP0169` process deprecation, emitted while prerendering
the generated OpenAPI operation pages):

```
(node:…) [DEP0169] DeprecationWarning: `url.parse()` behavior is not standardized and prone to errors that have security implications. Use the WHATWG URL API instead. CVEs are not issued for `url.parse()` vulnerabilities.
```

Stack: `urlParse (node:url)` ← `HTTPSnippet.prepare` ←
`generateOperationSnippets` (starlight-openapi's snippet generation, backed by
the `httpsnippet` library). Node also prints the fixed trailer line
`(Use \`node --trace-deprecation ...\` to show where the warning was created)`
for that same emission; the gate filters both exact strings.

**Why it is not fixed here.** The call lives inside the `httpsnippet`
dependency used by `starlight-openapi` for its documented snippet feature; no
configuration knob exists to avoid it, and patching a transitive dependency
would break on updates. It is a Node process deprecation notice, not an Astro
or Starlight warning, and does not affect output correctness.

**Narrow filter** (in `scripts/qa-gate.sh`, with this comment):

```sh
# Allowlisted: DEP0169 comes from httpsnippet (via starlight-openapi snippet
# generation) and is not fixable at this level (ADR 0005 §2).
grep -v 'DEP0169'
```

## Verification

`scripts/qa-gate.sh` applies only these two exact-string filters. Any other
`warn`/`warning` line in `pnpm build`, `pnpm check`, or `pnpm lint` fails the
gate.
