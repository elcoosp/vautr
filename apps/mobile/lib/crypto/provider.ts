/**
 * OPAQUE + key-derivation crypto provider for the mobile client.
 *
 * The MLP server authenticates with the OPAQUE PAKE (api.md §3) and derives the
 * master key / KEK / SVK tree locally (crypto.md §2). All of that work lives in
 * the Rust core (`vautr-wasm`), surfaced to JS as pure byte-bag functions.
 *
 * On device the production build links the Rust core through the native
 * TurboModule; in this environment (no compiled uniffi module, no emulator) the
 * integration test and the Node/dev path use the vendored `vautr-wasm` Node.js
 * build (`./wasm/vautr_wasm.js`). The provider interface lets either be plugged
 * in without changing the auth flow.
 */

export interface OpaqueStart {
  message: Uint8Array;
  state: Uint8Array;
}

export interface OpaqueLoginFinish {
  upload: Uint8Array;
  sessionKey: Uint8Array;
}

/** The crypto surface the auth flow needs (subset of vautr-wasm). */
export interface VautrCryptoProvider {
  generateKdfSalt(): Promise<Uint8Array>;
  deriveMasterKey(password: string, salt: Uint8Array): Promise<Uint8Array>;
  deriveKek(masterKey: Uint8Array): Promise<Uint8Array>;
  generateSvk(): Promise<Uint8Array>;
  wrapSvk(svk: Uint8Array, kek: Uint8Array): Promise<Uint8Array>;
  unwrapSvk(wrapped: Uint8Array, kek: Uint8Array): Promise<Uint8Array>;
  generateRecoveryMnemonic(): Promise<string>;
  wrapSvkWithRk(svk: Uint8Array, mnemonic: string): Promise<Uint8Array>;
  opaqueRegisterStart(password: string): Promise<OpaqueStart>;
  opaqueRegisterFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Promise<Uint8Array>;
  opaqueLoginStart(password: string): Promise<OpaqueStart>;
  opaqueLoginFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Promise<OpaqueLoginFinish>;
}

/** The stateless `vautr-wasm` module surface (typed from its .d.ts). */
export type WasmCryptoModule = {
  generate_kdf_salt_js(): Uint8Array;
  derive_master_key_js(password: string, salt: Uint8Array): Uint8Array;
  derive_kek_js(masterKey: Uint8Array): Uint8Array;
  generate_svk_js(): Uint8Array;
  wrap_svk_js(svk: Uint8Array, kek: Uint8Array): Uint8Array;
  unwrap_svk_js(wrapped: Uint8Array, kek: Uint8Array): Uint8Array;
  generate_recovery_mnemonic_js(): string;
  wrap_svk_with_rk_js(svk: Uint8Array, mnemonic: string): Uint8Array;
  opaque_register_start_js(password: string): { message: Uint8Array; state: Uint8Array };
  opaque_register_finish_js(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Uint8Array;
  opaque_login_start_js(password: string): { message: Uint8Array; state: Uint8Array };
  opaque_login_finish_js(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): OpaqueLoginFinish;
};

/** Adapter over the vendored Node.js wasm build. */
export class WasmCryptoProvider implements VautrCryptoProvider {
  private module: WasmCryptoModule | null = null;

  private async m(): Promise<WasmCryptoModule> {
    if (!this.module) {
      const mod = (await import('./wasm/vautr_wasm.js')) as unknown as WasmCryptoModule;
      this.module = mod;
    }
    return this.module;
  }

  async generateKdfSalt(): Promise<Uint8Array> {
    return (await this.m()).generate_kdf_salt_js();
  }
  async deriveMasterKey(password: string, salt: Uint8Array): Promise<Uint8Array> {
    return (await this.m()).derive_master_key_js(password, salt);
  }
  async deriveKek(masterKey: Uint8Array): Promise<Uint8Array> {
    return (await this.m()).derive_kek_js(masterKey);
  }
  async generateSvk(): Promise<Uint8Array> {
    return (await this.m()).generate_svk_js();
  }
  async wrapSvk(svk: Uint8Array, kek: Uint8Array): Promise<Uint8Array> {
    return (await this.m()).wrap_svk_js(svk, kek);
  }
  async unwrapSvk(wrapped: Uint8Array, kek: Uint8Array): Promise<Uint8Array> {
    return (await this.m()).unwrap_svk_js(wrapped, kek);
  }
  async generateRecoveryMnemonic(): Promise<string> {
    return (await this.m()).generate_recovery_mnemonic_js();
  }
  async wrapSvkWithRk(svk: Uint8Array, mnemonic: string): Promise<Uint8Array> {
    return (await this.m()).wrap_svk_with_rk_js(svk, mnemonic);
  }
  async opaqueRegisterStart(password: string): Promise<OpaqueStart> {
    return (await this.m()).opaque_register_start_js(password);
  }
  async opaqueRegisterFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Promise<Uint8Array> {
    return (await this.m()).opaque_register_finish_js(state, serverResponse, password, username);
  }
  async opaqueLoginStart(password: string): Promise<OpaqueStart> {
    return (await this.m()).opaque_login_start_js(password);
  }
  async opaqueLoginFinish(
    state: Uint8Array,
    serverResponse: Uint8Array,
    password: string,
    username: string,
  ): Promise<OpaqueLoginFinish> {
    return (await this.m()).opaque_login_finish_js(state, serverResponse, password, username);
  }
}

/** A crypto provider that delegates to the platform (native core) if present. */
export async function createCryptoProvider(): Promise<VautrCryptoProvider> {
  // In this build we use the vendored wasm (Node/dev path). A native TurboModule
  // implementation of the same surface can be returned here on device.
  return new WasmCryptoProvider();
}
