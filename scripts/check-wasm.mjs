#!/usr/bin/env node
// Fail the build if a required prebuilt wasm artifact is missing.
// The clients must ship real crypto; a missing .wasm means the throwing dev
// shim would be the effective path. Run via the `verify:wasm` npm script.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const required = [
  'apps/web/wasm-pkg/vautr_wasm_bg.wasm',
  'apps/extension/wasm-pkg/vautr_wasm_bg.wasm',
  'apps/extension/sw-wasm-pkg-nodejs/vautr_crypto_wasm_bg.wasm',
];

const missing = required.filter((p) => !existsSync(resolve(root, p)));
if (missing.length) {
  console.error('Missing prebuilt WASM artifacts:');
  for (const m of missing) console.error(`  - ${m}`);
  console.error('\nRun `pnpm prepare:wasm` (scripts/prepare-wasm.sh) to build them.');
  process.exit(1);
}
console.log('WASM artifacts present.');
