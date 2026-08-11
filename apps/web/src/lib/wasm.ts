/**
 * Real `vautr-wasm` `--target web` build, wired for the live server.
 *
 * wasm-bindgen's `--target web` output requires `await init()` before any of
 * the crypto functions are callable. We await it at module scope (top-level
 * await) so the dynamic `import('vautr-wasm')` performed by the
 * `@vautr/client-sdk` crypto adapter resolves only AFTER the wasm is ready.
 *
 * The generated wasm-bindgen types use `bigint` for `u64` fields. Vautr epochs
 * (`enc_key_gen`) are small values within safe-integer range, so the boundary
 * functions accept `number` and cast to `bigint`.
 *
 * This is the production module. The Vite alias `vautr-wasm` must point here
 * (not at the throwing `mockWasm.ts` shim) to exercise real crypto against the
 * live server.
 */
import init, * as raw from '../../wasm-pkg/vautr_wasm.js';

// Instantiate the wasm before anything is exported.
await init();

export const {
  generate_kdf_salt_js,
  derive_master_key_js,
  derive_kek_js,
  generate_svk_js,
  wrap_svk_js,
  unwrap_svk_js,
  generate_recovery_mnemonic_js,
  wrap_svk_with_rk_js,
  derive_dek_js,
}: {
  generate_kdf_salt_js: () => Uint8Array;
  derive_master_key_js: (password: string, salt: Uint8Array) => Uint8Array;
  derive_kek_js: (master_key: Uint8Array) => Uint8Array;
  generate_svk_js: () => Uint8Array;
  wrap_svk_js: (svk: Uint8Array, kek: Uint8Array) => Uint8Array;
  unwrap_svk_js: (wrapped: Uint8Array, kek: Uint8Array) => Uint8Array;
  generate_recovery_mnemonic_js: () => string;
  wrap_svk_with_rk_js: (svk: Uint8Array, mnemonic: string) => Uint8Array;
  derive_dek_js: (svk: Uint8Array) => Uint8Array;
} = raw as any;

function ensureBigInt(v: number): bigint {
  return BigInt(Math.trunc(v));
}

export function encrypt_item_js(
  uuid: string,
  enc_key_gen: number,
  dek: Uint8Array,
  plaintext: Uint8Array,
): Uint8Array {
  return (raw as any).encrypt_item_js(uuid, ensureBigInt(enc_key_gen), dek, plaintext);
}

export function decrypt_item_js(
  uuid: string,
  enc_key_gen: number,
  dek: Uint8Array,
  payload: Uint8Array,
): Uint8Array {
  return (raw as any).decrypt_item_js(uuid, ensureBigInt(enc_key_gen), dek, payload);
}

export function opaque_register_start_js(password: string): { message: Uint8Array; state: Uint8Array } {
  return (raw as any).opaque_register_start_js(password);
}

export function opaque_register_finish_js(
  client_state: Uint8Array,
  server_response: Uint8Array,
  password: string,
  username: string,
): Uint8Array {
  return (raw as any).opaque_register_finish_js(client_state, server_response, password, username);
}

export function opaque_login_start_js(password: string): { message: Uint8Array; state: Uint8Array } {
  return (raw as any).opaque_login_start_js(password);
}

export function opaque_login_finish_js(
  client_state: Uint8Array,
  server_response: Uint8Array,
  password: string,
  username: string,
): { upload: Uint8Array; sessionKey: Uint8Array } {
  const result = (raw as any).opaque_login_finish_js(
    client_state,
    server_response,
    password,
    username,
  ) as { upload: Uint8Array; session_key: Uint8Array };
  return { upload: result.upload, sessionKey: result.session_key };
}
