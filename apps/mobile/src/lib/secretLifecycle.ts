import type { MobileVautrClient, OpaqueHandle } from '@vautr/client-sdk/mobile';

/**
 * Tracks a revealed opaque handle across a screen's lifetime and guarantees
 * `release_secret` (zeroization) on dispose. The Detail screen calls `dispose`
 * from its `useEffect` unmount cleanup (ui-state-charts §3). The plaintext
 * secret never enters JS state — only the opaque u64-as-string handle lives here.
 */
export class SecretHandleScope {
  private handle: OpaqueHandle | null = null;

  constructor(private readonly client: MobileVautrClient) {}

  /** Record the handle returned by `reveal`. */
  set(handle: OpaqueHandle): void {
    this.handle = handle;
  }

  get current(): OpaqueHandle | null {
    return this.handle;
  }

  /**
   * Release the handle (`release_secret`), zeroizing the in-memory secret.
   * Idempotent: safe to call on unmount even if reveal never resolved.
   */
  async dispose(): Promise<void> {
    const handle = this.handle;
    this.handle = null;
    if (handle !== null) {
      await this.client.release(handle);
    }
  }
}
