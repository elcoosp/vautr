import { describe, expect, it } from 'vitest';

import { MobileApiClient } from '../../lib/api';
import { createMemoryTokenStore, VautrAuth } from '../../lib/auth';
import { ApiError } from '../../lib/http';

/**
 * Live-server integration test (Wave B5 gate).
 *
 * Requires a Vautr MLP server on `VAUTR_API_URL` (default http://localhost:8080)
 * with a fresh DB. Exercises the full mobile flow end-to-end over the real HTTP
 * transport + vendored OPAQUE wasm:
 *
 *   register -> login -> create project -> create secret -> reveal -> deny
 *
 * The deny assertion verifies that a caller who does not hold the reveal
 * authorization on a project is rejected: owner reveal succeeds, while a second,
 * unrelated user is denied by the server (403/404). This matches the mobile
 * `reveal` gating contract (api.md §4 secrets).
 */
const BASE = process.env.VAUTR_API_URL ?? 'http://localhost:8080';
const TEST_TIMEOUT = 30_000;

function randomName(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// base64 of "hello-mobile" (the server stores ciphertext as opaque base64).
const SECRET_VALUE_B64 = 'aGVsbG8tbW9iaWxl';

/** Register + login a throwaway user and return the authenticated client. */
async function registerUser(tag: string): Promise<MobileApiClient> {
  const api = new MobileApiClient({ baseUrl: BASE });
  const auth = new VautrAuth({ api, store: createMemoryTokenStore() });
  const username = randomName(tag);
  const password = `pw-${Math.random().toString(36).slice(2)}-Xy7!`;
  await auth.register(username, password);
  return api;
}

describe('mobile live-server integration (Wave B5 gate)', () => {
  it(
    'register → login → create project → create+reveal secret, and denies reveal without authorization',
    async () => {
      // ── User A: owner, can reveal ──────────────────────────────────────
      const apiA = await registerUser('uA');

      const project = await apiA.createProject({
        name: randomName('vault'),
        type: 'personal',
        description: 'Wave B5 integration test vault',
      });

      const secret = await apiA.createSecret({
        project_uuid: project.uuid,
        key: 'DATABASE_URL',
        value_ciphertext: SECRET_VALUE_B64,
      });

      // Owner reveal succeeds and returns the stored ciphertext.
      const revealed = await apiA.revealSecret(secret.uuid);
      expect(revealed.value_ciphertext).toBe(SECRET_VALUE_B64);
      expect(revealed.uuid).toBe(secret.uuid);

      // ── User B: authenticated but holds no access to A's project ──────
      const apiB = await registerUser('uB');

      // B has no reveal authorization on the secret → the server denies it.
      let denied = false;
      try {
        await apiB.revealSecret(secret.uuid);
      } catch (err) {
        denied = err instanceof ApiError && err.status >= 400;
      }
      expect(denied).toBe(true);

      // Owner can still reveal after the denied attempts.
      const again = await apiA.revealSecret(secret.uuid);
      expect(again.value_ciphertext).toBe(SECRET_VALUE_B64);
    },
    TEST_TIMEOUT,
  );
});
