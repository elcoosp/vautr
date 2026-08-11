/**
 * Node-target wrapper over the real `vautr-crypto-wasm` `--target nodejs` build,
 * used only by the vitest unit tests (Node runtime).
 *
 * The `--target nodejs` build auto-initialises synchronously at require time
 * (it reads the `.wasm` bytes via `fs`), so `init()` is a no-op. This differs
 * from the Service Worker's web-target wrapper (`wasmNodejs.ts`), which must
 * await the async loader because a browser SW cannot use `fs`.
 */
import * as raw from '../../sw-wasm-pkg-nodejs/vautr_crypto_wasm.js';

/** The nodejs build is already initialised at require time. */
export async function init(): Promise<void> {
  raw.init();
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
  return raw.decrypt_secret_with_svk(svk, uuid, BigInt(Math.trunc(encKeyGen)), payload);
}
