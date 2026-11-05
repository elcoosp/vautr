/**
 * Pact consumer contract test for `POST /sync/push-batch` (VTR-037).
 *
 * Defines the request/response contract the TypeScript SDK (`@vautr/client-sdk`,
 * see `src/realClient.ts` `push-batch` call) expects from the Rust server
 * (`vautr-server`). The shapes below mirror the SDK's `PushBatchItem` /
 * `PushBatchResult` interfaces exactly, so any server-side drift in the
 * push-batch API breaks the provider verification step and fails CI.
 *
 * The interaction mirrors the real SDK call (VTR-CI): the client always sends
 * its OPAQUE session token as `Authorization: Bearer ...`, so the contract
 * carries that header too and the provider's pact state seeds a session for it.
 * `target_version: 0` is a fresh create (server inserts at version 1), the
 * example payload is valid base64 (the server rejects non-base64 with 400),
 * and `version`/`updated_at` use integer matchers because the server echoes
 * live DB values that the consumer cannot predict.
 *
 * Run: `pnpm --filter @vautr/client-sdk test:pact`
 *   - Spins a Pact mock provider, exercises the interaction, writes
 *     `pacts/sync-push-batch.json` (the consumer contract artifact).
 * Provider verification (server side) is wired in `.github/workflows/verify-isolation.yml`
 * (nightly) and requires the Pact standalone verifier + a running `vautr-server`.
 */
import { join } from 'node:path';
import { MatchersV3, PactV3 } from '@pact-foundation/pact';
import { describe, expect, it } from 'vitest';

const provider = new PactV3({
  dir: join(__dirname, '..', '..', 'pacts'),
  consumer: 'vautr-client-sdk',
  provider: 'vautr-server',
});

const PACT_SESSION_TOKEN = 'vautr-pact-session-token';

const pushBatchItem = {
  uuid: '11111111-1111-1111-1111-111111111111',
  // 0 = fresh create; the server inserts the row at version 1 and returns it.
  target_version: 0,
  enc_key_gen: 1,
  payload: 'Y2lwaGVydGV4dC1ibG9i', // base64("ciphertext-blob")
  deleted_date: null,
};

const pushBatchResult = {
  uuid: MatchersV3.string('11111111-1111-1111-1111-111111111111'),
  status: MatchersV3.string('success'),
  // Server echoes the post-insert row; consumer only cares it is an integer.
  version: MatchersV3.integer(2),
  enc_key_gen: MatchersV3.integer(1),
  updated_at: MatchersV3.integer(1700000000),
};

describe('POST /sync/push-batch', () => {
  it('accepts a batch of pending item mutations and returns per-item results', async () => {
    await provider
      .uponReceiving('a push-batch of pending mutations')
      .withRequest({
        method: 'POST',
        path: '/sync/push-batch',
        headers: {
          Authorization: `Bearer ${PACT_SESSION_TOKEN}`,
        },
        body: {
          items: MatchersV3.eachLike(pushBatchItem, 1),
        },
      })
      .willRespondWith({
        status: 200,
        headers: { 'Content-Type': 'application/json' },
        body: {
          results: MatchersV3.eachLike(pushBatchResult, 1),
        },
      })
      .executeTest(async (mockserver) => {
        const base = (mockserver as unknown as { url: string }).url;
        // Mirrors realClient.ts: same path, same headers, same body shape.
        const res = await fetch(`${base}/sync/push-batch`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${PACT_SESSION_TOKEN}`,
          },
          body: JSON.stringify({ items: [pushBatchItem] }),
        });
        expect(res.status).toBe(200);
        const body = (await res.json()) as {
          results: {
            uuid: string;
            status: string;
            version: number;
            enc_key_gen: number;
            updated_at: number;
          }[];
        };
        expect(Array.isArray(body.results)).toBe(true);
        expect(body.results[0].uuid).toBe(pushBatchItem.uuid);
        expect(body.results[0].status).toBe('success');
        expect(body.results[0].version).toBeGreaterThan(0);
      });
  });

  // VTRFIX-BUG-C12: documents the OCC conflict result the SDK's conflict
  // re-target logic depends on. The server answers a stale `target_version`
  // with `status: "conflict"`, `version: null`, and the row it currently holds
  // in `current_server_state` (sync.rs `PushResult`), so the SDK can re-base
  // the pending item and land the local edit at `serverVersion + 1`.
  it('returns a conflict result carrying the server row state for a stale target_version', async () => {
    await provider
      .uponReceiving('a push-batch item whose target_version is stale')
      .withRequest({
        method: 'POST',
        path: '/sync/push-batch',
        headers: {
          Authorization: `Bearer ${PACT_SESSION_TOKEN}`,
        },
        body: {
          items: MatchersV3.eachLike({ ...pushBatchItem, target_version: 2 }, 1),
        },
      })
      .willRespondWith({
        status: 200,
        headers: { 'Content-Type': 'application/json' },
        body: {
          results: MatchersV3.eachLike(
            {
              uuid: MatchersV3.string(pushBatchItem.uuid),
              status: MatchersV3.string('conflict'),
              version: null,
              enc_key_gen: null,
              updated_at: null,
              current_server_state: MatchersV3.like({
                version: MatchersV3.integer(3),
                enc_key_gen: MatchersV3.integer(1),
              }),
            },
            1,
          ),
        },
      })
      .executeTest(async (mockserver) => {
        const base = (mockserver as unknown as { url: string }).url;
        const stalePush = { ...pushBatchItem, target_version: 2 };
        const res = await fetch(`${base}/sync/push-batch`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${PACT_SESSION_TOKEN}`,
          },
          body: JSON.stringify({ items: [stalePush] }),
        });
        expect(res.status).toBe(200);
        const body = (await res.json()) as {
          results: {
            uuid: string;
            status: string;
            current_server_state: { version: number } | null;
          }[];
        };
        expect(body.results[0]?.status).toBe('conflict');
        expect(body.results[0]?.current_server_state?.version).toBe(3);
      });
  });
});
