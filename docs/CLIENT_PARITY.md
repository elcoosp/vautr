# Client Feature Parity Matrix

Last reconciled: 2026-08-16 (post VTR-099/101/102/103 UI recompose + VTR-104
mobile uniffi core link + VTR-105 web key-rotation). Scope: the four clients
shipping from this repo — **web**, **desktop** (GPUI), **extension**,
**mobile** (React Native) — plus the shared **Rust core**
(`vautr-app-state`, `vautr-sync`, `vautr-ffi`, `vautr-db`, `vautr-crypto`) that
backs them.

The Rust core implements every capability below. The gaps below are purely in how
each *frontend* consumes the core. "Parity" in the zero-knowledge sense means: the
client never holds a decrypted secret in JS/UI memory longer than the reveal
interaction, and it offers the same user-facing capability set.

Legend: ✅ full · ⚠️ partial / wired-but-conditional · ✗ missing

| Capability | core | web | desktop | extension | mobile |
|---|---|---|---|---|---|
| Local vault (FFI / orchestrator) | ✅ | ✗ HTTP client | ✅ vautr-app-state | ✗ HTTP client | ✅ FFI linked (VTR-104); `getMobileClient()` non-null on device/sim |
| Secure secret reveal (opaque handle → native overlay) | ✅ | ✅ handle-based (VTR-062) | ✅ (GPUI, plaintext local) | ✅ transient wasm copy (VTR-062 follow-up) | ✅ `SecretOverlay` via `getMobileClient` (native-linked VTR-104) |
| Conflict modal (VTR-056) | ✅ `ConflictDetected` | ✅ | ✅ (VTR-063) | ✅ (VTR-064) | ✅ `conflictQueue` (native-linked) |
| Import / export (VTR-039) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| Machine accounts (VTR-047) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| Tokens (VTR-047) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| MFA / WebAuthn (VTR-049) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Generator | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sharing / key rotation | ✅ crypto + server `/shares`, `/account/rotate-key` | ✅ sharing UI (InboxView/GroupsView/ItemDetail) + ⚠️ key-rotation UI added (VTR-105, settings) | ✅ local orchestrator + key rotation (VTR-065) | ✅ ext sharing done (VTR-066) + key rotation (VTR-065) | ✅ native sharing client (`MobileSharingClient`); UI gated on core, activates with VTR-104 |
| Quarantine reaper UI (VTR-047) | ✅ | ✗ no UI | ✅ `watch_state` subscription | ✗ no server endpoint | ✅ local orchestrator |

## Security-invariant gaps (highest priority)

1. **Mobile native module (post-auth surface) is now linked (VTR-104).** The
   `vautr-ffi` uniffi core builds for `aarch64-apple-ios-sim` and links via the
   `VautrNativeModule` pod, so `getMobileClient()` is non-null: the secure
   `SecretOverlay` reveal path, native sharing (`MobileSharingClient`), and
   `sync`/`watch_state` are live on device/simulator. **Caveat (open):** account
   *creation / first unlock* still cannot run on RN/Hermes — the OPAQUE PAKE +
   KDF + SVK/KEK/recovery crypto lives in the `vautr-wasm` crate, which targets a
   JS engine with WebAssembly; Hermes has no WASM, and `vautr-ffi`'s
   `MobileClient` expects those keys to be supplied by the client (it has no
   register/login of its own). Until OPAQUE is ported into `vautr-ffi` (a
   follow-up VTR), mobile register/login fails fast with a clear "use web or
   desktop" message rather than a silent empty error. (The `VautrNativeBridge`
   surface and Swift bindings are in place and ready to accept `register`/
   `login` once the Rust side exposes them.)

2. **Web/extension reveal is server-backed, not wasm-decrypt-in-tab.** Both now
   avoid plaintext-in-JS-state (web = opaque wasm handle + `performAction`;
   extension = transient wasm decrypt → clipboard). Plaintext still crosses the
   network boundary to the (trusted) server; this matches web/extension's
   documented threat model (the browser/extension process is the trust root).

## Capability gaps (what remains)

3. **Sharing UI on web/mobile (⚠️).** The full zero-knowledge sharing flow is
   built and verified end-to-end for the **extension** (VTR-066): server PKI
   (`POST /shares/`, `/shares/{id}/payload`, `/shares/inbox`, revoke),
   `PUT /users/{id}/public-key`, `vautr-wasm` full share/accept FFI, `VautrMlpClient`
   HTTP transport, `VautrWebClient` `SharingManager`, and the extension Share
   button + Inbox tab. ADR-007 sharing generates a fresh random SIK per share and
   DEM-encrypts the item *plaintext* — no per-item-SIK data model required (the
   earlier "per-item SIK blocker" was incorrect; the desktop's `share_item` takes
   plaintext, not a stored key). **Remaining:** the same SDK surface must be wired
   into the **web** and **mobile** UIs; **group sharing** UI (ADR-007 §6) is
   implemented in `vautr-sharing` but has no client UI yet.

4. **Key-rotation UI on web/extension/mobile.** The server has
   `POST /account/rotate-key` (`new_min_enc_key_gen` + MP-wrapped `svk`); desktop
   drives it via its local orchestrator's `rotate_key`, and the extension now
   surfaces it (VTR-065) by re-entering the master password to re-wrap the SVK
   client-side. Web/mobile still lack a rotation UI (no client-crypto surface
   exposed there yet).

5. **Quarantine reaper UI on web/extension (✗).** Quarantine/reaper is a
   local-orchestrator concept surfaced via `watch_state()` (desktop ✅, mobile ✅).
   The server has no quarantine endpoint, so web/extension cannot show it without
   a server-side reaper + event stream. New VTR if desired.

## Stubs / placeholder audit

- **Rust**: zero `todo!()`, `unimplemented!()`, `unreachable!()` in `core/`.
- **TypeScript**: `apps/web/src/lib/mockWasm.ts` is a dev-only throwing shim used
  only by `vite.config.ts` test harness; production uses real `wasm.ts`.
- **Mobile native**: `modules/vautr-native/` (Kotlin/Swift/Maestro) is real,
  complete source but cannot be compiled here (no SDK). Documented, not stubbed.
- **No `// TODO` feature toggles left dangling** in shipping code paths.

## Open issues

- **Web/mobile sharing UI** — the SDK surface (`VautrMlpClient` + `VautrWebClient`
  `SharingManager`) is done and verified (VTR-066, extension). Web and mobile need
  their Share/Inbox UI wired to that surface.
- **Group sharing UI** (ADR-007 §6) — implemented in `vautr-sharing`; no client UI.
- **Quarantine reaper** (web/ext) — server-side reaper + event stream still to
  build (see gap #5).

All previously-tracked parity VTRs (VTR-039/047/049/056/059/060/061/062/063/064,
the ext ZK follow-up, and VTR-065/066) are closed. Remaining items are
backend/client-crypto work, not parity ports, and are intentionally left for
dedicated VTRs rather than stubbed into the UI.

## Verification

- `cargo test --workspace` green.
- `cargo build -p vautr-desktop -p vautr-app-state` green.
- `pnpm typecheck` (web / extension / mobile) + `pnpm lint` green.
- `apps/mobile` vitest `secretLifecycle` green (secure-overlay path exercised).
- `hermes verify` tracks the 10/10 gate.
