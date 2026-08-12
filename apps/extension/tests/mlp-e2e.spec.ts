import { createRequire } from 'node:module';
import { DatabaseSync } from 'node:sqlite';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { getExtensionId, launchExtensionContext } from './helpers';

/**
 * MLP E2E (Vault MLP §3): projects + secrets + reveal against the live server.
 *
 * Setup is driven directly against `http://127.0.0.1:8080` with the real
 * OPAQUE wasm (register -> login) so we control both users' sessions. The UI
 * assertions run through the real popup:
 *   - Owner reveals a secret  -> the value is shown.
 *   - A `can_view` member with NO `secrets:reveal` grant -> reveal is denied.
 */

const BASE = 'http://127.0.0.1:8080';
const SECRET_VALUE = 'mlp-super-secret-value-42';

const require = createRequire(import.meta.url);
const specRoot = dirname(fileURLToPath(import.meta.url));
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const wasm: any = require(join(specRoot, '..', 'wasm-pkg-nodejs/vautr_wasm.js'));

const b64 = (b: Uint8Array): string => Buffer.from(b).toString('base64');
const unb64 = (s: string): Uint8Array => new Uint8Array(Buffer.from(s, 'base64'));

async function req(
  method: string,
  path: string,
  body?: unknown,
  token?: string,
): Promise<{ status: number; body: any; text: string }> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(BASE + path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let parsed: any = null;
  try {
    parsed = text ? JSON.parse(text) : null;
  } catch {
    parsed = null;
  }
  return { status: res.status, body: parsed, text };
}

async function registerLogin(
  username: string,
  password: string,
): Promise<{ token: string; kdfSalt: string }> {
  const salt = wasm.generate_kdf_salt_js();
  const mk = wasm.derive_master_key_js(password, salt);
  const kek = wasm.derive_kek_js(mk);
  const svk = wasm.generate_svk_js();
  const svkWrapped = wasm.wrap_svk_js(svk, kek);
  const mnemonic = wasm.generate_recovery_mnemonic_js();
  const svkRkWrapped = wasm.wrap_svk_with_rk_js(svk, mnemonic);

  const rs = wasm.opaque_register_start_js(password);
  const sr = await req('POST', '/auth/register/start', {
    username,
    registration_start: b64(rs.message),
  });
  const upload = wasm.opaque_register_finish_js(
    rs.state,
    unb64(sr.body.registration_response),
    password,
    username,
  );
  await req('POST', '/auth/register/finish', {
    username,
    registration_finish: b64(upload),
    server_public_key: b64(new Uint8Array(32)),
    kdf_salt: b64(salt),
    svk_ciphertext_blob: b64(svkWrapped),
    svk_ciphertext_blob_rk: b64(svkRkWrapped),
  });

  const ls = wasm.opaque_login_start_js(password);
  const lr = await req('POST', '/auth/login/start', { username, login_start: b64(ls.message) });
  const lf = wasm.opaque_login_finish_js(
    ls.state,
    unb64(lr.body.login_response),
    password,
    username,
  );
  const fr = await req('POST', '/auth/login/finish', { username, login_finish: b64(lf.upload) });
  return { token: fr.body.session_token, kdfSalt: b64(salt) };
}

/**
 * Resolve a user's uuid by email. The server exposes no user-lookup endpoint,
 * so we read the uuid from its SQLite database (path via `VAUTR_DB_PATH`,
 * defaulting to `mlp.db` next to the server). Metadata-only: never secrets.
 */
function userUuidByEmail(dbPath: string, email: string): string {
  const db = new DatabaseSync(dbPath, { readOnly: true });
  try {
    const row = db.prepare('SELECT id FROM users WHERE email = ?').get(email) as
      | { id: string }
      | undefined;
    if (!row) throw new Error(`user not found in db: ${email}`);
    return row.id;
  } finally {
    db.close();
  }
}

// --- shared state populated by setup ---------------------------------------
let ownerUsername: string;
let viewerUsername: string;
let ownerToken: string;
let viewerToken: string;
let ownerKdfSalt: string;
let viewerKdfSalt: string;
let viewerUuid: string;
let projectUuid: string;
const PROJECT_NAME = 'mlp-shared';
const SECRET_KEY = 'API_KEY';
const PASSWORD = 'correct-horse-battery-staple-mlp';

test.describe
  .serial('MLP projects & secrets E2E', () => {
    test.beforeAll(async () => {
      const stamp = Date.now();
      ownerUsername = `owner-${stamp}@vautr.test`;
      viewerUsername = `viewer-${stamp}@vautr.test`;

      const owner = await registerLogin(ownerUsername, PASSWORD);
      const viewer = await registerLogin(viewerUsername, PASSWORD);
      ownerToken = owner.token;
      viewerToken = viewer.token;
      ownerKdfSalt = owner.kdfSalt;
      viewerKdfSalt = viewer.kdfSalt;
      const dbPath = process.env.VAUTR_DB_PATH ?? 'mlp.db';
      viewerUuid = userUuidByEmail(dbPath, viewerUsername);

      // Owner creates the project + secret.
      const proj = await req(
        'POST',
        '/projects',
        { name: PROJECT_NAME, type: 'shared' },
        ownerToken,
      );
      expect(proj.status).toBe(201);
      projectUuid = proj.body.uuid;

      const sec = await req(
        'POST',
        '/secrets',
        {
          project_uuid: projectUuid,
          key: SECRET_KEY,
          value_ciphertext: b64(Uint8Array.from(Buffer.from(SECRET_VALUE))),
        },
        ownerToken,
      );
      expect(sec.status).toBe(201);

      // Owner grants the viewer `can_view` (metadata only, NO secrets:reveal).
      const add = await req(
        'POST',
        `/projects/${projectUuid}/members`,
        { user_uuid: viewerUuid, role: 'member', permission: 'can_view' },
        ownerToken,
      );
      expect(add.status).toBe(201);

      // Sanity: viewer can list metadata but reveal is denied.
      const list = await req('GET', `/projects/${projectUuid}/secrets`, null, viewerToken);
      expect(list.status).toBe(200);
      expect(list.body.secrets[0].key).toBe(SECRET_KEY);
      const revealDenied = await req(
        'GET',
        `/secrets/${list.body.secrets[0].uuid}/value`,
        null,
        viewerToken,
      );
      expect(revealDenied.status).toBe(403);
    });

    async function loginViaUi(
      context: BrowserContext,
      extId: string,
      username: string,
      kdfSalt: string,
    ): Promise<Page> {
      const popup = await context.newPage();
      await popup.goto(`chrome-extension://${extId}/src/popup/popup.html`);
      await popup.waitForSelector('#auth-username');
      // Accounts are provisioned via HTTP, so the popup never ran its own register
      // and has no KDF salt in its IndexedDB state. Seed the salt (the one used at
      // register) so the real login flow can derive the master key, exactly as a
      // native register-then-login session would. (See IndexedDbStore in the SDK.)
      await popup.evaluate(async (salt) => {
        const open = indexedDB.open('vautr-client', 1);
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
          // Mirror IndexedDbStore.openDb: create both stores on first open so the
          // SDK's later transactions resolve.
          open.onupgradeneeded = () => {
            if (!open.result.objectStoreNames.contains('state'))
              open.result.createObjectStore('state');
            if (!open.result.objectStoreNames.contains('items'))
              open.result.createObjectStore('items', { keyPath: 'uuid' });
          };
          open.onsuccess = () => resolve(open.result);
          open.onerror = () => reject(open.error);
        });
        const tx = db.transaction('state', 'readwrite');
        tx.objectStore('state').put({ kdfSalt: salt }, 'root');
        await new Promise<void>((resolve, reject) => {
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      }, kdfSalt);
      await popup.fill('#auth-username', username);
      await popup.fill('#auth-password', PASSWORD);
      await popup.getByRole('button', { name: 'Unlock' }).click();
      await popup.getByText(username, { exact: true }).waitFor({ timeout: 30_000 });
      return popup;
    }

    async function openSecretsAndReveal(popup: Page, projectName: string): Promise<Page> {
      await popup.getByRole('tab', { name: 'Secrets' }).click();
      // Select the project via the shadcn Select.
      await popup.locator('[role="combobox"]').first().click();
      await popup.getByRole('option', { name: projectName }).click();
      await popup.getByText(SECRET_KEY, { exact: true }).waitFor({ timeout: 15_000 });
      await popup.getByRole('button', { name: 'Reveal' }).click();
      return popup;
    }

    test('owner reveals a secret and sees the plaintext value', async () => {
      const profileDir = mkdtempSync(join(tmpdir(), 'vautr-mlp-owner-'));
      const context = await launchExtensionContext({ profileDir });
      try {
        const extId = await getExtensionId(context);
        const popup = await loginViaUi(context, extId, ownerUsername, ownerKdfSalt);
        await openSecretsAndReveal(popup, PROJECT_NAME);
        await popup.getByText(SECRET_VALUE, { exact: true }).waitFor({ timeout: 15_000 });
        await expect(popup.getByText(SECRET_VALUE, { exact: true })).toBeVisible();
      } finally {
        await context.close();
        rmSync(profileDir, { recursive: true, force: true });
      }
    });

    test('a can_view member without a secrets:reveal grant is denied', async () => {
      const profileDir = mkdtempSync(join(tmpdir(), 'vautr-mlp-viewer-'));
      const context = await launchExtensionContext({ profileDir });
      try {
        const extId = await getExtensionId(context);
        const popup = await loginViaUi(context, extId, viewerUsername, viewerKdfSalt);
        await openSecretsAndReveal(popup, PROJECT_NAME);
        // "Reveal denied" is rendered both inline (Secrets tab) and as a toast;
        // use .first() to avoid a strict-mode clash.
        await popup
          .getByText(/Reveal denied/i)
          .first()
          .waitFor({ timeout: 15_000 });
        await expect(popup.getByText(/Reveal denied/i).first()).toBeVisible();
        await expect(popup.getByText(SECRET_VALUE, { exact: true })).not.toBeVisible();
      } finally {
        await context.close();
        rmSync(profileDir, { recursive: true, force: true });
      }
    });
  });
