# QA Report — Vautr Documentation Site

> Generated from the final pre-zip build. Tools: `scripts/qa-screenshots.mjs`
> (every route × desktop 1440×900 / mobile 390×844 × light/dark, full-page
> screenshots in `.qa/screenshots/`), `scripts/qa-interactions.mjs` (search modal,
> theme picker, mobile nav, tabs), and `scripts/qa-gate.sh` (zero-warning gate).

## Automated quality gates

| Gate | Result |
| --- | --- |
| `pnpm build` | ✅ exit 0 — 104 pages + Pagefind index; links-validator: all internal links valid |
| `pnpm check` | ✅ Result: 0 errors, 0 warnings, 0 hints (TypeScript 6.0.3) |
| `pnpm lint` | ✅ markdownlint-cli2: 0 issues in 17 files |
| `pnpm qa` | ✅ QA PASSED — 104 routes × 2 viewports × 2 themes (416 screenshots) |
| `pnpm qa:interactions` | ✅ search, theme, mobile nav, tabs — all PASS |
| `pnpm contrast` | ✅ accent roles ≥ 4.5:1 (AA) in both themes |

Two third-party warnings are narrowly allowlisted with justification in
[`docs/decisions/0005-known-warnings.md`](./decisions/0005-known-warnings.md):
the `@astrojs/sitemap` `site`-option notice (production URL chosen at deploy time)
and Node DEP0169 from `httpsnippet` via `starlight-openapi` snippet generation.

## Interactive checks (scripts/qa-interactions.mjs)

| Check | Result | Detail |
| --- | --- | --- |
| `search-modal-opens` | ✅ | dialog visible after clicking header search |
| `search-returns-results` | ✅ | 5 result(s) for query "vault" |
| `search-excerpt-present` | ✅ | first hit: "Import and Export Vault Data Import and Export Vault Data Import and Export Vault Data. " |
| `theme-toggle-dark` | ✅ | before=light after=dark |
| `mobile-menu-opens` | ✅ | popover-open=true, 104 sidebar link(s) visible |
| `tabs-switch-panel` | ✅ | pnpm="Terminal window pnpm add @vautr/client-sdk" npm="Terminal window npm install @vautr/client-sdk" |

## Per-route visual QA

Automated probes per cell: theme applied, exactly one `<h1>`, main text depth,
no horizontal overflow, no broken images, no console/page errors, no HTTP ≥ 400.
The 404 page and OpenAPI tag-index pages are intentionally exempt from the
content-depth probe (documented in `scripts/qa-screenshots.mjs`).

| Route | Desktop light | Desktop dark | Mobile light | Mobile dark | Notes |
| --- | --- | --- | --- | --- | --- |
| `/` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Home: logo sharp, indigo accent on CTAs/links, CardGrid, Tip aside, LinkCard, pagination. Light+dark, desktop+mobile. Interaction-tested: Search modal opens, 'vault' query returns results with excerpts. |
| `/404.html` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Custom 404: big heading, indigo CTA, two LinkCards. Exempt from content-depth rule by design. |
| `/api/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/accountdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/accountreclaim/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/accountrotatekey/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/accountstatus/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/auditlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/authloginfinish/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/authloginstart/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/authregisterfinish/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/authregisterstart/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/backupexport/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/backuprestore/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/backupstatus/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/filedownload/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/fileuploadcomplete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/fileuploadinitiate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/fileuploadstatus/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/groupadditem/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/groupaddmember/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/groupcreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/groupdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/groupget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/grouplist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/itemdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/itemupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/machineaccountcreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/machineaccountdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/machineaccountget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/machineaccountlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/machineaccountupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfapolicyget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfapolicyupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfastatus/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfatotpissue/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfatotpverify/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfawebauthnenrollfinish/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/mfawebauthnenrollstart/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/offboarduser/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectcreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgroupaddmember/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgroupcreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgroupdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgrouplist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgroupremovemember/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectgroupupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectmemberadd/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectmemberlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectmemberremove/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectmemberupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/projectupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/recoverchallenge/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/recovercomplete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/recoververify/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretcreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretdelete/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretgetvalue/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/secretupdate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/sharecreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/shareinbox/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/sharerevoke/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/shareuploadpayload/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/syncpull/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/syncpullpayloads/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/syncpushbatch/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/account/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/auth/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/backup/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/files/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/items/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/machine-accounts/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/mfa/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/projects/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/recovery/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/secrets/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/sharing/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/sync/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tags/tokens/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tokencreate/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tokenget/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tokenlist/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/tokenrevoke/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/api/operations/userspublickey/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/explanation/key-hierarchy-recovery/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/explanation/sync-occ/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/explanation/zero-knowledge-model/` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Explanation (light): comparison table fits, headings/links styled, Next steps links resolve. |
| `/guides/backups/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/enable-mfa/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/import-export/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/machine-accounts/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/reverse-proxy-tls/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/self-host-bare-metal/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/guides/self-host-docker/` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Guide (light): code blocks with copy button, profiles, Caution aside, tables/inline chips, TOC, pagination. Interaction-tested: Theme select flips light->dark via native <select>. |
| `/guides/share-items/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/reference/cli/` | ✅ | ✅ | ✅ | ✅ | Automated QA pass. |
| `/reference/client-sdk/` | ✅ | ✅ | ✅ | ✅ | Interaction-tested: Tabs switch pnpm -> npm panel content. |
| `/reference/server-configuration/` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Longest page (dark): all variable tables fit, no cut columns, inline code chips legible, TOC highlights. |
| `/tutorials/first-vault/` | ✅ | ✅ | ✅ | ✅ | Eyeballed: Tutorial (dark): numbered Steps, highlighted code tokens, Caution aside, sidebar active state, TOC, prev/next. Interaction-tested: Mobile hamburger opens popover sidebar (104 links). |

## Manual eyeball-pass summary

- **Logo**: SVG with `viewBox`, sharp at 2× in both themes; no halo/clipping.
- **Accent**: derived from the Vautr brand gradient (#4F46E5 → #06B6D4); visible on
  links, active sidebar item, focus states — not the Starlight default blue.
- **Code blocks**: Shiki via Expressive Code, multi-token highlighting, copy button,
  horizontal scroll inside the block; line highlighting renders where used.
- **Tables**: fit within the content column; no truncated columns on the longest page.
- **Components**: `<Aside>` (Tip/Caution), `<CardGrid>`, `<Tabs>`, `<Steps>`, `<FileTree>`
  render with correct icons, spacing, and borders.
- **Sidebar/TOC/pagination**: active states correct; TOC tracks headings; prev/next sane.
- **Mobile**: hamburger opens the nav popover, content stacks without clipping.
- **Dark mode**: no pure-white panels, no invisible text, code backgrounds distinct.
- **Search**: Pagefind modal returns sensible titled excerpts for domain terms.

## Screenshots

Full-page screenshots for every route/view/theme are written to `.qa/screenshots/`
(416 files) and interaction shots to `.qa/interactions/`. `.qa/` is a local QA
artifact directory: git-ignored and excluded from the deliverable zip.

