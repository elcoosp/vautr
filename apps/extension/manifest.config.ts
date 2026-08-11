import { defineManifest } from '@crxjs/vite-plugin';

/**
 * Manifest V3 extension manifest (build-env-deploy §3.3).
 *
 * Architecture split:
 *  - Popup: full `VautrClient` in a popup-scoped Web Worker (`--target web`).
 *  - Service Worker: 100% stateless autofill. Wakes, runs ONE action, ends.
 *    Uses the `--target nodejs` crypto WASM. NEVER opens SQLite.
 *
 * API URL is injected at runtime via `chrome.storage.local` (§5).
 */
export default defineManifest({
  manifest_version: 3,
  name: 'Vautr',
  version: '0.1.0',
  description: 'Stateless Vautr autofill',
  action: {
    default_title: 'Vautr',
    default_popup: 'src/popup/popup.html',
  },
  background: {
    service_worker: 'src/background/serviceWorker.ts',
    type: 'module',
  },
  content_scripts: [
    {
      matches: ['<all_urls>'],
      js: ['src/content/autofill.ts'],
      run_at: 'document_idle',
    },
  ],
  permissions: ['storage', 'activeTab', 'scripting', 'clipboardWrite'],
  host_permissions: ['<all_urls>'],
  // The real wasm-bindgen crypto needs `WebAssembly.instantiateStreaming`, which
  // the default MV3 CSP forbids. Allow wasm so the stateless SW can decrypt.
  content_security_policy: {
    extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'self';",
  },
});
