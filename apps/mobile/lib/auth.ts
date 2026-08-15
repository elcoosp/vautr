/**
 * Session + auth for the mobile client.
 *
 * Implements the OPAQUE register/login flow (api.md §3) against the live server,
 * then persists the bearer token + KDF salt in the OS keychain via
 * `expo-secure-store` (biometric-accessible). The token store is injected so the
 * live-server integration test can use an in-memory store instead.
 */

type SecureStoreModule = typeof import('expo-secure-store');

/** Lazily load expo-secure-store so Node/test imports never pull in RN. */
async function getSecureStore(): Promise<SecureStoreModule> {
  return await import('expo-secure-store');
}

import type { MobileApiClient } from './api';
import { createCryptoProvider, type VautrCryptoProvider } from './crypto/provider';

/** Keychain keys (expo-secure-store). */
const TOKEN_KEY = 'vautr.session_token';
const SALT_KEY = 'vautr.kdf_salt';
const USERNAME_KEY = 'vautr.username';
const RECOVERY_ENC_KEY = 'vautr.recovery_mnemonic_enc';

/** Minimal base64 codec that works in Node + RN. */
export function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
export function fromBase64(value: string): Uint8Array {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

/** Abstracted credential persistence (keychain on device, memory in tests). */
export interface TokenStore {
  getToken(): Promise<string | null>;
  setToken(token: string | null): Promise<void>;
  getKdfSalt(): Promise<string | null>;
  setKdfSalt(salt: string | null): Promise<void>;
  getUsername(): Promise<string | null>;
  setUsername(username: string | null): Promise<void>;
  /**
   * Recovery Key mnemonic, sealed under the KEK (base64). ZK: the plaintext
   * mnemonic is never persisted; only this KEK-sealed blob is stored in the
   * keychain. `getEmergencyKit()` opens it after login (KEK available).
   */
  getRecoveryMnemonicEnc(): Promise<string | null>;
  setRecoveryMnemonicEnc(enc: string | null): Promise<void>;
}

/** Keychain-backed store using expo-secure-store (device). */
export const secureTokenStore: TokenStore = {
  async getToken() {
    return (await getSecureStore()).getItemAsync(TOKEN_KEY);
  },
  async setToken(token) {
    const ss = await getSecureStore();
    if (token === null) await ss.deleteItemAsync(TOKEN_KEY);
    else await ss.setItemAsync(TOKEN_KEY, token);
  },
  async getKdfSalt() {
    return (await getSecureStore()).getItemAsync(SALT_KEY);
  },
  async setKdfSalt(salt) {
    const ss = await getSecureStore();
    if (salt === null) await ss.deleteItemAsync(SALT_KEY);
    else await ss.setItemAsync(SALT_KEY, salt);
  },
  async getUsername() {
    return (await getSecureStore()).getItemAsync(USERNAME_KEY);
  },
  async setUsername(username) {
    const ss = await getSecureStore();
    if (username === null) await ss.deleteItemAsync(USERNAME_KEY);
    else await ss.setItemAsync(USERNAME_KEY, username);
  },
  async getRecoveryMnemonicEnc() {
    return (await getSecureStore()).getItemAsync(RECOVERY_ENC_KEY);
  },
  async setRecoveryMnemonicEnc(enc) {
    const ss = await getSecureStore();
    if (enc === null) await ss.deleteItemAsync(RECOVERY_ENC_KEY);
    else await ss.setItemAsync(RECOVERY_ENC_KEY, enc);
  },
};

/** In-memory token store (used by the live-server integration test). */
export function createMemoryTokenStore(): TokenStore {
  const data: Record<string, string | null> = {
    token: null,
    salt: null,
    username: null,
  };
  return {
    async getToken() {
      return data.token ?? null;
    },
    async setToken(token) {
      data.token = token;
    },
    async getKdfSalt() {
      return data.salt ?? null;
    },
    async setKdfSalt(salt) {
      data.salt = salt;
    },
    async getUsername() {
      return data.username ?? null;
    },
    async setUsername(username) {
      data.username = username;
    },
    async getRecoveryMnemonicEnc() {
      return data.recoveryEnc ?? null;
    },
    async setRecoveryMnemonicEnc(enc) {
      data.recoveryEnc = enc;
    },
  };
}

export interface AuthResult {
  username: string;
  sessionToken: string;
}

/** OPAQUE register + login against the live server. */
export class VautrAuth {
  private readonly api: MobileApiClient;
  private readonly store: TokenStore;
  private readonly crypto: VautrCryptoProvider | null;
  /** Decrypted Recovery Key mnemonic (opened at login). */
  private cachedMnemonic: string | null = null;
  /** Pending kit from the just-completed register (shown once in onboarding). */
  pendingRecoveryMnemonic: string | null = null;

  constructor(options: {
    api: MobileApiClient;
    store: TokenStore;
    crypto?: VautrCryptoProvider | null;
  }) {
    this.api = options.api;
    this.store = options.store;
    this.crypto = options.crypto ?? null;
  }

  private async readyCrypto(): Promise<VautrCryptoProvider> {
    return this.crypto ?? createCryptoProvider();
  }

  /** Register a brand-new account (OPAQUE registration, api.md §3.1). */
  async register(username: string, password: string): Promise<AuthResult> {
    const crypto = await this.readyCrypto();

    const kdfSalt = await crypto.generateKdfSalt();
    const mk = await crypto.deriveMasterKey(password, kdfSalt);
    const kek = await crypto.deriveKek(mk);
    const svk = await crypto.generateSvk();
    const svkWrapped = await crypto.wrapSvk(svk, kek);
    const mnemonic = await crypto.generateRecoveryMnemonic();
    const svkRkWrapped = await crypto.wrapSvkWithRk(svk, mnemonic);

    // Seal the recovery mnemonic under the KEK so it can be re-shown later
    // (Emergency Kit) without ever leaving the device in plaintext.
    const mnemonicEnc = toBase64(await crypto.wrapSvk(new TextEncoder().encode(mnemonic), kek));
    this.pendingRecoveryMnemonic = mnemonic;

    const start = await crypto.opaqueRegisterStart(password);
    const startResp = await this.api.authRegisterStart(username, toBase64(start.message));
    if (!startResp.registration_response) {
      throw new Error('register/start returned no response');
    }
    const upload = await crypto.opaqueRegisterFinish(
      start.state,
      fromBase64(startResp.registration_response),
      password,
      username,
    );
    await this.api.authRegisterFinish(
      username,
      toBase64(upload),
      toBase64(new Uint8Array(32)),
      toBase64(kdfSalt),
      toBase64(svkWrapped),
      toBase64(svkRkWrapped),
    );

    // Persist the KDF salt so a later login can re-derive the master key.
    await this.store.setUsername(username);
    await this.store.setKdfSalt(toBase64(kdfSalt));
    await this.store.setRecoveryMnemonicEnc(mnemonicEnc);

    // Register does not mint a session token; follow with login().
    return this.login(username, password);
  }

  /**
   * Emergency Kit (Recovery Key) for display/download. ZK: the mnemonic is
   * opened locally from the KEK-sealed keychain blob; never transmitted.
   * Returns null if no kit was generated (legacy accounts).
   */
  getEmergencyKit(): { mnemonic: string; words: string[] } | null {
    const m = this.cachedMnemonic;
    if (!m) return null;
    return { mnemonic: m, words: m.split(/\s+/).filter(Boolean) };
  }
  async login(username: string, password: string): Promise<AuthResult> {
    const crypto = await this.readyCrypto();

    const start = await crypto.opaqueLoginStart(password);
    const startResp = await this.api.authLoginStart(username, toBase64(start.message));
    if (!startResp.login_response) {
      throw new Error('login/start returned no response');
    }
    const finish = await crypto.opaqueLoginFinish(
      start.state,
      fromBase64(startResp.login_response),
      password,
      username,
    );
    const finishResp = await this.api.authLoginFinish(username, toBase64(finish.upload));

    this.api.setToken(finishResp.session_token);
    await this.store.setToken(finishResp.session_token);
    await this.store.setUsername(username);

    // Open the KEK-sealed Recovery Key mnemonic so the Emergency Kit can be
    // shown. ZK: opened locally from the keychain blob; never sent anywhere.
    try {
      const saltB64 = await this.store.getKdfSalt();
      const encB64 = await this.store.getRecoveryMnemonicEnc();
      if (saltB64 && encB64) {
        const mk = await crypto.deriveMasterKey(password, fromBase64(saltB64));
        const kek = await crypto.deriveKek(mk);
        const opened = await crypto.unwrapSvk(fromBase64(encB64), kek);
        this.cachedMnemonic = new TextDecoder().decode(opened);
      }
    } catch {
      this.cachedMnemonic = null;
    }

    return { username, sessionToken: finishResp.session_token };
  }

  /** Whether a session token is persisted (used to skip the login screen). */
  async hasSession(): Promise<boolean> {
    return (await this.store.getToken()) !== null;
  }

  /** Load the persisted token back onto a fresh API client. */
  async restore(api: MobileApiClient): Promise<boolean> {
    const token = await this.store.getToken();
    if (token) {
      api.setToken(token);
      return true;
    }
    return false;
  }

  /** The persisted username, if any. */
  async getUsername(): Promise<string | null> {
    return this.store.getUsername();
  }

  /** Clear the local session (does not revoke the server token). */
  async logout(): Promise<void> {
    this.api.setToken(null);
    await this.store.setToken(null);
  }
}
