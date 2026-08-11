/**
 * Vautr client SDK — password generator + weak / reused detection (mlp-scope §3).
 *
 * These are pure, dependency-free helpers so both the extension popup and the
 * web vault can share one implementation. No crypto secrets are involved; the
 * functions operate on plaintext the user is actively creating/revealing.
 */

export interface GeneratorOptions {
  length?: number;
  upper?: boolean;
  lower?: boolean;
  digits?: boolean;
  symbols?: boolean;
  /** Drop visually ambiguous characters (0O, 1lI, etc.). */
  excludeAmbiguous?: boolean;
}

const AMBIGUOUS = '0O1lI|';
const CHARSETS = {
  upper: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ',
  lower: 'abcdefghijklmnopqrstuvwxyz',
  digits: '0123456789',
  symbols: '!@#$%^&*()-_=+[]{};:,.<>?',
};

const DEFAULT_OPTIONS: Required<GeneratorOptions> = {
  length: 20,
  upper: true,
  lower: true,
  digits: true,
  symbols: true,
  excludeAmbiguous: true,
};

function secureRandomInt(max: number): number {
  const arr = new Uint32Array(1);
  crypto.getRandomValues(arr);
  return (arr[0] ?? 0) % max;
}

/**
 * Generate a cryptographically-random password from the selected charsets.
 * Guarantees at least one character from each enabled charset so it always
 * meets every enabled complexity rule.
 */
export function generatePassword(options: GeneratorOptions = {}): string {
  const opts: Required<GeneratorOptions> = { ...DEFAULT_OPTIONS, ...options };
  const enabled = (['upper', 'lower', 'digits', 'symbols'] as const).filter(
    (k) => opts[k],
  );
  if (enabled.length === 0) {
    return '';
  }
  const pool = enabled
    .map((k) => CHARSETS[k])
    .join('')
    .split('');
  const filtered = opts.excludeAmbiguous
    ? pool.filter((c) => !AMBIGUOUS.includes(c))
    : pool;

  const parts: string[] = [];
  // Guarantee one of each enabled charset.
  for (const k of enabled) {
    const chars = (opts.excludeAmbiguous
      ? CHARSETS[k].split('').filter((c) => !AMBIGUOUS.includes(c))
      : CHARSETS[k].split('')
    ).filter((c) => c);
    if (chars.length > 0) {
      parts.push(chars[secureRandomInt(chars.length)] ?? '');
    }
  }
  while (parts.length < opts.length) {
    parts.push(filtered[secureRandomInt(filtered.length)] ?? '');
  }
  // Fisher-Yates shuffle.
  for (let i = parts.length - 1; i > 0; i -= 1) {
    const j = secureRandomInt(i + 1);
    const a = parts[i] ?? '';
    const b = parts[j] ?? '';
    parts[i] = b;
    parts[j] = a;
  }
  return parts.join('');
}

/** Shannon entropy estimate in bits (approximation for UI scoring). */
export function estimateEntropyBits(password: string): number {
  if (!password) return 0;
  const poolSizes: number[] = [];
  if (/[a-z]/.test(password)) poolSizes.push(26);
  if (/[A-Z]/.test(password)) poolSizes.push(26);
  if (/[0-9]/.test(password)) poolSizes.push(10);
  if (/[^a-zA-Z0-9]/.test(password)) poolSizes.push(32);
  if (!poolSizes.length) return 0;
  const pool = poolSizes.reduce((a, b) => a + b, 0);
  return password.length * Math.log2(pool);
}

export interface PasswordStrength {
  score: 0 | 1 | 2 | 3 | 4;
  entropy: number;
  length: number;
  hasUpper: boolean;
  hasLower: boolean;
  hasDigit: boolean;
  hasSymbol: boolean;
  feedback: string[];
}

/** Assess password strength (0-4) using entropy + character-class coverage. */
export function assessPassword(password: string): PasswordStrength {
  const entropy = estimateEntropyBits(password);
  const length = password.length;
  const hasUpper = /[A-Z]/.test(password);
  const hasLower = /[a-z]/.test(password);
  const hasDigit = /[0-9]/.test(password);
  const hasSymbol = /[^a-zA-Z0-9]/.test(password);
  const classes = [hasUpper, hasLower, hasDigit, hasSymbol].filter(Boolean).length;

  let score: PasswordStrength['score'] = 0;
  if (length >= 8 && classes >= 3) score = 1;
  if (length >= 12 && entropy >= 50) score = 2;
  if (length >= 16 && entropy >= 70 && classes >= 3) score = 3;
  if (length >= 20 && entropy >= 100) score = 4;

  const feedback: string[] = [];
  if (length < 12) feedback.push('Use at least 12 characters.');
  if (classes < 3) feedback.push('Mix upper/lowercase, digits and symbols.');
  if (entropy < 60) feedback.push('Avoid words, names and keyboard patterns.');
  if (['password', '123456', 'qwerty', 'admin', 'letmein'].includes(password.toLowerCase())) {
    score = 0;
    feedback.push('This is a known common password.');
  }
  return { score, entropy: Math.round(entropy), length, hasUpper, hasLower, hasDigit, hasSymbol, feedback };
}

/**
 * Detect whether `candidate` is reused against a set of known plaintext
 * passwords (e.g. other items the user has already revealed this session).
 * Comparison is case-sensitive, whitespace-preserving to avoid false positives.
 */
export function isReusedPassword(candidate: string, knownPasswords: readonly string[]): boolean {
  if (!candidate) return false;
  return knownPasswords.some((p) => p === candidate);
}

/** Convenience label for a strength score. */
export function strengthLabel(score: PasswordStrength['score']): string {
  switch (score) {
    case 0:
      return 'Very weak';
    case 1:
      return 'Weak';
    case 2:
      return 'Fair';
    case 3:
      return 'Strong';
    case 4:
      return 'Excellent';
  }
}
