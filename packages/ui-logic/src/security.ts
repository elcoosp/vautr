/**
 * Password-manager security logic (mlp-scope.md §3): a cryptographically-secure
 * password generator, weak / reused-password detection, and a standards-based
 * TOTP code generator for the vault UI.
 *
 * All pure, framework-agnostic logic lives here so web / extension clients can
 * share it (packages/ui-logic is the shared seam). Plaintext passwords are only
 * ever held transiently for strength checks; nothing is persisted.
 */

// ---------------------------------------------------------------------------
// Password generator
// ---------------------------------------------------------------------------

export interface GeneratorOptions {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  avoidAmbiguous?: boolean;
}

const UPPER = 'ABCDEFGHJKLMNPQRSTUVWXYZ';
const LOWER = 'abcdefghijkmnopqrstuvwxyz';
const DIGITS = '23456789';
const SYMBOLS = '!@#$%^&*()-_=+[]{};:,.?';
const AMBIGUOUS = new Set(['I', 'O', 'l', '0', '1', 'o']);

function charsetFor(options: GeneratorOptions): string {
  let chars = '';
  if (options.uppercase) chars += UPPER;
  if (options.lowercase) chars += LOWER;
  if (options.digits) chars += DIGITS;
  if (options.symbols) chars += SYMBOLS;
  if (options.avoidAmbiguous) {
    chars = [...chars].filter((c) => !AMBIGUOUS.has(c)).join('');
  }
  return chars;
}

/** Generate a cryptographically-secure random password (WebCrypto CSPRNG). */
export function generatePassword(options: GeneratorOptions): string {
  const chars = charsetFor(options);
  if (!chars) {
    throw new Error('at least one character class is required');
  }
  const bytes = new Uint32Array(options.length);
  crypto.getRandomValues(bytes);
  const pool = Uint32Array.from(chars, (_c, i) => i);
  const result = new Array<string>(options.length);
  for (let i = 0; i < options.length; i++) {
    const rand = bytes[i] ?? 0;
    const poolIdx = pool[rand % pool.length] ?? 0;
    result[i] = chars[poolIdx] ?? 'a';
  }
  return result.join('');
}

export const DEFAULT_GENERATOR_OPTIONS: GeneratorOptions = {
  length: 20,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true,
  avoidAmbiguous: true,
};

// ---------------------------------------------------------------------------
// Strength / weak / reused detection
// ---------------------------------------------------------------------------

export interface StrengthResult {
  /** Estimated Shannon entropy in bits. */
  entropyBits: number;
  /** Whether the password appears on a common/known-weak list. */
  isCommon: boolean;
  /** Whether the password matches a known/reused password in the account. */
  isReused: boolean;
  score: 'weak' | 'fair' | 'good' | 'strong';
  suggestions: string[];
}

/** A small list of notoriously common passwords (mlp-scope §3 weak detection). */
const COMMON_PASSWORDS = new Set([
  'password',
  'password1',
  'password123',
  '123456',
  '12345678',
  '123456789',
  'qwerty',
  'qwerty123',
  'letmein',
  'welcome',
  'admin',
  'admin123',
  'iloveyou',
  'monkey',
  'dragon',
  'abc123',
  'football',
  'princess',
  'sunshine',
  'master',
  'login',
  '111111',
  '000000',
  '1234567890',
  '1q2w3e4r',
  'trustno1',
  'zaq1zaq1',
]);

/** Estimate Shannon entropy from character-class diversity + length. */
export function entropyBits(password: string): number {
  if (!password) return 0;
  let poolSize = 0;
  if (/[a-z]/.test(password)) poolSize += 26;
  if (/[A-Z]/.test(password)) poolSize += 26;
  if (/[0-9]/.test(password)) poolSize += 10;
  if (/[^A-Za-z0-9]/.test(password)) poolSize += 32;
  return Math.round(password.length * Math.log2(Math.max(poolSize, 2)));
}

/** Classify a password's strength and flag weak / reused candidates. */
export function analyzePassword(password: string, knownPasswords: readonly string[] = []): StrengthResult {
  const entropyBitsValue = entropyBits(password);
  const isCommon = COMMON_PASSWORDS.has(password.toLowerCase());
  const isReused =
    password.length > 0 &&
    knownPasswords.some((p) => p.length > 0 && p === password);
  const suggestions: string[] = [];
  if (password.length < 12) suggestions.push('Use at least 12 characters.');
  if (!/[a-z]/.test(password)) suggestions.push('Add lowercase letters.');
  if (!/[A-Z]/.test(password)) suggestions.push('Add uppercase letters.');
  if (!/[0-9]/.test(password)) suggestions.push('Add digits.');
  if (!/[^A-Za-z0-9]/.test(password)) suggestions.push('Add symbols.');
  if (isCommon) suggestions.push('This password is on the list of common passwords.');
  if (isReused) suggestions.push('You have reused this password elsewhere.');

  let score: StrengthResult['score'];
  if (isCommon || password.length < 8 || entropyBitsValue < 40) score = 'weak';
  else if (entropyBitsValue < 60) score = 'fair';
  else if (entropyBitsValue < 90) score = 'good';
  else score = 'strong';

  return { entropyBits: entropyBitsValue, isCommon, isReused, score, suggestions };
}

// ---------------------------------------------------------------------------
// TOTP (RFC 6238) — HMAC-SHA1 via WebCrypto, base32 secret decoding
// ---------------------------------------------------------------------------

const BASE32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

function base32Decode(input: string): Uint8Array {
  const clean = input.replace(/[\s=]/g, '').toUpperCase();
  let bits = 0;
  let value = 0;
  const out: number[] = [];
  for (const ch of clean) {
    const idx = BASE32.indexOf(ch);
    if (idx < 0) continue;
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return new Uint8Array(out);
}

/** Extract the base32 secret from an otpauth:// URI or a bare secret. */
export function totpSecret(otpauth: string): string {
  if (otpauth.startsWith('otpauth://')) {
    const url = new URL(otpauth);
    const secret = url.searchParams.get('secret');
    if (secret) return secret;
    // Fall back to parsing the authority path "otpauth://totp/...:label?secret=..."
    const match = otpauth.match(/secret=([A-Za-z2-7]+)/);
    if (match) return match[1] ?? '';
    return '';
  }
  return otpauth;
}

/** Compute the current TOTP code for a base32 secret (RFC 6238, SHA1, 6 digits). */
export async function totpCode(otpauth: string, step = 30): Promise<string> {
  const secret = totpSecret(otpauth);
  if (!secret) throw new Error('no TOTP secret');
  const key = await crypto.subtle.importKey(
    'raw',
    base32Decode(secret) as unknown as BufferSource,
    { name: 'HMAC', hash: 'SHA-1' },
    false,
    ['sign'],
  );
  const counter = BigInt(Math.floor(Date.now() / 1000 / step));
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setUint32(4, Number(counter & 0xffffffffn), false);
  view.setUint32(0, Number(counter >> 32n), false);
  const mac = await crypto.subtle.sign('HMAC', key, buf as unknown as BufferSource);
  const hmac = new Uint8Array(mac);
  const offset = hmac[hmac.length - 1] ?? 0;
  const bin =
    (((hmac[offset] ?? 0) & 0x7f) << 24) |
    (((hmac[offset + 1] ?? 0) & 0xff) << 16) |
    (((hmac[offset + 2] ?? 0) & 0xff) << 8) |
    ((hmac[offset + 3] ?? 0) & 0xff);
  const code = bin % 1_000_000;
  return String(code).padStart(6, '0');
}
