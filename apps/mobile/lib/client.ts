import {
  getMobileClient,
  initializeVautrCore,
  type SecureEnclaveBridge,
  type VautrNativeBridge,
} from '@vautr/client-sdk/mobile';
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
 * Locate the compiled uniffi TurboModule bridge, if it is linked into this
 * build. The native module (`modules/vautr-native`) registers a `NativeVautr`
 * TurboModule; when absent (HTTP-only / dev build without the native glue),
 * this returns `null` and the app stays on the HTTP client.
 */
function resolveNativeBridge(): VautrNativeBridge | null {
  const g = globalThis as unknown as { __NATIVE_VAUTR__?: VautrNativeBridge };
  return g.__NATIVE_VAUTR__ ?? null;
}

/**
 * Boot the local vault core (VTR-048/056). When the uniffi native module is
 * linked, this initializes the FFI `MobileVautrClient` so `getMobileClient()`
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
    const secureEnclave: SecureEnclaveBridge = {
      // Until the native side registers an expo-secure-store-backed adapter, the
      // core still boots; biometric SVK persistence is a no-op (HTTP reveal
      // path remains available). The native module overrides these at link time.
      saveSvk: async () => {},
      loadSvk: async () => null,
      deleteSvk: async () => {},
      hasSvk: async () => false,
    };
    await initializeVautrCore({ native, secureEnclave, dbPath });
  } catch {
    // Native core failed to initialize — stay on the HTTP client.
  }
}

/** Whether the secure local-vault FFI client is active (vs HTTP-only). */
export function isLocalVaultActive(): boolean {
  return getMobileClient() !== null;
}
