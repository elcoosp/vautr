import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vitest/config';

/**
 * Vitest config for the extension package.
 *
 * Only `*.test.ts` files run under `pnpm test`; the Playwright smoke spec
 * (`*.spec.ts`) is excluded so it does not run under vitest.
 */
export default defineConfig({
  resolve: {
    alias: {
      '@vautr/client-sdk/extension': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/extension.ts', import.meta.url),
      ),
      'vautr-wasm-nodejs': fileURLToPath(
        new URL('./src/lib/wasmNodejs.ts', import.meta.url),
      ),
    },
  },
  test: {
    environment: 'node',
    include: ['tests/**/*.test.ts'],
    exclude: ['**/*.spec.ts'],
  },
});
