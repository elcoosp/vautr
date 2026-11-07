# Vautr Documentation Site

Production-grade documentation for [Vautr](https://github.com/elcoosp/vautr), a
zero-knowledge password and secrets manager. Built with
[Astro 7.4](https://docs.astro.build) and
[Starlight 0.42](https://starlight.astro.build), organized following the
[Diátaxis](https://diataxis.fr) framework (tutorials / how-to guides /
reference / explanation).

## Requirements

- Node.js 24+
- [pnpm](https://pnpm.io) 10.x
- Playwright Chromium for visual QA: `pnpm exec playwright install chromium`

## Commands

All commands run from the project root:

| Command                        | Action                                                            |
| :----------------------------- | :---------------------------------------------------------------- |
| `pnpm install`                 | Install dependencies                                              |
| `pnpm dev`                     | Start the dev server at `localhost:4321`                          |
| `pnpm build`                   | Generate `llms.txt`, build the site, then render `llms-full.txt`  |
| `pnpm preview`                 | Serve the production build locally                                |
| `pnpm check`                   | `astro check` (0 errors / 0 warnings required)                    |
| `pnpm lint`                    | `markdownlint-cli2` over `src/content/docs/**`                    |
| `pnpm qa`                      | Screenshot every built route × 2 viewports × 2 themes             |
| `pnpm qa:interactions`         | Exercise search modal, theme picker, mobile nav, tabs             |
| `pnpm contrast`                | Verify WCAG AA contrast of the accent palette in both themes      |
| `bash scripts/qa-gate.sh`      | Run build + check + lint and assert the zero-warning gate         |

Full release procedure: `pnpm build` → `pnpm preview --port 4321 &` →
`pnpm qa` → `pnpm qa:interactions` → eyeball `.qa/screenshots/` → compare with
[`docs/qa-report.md`](./docs/qa-report.md).

## Project structure

```
.
├── public/                  # favicon.svg, robots.txt, vautr-openapi.json
├── scripts/                 # QA + llms.txt generation tooling
├── src/
│   ├── assets/              # logo-light.svg / logo-dark.svg (optimized)
│   ├── content/docs/        # Diátaxis pages: tutorials/ guides/ reference/ explanation/
│   ├── content/i18n/        # UI strings (en)
│   ├── styles/custom.css    # brand accent overrides (light + dark)
│   └── content.config.ts
├── astro.config.mjs         # Starlight config, plugins, OpenAPI integration
├── docs/                    # AGENTS.md, qa-report.md, decisions/ (ADRs)
└── patches/                 # pinned astro-expressive-code fix
```

## Documentation architecture

- **Tutorials** (`/tutorials/`) — guided learning path: your first vault.
- **How-to guides** (`/guides/`) — self-hosting (Docker, bare metal, reverse
  proxy), backups, MFA, import/export, machine accounts, sharing.
- **Reference** (`/reference/`) — CLI, server configuration, TypeScript SDK,
  plus the generated OpenAPI section under `/api/`.
- **Explanation** (`/explanation/`) — zero-knowledge model, key hierarchy,
  sync and optimistic concurrency.

For AI-agent consumption the build emits `llms.txt` and `llms-full.txt` at the
site root.

## Governance

Architectural decisions (TypeScript 6 pin, logo pipeline, Shiki highlighter,
QA tooling, plugin selection, known-warning allowlist) are recorded as ADRs in
[`docs/decisions/`](./docs/decisions/). The final visual QA results for every
route are recorded in [`docs/qa-report.md`](./docs/qa-report.md).
