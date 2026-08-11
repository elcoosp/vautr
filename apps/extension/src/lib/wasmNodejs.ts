/**
 * Thin wrapper over the real `vautr-crypto-wasm` `--target web` build for
 * the stateless Service Worker (build-env-deploy §3.3).
 *
 * The wasm-pack `--target web` output exposes `init()` as a named export
 * that must be called once before any other function.
 */

import initWasm, * as raw from '../../sw-wasm-pkg/vautr_crypto_wasm.js';

/**
 * One-time async initialization of the WASM instance.
 * The wasm-bindgen `--target web` glue exposes the async loader as its default
 * export (`__wbg_init`); the named `init()` is a no-op that assumes the instance
 * is already loaded. We must await the default loader so the WASM is ready
 * before `decrypt_secret_with_svk` is callable.
 */
export async function init(): Promise<void> {
  await initWasm();
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
