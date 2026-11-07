# ADR 0007: Starlight plugin selection

**Status:** Accepted
**Date:** 2026-11-07

The documentation architecture lists candidate HiDeoo plugins. Each is either
installed or consciously omitted here, with reasons. All installed plugins are
compatible with Astro 7.4.7 and Starlight 0.42.10 (verified from their
published peer-dependency ranges at install time).

## Installed

| Plugin | Version | Why |
| --- | --- | --- |
| `starlight-links-validator` | 0.27.0 | Fails the build on broken internal links — a release blocker for a docs site. `/api/**` is excluded because those pages are middleware-generated (ADR 0006). |
| `starlight-openapi` | 0.27.0 | The Vautr repository ships a complete OpenAPI 3.0.3 contract (`packages/api-contract/openapi.json`, 55 paths). Generating the REST reference from it keeps the docs in sync with the contract tests. Requires the `@astrojs/markdown-satteri` peer (installed). |
| `starlight-image-zoom` | 0.16.2 | Click-to-zoom on images; accessible `<dialog>`-based UI. Used by content screenshots. |
| `starlight-heading-badges` | 0.8.0 | Adds badges to headings for marking stability of features. |

## Omitted

| Plugin | Reason |
| --- | --- |
| `starlight-typedoc` | The TypeScript SDK's stable surface is small (one worker-bridge class, five generator helpers) and is documented by hand in the [TypeScript SDK reference](/reference/client-sdk/). TypeDoc would publish the package's internal modules (`realClient`, `mlp`, `storage`, worker protocol types) as API reference — an unstable internal surface the project does not document as public API. |
| `starlight-blog` | The project communicates releases through GitHub Releases; there is no blog or changelog content source to feed the plugin, and an empty section adds navigation noise. |
| `starlight-versions` | The project has no released versions to version; the plugin itself is in early development and the architecture guidance flags it with caution. Single-version docs are correct for now. |
| `@astrojs/starlight-docsearch` | No approved Algolia DocSearch application exists for this project. Pagefind (built into Starlight, zero-config, no external service) is the search solution. |

## Consequences

- If a DocSearch application is approved, add `@astrojs/starlight-docsearch`
  alongside Pagefind per the plugin's docs.
- If the SDK's public API stabilizes, revisit `starlight-typedoc`.
