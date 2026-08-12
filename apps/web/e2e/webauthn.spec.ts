import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';

/**
 * WebAuthn (FIDO2) optional second factor for MP unlock (VTR-052).
 *
 * Driven with a CDP virtual authenticator against the server's `/webauthn/*`
 * endpoints and `/account/status`. The page runs at the Relying-Party origin
 * (http://localhost:5173) so `navigator.credentials` produces assertions the
 * server accepts.
 *
 * Each test provisions its own user + session token directly in the DB, so the
 * server's in-memory 2FA-verified set, per-user credentials, and registration
 * exclude-lists stay isolated between tests.
 *
 * Cases covered (VTR-052 TDD):
 *   a. register a security key -> succeeds.
 *   b. MP unlock + WebAuthn assertion -> succeeds; /account/status un-gated.
 *   c. assertion fails/cancelled -> /account/status stays gated.
 *   d. disable second factor -> server removes the credential.
 *   e. multiple registered keys per user (backup key).
 */

const SERVER = 'http://localhost:8080';
const ORIGIN = 'http://localhost:5173';

const state = JSON.parse(readFileSync(join(tmpdir(), 'vautr-webauthn-e2e-state.json'), 'utf8')) as {
  dbPath: string;
  sessionToken: string;
  userId: string;
};

type Session = { token: string; userId: string; headers: Record<string, string> };

/** Provision a brand-new user + session token directly in the DB. */
function freshSession(): Session {
  const db = new DatabaseSync(state.dbPath);
  const now = Date.now();
  const userId = 'e2e-user-' + Math.random().toString(36).slice(2, 10);
  const token = 'e2e-tok-' + Math.random().toString(36).slice(2, 14);
  db.prepare(
    `INSERT INTO users (id, email, kdf_salt, opaque_record, svk_ciphertext_blob,
       svk_ciphertext_blob_rk, min_enc_key_gen, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)`,
  ).run(
    userId,
    `${userId}@vautr.test`,
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
  ).run(token, userId, now + 86_400_000, now);
  db.close();
  return {
    token,
    userId,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
  };
}

/** base64url / options-coercion helpers injected into every page before navigation. */
function injectHelpers(page: Page) {
  return page.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    w.b64url = (buf: ArrayBuffer) => {
      const bytes = new Uint8Array(buf);
      let s = '';
      for (const b of bytes) s += String.fromCharCode(b);
      return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
    };
    w.b64ToBuf = (s: string) => {
      const bin = atob(s.replace(/-/g, '+').replace(/_/g, '/'));
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      return bytes.buffer;
    };
    // Convert the webauthn-rs JSON challenge into the browser's
    // CredentialCreationOptions (challenge / user.id / excluded cred ids).
    // We drop the requested extensions: webauthn-rs asks for a UV-required
    // credProtect policy that Chrome flags as inconsistent with its own
    // `userVerification: preferred` selection. The extensions are optional for
    // the ceremony, so omitting them keeps registration working.
    w.fixCreate = (o: { publicKey: any }) => {
      const p = o.publicKey;
      p.challenge = (w.b64ToBuf as (s: string) => ArrayBuffer)(p.challenge);
      p.user.id = (w.b64ToBuf as (s: string) => ArrayBuffer)(p.user.id);
      if (p.excludeCredentials) {
        p.excludeCredentials = p.excludeCredentials.map((c: any) => ({
          ...c,
          id: (w.b64ToBuf as (s: string) => ArrayBuffer)(c.id),
        }));
      }
      delete p.extensions;
      return o;
    };
    // Convert the webauthn-rs JSON challenge into CredentialRequestOptions.
    w.fixGet = (o: { publicKey: any }) => {
      const p = o.publicKey;
      p.challenge = (w.b64ToBuf as (s: string) => ArrayBuffer)(p.challenge);
      if (p.allowCredentials) {
        p.allowCredentials = p.allowCredentials.map((c: any) => ({
          ...c,
          id: (w.b64ToBuf as (s: string) => ArrayBuffer)(c.id),
        }));
      }
      return o;
    };
  });
}

async function addAuthenticator(
  context: BrowserContext,
  page: Page,
  transport: 'internal' | 'usb' = 'internal',
) {
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport,
      hasResidentKey: false,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  return { cdp, authenticatorId: authenticatorId as string };
}

/** Register a fresh credential on the page's virtual authenticator. */
async function registerKey(page: Page, session: Session, label: string): Promise<void> {
  const r = await page.evaluate(
    ({ server, headers, label }) =>
      (async () => {
        const b64 = (buf: ArrayBuffer) =>
          (window as unknown as { b64url: (b: ArrayBuffer) => string }).b64url(buf);
        const start = await fetch(`${server}/webauthn/register/start`, {
          method: 'POST',
          headers,
          body: JSON.stringify({ label }),
        });
        if (!start.ok) throw new Error('register/start ' + start.status);
        const startJson = await start.json();
        const cred = (await navigator.credentials.create(
          (window as unknown as { fixCreate: (o: unknown) => unknown }).fixCreate(
            startJson.challenge,
          ) as CredentialCreationOptions,
        )) as unknown as {
          id: string;
          rawId: ArrayBuffer;
          type: string;
          response: Record<string, ArrayBuffer | null>;
        } | null;
        if (!cred) throw new Error('registration cancelled');
        const att = cred.response as unknown as AuthenticatorAttestationResponse;
        const response = {
          attestationObject: b64(att.attestationObject),
          clientDataJSON: b64(att.clientDataJSON),
        };
        const verify = await fetch(`${server}/webauthn/register/verify`, {
          method: 'POST',
          headers,
          body: JSON.stringify({
            request_id: startJson.request_id,
            credential: { id: cred.id, rawId: b64(cred.rawId), type: cred.type, response },
          }),
        });
        if (!verify.ok) {
          const body = await verify.text();
          throw new Error('register/verify ' + verify.status + ': ' + body);
        }
        return true;
      })(),
    { server: SERVER, headers: session.headers, label },
  );
  expect(r).toBe(true);
}

/** Complete a WebAuthn assertion; rejects if cancelled or the key is invalid. */
async function assertSecondFactor(page: Page, session: Session): Promise<void> {
  const r = await page.evaluate(
    ({ server, headers }) =>
      (async () => {
        const b64 = (buf: ArrayBuffer) =>
          (window as unknown as { b64url: (b: ArrayBuffer) => string }).b64url(buf);
        const start = await fetch(`${server}/webauthn/assert/start`, {
          method: 'POST',
          headers,
        });
        if (!start.ok) throw new Error('assert/start ' + start.status);
        const startJson = await start.json();
        const cred = (await navigator.credentials.get(
          (window as unknown as { fixGet: (o: unknown) => unknown }).fixGet(
            startJson.challenge,
          ) as CredentialRequestOptions,
        )) as unknown as {
          id: string;
          rawId: ArrayBuffer;
          type: string;
          response: Record<string, ArrayBuffer | null>;
        } | null;
        if (!cred) throw new Error('assertion cancelled');
        const att = cred.response as unknown as AuthenticatorAssertionResponse;
        const response: Record<string, string | null> = {
          authenticatorData: b64(att.authenticatorData),
          clientDataJSON: b64(att.clientDataJSON),
          signature: b64(att.signature),
        };
        if (att.userHandle) response.userHandle = b64(att.userHandle);
        const verify = await fetch(`${server}/webauthn/assert/verify`, {
          method: 'POST',
          headers,
          body: JSON.stringify({
            request_id: startJson.request_id,
            credential: { id: cred.id, rawId: b64(cred.rawId), type: cred.type, response },
          }),
        });
        if (!verify.ok) throw new Error('assert/verify ' + verify.status);
        return true;
      })(),
    { server: SERVER, headers: session.headers },
  );
  expect(r).toBe(true);
}

/** GET /account/status -> { required, hasSvk }. */
async function accountStatus(
  page: Page,
  session: Session,
): Promise<{ required: boolean; hasSvk: boolean }> {
  return page.evaluate(
    ({ server, headers }) =>
      fetch(`${server}/account/status`, { headers }).then(async (res) => {
        const body = await res.json();
        return {
          required: !!body.second_factor_required,
          hasSvk: !!(body.svk_ciphertext_blob && body.svk_ciphertext_blob.length > 0),
        };
      }),
    { server: SERVER, headers: session.headers },
  );
}

/** GET /webauthn/credentials -> { credentials: [{ cred_id, label }] }. */
async function listCredentials(
  page: Page,
  session: Session,
): Promise<{ cred_id: string; label: string }[]> {
  const body = await page.evaluate(
    ({ server, headers }) =>
      fetch(`${server}/webauthn/credentials`, { headers }).then((r) => r.json()),
    { server: SERVER, headers: session.headers },
  );
  return body.credentials ?? [];
}

test.beforeEach(async ({ context, page }) => {
  await injectHelpers(page);
  await page.goto(ORIGIN + '/');
  const { authenticatorId } = await addAuthenticator(context, page);
  (page as unknown as { __authId: string }).__authId = authenticatorId;
});

test('a: register a security key succeeds', async ({ page }) => {
  const session = freshSession();
  await registerKey(page, session, 'E2E YubiKey');
});

test('b: MP unlock + assertion un-gates account status', async ({ page }) => {
  const session = freshSession();
  await registerKey(page, session, 'Primary');

  const gated = await accountStatus(page, session);
  expect(gated.required).toBe(true);
  expect(gated.hasSvk).toBe(false);

  await assertSecondFactor(page, session);

  const open = await accountStatus(page, session);
  expect(open.required).toBe(false);
  expect(open.hasSvk).toBe(true);
});

test('c: failed/cancelled assertion keeps status gated', async ({ context, page }) => {
  const session = freshSession();
  await registerKey(page, session, 'CancelTarget');

  const gated = await accountStatus(page, session);
  expect(gated.required).toBe(true);

  // Swap in a fresh authenticator holding no credentials, so the next assertion
  // rejects deterministically ("no matching credential") instead of hanging.
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.removeVirtualAuthenticator', {
    authenticatorId: (page as unknown as { __authId: string }).__authId,
  });
  await addAuthenticator(context, page);

  await expect(
    assertSecondFactor(page, session).catch(() => {
      throw new Error('assertion should have been cancelled');
    }),
  ).rejects.toThrow();

  const stillGated = await accountStatus(page, session);
  expect(stillGated.required).toBe(true);
  expect(stillGated.hasSvk).toBe(false);
});

test('d: disabling second factor removes the credential', async ({ page }) => {
  const session = freshSession();
  await registerKey(page, session, 'DisableMe');
  const list = await listCredentials(page, session);
  expect(list.length).toBe(1);
  const credId = list[0]!.cred_id;

  const del = await page.evaluate(
    ({ server, headers, credId }) =>
      fetch(`${server}/webauthn/credentials/${encodeURIComponent(credId)}`, {
        method: 'DELETE',
        headers,
      }).then((r) => r.status),
    { server: SERVER, headers: session.headers, credId },
  );
  expect(del).toBe(200);

  const open = await accountStatus(page, session);
  expect(open.required).toBe(false);
  expect(open.hasSvk).toBe(true);
});

test('e: multiple registered keys per user', async ({ context, page }) => {
  const session = freshSession();
  await registerKey(page, session, 'Key A');

  // A second key needs a distinct authenticator. Chrome allows only one
  // `internal` authenticator, so swap in a `usb` one for the backup key.
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.removeVirtualAuthenticator', {
    authenticatorId: (page as unknown as { __authId: string }).__authId,
  });
  await addAuthenticator(context, page, 'usb');
  await registerKey(page, session, 'Key B');

  const creds = await listCredentials(page, session);
  expect(creds.length).toBeGreaterThanOrEqual(2);

  await assertSecondFactor(page, session);
  const open = await accountStatus(page, session);
  expect(open.required).toBe(false);
  expect(open.hasSvk).toBe(true);
});
