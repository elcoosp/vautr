# Vautr Interaction Patterns (canon)

> Phase 0 canon. Defines *how* reveal / create / edit / delete / lock behave so
> every client is identical. Today they diverge; this doc names the target.
> Code changes for these live in Phase 3 (primitives) and Phase 4 (behavior).
>
> Companion docs: [design-system.md](./design-system.md),
> [screen-catalog.md](./screen-catalog.md), [copy.md](./copy.md).

---

## 1. Reveal (highest-traffic pattern)

**Canon:** every reveal is a per-row **toggle** (`Reveal` / `Hide`). The
plaintext renders in a single canonical `RevealedValue` component — mono font,
accent-dim surface, copy affordance. Access denials render in a single canonical
`RevealDenied` component — destructive tint + reason. The in-memory copy is
**zeroized on hide / lock**.

**Current state (2026-08-12):**
- Extension (secrets + vault): per-row `Reveal`/`Hide` toggle, plaintext in a mono
  `bg-muted/40` box, denials in a destructive-tinted box. **Conforms.**
- Desktop (vault items + secrets): per-row toggle, plaintext in an accent-dim
  surface, held until lock/release. **Conforms** (zeroizes on lock via `do_lock`).
- Mobile (secrets): per-row toggle. Mobile is HTTP-only and ships **no
  `vautr-wasm` DEK**, so the server returns ciphertext it cannot decrypt
  client-side. The honest canonical adaptation is a distinct destructive-tinted
  `RevealDenied` box ("Encrypted on this device — open Vautr desktop or web to
  view"), not a fake-decrypted value. **Pattern aligned 2026-08-12**; full
  plaintext reveal on mobile requires wiring `vautr-wasm` into the app (open
  item).
- → Reveal pattern is aligned across ext / desktop / mobile. Remaining: mobile
  client-side decryption (architectural — add the wasm crypto client).

**Reveal security (correctness blocker — Phase 4.2):**
- ⚠️ `apps/extension/src/popup/components/SecretsTab.tsx` `encodeValue` is
  literally `btoa(value)` and `decodeValue` is `atob` — **encoding, not
  encryption.** Desktop uses real AEAD under the DEK. The popup's `VaultTab`
  already routes through real `vautr-wasm` crypto; `SecretsTab` must too. Fix
  before any polish pass (see [../plans/ui-ux.md](../plans/ui-ux.md) Phase 4.2).

---

## 2. Create

**Canon:** `Dialog` modal for create flows on desktop / web / mobile /
extension. Inline forms only for quick-add in dense lists (Vault quick-add).

**Current state:**
- Desktop projects: inline form in the left panel.
- Desktop secrets: inline form below the list.
- Extension projects & secrets: `Dialog` modal.
- → Move desktop's inline project/secret forms to `Dialog` for parity.

---

## 3. Destructive confirmations

**Canon:** any destructive action on a non-trivial object opens a `Dialog` with
explicit `Cancel / Delete`. Irreversible org-level actions (offboard, delete
project) require an **input-typed confirm** (type the name to enable Delete).

**Current state (2026-08-12):**
- Desktop `Delete project` / `Delete secret` / `Offboard` fire immediately
  (`do_delete` at `desktop_view.rs:502` — no confirm). **Gap remains** (needs a
  GPUI confirm modal; no dialog primitive currently exists in the desktop
  shell).
- Extension `Delete project` fires immediately with only a toast. **Gap remains.**
- → Wire confirmation dialogs everywhere. Mobile secrets view is currently
  read-only (no delete surface yet) — add delete + confirm when the surface
  lands.

---

## 4. Lock & unlock

**Canon:** a single `useLock()` hook (web/ext/mobile) / `LockManager`
(desktop) that (a) wipes the DEK/SVK, (b) releases every active `SecretHandle`,
(c) clears UI state, (d) routes to the unlock screen.

**Current state:**
- Desktop: sidebar `Log out` → `do_lock` → zeroizes + clears.
- Extension: popup header `Lock` → `client.lock()` + `disposePopupClient()`.
- Mobile: biometric unlock (FaceID) per `app.json`, but **no visible lock
  button** in the dump.
- → Mobile: add a lock button + auto-lock on background. Desktop: add an
  auto-lock idle timer. Extension: auto-lock on `pagehide` (already does) + idle.

---

## 5. Session restore

**Canon:** launch restores an existing session token and routes straight to the
authenticated surface (no re-login) when the token is valid + unexpired.

**Current state:** mobile restores in `App.tsx`
(`services.auth.hasSession() → restore → setAuthenticated`). **Desktop does
not** — every launch requires fresh login, even though the token is already
persisted in `VaultConfig`. → Add session-token restore to desktop.

---

## 6. Form state & error display

**Canon:** every form exposes a typed `FormState = Idle | Submitting |
Error(message) | Success(message)`; render rules are *derived*, not
string-matched.

**Current state:** desktop parses `login_status` strings (`status.contains
("failed")` to pick a color); extension uses `setError`/`setStatus` in a zustand
store. → Unify on the typed `FormState` shape across clients.

---

## 7. Toast / sonner parity

**Canon:** one toast system per client with variants `success | error | info |
loading` and the same icons (extension's `sonner.tsx` already defines them).

**Current state:** extension has `<Toaster>` (sonner); desktop uses inline
status text; mobile has none. → Ship a single toast surface on desktop + mobile
matching the extension's variants.

---

## 8. Reveal appears instantly (motion rule)

Revealed secret values, the brand mark, and top-level navigation changes are
**never animated** (no fade on reveal — it appears instantly to convey trust).
See [design-system.md](./design-system.md) §3 and Phase 6 for the full motion
canon.

---

## 9. Exit criteria (Phase 0)
- Reveal / create / edit / delete / lock are *named* identically here; later
  phases implement them without re-arguing.
- The `btoa` gap is recorded as a blocking item, not lost in a polish pass.
