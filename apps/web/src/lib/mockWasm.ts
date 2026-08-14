/**
 * Dev shim for the `vautr-wasm` module (crypto-only surface, ADR-005).
 *
 * The real module is produced by `wasm-pack` from `core/vautr-wasm` and exposes
 * the `WebClient` + the auth/key/item crypto primitives. This shim mirrors the
 * exact JS API so the SPA typechecks and builds without a WASM build, but the
 * crypto functions throw at runtime with a clear message — it never silently
 * fakes cryptography. Swap the Vite alias to the real built module to exercise
 * real crypto (register/login/sync against the live server).
 *
 * The register→login→add→pull gate against the live server is proven at the
 * Rust level by `core/vautr-wasm/tests/live_server.rs`.
 */

type ActionHandler = (actionJson: string, secret: string) => void;

/** OPAQUE object shapes (matching the real glue). */
export interface OpaqueStart {
  message: Uint8Array;
  state: Uint8Array;
}
export interface OpaqueLoginFinish {
  upload: Uint8Array;
  sessionKey: Uint8Array;
}

/** Surface parity with the real module (data.md §1 rule 4). */
export class WebClient {
  // biome-ignore lint/correctness/noUnusedPrivateClassMembers: stored for API parity with the real wasm glue; assigned via set_action_handler
  private handler: ActionHandler | null = null;

  set_action_handler(handler: ActionHandler): void {
    this.handler = handler;
  }
  unlock_with_raw_key(_rawKey: Uint8Array, _localGen: number): void {
    throw unbuilt();
  }
  reveal_secret(_uuid: string, _encKeyGen: number, _payload: Uint8Array): number {
    throw unbuilt();
  }
  perform_action(_actionJson: string, _handle: number): void {
    throw unbuilt();
  }
  release_secret(_handle: number): void {
    throw unbuilt();
  }
  encrypt_secret(_uuid: string, _encKeyGen: number, _plaintext: Uint8Array): Uint8Array {
    throw unbuilt();
  }
}

/** Raised by the dev shim so nothing silently fakes cryptography. */
function unbuilt(): Error {
  return new Error(
    'vautr-wasm build required: run the repo wasm-pack build to wire real crypto for the live server.',
  );
}

const stub = <T>(): T => {
  throw unbuilt();
};

// --- auth.rs: KDF / key tree / SVK / item / OPAQUE (surface parity) ---
export const generate_kdf_salt_js = stub<() => Uint8Array>();
export const derive_master_key_js = stub<(password: string, salt: Uint8Array) => Uint8Array>();
export const derive_kek_js = stub<(masterKey: Uint8Array) => Uint8Array>();
export const generate_svk_js = stub<() => Uint8Array>();
export const wrap_svk_js = stub<(svk: Uint8Array, kek: Uint8Array) => Uint8Array>();
export const unwrap_svk_js = stub<(wrapped: Uint8Array, kek: Uint8Array) => Uint8Array>();
export const generate_recovery_mnemonic_js = stub<() => string>();
export const wrap_svk_with_rk_js = stub<(svk: Uint8Array, mnemonic: string) => Uint8Array>();
export const derive_dek_js = stub<(svk: Uint8Array) => Uint8Array>();
export const encrypt_item_js =
  stub<(uuid: string, encKeyGen: number, dek: Uint8Array, plaintext: Uint8Array) => Uint8Array>();
export const decrypt_item_js =
  stub<(uuid: string, encKeyGen: number, dek: Uint8Array, payload: Uint8Array) => Uint8Array>();
export const opaque_register_start_js = stub<(password: string) => OpaqueStart>();
export const opaque_register_finish_js =
  stub<
    (
      clientState: Uint8Array,
      serverResponse: Uint8Array,
      password: string,
      username: string,
    ) => Uint8Array
  >();
export const opaque_login_start_js = stub<(password: string) => OpaqueStart>();
export const opaque_login_finish_js =
  stub<
    (
      clientState: Uint8Array,
      serverResponse: Uint8Array,
      password: string,
      username: string,
    ) => OpaqueLoginFinish
  >();

// --- sharing.rs: ADR-007 1:1 sharing (surface parity; throws without built wasm) ---
export const generate_sharing_keypair = stub<() => string>();
export const restore_sharing_keypair = stub<(secretB64: string) => string>();
export const share_item =
  stub<
    (
      senderUuid: string,
      recipientUuid: string,
      itemUuid: string,
      recipientPublicB64: string,
      plaintext: Uint8Array,
    ) => string
  >();
export const accept_share =
  stub<(incomingJson: string, recipientSecretB64: string) => Uint8Array>();

// --- group sharing (sharing-pki.md §6) ---
export const create_sharing_group = stub<(name: string, adminUuid: string) => string>();
export const add_group_member =
  stub<(groupJson: string, memberUuid: string, memberPublicB64: string) => string>();
export const unwrap_group_key = stub<(inboxJson: string, recipientSecretB64: string) => string>();
export const encrypt_group_item =
  stub<(groupJson: string, itemUuid: string, plaintext: Uint8Array) => string>();
export const decrypt_group_item =
  stub<(groupJson: string, itemUuid: string, ciphertextB64: string) => Uint8Array>();
