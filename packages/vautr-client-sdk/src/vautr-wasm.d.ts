/**
 * Ambient type declaration for the vautr-wasm WebAssembly glue module.
 *
 * The real module is produced by `wasm-pack` from `core/vautr-wasm`. The web
 * app aliases this specifier to either the built `.wasm` glue or a mock during
 * development (see apps/web/vite.config). Only the crypto-only `WebClient`
 * surface is exposed to JS (data.md §1 rule 4); `read_secret` is compiled out.
 */
declare module 'vautr-wasm' {
  export class WebClient {
    constructor();
    set_action_handler(handler: (actionJson: string, secret: string) => void): void;
    unlock_with_raw_key(rawKey: Uint8Array, localGen: number): void;
    reveal_secret(uuid: string, encKeyGen: number, payload: Uint8Array): number;
    perform_action(actionJson: string, handle: number): void;
    release_secret(handle: number): void;
    encrypt_secret(uuid: string, encKeyGen: number, plaintext: Uint8Array): Uint8Array;
  }
}
