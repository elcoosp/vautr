# Client Feature Parity Matrix

Last reconciled: 2026-08-13. Scope: the four clients shipping from this repo —
**web**, **desktop** (GPUI), **extension**, **mobile** (React Native) — plus the
shared **Rust core** (`vautr-app-state`, `vautr-sync`, `vautr-ffi`, `vautr-db`,
`vautr-crypto`) that backs them.

The Rust core implements every capability below. The gaps below are purely in how
each *frontend* consumes the core. "Parity" in the zero-knowledge sense means: the
client never holds a decrypted secret in JS/UI memory longer than the reveal
interaction, and it offers the same user-facing capability set.

Legend: ✅ full · ⚠️ partial / wired-but-conditional · ✗ missing

| Capability | core | web | desktop | extension | mobile |
|---|---|---|---|---|---|
| Local vault (FFI / orchestrator) | ✅ | ✗ HTTP client | ✅ vautr-app-state | ✗ HTTP client | ⚠️ FFI path compiles; native module not linked at runtime |
| Secure secret reveal (opaque handle → native overlay) | ✅ | ✗ HTTP `revealSecret` | ✅ (GPUI, plaintext local) | ✗ HTTP reveal | ⚠️ `SecretOverlay`/`MobileVautrClient` exist; shipping screen still uses `services.api.revealSecret` plaintext (VTR-048 under-native) |
| Conflict modal (VTR-056) | ✅ `ConflictDetected` | ✅ | ✅ (added this batch) | ✗ | ✗ |
| Import / export (VTR-039) | ✅ | ✅ | ✗ | ✗ | ✅ |
| Machine accounts (VTR-047 sidebar) | ✅ | ✅ | ✗ | ✗ | ✅ |
| Tokens (VTR-047 sidebar) | ✅ | ✅ | ✗ | ✗ | ✅ |
| MFA / WebAuthn (VTR-049) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Generator | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sharing / key rotation | ✅ | ✅ (core) | ? | ? | ? |
| Quarantine reaper UI (VTR-047) | ✅ | ✅ | ✅ `subscribe_quarantine_events` | ✗ | ✅ |

## Security-invariant gaps (highest priority)

1. **Mobile reveals plaintext into JS** (VTR-048 parity gap). The native
   `MobileVautrClient` + `SecretOverlay` + Kotlin/Swift `NativeSecretView`
   source exists, but the shipping secrets screen (`_app.projects.$projectId.tsx`)
   calls `services.api.revealSecret(uuid)` which returns plaintext into React
   state. `getMobileClient()` returns `null` because no compiled uniffi/TurboModule
   is linked in the mobile app, so the secure overlay never runs. This batch wired
   the screen to prefer `MobileVautrClient` + `SecretOverlay` when the FFI client
   is present and added `bootVautrCore()` to initialize it on app mount, but the
   native module itself is still a build dependency (cannot be compiled in this
   sandbox: no Android SDK / Xcode). See open issue **VTR-057**.

2. **Web & extension reveal over HTTP** — same class of gap as mobile: plaintext
   crosses the JS boundary. Acceptable only because web/extension are explicitly
   server-backed (the threat model assumes the user trusts the browser tab's
   process); but for full ZK parity with desktop they would need a WASM-side
   decrypt + ephemeral DOM binding. Tracked as **VTR-058** (lower priority than
   mobile, since web is the reference client and the server is the trust root).

## Capability gaps (parity with web)

3. **Desktop lacks import/export, machine accounts, tokens, sharing UI**
   (VTR-059). Desktop drives the full local-orchestrator vault but its GPUI
   surface is thin: only vault list, secret view, generator, MFA, quarantine, and
   (now) conflict modal. The web sidebar sections (Dashboard, Secrets, Machine
   accounts, Tokens, Import/export) have no desktop equivalent yet.

4. **Extension lacks conflict modal, import/export, machine accounts, tokens**
   (VTR-060). The extension is the thinnest client: server-backed vault access +
   generator + MFA only.

5. **Mobile lacks conflict modal** (folded into VTR-057 — once the FFI client is
   linked, the `conflictQueue`/`resolveConflict` machinery from VTR-056 already
   exists in `packages/ui-logic` and can be mounted; only the native bridge blocks it).

## Stubs / placeholder audit

- **Rust**: zero `todo!()`, `unimplemented!()`, `unreachable!()` in `core/`.
- **TypeScript**: `apps/web/src/lib/mockWasm.ts` is a dev-only throwing shim used
  only by `vite.config.ts` test harness; production uses real `wasm.ts`. Not a stub
  in the shipping path.
- **Mobile native**: `modules/vautr-native/` (Kotlin/Swift/Maestro) is real,
  complete source but cannot be compiled here (no SDK). Documented, not stubbed.
- **No `// TODO` feature toggles left dangling** in shipping code paths.

## Verification

- `cargo test --workspace` green.
- `cargo build -p vautr-desktop -p vautr-app-state` green (this batch).
- `pnpm typecheck` (mobile) + `pnpm lint` green (this batch, A).
- `apps/mobile` vitest `secretLifecycle` green (secure-overlay path exercised).
- `hermes verify` tracks the 10/10 gate.

## Open issues

- VTR-057 — Link mobile uniffi/TurboModule native bridge; activate secure overlay.
- VTR-058 — (optional) WASM-side decrypt + ephemeral bind for web/extension.
- VTR-059 — Desktop: import/export, machine accounts, tokens, sharing UI.
- VTR-060 — Extension: conflict modal + import/export + machine accounts + tokens.
