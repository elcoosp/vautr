/**
 * Ambient type declaration for the stateless `vautr-wasm-nodejs` WebAssembly
 * glue module (build-env-deploy §3.3).
 *
 * This module is produced by `wasm-pack build ./core/vautr-crypto --target nodejs`
 * and is used ONLY by the Manifest V3 autofill Service Worker. The worker must
 * stay 100% stateless: it wakes on an OS event, decrypts one item from the raw
 * SVK, and terminates. It NEVER touches SQLite.
 *
 * SECURITY: this surface exposes only the crypto-only `decrypt_secret_with_svk`
 * entrypoint. `read_secret` (desktop-api feature) is compiled OUT of this WASM
 * target and MUST NOT appear in the extension bundle (build-env-deploy §2.5).
 * The browser extension build aliases this specifier to the built glue or a dev
 * mock (see apps/extension/vite.config).
 */
declare module 'vautr-wasm-nodejs' {
  /** One-time async initialisation of the nodejs-target WASM instance. */
  export function init(): Promise<void>;

  /**
   * Statelessly decrypt a single item's ciphertext directly from the raw SVK.
   * u64 epoch (encKeyGen) crosses as a JS number (fits in safe-integer range).
   */
  export function decrypt_secret_with_svk(
    svk: Uint8Array,
    uuid: string,
    encKeyGen: number,
    payload: Uint8Array,
  ): string;
}
