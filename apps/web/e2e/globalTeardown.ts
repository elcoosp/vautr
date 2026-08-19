/**
 * Teardown for the WebAuthn (FIDO2) e2e (VTR-052).
 *
 * The Vautr server is managed by Playwright as a `webServer` entry, so its
 * lifecycle (start / keep-alive / kill) is handled automatically. This is kept
 * as a no-op so the config can still reference a globalTeardown without error.
 */
export default function globalTeardown(): void {
  // Intentionally empty: the backend server is torn down by Playwright's
  // webServer management. No leftover process should remain (the webServer
  // command used `exec`, so the server IS the managed child).
}
