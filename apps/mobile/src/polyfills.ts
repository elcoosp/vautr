/**
 * Web Crypto polyfill for React Native / Hermes.
 *
 * `@onboardjs/core` calls the Web Crypto global (`crypto.getRandomValues`) when
 * building onboarding flow IDs. Hermes does not expose `crypto` as a global by
 * default, so we set it here. We prefer any platform-provided implementation
 * (React Native 0.73+ ships a JSI Web Crypto under `globalThis.crypto` in the
 * New Architecture) and only fall back to a `Math.random`-backed fill for the
 * non-security-critical onboarding identifiers.
 */
const g = globalThis as unknown as {
  crypto?: { getRandomValues?: (a: Uint8Array) => Uint8Array; randomUUID?: () => string };
};

if (typeof g.crypto === 'undefined' || typeof g.crypto.getRandomValues !== 'function') {
  const cryptoImpl = g.crypto ?? {};
  cryptoImpl.getRandomValues = (arr: Uint8Array): Uint8Array => {
    for (let i = 0; i < arr.length; i++) {
      arr[i] = Math.floor(Math.random() * 256);
    }
    return arr;
  };
  if (typeof cryptoImpl.randomUUID !== 'function') {
    cryptoImpl.randomUUID = (): string => {
      // getRandomValues is assigned just above, so the assertion is sound.
      // biome-ignore lint/style/noNonNullAssertion: guaranteed defined on the prior line
      const bytes = cryptoImpl.getRandomValues!(new Uint8Array(16));
      const hex = Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
      return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    };
  }
  g.crypto = cryptoImpl;
}

export {};
