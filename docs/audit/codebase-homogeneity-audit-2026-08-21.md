# Vautr Codebase Audit — Stubs, Semi-implemented Features & Cross-Client UI Homogeneity

Date: 2026-08-21 · Auditor: Hermes Agent
Scope: full monorepo (`/Users/adm/Documents/Repos/vautr`), grounded in live source
(not `docs/plans/ui-ux.md` "current state", which is known stale — per
`vautr-frontend-convergence` skill rule).

Method: 4 parallel subagent audits (stub/TODO scan, feature matrix, server/core/CLI
depth, UI/UX consistency) + independent source verification by the auditor. Every
finding below cites `file:line` evidence. No files were modified.

---

## 0. TL;DR

- **No `unimplemented!()` / `todo!()` / `panic!("not …")` stubs exist in compiled
  source.** A repo-wide grep returned only doc-comment mentions ("honors the no
  `todo!()` rule") and HTML `placeholder=` attributes. The crypto core, server, and
  SDK are genuinely *implemented*, not scaffolded.
- **The four primary clients (web / extension / mobile / desktop) are far more
  converged than the plan docs claim.** Shares read+write, MFA, machine-accounts,
  tokens, import/export, onboarding, and the ZK reveal path are present on all four.
- **Real, defensible homogeneity GAPS remain** (see §3). The sharpest ones:
  1. **CLI is `bws`-style and omits `share` / `accept-share` / `rotate` /
     `export` / `import`** — intentional but breaks "every feature on every client".
  2. **No client has a global Search surface** (all 4 MISSING).
  3. **Audit log** present on web/extension/mobile but **absent on desktop**.
  4. **Dashboard** present on web/desktop/mobile but **absent on extension**.
  5. **Settings surface** missing as a first-class entry on extension (nested/absent).
  6. **Web** has no dedicated MFA / Import-Export nav entries (nested under Settings).
- **Interaction-pattern divergences** (see §4): desktop delete is confirm-gated
  (good, VTR-104 DONE); mobile reveal is the ZK native-overlay path (VTR-104/VTR-061
  CLOSED — do NOT re-raise); web/extension reveal is JS-decrypt-then-render (ZK not
  required there per AGENTS.md, but inconsistent with mobile/desktop).

---

## 1. Stub / placeholder scan (all languages)

Result: **clean of genuine stubs.** Evidence:

- `grep -rn "unimplemented!\|todo!()\|FIXME\|XXX" --include=*.rs` (excl `target/`,
  `test/`) → only:
  - `core/vautr-telemetry/src/lib.rs:30` doc comment ("The `no todo!()` roadmap rule
    is honored throughout.")
  - `apps/cli/src/lib.rs:22` doc comment ("No `todo!()` / `panic!()` in compiled code.")
- `grep` for `TODO|FIXME|placeholder|NotImplemented|Coming soon` across
  `apps/{web,extension,mobile,desktop}/src`, `apps/cli/src`, `packages`, `core`
  (excl `node_modules`, `routeTree.gen.ts`, `vautr_wasm.*`) → **all hits are either**
  (a) HTML `placeholder=` input hints (legitimate UI), or
  (b) `apps/web/src/bones/registry.ts:4` — an *intentional* documented no-op
  placeholder for the `boneyard` skeleton lib, explicitly noted as temporary.
- No empty `impl` blocks, no `Ok(())` no-op transport methods, no `return null`
  where a real body is expected.

**Conclusion:** there is no "fake feature" debt to burn down. The risk surface is
*homogeneity*, not stubbed code.

---

## 2. Feature Homogeneity Matrix (clients × features)

Legend: ✅ full · 🟡 partial/orphaned/nested · ❌ missing.

| # | Feature | web | extension | mobile | desktop | CLI | SDK (`realClient`) |
|---|---------|-----|-----------|--------|---------|-----|--------------------|
| 1 | Register / Login (OPAQUE) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 2 | Unlock + Lock | ✅ | ✅ | ✅ | ✅ | ✅ (session) | ✅ |
| 3 | Vault / Secrets CRUD | ✅ | ✅ | ✅ | ✅ | ✅ (list/get/run/create/edit) | ✅ |
| 4 | Projects CRUD | ✅ | ✅ | ✅ | ✅ | ✅ (create/edit) | ✅ |
| 5 | Generator (pw/TOTP) | ✅ | ✅ | ✅ | ✅ | ❌ (not a CLI concern) | n/a |
| 6 | Reveal secret (ZK) | 🟡 JS-decrypt | 🟡 JS-decrypt | ✅ native overlay | ✅ `read_secret` | ❌ (get only) | ✅ opaque handle |
| 7 | Shares: send | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| 7b| Shares: inbox/accept | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| 7c| Shares: groups | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| 8 | MFA / WebAuthn | 🟡 nested in Settings | ✅ tab | ✅ route | ✅ section | ❌ | ✅ |
| 9 | Machine accounts | ✅ | ✅ | ✅ | ✅ | ✅ (provision+token) | ✅ |
| 10| Tokens mgmt | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| 11| Backup import/export | 🟡 nested in Settings | ✅ tab | ✅ route | ✅ section | ❌ | ✅ |
| 12| Settings (key rotation) | ✅ | 🟡 nested/absent | ✅ | ✅ | ❌ | ✅ |
| 13| Onboarding / first-run | ✅ | ✅ | ✅ | ✅ (native) | ❌ | n/a |
| 14| Emergency kit / recovery | 🟡 in mfa/settings | 🟡 in mfa | ✅ | ✅ (kit HTML) | ❌ | n/a |
| 15| Search (global) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 16| Audit log | ✅ | ✅ | ✅ | ❌ | ❌ | n/a |
| 17| Dashboard | ✅ | ❌ | ✅ | ✅ | ❌ | n/a |

### Per-client divergence evidence

**Web** (`apps/web/src/routes/_authed.tsx:28-39`) sidebar entries:
Dashboard, Projects, Vault, Generator, Secrets, Shares, Machine accounts, Tokens,
Settings. Missing dedicated nav for **MFA** and **Import/Export** (both reached only
via Settings sub-pages). Has **Dashboard, Audit (`audit.tsx`), Search? ❌** (no global
search route; only per-list filtering). Web reveal (`ItemDetail`/`revealSecret`) is
JS-decrypt-then-render — acceptable per AGENTS.md (ZK reveal path is desktop-only
feature gate) but visually/behaviorally distinct from mobile/desktop opaque handles.

**Extension** (`apps/extension/src/popup/App.tsx:191-218`) tabs:
vault, projects, secrets, generator, mfa, import-export, machine-accounts, tokens,
shares, audit. **Missing: Dashboard, Settings (no dedicated tab), Search.** MFA and
Import/Export are first-class tabs (good). No Settings surface at all — key rotation /
emergency-kit live only inside MFA/Import-Export tabs. This is the least-complete
*administrative* surface.

**Mobile** (`apps/mobile/src/routes/`) routes: `__root, _app.audit, _app.dashboard,
_app.generator, _app.import-export, _app.machine-accounts, _app.mfa, _app.projects.*,
_app.secrets, _app.settings, _app.shares, _app.tokens, _auth.*`. **Most complete
surface set** — has every section incl. Dashboard, Audit, Settings, Search? ❌ (no
global search route). ZK reveal is the native `SecretOverlay` + `MobileVautrClient`
(`bootVautrCore()` in `__root.tsx`, `getMobileClient()` non-null → `client.reveal(uuid)`
returns opaque handle; plaintext never enters JS). **This parity is SATISFIED
(VTR-104 + VTR-061 CLOSED) — do NOT re-raise as a gap.**

**Desktop** (`apps/desktop/src/desktop_view.rs:44-56` `Section` enum, sidebar tuple
`3706-3717`): Dashboard, Projects, Vault, Generator, Secrets, Machine accounts, Tokens,
Mfa, Import/export, Shares, Settings. **Missing: Audit** (no `Section::Audit`, no
`render_audit`). Has onboarding (`onboarding_step`, `render_onboarding`, VTR-075) and
Emergency Kit (`render_kit_html`). Delete is **confirm-gated** (`render_delete_confirm_modal`
+ `do_delete`/`do_delete_project`/`do_delete_secret`/`do_delete_machine`, type-to-confirm
for org-level) — VTR-104 DONE, do NOT re-introduce an immediate-delete modal.

**CLI** (`apps/cli/src/cli.rs:27-53`): `bws`-style surface — Login, Register,
MachineAccount, Logout, List, Get, Run, Create{Project,Secret}, Edit{Project,Secret}.
**No** `share`/`accept-share`/`rotate`/`export`/`import` subcommands. This is a
deliberate `bws` (Bitwarden-CLI-compatible) subset, so it is a *design choice*, but it
means 5 SDK capabilities have no CLI exposure → homogeneity gap for "power-user"
flows. Flag, don't treat as a bug.

**SDK** (`packages/vautr-client-sdk/src/realClient.ts:260-1013`) is the canonical
feature surface and is **fully implemented**: register, login, lock, forget,
rotateKey, ensureSharingKey, shareItem, getShareInbox, acceptShare, revokeShare,
createGroup, addGroupMember, getGroupInbox, unwrapGroupKey, shareItemToGroup,
getGroupItems, acceptGroupItem, revokeGroupItem, rotateGroup, removeGroupMember,
getGroupKey, sync, addItem, reveal (opaque handle), getItemPlaintext, revealSecret,
encrypt/decrypt, performAction, release, resolveConflict. Mobile native client
(`mobile.ts`) mirrors reveal/share with native bridge. **The SDK is not the bottleneck;
client wiring is.**

---

## 3. Concrete Homogeneity Gaps (priority order)

### G1 — CLI missing share / accept / rotate / export / import  (🟡 design, real gap)
- Files: `apps/cli/src/cli.rs:27-53`, `apps/cli/src/lib.rs` (no such commands).
- Impact: `vautr-cli` cannot share secrets, rotate keys, or back up — three
  first-class features every other client has. Justified as `bws`-compat but should be
  documented as a known deviation or extended.

### G2 — No global Search on ANY client  (❌ all 4)
- Evidence: no `/search` route in web/extension/mobile; no `Section::Search` in
  desktop (`desktop_view.rs:44-56`); SDK has no search method.
- Impact: users filter per-list only. A "search across secrets/projects" surface is
  absent everywhere — uniform gap, so not a divergence, but a missing feature.

### G3 — Audit log absent on desktop  (❌ desktop only)
- Evidence: `desktop_view.rs` `Section` enum (44-56) + sidebar tuple (3706-3717) have
  no Audit; web/extension/mobile all have it (`audit.tsx`, `AuditTab`, `_app.audit.tsx`).
- Impact: desktop users cannot review their access/security audit trail.

### G4 — Dashboard absent on extension  (❌ extension only)
- Evidence: extension tabs (App.tsx:191-218) have no `dashboard`; web/desktop/mobile do.
- Impact: extension popup users get no at-a-glance stats/backup-status landing.

### G5 — Settings surface not first-class on extension  (🟡 extension)
- Evidence: no `settings` tab in `App.tsx:191-218`; key-rotation/emergency-kit live
  inside MFA/Import-Export tabs. Web/desktop/mobile all have a dedicated Settings.
- Contrast: web nests MFA+Import/Export under Settings too, so web is consistent with
  extension here; desktop/mobile are the "fully flat" outlier. Pick ONE canonical IA.

### G6 — Web MFA / Import-Export not in sidebar  (🟡 web)
- Evidence: `_authed.tsx:28-39` lists no MFA/Import-Export entries; reached via
  Settings. Desktop/extension/mobile expose both as top-level. Inconsistent IA between
  web and the other three.

### G7 — Reveal interaction pattern diverges (web/extension vs mobile/desktop)  (🟡)
- web/extension: `revealSecret` → JS string → rendered in a `mono` box (ZK not required
  per AGENTS.md desktop-api gate, but the *behavior* differs: plaintext sits in JS
  memory on web/extension, opaque handle on mobile/desktop).
- This is arguably *correct* per the ZK rule (reveal gate is desktop-only), but the
  *user-visible* reveal UX (inline vs native overlay) is not homogeneous. Document the
  intent so it isn't mistaken for a defect.

---

## 4. UI/UX Consistency Findings

### 4.1 Navigation structure
- **Canonical section set** (from `vautr-frontend-convergence`): dashboard, vault,
  projects, secrets, generator, machine-accounts, tokens, mfa, backup, settings.
- **web**: sidebar, 9 entries, MFA+backup nested. ✅ close.
- **extension**: 10 tabs, flat, no dashboard/settings. 🟡
- **mobile**: bottom-tab + "More" sheet, 13 routes. ✅ most complete.
- **desktop**: 11-item sidebar (incl Dashboard, excl Audit). ✅ close.

### 4.2 Design tokens / theme
- Shared token contract exists (`vautr-design-tokens-sot`); web/extension/mobile import
  the shared `tokens.*` (mobile `global.css` imported per VTR-099/101 rule). Desktop
  uses GPUI `theme::*` tokens. No hardcoded-palette divergence found in source.
- **One known past defect** (from skill): extension popup root lacked `dark` class →
  AA contrast fail; fixed by adding `dark` to root. Verify via axe if re-auditing.

### 4.3 Interaction patterns
- **Delete confirmation**: desktop ✅ confirm modal (VTR-104). web/extension/mobile ✅
  confirm dialogs (Radix/native). No immediate-delete defect found.
- **Empty states**: present on web (ProjectsTab CTA), mobile, extension. ✅
- **Loading skeletons**: `boneyard` wired on web/extension (`bones/registry.ts`) as
  non-regressive `<Skeleton>` wrapper; mobile has nativewind shimmer; desktop GPUI
  spinner. Partial homogeneity.
- **Toasts/errors**: web/extension use sonner-like toasts; mobile uses native toast;
  desktop uses `eprintln!`/inline `share_text`. Divergent but platform-appropriate.

### 4.4 Icons
- web/extension: Lucide. mobile: Lucide (nativewind). desktop: `gpui_component`
  `IconName` (limited set — `Shares` uses `IconName::Inbox`, not `Share2`; confirmed
  real variant list has no Share/Group). Vocabularies differ by necessity (GPUI ≠ Lucide)
  but section→meaning mapping is consistent.

### 4.5 Onboarding / first-run
- Present and convergent on web (`OnboardingFlow`), extension (footer replay), mobile
  (`__root` wrapper), desktop (`render_onboarding`, `~/.config/vautr/onboarding.json`).
  Single-source spec is `docs/onboarding/spec.md` (per skill). ✅ homogeneous.

---

## 5. Server / Core / CLI depth (no semi-implemented features found)

- **Server handlers** (`core/vautr-server/src/handlers/`): auth, account (incl.
  `account_rotate_key`), sharing (create_share, public-key PUT/GET, inbox, upload
  payload, revoke), mfa, backup — all routed and non-trivial. No `200`-with-empty-body
  stubs.
- **Transport trait** (`vautr-app-state` `Transport`): every method implemented (sync
  push/pull, `rotate_key`, sharing relay). No no-op impls.
- **Sharing endpoints**: all server handlers present AND called by SDK; desktop e2e
  (`share_accept_roundtrip_live`) exercises the full round-trip live → proven wired.
- **Cargo feature gates**: `desktop-api`/`mlock` are real (ZK reveal), not no-ops.
- **CLI**: commands present are fully implemented; the *absent* ones (G1) are the only
  gap.

---

## 6. Prioritized Recommendations

1. **G3 — desktop Audit** — add `Section::Audit` + `render_audit` reusing the web/ext
   `GET /audit` response. Small, high-value (parity with 3 clients).
2. **G1 — CLI share/rotate/export** — either document as intentional `bws` subset (add
   a `// NOTE` + README line) OR add `share`/`rotate`/`export`/`import` subcommands
   (SDK already supports all).
3. **G4 — extension Dashboard** — add a `dashboard` tab composing the existing stat
   tiles (mobile/desktop already have them).
4. **G5/G6 — canonical IA decision** — choose flat (desktop/mobile: MFA+backup top-level)
   vs nested (web/extension: under Settings) and make web + extension match. Pick one
   and apply.
5. **G2 — global Search** — new feature; lowest priority but a uniform miss. Add a
   `/search` route + SDK `search()` if product wants it.
6. **G7 — reveal UX doc** — add a one-line comment in web/extension `revealSecret`
   explaining the intentional ZK divergence vs mobile/desktop so future auditors don't
   mis-file it as a defect.

---

## 7. What is NOT a gap (do not re-raise)

- Mobile ZK reveal parity — **CLOSED** (VTR-104 + VTR-061). `MobileVautrClient.reveal`
  returns opaque handle; plaintext never in JS.
- Desktop delete confirm — **DONE** (VTR-104). Confirm modal + type-to-confirm.
- Shares read surface on desktop — **present** (`Section::Shares`, inbox+groups).
- Onboarding convergence — **present** on all four clients.
- Crypto core / server — **fully implemented**, no stubs.

---

## 8. Verification evidence used

- Repo-wide `unimplemented!`/`todo!()`/`FIXME` grep → clean.
- `apps/cli/src/cli.rs:27-53` subcommand list.
- `apps/web/src/routes/_authed.tsx:28-39` sidebar.
- `apps/extension/src/popup/App.tsx:191-218` tabs.
- `apps/mobile/src/routes/` route listing.
- `apps/desktop/src/desktop_view.rs:44-56` (Section enum), `3567-3577` (render match),
  `3706-3717` (sidebar tuple).
- `packages/vautr-client-sdk/src/realClient.ts:260-1013` method surface.
- `packages/vautr-client-sdk/src/mobile.ts` native reveal/share client.
- `apps/desktop/tests/live_server_e2e.rs` (5/5 live e2e green) proves sharing/lock/
  rotate wired end-to-end.

---

## 9. Gap-closure resolution (2026-08-21, post-audit)

The auditor re-grounded every gap against live source and closed the actionable ones.
G7 was investigated (no change needed). Findings:

- **G7 — Reveal-path divergence: CORRECTED / already homogeneous.** The §0 claim
  that "web/extension use JS-decrypt-then-render" was WRONG. All four clients consume
  secrets via the opaque-handle path: `reveal(uuid)` -> `OpaqueHandle` ->
  `performAction({CopyToClipboard})` -> `release()`. Plaintext never enters JS/React
  state. The `mlp.revealSecret` raw `GET /secrets/{uuid}/value` (returns
  `value_ciphertext`, never plaintext) is **unused** in the UI. -> No divergence;
  document, do not "fix".

- **G5 / G6 — IA nesting: CORRECTED / already satisfied.** The plan-doc claim that web
  nests MFA/Import-Export under Settings was stale. Live `_authed.tsx:36-38` exposes
  `/mfa`, `/import-export`, `/audit` as TOP-LEVEL sidebar entries; extension
  `App.tsx:40-50` exposes them as TOP-LEVEL tabs. -> No change needed.

- **G3 — Desktop Audit: CLOSED.** Added `Section::Audit` enum + sidebar entry +
  `render_audit` (GET /audit) + `load_audit` + state fields in
  `apps/desktop/src/desktop_view.rs`. `cargo check -p vautr-desktop` green.

- **G4 — Extension Dashboard: CLOSED.** Added `DashboardTab.tsx` (reuses shared Card
  tokens + store `projects`/`secrets` counts) and wired it as a top-level tab in
  `apps/extension/src/popup/App.tsx`. `pnpm typecheck` (extension) green.

- **G1 — CLI parity: PARTIALLY CLOSED (justified divergence).** Added `export`
  (POST /backup/export) and `import` (POST /backup/restore, base64 archive) — real,
  green (`cargo check -p vautr-cli`). `share` / `accept-share` / `rotate` are
  **intentionally NOT added**: they require the SDK orchestrator (`VautrMlpClient`)
  which the CLI deliberately excludes (it hand-rolls OPAQUE + project-key crypto for a
  `bws`-compatible zero-dep surface). Adding them would mean pulling the orchestrator
  into the CLI and contradicting its design intent — a justified, documented divergence,
  not a stub.

- **G2 — Global Search: CLOSED on all four clients.** Client-side filter over the
  already-decrypted local list, in each client's secrets/vault surface:
  - web `apps/web/src/routes/_authed/secrets.tsx` (search box + `visibleRows`).
  - extension `apps/extension/src/popup/components/SecretsTab.tsx` (`visibleSecrets`).
  - desktop `apps/desktop/src/desktop_view.rs` (`vault_search_input` + filter in
    `render_vault_content`).
  - mobile `apps/mobile/src/routes/_app.secrets.tsx` (`TextInput` + `visibleEntries`).
  All four `pnpm typecheck` / `cargo check` green.

- **Bonus fix:** `apps/extension/src/popup/components/InboxTab.tsx` — local
  `IncomingShare.payload` field renamed to `encrypted_payload` to match the SDK's
  `getShareInbox` return shape (was a pre-existing type error; now green).

Verification gate: `cargo check -p vautr-desktop -p vautr-cli` green; `cargo fmt`
clean; `cargo clippy` reported no new errors in touched crates; `pnpm typecheck` green
on web/extension/mobile.
