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
      '@vautr/client-sdk': fileURLToPath(
        new URL('../../packages/vautr-client-sdk/src/index.ts', import.meta.url),
      ),
      'vautr-wasm': fileURLToPath(new URL('./src/lib/mockWasm.ts', import.meta.url)),
    },
  },
});
