import { VautrMlpClient } from '@vautr/client-sdk';
import {
  getMobileClient,
  initializeVautrCore,
  type SecureEnclaveBridge,
  type VautrNativeBridge,
} from '@vautr/client-sdk/mobile';
import { createSecureEnclaveBridge, createVautrNativeBridge } from '@vautr/native';
import { MobileApiClient } from './api';
import { secureTokenStore, VautrAuth } from './auth';

/** App-wide shared API client + auth (lazy singleton). */
class AppServices {
  private _api: MobileApiClient | null = null;
  private _auth: VautrAuth | null = null;
  private _mlp: VautrMlpClient | null = null;

  get api(): MobileApiClient {
    if (!this._api) {
      this._api = new MobileApiClient();
    }
    return this._api;
  }

  get auth(): VautrAuth {
    if (!this._auth) {
      this._auth = new VautrAuth({ api: this.api, store: secureTokenStore });
    }
    return this._auth;
  }

  /** MLP (org-model) client — shares the same authenticated HTTP session as `api`. */
  get mlp(): VautrMlpClient {
    if (!this._mlp) {
      this._mlp = new VautrMlpClient(
        this.api.http as unknown as ConstructorParameters<typeof VautrMlpClient>[0],
      );
    }
    return this._mlp;
  }

  /** Recreate the auth singleton (used after logout). */
  reset(): void {
    this._auth = null;
    this._api = null;
    this._mlp = null;
  }
}

export const services = new AppServices();

let coreBooted = false;

/**
 * Locate the compiled uniffi TurboModule bridge. Prefers the Expo-native
 * `@vautr/native` module (VTR-061) which links the Rust core into JSI; falls
 * back to a manually-registered `globalThis.__NATIVE_VAUTR__` (legacy/dev).
 * Returns `null` when no native core is linked, so the app stays on the HTTP
 * client.
 */
function resolveNativeBridge(): VautrNativeBridge | null {
  return createVautrNativeBridge() ?? null;
}

/**
 * Boot the local vault core (VTR-048/056/061). When the uniffi native module
 * is linked, this initializes the FFI `MobileVautrClient` so `getMobileClient()`
 * is non-null and the secrets screens use the opaque-handle native-overlay
 * reveal path (plaintext never enters JS). When it is not linked, this is a
 * safe no-op — the HTTP client remains the source of truth. Idempotent.
 */
export async function bootVautrCore(dbPath: string): Promise<void> {
  if (coreBooted) {
    return;
  }
  coreBooted = true;
  const native = resolveNativeBridge();
  if (!native) {
    return; // HTTP-only build: no local vault core.
  }
  try {
    const secureEnclave: SecureEnclaveBridge = createSecureEnclaveBridge();
    await initializeVautrCore({ native, secureEnclave, dbPath });
    // biome-ignore lint/suspicious/noConsole: intentional boot diagnostics
    console.log(`[VAUTR-CORE] booted OK; isLocalVaultActive=${isLocalVaultActive()}`);
  } catch (e) {
    // biome-ignore lint/suspicious/noConsole: intentional boot diagnostics
    console.error(
      `[VAUTR-CORE] boot FAILED: ${e && (e as Error).stack ? (e as Error).stack : String(e)}`,
    );
  }
}

/** Whether the secure local-vault FFI client is active (vs HTTP-only). */
export function isLocalVaultActive(): boolean {
  return getMobileClient() !== null;
}

/**
 * VTRFIX-FEAT-H02: drive the Emergency Recovery Kit flow end-to-end on mobile.
 *
 * Uses the Rust core (via the native bridge) for the mnemonic-derived Ed25519
 * signing and the SVK unwrap, and the HTTP API for the challenge/verify/info
 * round-trips. The final re-wrap step is a documented follow-up (see
 * docs/issues/VTRFIX-LOG.md) because the FFI's complete_recovery binding does
 * not yet expose the atomic re-wrap on mobile.
 */
export async function recoverWithKit(
  username: string,
  mnemonic: string,
  _newPassword: string,
): Promise<{ newMnemonic: string }> {
  const native = getMobileClient();
  if (!native) {
    throw new Error('native core not linked; cannot recover on this device');
  }
  const api = services.api.http as unknown as {
    request<T>(method: string, path: string, body?: unknown): Promise<T>;
  };

  const challenge = await api.request<{ nonce: string }>(
    'POST',
    '/account/recover/challenge',
    { email: username },
  );
  const signature = await native.signRecoveryNonce(mnemonic, challenge.nonce);
  const verify = await api.request<{ recovery_token: string }>(
    'POST',
    '/account/recover/verify',
    { email: username, signature },
  );
  const info = await api.request<{ svk_ciphertext_blob_rk: string; user_id: string }>(
    'POST',
    '/account/recover/info',
    { recovery_token: verify.recovery_token },
  );
  const svk = await native.recoverSvk(mnemonic, info.svk_ciphertext_blob_rk, info.user_id);
  void svk;
  throw new Error(
    'recoverWithKit: signature + unwrap succeeded; final re-wrap needs the FFI complete binding',
  );
}
