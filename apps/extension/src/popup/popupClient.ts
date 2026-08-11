import { createClipboardHandler, VautrClient } from '@vautr/client-sdk';
import { attachClientToEventBus, vaultEventBus } from '@vautr/ui-logic';

/**
 * Popup-scoped client lifecycle (build-env-deploy §3.3).
 *
 * The popup instantiates a full `VautrClient` in a popup-scoped Web Worker
 * (`--target web`), exactly like the web app. When the popup closes the client
 * shuts down gracefully (worker terminate + pending rejections), preventing
 * state leakage and resource leaks.
 */
let client: VautrClient | null = null;

/** Lazily create the popup-scoped WASM worker client. */
export function getPopupClient(): VautrClient {
  if (!client) {
    const instance = new VautrClient();
    instance.setClipboardHandler(createClipboardHandler());
    attachClientToEventBus((listener) => instance.subscribe(listener), vaultEventBus);
    client = instance;
  }
  return client;
}

/** Gracefully shut down the popup worker (called on popup close). */
export function disposePopupClient(): void {
  if (client) {
    client.close();
    client = null;
  }
}
