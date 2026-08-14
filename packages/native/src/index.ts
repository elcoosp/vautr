/**
 * @vautr/native — Expo TurboModule bridge for the Vautr uniffi core.
 *
 * This package links `vautr-ffi` (uniffi 0.31) into the React Native JSI
 * instance and exposes the `VautrNativeBridge` contract (defined in
 * `@vautr/client-sdk/mobile`) to the app. The native code lives in
 * `ios/VautrNativeModule.swift` + `android/.../VautrNativeModule.kt`; this TS
 * layer is a thin, typed accessor.
 *
 * Zero-knowledge invariant (ADR-003 / data.md §1): bytes cross the bridge as
 * `Uint8Array` (lowered to native `Data`/byte arrays), opaque handles as
 * `string` (u64), and plaintext secrets are produced only by the native
 * overlay — JS never holds a decrypted secret string.
 */

import type { SecureEnclaveBridge, VautrNativeBridge } from '@vautr/client-sdk/mobile';
import { requireNativeModule } from 'expo-modules-core';

/** The native Turbo Module (registered by the iOS/Android EXModule). */
interface NativeVautrModule {
  /** Link the Rust core into JSI and run migrations. */
  initialize(dbPath: string): Promise<void>;
  unlock(rawKey: Uint8Array, localGen: number): Promise<void>;
  listOverviews(): Promise<string>;
  revealSecret(uuid: string): Promise<string>;
  releaseSecret(handle: string): Promise<void>;
  renderSecretInOverlay(handle: string): Promise<void>;
  lock(): Promise<void>;
  sync(): Promise<void>;
  setSecureEnclaveBridge(bridge: SecureEnclaveBridge): Promise<void>;
  ensureSharingKey(): Promise<string>;
  setSharingSecret(secretB64: string | null): Promise<void>;
  shareItem(
    senderUuid: string,
    recipientUuid: string,
    itemUuid: string,
    recipientPubkeyB64: string,
    plaintext: Uint8Array,
  ): Promise<string>;
  acceptShare(incomingJson: string): Promise<Uint8Array>;
  createGroup(name: string, adminUuid: string): Promise<string>;
  addGroupMember(groupJson: string, memberUuid: string, memberPubkeyB64: string): Promise<string>;
  unwrapGroupKey(inboxJson: string): Promise<string>;
  encryptGroupItem(groupJson: string, itemUuid: string, plaintext: Uint8Array): Promise<string>;
  decryptGroupItem(groupJson: string, itemUuid: string, ctB64: string): Promise<Uint8Array>;
}

function getNativeModule(): NativeVautrModule | null {
  try {
    return requireNativeModule<NativeVautrModule>('VautrNativeModule');
  } catch {
    // Module not linked (HTTP-only build / simulator without native prebuild).
    return null;
  }
}

/**
 * Returns the native `VautrNativeBridge`, or `null` when the uniffi core is not
 * linked. The app's `bootVautrCore` treats `null` as an HTTP-only build and
 * stays on the server-backed client (VTR-061 fallback).
 */
export function createVautrNativeBridge(): VautrNativeBridge | null {
  const native = getNativeModule();
  if (!native) return null;
  return native as unknown as VautrNativeBridge;
}

/**
 * OS-keystore biometrics adapter over `expo-secure-store` + local
 * authentication. Backs `MobileClient.setSecureEnclaveBridge` so a biometric
 * unlock can recover the SVK without re-prompting for a master password
 * (build-env-deploy §2.5).
 */
export function createSecureEnclaveBridge(): SecureEnclaveBridge {
  // Lazily required so `@vautr/native` itself does not hard-depend on these
  // plugins at module-load on non-native (web) consumers.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const SecureStore = require('expo-secure-store') as typeof import('expo-secure-store');
  const LocalAuth =
    require('expo-local-authentication') as typeof import('expo-local-authentication');

  const KEY = 'vautr.svk';

  return {
    async saveSvk(svk: Uint8Array): Promise<void> {
      const b64 = arrayBufferToBase64(svk);
      // Gate the write behind a biometric prompt so the SVK only lands in the
      // keychain after an explicit user authentication.
      const ok = await LocalAuth.authenticateAsync({
        promptMessage: 'Store vault key with biometrics',
        disableDeviceFallback: false,
      });
      if (!ok.success) throw new Error('biometric auth declined');
      await SecureStore.setItemAsync(KEY, b64);
    },
    async loadSvk(): Promise<Uint8Array | null> {
      const ok = await LocalAuth.authenticateAsync({
        promptMessage: 'Unlock vault with biometrics',
        disableDeviceFallback: false,
      });
      if (!ok.success) return null;
      const b64 = await SecureStore.getItemAsync(KEY);
      if (!b64) return null;
      return base64ToArrayBuffer(b64);
    },
    async deleteSvk(): Promise<void> {
      await SecureStore.deleteItemAsync(KEY);
    },
    async hasSvk(): Promise<boolean> {
      const b64 = await SecureStore.getItemAsync(KEY);
      return b64 !== null && b64.length > 0;
    },
  };
}

function arrayBufferToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes.at(i) ?? 0);
  }
  if (typeof btoa === 'function') return btoa(binary);
  // React Native / Hermes fallback.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const { Buffer } = require('node:buffer');
  return Buffer.from(binary, 'binary').toString('base64');
}

function base64ToArrayBuffer(b64: string): Uint8Array {
  let binary: string;
  if (typeof atob === 'function') {
    binary = atob(b64);
  } else {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { Buffer } = require('node:buffer');
    binary = Buffer.from(b64, 'base64').toString('binary');
  }
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
