# Vautr native module (VTR-048)

Native secret-overlay component for the React Native mobile client. The plaintext
secret is rendered by a **native** view (Kotlin `TextView` / SwiftUI `UILabel`) and
never enters the JS heap — the JS layer only ever holds an opaque `u64` handle
(surfaced as a `String`). This implements ADR-003's opaque-handle pattern and
satisfies the desktop-only secret-reveal API is-not-in-mobile constraint enforced by
`scripts/check-restricted-api.sh`.

## How it fits together

1. JS calls `SecretHandleScope.view()` → `MobileVautrClient.renderInOverlay(handle)`
   (see `packages/vautr-client-sdk/src/mobile.ts`). JS passes **only the handle**.
2. The Turbo Module forwards the handle to the Rust core
   (`MobileClient.render_secret_in_overlay` in `core/vautr-ffi`), which delegates
   `perform_action(RenderInOverlay { handle })`.
3. The core decrypts the secret and calls the registered native
   `PlatformActionHandler.onAction(RenderInOverlay, secret)` — the plaintext goes
   **straight to native code**, not JS.
4. The native overlay renders it. On unmount (`onDetachedFromWindow` / `onDisappear`
   / `deinit`) it calls `releaseSecret(handle)`, zeroizing the in-memory secret in
   Rust.

## Source layout

- `android/.../secret/NativeSecretView.kt` — the overlay `TextView` + the
  `SecretOverlayActionHandler` (`PlatformActionHandler` impl).
- `android/.../secret/NativeSecretViewTest.kt` — TDD1 + TDD3 (JVM unit test).
- `ios/NativeSecretView.swift` — SwiftUI overlay + handler.
- `ios/NativeSecretViewTests.swift` — TDD2 + TDD3 (`swift test`).
- `e2e/secret_overlay.yaml` — TDD5 Maestro flow (reveal → visible → navigate → cleared).

## Toolchain note (verification status)

The Rust + TypeScript layers of VTR-048 are verified in CI (`cargo test -p vautr-ffi`,
`vitest` in `apps/mobile`). The Kotlin, Swift, and Maestro artifacts above require
the Android SDK / Xcode / Maestro toolchains and a uniffi-generated `VautrClient`
(`io.vautr.mobile.ffi.*`) to compile and run. They are written against the
`VautrNativeBridge` contract in `packages/vautr-client-sdk/src/mobile.ts` and are
intended to be built as part of the native module target — they are not exercised by
the sandbox's `cargo`/`vitest` gate.
