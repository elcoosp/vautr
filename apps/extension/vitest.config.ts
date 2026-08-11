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
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@vautr/client-sdk/extension': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/extension.ts', import.meta.url),
      ),
      '@vautr/client-sdk/storage': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/storage.ts', import.meta.url),
      ),
      '@vautr/client-sdk/real': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/realClient.ts', import.meta.url),
      ),
      '@vautr/client-sdk': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/index.ts', import.meta.url),
      ),
      '@vautr/api-contract': fileURLToPath(
        new URL('../../packages/api-contract/src/index.ts', import.meta.url),
      ),
      'vautr-wasm-nodejs': fileURLToPath(
        new URL('./src/lib/wasmNodejs.node.ts', import.meta.url),
      ),
    },
  },
  test: {
    environment: 'node',
    include: ['tests/**/*.test.ts'],
    exclude: ['**/*.spec.ts'],
  },
});
