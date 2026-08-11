/**
 * Development mock of the `vautr-wasm` `WebClient` (crypto-only surface).
 *
 * The real module is produced by `wasm-pack` from `core/vautr-wasm`. This mock
 * lets the SPA run without a WASM build by mirroring the exact JS API the web
 * worker expects. It performs NO real cryptography; it only demonstrates the
 * opaque-handle / perform_action boundary (client.md §4). Swap the Vite alias
 * to the real built module to exercise actual crypto.
 */

type ActionHandler = (actionJson: string, secret: string) => void;

const DEMO_SECRET = 'vautr-demo-password-0x3f9a';

export class WebClient {
  private handler: ActionHandler | null = null;
  private secrets = new Map<number, string>();
  private next = 1;
  private unlocked = false;

  set_action_handler(handler: ActionHandler): void {
    this.handler = handler;
  }

  unlock_with_raw_key(_rawKey: Uint8Array, _localGen: number): void {
    this.unlocked = true;
  }

  reveal_secret(_uuid: string, _encKeyGen: number, _payload: Uint8Array): number {
    if (!this.unlocked) {
      throw new Error('vault is locked');
    }
    const id = this.next++;
    this.secrets.set(id, DEMO_SECRET);
    return id;
  }

  perform_action(actionJson: string, handle: number): void {
    const secret = this.secrets.get(handle);
    if (secret === undefined) {
      throw new Error('handle expired or unknown');
    }
    if (!this.handler) {
      throw new Error('no platform action handler registered');
    }
    this.handler(actionJson, secret);
  }

  release_secret(handle: number): void {
    this.secrets.delete(handle);
  }

  encrypt_secret(_uuid: string, _encKeyGen: number, plaintext: Uint8Array): Uint8Array {
    // Non-cryptographic round-trip for the demo. Real builds use AEAD.
    return new Uint8Array(plaintext);
  }
}
