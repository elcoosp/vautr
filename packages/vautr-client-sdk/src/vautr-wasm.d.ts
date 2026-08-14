/**
 * Ambient type declaration for the vautr-wasm WebAssembly glue module.
 *
 * The real module is produced by `wasm-pack` from `core/vautr-wasm` (ADR-005:
 * crypto-only, no tokio/reqwest/sea-orm). It exposes two surfaces to JS:
 *
 *   1. `WebClient` — the opaque-handle secret client (data.md §1 rule 4). Only
 *      the crypto-only subset is exported; `read_secret` is compiled out.
 *   2. The auth + key + item crypto primitives from `auth.rs` — KDF, OPAQUE,
 *      SVK wrap/recovery, DEK derivation, and item AEAD. These are the
 *      boundaries the browser client uses for register/login/sync (api.md §3-5).
 *
 * `Vec<u8>` crosses as `Uint8Array`; `&str` as `string`; `{message,state}` and
 * `{upload,session_key}` objects carry `Uint8Array` values.
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

  // --- auth.rs: KDF / key tree / SVK wrap ---
  export function generate_kdf_salt_js(): Uint8Array;
  export function derive_master_key_js(password: string, salt: Uint8Array): Uint8Array;
  export function derive_kek_js(masterKey: Uint8Array): Uint8Array;
  export function generate_svk_js(): Uint8Array;
  export function wrap_svk_js(svk: Uint8Array, kek: Uint8Array): Uint8Array;
  export function unwrap_svk_js(wrapped: Uint8Array, kek: Uint8Array): Uint8Array;
  export function generate_recovery_mnemonic_js(): string;
  export function wrap_svk_with_rk_js(svk: Uint8Array, mnemonic: string): Uint8Array;

  // --- auth.rs: item DEK + AEAD ---
  export function derive_dek_js(svk: Uint8Array): Uint8Array;
  export function encrypt_item_js(
    uuid: string,
    encKeyGen: number,
    dek: Uint8Array,
    plaintext: Uint8Array,
  ): Uint8Array;
  export function decrypt_item_js(
    uuid: string,
    encKeyGen: number,
    dek: Uint8Array,
    payload: Uint8Array,
  ): Uint8Array;

  // --- auth.rs: OPAQUE ---
  export function opaque_register_start_js(password: string): {
    message: Uint8Array;
    state: Uint8Array;
  };
  export function opaque_register_finish_js(
    clientState: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Uint8Array;
  export function opaque_login_start_js(password: string): {
    message: Uint8Array;
    state: Uint8Array;
  };
  export function opaque_login_finish_js(
    clientState: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): { upload: Uint8Array; sessionKey: Uint8Array };

  // --- sharing.rs: ADR-007 1:1 sharing (KEM + DEM) ---
  // `generate_sharing_keypair` / `restore_sharing_keypair` return a JSON object
  // `{ public: <b64>, secret: <b64> }`. `share_item` returns a `vautr_sharing::
  // ShareBundle` JSON string. `accept_share` returns the recovered plaintext.
  export function generate_sharing_keypair(): string;
  export function restore_sharing_keypair(secretB64: string): string;
  export function share_item(
    senderUuid: string,
    recipientUuid: string,
    itemUuid: string,
    recipientPublicB64: string,
    plaintext: Uint8Array,
  ): string;
  export function accept_share(incomingJson: string, recipientSecretB64: string): Uint8Array;

  // --- sharing.rs: group sharing (sharing-pki.md §6) ---
  // `create_sharing_group` returns `{ group: {group_id, name, admin_uuid}, secret }`.
  // `add_group_member` returns a WrappedGroupKey JSON. `unwrap_group_key` takes a
  // group-inbox entry + recipient secret and returns the member's `{ group, secret }`.
  // `encrypt_group_item` / `decrypt_group_item` operate under the Group SIK.
  export function create_sharing_group(name: string, adminUuid: string): string;
  export function add_group_member(
    groupJson: string,
    memberUuid: string,
    memberPublicB64: string,
  ): string;
  export function unwrap_group_key(inboxJson: string, recipientSecretB64: string): string;
  export function encrypt_group_item(
    groupJson: string,
    itemUuid: string,
    plaintext: Uint8Array,
  ): string;
  export function decrypt_group_item(
    groupJson: string,
    itemUuid: string,
    ciphertextB64: string,
  ): Uint8Array;
}
