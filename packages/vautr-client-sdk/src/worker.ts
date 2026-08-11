/// <reference lib="webworker" />
import type { CoreAction, WorkerRequest, WorkerResponse } from './types';

/**
 * Vautr WASM worker bridge (client.md §4). This script runs the crypto-only
 * `WebClient` inside a Web Worker and routes requests/responses over
 * postMessage. Secrets never cross into the React main thread except through
 * the `clipboard` platform event, which the secure glue consumes directly.
 */
let client: import('vautr-wasm').WebClient | null = null;

async function ensureClient(): Promise<import('vautr-wasm').WebClient> {
  if (client) {
    return client;
  }
  const wasm = await import('vautr-wasm');
  const instance = new wasm.WebClient();
  // Route copy/autofill actions back to the main-thread secure glue. The
  // plaintext secret is delivered here and zeroized by the glue (client.md §3).
  instance.set_action_handler((actionJson, secret) => {
    const action = parseActionJson(actionJson);
    const response: WorkerResponse = { id: 'clipboard', action, secret };
    postMessage(response);
  });
  client = instance;
  return client;
}

function parseActionJson(json: string): CoreAction {
  try {
    const parsed: unknown = JSON.parse(json);
    if (
      parsed &&
      typeof parsed === 'object' &&
      'CopyToClipboard' in parsed &&
      (parsed as { CopyToClipboard: { handle: number } }).CopyToClipboard
    ) {
      return {
        type: 'CopyToClipboard',
        handle: String((parsed as { CopyToClipboard: { handle: number } }).CopyToClipboard.handle),
      };
    }
  } catch {
    // fall through to Autofill default
  }
  return { type: 'Autofill', handle: '0' };
}

function serializeAction(action: CoreAction): string {
  const handle = Number(action.handle);
  return action.type === 'CopyToClipboard'
    ? JSON.stringify({ CopyToClipboard: { handle } })
    : JSON.stringify({ Autofill: { handle } });
}

self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;
  void handle(request);
};

async function handle(request: WorkerRequest): Promise<void> {
  try {
    const c = await ensureClient();
    switch (request.method) {
      case 'unlock': {
        const rawKey = new Uint8Array(request.args.rawKey);
        c.unlock_with_raw_key(rawKey, request.args.localGen);
        respond(request.id, true, undefined);
        break;
      }
      case 'reveal': {
        const payload = new Uint8Array(request.args.payload);
        const handle = c.reveal_secret(request.args.uuid, request.args.encKeyGen, payload);
        respond(request.id, true, String(handle));
        break;
      }
      case 'perform': {
        const actionJson = serializeAction(request.args.action);
        c.perform_action(actionJson, Number(request.args.action.handle));
        respond(request.id, true, undefined);
        break;
      }
      case 'release': {
        c.release_secret(Number(request.args.handle));
        respond(request.id, true, undefined);
        break;
      }
      case 'encrypt': {
        const plaintext = new Uint8Array(request.args.plaintext);
        const envelope = c.encrypt_secret(request.args.uuid, request.args.encKeyGen, plaintext);
        respond(request.id, true, Array.from(envelope));
        break;
      }
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    respond(request.id, false, undefined, message);
  }
}

function respond(id: string, ok: boolean, result: unknown, error?: string): void {
  const response: WorkerResponse = ok
    ? { id, ok: true, result }
    : { id, ok: false, error: error ?? 'unknown error' };
  postMessage(response);
}
