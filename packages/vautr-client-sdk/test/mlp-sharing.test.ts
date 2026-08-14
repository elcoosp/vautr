import { describe, expect, it } from 'vitest';
import type { ApiClient } from '../src/api';
import { VautrMlpClient } from '../src/mlp';

/**
 * SDK glue tests for the sharing endpoints (ADR-007). The crypto itself is
 * proven at the Rust level (core/vautr-wasm sharing tests + core/vautr-sharing
 * tests); these verify the `VautrMlpClient` builds the correct HTTP method,
 * path, and body for each sharing operation.
 */
function fakeApi(): ApiClient & { calls: Array<{ method: string; path: string; body?: unknown }> } {
  const calls: Array<{ method: string; path: string; body?: unknown }> = [];
  const api = {
    calls,
    async request<T>(method: string, path: string, body?: unknown): Promise<T> {
      calls.push({ method, path, body });
      if (path === '/users/u1/public-key' || path === '/users/me/public-key') {
        return { user_id: 'me', public_key: 'PK' } as unknown as T;
      }
      if (path === '/shares/') {
        return {
          share_id: 'item-1',
          sender_uuid: 'me',
          recipient_uuid: 'u1',
          item_uuid: 'item-1',
        } as unknown as T;
      }
      if (path === '/shares/item-1/payload') {
        return { share_id: 'item-1', status: 'ok' } as unknown as T;
      }
      if (path === '/shares/inbox') {
        return [
          {
            share_id: 'item-1',
            sender_uuid: 'u1',
            item_uuid: 'item-1',
            wrapped_sik: 'W',
            ephemeral_public_key: 'E',
            payload: 'P',
          },
        ] as unknown as T;
      }
      if (path === '/shares/item-1') {
        return { share_id: 'item-1', status: 'revoked' } as unknown as T;
      }
      return undefined as unknown as T;
    },
  };
  return api as unknown as typeof api;
}

describe('VautrMlpClient sharing endpoints', () => {
  it('publishes the sharing public key with PUT and caller id', async () => {
    const api = fakeApi();
    const mlp = new VautrMlpClient(api);
    const res = await mlp.publishSharingPublicKey('me', 'PK');
    expect(res.public_key).toBe('PK');
    expect(api.calls[0]).toEqual({
      method: 'PUT',
      path: '/users/me/public-key',
      body: { public_key: 'PK' },
    });
  });

  it('fetches a recipient public key', async () => {
    const api = fakeApi();
    const mlp = new VautrMlpClient(api);
    const res = await mlp.getSharingPublicKey('u1');
    expect(res.public_key).toBe('PK');
    expect(api.calls[0]).toEqual({ method: 'GET', path: '/users/u1/public-key' });
  });

  it('creates a share then uploads its payload', async () => {
    const api = fakeApi();
    const mlp = new VautrMlpClient(api);
    await mlp.createShare({
      item_uuid: 'item-1',
      recipient_uuid: 'u1',
      wrapped_sik: 'W',
      ephemeral_public_key: 'E',
    });
    await mlp.uploadSharePayload('item-1', 'PAYLOAD');
    expect(api.calls[0]).toEqual({
      method: 'POST',
      path: '/shares/',
      body: {
        item_uuid: 'item-1',
        recipient_uuid: 'u1',
        wrapped_sik: 'W',
        ephemeral_public_key: 'E',
      },
    });
    expect(api.calls[1]).toEqual({
      method: 'POST',
      path: '/shares/item-1/payload',
      body: { payload: 'PAYLOAD' },
    });
  });

  it('lists the share inbox', async () => {
    const api = fakeApi();
    const mlp = new VautrMlpClient(api);
    const inbox = await mlp.listShareInbox();
    expect(inbox).toHaveLength(1);
    expect(inbox[0].item_uuid).toBe('item-1');
    expect(api.calls[0]).toEqual({ method: 'GET', path: '/shares/inbox' });
  });

  it('revokes a share by item uuid', async () => {
    const api = fakeApi();
    const mlp = new VautrMlpClient(api);
    await mlp.revokeShare('item-1');
    expect(api.calls[0]).toEqual({ method: 'DELETE', path: '/shares/item-1' });
  });
});
