/**
 * Pact consumer contract test for `POST /sync/push-batch` (VTR-037).
 *
 * Defines the request/response contract the TypeScript SDK (`@vautr/client-sdk`,
 * see `src/realClient.ts` `push-batch` call) expects from the Rust server
 * (`vautr-server`). The shapes below mirror the SDK's `PushBatchItem` /
 * `PushBatchResult` interfaces exactly, so any server-side drift in the
 * push-batch API breaks the provider verification step and fails CI.
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

const pushBatchItem = {
  uuid: '11111111-1111-1111-1111-111111111111',
  target_version: 1,
  enc_key_gen: 1,
  payload: 'encryption-blob-base64-or-null',
  deleted_date: null,
};

const pushBatchResult = {
  uuid: '11111111-1111-1111-1111-111111111111',
  status: 'success',
  version: 2,
  enc_key_gen: 1,
  updated_at: 1700000000,
};

describe('POST /sync/push-batch', () => {
  it('accepts a batch of pending item mutations and returns per-item results', async () => {
    await provider
      .uponReceiving('a push-batch of pending mutations')
      .withRequest({
        method: 'POST',
        path: '/sync/push-batch',
        headers: { 'Content-Type': 'application/json' },
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
        const res = await fetch(`${base}/sync/push-batch`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ items: [pushBatchItem] }),
        });
        expect(res.status).toBe(200);
        const body = (await res.json()) as { results: (typeof pushBatchResult)[] };
        expect(Array.isArray(body.results)).toBe(true);
        expect(body.results[0].uuid).toBe(pushBatchItem.uuid);
        expect(body.results[0].status).toBe('success');
      });
  });
});
