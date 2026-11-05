/**
 * Web Crypto guard for React Native / Hermes.
 *
 * VTRFIX-SEC-C04: this module previously installed a `Math.random`-backed
 * `crypto.getRandomValues` (+ `randomUUID`) global. That silently defeated the
 * CSPRNG fallback logic in `lib/http.ts` (every "random" byte became
 * predictable) and made a weak PRNG reachable from any security-relevant code
 * path. A Math.random fill is NOT acceptable randomness for a password manager,
 * even "just for onboarding IDs" — identifiers derived from it were spoofable.
 *
 * Current behaviour:
 * - If the platform provides `crypto.getRandomValues` (the RN New Architecture
 *   ships a JSI Web Crypto; newer Hermes builds may too), everything works and
 *   there is nothing to do.
 * - If it does not, we deliberately install NOTHING. Callers fail loudly
 *   (ReferenceError) instead of silently using a predictable PRNG.
 *   `lib/http.ts` owns its own counter+timestamp fallback for X-Request-ID so
 *   request IDs never depend on this global.
 *
 * To restore a polyfill for onboarding IDs, add `expo-crypto` and seed the
 * global from `expo-crypto.getRandomValues` (a genuine CSPRNG) in a follow-up
 * VTR. Do not reintroduce Math.random here.
 */

export {};
