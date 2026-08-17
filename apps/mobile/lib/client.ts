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

  /** Recreate the auth singleton (used after logout). */
  reset(): void {
    this._auth = null;
    this._api = null;
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
    // eslint-disable-next-line no-console
    console.log('[VAUTR-CORE] booted OK; isLocalVaultActive=' + isLocalVaultActive());
  } catch (e) {
    // eslint-disable-next-line no-console
    console.error('[VAUTR-CORE] boot FAILED: ' + (e && (e as Error).stack ? (e as Error).stack : String(e)));
  }
}

/** Whether the secure local-vault FFI client is active (vs HTTP-only). */
export function isLocalVaultActive(): boolean {
  return getMobileClient() !== null;
}
