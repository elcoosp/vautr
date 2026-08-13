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

/**
 * VTR-048 (TDD4): the native overlay path. `view()` delegates the revealed
 * secret to the native overlay via `renderInOverlay(handle)` — the opaque
 * handle (as string) is the ONLY thing JS passes; the plaintext secret is
 * delivered to native code by the core and never enters the JS heap. `dispose`
 * still releases the handle (zeroization) on unmount.
 */
describe('SecretHandleScope.view (native overlay, VTR-048)', () => {
  it('calls client.renderInOverlay with the handle, never a plaintext secret', async () => {
    const renderInOverlay = vi.fn().mockResolvedValue(undefined);
    const release = vi.fn().mockResolvedValue(undefined);
    const client = { renderInOverlay, release } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    scope.set('9' satisfies OpaqueHandle);

    await scope.view();

    expect(renderInOverlay).toHaveBeenCalledTimes(1);
    expect(renderInOverlay).toHaveBeenCalledWith('9');
    // JS never receives the plaintext — the only value forwarded is the opaque
    // handle; the secret string is delivered to native by the core, not here.
    expect(release).not.toHaveBeenCalled();
  });

  it('is a no-op when no handle is recorded', async () => {
    const renderInOverlay = vi.fn().mockResolvedValue(undefined);
    const client = { renderInOverlay } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    await scope.view();

    expect(renderInOverlay).not.toHaveBeenCalled();
  });

  it('releases the handle on dispose after viewing (overlay unmount)', async () => {
    const renderInOverlay = vi.fn().mockResolvedValue(undefined);
    const release = vi.fn().mockResolvedValue(undefined);
    const client = { renderInOverlay, release } as unknown as MobileVautrClient;

    const scope = new SecretHandleScope(client);
    scope.set('11' satisfies OpaqueHandle);
    await scope.view();
    await scope.dispose();

    expect(release).toHaveBeenCalledTimes(1);
    expect(release).toHaveBeenCalledWith('11');
    expect(scope.current).toBeNull();
  });
});
