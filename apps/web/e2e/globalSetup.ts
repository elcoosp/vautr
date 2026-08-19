import { writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';

/**
 * E2E setup for the WebAuthn (FIDO2) second-factor spec (VTR-052).
 *
 * The Vautr server itself is now managed by Playwright as a `webServer` entry
 * (built with the `webauthn` feature, started before tests, kept alive for the
 * whole run). This setup only seeds a user + session into that already-running
 * server's DB and records the DB path / session token for the specs.
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '../../..');

const SESSION_TOKEN = 'e2e-webauthn-token';
const USER_ID = 'e2e-user-0001';
// NOTE: must match the webServer env VAUTR_DB_URL in playwright.config.ts exactly.
// We use a literal `/tmp` path (not os.tmpdir(), which on macOS resolves to a
// per-user /var/folders path that would not match the server's DB file).
const DB_PATH = '/tmp/vautr-webauthn-e2e.db';
const STATE_PATH = join(tmpdir(), 'vautr-webauthn-e2e-state.json');
const SERVER = 'http://localhost:8080';

export default async function globalSetup(): Promise<void> {
  // Wait until the managed server (webServer entry) accepts connections. The
  // unauthenticated /webauthn/status responds 401 (schema migrated), which
  // counts as "ready".
  await waitForServer();

  // Seed the user + session. The server runs migrations asynchronously, so the
  // `users` table may not exist the instant globalSetup runs. Poll sqlite_master
  // (which is always present) until the `users` table is confirmed, then seed on
  // that same confirmed connection.
  const now = Date.now();
  const deadline = Date.now() + 90_000;

  let seeded = false;
  while (Date.now() < deadline) {
    let db: DatabaseSync | undefined;
    try {
      db = new DatabaseSync(DB_PATH);
      const row = db
        .prepare(`SELECT name FROM sqlite_master WHERE type='table' AND name='users'`)
        .get() as { name?: string } | undefined;
      if (row?.name === 'users') {
        db.prepare(
          `INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob,
             svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)`,
        ).run(
          USER_ID,
          'e2e@vautr.test',
          new Uint8Array(32),
          new Uint8Array(16),
          new Uint8Array(48),
          new Uint8Array(48),
          now,
          now,
        );
        db.prepare(
          `INSERT INTO sessions (token, user_id, expires_at, created_at)
           VALUES (?, ?, ?, ?)`,
        ).run(SESSION_TOKEN, USER_ID, now + 86_400_000, now);
        seeded = true;
        break;
      }
    } catch {
      // table not readable yet
    } finally {
      db?.close();
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  if (!seeded) throw new Error('[e2e] timed out waiting for users table to be created');

  writeFileSync(
    STATE_PATH,
    JSON.stringify({
      dbPath: DB_PATH,
      sessionToken: SESSION_TOKEN,
      userId: USER_ID,
    }),
  );
  console.log('[e2e] seeded user + session; vautr-server ready');
}

async function waitForServer(): Promise<void> {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    try {
      // 401 from the gated /webauthn/status means the schema has migrated and
      // the endpoint is live (vs. a 500 / connection-refused while still booting).
      const res = await fetch(`${SERVER}/webauthn/status`);
      if (res.status === 401) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('[e2e] timed out waiting for vautr-server to become ready');
}
