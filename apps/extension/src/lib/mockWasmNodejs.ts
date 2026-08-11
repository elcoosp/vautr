/**
 * Development mock of the stateless `vautr-wasm-nodejs` crypto module
 * (`--target nodejs`, build-env-deploy §3.3).
 *
 * Mirrors the surface the stateless autofill Service Worker expects. It performs
 * NO real cryptography; it only proves the decrypt-once stateless boundary. The
 * real module is produced by `wasm-pack build ./core/vautr-crypto --target nodejs`.
 * Swap the Vite alias to the real built glue to exercise real AEAD decryption.
 *
 * SECURITY: this module never exposes `read_secret`; only the crypto-only
 * `decrypt_secret_with_svk` entrypoint exists here.
 */

const DEMO_SECRET = 'vautr-demo-password-0x3f9a';

/** One-time async initialisation of the nodejs-target WASM instance. */
export async function init(): Promise<void> {
  // no-op in the mock; real glue loads + initialises the WASM binary.
}

/** Statelessly decrypt a single item's ciphertext from the raw SVK. */
export function decrypt_secret_with_svk(
  _svk: Uint8Array,
  _uuid: string,
  _encKeyGen: number,
  _payload: Uint8Array,
): string {
  return DEMO_SECRET;
}
