import type {
  ClipboardHandler,
  CoreAction,
  OpaqueHandle,
  VaultStateUpdate,
  WorkerRequest,
  WorkerResponse,
} from './types';

const workerUrl = new URL('./worker.ts', import.meta.url);

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
}

/**
 * Promise-based client SDK wrapping the Vautr WASM worker bridge.
 *
 * Lifecycle: `new VautrClient()` spawns the worker. Every call returns a
 * Promise resolved when the worker replies. u64 values (handles, epochs) are
 * passed as strings (data.md §1 rule 1). Plaintext secrets only ever travel
 * through the `clipboard` platform event consumed by the registered handler and
 * are never retained here.
 *
 * This worker bridge is used by the browser-extension popup and other
 * `--target web` consumers. The web app itself drives the full client
 * (`@vautr/client-sdk/real`) for register/login/sync against the live server.
 */
export class VautrClient {
  private readonly worker: Worker;
  private readonly pending = new Map<string, PendingRequest>();
  private seq = 0;
  private listeners = new Set<(update: VaultStateUpdate) => void>();
  private clipboardHandler: ClipboardHandler | null = null;

  constructor(options: { worker?: Worker } = {}) {
    this.worker = options.worker ?? new Worker(workerUrl, { type: 'module' });
    this.worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
      const message = event.data;
      if ('action' in message) {
        // Platform event: route the secret directly to the secure glue. The
        // handler is responsible for writing and zeroizing; it never reaches
        // React state.
        if (this.clipboardHandler) {
          this.clipboardHandler(message.action, message.secret);
        }
        return;
      }
      const pending = this.pending.get(message.id);
      if (!pending) {
        return;
      }
      this.pending.delete(message.id);
      if (message.ok) {
        pending.resolve(message.result);
      } else {
        pending.reject(new Error(message.error));
      }
    };
    this.worker.onerror = (event) => {
      for (const pending of this.pending.values()) {
        pending.reject(new Error(event.message ?? 'worker error'));
      }
      this.pending.clear();
    };
  }

  /** Subscribe to `VaultStateUpdate` events pushed by the worker. */
  subscribe(listener: (update: VaultStateUpdate) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Install the clipboard/autofill platform adapter. */
  setClipboardHandler(handler: ClipboardHandler): void {
    this.clipboardHandler = handler;
  }

  /** Unlock the vault with a raw 32-byte key + local key generation. */
  unlock(rawKey: Uint8Array, localGen: number): Promise<void> {
    return this.request<void>('unlock', { rawKey, localGen });
  }

  /** Reveal a secret behind an opaque handle (u64 as string). */
  reveal(uuid: string, encKeyGen: number, payload: Uint8Array): Promise<OpaqueHandle> {
    return this.request<OpaqueHandle>('reveal', { uuid, encKeyGen, payload });
  }

  /** Delegate copy/autofill to the platform adapter. */
  performAction(action: CoreAction): Promise<void> {
    return this.request<void>('perform', { action });
  }

  /** Explicitly dispose a handle (zeroizes in-memory secret). */
  release(handle: OpaqueHandle): Promise<void> {
    return this.request<void>('release', { handle });
  }

  /** Re-encrypt a plaintext payload under the current DEK. */
  encrypt(uuid: string, encKeyGen: number, plaintext: Uint8Array): Promise<number[]> {
    return this.request<number[]>('encrypt', { uuid, encKeyGen, plaintext });
  }

  /** Terminate the worker (e.g. tab close / lock). */
  close(): void {
    this.worker.terminate();
    for (const pending of this.pending.values()) {
      pending.reject(new Error('client closed'));
    }
    this.pending.clear();
    this.listeners.clear();
  }

  private request<T>(method: WorkerRequest['method'], args: unknown): Promise<T> {
    const id = String(++this.seq);
    const payload: WorkerRequest = { id, method, args } as unknown as WorkerRequest;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      this.worker.postMessage(payload);
    });
  }
}

/** Default clipboard adapter: writes the secret to the OS clipboard and zeroizes. */
export function createClipboardHandler(): ClipboardHandler {
  return (action, secret) => {
    if (action.type === 'CopyToClipboard') {
      // The secret is a local variable; we overwrite it immediately after use.
      void navigator.clipboard.writeText(secret).then(() => {
        // Explicitly clear the local reference (best-effort zeroization).
        // The string may be interned by the engine, but we avoid retaining it.
        (secret as unknown) = '';
      });
    }
    // Autofill is out of scope for the web worker bridge in this milestone.
  };
}
