/**
 * Thin wrapper over the real `vautr-crypto-wasm` `--target web` build for
 * the stateless Service Worker (build-env-deploy §3.3).
 *
 * The wasm-pack `--target web` output exposes `init()` as a named export
 * that must be called once before any other function.
 */

import * as raw from '../../sw-wasm-pkg/vautr_crypto_wasm.js';

/**
 * One-time async initialization of the WASM instance.
 * The web-target glue uses `init()` to load + instantiate the WASM bytes.
 */
export async function init(): Promise<void> {
  await raw.init();
}

/**
 * Statelessly decrypt a single item's ciphertext from the raw SVK.
 * `encKeyGen` is a u64 epoch within JS safe-integer range.
 */
export function decrypt_secret_with_svk(
  svk: Uint8Array,
  uuid: string,
  encKeyGen: number,
  payload: Uint8Array,
): string {
  return (raw as any).decrypt_secret_with_svk(svk, uuid, BigInt(Math.trunc(encKeyGen)), payload);
}
