import { spawn, type ChildProcess } from 'node:child_process';
import { execSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

/**
 * E2E setup for the WebAuthn (FIDO2) second-factor spec (VTR-052).
 *
 * Runs AFTER Playwright's webServers (the static RP-origin server on :5173) and
 * BEFORE the tests: spawns the Vautr server binary (built with the `webauthn`
 * feature) against a fresh temp DB, waits until it accepts connections (sqlx
 * migrations have run), then seeds a user + a far-future session token via a
 * second SQLite connection (WAL allows this while the server pool is open). The
 * child PID is recorded so `globalTeardown` can stop it.
 *
 * We spawn the compiled binary directly (not `cargo run`) so the recorded PID is
 * the server process itself and no orphan survives when the parent is killed.
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '../../..');
const BIN = join(REPO, 'target/debug/vautr-server');

const SESSION_TOKEN = 'e2e-webauthn-token';
const USER_ID = 'e2e-user-0001';
const DB_PATH = join(tmpdir(), `vautr-webauthn-e2e-${Date.now()}.db`);
const STATE_PATH = join(tmpdir(), 'vautr-webauthn-e2e-state.json');
const SERVER = 'http://localhost:8080';

let child: ChildProcess | undefined;

export default async function globalSetup(): Promise<void> {
  rmSync(DB_PATH, { force: true });
  mkdirSync(dirname(DB_PATH), { recursive: true });

  // Clear any stale server from an aborted previous run, then make sure the
  // webauthn-featured binary is up to date.
  try {
    execSync('pkill -f "target/debug/vautr-server"', { stdio: 'ignore' });
  } catch {
    // nothing to kill
  }
  execSync('cargo build -p vautr-server --features webauthn', {
    cwd: REPO,
    stdio: 'inherit',
  });

  child = spawn(BIN, [], {
    cwd: REPO,
    env: {
      ...process.env,
      VAUTR_DB_URL: `sqlite:${DB_PATH}`,
      VAUTR_WEBAUTHN_RP_ID: 'localhost',
      VAUTR_WEBAUTHN_ORIGIN: 'http://localhost:5173',
    },
    stdio: 'ignore',
  });
  child.on('exit', (code) => {
    console.error(`[e2e] vautr-server exited unexpectedly with code ${code}`);
  });

  // Wait until the server accepts connections. The unauthenticated /webauthn/status
  // responds 401 (schema migrated), which counts as "ready" for the health probe.
  await waitForServer();

  // Seed the user + session after migrations have created the schema.
  const db = new DatabaseSync(DB_PATH);
  const now = Date.now();
  db.prepare(
    `INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob,
       svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)`,
  ).run(
    USER_ID, 'e2e@vautr.test', new Uint8Array(32), new Uint8Array(16),
    new Uint8Array(48), new Uint8Array(48), now, now,
  );
  db.prepare(
    `INSERT INTO sessions (token, user_id, expires_at, created_at)
     VALUES (?, ?, ?, ?)`,
  ).run(SESSION_TOKEN, USER_ID, now + 86_400_000, now);
  db.close();

  writeFileSync(
    STATE_PATH,
    JSON.stringify({
      dbPath: DB_PATH,
      sessionToken: SESSION_TOKEN,
      userId: USER_ID,
      pid: child.pid,
    }),
  );
  console.log('[e2e] seeded user + session; vautr-server ready');
}

async function waitForServer(): Promise<void> {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (child?.exitCode !== null && child?.exitCode !== undefined) {
      throw new Error(`[e2e] vautr-server exited during startup (code ${child.exitCode})`);
    }
    try {
      await fetch(`${SERVER}/webauthn/status`);
      return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('[e2e] timed out waiting for vautr-server to become ready');
}
