import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
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
