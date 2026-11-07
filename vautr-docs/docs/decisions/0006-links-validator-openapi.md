# ADR 0006: Exclude generated OpenAPI routes from starlight-links-validator

**Status:** Accepted
**Date:** 2026-11-07

## Context

`starlight-links-validator` validates internal links written in the docs
collection against the set of pages Starlight knows about. The REST API
reference pages (`/api/`, `/api/operations/…`) are generated at build time by
`starlight-openapi` through route middleware and are not part of the docs
content collection, so the validator reports links to them as broken even
though the pages exist in `dist/` and render correctly.

## Decision

Configure the validator to exclude the generated routes:

```js
starlightLinksValidator({ exclude: ['/api/**'] }),
```

Every other internal link in the documentation remains validated, and the
build fails on any broken link outside `/api/**`.

## Consequences

- Links into the generated API reference (`/api/`, `/api/operations/<id>/`)
  are verified by the visual QA pass instead of the build-time validator.
- If `starlight-links-validator` gains first-class awareness of
  starlight-openapi routes, the exclusion should be removed.
