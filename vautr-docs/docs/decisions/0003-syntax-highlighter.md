# ADR 0003: Syntax highlighter — Shiki via Expressive Code

**Status:** Accepted
**Date:** 2026-11-07

## Context

Starlight renders code blocks through Expressive Code, whose supported
highlighter is **Shiki** (VS Code TextMate grammars, 180+ languages, build-time
execution, zero client-side JavaScript, dual light/dark themes, transformers
and Twoslash support).

Alternative lightweight highlighters (e.g. Rangi, ~13 kB, ~46 languages, regex
tokenizer) target chat UIs and small apps. The Rangi documentation's own
comparison table lists "Docs sites, rich code samples" as Shiki's use case, and
Rangi is not integrated with Expressive Code — adopting it would require a
custom highlighter plugin with no upstream support, while losing transformers,
Twoslash, and the language coverage documentation needs.

## Decision

Ship with Shiki through Starlight's built-in Expressive Code integration and do
not add a custom highlighter or a Rangi plugin. No custom `expressiveCode`
configuration is added: Starlight's default themes already follow the site's
light/dark toggle, and the default language set covers every language used in
this documentation (bash, caddyfile, json, ts, rust, yaml, text).

## Consequences

- Code blocks are highlighted at build time with zero client-side JS.
- Authors must tag every fenced code block with a language; an unlabeled fence
  renders as plain text (enforced by review and the visual QA pass).
