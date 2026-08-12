/**
 * E2E test: register → login → add item → stateless SW decrypt.
 *
 * Exercises the live Vautr server at localhost:8080 using the real
 * VautrWebClient (for auth + sync + add) and the stateless SW wasm
 * module (for independent decrypt verification).
 *
 * Run from repo root:
 *   npx tsx apps/extension/e2e/live-e2e.test.ts
 */

// Polyfill IndexedDB for Node.js (VautrWebClient requires it).
import 'fake-indexeddb/auto';
import * as nodeCrypto from 'node:crypto';

// Ensure crypto.randomUUID is available in Node.
if (!globalThis.crypto) {
  (globalThis.crypto as any) = nodeCrypto;
}
if (!(globalThis.crypto as any).randomUUID) {
  (globalThis.crypto as any).randomUUID = () => nodeCrypto.randomUUID();
}

import { VautrWebClient } from '../../packages/vautr-client-sdk/src/realClient';
import { createStatelessCrypto } from '../../packages/vautr-client-sdk/src/extension';
import { IndexedDbStore, fromBase64 } from '../../packages/vautr-client-sdk/src/storage';

const BASE_URL = 'http://localhost:8080';

function now(): string {
  return new Date().toISOString();
}

function log(msg: string): void {
  console.log(`[${now()}] ${msg}`);
}

async function main(): Promise<void> {
  const username = `e2e-${Date.now()}@vautr.test`;
  const password = 'correct-horse-battery-staple-e2e';
  const itemTitle = 'E2E Test Item';
  const itemUser = 'e2e-user@example.com';
  const itemPass = 's3cret-e2e-password-42!';

  log(`Starting E2E test with user: ${username}`);

  // 1. Register
  log('Step 1: Registering...');
  const store = new IndexedDbStore();
  const client = new VautrWebClient({ baseUrl: BASE_URL, store });
  await client.register(username, password);
  log('  Registered.');

  // 2. Login
  log('Step 2: Logging in...');
  await client.login(username, password);
  log('  Logged in.');

  // 3. Sync
  log('Step 3: Syncing...');
  await client.sync();
  log('  Synced.');

  // 4. Add an item
  log('Step 4: Adding item...');
  const overview = await client.addItem({
    title: itemTitle,
    username: itemUser,
    password: itemPass,
    url: 'https://example.com',
  });
  log(`  Added: ${overview.uuid}`);

  // 5. Push to server
  log('Step 5: Pushing to server...');
  await client.sync();
  log('  Pushed.');

  // 6. Get the SVK from the store for stateless decrypt
  const state = await store.getState();
  const svk = state.svk;
  if (!svk) {
    throw new Error('SVK not found in store after login');
  }
  log(`  SVK retrieved (${svk.length} bytes).`);

  // 7. Get the item payload from the store
  const item = await store.getItem(overview.uuid);
  if (!item || !item.payload) {
    throw new Error('Item not found or missing payload');
  }
  log(`  Item payload retrieved (${item.payload.length} chars base64).`);

  // 8. Stateless decrypt using the SW wasm module
  log('Step 6: Stateless decrypt via SW wasm...');
  const swCrypto = createStatelessCrypto();
  const decryptedPassword = await swCrypto.decryptSecret({
    svk,
    uuid: overview.uuid,
    encKeyGen: item.encKeyGen,
    payload: fromBase64(item.payload),
  });

  log(`  Decrypted password: "${decryptedPassword}"`);

  // 9. Verify
  if (decryptedPassword !== itemPass) {
    throw new Error(`PASSWORD MISMATCH: expected "${itemPass}", got "${decryptedPassword}"`);
  }
  log('✅ PASS: Stateless SW decrypt matches original password.');

  // 10. Also verify via client.reveal (opaque handle path)
  log('Step 7: Opaque-handle reveal...');
  const handle = await client.reveal(overview.uuid);
  log(`  Handle: ${handle}`);
  await client.performAction({ type: 'CopyToClipboard', handle });
  log('  performAction succeeded.');
  await client.release(handle);
  log('  Handle released.');

  log('✅ ALL E2E TESTS PASSED');
  await client.forget();
  process.exit(0);
}

main().catch((err) => {
  console.error(`❌ E2E FAILED: ${err instanceof Error ? err.message : String(err)}`);
  console.error(err);
  process.exit(1);
});
