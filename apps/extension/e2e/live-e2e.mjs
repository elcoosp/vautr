/**
 * E2E Integration Test: Real Register/Login/Add → Stateless SW Decrypt
 *
 * Uses the live Vautr server at http://localhost:8080 and the real
 * nodejs-target vautr-wasm + vautr-crypto-wasm modules.
 *
 * Flow:
 *   1. Register a new user via the OPAQUE protocol
 *   2. Login and get a session token
 *   3. Recover the SVK from the server
 *   4. Encrypt and push a new item
 *   5. Decrypt it statelessly with decrypt_secret_with_svk (SW wasm)
 *   6. Verify the password matches
 *
 * Run from repo root:
 *   node apps/extension/e2e/live-e2e.mjs
 */

import * as nodeCrypto from 'node:crypto';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

// Load nodejs-target wasm modules (CJS, require('fs') for .wasm bytes).
const wasmFull = require('../wasm-pkg-nodejs/vautr_wasm.js');
const wasmSw = require('../sw-wasm-pkg-nodejs/vautr_crypto_wasm.js');

const BASE = 'http://localhost:8080';

function log(msg) {
  console.log(`[${new Date().toISOString()}] ${msg}`);
}

// ---- Helpers ----

function btoaBytes(bytes) {
  return Buffer.from(bytes).toString('base64');
}

function atobBytes(b64) {
  return Uint8Array.from(Buffer.from(b64, 'base64'));
}

function randomUUID() {
  return nodeCrypto.randomUUID();
}

async function api(method, path, body, token) {
  const headers = {
    'Content-Type': 'application/json',
    'X-Request-ID': randomUUID(),
  };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const init = { method, headers };
  if (body !== undefined) init.body = JSON.stringify(body);
  const res = await fetch(`${BASE}${path}`, init);
  const text = await res.text();
  const data = text ? JSON.parse(text) : {};
  if (!res.ok) throw new Error(`${method} ${path}: ${res.status} ${JSON.stringify(data)}`);
  return data;
}

// ---- Main flow ----

async function main() {
  const username = `e2e-${Date.now()}@vautr.test`;
  const password = 'correct-horse-battery-staple-e2e';
  const itemTitle = 'E2E Test';
  const itemUser = 'e2e@example.com';
  const itemPass = 's3cret-e2e-pw-42!';

  log(`Starting E2E with user: ${username}`);

  // ---- 1. REGISTER ----
  log('1. Registering...');
  const salt = nodeCrypto.randomBytes(32);
  const mk = wasmFull.derive_master_key_js(password, salt);
  const kek = wasmFull.derive_kek_js(mk);
  const svk = wasmFull.generate_svk_js();
  const svkWrapped = wasmFull.wrap_svk_js(svk, kek);
  const mnemonic = wasmFull.generate_recovery_mnemonic_js();
  const svkRkWrapped = wasmFull.wrap_svk_with_rk_js(svk, mnemonic);

  const regStart = wasmFull.opaque_register_start_js(password);
  const regStartResp = await api('POST', '/auth/register/start', {
    username,
    registration_start: btoaBytes(regStart.message),
  });
  const regUpload = wasmFull.opaque_register_finish_js(
    regStart.state,
    atobBytes(regStartResp.registration_response),
    password,
    username,
  );
  await api('POST', '/auth/register/finish', {
    username,
    registration_finish: btoaBytes(regUpload),
    server_public_key: btoaBytes(new Uint8Array(32)),
    kdf_salt: btoaBytes(salt),
    svk_ciphertext_blob: btoaBytes(svkWrapped),
    svk_ciphertext_blob_rk: btoaBytes(svkRkWrapped),
  });
  log('  Registered.');

  // ---- 2. LOGIN ----
  log('2. Logging in...');
  const loginStart = wasmFull.opaque_login_start_js(password);
  const loginStartResp = await api('POST', '/auth/login/start', {
    username,
    login_start: btoaBytes(loginStart.message),
  });
  const loginFinish = wasmFull.opaque_login_finish_js(
    loginStart.state,
    atobBytes(loginStartResp.login_response),
    password,
    username,
  );
  const loginFinishResp = await api('POST', '/auth/login/finish', {
    username,
    login_finish: btoaBytes(loginFinish.upload),
  });
  const token = loginFinishResp.session_token;
  log('  Logged in. Token obtained.');

  // ---- 3. GET ACCOUNT STATUS (recover SVK) ----
  log('3. Getting account status...');
  const status = await api('GET', '/account/status', undefined, token);
  const recoveredSvk = wasmFull.unwrap_svk_js(atobBytes(status.svk_ciphertext_blob), kek);
  log(`  SVK recovered (${recoveredSvk.length} bytes).`);

  // ---- 4. ADD ITEM ----
  log('4. Adding item...');
  const itemUuid = randomUUID();
  const dek = wasmFull.derive_dek_js(recoveredSvk);
  const plaintext = JSON.stringify({
    title: itemTitle,
    subtitle: itemUser,
    iconKey: 'key',
    urls: ['https://example.com'],
    password: itemPass,
  });
  const ciphertext = wasmFull.encrypt_item_js(
    itemUuid,
    1n,
    dek,
    new TextEncoder().encode(plaintext),
  );
  const payloadB64 = btoaBytes(ciphertext);

  const pushResp = await api('POST', '/sync/push-batch', {
    items: [{
      uuid: itemUuid,
      target_version: 0,
      enc_key_gen: 1,
      payload: payloadB64,
      deleted_date: null,
    }],
  }, token);
  log(`  Push result: ${JSON.stringify(pushResp.results[0])}`);

  // ---- 5. STATELESS DECRYPT VIA SW WASM ----
  log('5. Stateless decrypt via SW wasm...');
  const decrypted = wasmSw.decrypt_secret_with_svk(
    recoveredSvk,
    itemUuid,
    1n, // bigint for u64
    ciphertext,
  );
  log(`  Decrypted password: "${decrypted}"`);

  // ---- 6. VERIFY ----
  if (decrypted !== itemPass) {
    throw new Error(`PASSWORD MISMATCH: expected "${itemPass}", got "${decrypted}"`);
  }
  log('✅ PASS: Stateless SW decrypt matches original password.');

  // Also verify via full wasm decrypt_item_js
  const decryptedFull = wasmFull.decrypt_item_js(itemUuid, 1n, dek, new Uint8Array(ciphertext));
  const parsed = JSON.parse(new TextDecoder().decode(decryptedFull));
  if (parsed.password !== itemPass) {
    throw new Error(`Full decrypt mismatch: ${parsed.password}`);
  }
  log('✅ PASS: Full item decrypt also matches.');

  log('🎉 ALL E2E TESTS PASSED');
  process.exit(0);
}

main().catch((err) => {
  console.error(`❌ E2E FAILED: ${err.message}`);
  console.error(err);
  process.exit(1);
});
