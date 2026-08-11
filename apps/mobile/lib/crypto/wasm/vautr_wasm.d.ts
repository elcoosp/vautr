/* tslint:disable */
/* eslint-disable */

/**
 * Web-facing crypto-only client. Holds the unlocked vault key (SVK) + derived
 * DEK, and a handle store for revealed secrets.
 */
export class WebClient {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Re-encrypt a plaintext payload under the current DEK (used by the JS layer
     * when persisting a new/updated item). Returns the AEAD envelope.
     */
    encrypt_secret(uuid: string, enc_key_gen: bigint, plaintext: Uint8Array): Uint8Array;
    /**
     * Create a (locked) web client.
     */
    constructor();
    /**
     * Delegate copy/autofill to the JS platform handler. `action_json` is a
     * serialized action (`{"CopyToClipboard":{"handle":N}}` or
     * `{"Autofill":{"handle":N}}`). The handler receives the plaintext secret
     * directly and is responsible for zeroizing it after use.
     */
    perform_action(action_json: string, handle: number): void;
    /**
     * Explicitly release a handle (zeroizes the in-memory secret).
     */
    release_secret(handle: number): void;
    /**
     * Decrypt an encrypted item payload and reveal it behind an opaque handle.
     * `payload` is the AEAD envelope; `uuid`/`enc_key_gen` bind the AD. Returns
     * the handle id (the plaintext never crosses back into JS).
     */
    reveal_secret(uuid: string, enc_key_gen: bigint, payload: Uint8Array): number;
    /**
     * Register the JS clipboard/autofill handler (`function(actionJson, secret)`).
     */
    set_action_handler(handler: Function): void;
    /**
     * Unlock directly with a raw 32-byte vault key + the local key generation.
     */
    unlock_with_raw_key(raw_key: Uint8Array, local_gen: bigint): void;
}

export function decrypt_item_js(uuid: string, enc_key_gen: bigint, dek: Uint8Array, payload: Uint8Array): Uint8Array;

export function derive_dek_js(svk: Uint8Array): Uint8Array;

export function derive_kek_js(master_key: Uint8Array): Uint8Array;

export function derive_master_key_js(password: string, salt: Uint8Array): Uint8Array;

export function encrypt_item_js(uuid: string, enc_key_gen: bigint, dek: Uint8Array, plaintext: Uint8Array): Uint8Array;

export function generate_kdf_salt_js(): Uint8Array;

export function generate_recovery_mnemonic_js(): string;

export function generate_svk_js(): Uint8Array;

/**
 * Finish OPAQUE login. Returns `{ upload, session_key }`.
 */
export function opaque_login_finish_js(client_state: Uint8Array, server_response: Uint8Array, password: string, username: string): any;

/**
 * Begin OPAQUE login. Returns `{ message, state }`.
 */
export function opaque_login_start_js(password: string): any;

/**
 * Finish OPAQUE registration. Returns the `registration_upload` (base64-ready).
 */
export function opaque_register_finish_js(client_state: Uint8Array, server_response: Uint8Array, password: string, username: string): Uint8Array;

/**
 * Begin OPAQUE registration. Returns `{ message, state }` (both `Uint8Array`).
 */
export function opaque_register_start_js(password: string): any;

export function unwrap_svk_js(wrapped: Uint8Array, kek: Uint8Array): Uint8Array;

export function wrap_svk_js(svk: Uint8Array, kek: Uint8Array): Uint8Array;

export function wrap_svk_with_rk_js(svk: Uint8Array, mnemonic: string): Uint8Array;
