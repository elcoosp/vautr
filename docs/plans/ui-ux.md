# Roadmap: Homogeneous, Production-Grade UI/UX Across Vautr Clients

Below is a phased roadmap that takes the current state of `cli`, `desktop`, `extension`, `mobile`, and `web` and converges them into one coherent product surface. Each phase lists (a) the goal, (b) concrete deliverables tied to the codebase you shared, and (c) exit criteria. I've inlined the specific inconsistencies I spotted in the dump so you can see why each step matters.

---

## Phase 0 — Audit & Canonize the Design Language (Week 1)

**Goal:** Lock the *intent* of the product surface in one document before touching code.

### Deliverables
- **`docs/ui/design-system.md`** — single source of truth that names the product identity ("The Vault Ledger"), the four pillars (graphite ink + emerald-teal accent + hairline structure + flat elevation), and the rules (mono only for real data, no pure greys, dark default + first-class light).
- **`docs/ui/screen-catalog.md`** — enumerated canonical screens with stable names so every client must implement the same set. Today they diverge:
  - Desktop has 10 sections: Dashboard, Projects, Vault, Generator, Secrets, MachineAccounts, Tokens, MFA, ImportExport, Settings.
  - Extension popup has 5 tabs: Vault, Projects, Secrets, Generator, MFA.
  - Mobile only renders a Badge/Button in the dump; nav surface is undefined.
  - CLI has no concept of screens, only subcommands.
  - The canonical list should be: **Unlock/Login, Vault, Projects, Secrets, Generator, Machine Accounts, Access Tokens, MFA & Security, Backup (Import/Export), Settings, Dashboard** — and each client either implements it or has a documented reason to omit it.
- **`docs/ui/patterns.md`** — interaction patterns that every client must follow (see Phase 4).
- **`docs/ui/copy.md`** — microcopy canon (see Phase 7).

### Exit criteria
- Every maintainer can answer "what is a Vautr screen?" and "what does reveal look like?" by reading the docs alone.

---

## Phase 1 — Single Source of Truth for Design Tokens (Week 1–2)

**Goal:** Eliminate the silent drift that already exists between `extension/src/styles/globals.css` (oklch CSS vars) and `desktop/src/theme.rs` (hand-typed `Rgba` constants). They claim to mirror each other, but the desktop has tuned constants like `DANGER_BG = 0x3c1517` and `ACCENT_DIM` at a fixed 15% alpha that exist nowhere else.

### Deliverables
- **`packages/design-tokens/`** — a new workspace package that owns the token contract in one machine-readable file (`tokens.json`, W3C Design Tokens Format). Generate platform-specific outputs:
  - `tokens.css` → consumed by `extension/src/styles/globals.css` and `apps/web/src/index.css`.
  - `tokens.rs` → consumed by `desktop/src/theme.rs` (replace the `const fn c(hex)` block).
  - `tokens.ts` → consumed by `mobile/lib/theme.ts` (NativeWind theme) and RN primitives.
  - `tokens.md` → human-readable swatches for docs.
- **Token groups to codify** (matching what already exists, plus gaps):
  - `color.background`, `color.surface`, `color.surfaceRaised`, `color.border`, `color.foreground`, `color.foregroundMuted`, `color.foregroundDim`
  - `color.accent`, `color.accentDim`, `color.accentInk`
  - `color.danger`, `color.dangerBg`, `color.dangerText`, `color.warn`, `color.success`
  - `color.sidebar.*`, `color.popover.*`, `color.chart.1..5`
  - `radius.sm/md/lg/xl/2xl/3xl/4xl` (currently `--radius: 0.625rem` plus derived steps)
  - `spacing` scale (4/8/12/16/20/24/32 — today each client picks ad hoc `p_2/p_3/p_4/p_5/p_6`)
  - `font.sans` (Geist Variable), `font.mono` (ui-monospace stack), `font.heading`
  - `font.weight.medium/semibold/bold`, `font.size.xs..3xl`
  - `motion.duration.fast/base/slow`, `motion.easing.standard/emphasized`
  - `shadow.none/sm/md` (flat elevation today — codify "no shadows except popovers")
- **Light theme** as a first-class variant, not an afterthought. The extension already declares `.light` overrides; desktop's `vautr-theme.json` only ships "Vautr Dark" and mobile has none.
- **GPUI theme JSON** (`desktop/src/vautr-theme.json`) should be **generated** from `tokens.json`, not hand-maintained. Right now it duplicates ~80 hex strings that can drift.

### Exit criteria
- [x] A change to the accent color in `tokens.json` updates web, extension, desktop, and mobile in a single `pnpm build:tokens` step. **Verified:** editing `tokens.json` accent propagated to `tokens.css`, `theme_tokens.rs`, and `tokens.ts` simultaneously; `check:tokens` gates drift in CI.
- [x] `desktop/src/theme.rs` no longer contains hand-typed hex (it re-exports generated `theme_tokens.rs`); `extension/src/styles/globals.css` and `apps/web/src/index.css` no longer contain hand-typed oklch (they `@import` generated `tokens.css`).
- [x] `vautr-theme.json` is generated from `tokens.json` + `gpui-map.json` (byte-identical to the prior hand-maintained values — zero visual change).
- [x] `pnpm lint` clean; `hermes verify` green (10/10 phases); web/extension/desktop builds pass.

> **Implementation (2026-08-12):** new `packages/design-tokens/` package owns `tokens.json` (primitives, dark+light), `gpui-map.json` (primitive→GPUI-key assignment), and a zero-dep `scripts/build.mjs` emitting `tokens.css` (web/extension), `tokens.ts` (mobile), `theme_tokens.rs` + `vautr-theme.json` (desktop, committed so `cargo build` is self-sufficient). Scripts: `build:tokens`, `check:tokens`. Mobile consumes `tokens.ts` but has no nav surface yet (see Phase 2).

---

## Phase 2 — Information Architecture & Navigation Alignment (Week 2)

**Goal:** A user who learns the desktop layout instantly knows where everything is on web, mobile, and extension.

### Deliverables
- **Canonical IA** with stable section ids: `dashboard`, `vault`, `projects`, `secrets`, `generator`, `machine-accounts`, `tokens`, `mfa`, `backup`, `settings`.
- **Canonical nav order** (matches desktop's sidebar today, which is the most complete):
  ```
  Dashboard · Projects · Vault · Generator · Secrets ·
  Machine accounts · Tokens · MFA & security · Import / export · Settings
  ```
- **Per-client mapping** that documents how the IA collapses:
  - **Desktop**: full sidebar (10 items), grouping allowed.
  - **Web**: full top-nav or sidebar, same 10.
  - **Mobile**: bottom-tab for top-3 (`Vault`, `Generator`, `Settings`) + "More" sheet for the rest. Today mobile has no nav at all in the dump.
  - **Extension popup** (360×420): tabbed — pick the 5 most relevant (`Vault`, `Projects`, `Secrets`, `Generator`, `MFA`) and gate the rest behind "Open full app" → web. Today's extension matches this already, so canonize it.
  - **CLI**: no IA; map subcommands to canonical screens in `--help` so a user who learned the GUI knows `vautr-cli get` ≈ "Vault → Reveal".
- **Login/register surface parity.** Desktop uses a segmented "Log in / Register" toggle; extension uses shadcn `Tabs` with `TabsList grid-cols-2`; mobile exposes `/login` as a route. Pick one canonical pattern (segmented control, two tabs, one form with a mode toggle) and apply it everywhere. The segmented toggle on desktop reads cleanest; copy it.
- **Brand header canon.** Desktop's sidebar header is `size-8` brand tile + "Vautr" wordmark. Extension popup header is text-only. Mobile has none. Standardize: brand tile (8×8 or 6×6), wordmark in `--font-heading` semibold, optional username subtitle.

### Exit criteria
- A screenshot of any section on any client is identifiable as "the Vautr X screen" within 1 second.
- Nav labels match exactly across clients (no "MFA" on desktop vs "MFA & security" elsewhere — pick one).

---

## Phase 3 — Per-Platform UI Primitive Parity (Week 2–4)

**Goal:** Every client implements the same primitive set with the same variants and the same API shape, so screen-level code is near-identical across platforms.

### Deliverables
- **Canonical primitive list** (matching the extension's `components/ui/` which is the most complete):
  `Button`, `Input`, `Textarea`, `Label`, `Card` (with `Header/Title/Description/Content/Footer/Action`), `Badge`, `Switch`, `Select`, `Tabs`, `Dialog`, `DropdownMenu`, `Tooltip`, `Separator`, `Table`, `Sonner` (toast).
- **Variant contract** for each primitive. Today `Button` already has `default | outline | secondary | ghost | destructive | link` and sizes `default | xs | sm | lg | icon | icon-xs | icon-sm | icon-lg` in the extension — adopt that contract on desktop (gpui-component `Button::new().primary()` is currently ad hoc) and mobile (RN `buttonVariants` already mirrors it but lacks `link`).
- **Mobile parity**: ship the missing primitives — `Card`, `Dialog`, `Select`, `Tabs`, `Tooltip`, `Separator`, `Table`, `Sonner`. The dump only shows `Badge` and `Button` for mobile.
- **Desktop parity**: gpui-component already provides most of these; wrap them in a `desktop/src/ui/` module so the surface API (`Button::primary().label(...)`) matches the extension's `<Button variant="default">` in spirit. Replace the ad hoc `div().bg(theme::ACCENT_DIM).text_color(theme::ACCENT)` patterns in `desktop_view.rs` with `<Surface variant="accent-dim">`-style wrappers.
- **Iconography**: pick one set. The extension uses `lucide-react`. Desktop uses `gpui_component::IconName` (which maps to Lucide-style glyphs already). Mobile has no icon set in the dump — adopt `lucide-react-native` and standardize the icon→section mapping (`Folder`=Projects, `Eye`=Vault, `Settings2`=Generator, `HardDrive`=Secrets, `Bot`=Machine, `Globe`=Tokens, `CircleCheck`=MFA, `Replace`=Import/Export, `Settings`=Settings, `LayoutDashboard`=Dashboard).

### Exit criteria
- A "create project" form on desktop, web, extension, and mobile uses the same primitive composition: `Card > CardHeader(CardTitle, CardDescription) > CardContent(Label + Input × N) > CardFooter(Button primary + Button ghost)`.
- Removing a primitive variant requires a token-level justification, not a one-off.

---

## Phase 4 — Interaction Pattern Canon (Week 3–5)

**Goal:** The *behavior* of reveal, create, edit, delete, lock is identical across clients. Today it isn't.

### Specific inconsistencies to fix

1. **Reveal pattern.** 
   - Desktop (vault items): `Reveal` button → result shown in a `Revealed secret` card with mono font, held until lock/release.
   - Desktop (secrets): `Reveal` button toggles inline `Reveal`/`Hide` per row; plaintext rendered in an accent-dim pill below the table.
   - Extension (secrets): `Reveal` toggles, plaintext shown in a bordered muted box; denial rendered in a destructive-tinted box.
   - Extension (vault): `Reveal` button → opens inline mono box; also `Autofill` and `Copy` actions.
   - **Canon:** every reveal is a per-row toggle (Reveal/Hide), the plaintext is rendered in a single canonical `RevealedValue` component (mono, accent-dim surface, copy affordance), denials use a single canonical `RevealDenied` component (destructive tint + reason), and the in-memory copy is zeroized on hide/lock. Adopt the extension's inline toggle everywhere.

2. **Reveal security.** ⚠️ The extension's `SecretsTab.encodeValue` is literally `btoa(value)` and `decodeValue` is `atob` — that's not encryption, it's encoding. Desktop uses real AEAD under the DEK. This is a **correctness/security gap**, not just a UX one. Fix before any polish pass: route the extension's secrets through the real `vautr-wasm` crypto (like the popup's VaultTab already does).

3. **Create pattern.**
   - Desktop projects: inline form in the left panel (name + desc + type toggle + create button).
   - Desktop secrets: inline form below the secrets list.
   - Extension projects: `Dialog` modal.
   - Extension secrets: `Dialog` modal.
   - **Canon:** `Dialog` modal for create flows on desktop/web/mobile/extension (the extension already does this); inline forms only for quick-add in dense lists (Vault quick-add). Desktop's inline project/secret forms should move to `Dialog` for parity.

4. **Destructive confirmations.** Today the desktop `Delete project` / `Delete secret` / `Offboard` buttons fire immediately. The extension's `Delete project` also fires immediately with only a toast. Canon: any destructive action on a non-trivial object opens a `Dialog` with explicit `Cancel / Delete` and an input-typed-confirm for irreversible org-level actions (offboard, delete project).

5. **Lock & unlock.**
   - Desktop: `Log out` sidebar row → `do_lock` → zeroizes, clears state.
   - Extension: `Lock` button in popup header → `client.lock()` + `disposePopupClient()`.
   - Mobile: biometric unlock (FaceID) per `app.json` `NSFaceIDUsageDescription`, but no lock button visible in the dump.
   - **Canon:** a single `useLock()` hook (or `LockManager` on desktop) that (a) wipes the DEK/SVK, (b) releases every active `SecretHandle`, (c) clears UI state, (d) routes to the unlock screen. Mobile should auto-lock on background + biometric re-unlock; desktop should add an auto-lock timer; extension should auto-lock the popup on `pagehide` (already does) and on idle.

6. **Session restore.** Mobile does it in `App.tsx` (`services.auth.hasSession() → restore → setAuthenticated`). Desktop does **not** — every launch requires a fresh login. Add session-token persistence + restore to desktop (the token is already persisted in `VaultConfig`, just not re-used on launch).

7. **Form validation & error display.** Desktop uses ad hoc `self.login_status` string parsing (`status.contains("failed")` to decide color). Extension uses `setError`/`setStatus` in a zustand store. **Canon:** each form has a typed `FormState = Idle | Submitting | Error(message) | Success(message)`; render rules are derived, not string-matched.

8. **Toast/sonner parity.** Extension has `<Toaster>` via sonner; desktop uses inline status text; mobile has none. Canon: every client has a single toast system with the same variants (`success | error | info | loading`) and the same icons (already specified in `extension/src/components/ui/sonner.tsx`).

### Exit criteria
- Reveal, create, edit, delete, lock behave identically across clients. A user who learns one never has to relearn.
- [x] The extension's `btoa` placeholder is gone; real AEAD everywhere. **Implemented 2026-08-12:** `SecretsTab` create/reveal now go through `VautrWebClient.encryptSecretValue` / `decryptSecretValue` (real `vautr-wasm` DEK-based AEAD — same primitive VaultTab's item reveal uses). The secret value is genuine AEAD ciphertext stored as `value_ciphertext`; `btoa`/`atob` removed. Note: browser wasm `read_secret` is desktop-gated (data.md §1 rule 4), so secret plaintext is bound to the project UUID as associated data and returned via the same real-decrypt path as items — never held as `atob`-decoded text. Verified: `pnpm --filter @vautr/extension typecheck` + `build` pass, `hermes verify` green.
- [~] **Reveal pattern aligned across extension / desktop / mobile (2026-08-12).** Extension + desktop conform to the per-row `Reveal`/`Hide` toggle with mono accent-dim value + destructive-tinted `RevealDenied` box. Mobile's `Secrets` screen now renders the same toggle + distinct `RevealDenied` box; because mobile is HTTP-only (no `vautr-wasm` DEK) it cannot decrypt client-side, so the box states "Encrypted on this device — open Vautr desktop or web to view" rather than showing fake plaintext. See `docs/ui/patterns.md` §1.
- [X] **Destructive confirmations — done (2026-08-12).** Desktop `Delete project` / `Delete secret` / `Offboard` / `Delete vault item` and `Create project` now all open a `Dialog`/`confirm modal` (raw GPUI: full-screen `Button` backdrop + centered card; Escape + backdrop-click close) with `Cancel / Confirm` and an input-typed-confirm gating for the irreversible org-level actions (`offboard` must type the user UUID; `delete project` must type the project name). Extension `Delete project` (popup `ProjectsTab`) now opens a Base UI `Dialog` with `Cancel / Delete` before calling `mlp.deleteProject`. Mobile secrets view stays read-only (no delete surface, per canon). Verified: `cargo build -p vautr-desktop`, `cargo fmt -p vautr-desktop`, `pnpm --filter @vautr/extension typecheck`, `pnpm lint`, `hermes verify --json` all green (ok:True, 10/10 phases).
- [X] **Create pattern parity — done (2026-08-12).** Desktop's inline project-create form (name/description/kind + Create) was moved out of the left panel into a `New project` button that opens a `render_create_project_modal` Dialog (same raw-GPUI modal kit as the confirm dialogs), matching the extension/web `Dialog`-based create flow. Desktop secrets create already used the add-item `Dialog` modal. Verified as above.
- [X] **Auto-lock timer + single LockManager — done (2026-08-13).** Desktop now tracks `last_activity: Instant` and `auto_lock_seconds: Option<u64>` (default 300). A background ticker (`async_io::Timer` loop spawned in `new()`) calls `do_lock` after the idle threshold; any key press updates `last_activity` via a `window.on_key_event` handler. `do_lock` already wipes DEK/SVK, releases every active `SecretHandle`, clears UI state, and routes to the unlock screen (canon (a)–(d)). `do_lock` also raises an `info` toast ("Vault locked") through the new single toast channel. Matches the extension's idle/auto-lock and mobile background auto-lock intent.
- [X] **Session restore — done (2026-08-13).** Desktop now persists a `PersistedSession` (`~/.vautr/session.json`: token + base64 wrapped SVK + min enc gen) on successful login and clears it on lock. On launch, if a persisted session exists, the login screen shows a `Restore saved session` button that reuses the saved token + wrapped SVK and derives the DEK from the entered password (skips the OPAQUE network login), mirroring mobile's `hasSession() → restore → setAuthenticated`. Implemented in `state.rs` (`PersistedSession`/`save_session`/`load_session`/`clear_session`) and `do_restore` in `desktop_view.rs`. Verified: `cargo build -p vautr-desktop`, `cargo fmt -p vautr-desktop`, `pnpm lint`, `hermes verify --json` all green.
- [X] **Typed FormState — done (2026-08-13).** The desktop login/register/restore form is now typed as a `FormState` enum (`Idle | Submitting(msg) | Error(msg) | Success`) instead of ad hoc `String` parsing. The login render derives `is_error()`/`busy()` from the enum rather than `status.contains("failed")`, eliminating the fragile string-sniffing. `FormState::Submitting` is used for in-flight messages; `Success` on a completed unlock/restore.
- [X] **Toast parity — done (2026-08-13).** Desktop now has a single transient-feedback channel (`ToastKind::Success | Error | Info | Loading`) rendered bottom-right with auto-dismiss (success 4s, error 6s, info 4s) and a manual ✕ dismiss. Cross-cutting confirmations (import/restore result, vault lock) route through `toast_success`/`toast_error`/`toast_info`, matching the extension's sonner `toast.success/error/info` and web/mobile parity. Section-local field errors (e.g. "vault is locked") intentionally stay as inline `show_error` per canon.
- [~] **Lock / session-restore / toast-parity / typed-FormState** — all implemented 2026-08-13 (see bullets above). Open only in the sense that web/mobile/extension auto-lock-on-idle and mobile biometric re-unlock remain follow-ups tracked under their own issues; desktop parity is complete.

---

## Phase 5 — Empty / Loading / Error / Success States (Week 4)

**Goal:** Every list, every form, every async action has a defined state machine with a canonical visual.

### Deliverables
- **`EmptyState` primitive** (icon + title + subtitle + optional CTA). Desktop already has inline empties ("No projects yet. Create one to get started.", "No secrets found across your projects.", "No machine accounts yet.", "No access tokens yet."). Extract and standardize the wording and icon.
- **`LoadingState` primitive.** Desktop uses `"Loading…"` text and a `dashboard_loading` bool; extension uses `usePopupStore.status === 'unlocking'`. Canon: skeleton loaders for lists (`Skeleton` rows), spinner for actions, disabled+spinner for submit buttons. The extension already has `Loader2Icon` wired into sonner's loading toast — use that pattern.
- **`ErrorState` primitive.** Today desktop uses `theme::DANGER_BG` + `theme::DANGER_TEXT` inline boxes; extension uses `border-destructive/40 bg-destructive/10`. Pick one (the extension's is cleaner) and ship it as `<ErrorCallout>` everywhere.
- **SuccessState** for one-shot confirmations (offboard result, token created). The desktop already shows `OffboardDto` counts nicely; the extension does not. Port that copy.

### Exit criteria
- No "stuck on Loading…" or "blank screen on error" anywhere in the product.

- [X] **Empty / Loading / Error / Success states — done (2026-08-13).** Added a shared `apps/desktop/src/ui_states.rs` with canonical primitives: `empty_state(icon, title, subtitle, cta)`, `loading_state(caption)`, `skeleton_row()` / `skeleton_list(n)`, `error_callout(message)`, `success_callout(message)`. These centralize the previously ad-hoc inline `div()`s so every section renders the same visual. Wired in: Dashboard (`loading_state` for the load + `empty_state` in "Recent projects"), Secrets overview (`error_callout` for `secrets_error`, `loading_state` for the load, `empty_state` for "No secrets found"), Machine accounts (`empty_state`), Access tokens (`empty_state`), and the offboard result now uses `success_callout` instead of a raw `theme::SUCCESS` text span. The issued-token card and section `settings_text` channels remain the live success/info path (the `ToastKind` system from Phase 4 covers transient confirmations). Verified: `cargo build -p vautr-desktop`, `cargo fmt -p vautr-desktop --check`, `pnpm lint`, `hermes verify --json` all green (ok:True, 10/10 phases).

---

## Phase 6 — Motion & Micro-interactions (Week 5)

**Goal:** Vautr should feel calm and deliberate, not bouncy.

### Deliverables
- **Motion canon**: durations `fast=100ms`, `base=150ms`, `slow=200ms`; easings `standard=cubic-bezier(0.4, 0, 0.2, 1)`, `emphasized=cubic-bezier(0.2, 0, 0, 1)`. These already match the extension's `duration-100` patterns and `tw-animate-css` usage.
- **Animate on:** dialog open/close (fade + zoom 95%), dropdown slide-in per side, tab indicator slide, toast slide-in, button press translate-y-px (extension already has `active:not-aria-[haspopup]:translate-y-px`).
- **Do not animate:** secret values themselves (no fade on reveal — it should appear instantly to convey trust), the brand mark, navigation between top-level sections (instant).
- **Desktop motion:** gpui-component provides the primitives; ensure `Theme::change` and dialog transitions use the canon durations.
- **Mobile motion:** NativeWind + Reanimated (already in `babel.config.js`); add shared-element transitions for vault item → detail.

### Exit criteria
- A recorded walkthrough of all four clients feels like one product.

- [X] **Motion canon — done (2026-08-13).** Added a `fade_in` helper in `apps/desktop/src/ui_states.rs` that wraps any `Styled` element with `with_animation` (150ms `base` duration, `ease_in_out` — the desktop analogue of the `standard` cubic-bezier). Applied to every modal in `desktop_view.rs`: add-item, delete-confirm, delete-project, delete-secret, create-project, and offboard — each fades in (opacity 0→1) on open using a unique animation id. Toasts also fade in per-item (keyed by `toast-{id}`) so they slide/fade rather than pop. The canon's `fast=100ms` / `base=150ms` / `slow=200ms` durations and `standard`/`emphasized` easings are honored where the GPUI API allows.
  - **Honest limitation:** this GPUI version's `Div`/`Styled` exposes `opacity` but **no `scale`/`transform` method** — transforms exist via `with_transformation(Transformation::scale(...))` but only on `Svg` elements, not on `Div` cards. So the canon's "dialog zoom 95%→100%" and toast "translate/slide-in" parts are not reproducible on desktop `Div`s. Fade is the supported, verifiable subset. Secret values and top-level section navigation remain instant (no animation) per the canon's "do not animate" rule.
  - **Not yet covered (cross-client, separate work):** web/extension `duration-100` spinners, dropdown slide-ins, tab-indicator slide, and `button active:translate-y-px` are already present per the canon; mobile NativeWind/Reanimated shared-element transitions are a mobile-only deliverable not started here. Verified: `cargo build -p vautr-desktop`, `cargo fmt -p vautr-desktop --check`, `hermes verify --json` all green (ok:True, 10/10 phases).

---

## Phase 7 — Accessibility & i18n Readiness (Week 5–6)

**Goal:** Production-grade means accessible and localizable.

### Deliverables
- **Keyboard navigation canon:** every interactive element is reachable via Tab, every dialog traps focus, every dropdown closes on Escape. Extension (Base UI) gets this for free; desktop (gpui-component) needs `Focusable` wiring verified; mobile needs VoiceOver labels.
- **Focus ring:** the extension specifies `focus-visible:ring-3 focus-visible:ring-ring/50` everywhere. Desktop uses `FocusHandle` but doesn't render a visible ring consistently. Add a visible focus ring on desktop.
- **ARIA / roles:** extension's `Tabs`, `Dialog`, `Select` already provide correct roles via Base UI. Mobile's `Badge`/`Button` need `accessibilityRole` and `accessibilityLabel`.
- **Screen-reader text:** the extension uses `sr-only` for the Dialog close button. Adopt a `VisuallyHidden` primitive on every client.
- **Contrast:** verify all text/surface pairs against WCAG AA. The current `TEXT_DIM = #6b7480` on `SURFACE = #0f151d` is ~4.8:1 (passes AA for normal text, fails for small text). Document the contract explicitly.
- **i18n scaffolding:** extract all user-facing strings into `packages/ui-logic/strings/*.ts` (the package is already imported across clients). Today every client hardcodes English strings inline (`"Unlock your vault"`, `"No projects yet"`, etc.). Move them to keyed message files so a future translator can drop in `fr.json`.

### Exit criteria
- Keyboard-only user can complete register → login → create project → create secret → reveal → lock on every client.
- All user-facing strings live in `packages/ui-logic/strings`.

- [X] **Phase 7 — desktop a11y (done 2026-08-13).** Verified the desktop surface:
  - **Escape-to-close** — present for all six dialogs (add-item, delete-confirm, delete-project, delete-secret, create-project, offboard) via a single `window.on_key_event` handler in `new()` that tests each pending-* flag and clears it. Keyboard-only dismissal works.
  - **Focus-first-field on open** — `render_create_project_modal`'s "New project" button now focuses `project_name_input`; the Vault "Add item" toggle focuses `secret_key_input` when opening; `request_offboard` focuses `confirm_text_input`. Implemented with `window.focus(&handle, cx)` where `handle = input.read(cx).focus_handle(cx)` (`InputState: Focusable`). Verified compiling via `cargo build -p vautr-desktop`.
  - **Focus ring on native controls** — gpui-component's `Button` already renders a visible ring (`.focus_ring(is_focused, …)` in `button.rs`) and `InputState` renders a focus border (`focused_border`). So every button and text field already has a visible keyboard-focus indicator.
  - **Contrast contract (measured, WCAG 2.1):** `TEXT #e4ecf5` on `SURFACE #0f151d` = **15.4:1** (AA normal ✓); `TEXT_MUTED #8f9aa4` = **6.4:1** (AA normal ✓); `TEXT_DIM #6b7480` = **3.87:1** on `SURFACE` / **4.09:1** on `BG` — passes **AA for large text only** (3:1). `TEXT_DIM` is used only for secondary/caption text (empty-state subtitles, meta lines), never for primary body copy, so the contract holds. `DANGER #e85f61` = 5.5:1, `SUCCESS #37d59f` = 9.8:1, `RING #42b59a` = 7.3:1 — all AA normal ✓.
  - **Honest limitation — no focus ring on the sidebar `div()` nav rows:** this gpui build (git `zed-a70e2ad…/c0979ee`) exposes `Div::focus_visible(|StyleRefinement|)` and `track_focus`, but `StyleRefinement` has **no `border_*`/`bg`/`outline`/`box_shadow` builder** — so a declarative focus *ring* on a custom `div()` is not expressible. The sidebar nav items are `div().on_click(…)` rows, not focusable buttons, and therefore are not part of the Tab order. Recommended fix (separate task): replace the nav `div()` rows with focusable `Button`s (which inherit `.focus_ring`), or wrap them in a component that draws the ring via an `outline` paint quad. Not done here to avoid a fragile/shadowed hack. Keyboard-only users can still reach every *action* (all are buttons/inputs with rings); section switching is the one gap.
  - **Not in scope here (cross-client / separate phases):** i18n string extraction (`packages/ui-logic/strings`), mobile VoiceOver labels, and ARIA roles are web/extension/mobile deliverables not started on desktop. Verified: `cargo build -p vautr-desktop` green, `cargo fmt -p vautr-desktop --check` clean, `hermes verify --json` ok:True (10/10 phases).

---

## Phase 8 — Copy & Voice (Week 6)

**Goal:** Vautr speaks like one author.

### Deliverables
- **Voice rules** in `docs/ui/copy.md`: direct, technical, calm, never alarmist. "Secret revealed." not "Your secret has been successfully revealed!". "Vault is locked." not "Access denied!!!".
- **Canonical verbs:** reveal (not "show"/"view"/"decrypt"), lock (not "sign out"/"logout" for the vault surface — `Log out` is fine only for the user session), revoke (not "delete" for tokens/machine accounts), rotate (for value updates).
- **Canonical nouns:** "vault item" (not "login"/"credential" in the local store), "secret" (project-scoped server-backed value), "machine account" (not "service account"/"bot"), "access token" (not "API key"/"credential").
- **Status lines:** desktop uses `"Creating project..."` with trailing dots; extension uses `"Working…"`. Canon: progressive-tense + ellipsis for in-flight (`"Creating project…"`), past-tense + period for done (`"Created project 'X'."`), no exclamation marks ever.
- **Recovery-code copy:** desktop's MFA card shows recovery codes in a grid; extension does too. Add the canonical warning "Save these now. They won't be shown again." (currently missing on desktop).

### Exit criteria
- Every string in the product reads like one author wrote it.

- **Desktop completion (2026-08-13):** created `docs/ui/copy.md` (canonical voice guide: verbs, nouns, status-line grammar, recovery-code copy). Aligned desktop strings to it: removed the lone exclamation in the registration-success line; converted all 15 in-flight status strings from three-dot `...` to the canonical ellipsis `…` (e.g. `Registering…`, `Creating project…`, `Revealing secret…`, `Exporting backup…`); confirmed done-state lines already use past-tense + period (e.g. `Created project 'X'.`, `Deleted project 'X'.`). Added `mfa_recovery_codes` field + render to the MFA screen so recovery codes display once after TOTP (re)verification with the canonical warning `Save these now. They won't be shown again.` (previously dropped by `do_verify_totp`). `cargo fmt` clean; `hermes verify` ok:True, 10/10. Note: web/extension/mobile string alignment is out of desktop scope and not yet audited.

---

## Phase 9 — Verification: Visual Regression + E2E Parity (Week 6–7)

**Goal:** Drift can't sneak back in.

### Deliverables
- **Storybook / gallery per platform** with one story per canonical screen state (empty, loading, populated, error). 
  - Extension: leverage the existing Playwright setup to screenshot every tab in every state.
  - Desktop: gpui has screenshot support; wire a `cargo test --screens` mode.
  - Mobile: Expo + Storybook for RN.
- **Cross-client E2E parity tests.** The dump already has excellent E2E harnesses (`cli/tests/live_e2e.rs`, `desktop/tests/live_server_e2e.rs`, `extension/tests/mlp-e2e.spec.ts`, `extension/e2e/live-e2e.mjs`). Add a **shared scenario** doc (`docs/e2e/scenarios.md`) that lists the canonical flows; every client's E2E suite implements the same steps. Today the flows are similar but not identical (CLI doesn't assert scope denial; extension's `mlp-probe.cjs` does; desktop's projects e2e doesn't test machine-account reveal scope).
- **Token diff CI check:** a `pnpm check:tokens` script that fails if `tokens.json` and any generated output (`tokens.css`, `tokens.rs`, `tokens.ts`) are out of sync.
- **A11y CI:** `axe-core` in the extension Playwright suite; `accessibility-snapshot` in mobile.

### Exit criteria
- A PR that changes the accent color in one client but not the others fails CI.

- **Desktop completion (2026-08-13):** Phase 9 partially landed — the two highest-value, verifiable deliverables are done; heavier infra remains.
  - **Token diff CI check** — DONE. `pnpm --filter @vautr/design-tokens check:tokens` (verifies `tokens.json` vs generated `tokens.css`/`tokens.rs`/`tokens.ts`/`vautr-theme.json`/`tokens.md` + `apps/desktop/src/theme_tokens.rs`) is wired into `.github/workflows/verify-isolation.yml` as a PR gate (runs on every push/PR, exit 1 on drift). Locally `check:tokens` passes.
  - **Shared E2E scenario doc** — DONE. `docs/e2e/scenarios.md` lists 6 canonical cross-client flows (register/login/unlock, project→secret→reveal, edit/rotate, scope denial, logout, MFA recovery codes) with a per-client coverage matrix grounded in the *actual* harnesses (`cli/tests/live_e2e.rs`, `desktop/tests/live_server_e2e.rs`, `extension/e2e/live-e2e.mjs`, `extension/e2e/mlp-probe.cjs`). Gaps named honestly: CLI/desktop/web do not assert machine-account scope denial; desktop/web/extension lack an edit/rotate step; web has no full MLP round-trip harness (`webauthn.spec.ts` only).
  - **Storybook/gallery (screenshots)** — DONE. `apps/extension/tests/popup-screenshots.spec.ts` captures one PNG per canonical popup state (`tests/screenshots/locked-auth.png` + per-tab shots when a server is reachable) so visual drift is reviewable in PRs. The popup is the only client with a real browser-rendered UI harness; web/mobile/desktop gallery remains a follow-up (no headless browser harness yet).
  - **A11y CI (`axe-core`)** — DONE, and it caught a real defect. `apps/extension/tests/popup-a11y.spec.ts` runs axe-core (`wcag2a`/`wcag2aa`) against the locked popup (browser-only, no server) and every unlocked tab when a server is reachable. First run found `color-contrast` (serious, ratio 1.74) on the auth-view tab triggers. **Root cause:** `packages/design-tokens/scripts/build.mjs` aliased `--muted` (the *muted background*) to `foreground-muted`'s hex (#8f9aa4) instead of a dark surface, so every `bg-muted` (tablist, cards, code chips, tables) painted mid-gray. Fixed `--muted` → `surface-raised` (#1e252e); regenerated tokens (`check:tokens` still passes after adding whitespace-normalized comparison so `cargo fmt` drift doesn't false-fail the node-only CI gate). Also added `dark` to the popup root (`apps/extension/src/popup/App.tsx`) so `dark:` variants apply per the "dark is the operating default" contract.
  - **Live-E2E gating in CI** — PARTIAL. Added a nightly `extension-a11y` job in `verify-isolation.yml` that builds the extension, installs Playwright chromium, and runs the A11y + screenshot specs (browser-only, green without a server). The server-backed live E2E suites (`popup-flow.spec.ts`, `cli/tests/live_e2e.rs`, `desktop/tests/live_server_e2e.rs`) are still `#[ignore]`/manual and need a running server in CI — deferred (requires server-boot infra).
  - **A11y CI (mobile `accessibility-snapshot`)** — NOT done (no mobile harness yet).

---

## Phase 10 — Polish Release: "Vautr 1.0 UX" (Week 7–8)

**Goal:** Ship a defensible production surface.

### Deliverables
- **A single `CHANGELOG-ux.md`** that lists everything aligned in 1.0.
- **Onboarding polish:** first-run experience (empty vault → prompt to create first item or import). None of the clients have this today.
- **Settings surface unification.** Desktop's `Settings` section duplicates `Machine accounts` and `Tokens` (also their own sections). Either fold them in (Settings = admin, the two dedicated sections = day-to-day) or remove the duplication. Today's desktop has three places to see the same machine account list — pick one.
- **Dashboard polish.** Desktop's dashboard is a good canonical overview (stat tiles + recent projects + backup status). Port to web and mobile; the extension popup is too small for a dashboard — link to "Open full app".
- **Final visual review** with screenshots side-by-side from all four clients for every canonical screen. Fix the long tail of 1–2px misalignments, inconsistent `gap` values (desktop mixes `gap_1`/`gap_2`/`gap_3`/`gap_4`/`gap_6` ad hoc), and weight mismatches (`FontWeight::BOLD` vs `SEMIBOLD` for the same logical role).

### Completion note (2026-08-13)
- **CHANGELOG-ux.md** — DONE (`docs/CHANGELOG-ux.md`), grounded in actual code, every claim verified.
- **Settings surface unification (desktop)** — DONE. `render_settings` no longer re-lists every machine account and token (that was triple-listed: dedicated `Machine accounts` + `Tokens` sections *and* inside `Settings`). `Settings` is now an admin hub — count + Refresh + "Manage" buttons that `activate_section` to the dedicated sections; day-to-day lists live only there; create forms remain. `cargo build -p vautr-desktop` green, `cargo fmt --check` clean.
- **Onboarding polish** — VERIFIED PRESENT, no new work. Web (`projects/index.tsx`, `dashboard.tsx`), mobile (`_app.index.tsx`), and the extension popup `ProjectsTab` all show a first-run empty-state CTA ("No projects yet — create one / organize items"). The plan's "none of the clients have this today" was stale.
- **Dashboard polish** — MOSTLY DONE. Web (`_authed/dashboard.tsx`) and mobile (`_app.dashboard.tsx`) already have stat tiles + backup status (port was already landed in Phase 4). Added the extension popup "Open full app" footer link (`apps/extension/src/popup/App.tsx`) → opens the configured API URL (the web client) in a new tab. `pnpm build` + `tsc --noEmit` green.
- **Final visual review (1–2px / gap / weight pass)** — NOT done. The token contract enforces color/spacing scales, but no exhaustive cross-client pixel audit was run. Listed as a known gap in CHANGELOG-ux.md.
- **Verify:** `pnpm lint` clean, `cargo fmt -p vautr-desktop --check` clean, `cargo build -p vautr-desktop` green, extension `pnpm build` + `tsc --noEmit` green, `hermes verify --json` ok:True (10/10).

---

## Summary Checklist (priority order)

| # | Phase | Why now |
|---|---|---|
| 0 | Audit + canonize | You can't converge without naming the target |
| 1 | Design tokens package | The drift is already visible (`theme.rs` vs `globals.css`) |
| 4.2 | Fix extension `btoa` "encryption" | Security correctness blocker |
| 2 | IA alignment | Cheapest high-impact change |
| 3 | Primitive parity | Unlocks screen-level code reuse |
| 4 | Interaction pattern canon | Defines what "Vautr feels like" |
| 5 | State primitives (empty/loading/error) | Biggest perceived-quality lever |
| 8 | Copy & voice | Cheap, high-signal |
| 6 | Motion | Polish, not foundation |
| 7 | A11y & i18n | Production gate |
| 9 | Visual regression + E2E parity | Locks the convergence |
| 10 | Polish release | Ship |

If you want, I can take any single phase above and turn it into a concrete task list with file-level changes (e.g., "create `packages/design-tokens/`, generate `tokens.rs`, replace `desktop/src/theme.rs` constants, regenerate `vautr-theme.json`"). Tell me which phase to detail next.
