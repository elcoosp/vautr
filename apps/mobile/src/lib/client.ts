import { initializeVautrCore, type MobileVautrClient } from '@vautr/client-sdk/mobile';

import { getNativeBridge, secureEnclaveBridge } from '../native';

const DB_PATH = 'vautr.sqlite3';

let clientPromise: Promise<MobileVautrClient> | null = null;

/**
 * Lazy singleton boot (skill matrix boot pattern): on first call, links the Rust
 * core into the JSI instance via `initializeVautrCore`, registers the OS-keystore
 * `SecureEnclaveBridge`, and returns a ready client. Callers await the promise;
 * the UI wraps it in a `useTransition` so the allocation never blocks rendering.
 */
export function getClient(): Promise<MobileVautrClient> {
  if (!clientPromise) {
    clientPromise = initializeVautrCore({
      native: getNativeBridge(),
      secureEnclave: secureEnclaveBridge,
      dbPath: DB_PATH,
    }).catch((err: unknown) => {
      // Allow a retry (BootScreen) on transient boot failures.
      clientPromise = null;
      throw err;
    });
  }
  return clientPromise;
}
