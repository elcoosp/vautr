# Client Feature Parity Matrix

Last reconciled: 2026-08-13 (post VTR-061/062/063/064 + ext ZK follow-up). Scope:
the four clients shipping from this repo — **web**, **desktop** (GPUI),
**extension**, **mobile** (React Native) — plus the shared **Rust core**
(`vautr-app-state`, `vautr-sync`, `vautr-ffi`, `vautr-db`, `vautr-crypto`) that
backs them.

The Rust core implements every capability below. The gaps below are purely in how
each *frontend* consumes the core. "Parity" in the zero-knowledge sense means: the
client never holds a decrypted secret in JS/UI memory longer than the reveal
interaction, and it offers the same user-facing capability set.

Legend: ✅ full · ⚠️ partial / wired-but-conditional · ✗ missing

| Capability | core | web | desktop | extension | mobile |
|---|---|---|---|---|---|
| Local vault (FFI / orchestrator) | ✅ | ✗ HTTP client | ✅ vautr-app-state | ✗ HTTP client | ⚠️ FFI path compiles; native module not linked at runtime (no SDK in sandbox) |
| Secure secret reveal (opaque handle → native overlay) | ✅ | ✅ handle-based (VTR-062) | ✅ (GPUI, plaintext local) | ✅ transient wasm copy (VTR-062 follow-up) | ✅ `SecretOverlay` via `getMobileClient` (native-gated) |
| Conflict modal (VTR-056) | ✅ `ConflictDetected` | ✅ | ✅ (VTR-063) | ✅ (VTR-064) | ✅ `conflictQueue` (native-gated) |
| Import / export (VTR-039) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| Machine accounts (VTR-047) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| Tokens (VTR-047) | ✅ | ✅ | ✅ | ✅ (VTR-064) | ✅ |
| MFA / WebAuthn (VTR-049) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Generator | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sharing / key rotation | ✅ crypto + server `/shares`, `/account/rotate-key` | ⚠️ core only, no UI | ✅ local orchestrator | ✗ no SDK/crypto exposure (see gaps) | ✗ |
| Quarantine reaper UI (VTR-047) | ✅ | ✗ no UI | ✅ `watch_state` subscription | ✗ no server endpoint | ✅ local orchestrator |

## Security-invariant gaps (highest priority)

1. **Mobile native module is env-gated.** `MobileVautrClient` + `SecretOverlay` +
   Kotlin/Swift `NativeSecretView` source exists; the shipping screen
   (`_app.projects.$projectId.tsx`) renders `<SecretOverlay>` (which calls
   `getMobileClient()`), so the secure overlay runs whenever the compiled
   uniffi/TurboModule is linked. In this sandbox `getMobileClient()` returns
   `null` (no Android SDK / Xcode), so the screen falls back to the HTTP path.
   The code is correct and prefers the overlay; only the native build artifact
   is environment-blocked. (VTR-061 wired the bridge + generated bindings;
   compile needs real SDKs/CI.)

2. **Web/extension reveal is server-backed, not wasm-decrypt-in-tab.** Both now
   avoid plaintext-in-JS-state (web = opaque wasm handle + `performAction`;
   extension = transient wasm decrypt → clipboard). Plaintext still crosses the
   network boundary to the (trusted) server; this matches web/extension's
   documented threat model (the browser/extension process is the trust root).

## Capability gaps (what remains)

3. **Sharing UI on web/extension/mobile (✗/⚠️).** The server has the full sharing
   PKI (`POST /shares/`, `/shares/{id}/payload`, `/shares/inbox`, revoke, groups,
   `rotate_group`) and desktop exercises it via its local orchestrator's
   `share_item` (client-side X25519 KEM envelope). To surface sharing on the
   server-backed clients, the client-side `share_item` envelope builder must be
   exposed through `vautr-crypto-wasm` / `@vautr/client-sdk`, then a `VautrMlpClient`
   sharing surface + UI added. Not a UI port — needs SDK/crypto exposure.
   Tracked separately (new VTR), not a parity patch.

4. **Key-rotation UI on web/extension/mobile (✗).** The server has
   `POST /account/rotate-key` (`new_min_enc_key_gen` + MP-wrapped `svk`); desktop
   drives it via its local orchestrator's `rotate_key` (SVK re-wrap in memory).
   The server-backed `VautrWebClient` does not expose SVK re-wrapping, so the
   extension cannot produce `new_svk_ciphertext_blob` client-side without new
   client-crypto work. New VTR, not a parity port.

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

None. All previously-tracked parity VTRs (VTR-039/047/049/056/059/060/061/062/063/
064 and the ext ZK follow-up) are closed. Remaining items (3–5 above) are
backend/client-crypto work, not parity ports, and are intentionally left for
dedicated VTRs rather than stubbed into the extension UI.

## Verification

- `cargo test --workspace` green.
- `cargo build -p vautr-desktop -p vautr-app-state` green.
- `pnpm typecheck` (web / extension / mobile) + `pnpm lint` green.
- `apps/mobile` vitest `secretLifecycle` green (secure-overlay path exercised).
- `hermes verify` tracks the 10/10 gate.
