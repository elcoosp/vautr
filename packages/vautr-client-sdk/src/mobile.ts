/**
 * Vautr mobile bridge (React Native + Expo, build-env-deploy.md §3.1).
 *
 * Owned by the mobile client (deconflict: this module is additive and does not
 * touch the shared web/extension worker surface in `./client.ts` / `./types.ts`).
 *
 * The Rust core (vautr-ffi, uniffi 0.31) exposes `initialize`, `unlock`,
 * `list_overviews`, `reveal_secret`, `lock`, `sync` plus a `SecureEnclaveBridge`
 * for OS-keystore biometric storage of the SVK. On React Native this native
 * surface is reached through a TurboModule (the `VautrNativeBridge` contract
 * below) that the app supplies. u64 values (opaque handles) are surfaced to JS
 * as strings, matching the web/extension WASM worker bridge (data.md §1 rule 1).
 *
 * Plaintext secrets never cross into JS: `reveal` returns an opaque handle and
 * the secret is only delegated back to the OS clipboard/autofill via the native
 * platform handler.
 */

import type { DecryptedOverview, OpaqueHandle } from './types';

/** Opaque u64 surfaced as a string. Re-exported for mobile consumers. */
/** Re-exported list-item type for mobile consumers. */
export type { DecryptedOverview, OpaqueHandle } from './types';

/**
 * OS-keystore biometrics storage of the 32-byte SVK. The app implements this
 * over `expo-secure-store` (Keychain / Android Keystore) + `expo-local-authentication`
 * and registers it so a biometric unlock never re-prompts for a master password.
 */
export interface SecureEnclaveBridge {
  /** Persist the 32-byte SVK under biometric (or device-passcode) protection. */
  saveSvk(svk: Uint8Array): Promise<void>;
  /** Load the stored SVK. Returns `null` if absent or biometric auth cancelled. */
  loadSvk(): Promise<Uint8Array | null>;
  /** Delete the stored SVK (e.g. on explicit lock or vault removal). */
  deleteSvk(): Promise<void>;
  /** Whether an SVK is currently stored and available for a biometric unlock. */
  hasSvk(): Promise<boolean>;
}

/**
 * The native (TurboModule) contract the app implements to link the uniffi core
 * into the JSI instance. Every method is async so JSI calls can be run under
 * `useTransition` and never block the UI thread; errors are `Result` surfaced
 * as `.message` via try/catch.
 */
export interface VautrNativeBridge {
  /** Link the Rust core into the JSI instance and run migrations. */
  initialize(dbPath: string): Promise<void>;
  /** Unlock with a raw 32-byte SVK + local key generation. */
  unlock(rawKey: Uint8Array, localGen: number): Promise<void>;
  /** List overviews, most-recently-used first. */
  listOverviews(): Promise<DecryptedOverview[]>;
  /** Reveal a secret behind an opaque handle (u64 as string). */
  revealSecret(uuid: string): Promise<OpaqueHandle>;
  /** Explicitly dispose a handle (zeroizes the in-memory secret). */
  releaseSecret(handle: OpaqueHandle): Promise<void>;
  /**
   * Render a revealed secret in the native overlay view (VTR-048, ADR-003).
   * The native TurboModule delivers the plaintext only to the native overlay
   * component (Kotlin/Swift) via the registered `PlatformActionHandler` — the
   * JS side never receives the secret string, only the opaque handle.
   */
  renderInOverlay(handle: OpaqueHandle): Promise<void>;
  /** Lock the vault (zeroizes keys + in-memory secrets). */
  lock(): Promise<void>;
  /** Run a metadata-first sync. */
  sync(): Promise<void>;
  /** Register the OS-keystore SVK adapter (biometric unlock). */
  setSecureEnclaveBridge(bridge: SecureEnclaveBridge): Promise<void>;
}

/** Options to {@link initializeVautrCore}. */
export interface InitializeCoreOptions {
  /** The native TurboModule bridge linked into the JSI instance. */
  native: VautrNativeBridge;
  /** OS-keystore SVK adapter (biometric unlock). */
  secureEnclave: SecureEnclaveBridge;
  /** SQLite vault DB path on device. */
  dbPath: string;
}

/**
 * Promise-based mobile client over the native bridge. Mirrors the web
 * `VautrClient` surface for the Lock/List/Detail screens.
 */
export class MobileVautrClient {
  private readonly native: VautrNativeBridge;

  constructor(native: VautrNativeBridge) {
    this.native = native;
  }

  /** Unlock the vault with a raw 32-byte SVK (biometric path). */
  unlock(rawKey: Uint8Array, localGen: number): Promise<void> {
    return this.native.unlock(rawKey, localGen);
  }

  /** List all overviews, most-recently-used first. */
  listOverviews(): Promise<DecryptedOverview[]> {
    return this.native.listOverviews();
  }

  /** Reveal a secret behind an opaque handle (u64 as string). */
  reveal(uuid: string): Promise<OpaqueHandle> {
    return this.native.revealSecret(uuid);
  }

  /** Explicitly dispose a handle (zeroizes the in-memory secret). */
  release(handle: OpaqueHandle): Promise<void> {
    return this.native.releaseSecret(handle);
  }

  /**
   * Render the revealed secret in the native overlay view (VTR-048, ADR-003).
   * The plaintext is delivered only to the native overlay component via the
   * registered `PlatformActionHandler`; JS keeps only the opaque handle.
   */
  renderInOverlay(handle: OpaqueHandle): Promise<void> {
    return this.native.renderInOverlay(handle);
  }

  /** Lock the vault. */
  lock(): Promise<void> {
    return this.native.lock();
  }

  /** Run a metadata-first sync. */
  sync(): Promise<void> {
    return this.native.sync();
  }
}

let activeClient: MobileVautrClient | null = null;

/**
 * Boot the mobile core (skill matrix boot pattern): links the Rust structures
 * into the JSI instance via `native.initialize`, registers the `SecureEnclaveBridge`,
 * and returns a ready `MobileVautrClient`. Await this on mount; keep it wrapped in
 * a `useTransition` so the JSI allocation never blocks the UI thread.
 */
export async function initializeVautrCore(
  options: InitializeCoreOptions,
): Promise<MobileVautrClient> {
  const { native, secureEnclave, dbPath } = options;
  await native.initialize(dbPath);
  await native.setSecureEnclaveBridge(secureEnclave);
  activeClient = new MobileVautrClient(native);
  return activeClient;
}

/** The active mobile client, or `null` before {@link initializeVautrCore}. */
export function getMobileClient(): MobileVautrClient | null {
  return activeClient;
}
