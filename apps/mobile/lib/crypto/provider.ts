/**
 * OPAQUE + key-derivation crypto provider for the mobile client.
 *
 * The MLP server authenticates with the OPAQUE PAKE (api.md §3) and derives the
 * master key / KEK / SVK tree locally (crypto.md §2). On web and the browser
 * extension this runs in the `vautr-wasm` module (a JS-engine WebAssembly
 * build). React Native's Hermes engine does not implement WebAssembly, so the
 * wasm crypto path cannot run on device.
 *
 * The intended mobile path is the native uniffi core (`vautr-ffi`), which is
 * linked for the post-auth vault surface (reveal overlay, sharing, sync). That
 * core currently expects the OPAQUE-derived `kdf_salt` / `wrapped_svk` to be
 * supplied by the client, i.e. it assumes OPAQUE registration already happened
 * elsewhere. Account creation / first unlock on mobile therefore requires the
 * OPAQUE client to run natively — a follow-up that ports the OPAQUE flow into
 * `vautr-ffi` (VTR-104 follow-up). Until then we fail fast with a clear,
 * actionable message rather than a silent empty error.
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

/**
 * Mobile crypto is provided by the native core once OPAQUE is ported into
 * `vautr-ffi`. Today it is unavailable on the RN/Hermes runtime, so we surface a
 * clear, actionable error instead of crashing or failing silently.
 */
export async function createCryptoProvider(): Promise<VautrCryptoProvider> {
  throw new Error(
    'Vault creation and first unlock require the Vautr web or desktop app. ' +
      'The mobile app uses the on-device vault core, which does not yet run ' +
      'the OPAQUE key agreement on this runtime.',
  );
}
