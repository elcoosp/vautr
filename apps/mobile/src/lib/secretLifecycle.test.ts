import type { MobileVautrClient, OpaqueHandle } from '@vautr/client-sdk/mobile';
import { describe, expect, it, vi } from 'vitest';

import { SecretHandleScope } from './secretLifecycle';

/**
 * Confirms the Detail screen's unmount contract (ui-state-charts §3): disposing
 * the handle scope invokes `release_secret` on the client, zeroizing the
 * in-memory secret. Complements the Rust FFI flow test
 * (`mobile_unlock_list_reveal_release_lock` in core/vautr-ffi).
 */
describe('SecretHandleScope (release_secret on unmount)', () => {
  it('calls client.release for the revealed handle on dispose', async () => {
    const release = vi.fn().mockResolvedValue(undefined);
    const client = { release } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    scope.set('7' satisfies OpaqueHandle);

    await scope.dispose();

    expect(release).toHaveBeenCalledTimes(1);
    expect(release).toHaveBeenCalledWith('7');
    expect(scope.current).toBeNull();
  });

  it('is idempotent — releases only once even if disposed again', async () => {
    const release = vi.fn().mockResolvedValue(undefined);
    const client = { release } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    scope.set('42' satisfies OpaqueHandle);
    await scope.dispose();
    await scope.dispose();

    expect(release).toHaveBeenCalledTimes(1);
  });

  it('does not release when reveal never resolved (no handle)', async () => {
    const release = vi.fn();
    const client = { release } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    await scope.dispose();

    expect(release).not.toHaveBeenCalled();
  });
});
