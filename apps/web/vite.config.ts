import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import { TanStackRouterVite } from '@tanstack/router-plugin/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  // Bind a deterministic IPv4 address/port so tooling that probes
  // http://127.0.0.1:5173 (e.g. `hermes verify`) can reach the dev server.
  // By default Vite binds `localhost`, which on macOS resolves to IPv6 `::1`
  // first and is unreachable over IPv4.
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
  },
  plugins: [TanStackRouterVite(), react(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@vautr/api-contract': fileURLToPath(
        new URL('../../packages/api-contract/src/index.ts', import.meta.url),
      ),
      '@vautr/client-sdk/storage': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/storage.ts', import.meta.url),
      ),
      '@vautr/ui-logic': fileURLToPath(
        new URL('../../packages/ui-logic/src/index.ts', import.meta.url),
      ),
      '@vautr/client-sdk/real': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/realClient.ts', import.meta.url),
      ),
      '@vautr/client-sdk': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/index.ts', import.meta.url),
      ),
      // Real `vautr-wasm` --target web build (init + re-export). Points at the
      // wrapper in src/lib/wasm.ts so register/login/sync use real crypto
      // against the live server. mockWasm.ts is only the throwing dev shim.
      'vautr-wasm': fileURLToPath(new URL('./src/lib/wasm.ts', import.meta.url)),
    },
  },
});
