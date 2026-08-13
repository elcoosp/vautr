import type { SecureEnclaveBridge, VautrNativeBridge } from '@vautr/client-sdk/mobile';
import { requireNativeModule } from 'expo-modules-core';

/**
 * The compiled Expo TurboModule that links the uniffi `vautr-ffi` core into
 * the JSI instance. Its method surface mirrors `VautrNativeBridge` exactly
 * (see packages/vautr-client-sdk/src/mobile.ts). When the native module is not
 * linked (dev build without the Rust toolchain / prebuild), `requireNativeModule`
 * resolves to an empty object, so callers must treat a missing method as
 * "local vault unavailable" and fall back to the HTTP client.
 */
export interface VautrNativeModule extends VautrNativeBridge {
  /** Expo module self-identification; lets JS confirm the bridge is present. */
  readonly __vautrNative: true;
}

const NativeVautr = requireNativeModule<VautrNativeModule>('VautrNative');

/**
 * Build the `VautrNativeBridge` consumed by `initializeVautrCore`. Returns
 * `null` when the native module is absent so `bootVautrCore` can no-op safely
 * (HTTP-only fallback).
 */
export function createVautrNativeBridge(): VautrNativeBridge | null {
  if (!NativeVautr || typeof NativeVautr.initialize !== 'function') {
    return null;
  }
  return NativeVautr as unknown as VautrNativeBridge;
}

/**
 * Default OS-keystore (biometric) SVK adapter backed by `expo-secure-store`,
 * matching `SecureEnclaveBridge`. The native module registers this with the
 * Rust core via `setSecureEnclaveBridge` during `bootVautrCore`.
 */
export function createSecureEnclaveBridge(): SecureEnclaveBridge {
  // Lazily required so the module still type-checks in environments where the
  // Expo packages are not installed (e.g. web/extension builds).
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const SecureStore = require('expo-secure-store') as {
    setItemAsync: (k: string, v: string) => Promise<void>;
    getItemAsync: (k: string) => Promise<string | null>;
    deleteItemAsync: (k: string) => Promise<void>;
  };
  const SVK_KEY = 'vautr.svk';

  // React Native has no Node `Buffer`; use base64 helpers available on Hermes.
  const toB64 = (bytes: Uint8Array): string => {
    let bin = '';
    for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i] ?? 0);
    return btoa(bin);
  };
  const fromB64 = (b64: string): Uint8Array => {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  };

  return {
    async saveSvk(svk: Uint8Array): Promise<void> {
      await SecureStore.setItemAsync(SVK_KEY, toB64(svk));
    },
    async loadSvk(): Promise<Uint8Array | null> {
      const b64 = await SecureStore.getItemAsync(SVK_KEY);
      return b64 ? fromB64(b64) : null;
    },
    async deleteSvk(): Promise<void> {
      await SecureStore.deleteItemAsync(SVK_KEY);
    },
    async hasSvk(): Promise<boolean> {
      return (await SecureStore.getItemAsync(SVK_KEY)) !== null;
    },
  };
}

export default NativeVautr;
