#!/usr/bin/env bash
# Zero-warning quality gate for the Vautr documentation site.
#
# Runs the build, type-check, and lint, captures full logs into .qa/, and
# asserts that no warn/warning line appears — except the two narrowly
# allowlisted third-party warnings documented in
# docs/decisions/0005-known-warnings.md.
set -uo pipefail

cd "$(dirname "$0")/.."
mkdir -p .qa

fail=0

echo "== pnpm build ==" && pnpm build 2>&1 | tee .qa/build.log || fail=1
echo "== pnpm check ==" && pnpm check 2>&1 | tee .qa/check.log || fail=1
echo "== pnpm lint ==" && pnpm lint 2>&1 | tee .qa/lint.log || fail=1

[ "$fail" -eq 0 ] || {
        echo "GATE FAILED — a step exited non-zero."
        exit 1
}

# astro check summary must report zero errors, zero warnings, zero hints.
if grep -Eq '^Result:.*[1-9][0-9]* (error|warning|hint)' .qa/check.log; then
        echo "GATE FAILED — astro check reported errors/warnings/hints."
        grep -E '^Result:' .qa/check.log
        exit 1
fi

# No warn/warning lines allowed anywhere, except the documented allowlist.
# Allowlisted (docs/decisions/0005-known-warnings.md):
#   1. @astrojs/sitemap has no `site` for a standalone docs deliverable whose
#      production URL is chosen at deploy time.
#   2. DEP0169 comes from httpsnippet (via starlight-openapi snippet
#      generation) and is not fixable at this level; the "trace-deprecation"
#      line is the trailer Node prints for that same DEP0169 emission.
remaining="$(
        grep -Ei '\b(warn|warning)\b' .qa/build.log .qa/check.log .qa/lint.log |
                grep -v 'The Sitemap integration requires the `site` astro.config option' |
                grep -v 'DEP0169' |
                grep -v '(Use `node --trace-deprecation ...` to show where the warning was created)' |
                grep -v '^Result:' || true
)"
if [ -n "$remaining" ]; then
        echo "GATE FAILED — unexpected warn/warning lines:"
        echo "$remaining"
        exit 1
fi

echo "GATE PASSED — build, check, and lint clean (2 documented allowlisted third-party warnings)."
