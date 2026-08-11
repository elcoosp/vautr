import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { crx } from '@crxjs/vite-plugin';
import manifest from './manifest.config';

/**
 * Vite config for the Manifest V3 extension (build-env-deploy §3.3).
 *
 * The SDK and ui-logic packages are consumed directly from source (like the web
 * app) so there is no publish/build dependency between extension and packages.
 * The two WASM specifiers are aliased to dev mocks:
 *  - `vautr-wasm`        -> popup's `--target web` WebClient mock.
 *  - `vautr-wasm-nodejs` -> SW's  `--target nodejs` stateless crypto mock.
 * Point these aliases at the real wasm-pack outputs to exercise real crypto.
 */
export default defineConfig({
  plugins: [react(), crx({ manifest })],
  resolve: {
    alias: {
      '@vautr/ui-logic': fileURLToPath(
        new URL('../../packages/ui-logic/src/index.ts', import.meta.url),
      ),
      // Longer subpath MUST precede the bare package alias so the resolver
      // matches `@vautr/client-sdk/extension` before `@vautr/client-sdk`.
      '@vautr/client-sdk/extension': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/extension.ts', import.meta.url),
      ),
      '@vautr/client-sdk': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/index.ts', import.meta.url),
      ),
      'vautr-wasm': fileURLToPath(new URL('./src/lib/mockWasmWeb.ts', import.meta.url)),
      'vautr-wasm-nodejs': fileURLToPath(
        new URL('./src/lib/mockWasmNodejs.ts', import.meta.url),
      ),
    },
  },
  build: {
    // CRXJS expects a plain `dist/` layout for the unpacked extension.
    target: 'esnext',
  },
});
