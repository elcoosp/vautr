/**
 * Native glue for the mobile app.
 *
 * - `secureEnclaveBridge`: real OS-keystore SVK adapter over `expo-secure-store`
 *   (Keychain / Android Keystore) with biometric gating via `expo-local-authentication`.
 * - `getNativeBridge()`: returns the real uniffi TurboModule (`NativeModules.VautrCore`)
 *   when the dev client / native build provides it, falling back to the in-memory
 *   demo bridge so `expo start` works in Expo Go (New Architecture Bridgeless).
 */

import * as LocalAuthentication from 'expo-local-authentication';
import * as SecureStore from 'expo-secure-store';
import { NativeModules } from 'react-native';

import type { SecureEnclaveBridge, VautrNativeBridge } from '@vautr/client-sdk/mobile';

import { createDemoNativeBridge, makeDemoKey } from './lib/demoCore';

export { makeDemoKey };

const SVK_KEY = 'vautr.svk';
const SVK_STORAGE_OPTS: SecureStore.SecureStoreOptions = {
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  requireAuthentication: true,
  authenticationPrompt: 'Vautr needs your device credentials to unlock your vault.',
};

/**
 * Real OS-keystore SVK adapter. `saveSvk` writes the 32-byte SVK under
 * biometric protection; `loadSvk` prompts for Face ID / Touch ID before reading
 * it back (the SVK is the "something you are" factor that avoids re-entering the
 * master password on every unlock).
 */
export const secureEnclaveBridge: SecureEnclaveBridge = {
  async saveSvk(svk) {
    await SecureStore.setItemAsync(SVK_KEY, bytesToBase64(svk), SVK_STORAGE_OPTS);
  },
  async loadSvk() {
    const hasHardware = await LocalAuthentication.hasHardwareAsync();
    if (hasHardware) {
      const result = await LocalAuthentication.authenticateAsync({
        promptMessage: 'Unlock Vautr',
        cancelLabel: 'Cancel',
        disableDeviceFallback: false,
      });
      if (!result.success) {
        return null;
      }
    }
    const stored = await SecureStore.getItemAsync(SVK_KEY);
    return stored ? base64ToBytes(stored) : null;
  },
  async deleteSvk() {
    await SecureStore.deleteItemAsync(SVK_KEY);
  },
  async hasSvk() {
    return (await SecureStore.getItemAsync(SVK_KEY)) != null;
  },
};

/**
 * Read the real uniffi TurboModule. The module is added by the native build
 * (build-env-deploy §3.1 / the Skill Matrix boot pattern); it links the Rust
 * structures into the JSI instance and exposes the `VautrNativeBridge` surface.
 */
function getRealNativeBridge(): VautrNativeBridge | null {
  const mod = NativeModules.VautrCore as Partial<VautrNativeBridge> | null | undefined;
  if (!mod || typeof mod.initialize !== 'function' || typeof mod.unlock !== 'function') {
    return null;
  }
  return mod as VautrNativeBridge;
}

let cachedBridge: VautrNativeBridge | null = null;

/** The active native bridge: real TurboModule when available, else the demo. */
export function getNativeBridge(): VautrNativeBridge {
  if (!cachedBridge) {
    cachedBridge = getRealNativeBridge() ?? createDemoNativeBridge();
  }
  return cachedBridge;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  if (typeof btoa === 'function') {
    return btoa(binary);
  }
  return globalThis.btoa?.(binary) ?? binary;
}

function base64ToBytes(b64: string): Uint8Array {
  const binary = typeof atob === 'function' ? atob(b64) : (globalThis.atob?.(b64) ?? '');
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
