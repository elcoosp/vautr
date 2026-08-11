/**
 * Crypto boundary for the browser client (ADR-005).
 *
 * All cryptographic work (KDF, OPAQUE, SVK wrap/recovery, item AEAD) lives in
 * the Rust `vautr-wasm` crate. The JS layer only ferries byte blobs and strings
 * (crypto.md §2-§7). This module is the *single* seam the rest of the client
 * touches for crypto, so it can be exercised against the live server with the
 * real wasm module.
 *
 * The adapter loads `vautr-wasm` lazily (it is a wasm module; the browser
 * aliases it to the built glue — see apps/web/vite.config). Call `ready()`
 * before issuing any operation.
 */

export interface OpaqueStart {
  /** Message to send to the server (base64 wire). */
  message: Uint8Array;
  /** Client-side OPAQUE state to keep for the finish step. */
  state: Uint8Array;
}

export interface OpaqueLoginFinish {
  /** Login upload to send to `/auth/login/finish`. */
  upload: Uint8Array;
  /** OPAQUE session key (server mints the bearer token from it). */
  sessionKey: Uint8Array;
}

type WasmModule = typeof import('vautr-wasm');

/**
 * Adapter over the real `vautr-wasm` module. Loads lazily on first `ready()`.
 * Stateless: each call is a pure call into the wasm crypto primitives.
 */
export class AsyncCryptoAdapter {
  private modulePromise: Promise<WasmModule> | null = null;
  private module: WasmModule | null = null;

  /** Ensure the wasm module is loaded. Idempotent. */
  async ready(): Promise<void> {
    if (!this.module) {
      this.module = await (this.modulePromise ??= import('vautr-wasm'));
    }
  }

  private m(): WasmModule {
    if (!this.module) {
      throw new Error('crypto adapter not ready: call ready() first');
    }
    return this.module;
  }

  // --- KDF / key tree / SVK (crypto.md §2) ---
  generateKdfSalt(): Uint8Array {
    return this.m().generate_kdf_salt_js();
  }
  deriveMasterKey(password: string, salt: Uint8Array): Uint8Array {
    return this.m().derive_master_key_js(password, salt);
  }
  deriveKek(masterKey: Uint8Array): Uint8Array {
    return this.m().derive_kek_js(masterKey);
  }
  generateSvk(): Uint8Array {
    return this.m().generate_svk_js();
  }
  wrapSvk(svk: Uint8Array, kek: Uint8Array): Uint8Array {
    return this.m().wrap_svk_js(svk, kek);
  }
  unwrapSvk(wrapped: Uint8Array, kek: Uint8Array): Uint8Array {
    return this.m().unwrap_svk_js(wrapped, kek);
  }

  // --- recovery (REQ-RECOVERY-01) ---
  generateRecoveryMnemonic(): string {
    return this.m().generate_recovery_mnemonic_js();
  }
  wrapSvkWithRk(svk: Uint8Array, mnemonic: string): Uint8Array {
    return this.m().wrap_svk_with_rk_js(svk, mnemonic);
  }

  // --- item DEK + AEAD (crypto.md §2 step 5, §3) ---
  deriveDek(svk: Uint8Array): Uint8Array {
    return this.m().derive_dek_js(svk);
  }
  encryptItem(uuid: string, encKeyGen: number, dek: Uint8Array, plaintext: Uint8Array): Uint8Array {
    return this.m().encrypt_item_js(uuid, encKeyGen, dek, plaintext);
  }
  decryptItem(uuid: string, encKeyGen: number, dek: Uint8Array, payload: Uint8Array): Uint8Array {
    return this.m().decrypt_item_js(uuid, encKeyGen, dek, payload);
  }

  // --- OPAQUE (api.md §3) ---
  opaqueRegisterStart(password: string): OpaqueStart {
    return this.m().opaque_register_start_js(password);
  }
  opaqueRegisterFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Uint8Array {
    return this.m().opaque_register_finish_js(state, serverResponse, password, username);
  }
  opaqueLoginStart(password: string): OpaqueStart {
    return this.m().opaque_login_start_js(password);
  }
  opaqueLoginFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): OpaqueLoginFinish {
    return this.m().opaque_login_finish_js(state, serverResponse, password, username);
  }
}
